use std::cell::Cell;
use std::io;
use std::marker::PhantomData;
use std::ptr::null_mut;
use std::rc::Rc;
use std::sync::Arc;

use thiserror::Error;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowThreadProcessId, HTTRANSPARENT, MA_NOACTIVATE, WM_MOUSEACTIVATE, WM_NCDESTROY,
    WM_NCHITTEST,
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowId};

/// Native policies shared by Winit and Bevy viewport owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeWindowPolicy {
    pub accepts_pointer_input: bool,
    pub no_focus_on_click: bool,
}

impl Default for NativeWindowPolicy {
    fn default() -> Self {
        Self {
            accepts_pointer_input: true,
            no_focus_on_click: false,
        }
    }
}

/// A failure installing or updating a native policy lease.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum WindowPolicyError {
    #[error("the Winit window handle is unavailable: {message}")]
    WindowHandleUnavailable { message: String },
    #[error("Winit returned a non-Win32 window handle on Windows")]
    UnexpectedHandleKind,
    #[error("the native window owner thread is unavailable")]
    WindowOwnerUnavailable,
    #[error(
        "the native window belongs to thread {owner_thread_id}, but the current thread is {current_thread_id}"
    )]
    WrongWindowThread {
        owner_thread_id: u32,
        current_thread_id: u32,
    },
    #[error("SetWindowSubclass failed: {message}")]
    InstallFailed { message: String },
    #[error("the native policy hook is no longer installed on its exact window")]
    HookDetached,
    #[error("the native window has already been destroyed")]
    WindowDestroyed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LeasePhase {
    Installed,
    Abandoned,
    Destroying,
    Detached,
    Destroyed,
}

struct LeaseState {
    hwnd: HWND,
    window_id: WindowId,
    subclass_id: usize,
    owner_thread_id: u32,
    phase: Cell<LeasePhase>,
    callback_ref_owned: Cell<bool>,
    accepts_pointer_input: std::sync::atomic::AtomicBool,
    no_focus_on_click: std::sync::atomic::AtomicBool,
}

/// An exact-window policy lease.
///
/// The lease is intentionally neither cloneable nor transferable between threads. Its callback
/// is installed on one HWND and can never be retargeted through this API. All operations must run
/// on the HWND's owner thread, as required by the Win32 subclass contract.
///
/// ```compile_fail
/// use dear_imgui_winit::native_support::WindowPolicyLease;
///
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<WindowPolicyLease>();
/// ```
///
/// ```compile_fail
/// use dear_imgui_winit::native_support::WindowPolicyLease;
///
/// fn requires_send_sync<T: Send + Sync>() {}
/// requires_send_sync::<WindowPolicyLease>();
/// ```
pub struct WindowPolicyLease {
    state: Arc<LeaseState>,
    // Win32 subclass procedures are thread-affine. This marker makes that contract explicit.
    _not_send_sync: PhantomData<Rc<()>>,
}

impl WindowPolicyLease {
    /// Installs a policy hook on the exact Winit window.
    pub fn install(window: &Window, policy: NativeWindowPolicy) -> Result<Self, WindowPolicyError> {
        let (hwnd, window_id) = window_handle(window)?;
        let owner_thread_id = window_owner_thread(hwnd)?;
        validate_owner_thread(owner_thread_id, current_thread_id())?;
        let state = Arc::new(LeaseState {
            hwnd,
            window_id,
            subclass_id: next_subclass_id(),
            owner_thread_id,
            phase: Cell::new(LeasePhase::Installed),
            callback_ref_owned: Cell::new(true),
            accepts_pointer_input: std::sync::atomic::AtomicBool::new(policy.accepts_pointer_input),
            no_focus_on_click: std::sync::atomic::AtomicBool::new(policy.no_focus_on_click),
        });
        let callback_ref = Arc::into_raw(Arc::clone(&state)) as usize;
        let installed = unsafe {
            SetWindowSubclass(
                hwnd,
                Some(window_policy_subclass),
                state.subclass_id,
                callback_ref,
            )
        };
        if installed == 0 {
            // The callback reference never became owned by USER32.
            unsafe { drop(Arc::from_raw(callback_ref as *const LeaseState)) };
            state.callback_ref_owned.set(false);
            return Err(WindowPolicyError::InstallFailed {
                message: io::Error::last_os_error().to_string(),
            });
        }
        Ok(Self {
            state,
            _not_send_sync: PhantomData,
        })
    }

    /// Updates policy bits without changing the leased HWND.
    pub fn update(&mut self, policy: NativeWindowPolicy) -> Result<(), WindowPolicyError> {
        match self.state.phase.get() {
            LeasePhase::Destroyed | LeasePhase::Destroying => {
                return Err(WindowPolicyError::WindowDestroyed);
            }
            LeasePhase::Detached | LeasePhase::Abandoned => {
                return Err(WindowPolicyError::HookDetached);
            }
            LeasePhase::Installed => {}
        }
        validate_state_thread(&self.state)?;
        use std::sync::atomic::Ordering;
        self.state
            .accepts_pointer_input
            .store(policy.accepts_pointer_input, Ordering::Release);
        self.state
            .no_focus_on_click
            .store(policy.no_focus_on_click, Ordering::Release);
        Ok(())
    }

    /// Returns whether the lease still refers to this exact Winit window.
    pub fn matches_window(&self, window: &Window) -> bool {
        self.state.phase.get() == LeasePhase::Installed
            && current_thread_id() == self.state.owner_thread_id
            && window.id() == self.state.window_id
            && window_handle(window).is_ok_and(|(hwnd, _)| hwnd == self.state.hwnd)
            && window_owner_thread(self.state.hwnd)
                .is_ok_and(|owner| owner == self.state.owner_thread_id)
    }
}

impl Drop for WindowPolicyLease {
    fn drop(&mut self) {
        if self.state.phase.get() != LeasePhase::Installed {
            return;
        }
        // Drop cannot report an error. If the caller violates the thread-affinity contract,
        // leave the callback-owned Arc for WM_NCDESTROY instead of risking a use-after-free.
        if validate_state_thread(&self.state).is_err() {
            self.state.phase.set(LeasePhase::Abandoned);
            return;
        }
        let removed = unsafe {
            RemoveWindowSubclass(
                self.state.hwnd,
                Some(window_policy_subclass),
                self.state.subclass_id,
            )
        } != 0;
        if removed {
            self.state.phase.set(LeasePhase::Detached);
            release_callback_ref(&self.state);
        } else {
            // A failed removal does not prove that USER32 stopped dispatching the callback.
            // Retain the raw Arc until the terminal WM_NCDESTROY callback.
            self.state.phase.set(LeasePhase::Abandoned);
        }
    }
}

fn release_callback_ref(state: &LeaseState) {
    if state.callback_ref_owned.replace(false) {
        // The raw pointer is the one passed as dwRefData. It is recovered only after USER32 has
        // stopped dispatching the subclass, or after WM_NCDESTROY has become terminal.
        let raw = state as *const LeaseState;
        unsafe { drop(Arc::from_raw(raw)) };
    }
}

fn current_thread_id() -> u32 {
    unsafe { GetCurrentThreadId() }
}

fn window_owner_thread(hwnd: HWND) -> Result<u32, WindowPolicyError> {
    let owner = unsafe { GetWindowThreadProcessId(hwnd, null_mut()) };
    (owner != 0)
        .then_some(owner)
        .ok_or(WindowPolicyError::WindowOwnerUnavailable)
}

fn validate_owner_thread(
    owner_thread_id: u32,
    current_thread_id: u32,
) -> Result<(), WindowPolicyError> {
    (owner_thread_id == current_thread_id).then_some(()).ok_or(
        WindowPolicyError::WrongWindowThread {
            owner_thread_id,
            current_thread_id,
        },
    )
}

fn validate_state_thread(state: &LeaseState) -> Result<(), WindowPolicyError> {
    let current = current_thread_id();
    validate_owner_thread(state.owner_thread_id, current)?;
    let owner = window_owner_thread(state.hwnd)?;
    if owner != state.owner_thread_id {
        return Err(WindowPolicyError::WrongWindowThread {
            owner_thread_id: owner,
            current_thread_id: current,
        });
    }
    Ok(())
}

fn window_handle(window: &Window) -> Result<(HWND, WindowId), WindowPolicyError> {
    let handle = window
        .window_handle()
        .map_err(|error| WindowPolicyError::WindowHandleUnavailable {
            message: error.to_string(),
        })?
        .as_raw();
    let RawWindowHandle::Win32(handle) = handle else {
        return Err(WindowPolicyError::UnexpectedHandleKind);
    };
    Ok((handle.hwnd.get() as HWND, window.id()))
}

fn next_subclass_id() -> usize {
    static NEXT_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
    loop {
        let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if id != 0 {
            return id;
        }
    }
}

unsafe extern "system" fn window_policy_subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    subclass_id: usize,
    reference_data: usize,
) -> LRESULT {
    // `reference_data` is private state installed by SetWindowSubclass above. USER32 owns this
    // callback reference until RemoveWindowSubclass or the terminal WM_NCDESTROY path.
    let raw = reference_data as *const LeaseState;
    if raw.is_null() {
        return unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    }
    let state = unsafe { &*raw };
    if state.hwnd != hwnd || state.subclass_id != subclass_id {
        return unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    }

    // Hold a temporary Arc while the callback executes. This protects the state if the owning
    // lease is dropped reentrantly from code reached by DefSubclassProc.
    unsafe { Arc::increment_strong_count(raw) };
    let callback_state = unsafe { Arc::from_raw(raw) };
    use std::sync::atomic::Ordering;

    if message == WM_NCDESTROY {
        // Mark destruction before forwarding: reentrant Drop must not call USER32 on a dying HWND.
        let terminal = matches!(
            callback_state.phase.replace(LeasePhase::Destroying),
            LeasePhase::Installed | LeasePhase::Abandoned
        );
        // USER32 may already have removed the subclass. Failure is harmless here because the
        // HWND is terminal; the callback-owned Arc is released only after DefSubclassProc.
        let _ = unsafe { RemoveWindowSubclass(hwnd, Some(window_policy_subclass), subclass_id) };
        let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
        if terminal {
            callback_state.phase.set(LeasePhase::Destroyed);
            release_callback_ref(&callback_state);
        }
        return result;
    }

    if matches!(
        callback_state.phase.get(),
        LeasePhase::Abandoned
            | LeasePhase::Detached
            | LeasePhase::Destroyed
            | LeasePhase::Destroying
    ) {
        return unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    }
    if message == WM_NCHITTEST && !callback_state.accepts_pointer_input.load(Ordering::Acquire) {
        return HTTRANSPARENT as LRESULT;
    }
    if message == WM_MOUSEACTIVATE && callback_state.no_focus_on_click.load(Ordering::Acquire) {
        return MA_NOACTIVATE as LRESULT;
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn policy_defaults_to_interactive_and_focusable() {
        assert_eq!(
            NativeWindowPolicy::default(),
            NativeWindowPolicy {
                accepts_pointer_input: true,
                no_focus_on_click: false,
            }
        );
    }

    #[test]
    fn wrong_thread_is_rejected_by_the_pure_seam() {
        assert_eq!(
            validate_owner_thread(7, 8),
            Err(WindowPolicyError::WrongWindowThread {
                owner_thread_id: 7,
                current_thread_id: 8,
            })
        );
        assert!(validate_owner_thread(7, 7).is_ok());
    }

    #[test]
    fn callback_state_changes_are_atomic() {
        let state = LeaseState {
            hwnd: std::ptr::null_mut(),
            window_id: WindowId::dummy(),
            subclass_id: 1,
            owner_thread_id: 1,
            phase: Cell::new(LeasePhase::Installed),
            callback_ref_owned: Cell::new(false),
            accepts_pointer_input: std::sync::atomic::AtomicBool::new(true),
            no_focus_on_click: std::sync::atomic::AtomicBool::new(false),
        };
        state.accepts_pointer_input.store(false, Ordering::Release);
        state.no_focus_on_click.store(true, Ordering::Release);
        assert!(!state.accepts_pointer_input.load(Ordering::Acquire));
        assert!(state.no_focus_on_click.load(Ordering::Acquire));
    }

    #[test]
    fn terminal_callback_ref_release_is_one_shot() {
        let state = LeaseState {
            hwnd: std::ptr::null_mut(),
            window_id: WindowId::dummy(),
            subclass_id: 1,
            owner_thread_id: 1,
            phase: Cell::new(LeasePhase::Destroyed),
            callback_ref_owned: Cell::new(false),
            accepts_pointer_input: std::sync::atomic::AtomicBool::new(true),
            no_focus_on_click: std::sync::atomic::AtomicBool::new(false),
        };
        release_callback_ref(&state);
        assert!(!state.callback_ref_owned.get());
    }
}

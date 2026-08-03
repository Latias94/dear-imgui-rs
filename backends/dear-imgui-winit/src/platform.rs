//! Main platform implementation for Dear ImGui winit backend
//!
//! This module contains the core `WinitPlatform` struct and its implementation
//! for integrating Dear ImGui with winit windowing.

use std::cell::{Cell, RefCell};
use std::ffi::{c_char, c_void};
use std::rc::Rc;
use std::sync::Arc;

#[cfg(feature = "multi-viewport")]
use dear_imgui_rs::ContextPlatformWindowTeardown;
use dear_imgui_rs::{
    BackendFlags, ConfigFlags, Context, ContextAttachment, ContextAttachmentError,
    ContextAttachmentHandle, ContextAttachmentLease, ContextAttachmentRole,
    ContextAttachmentTeardownError, ContextBinding, ContextBindingError, ContextDestroyed,
    ContextPlatformAttachmentReleaseError, ContextPlatformWindowTeardownError, ContextTeardown,
};
use thiserror::Error;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{Event, WindowEvent};
use winit::window::{Window, WindowAttributes};

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

use crate::cursor::CursorSettings;
use crate::events;
use crate::sanitize;

struct WinitPlatformAttachmentMarker;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlatformState {
    Active,
    Faulted,
    ShuttingDown,
    Detached,
    ContextDestroyed,
}

/// Failure to attach or operate the Winit platform backend.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum WinitPlatformError {
    /// The Dear ImGui Context rejected the platform attachment.
    #[error(transparent)]
    Attachment(#[from] ContextAttachmentError),
    /// The Context rejected release of the active platform attachment generation.
    #[error(transparent)]
    PlatformAttachmentRelease(#[from] ContextPlatformAttachmentReleaseError),
    /// The originating Dear ImGui Context can no longer be entered normally.
    #[error(transparent)]
    Context(#[from] ContextBindingError),
    /// Dear ImGui rejected an explicit platform-window teardown transaction.
    #[error(transparent)]
    PlatformWindowTeardown(#[from] ContextPlatformWindowTeardownError),
    /// The supplied Context is not the Context owned by this platform backend.
    #[error("the Winit platform backend belongs to a different Dear ImGui context")]
    ContextMismatch,
    /// Another platform backend already owns a required global field.
    #[error("Dear ImGui platform state `{field}` is already owned")]
    PlatformStateOccupied { field: &'static str },
    /// A field claimed by this platform backend changed while it remained attached.
    #[error("Dear ImGui platform state `{field}` changed while Winit was attached")]
    PlatformStateReplaced { field: &'static str },
    /// No main window has been attached to the platform backend.
    #[error("attach a main Winit window before using this operation")]
    WindowNotAttached,
    /// The supplied window is not the platform backend's attached main window.
    #[error("the Winit window does not match the platform backend's attached main window")]
    WindowMismatch,
    /// Multi-viewport support is already attached to this platform owner.
    #[error("Winit multi-viewport support is already attached")]
    RuntimeAlreadyAttached,
    /// A configuration mutation would invalidate the active multi-viewport coordinate contract.
    #[error("Winit platform configuration is locked while multi-viewport support is attached")]
    RuntimeConfigurationLocked,
    /// The build artifact lacks the aggregate callback bridge required by this backend.
    #[error("dear-imgui-sys was built without PlatformIO aggregate ABI hooks")]
    AggregateCallbackHooksUnavailable,
    /// Another platform backend already owns one of the required callback slots.
    #[error("ImGuiPlatformIO callback `{callback}` is already owned by another platform backend")]
    PlatformCallbackOccupied { callback: &'static str },
    /// Another platform backend already advertises a capability owned by this runtime.
    #[error("Dear ImGui backend capability `{flag}` is already owned by another platform backend")]
    PlatformCapabilityOccupied { flag: &'static str },
    /// A slot in the captured platform callback table changed while the runtime remained attached.
    #[error(
        "Winit platform callback table slot `{callback}` changed while the runtime was attached"
    )]
    PlatformCallbackReplaced { callback: &'static str },
    /// Platform teardown was requested before the renderer released its viewport callback.
    #[error("renderer state `{field}` is still installed; shut down the renderer before Winit")]
    RendererShutdownRequired { field: &'static str },
    /// A viewport already has platform data owned by another backend.
    #[error("viewport platform data or handle is already owned by another platform backend")]
    ForeignPlatformUserData,
    /// A live viewport stopped matching the Winit platform data registered for it.
    #[error("Winit lost ownership of viewport {viewport_id} field `{field}`")]
    ViewportOwnershipLost {
        /// Dear ImGui viewport identifier whose native platform state drifted.
        viewport_id: u32,
        /// Native platform field whose value no longer matches Winit's registration.
        field: &'static str,
    },
    /// Winit did not expose any monitor geometry that can back Dear ImGui viewports.
    #[error("Winit did not expose any monitor geometry")]
    NoMonitors,
    /// Winit exposed monitor geometry that violates Dear ImGui's platform contract.
    #[error("Winit monitor {monitor} is invalid: {reason}")]
    InvalidMonitorGeometry {
        monitor: usize,
        reason: &'static str,
    },
    /// Dear ImGui supplied viewport geometry that cannot be represented by Winit.
    #[error("Dear ImGui viewport geometry is invalid during {operation}: {reason}")]
    InvalidViewportGeometry {
        operation: &'static str,
        reason: &'static str,
    },
    /// Custom single-window coordinate scaling is not implemented for platform viewports.
    #[error("Winit multi-viewport requires HiDpiMode::Default")]
    CustomHiDpiModeUnsupported,
    /// Wayland cannot provide the desktop-space positioning required by Dear ImGui viewports.
    #[error("Wayland is unsupported by the Winit multi-viewport backend; use X11 on Linux")]
    WaylandUnsupported,
    /// The target has no supported native desktop window-system contract.
    #[error("the Winit multi-viewport backend does not support target `{target}`")]
    UnsupportedWindowSystem { target: &'static str },
    /// A requested viewport flag cannot be implemented faithfully for this operation.
    #[error("Winit cannot honor viewport flag `{flag}` during {operation}")]
    UnsupportedViewportFlag {
        flag: &'static str,
        operation: &'static str,
    },
    /// The monitor count cannot be represented by Dear ImGui's native vector.
    #[error("the Winit monitor count exceeds i32::MAX")]
    MonitorCountOverflow,
    /// Dear ImGui's allocator could not reserve monitor storage.
    #[error("Dear ImGui failed to allocate Winit monitor storage")]
    MonitorStorageAllocationFailed,
    /// Dear ImGui requested a new viewport outside a scoped Winit event-loop entry.
    #[error("Winit viewport creation requires WinitPlatformRuntime::with_event_loop")]
    EventLoopUnavailable,
    /// Winit failed to create a secondary viewport window.
    #[error("Winit failed to create a secondary viewport window: {message}")]
    WindowCreation { message: String },
    /// A fallible operation on a secondary Winit window failed.
    #[error("Winit viewport operation `{operation}` failed: {message}")]
    WindowOperation {
        operation: &'static str,
        message: String,
    },
    /// A Rust platform callback panicked; the panic was contained at the C ABI boundary.
    #[error("Winit platform callback `{callback}` panicked")]
    CallbackPanicked { callback: &'static str },
    /// The owning runtime has already shut down or entered a terminal fault.
    #[error("the Winit platform runtime is no longer attached")]
    RuntimeDetached,
    #[cfg(test)]
    #[error("injected Winit construction failure after {stage}")]
    InjectedConstructionFailure { stage: &'static str },
}

type SetImeDataCallback = unsafe extern "C" fn(
    *mut dear_imgui_rs::sys::ImGuiContext,
    *mut dear_imgui_rs::sys::ImGuiViewport,
    *mut dear_imgui_rs::sys::ImGuiPlatformImeData,
);

/// IME hook: Dear ImGui calls this when the text input caret moves. We forward
/// the position to winit so platforms that support it can position the IME
/// candidate/composition window near the caret.
unsafe extern "C" fn imgui_winit_set_ime_data(
    ctx: *mut dear_imgui_rs::sys::ImGuiContext,
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    data: *mut dear_imgui_rs::sys::ImGuiPlatformImeData,
) {
    use dear_imgui_rs::sys::{ImGuiPlatformImeData, ImGuiViewport};

    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        if viewport.is_null() || data.is_null() {
            return;
        }
        let Some(control) = platform_control_for_context(ctx) else {
            return;
        };
        let _ = control.binding.try_with_bound_context(|| {
            if let Err(error) = control.validate_complete_contract_in_current_context() {
                control.fail_current_contract(error);
                return;
            }
            #[cfg(feature = "multi-viewport")]
            let window = crate::multi_viewport::window_for_viewport(ctx, viewport)
                .or_else(|| control.attached_window().ok());
            #[cfg(not(feature = "multi-viewport"))]
            let window = control.attached_window().ok();
            let Some(window) = window else {
                return;
            };

            let ime: &ImGuiPlatformImeData = &*data;
            let vp: &ImGuiViewport = &*viewport;
            if !ime.WantVisible && !ime.WantTextInput {
                return;
            }
            let line_height = if ime.InputLineHeight > 0.0 {
                ime.InputLineHeight
            } else {
                16.0_f32
            };
            #[cfg(feature = "multi-viewport")]
            let area = if control.has_live_runtime() {
                crate::multi_viewport::ime_cursor_area_for_viewport(
                    &window,
                    [ime.InputPos.x, ime.InputPos.y],
                    [vp.Pos.x, vp.Pos.y],
                    line_height,
                )
            } else {
                Some((
                    LogicalPosition::new(
                        f64::from(ime.InputPos.x - vp.Pos.x),
                        f64::from(ime.InputPos.y - vp.Pos.y),
                    ),
                    LogicalSize::new(f64::from(line_height), f64::from(line_height)),
                ))
            };
            #[cfg(not(feature = "multi-viewport"))]
            let area = Some((
                LogicalPosition::new(
                    f64::from(ime.InputPos.x - vp.Pos.x),
                    f64::from(ime.InputPos.y - vp.Pos.y),
                ),
                LogicalSize::new(f64::from(line_height), f64::from(line_height)),
            ));
            if let Some((position, size)) = area {
                window.set_ime_cursor_area(position, size);
            }
        });
    }));
    if res.is_err()
        && let Some(control) = platform_control_for_context(ctx)
    {
        let _ = control.binding.try_with_bound_context(|| {
            control.fail_current_contract(WinitPlatformError::CallbackPanicked {
                callback: "Platform_SetImeDataFn",
            });
        });
    }
}

fn ime_callback_eq(left: Option<SetImeDataCallback>, right: Option<SetImeDataCallback>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => std::ptr::fn_addr_eq(left, right),
        (None, None) => true,
        _ => false,
    }
}

const WINIT_BASE_FLAGS: BackendFlags = BackendFlags::HAS_MOUSE_CURSORS;
#[cfg(all(feature = "multi-viewport", target_os = "windows"))]
pub(crate) const WINIT_VIEWPORT_FLAGS: BackendFlags =
    BackendFlags::PLATFORM_HAS_VIEWPORTS.union(BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT);
#[cfg(all(feature = "multi-viewport", not(target_os = "windows")))]
pub(crate) const WINIT_VIEWPORT_FLAGS: BackendFlags = BackendFlags::PLATFORM_HAS_VIEWPORTS;
#[cfg(not(feature = "multi-viewport"))]
pub(crate) const WINIT_VIEWPORT_FLAGS: BackendFlags = BackendFlags::empty();
const WINIT_RESERVED_FLAGS: BackendFlags = WINIT_BASE_FLAGS
    .union(BackendFlags::HAS_SET_MOUSE_POS)
    .union(WINIT_VIEWPORT_FLAGS)
    .union(BackendFlags::HAS_PARENT_VIEWPORT);

#[repr(C)]
struct PlatformOwnerToken {
    marker: u8,
}

fn winit_backend_name_ptr() -> *const c_char {
    concat!("dear-imgui-winit ", env!("CARGO_PKG_VERSION"), "\0")
        .as_ptr()
        .cast()
}

struct RegisteredPlatform {
    context_raw: usize,
    context_id: dear_imgui_rs::ContextId,
    control: std::rc::Weak<WinitPlatformControl>,
}

thread_local! {
    static PLATFORM_CONTROLS: RefCell<Vec<RegisteredPlatform>> = const { RefCell::new(Vec::new()) };
}

fn register_platform_control(control: &Rc<WinitPlatformControl>) {
    PLATFORM_CONTROLS.with(|controls| {
        let mut controls = controls.borrow_mut();
        controls.retain(|entry| entry.control.strong_count() > 0);
        controls.push(RegisteredPlatform {
            context_raw: control.context_raw as usize,
            context_id: control.binding.id(),
            control: Rc::downgrade(control),
        });
    });
}

fn unregister_platform_control(context_id: dear_imgui_rs::ContextId) {
    PLATFORM_CONTROLS.with(|controls| {
        controls
            .borrow_mut()
            .retain(|entry| entry.context_id != context_id);
    });
}

fn platform_control_for_context(
    context_raw: *mut dear_imgui_rs::sys::ImGuiContext,
) -> Option<Rc<WinitPlatformControl>> {
    if context_raw.is_null() {
        return None;
    }
    PLATFORM_CONTROLS.with(|controls| {
        let mut controls = controls.borrow_mut();
        controls.retain(|entry| entry.control.strong_count() > 0);
        controls
            .iter()
            .find(|entry| entry.context_raw == context_raw as usize)
            .and_then(|entry| entry.control.upgrade())
    })
}

pub(crate) struct WinitPlatformControl {
    context_raw: *mut dear_imgui_rs::sys::ImGuiContext,
    binding: ContextBinding,
    state: Cell<PlatformState>,
    token: Box<PlatformOwnerToken>,
    installed_name: Cell<*const c_char>,
    baseline_ime_callback: Cell<Option<SetImeDataCallback>>,
    baseline_ime_user_data: Cell<*mut c_void>,
    attached_window: RefCell<Option<Arc<Window>>>,
    ime_allowed: Cell<bool>,
    terminal_fault: RefCell<Option<WinitPlatformError>>,
    attachment_handle: RefCell<Option<ContextAttachmentHandle>>,
    #[cfg(feature = "multi-viewport")]
    runtime: RefCell<Option<Rc<crate::multi_viewport::RuntimeControl>>>,
    #[cfg(feature = "multi-viewport")]
    cursor_settings: Cell<Option<CursorSettings>>,
}

impl WinitPlatformControl {
    fn preflight(
        context: &Context,
    ) -> Result<(Option<SetImeDataCallback>, *mut c_void), WinitPlatformError> {
        context.binding().with_bound_context(|| unsafe {
            let io = dear_imgui_rs::sys::igGetIO_Nil();
            let platform_io = dear_imgui_rs::sys::igGetPlatformIO_Nil();
            if io.is_null() || platform_io.is_null() {
                return Err(WinitPlatformError::ContextMismatch);
            }
            for (occupied, field) in [
                (!(*io).BackendPlatformName.is_null(), "BackendPlatformName"),
                (
                    !(*io).BackendPlatformUserData.is_null(),
                    "BackendPlatformUserData",
                ),
                (
                    !(*platform_io).Platform_ImeUserData.is_null(),
                    "Platform_ImeUserData",
                ),
            ] {
                if occupied {
                    return Err(WinitPlatformError::PlatformStateOccupied { field });
                }
            }
            let occupied_flags =
                BackendFlags::from_bits_retain((*io).BackendFlags) & WINIT_RESERVED_FLAGS;
            if !occupied_flags.is_empty() {
                return Err(WinitPlatformError::PlatformStateOccupied {
                    field: "BackendFlags",
                });
            }
            Ok((
                (*platform_io).Platform_SetImeDataFn,
                (*platform_io).Platform_ImeUserData,
            ))
        })
    }

    fn claim(
        context: &mut Context,
    ) -> Result<(Rc<Self>, ContextAttachmentLease), WinitPlatformError> {
        let (baseline_ime_callback, baseline_ime_user_data) = Self::preflight(context)?;
        let control = Rc::new(Self {
            context_raw: context.as_raw(),
            binding: context.binding(),
            state: Cell::new(PlatformState::Active),
            token: Box::new(PlatformOwnerToken { marker: 1 }),
            installed_name: Cell::new(std::ptr::null()),
            baseline_ime_callback: Cell::new(baseline_ime_callback),
            baseline_ime_user_data: Cell::new(baseline_ime_user_data),
            attached_window: RefCell::new(None),
            ime_allowed: Cell::new(false),
            terminal_fault: RefCell::new(None),
            attachment_handle: RefCell::new(None),
            #[cfg(feature = "multi-viewport")]
            runtime: RefCell::new(None),
            #[cfg(feature = "multi-viewport")]
            cursor_settings: Cell::new(None),
        });
        let attachment = context.register_attachment::<WinitPlatformAttachmentMarker>(
            ContextAttachmentRole::Platform,
            Rc::clone(&control) as Rc<dyn ContextAttachment>,
        )?;
        control
            .attachment_handle
            .borrow_mut()
            .replace(attachment.handle());

        context.binding().with_bound_context(|| unsafe {
            let io = dear_imgui_rs::sys::igGetIO_Nil();
            (*io).BackendPlatformName = winit_backend_name_ptr();
            control.installed_name.set(winit_backend_name_ptr());
            (*io).BackendPlatformUserData = control.token_ptr();
            (*io).BackendFlags |= WINIT_BASE_FLAGS.bits();
        });
        register_platform_control(&control);
        Ok((control, attachment))
    }

    fn token_ptr(&self) -> *mut c_void {
        std::ptr::from_ref::<PlatformOwnerToken>(&self.token)
            .cast_mut()
            .cast()
    }

    #[cfg(test)]
    pub(crate) fn binding(&self) -> &ContextBinding {
        &self.binding
    }

    fn expected_owned_flags(&self) -> BackendFlags {
        #[cfg(feature = "multi-viewport")]
        let viewport_flags = self
            .runtime
            .borrow()
            .as_ref()
            .filter(|runtime| !runtime.is_released())
            .map_or(BackendFlags::empty(), |_| WINIT_VIEWPORT_FLAGS);
        #[cfg(not(feature = "multi-viewport"))]
        let viewport_flags = BackendFlags::empty();
        WINIT_BASE_FLAGS | viewport_flags
    }

    pub(crate) fn ensure_context(&self, context: &Context) -> Result<(), WinitPlatformError> {
        if context.id() == self.binding.id() {
            Ok(())
        } else {
            Err(WinitPlatformError::ContextMismatch)
        }
    }

    fn ensure_active(&self) -> Result<(), WinitPlatformError> {
        if let Some(error) = self.terminal_fault.borrow().clone() {
            return Err(error);
        }
        match self.state.get() {
            PlatformState::Active => Ok(()),
            PlatformState::Faulted
            | PlatformState::ShuttingDown
            | PlatformState::Detached
            | PlatformState::ContextDestroyed => Err(WinitPlatformError::RuntimeDetached),
        }
    }

    fn validate_base_contract_in_current_context(&self) -> Result<(), WinitPlatformError> {
        self.ensure_active()?;
        unsafe {
            if dear_imgui_rs::sys::igGetCurrentContext() != self.context_raw {
                return Err(WinitPlatformError::ContextMismatch);
            }
            let io = dear_imgui_rs::sys::igGetIO_Nil();
            let platform_io = dear_imgui_rs::sys::igGetPlatformIO_Nil();
            if io.is_null() || platform_io.is_null() {
                return Err(WinitPlatformError::ContextMismatch);
            }
            for (matches, field) in [
                (
                    (*io).BackendPlatformName == self.installed_name.get(),
                    "BackendPlatformName",
                ),
                (
                    (*io).BackendPlatformUserData == self.token_ptr(),
                    "BackendPlatformUserData",
                ),
                (
                    (BackendFlags::from_bits_retain((*io).BackendFlags) & WINIT_RESERVED_FLAGS)
                        == self.expected_owned_flags(),
                    "BackendFlags",
                ),
            ] {
                if !matches {
                    return Err(WinitPlatformError::PlatformStateReplaced { field });
                }
            }

            let attached_window = self.attached_window.borrow();
            let expected_window = attached_window.as_ref().map(Arc::as_ptr);
            let callback_matches = match ((*platform_io).Platform_SetImeDataFn, expected_window) {
                (actual, None) => ime_callback_eq(actual, self.baseline_ime_callback.get()),
                (Some(actual), Some(_)) => {
                    std::ptr::fn_addr_eq(actual, imgui_winit_set_ime_data as SetImeDataCallback)
                }
                _ => false,
            };
            if !callback_matches {
                return Err(WinitPlatformError::PlatformStateReplaced {
                    field: "Platform_SetImeDataFn",
                });
            }
            let expected_user_data = expected_window
                .map_or(self.baseline_ime_user_data.get(), |window| {
                    window.cast_mut().cast()
                });
            if (*platform_io).Platform_ImeUserData != expected_user_data {
                return Err(WinitPlatformError::PlatformStateReplaced {
                    field: "Platform_ImeUserData",
                });
            }
        }
        Ok(())
    }

    pub(crate) fn validate_complete_contract_in_current_context(
        &self,
    ) -> Result<(), WinitPlatformError> {
        self.validate_base_contract_in_current_context()?;
        #[cfg(feature = "multi-viewport")]
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            runtime.validate_publication_contract_in_current_context()?;
        }
        Ok(())
    }

    pub(crate) fn validate_operational_contract(&self) -> Result<(), WinitPlatformError> {
        self.binding.try_with_bound_context(|| {
            let result = self.validate_complete_contract_in_current_context();
            if let Err(error) = &result {
                self.fail_current_contract(error.clone());
            }
            result
        })?
    }

    pub(crate) fn terminal_fault(&self) -> Option<WinitPlatformError> {
        if matches!(
            self.state.get(),
            PlatformState::Detached | PlatformState::ContextDestroyed
        ) {
            None
        } else {
            self.terminal_fault.borrow().clone()
        }
    }

    pub(crate) fn owns_base_publication_in_current_context(&self) -> bool {
        unsafe {
            if dear_imgui_rs::sys::igGetCurrentContext() != self.context_raw {
                return false;
            }
            let io = dear_imgui_rs::sys::igGetIO_Nil();
            let platform_io = dear_imgui_rs::sys::igGetPlatformIO_Nil();
            if io.is_null() || platform_io.is_null() {
                return false;
            }
            if (*io).BackendPlatformName == self.installed_name.get()
                || (*io).BackendPlatformUserData == self.token_ptr()
            {
                return true;
            }
            let attached_window = self.attached_window.borrow();
            let Some(window) = attached_window.as_ref() else {
                return false;
            };
            let expected_window = Arc::as_ptr(window).cast_mut().cast::<c_void>();
            (*platform_io)
                .Platform_SetImeDataFn
                .is_some_and(|callback| {
                    std::ptr::fn_addr_eq(callback, imgui_winit_set_ime_data as SetImeDataCallback)
                })
                || (*platform_io).Platform_ImeUserData == expected_window
        }
    }

    fn owns_any_publication_or_callback_in_current_context(&self) -> bool {
        if self.owns_base_publication_in_current_context() {
            return true;
        }
        #[cfg(feature = "multi-viewport")]
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            return runtime.owns_any_platform_callback_in_current_context();
        }
        false
    }

    pub(crate) fn fail_current_contract(&self, error: WinitPlatformError) {
        let owns_winit_state = self.owns_any_publication_or_callback_in_current_context();
        if self.terminal_fault.borrow().is_none() {
            *self.terminal_fault.borrow_mut() = Some(error);
        }
        self.state.set(PlatformState::Faulted);
        #[cfg(feature = "multi-viewport")]
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            runtime.mark_faulted();
        }
        unsafe {
            if owns_winit_state && dear_imgui_rs::sys::igGetCurrentContext() == self.context_raw {
                let io = dear_imgui_rs::sys::igGetIO_Nil();
                if !io.is_null() {
                    (*io).BackendFlags &= !(WINIT_BASE_FLAGS | WINIT_VIEWPORT_FLAGS).bits();
                }
            }
        }
    }

    pub(crate) fn attached_window(&self) -> Result<Arc<Window>, WinitPlatformError> {
        self.attached_window
            .borrow()
            .clone()
            .ok_or(WinitPlatformError::WindowNotAttached)
    }

    pub(crate) fn set_ime_allowed_for_owned_windows(&self, allowed: bool) {
        self.ime_allowed.set(allowed);
        if let Some(window) = self.attached_window.borrow().as_ref() {
            window.set_ime_allowed(allowed);
        }
        #[cfg(feature = "multi-viewport")]
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            runtime.set_ime_allowed(allowed);
        }
    }

    #[cfg(feature = "multi-viewport")]
    pub(crate) fn has_live_runtime(&self) -> bool {
        self.runtime
            .borrow()
            .as_ref()
            .is_some_and(|runtime| !runtime.is_released())
    }

    pub(crate) fn attachment_handle(&self) -> Result<ContextAttachmentHandle, WinitPlatformError> {
        self.attachment_handle
            .borrow()
            .clone()
            .filter(ContextAttachmentHandle::is_attached)
            .ok_or(WinitPlatformError::RuntimeDetached)
    }

    #[cfg(feature = "multi-viewport")]
    fn note_runtime_key(&self, window_id: winit::window::WindowId, event: &winit::event::KeyEvent) {
        let Some(key) = crate::input::winit_key_to_imgui_key(&event.logical_key, event.location)
        else {
            return;
        };
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            runtime.note_key(window_id, key, event.state.is_pressed());
        }
    }

    #[cfg(feature = "multi-viewport")]
    fn note_runtime_modifiers(
        &self,
        window_id: winit::window::WindowId,
        modifiers: &winit::event::Modifiers,
    ) {
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            for (key, pressed) in crate::events::modifier_key_events(modifiers) {
                runtime.note_key(window_id, key, pressed);
            }
        }
    }

    #[cfg(feature = "multi-viewport")]
    fn note_runtime_mouse_button(
        &self,
        window_id: winit::window::WindowId,
        button: winit::event::MouseButton,
        state: winit::event::ElementState,
    ) {
        let Some(button) = crate::input::to_imgui_mouse_button(button) else {
            return;
        };
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            runtime.note_mouse_button(window_id, button, state.is_pressed());
        }
    }

    #[cfg(feature = "multi-viewport")]
    fn note_runtime_cursor_left(&self) {
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            runtime.note_cursor_left();
        }
    }

    #[cfg(feature = "multi-viewport")]
    fn note_runtime_cursor_available(&self) {
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            runtime.note_cursor_available();
        }
    }

    #[cfg(feature = "multi-viewport")]
    fn note_runtime_window_focus(
        &self,
        window_id: winit::window::WindowId,
        focused: bool,
        context: &mut Context,
    ) -> bool {
        let runtime = self
            .runtime
            .borrow()
            .as_ref()
            .filter(|runtime| !runtime.is_released())
            .cloned();
        let Some(runtime) = runtime else {
            return false;
        };
        runtime.note_window_focus(window_id, focused, context);
        true
    }

    #[cfg(feature = "multi-viewport")]
    fn note_runtime_window_geometry(
        &self,
        window_id: winit::window::WindowId,
        position: bool,
        size: bool,
    ) {
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            runtime.note_window_geometry(window_id, position, size);
        }
    }

    #[cfg(feature = "multi-viewport")]
    pub(crate) fn refresh_runtime_state(
        &self,
        context: &mut Context,
    ) -> Result<(), WinitPlatformError> {
        let runtime = self
            .runtime
            .borrow()
            .as_ref()
            .filter(|runtime| !runtime.is_released())
            .cloned();
        let Some(runtime) = runtime else {
            return Ok(());
        };
        runtime.reconcile_geometry_state();
        runtime.reconcile_input_state(context);
        let result = runtime.refresh_monitors(context);
        #[cfg(target_os = "windows")]
        let result = result.and_then(|()| runtime.refresh_native_mouse(context));
        if let Err(error) = result {
            self.fail_current_contract(error.clone());
            return Err(error);
        }
        Ok(())
    }

    #[cfg(feature = "multi-viewport")]
    pub(crate) fn set_cursor_settings(&self, settings: Option<CursorSettings>) {
        self.cursor_settings.set(settings);
        let Some(settings) = settings else {
            return;
        };
        if let Some(window) = self.attached_window.borrow().as_ref() {
            settings.apply(window);
        }
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            runtime.apply_cursor_settings(settings);
        }
    }

    #[cfg(feature = "multi-viewport")]
    pub(crate) fn apply_current_window_state(&self, window: &Window) {
        window.set_ime_allowed(self.ime_allowed.get());
        if let Some(settings) = self.cursor_settings.get() {
            settings.apply(window);
        }
    }

    fn ensure_window(&self, window: &Window) -> Result<Arc<Window>, WinitPlatformError> {
        let attached = self.attached_window()?;
        if std::ptr::eq(Arc::as_ptr(&attached), window) {
            Ok(attached)
        } else {
            Err(WinitPlatformError::WindowMismatch)
        }
    }

    fn validate_entry(&self, context: &Context, window: &Window) -> Result<(), WinitPlatformError> {
        self.ensure_context(context)?;
        self.ensure_window(window)?;
        self.validate_operational_contract()
    }

    fn validate_window_entry(&self, window: &Window) -> Result<(), WinitPlatformError> {
        self.ensure_window(window)?;
        self.validate_operational_contract()
    }

    fn attach_window(
        &self,
        context: &mut Context,
        window: Arc<Window>,
    ) -> Result<(), WinitPlatformError> {
        self.ensure_context(context)?;
        self.validate_operational_contract()?;
        self.binding.try_with_bound_context(|| {
            if let Some(existing) = self.attached_window.borrow().as_ref()
                && !Arc::ptr_eq(existing, &window)
            {
                return Err(WinitPlatformError::WindowMismatch);
            }
            unsafe {
                let platform_io = dear_imgui_rs::sys::igGetPlatformIO_Nil();
                (*platform_io).Platform_SetImeDataFn = Some(imgui_winit_set_ime_data);
                (*platform_io).Platform_ImeUserData = Arc::as_ptr(&window).cast_mut().cast();
            }
            self.attached_window.borrow_mut().replace(window);
            Ok(())
        })?
    }

    fn detach_window(
        &self,
        context: &mut Context,
        window: &Arc<Window>,
    ) -> Result<(), WinitPlatformError> {
        self.ensure_context(context)?;
        self.validate_operational_contract()?;
        self.binding.try_with_bound_context(|| {
            let attached = self.attached_window()?;
            if !Arc::ptr_eq(&attached, window) {
                return Err(WinitPlatformError::WindowMismatch);
            }
            unsafe {
                let platform_io = dear_imgui_rs::sys::igGetPlatformIO_Nil();
                (*platform_io).Platform_SetImeDataFn = self.baseline_ime_callback.get();
                (*platform_io).Platform_ImeUserData = self.baseline_ime_user_data.get();
            }
            self.attached_window.borrow_mut().take();
            Ok(())
        })?
    }

    #[cfg(feature = "multi-viewport")]
    pub(crate) fn install_runtime(
        &self,
        runtime: Rc<crate::multi_viewport::RuntimeControl>,
    ) -> Result<(), WinitPlatformError> {
        let mut slot = self.runtime.borrow_mut();
        if slot.is_some() {
            return Err(WinitPlatformError::RuntimeAlreadyAttached);
        }
        *slot = Some(runtime);
        Ok(())
    }

    #[cfg(feature = "multi-viewport")]
    pub(crate) fn clear_runtime(&self, runtime: &Rc<crate::multi_viewport::RuntimeControl>) {
        self.clear_runtime_by_control(runtime.as_ref());
    }

    #[cfg(feature = "multi-viewport")]
    fn clear_runtime_by_control(&self, runtime: &crate::multi_viewport::RuntimeControl) {
        let mut slot = self.runtime.borrow_mut();
        if slot
            .as_ref()
            .is_some_and(|installed| std::ptr::eq(installed.as_ref(), runtime))
        {
            slot.take();
        }
    }

    fn release_base_in_current_context(&self) -> Result<(), WinitPlatformError> {
        let mut replaced = None;
        let owns_base_publication = self.owns_base_publication_in_current_context();
        unsafe {
            let io = dear_imgui_rs::sys::igGetIO_Nil();
            let platform_io = dear_imgui_rs::sys::igGetPlatformIO_Nil();
            if io.is_null() || platform_io.is_null() {
                return Err(WinitPlatformError::ContextMismatch);
            }
            let attached_window = self
                .attached_window
                .borrow()
                .as_ref()
                .map(|window| Arc::as_ptr(window).cast_mut().cast::<c_void>());
            if let Some(expected_window) = attached_window {
                let callback_is_ours =
                    (*platform_io)
                        .Platform_SetImeDataFn
                        .is_some_and(|callback| {
                            std::ptr::fn_addr_eq(
                                callback,
                                imgui_winit_set_ime_data as SetImeDataCallback,
                            )
                        });
                if callback_is_ours {
                    (*platform_io).Platform_SetImeDataFn = self.baseline_ime_callback.get();
                } else {
                    replaced.get_or_insert("Platform_SetImeDataFn");
                }
                if (*platform_io).Platform_ImeUserData == expected_window {
                    (*platform_io).Platform_ImeUserData = self.baseline_ime_user_data.get();
                } else {
                    replaced.get_or_insert("Platform_ImeUserData");
                }
            } else {
                if !ime_callback_eq(
                    (*platform_io).Platform_SetImeDataFn,
                    self.baseline_ime_callback.get(),
                ) {
                    replaced.get_or_insert("Platform_SetImeDataFn");
                }
                if (*platform_io).Platform_ImeUserData != self.baseline_ime_user_data.get() {
                    replaced.get_or_insert("Platform_ImeUserData");
                }
            }
            let expected_flags = self.expected_owned_flags();
            if owns_base_publication
                && self.terminal_fault.borrow().is_none()
                && (BackendFlags::from_bits_retain((*io).BackendFlags) & WINIT_RESERVED_FLAGS)
                    != expected_flags
            {
                replaced.get_or_insert("BackendFlags");
            }
            if (*io).BackendPlatformUserData == self.token_ptr() {
                (*io).BackendPlatformUserData = std::ptr::null_mut();
            } else {
                replaced.get_or_insert("BackendPlatformUserData");
            }
            if (*io).BackendPlatformName == self.installed_name.get() {
                (*io).BackendPlatformName = std::ptr::null();
            } else {
                replaced.get_or_insert("BackendPlatformName");
            }
            if owns_base_publication {
                (*io).BackendFlags &= !WINIT_BASE_FLAGS.bits();
            }
        }
        if let Some(window) = self.attached_window.borrow_mut().take() {
            window.set_ime_allowed(false);
        }
        replaced.map_or(Ok(()), |field| {
            Err(WinitPlatformError::PlatformStateReplaced { field })
        })
    }
}

impl ContextAttachment for WinitPlatformControl {
    #[cfg(feature = "multi-viewport")]
    fn begin_platform_window_teardown(
        &self,
        context: &ContextPlatformWindowTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        let runtime = self.runtime.borrow().clone();
        context
            .with_bound_context(|| -> Result<(), WinitPlatformError> {
                // `WinitPlatformRuntime::shutdown` already performed its typed preflight and
                // opened this callback guard before it entered the core transaction. Preserve
                // that error path instead of wrapping it through the generic attachment error.
                if runtime
                    .as_ref()
                    .is_some_and(|runtime| runtime.teardown_callbacks_active())
                {
                    return Ok(());
                }

                self.validate_base_contract_in_current_context()?;
                if let Some(runtime) = runtime.as_ref() {
                    runtime.begin_context_platform_window_teardown()?;
                }
                Ok(())
            })
            .map_err(|error| ContextAttachmentTeardownError::new(error.to_string()))
    }

    #[cfg(feature = "multi-viewport")]
    fn end_platform_window_teardown(
        &self,
        context: &ContextPlatformWindowTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        let runtime = self.runtime.borrow().clone();
        let Some(runtime) = runtime else {
            return Ok(());
        };

        let result = context.with_bound_context(|| -> Result<(), WinitPlatformError> {
            runtime.finish_context_platform_window_teardown()?;
            Ok(())
        });
        if runtime.is_released() {
            self.clear_runtime_by_control(&runtime);
        }
        result.map_err(|error| ContextAttachmentTeardownError::new(error.to_string()))
    }

    fn quiesce(
        &self,
        _context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        self.state.set(PlatformState::ShuttingDown);
        #[cfg(feature = "multi-viewport")]
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            runtime.quiesce_from_platform();
        }
        Ok(())
    }

    fn release_platform_windows(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        #[cfg(feature = "multi-viewport")]
        if let Some(runtime) = self.runtime.borrow().as_ref() {
            runtime.release_from_platform_teardown(context)?;
        }
        context
            .with_bound_context(|| self.release_base_in_current_context())
            .map_err(|error| ContextAttachmentTeardownError::new(error.to_string()))
    }

    fn context_destroyed(&self, _context: ContextDestroyed) {
        #[cfg(feature = "multi-viewport")]
        if let Some(runtime) = self.runtime.borrow_mut().take() {
            runtime.context_destroyed_from_platform(_context);
        }
        self.attached_window.borrow_mut().take();
        unregister_platform_control(self.binding.id());
        self.attachment_handle.borrow_mut().take();
        self.state.set(PlatformState::ContextDestroyed);
    }
}

/// DPI scaling mode for the platform
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub enum HiDpiMode {
    /// Use the default DPI scaling
    #[default]
    Default,
    /// Use a custom scale factor
    Locked(f64),
    /// Round the scale factor to the nearest integer
    Rounded,
}

/// Main platform backend for Dear ImGui with winit integration
pub struct WinitPlatform {
    control: Rc<WinitPlatformControl>,
    attachment: Option<ContextAttachmentLease>,
    hidpi_mode: HiDpiMode,
    hidpi_factor: f64,
    cursor_cache: Option<CursorSettings>,
    ime_enabled: bool,
    ime_auto_manage: bool,
    last_frame: Instant,
}

impl WinitPlatform {
    /// Create a new winit platform backend
    ///
    /// # Example
    ///
    /// ```
    /// use dear_imgui_rs::Context;
    /// use dear_imgui_winit::WinitPlatform;
    ///
    /// let mut imgui_ctx = Context::create();
    /// let mut platform = WinitPlatform::new(&mut imgui_ctx).unwrap();
    /// ```
    pub fn new(imgui_ctx: &mut Context) -> Result<Self, WinitPlatformError> {
        let (control, attachment) = WinitPlatformControl::claim(imgui_ctx)?;
        Ok(Self {
            control,
            attachment: Some(attachment),
            hidpi_mode: HiDpiMode::default(),
            hidpi_factor: 1.0,
            cursor_cache: None,
            ime_enabled: false,
            ime_auto_manage: true,
            last_frame: Instant::now(),
        })
    }

    /// Set the DPI scaling mode.
    ///
    /// The mode is part of the primary-window coordinate mapping, so it cannot change while a
    /// multi-viewport runtime is attached. Secondary windows always use Winit's native desktop
    /// coordinate space.
    pub fn set_hidpi_mode(&mut self, hidpi_mode: HiDpiMode) -> Result<(), WinitPlatformError> {
        self.ensure_runtime_configuration_mutable()?;
        self.hidpi_mode = hidpi_mode;
        Ok(())
    }

    /// Return the configured DPI scaling mode.
    pub fn hidpi_mode(&self) -> HiDpiMode {
        self.hidpi_mode
    }

    /// Enable or disable IME events for the attached window.
    ///
    /// Winit does not deliver `WindowEvent::Ime` events unless IME is explicitly
    /// allowed on the window. When `ime_auto_manage` is enabled (default), the
    /// backend will override this based on `io.want_text_input()` every frame.
    /// Use this helper for immediate overrides (e.g. when auto-management is
    /// disabled or you want to force a specific state for a while).
    pub fn set_ime_allowed(&mut self, allowed: bool) -> Result<(), WinitPlatformError> {
        let window = self.control.attached_window()?;
        self.control.validate_window_entry(&window)?;
        self.control.set_ime_allowed_for_owned_windows(allowed);
        self.ime_enabled = allowed;
        Ok(())
    }

    /// Returns whether IME is currently allowed for the attached window.
    ///
    /// This reflects the last state set via `set_ime_allowed` or IME
    /// `WindowEvent::Ime(Enabled/Disabled)` notifications.
    pub fn ime_enabled(&self) -> bool {
        self.ime_enabled
    }

    /// Enable or disable automatic IME management.
    ///
    /// When enabled (default), the backend will call `set_ime_allowed` based on
    /// Dear ImGui's `io.want_text_input()` flag each frame, turning IME on
    /// while text widgets are active and off otherwise. When disabled, IME
    /// state is left entirely under application control.
    pub fn set_ime_auto_management(&mut self, enabled: bool) {
        self.ime_auto_manage = enabled;
    }

    /// Get the current DPI scaling factor
    pub fn hidpi_factor(&self) -> f64 {
        self.hidpi_factor
    }

    /// Attach the platform to a window.
    ///
    /// The platform keeps shared ownership of the exact window allocation until detach or
    /// Context teardown, so the native IME callback cannot outlive the Winit window.
    pub fn attach_window(
        &mut self,
        window: Arc<Window>,
        hidpi_mode: HiDpiMode,
        imgui_ctx: &mut Context,
    ) -> Result<(), WinitPlatformError> {
        // This must precede the IME attachment and every cached scale update. Reattaching the
        // same window with a different coordinate mode is still a contract violation once
        // secondary viewports exist.
        self.ensure_runtime_configuration_mutable()?;
        self.control.attach_window(imgui_ctx, Arc::clone(&window))?;
        self.hidpi_mode = hidpi_mode;
        self.hidpi_factor = self.hidpi_factor_for_window(&window);

        // Convert via winit scale then adapt to our active HiDPI mode
        let logical_size = window
            .inner_size()
            .to_logical(sanitize::positive_finite_or(window.scale_factor(), 1.0));
        let logical_size = self.scale_size_from_winit(&window, logical_size);
        let io = imgui_ctx.io_mut();

        io.set_display_size(sanitize::finite_non_negative_size(logical_size));
        io.set_display_framebuffer_scale(sanitize::framebuffer_scale(self.hidpi_factor, 1.0));

        // Enable IME by default so WindowEvent::Ime events and IME composition
        // are available on desktop platforms. Auto-management (when enabled)
        // will further refine this for text widgets.
        self.set_ime_allowed(true)?;
        Ok(())
    }

    /// Detach the platform from a window and clear winit-owned IME hooks.
    ///
    /// Multi-viewport support must be shut down before detaching the shared main window.
    pub fn detach_window(
        &mut self,
        imgui_ctx: &mut Context,
    ) -> Result<Arc<Window>, WinitPlatformError> {
        #[cfg(feature = "multi-viewport")]
        if self.control.runtime.borrow().is_some() {
            return Err(WinitPlatformError::RuntimeAlreadyAttached);
        }
        let window = self.control.attached_window()?;
        self.control.detach_window(imgui_ctx, &window)?;
        window.set_ime_allowed(false);
        self.ime_enabled = false;
        Ok(window)
    }

    /// Explicitly releases every Winit-owned field from the bound Context.
    ///
    /// Multi-viewport state, when present, is released first. The operation preserves foreign
    /// replacements and reports the first ownership violation after clearing fields that still
    /// have Winit's exact pointer identity. An active renderer attachment rejects shutdown before
    /// any frame or native state changes; shut down that renderer and retry.
    pub fn shutdown(&mut self, imgui_ctx: &mut Context) -> Result<(), WinitPlatformError> {
        self.control.ensure_context(imgui_ctx)?;
        if matches!(
            self.control.state.get(),
            PlatformState::Detached | PlatformState::ContextDestroyed
        ) {
            return Ok(());
        }
        let attachment = self.control.attachment_handle()?;
        let mut release = imgui_ctx.prepare_platform_attachment_release(&attachment)?;
        let _imgui_ctx = release.context_mut();
        let terminal_fault = self.control.terminal_fault();
        #[cfg(feature = "multi-viewport")]
        let pending_error = {
            let mut pending_error = None;
            if let Some(runtime) = self.control.runtime.borrow().clone() {
                if let Err(error) = runtime.shutdown_from_platform(_imgui_ctx) {
                    if !runtime.is_released() {
                        return Err(error);
                    }
                    pending_error = Some(error);
                }
                self.control.clear_runtime(&runtime);
            }
            pending_error
        };
        #[cfg(not(feature = "multi-viewport"))]
        let pending_error = None;

        let result = self
            .control
            .binding
            .try_with_bound_context(|| self.control.release_base_in_current_context())?;
        unregister_platform_control(self.control.binding.id());
        self.control.state.set(PlatformState::Detached);
        self.control.attachment_handle.borrow_mut().take();
        release.commit();
        self.attachment.take();
        match (terminal_fault.or(pending_error), result) {
            (Some(error), _) => Err(error),
            (None, result) => result,
        }
    }

    /// Returns the Context-bound owner shared with the multi-viewport runtime.
    #[cfg(any(feature = "multi-viewport", test))]
    pub(crate) fn control(&self) -> Rc<WinitPlatformControl> {
        Rc::clone(&self.control)
    }

    #[cfg(feature = "multi-viewport")]
    pub(crate) fn ensure_runtime_configuration_mutable(&self) -> Result<(), WinitPlatformError> {
        if self.control.runtime.borrow().is_some() {
            Err(WinitPlatformError::RuntimeConfigurationLocked)
        } else {
            Ok(())
        }
    }

    #[cfg(not(feature = "multi-viewport"))]
    pub(crate) fn ensure_runtime_configuration_mutable(&self) -> Result<(), WinitPlatformError> {
        Ok(())
    }

    /// Handle a winit event.
    ///
    /// This is the most general entry point: pass the full `Event<T>` from
    /// your event loop and the backend will dispatch to the appropriate
    /// handlers. For `ApplicationHandler::window_event`, where you already
    /// receive a `WindowEvent` for a specific window, you can use
    /// `handle_window_event` instead and avoid constructing a synthetic
    /// `Event::WindowEvent`.
    pub fn handle_event<T>(
        &mut self,
        imgui_ctx: &mut Context,
        window: &Window,
        event: &Event<T>,
    ) -> Result<bool, WinitPlatformError> {
        if !event_targets_window(window.id(), event) {
            return Ok(false);
        }
        self.control.validate_entry(imgui_ctx, window)?;
        Ok(match event {
            Event::WindowEvent { event, .. } => {
                self.handle_window_event_internal(imgui_ctx, window, event)
            }
            Event::DeviceEvent { event, .. } => {
                events::handle_device_event(event);
                false
            }
            _ => false,
        })
    }

    /// Handle a single window event for a given window.
    ///
    /// This is a convenience wrapper for frameworks that already route
    /// window-local events, such as winit's `ApplicationHandler::window_event`,
    /// and don't need to build a full `Event::WindowEvent` value.
    pub fn handle_window_event(
        &mut self,
        imgui_ctx: &mut Context,
        window: &Window,
        event: &WindowEvent,
    ) -> Result<bool, WinitPlatformError> {
        self.control.validate_entry(imgui_ctx, window)?;
        Ok(self.handle_window_event_internal(imgui_ctx, window, event))
    }

    /// Internal implementation for window event handling.
    fn handle_window_event_internal(
        &mut self,
        imgui_ctx: &mut Context,
        window: &Window,
        event: &WindowEvent,
    ) -> bool {
        match event {
            WindowEvent::Resized(physical_size) => {
                #[cfg(feature = "multi-viewport")]
                if self.control.has_live_runtime() {
                    self.control
                        .note_runtime_window_geometry(window.id(), false, true);
                    let io = imgui_ctx.io_mut();
                    io.set_display_size(crate::multi_viewport::desktop_size_for_window(window));
                    io.set_display_framebuffer_scale(
                        crate::multi_viewport::framebuffer_scale_for_window(window),
                    );
                    return false;
                }
                let logical_size = physical_size
                    .to_logical(sanitize::positive_finite_or(window.scale_factor(), 1.0));
                let logical_size = self.scale_size_from_winit(window, logical_size);
                imgui_ctx
                    .io_mut()
                    .set_display_size(sanitize::finite_non_negative_size(logical_size));
                false
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let new_hidpi = self.hidpi_factor_for_scale(*scale_factor);
                #[cfg(feature = "multi-viewport")]
                if self.control.has_live_runtime() {
                    self.control
                        .note_runtime_window_geometry(window.id(), true, true);
                    // Native desktop coordinates do not change when a viewport crosses a DPI
                    // boundary. Only its UI DPI and framebuffer relation change.
                    self.hidpi_factor = new_hidpi;
                    let io = imgui_ctx.io_mut();
                    io.set_display_size(crate::multi_viewport::desktop_size_for_window(window));
                    io.set_display_framebuffer_scale(
                        crate::multi_viewport::framebuffer_scale_for_window(window),
                    );
                    return false;
                }
                // Adjust mouse position proportionally when DPI factor changes
                {
                    let io = imgui_ctx.io_mut();
                    let mouse = io.mouse_pos();
                    if let Some(scaled) =
                        rescale_mouse_pos_for_hidpi_change(mouse, self.hidpi_factor, new_hidpi)
                    {
                        io.set_mouse_pos(scaled);
                    }
                }
                self.hidpi_factor = new_hidpi;

                let logical_size = window
                    .inner_size()
                    .to_logical(sanitize::positive_finite_or(window.scale_factor(), 1.0));
                let logical_size = self.scale_size_from_winit(window, logical_size);
                let io = imgui_ctx.io_mut();
                io.set_display_size(sanitize::finite_non_negative_size(logical_size));
                io.set_display_framebuffer_scale(sanitize::framebuffer_scale(
                    self.hidpi_factor,
                    1.0,
                ));
                false
            }
            WindowEvent::KeyboardInput { event, .. } => {
                #[cfg(feature = "multi-viewport")]
                if self.control.has_live_runtime() {
                    self.control.note_runtime_key(window.id(), event);
                }
                events::handle_keyboard_input(event, imgui_ctx)
            }
            WindowEvent::CursorMoved { position, .. } => {
                #[cfg(feature = "multi-viewport")]
                {
                    if self.control.has_live_runtime() {
                        self.control.note_runtime_cursor_available();
                        let Some(position) = crate::multi_viewport::client_physical_to_screen_pos(
                            window,
                            [position.x, position.y],
                        ) else {
                            return imgui_ctx.io().want_capture_mouse();
                        };
                        return events::handle_cursor_moved(
                            [f64::from(position[0]), f64::from(position[1])],
                            imgui_ctx,
                        );
                    }
                }
                // Fallback: local logical coordinates
                let position =
                    position.to_logical(sanitize::positive_finite_or(window.scale_factor(), 1.0));
                let position = self.scale_pos_from_winit(window, position);
                events::handle_cursor_moved([position.x, position.y], imgui_ctx)
            }
            WindowEvent::MouseInput { button, state, .. } => {
                #[cfg(feature = "multi-viewport")]
                if self.control.has_live_runtime() {
                    self.control
                        .note_runtime_mouse_button(window.id(), *button, *state);
                }
                events::handle_mouse_button(*button, *state, imgui_ctx)
            }
            WindowEvent::MouseWheel { delta, .. } => events::handle_mouse_wheel(*delta, imgui_ctx),
            // Single-window mode invalidates immediately. Multi-viewport mode delays the leave
            // so an in-flight drag can enter another owned native window without losing position.
            WindowEvent::CursorLeft { .. } => {
                #[cfg(feature = "multi-viewport")]
                if self.control.has_live_runtime() {
                    self.control.note_runtime_cursor_left();
                    return false;
                }
                {
                    let io = imgui_ctx.io_mut();
                    io.add_mouse_pos_event([-f32::MAX, -f32::MAX]);
                }
                false
            }
            #[cfg(feature = "multi-viewport")]
            WindowEvent::Moved(_) if self.control.has_live_runtime() => {
                self.control
                    .note_runtime_window_geometry(window.id(), true, false);
                false
            }
            WindowEvent::CursorEntered { .. } => {
                #[cfg(feature = "multi-viewport")]
                if self.control.has_live_runtime() {
                    self.control.note_runtime_cursor_available();
                }
                false
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                #[cfg(feature = "multi-viewport")]
                if self.control.has_live_runtime() {
                    self.control.note_runtime_modifiers(window.id(), modifiers);
                }
                events::handle_modifiers_changed(modifiers, imgui_ctx);
                false
            }
            WindowEvent::Ime(ime) => {
                events::handle_ime_event(ime, imgui_ctx);
                // Track IME enabled/disabled state based on winit notifications.
                self.ime_enabled = !matches!(ime, winit::event::Ime::Disabled);
                imgui_ctx.io().want_capture_keyboard()
            }
            WindowEvent::Touch(touch) => {
                #[cfg(feature = "multi-viewport")]
                if self.control.has_live_runtime() {
                    let position = crate::multi_viewport::client_physical_to_screen_pos(
                        window,
                        [touch.location.x, touch.location.y],
                    );
                    let _ = events::handle_touch_event_at(
                        touch,
                        position,
                        Some(imgui_ctx.main_viewport().id()),
                        imgui_ctx,
                    );
                    return imgui_ctx.io().want_capture_mouse();
                }
                events::handle_touch_event(touch, window, imgui_ctx);
                imgui_ctx.io().want_capture_mouse()
            }
            WindowEvent::Focused(focused) => {
                #[cfg(feature = "multi-viewport")]
                if self
                    .control
                    .note_runtime_window_focus(window.id(), *focused, imgui_ctx)
                {
                    return false;
                }
                events::handle_focused(*focused, imgui_ctx)
            }
            _ => false,
        }
    }

    /// Update frame timing and platform state before calling [`Context::frame`].
    pub fn prepare_frame(
        &mut self,
        imgui_ctx: &mut Context,
        window: &Window,
    ) -> Result<(), WinitPlatformError> {
        self.control.validate_entry(imgui_ctx, window)?;
        let now = Instant::now();
        let delta = now - self.last_frame;
        let delta_s = delta.as_secs() as f32 + delta.subsec_nanos() as f32 / 1_000_000_000.0;
        self.last_frame = now;

        imgui_ctx.io_mut().set_delta_time(delta_s);

        // Keep the main viewport's native desktop coordinate unit and framebuffer relation in
        // sync while an owning multi-viewport runtime is attached.
        #[cfg(feature = "multi-viewport")]
        {
            if self.control.has_live_runtime() {
                self.control.refresh_runtime_state(imgui_ctx)?;
                let winit_scale = sanitize::positive_finite_or(window.scale_factor(), 1.0);
                let hidpi = self.hidpi_factor_for_scale(winit_scale);
                self.hidpi_factor = hidpi;

                let io = imgui_ctx.io_mut();
                io.set_display_size(crate::multi_viewport::desktop_size_for_window(window));
                io.set_display_framebuffer_scale(
                    crate::multi_viewport::framebuffer_scale_for_window(window),
                );
            }
        }

        #[cfg(feature = "multi-viewport")]
        let runtime_owns_desktop_cursor = self.control.has_live_runtime();
        #[cfg(not(feature = "multi-viewport"))]
        let runtime_owns_desktop_cursor = false;

        // If backend supports setting mouse pos and ImGui requests it, honor it. Winit cannot set
        // a global desktop pointer, so a live multi-viewport runtime intentionally skips this.
        if imgui_ctx.io().want_set_mouse_pos() && !runtime_owns_desktop_cursor {
            let pos = imgui_ctx.io().mouse_pos();
            let logical_pos = self
                .scale_pos_for_winit(window, LogicalPosition::new(pos[0] as f64, pos[1] as f64));
            if let Some(pos) = sanitize::finite_position(logical_pos) {
                let _ = window.set_cursor_position(LogicalPosition::new(pos[0], pos[1]));
            }
        }
        // Cursor and IME state depend on the completed UI and are updated by `prepare_render`.
        Ok(())
    }

    /// Toggle Dear ImGui software-drawn cursor.
    /// When enabled, the OS cursor is hidden and ImGui draws the cursor in draw data.
    pub fn set_software_cursor_enabled(
        &mut self,
        imgui_ctx: &mut Context,
        enabled: bool,
    ) -> Result<(), WinitPlatformError> {
        self.control.ensure_context(imgui_ctx)?;
        self.control.validate_operational_contract()?;
        imgui_ctx.io_mut().set_mouse_draw_cursor(enabled);
        // Invalidate cursor cache so the next `prepare_render` applies the visibility change.
        self.cursor_cache = None;
        Ok(())
    }

    /// Update cursor and IME state after UI construction and before rendering.
    pub fn prepare_render(
        &mut self,
        ui: &dear_imgui_rs::Ui,
        window: &Window,
    ) -> Result<(), WinitPlatformError> {
        if ui.context_id() != self.control.binding.id() {
            return Err(WinitPlatformError::ContextMismatch);
        }
        self.control.validate_window_entry(window)?;
        // Auto-manage IME allowed state based on Dear ImGui's intent. This lets
        // the platform show/hide IME (and soft keyboards on mobile) only when
        // text input widgets are active.
        if self.ime_auto_manage {
            let want_text = ui.io().want_text_input();
            if want_text && !self.ime_enabled {
                self.control.set_ime_allowed_for_owned_windows(true);
                self.ime_enabled = true;
            } else if !want_text && self.ime_enabled {
                self.control.set_ime_allowed_for_owned_windows(false);
                self.ime_enabled = false;
            }
        }

        // Only change OS cursor if not disabled by config flags
        if !ui
            .io()
            .config_flags()
            .contains(ConfigFlags::NO_MOUSE_CURSOR_CHANGE)
        {
            // Our Io wrapper does not currently expose MouseDrawCursor, assume false (OS cursor)
            let cursor = CursorSettings {
                cursor: ui.mouse_cursor(),
                draw_cursor: ui.io().mouse_draw_cursor(),
            };
            if self.cursor_cache != Some(cursor) {
                #[cfg(feature = "multi-viewport")]
                self.control.set_cursor_settings(Some(cursor));
                #[cfg(not(feature = "multi-viewport"))]
                cursor.apply(window);
                self.cursor_cache = Some(cursor);
            }
        } else {
            self.cursor_cache = None;
            #[cfg(feature = "multi-viewport")]
            self.control.set_cursor_settings(None);
        }
        Ok(())
    }

    /// Scale a logical size from winit to our active HiDPI mode
    pub fn scale_size_from_winit(
        &self,
        window: &Window,
        logical_size: LogicalSize<f64>,
    ) -> LogicalSize<f64> {
        match self.hidpi_mode {
            HiDpiMode::Default => logical_size,
            // Convert to physical using winit scale, then back to logical with our factor
            _ => logical_size
                .to_physical::<f64>(sanitize::positive_finite_or(window.scale_factor(), 1.0))
                .to_logical(sanitize::positive_finite_or(self.hidpi_factor, 1.0)),
        }
    }

    /// Scale a logical position from winit to our active HiDPI mode
    pub fn scale_pos_from_winit(
        &self,
        window: &Window,
        logical_pos: LogicalPosition<f64>,
    ) -> LogicalPosition<f64> {
        match self.hidpi_mode {
            HiDpiMode::Default => logical_pos,
            _ => logical_pos
                .to_physical::<f64>(sanitize::positive_finite_or(window.scale_factor(), 1.0))
                .to_logical(sanitize::positive_finite_or(self.hidpi_factor, 1.0)),
        }
    }

    /// Scale a logical position for winit based on our active HiDPI mode
    pub fn scale_pos_for_winit(
        &self,
        window: &Window,
        logical_pos: LogicalPosition<f64>,
    ) -> LogicalPosition<f64> {
        match self.hidpi_mode {
            HiDpiMode::Default => logical_pos,
            _ => logical_pos
                .to_physical::<f64>(sanitize::positive_finite_or(self.hidpi_factor, 1.0))
                .to_logical(sanitize::positive_finite_or(window.scale_factor(), 1.0)),
        }
    }

    fn hidpi_factor_for_window(&self, window: &Window) -> f64 {
        self.hidpi_factor_for_scale(window.scale_factor())
    }

    fn hidpi_factor_for_scale(&self, scale_factor: f64) -> f64 {
        let scale_factor = sanitize::positive_finite_or(scale_factor, 1.0);
        match self.hidpi_mode {
            HiDpiMode::Default => scale_factor,
            HiDpiMode::Locked(factor) => sanitize::positive_finite_or(factor, 1.0),
            HiDpiMode::Rounded => sanitize::positive_finite_or(scale_factor.round(), 1.0),
        }
    }

    /// Create window attributes with Dear ImGui defaults
    pub fn create_window_attributes() -> WindowAttributes {
        WindowAttributes::default()
            .with_title("Dear ImGui Window")
            .with_inner_size(LogicalSize::new(1024.0, 768.0))
    }
}

impl Drop for WinitPlatform {
    fn drop(&mut self) {
        #[cfg(feature = "multi-viewport")]
        let runtime_attached = self.control.runtime.borrow().is_some();
        #[cfg(not(feature = "multi-viewport"))]
        let runtime_attached = false;
        let renderer_attached = self
            .attachment
            .as_ref()
            .is_some_and(|attachment| attachment.handle().has_active_renderer_dependency());
        if !runtime_attached
            && !renderer_attached
            && self
                .control
                .binding
                .try_with_bound_context(|| self.control.release_base_in_current_context())
                .is_ok()
        {
            unregister_platform_control(self.control.binding.id());
            self.control.state.set(PlatformState::Detached);
            if let Some(mut attachment) = self.attachment.take() {
                let _ = attachment
                    .detach()
                    .expect("Winit verified that no renderer attachment blocks platform detach");
            }
            return;
        }
        if let Some(attachment) = self.attachment.take() {
            attachment.defer_to_context();
        }
    }
}

fn event_targets_window<T>(window_id: winit::window::WindowId, event: &Event<T>) -> bool {
    !matches!(
        event,
        Event::WindowEvent {
            window_id: event_window_id,
            ..
        } if *event_window_id != window_id
    )
}

fn rescale_mouse_pos_for_hidpi_change(
    mouse: [f32; 2],
    old_hidpi: f64,
    new_hidpi: f64,
) -> Option<[f32; 2]> {
    let mouse = sanitize::finite_vec2_f32(mouse)?;
    let old_hidpi = sanitize::positive_finite_or(old_hidpi, 1.0);
    let scale = sanitize::positive_finite_or(new_hidpi / old_hidpi, 1.0);
    sanitize::finite_vec2_f32([mouse[0] * scale as f32, mouse[1] * scale as f32])
}

#[cfg(test)]
mod tests {
    use std::ffi::{CStr, CString};

    use super::*;
    use crate::test_util::test_sync::lock_context;

    struct ActiveRendererMarker;
    struct ActiveRendererAttachment;

    impl ContextAttachment for ActiveRendererAttachment {}

    unsafe extern "C" fn foreign_ime_callback(
        _context: *mut dear_imgui_rs::sys::ImGuiContext,
        _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
        _data: *mut dear_imgui_rs::sys::ImGuiPlatformImeData,
    ) {
    }

    #[test]
    fn test_hidpi_mode_default() {
        assert_eq!(HiDpiMode::default(), HiDpiMode::Default);
    }

    #[test]
    fn test_platform_creation() {
        let _guard = lock_context();
        let mut ctx = Context::create();
        let platform = WinitPlatform::new(&mut ctx).unwrap();

        assert_eq!(platform.hidpi_mode, HiDpiMode::Default);
        assert_eq!(platform.hidpi_factor, 1.0);
        assert_eq!(platform.cursor_cache, None);
        assert!(!platform.ime_enabled);
    }

    #[test]
    fn platform_shutdown_rejects_an_active_renderer_before_releasing_base_state() {
        let _guard = lock_context();
        let mut context = Context::create();
        let mut platform = WinitPlatform::new(&mut context).unwrap();
        let mut renderer = context
            .register_attachment::<ActiveRendererMarker>(
                ContextAttachmentRole::Renderer,
                Rc::new(ActiveRendererAttachment),
            )
            .unwrap();
        let io = unsafe { dear_imgui_rs::sys::igGetIO_ContextPtr(context.as_raw()) };

        assert!(matches!(
            platform.shutdown(&mut context),
            Err(WinitPlatformError::PlatformAttachmentRelease(
                ContextPlatformAttachmentReleaseError::RendererActive
            ))
        ));
        assert_eq!(
            unsafe { (*io).BackendPlatformUserData },
            platform.control.token_ptr()
        );
        assert!(platform.control.attachment_handle().unwrap().is_attached());

        assert_eq!(renderer.detach(), Ok(true));
        platform.shutdown(&mut context).unwrap();
    }

    #[test]
    fn platform_drop_defers_base_release_while_a_renderer_attachment_is_active() {
        let _guard = lock_context();
        let mut context = Context::create();
        let platform = WinitPlatform::new(&mut context).unwrap();
        let control = platform.control();
        let mut renderer = context
            .register_attachment::<ActiveRendererMarker>(
                ContextAttachmentRole::Renderer,
                Rc::new(ActiveRendererAttachment),
            )
            .unwrap();
        let io = unsafe { dear_imgui_rs::sys::igGetIO_ContextPtr(context.as_raw()) };

        drop(platform);

        assert_eq!(
            unsafe { (*io).BackendPlatformUserData },
            control.token_ptr()
        );
        assert!(control.attachment_handle().unwrap().is_attached());
        assert_eq!(renderer.detach(), Ok(true));
        drop(context);
        assert_eq!(control.state.get(), PlatformState::ContextDestroyed);
    }

    #[test]
    fn platform_claim_publishes_stable_identity_and_cleans_up_exact_ownership() {
        let _guard = lock_context();
        let mut context = Context::create();
        let platform_io =
            unsafe { dear_imgui_rs::sys::igGetPlatformIO_ContextPtr(context.as_raw()) };
        let baseline_ime_callback = unsafe { (*platform_io).Platform_SetImeDataFn };
        let baseline_ime_user_data = unsafe { (*platform_io).Platform_ImeUserData };

        let platform = WinitPlatform::new(&mut context).unwrap();
        let control = platform.control();
        let io = unsafe { dear_imgui_rs::sys::igGetIO_ContextPtr(context.as_raw()) };

        assert_ne!(std::mem::size_of::<PlatformOwnerToken>(), 0);
        assert_eq!(
            unsafe { (*io).BackendPlatformName },
            winit_backend_name_ptr()
        );
        assert_eq!(
            unsafe { (*io).BackendPlatformUserData },
            control.token_ptr()
        );
        assert_eq!(
            unsafe { CStr::from_ptr((*io).BackendPlatformName) },
            unsafe { CStr::from_ptr(winit_backend_name_ptr()) }
        );
        assert_eq!(
            BackendFlags::from_bits_retain(unsafe { (*io).BackendFlags }) & WINIT_RESERVED_FLAGS,
            WINIT_BASE_FLAGS
        );
        assert!(ime_callback_eq(
            unsafe { (*platform_io).Platform_SetImeDataFn },
            baseline_ime_callback
        ));
        assert_eq!(
            unsafe { (*platform_io).Platform_ImeUserData },
            baseline_ime_user_data
        );

        drop(platform);

        assert!(unsafe { (*io).BackendPlatformName.is_null() });
        assert!(unsafe { (*io).BackendPlatformUserData.is_null() });
        assert!(
            (BackendFlags::from_bits_retain(unsafe { (*io).BackendFlags }) & WINIT_RESERVED_FLAGS)
                .is_empty()
        );
        assert!(ime_callback_eq(
            unsafe { (*platform_io).Platform_SetImeDataFn },
            baseline_ime_callback
        ));
        assert_eq!(
            unsafe { (*platform_io).Platform_ImeUserData },
            baseline_ime_user_data
        );
    }

    #[test]
    fn platform_attachment_is_unique_per_context_and_reusable_after_release() {
        let _guard = lock_context();
        let mut context = Context::create();
        let platform = WinitPlatform::new(&mut context).unwrap();

        let error = match WinitPlatform::new(&mut context) {
            Ok(_) => panic!("a Context cannot have two Winit platform owners"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            WinitPlatformError::PlatformStateOccupied {
                field: "BackendPlatformName"
            }
        );

        drop(platform);
        drop(WinitPlatform::new(&mut context).unwrap());
    }

    #[test]
    fn base_contract_reports_each_replaced_owned_field() {
        let _guard = lock_context();
        let mut context = Context::create();
        let mut platform = WinitPlatform::new(&mut context).unwrap();
        let control = platform.control();
        let io = unsafe { dear_imgui_rs::sys::igGetIO_ContextPtr(context.as_raw()) };
        let platform_io =
            unsafe { dear_imgui_rs::sys::igGetPlatformIO_ContextPtr(context.as_raw()) };
        let baseline_ime_callback = unsafe { (*platform_io).Platform_SetImeDataFn };

        let validate = || {
            control
                .binding()
                .with_bound_context(|| control.validate_complete_contract_in_current_context())
        };

        unsafe { (*io).BackendPlatformName = std::ptr::null() };
        assert_eq!(
            validate(),
            Err(WinitPlatformError::PlatformStateReplaced {
                field: "BackendPlatformName"
            })
        );
        unsafe { (*io).BackendPlatformName = winit_backend_name_ptr() };

        unsafe { (*io).BackendPlatformUserData = std::ptr::null_mut() };
        assert_eq!(
            validate(),
            Err(WinitPlatformError::PlatformStateReplaced {
                field: "BackendPlatformUserData"
            })
        );
        unsafe { (*io).BackendPlatformUserData = control.token_ptr() };

        unsafe { (*io).BackendFlags &= !WINIT_BASE_FLAGS.bits() };
        assert_eq!(
            validate(),
            Err(WinitPlatformError::PlatformStateReplaced {
                field: "BackendFlags"
            })
        );
        unsafe { (*io).BackendFlags |= WINIT_BASE_FLAGS.bits() };

        unsafe { (*platform_io).Platform_SetImeDataFn = Some(foreign_ime_callback) };
        assert_eq!(
            validate(),
            Err(WinitPlatformError::PlatformStateReplaced {
                field: "Platform_SetImeDataFn"
            })
        );
        unsafe { (*platform_io).Platform_SetImeDataFn = baseline_ime_callback };

        let foreign_ime_user_data = std::ptr::dangling_mut::<u8>().cast();
        unsafe { (*platform_io).Platform_ImeUserData = foreign_ime_user_data };
        assert_eq!(
            validate(),
            Err(WinitPlatformError::PlatformStateReplaced {
                field: "Platform_ImeUserData"
            })
        );
        unsafe { (*platform_io).Platform_ImeUserData = std::ptr::null_mut() };

        platform.shutdown(&mut context).unwrap();
    }

    #[test]
    fn public_base_entry_latches_contract_drift_until_ordered_shutdown() {
        let _guard = lock_context();
        let mut context = Context::create();
        let mut platform = WinitPlatform::new(&mut context).unwrap();
        let io = unsafe { dear_imgui_rs::sys::igGetIO_ContextPtr(context.as_raw()) };
        unsafe { (*io).BackendPlatformName = std::ptr::null() };

        let expected = WinitPlatformError::PlatformStateReplaced {
            field: "BackendPlatformName",
        };
        assert_eq!(
            platform.set_software_cursor_enabled(&mut context, true),
            Err(expected.clone())
        );
        assert!(
            !BackendFlags::from_bits_retain(unsafe { (*io).BackendFlags })
                .contains(WINIT_BASE_FLAGS)
        );

        unsafe { (*io).BackendPlatformName = winit_backend_name_ptr() };
        assert_eq!(
            platform.set_software_cursor_enabled(&mut context, false),
            Err(expected.clone())
        );
        assert_eq!(platform.shutdown(&mut context), Err(expected));
        assert!(unsafe { (*io).BackendPlatformName.is_null() });
        assert!(unsafe { (*io).BackendPlatformUserData.is_null() });
        assert_eq!(platform.shutdown(&mut context), Ok(()));
    }

    #[test]
    fn shutdown_preserves_a_same_text_foreign_backend_name_pointer() {
        let _guard = lock_context();
        let mut context = Context::create();
        let mut platform = WinitPlatform::new(&mut context).unwrap();
        let io = unsafe { dear_imgui_rs::sys::igGetIO_ContextPtr(context.as_raw()) };
        let foreign_name = CString::new(
            unsafe { CStr::from_ptr(winit_backend_name_ptr()) }
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert_ne!(foreign_name.as_ptr(), winit_backend_name_ptr());
        unsafe { (*io).BackendPlatformName = foreign_name.as_ptr() };

        assert_eq!(
            platform.shutdown(&mut context),
            Err(WinitPlatformError::PlatformStateReplaced {
                field: "BackendPlatformName"
            })
        );
        assert_eq!(unsafe { (*io).BackendPlatformName }, foreign_name.as_ptr());
        assert_eq!(
            unsafe { CStr::from_ptr((*io).BackendPlatformName) },
            unsafe { CStr::from_ptr(winit_backend_name_ptr()) }
        );
        assert!(unsafe { (*io).BackendPlatformUserData.is_null() });

        unsafe { (*io).BackendPlatformName = std::ptr::null() };
        drop(WinitPlatform::new(&mut context).unwrap());
    }

    #[test]
    fn explicit_shutdown_preserves_complete_foreign_base_takeover_and_flags() {
        let _guard = lock_context();
        let mut context = Context::create();
        let mut platform = WinitPlatform::new(&mut context).unwrap();
        let io = unsafe { dear_imgui_rs::sys::igGetIO_ContextPtr(context.as_raw()) };
        let foreign_name = CString::new("foreign-platform").unwrap();
        let foreign_token = std::ptr::dangling_mut::<u8>().cast();
        unsafe {
            (*io).BackendPlatformName = foreign_name.as_ptr();
            (*io).BackendPlatformUserData = foreign_token;
        }

        assert_eq!(
            platform.shutdown(&mut context),
            Err(WinitPlatformError::PlatformStateReplaced {
                field: "BackendPlatformUserData"
            })
        );
        assert_eq!(unsafe { (*io).BackendPlatformName }, foreign_name.as_ptr());
        assert_eq!(unsafe { (*io).BackendPlatformUserData }, foreign_token);
        assert!(
            BackendFlags::from_bits_retain(unsafe { (*io).BackendFlags })
                .contains(WINIT_BASE_FLAGS)
        );

        unsafe {
            (*io).BackendPlatformName = std::ptr::null();
            (*io).BackendPlatformUserData = std::ptr::null_mut();
            (*io).BackendFlags &= !WINIT_BASE_FLAGS.bits();
        }
    }

    #[test]
    fn drop_preserves_complete_foreign_base_takeover_and_flags() {
        let _guard = lock_context();
        let mut context = Context::create();
        let io = unsafe { dear_imgui_rs::sys::igGetIO_ContextPtr(context.as_raw()) };
        let foreign_name = CString::new("foreign-platform").unwrap();
        let foreign_token = std::ptr::dangling_mut::<u8>().cast();
        let platform = WinitPlatform::new(&mut context).unwrap();
        unsafe {
            (*io).BackendPlatformName = foreign_name.as_ptr();
            (*io).BackendPlatformUserData = foreign_token;
        }

        drop(platform);

        assert_eq!(unsafe { (*io).BackendPlatformName }, foreign_name.as_ptr());
        assert_eq!(unsafe { (*io).BackendPlatformUserData }, foreign_token);
        assert!(
            BackendFlags::from_bits_retain(unsafe { (*io).BackendFlags })
                .contains(WINIT_BASE_FLAGS)
        );

        unsafe {
            (*io).BackendPlatformName = std::ptr::null();
            (*io).BackendPlatformUserData = std::ptr::null_mut();
            (*io).BackendFlags &= !WINIT_BASE_FLAGS.bits();
        }
    }

    #[test]
    fn complete_foreign_takeover_does_not_revoke_foreign_flags_on_contract_fault() {
        let _guard = lock_context();
        let mut context = Context::create();
        let mut platform = WinitPlatform::new(&mut context).unwrap();
        let io = unsafe { dear_imgui_rs::sys::igGetIO_ContextPtr(context.as_raw()) };
        let foreign_name = CString::new("foreign-platform").unwrap();
        let foreign_token = std::ptr::dangling_mut::<u8>().cast();
        unsafe {
            (*io).BackendPlatformName = foreign_name.as_ptr();
            (*io).BackendPlatformUserData = foreign_token;
        }

        let expected = WinitPlatformError::PlatformStateReplaced {
            field: "BackendPlatformName",
        };
        assert_eq!(
            platform.set_software_cursor_enabled(&mut context, true),
            Err(expected.clone())
        );
        assert!(
            BackendFlags::from_bits_retain(unsafe { (*io).BackendFlags })
                .contains(WINIT_BASE_FLAGS)
        );
        assert_eq!(platform.shutdown(&mut context), Err(expected));
        assert_eq!(unsafe { (*io).BackendPlatformName }, foreign_name.as_ptr());
        assert_eq!(unsafe { (*io).BackendPlatformUserData }, foreign_token);
        assert!(
            BackendFlags::from_bits_retain(unsafe { (*io).BackendFlags })
                .contains(WINIT_BASE_FLAGS)
        );

        unsafe {
            (*io).BackendPlatformName = std::ptr::null();
            (*io).BackendPlatformUserData = std::ptr::null_mut();
            (*io).BackendFlags &= !WINIT_BASE_FLAGS.bits();
        }
    }

    #[test]
    fn test_hidpi_mode_setting() {
        let _guard = lock_context();
        let mut ctx = Context::create();
        let mut platform = WinitPlatform::new(&mut ctx).unwrap();

        platform.set_hidpi_mode(HiDpiMode::Locked(2.0)).unwrap();
        assert_eq!(platform.hidpi_mode, HiDpiMode::Locked(2.0));

        platform.set_hidpi_mode(HiDpiMode::Rounded).unwrap();
        assert_eq!(platform.hidpi_mode, HiDpiMode::Rounded);
    }

    #[test]
    fn full_window_events_are_filtered_by_window_id_before_dispatch() {
        let target = winit::window::WindowId::from(41_u64);
        let foreign = winit::window::WindowId::from(42_u64);
        let foreign_event = Event::<()>::WindowEvent {
            window_id: foreign,
            event: WindowEvent::Focused(true),
        };
        let target_event = Event::<()>::WindowEvent {
            window_id: target,
            event: WindowEvent::Focused(true),
        };

        assert!(!event_targets_window(target, &foreign_event));
        assert!(event_targets_window(target, &target_event));
        assert!(event_targets_window(target, &Event::<()>::AboutToWait));
    }

    #[test]
    fn rescale_mouse_pos_for_hidpi_change_rejects_non_finite_results() {
        assert_eq!(
            rescale_mouse_pos_for_hidpi_change([10.0, 20.0], 1.0, 2.0),
            Some([20.0, 40.0])
        );
        assert_eq!(
            rescale_mouse_pos_for_hidpi_change([f32::NAN, 20.0], 1.0, 2.0),
            None
        );
        assert_eq!(
            rescale_mouse_pos_for_hidpi_change([10.0, 20.0], 0.0, 2.0),
            Some([20.0, 40.0])
        );
        assert_eq!(
            rescale_mouse_pos_for_hidpi_change([f32::MAX, 20.0], 1.0, f64::MAX),
            None
        );
    }

    #[test]
    fn test_window_attributes_creation() {
        let attrs = WinitPlatform::create_window_attributes();
        // Just test that it doesn't panic - actual values depend on winit defaults
        let _ = attrs;
    }
}

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::num::NonZeroU64;
use std::ptr;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::ThreadId;

use parking_lot::{Mutex, ReentrantMutex};
use thiserror::Error;

use crate::sys;

// All safe context switching, including backend callbacks, serializes through this lock.
pub(crate) static CTX_MUTEX: ReentrantMutex<()> = parking_lot::const_reentrant_mutex(());

static CONTEXT_THREAD_OWNER: Mutex<ContextThreadOwner> =
    parking_lot::const_mutex(ContextThreadOwner::new());

static NEXT_CONTEXT_ID: AtomicU64 = AtomicU64::new(1);

thread_local! {
    // Reusing an address replaces its entry, while guards that captured the old Weak retain the
    // old Context generation and cannot restore the reused address by mistake.
    static MANAGED_CONTEXTS: RefCell<HashMap<usize, ManagedContextEntry>> =
        RefCell::new(HashMap::new());
    static BOUND_CONTEXT_DEPTH: Cell<usize> = const { Cell::new(0) };
}

#[derive(Debug)]
struct ContextThreadOwner {
    thread: Option<ThreadId>,
    live_contexts: usize,
}

impl ContextThreadOwner {
    const fn new() -> Self {
        Self {
            thread: None,
            live_contexts: 0,
        }
    }
}

/// Process-global ownership of Dear ImGui's default `GImGui` storage.
#[derive(Debug)]
pub(crate) struct ContextThreadLease {
    thread: ThreadId,
}

impl ContextThreadLease {
    pub(crate) fn acquire() -> crate::error::ImGuiResult<Self> {
        let thread = std::thread::current().id();
        let mut owner = CONTEXT_THREAD_OWNER.lock();
        if owner.thread.is_some_and(|current| current != thread) {
            return Err(crate::error::ImGuiError::ContextThreadConflict);
        }
        owner.live_contexts = owner.live_contexts.checked_add(1).ok_or_else(|| {
            crate::error::ImGuiError::context_creation(
                "process Context ownership count is exhausted",
            )
        })?;
        owner.thread = Some(thread);
        Ok(Self { thread })
    }
}

impl Drop for ContextThreadLease {
    fn drop(&mut self) {
        let mut owner = CONTEXT_THREAD_OWNER.lock();
        debug_assert_eq!(owner.thread, Some(self.thread));
        debug_assert!(owner.live_contexts > 0);
        owner.live_contexts -= 1;
        if owner.live_contexts == 0 {
            owner.thread = None;
        }
    }
}

pub(super) fn bound_context_scope_active() -> bool {
    BOUND_CONTEXT_DEPTH.with(|depth| depth.get() != 0)
}

#[derive(Clone)]
enum ManagedContextEntry {
    Live {
        id: ContextId,
        state: Weak<ContextState>,
    },
    Dead {
        id: ContextId,
    },
}

/// Process-unique identity for a Dear ImGui context.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ContextId(NonZeroU64);

impl ContextId {
    pub(crate) fn allocate() -> Option<Self> {
        let value = NEXT_CONTEXT_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .ok()?;
        NonZeroU64::new(value).map(Self)
    }

    /// Returns the stable numeric identity assigned to this Context.
    pub fn get(self) -> NonZeroU64 {
        self.0
    }
}

/// Lifecycle visible to persistent safe Context capabilities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ContextLifecycle {
    /// The native Context accepts ordinary safe calls.
    Alive,
    /// Context teardown has started; only core-created teardown access is valid.
    Dropping,
    /// The native Context has been destroyed and its pointer is a tombstone.
    NativeDestroyed,
}

pub(crate) struct ContextState {
    id: ContextId,
    address: usize,
    raw: Cell<*mut sys::ImGuiContext>,
    lifecycle: Cell<ContextLifecycle>,
    dockspace_submissions: RefCell<FrameIdClaims>,
    dock_layout_applications: RefCell<FrameIdClaims>,
}

#[derive(Default)]
struct FrameIdClaims {
    frame: Option<i32>,
    ids: HashSet<sys::ImGuiID>,
}

impl FrameIdClaims {
    fn claim(&mut self, frame: i32, id: sys::ImGuiID) -> bool {
        if self.frame != Some(frame) {
            self.frame = Some(frame);
            self.ids.clear();
        }
        self.ids.insert(id)
    }

    fn release(&mut self, frame: i32, id: sys::ImGuiID) {
        if self.frame == Some(frame) {
            self.ids.remove(&id);
        }
    }
}

impl fmt::Debug for ContextState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContextState")
            .field("id", &self.id)
            .field("raw", &self.raw.get())
            .field("lifecycle", &self.lifecycle.get())
            .finish()
    }
}

impl ContextState {
    pub(crate) fn new(id: ContextId, raw: *mut sys::ImGuiContext) -> Rc<Self> {
        let state = Rc::new(Self {
            id,
            address: raw as usize,
            raw: Cell::new(raw),
            lifecycle: Cell::new(ContextLifecycle::Alive),
            dockspace_submissions: RefCell::new(FrameIdClaims::default()),
            dock_layout_applications: RefCell::new(FrameIdClaims::default()),
        });
        MANAGED_CONTEXTS.with(|contexts| {
            contexts.borrow_mut().insert(
                raw as usize,
                ManagedContextEntry::Live {
                    id,
                    state: Rc::downgrade(&state),
                },
            );
        });
        state
    }

    pub(crate) fn id(&self) -> ContextId {
        self.id
    }

    pub(crate) fn lifecycle(&self) -> ContextLifecycle {
        self.lifecycle.get()
    }

    pub(crate) fn raw_during_teardown(&self) -> *mut sys::ImGuiContext {
        self.raw.get()
    }

    pub(crate) fn begin_drop(&self) {
        debug_assert_eq!(self.lifecycle.get(), ContextLifecycle::Alive);
        self.lifecycle.set(ContextLifecycle::Dropping);
    }

    pub(crate) fn mark_native_destroyed(&self) {
        self.raw.set(ptr::null_mut());
        self.lifecycle.set(ContextLifecycle::NativeDestroyed);
    }
}

impl Drop for ContextState {
    fn drop(&mut self) {
        let _ = MANAGED_CONTEXTS.try_with(|contexts| {
            let mut contexts = contexts.borrow_mut();
            let Some(entry) = contexts.get_mut(&self.address) else {
                return;
            };
            let matches_generation = match entry {
                ManagedContextEntry::Live { id, .. } | ManagedContextEntry::Dead { id } => {
                    *id == self.id
                }
            };
            if matches_generation {
                *entry = ManagedContextEntry::Dead { id: self.id };
            }
        });
    }
}

/// Failure to enter a Context through a persistent binding capability.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum ContextBindingError {
    /// Context teardown has started, so ordinary safe access is no longer permitted.
    #[error("Dear ImGui context teardown is in progress")]
    Dropping,
    /// The originating native Context no longer exists.
    #[error("Dear ImGui context has been destroyed")]
    NativeDestroyed,
}

/// Persistent, non-thread-safe capability for calling against one live Context.
#[derive(Clone)]
#[must_use]
pub struct ContextBinding {
    state: Weak<ContextState>,
    id: ContextId,
}

impl fmt::Debug for ContextBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContextBinding")
            .field("id", &self.id)
            .field("lifecycle", &self.lifecycle())
            .finish()
    }
}

impl ContextBinding {
    pub(crate) fn new(state: &Rc<ContextState>) -> Self {
        Self {
            state: Rc::downgrade(state),
            id: state.id(),
        }
    }

    /// Returns the identity of the originating Context.
    pub fn id(&self) -> ContextId {
        self.id
    }

    /// Returns the latest observable lifecycle state.
    pub fn lifecycle(&self) -> ContextLifecycle {
        self.state
            .upgrade()
            .map_or(ContextLifecycle::NativeDestroyed, |state| state.lifecycle())
    }

    /// Returns true only while ordinary safe calls may enter the Context.
    pub fn is_alive(&self) -> bool {
        self.lifecycle() == ContextLifecycle::Alive
    }

    pub(crate) fn claim_dockspace_submission(
        &self,
        frame: i32,
        id: sys::ImGuiID,
    ) -> Option<DockspaceFrameClaim> {
        self.claim_dockspace_frame_id(frame, id, DockspaceClaimKind::Submission)
    }

    pub(crate) fn claim_dock_layout_application(
        &self,
        frame: i32,
        id: sys::ImGuiID,
    ) -> Option<DockspaceFrameClaim> {
        self.claim_dockspace_frame_id(frame, id, DockspaceClaimKind::LayoutApplication)
    }

    fn claim_dockspace_frame_id(
        &self,
        frame: i32,
        id: sys::ImGuiID,
        kind: DockspaceClaimKind,
    ) -> Option<DockspaceFrameClaim> {
        let state = self.state.upgrade()?;
        if state.lifecycle() != ContextLifecycle::Alive {
            return None;
        }

        let claimed = match kind {
            DockspaceClaimKind::Submission => {
                state.dockspace_submissions.borrow_mut().claim(frame, id)
            }
            DockspaceClaimKind::LayoutApplication => {
                state.dock_layout_applications.borrow_mut().claim(frame, id)
            }
        };
        if !claimed {
            return None;
        }

        Some(DockspaceFrameClaim {
            state: Rc::downgrade(&state),
            frame,
            id,
            kind,
            committed: false,
        })
    }

    /// Runs a closure while the originating Context is current.
    pub fn try_with_bound_context<R>(
        &self,
        f: impl FnOnce() -> R,
    ) -> Result<R, ContextBindingError> {
        self.try_with_bound_context_guarded(|_| f())
    }

    pub(crate) fn try_with_bound_context_guarded<R>(
        &self,
        f: impl FnOnce(&mut RawBoundContextGuard) -> R,
    ) -> Result<R, ContextBindingError> {
        let state = self
            .state
            .upgrade()
            .ok_or(ContextBindingError::NativeDestroyed)?;
        match state.lifecycle() {
            ContextLifecycle::Alive => {}
            ContextLifecycle::Dropping => return Err(ContextBindingError::Dropping),
            ContextLifecycle::NativeDestroyed => {
                return Err(ContextBindingError::NativeDestroyed);
            }
        }

        let _lock = CTX_MUTEX.lock();
        match state.lifecycle() {
            ContextLifecycle::Alive => {}
            ContextLifecycle::Dropping => return Err(ContextBindingError::Dropping),
            ContextLifecycle::NativeDestroyed => {
                return Err(ContextBindingError::NativeDestroyed);
            }
        }
        let raw = state.raw.get();
        if raw.is_null() {
            return Err(ContextBindingError::NativeDestroyed);
        }

        let mut bound = RawBoundContextGuard::bind(raw);
        Ok(f(&mut bound))
    }

    /// Runs a closure while the originating Context is current.
    ///
    /// # Panics
    ///
    /// Panics if Context teardown has started or the native Context was destroyed. Use
    /// [`ContextBinding::try_with_bound_context`] when teardown is an expected condition.
    pub fn with_bound_context<R>(&self, f: impl FnOnce() -> R) -> R {
        self.try_with_bound_context(f)
            .unwrap_or_else(|error| panic!("ContextBinding::with_bound_context(): {error}"))
    }
}

#[derive(Clone, Copy)]
enum DockspaceClaimKind {
    Submission,
    LayoutApplication,
}

pub(crate) struct DockspaceFrameClaim {
    state: Weak<ContextState>,
    frame: i32,
    id: sys::ImGuiID,
    kind: DockspaceClaimKind,
    committed: bool,
}

impl DockspaceFrameClaim {
    pub(crate) fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for DockspaceFrameClaim {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        let Some(state) = self.state.upgrade() else {
            return;
        };
        match self.kind {
            DockspaceClaimKind::Submission => state
                .dockspace_submissions
                .borrow_mut()
                .release(self.frame, self.id),
            DockspaceClaimKind::LayoutApplication => state
                .dock_layout_applications
                .borrow_mut()
                .release(self.frame, self.id),
        }
    }
}

/// A weak token that reports whether ordinary access to a Context is still valid.
#[derive(Clone, Debug)]
#[must_use]
pub struct ContextAliveToken(ContextBinding);

impl ContextAliveToken {
    pub(crate) fn from_binding(binding: ContextBinding) -> Self {
        Self(binding)
    }

    /// Returns true only while the originating Context is alive and not dropping.
    pub fn is_alive(&self) -> bool {
        self.0.is_alive()
    }
}

pub(crate) struct RawBoundContextGuard {
    target: *mut sys::ImGuiContext,
    target_state: Option<ManagedContextEntry>,
    previous: *mut sys::ImGuiContext,
    previous_state: Option<ManagedContextEntry>,
    restore: bool,
}

impl RawBoundContextGuard {
    pub(crate) fn bind(target: *mut sys::ImGuiContext) -> Self {
        BOUND_CONTEXT_DEPTH.with(|depth| {
            depth.set(
                depth
                    .get()
                    .checked_add(1)
                    .expect("Dear ImGui Context binding depth overflowed"),
            );
        });
        unsafe {
            let previous = sys::igGetCurrentContext();
            let restore = previous != target;
            let target_state = MANAGED_CONTEXTS
                .try_with(|contexts| contexts.borrow().get(&(target as usize)).cloned())
                .ok()
                .flatten();
            let previous_state = if restore {
                MANAGED_CONTEXTS
                    .try_with(|contexts| contexts.borrow().get(&(previous as usize)).cloned())
                    .ok()
                    .flatten()
            } else {
                None
            };
            if restore {
                sys::igSetCurrentContext(target);
            }
            Self {
                target,
                target_state,
                previous,
                previous_state,
                restore,
            }
        }
    }

    pub(crate) fn previous_context(&self) -> *mut sys::ImGuiContext {
        self.previous
    }

    pub(crate) fn restore_bound_target_or_clear(&mut self) {
        self.previous = self.target;
        self.previous_state = self.target_state.clone();
        self.restore = true;
    }
}

impl Drop for RawBoundContextGuard {
    fn drop(&mut self) {
        if self.restore {
            let previous_is_valid = match self.previous_state.as_ref() {
                None => true,
                Some(ManagedContextEntry::Live { id, state }) => {
                    state.upgrade().is_some_and(|state| {
                        state.id() == *id
                            && state.lifecycle() != ContextLifecycle::NativeDestroyed
                            && state.raw_during_teardown() == self.previous
                    })
                }
                Some(ManagedContextEntry::Dead { .. }) => false,
            };
            set_current_context(if previous_is_valid {
                self.previous
            } else {
                ptr::null_mut()
            });
        }
        BOUND_CONTEXT_DEPTH.with(|depth| {
            let current = depth.get();
            debug_assert!(current > 0);
            depth.set(current - 1);
        });
    }
}

pub(super) fn clear_current_context() {
    set_current_context(ptr::null_mut());
}

pub(super) fn set_current_context(ctx: *mut sys::ImGuiContext) {
    unsafe { sys::igSetCurrentContext(ctx) }
}

pub(super) fn no_current_context() -> bool {
    let ctx = unsafe { sys::igGetCurrentContext() };
    ctx.is_null()
}

pub(crate) fn with_bound_context<R>(ctx: *mut sys::ImGuiContext, f: impl FnOnce() -> R) -> R {
    let _lock = CTX_MUTEX.lock();
    let _bound = RawBoundContextGuard::bind(ctx);
    f()
}

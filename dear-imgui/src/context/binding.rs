use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::num::NonZeroU64;
use std::ptr;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::ReentrantMutex;
use thiserror::Error;

use crate::sys;

// All safe context switching, including backend callbacks, serializes through this lock.
pub(crate) static CTX_MUTEX: ReentrantMutex<()> = parking_lot::const_reentrant_mutex(());

static NEXT_CONTEXT_ID: AtomicU64 = AtomicU64::new(1);

thread_local! {
    // Reusing an address replaces its entry, while guards that captured the old Weak retain the
    // old Context generation and cannot restore the reused address by mistake.
    static MANAGED_CONTEXTS: RefCell<HashMap<usize, ManagedContextEntry>> =
        RefCell::new(HashMap::new());
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

    /// Runs a closure while the originating Context is current.
    pub fn try_with_bound_context<R>(
        &self,
        f: impl FnOnce() -> R,
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

        let _bound = RawBoundContextGuard::bind(raw);
        Ok(f())
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
    previous: *mut sys::ImGuiContext,
    previous_state: Option<ManagedContextEntry>,
    restore: bool,
}

impl RawBoundContextGuard {
    pub(crate) fn bind(target: *mut sys::ImGuiContext) -> Self {
        unsafe {
            let previous = sys::igGetCurrentContext();
            let restore = previous != target;
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
                previous,
                previous_state,
                restore,
            }
        }
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

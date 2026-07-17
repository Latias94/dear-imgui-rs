use std::any::TypeId;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::{Rc, Weak};

use thiserror::Error;

use super::binding::{CTX_MUTEX, ContextId, ContextLifecycle, ContextState, RawBoundContextGuard};

/// Ordered phase of Context teardown exposed to an attachment hook.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ContextAttachmentPhase {
    /// Stop new work and make callbacks inert.
    Quiesce,
    /// Release renderer-owned resources for secondary viewports.
    RendererResources,
    /// Destroy platform-owned secondary windows.
    PlatformWindows,
}

/// Exclusive role claimed by a Context attachment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ContextAttachmentRole {
    /// An extension without platform or renderer ordering requirements.
    Extension,
    /// The Context's renderer runtime.
    Renderer,
    /// The Context's platform runtime.
    Platform,
}

/// Type-erased lifecycle hooks owned by a Context.
///
/// Hooks must be idempotent. Panics are contained so one attachment cannot prevent the remaining
/// teardown phases from running. Explicit backend shutdown APIs remain responsible for reporting
/// actionable errors.
pub trait ContextAttachment {
    /// Stops new work before native teardown begins.
    fn quiesce(&self, _context: &ContextTeardown<'_>) {}

    /// Releases renderer resources before platform windows are destroyed.
    fn release_renderer_resources(&self, _context: &ContextTeardown<'_>) {}

    /// Releases platform windows before the native Context is destroyed.
    fn release_platform_windows(&self, _context: &ContextTeardown<'_>) {}

    /// Tombstones Rust state after the native Context has been destroyed.
    fn context_destroyed(&self, _context: ContextDestroyed) {}
}

/// Failure to register an attachment with a Context.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum ContextAttachmentError {
    /// An attachment with the same marker type is already active.
    #[error("an attachment with this marker type is already registered")]
    DuplicateAttachment,
    /// The exclusive platform or renderer role is already occupied.
    #[error("the {0:?} attachment role is already occupied")]
    RoleOccupied(ContextAttachmentRole),
    /// A renderer cannot attach until a platform runtime is registered.
    #[error("a renderer attachment requires an active platform attachment")]
    MissingPlatform,
    /// Context teardown has already started.
    #[error("Dear ImGui context teardown has already started")]
    ContextDropping,
}

/// Phase-limited access passed to pre-destroy attachment hooks.
pub struct ContextTeardown<'a> {
    state: &'a ContextState,
    phase: ContextAttachmentPhase,
}

impl fmt::Debug for ContextTeardown<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContextTeardown")
            .field("id", &self.id())
            .field("phase", &self.phase)
            .finish_non_exhaustive()
    }
}

impl ContextTeardown<'_> {
    /// Returns the Context identity being torn down.
    pub fn id(&self) -> ContextId {
        self.state.id()
    }

    /// Returns the currently executing teardown phase.
    pub fn phase(&self) -> ContextAttachmentPhase {
        self.phase
    }

    /// Runs a closure while the dropping Context is current.
    ///
    /// This capability is valid only for the duration of the current pre-destroy hook.
    pub fn with_bound_context<R>(&self, f: impl FnOnce() -> R) -> R {
        assert_eq!(
            self.state.lifecycle(),
            ContextLifecycle::Dropping,
            "ContextTeardown used outside pre-destroy teardown"
        );
        let raw = self.state.raw_during_teardown();
        assert!(
            !raw.is_null(),
            "ContextTeardown used after native Context destruction"
        );

        let _lock = CTX_MUTEX.lock();
        let _bound = RawBoundContextGuard::bind(raw);
        f()
    }

    #[cfg(test)]
    pub(super) fn as_raw_for_test(&self) -> *mut crate::sys::ImGuiContext {
        self.state.raw_during_teardown()
    }
}

/// Pointer-free notification passed after native Context destruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextDestroyed {
    id: ContextId,
}

impl ContextDestroyed {
    /// Returns the identity of the destroyed Context.
    pub fn id(self) -> ContextId {
        self.id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AttachmentState {
    Active,
    Teardown,
    Complete,
    Detached,
}

pub(super) struct AttachmentControl {
    marker: TypeId,
    role: ContextAttachmentRole,
    state: Cell<AttachmentState>,
    attachment: RefCell<Option<Rc<dyn ContextAttachment>>>,
}

impl AttachmentControl {
    fn detach(&self) -> bool {
        if self.state.get() != AttachmentState::Active {
            return false;
        }
        self.state.set(AttachmentState::Detached);
        self.attachment.borrow_mut().take();
        true
    }
}

impl fmt::Debug for AttachmentControl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AttachmentControl")
            .field("marker", &self.marker)
            .field("role", &self.role)
            .field("state", &self.state.get())
            .finish_non_exhaustive()
    }
}

/// Lease that unregisters an attachment when explicitly detached or dropped.
#[derive(Debug)]
#[must_use = "dropping the lease immediately detaches the Context attachment"]
pub struct ContextAttachmentLease {
    control: Weak<AttachmentControl>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl ContextAttachmentLease {
    /// Detaches the attachment if teardown has not started.
    ///
    /// Returns `true` only for the transition from attached to detached.
    pub fn detach(&mut self) -> bool {
        self.control
            .upgrade()
            .is_some_and(|control| control.detach())
    }

    /// Returns whether the attachment is still active.
    pub fn is_attached(&self) -> bool {
        self.control
            .upgrade()
            .is_some_and(|control| control.state.get() == AttachmentState::Active)
    }
}

impl Drop for ContextAttachmentLease {
    fn drop(&mut self) {
        let _ = self.detach();
    }
}

#[derive(Default)]
pub(super) struct AttachmentRegistry {
    controls: Vec<Rc<AttachmentControl>>,
    tearing_down: bool,
}

impl fmt::Debug for AttachmentRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AttachmentRegistry")
            .field("controls", &self.controls)
            .field("tearing_down", &self.tearing_down)
            .finish()
    }
}

impl AttachmentRegistry {
    pub(super) fn register<Marker: 'static>(
        &mut self,
        lifecycle: ContextLifecycle,
        role: ContextAttachmentRole,
        attachment: Rc<dyn ContextAttachment>,
    ) -> Result<ContextAttachmentLease, ContextAttachmentError> {
        if lifecycle != ContextLifecycle::Alive || self.tearing_down {
            return Err(ContextAttachmentError::ContextDropping);
        }

        self.controls
            .retain(|control| control.state.get() != AttachmentState::Detached);
        let marker = TypeId::of::<Marker>();
        if self.controls.iter().any(|control| control.marker == marker) {
            return Err(ContextAttachmentError::DuplicateAttachment);
        }
        if role == ContextAttachmentRole::Renderer
            && !self.role_is_active(ContextAttachmentRole::Platform)
        {
            return Err(ContextAttachmentError::MissingPlatform);
        }
        if role != ContextAttachmentRole::Extension && self.role_is_active(role) {
            return Err(ContextAttachmentError::RoleOccupied(role));
        }

        let control = Rc::new(AttachmentControl {
            marker,
            role,
            state: Cell::new(AttachmentState::Active),
            attachment: RefCell::new(Some(attachment)),
        });
        let lease = ContextAttachmentLease {
            control: Rc::downgrade(&control),
            _not_send_or_sync: PhantomData,
        };
        self.controls.push(control);
        Ok(lease)
    }

    fn role_is_active(&self, role: ContextAttachmentRole) -> bool {
        self.controls
            .iter()
            .any(|control| control.role == role && control.state.get() == AttachmentState::Active)
    }

    pub(super) fn begin_teardown(&mut self) -> Vec<Rc<AttachmentControl>> {
        self.tearing_down = true;
        let controls = std::mem::take(&mut self.controls);
        controls
            .into_iter()
            .filter(|control| {
                if control.state.get() != AttachmentState::Active {
                    return false;
                }
                control.state.set(AttachmentState::Teardown);
                true
            })
            .collect()
    }
}

pub(super) fn run_pre_destroy_phase(
    controls: &[Rc<AttachmentControl>],
    state: &ContextState,
    phase: ContextAttachmentPhase,
) {
    let context = ContextTeardown { state, phase };
    for control in controls {
        let Some(attachment) = control.attachment.borrow().clone() else {
            continue;
        };
        let _ = catch_unwind(AssertUnwindSafe(|| match phase {
            ContextAttachmentPhase::Quiesce => attachment.quiesce(&context),
            ContextAttachmentPhase::RendererResources => {
                attachment.release_renderer_resources(&context);
            }
            ContextAttachmentPhase::PlatformWindows => {
                attachment.release_platform_windows(&context);
            }
        }));
    }
}

pub(super) fn run_post_destroy(controls: Vec<Rc<AttachmentControl>>, context_id: ContextId) {
    let context = ContextDestroyed { id: context_id };
    for control in controls {
        if let Some(attachment) = control.attachment.borrow().clone() {
            let _ = catch_unwind(AssertUnwindSafe(|| attachment.context_destroyed(context)));
        }
        control.state.set(AttachmentState::Complete);
        let attachment = control.attachment.borrow_mut().take();
        let _ = catch_unwind(AssertUnwindSafe(move || drop(attachment)));
    }
}

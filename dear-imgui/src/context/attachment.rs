use std::any::TypeId;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr::NonNull;
use std::rc::{Rc, Weak};

use thiserror::Error;

use crate::render::RendererConsumer;

use super::binding::{self, ContextId, ContextLifecycle, ContextState};
use super::core::Context;

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

/// Non-retryable failure reported by an attachment during `Context::drop`.
///
/// Backends should expose their concrete error from explicit shutdown APIs. This erased error is
/// only for the final ownership fallback, where continuing into a later teardown phase would be
/// unsafe and the process therefore aborts after all peers in the current phase are notified.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct ContextAttachmentTeardownError {
    message: String,
}

impl ContextAttachmentTeardownError {
    /// Create a fail-stop attachment error from a backend diagnostic.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Type-erased lifecycle hooks owned by a Context.
///
/// Hooks must be idempotent. If a hook panics, the remaining attachments in that phase are still
/// notified, then the process aborts before a later destructive phase can violate resource
/// ordering. Explicit backend shutdown APIs remain responsible for reporting retryable errors;
/// attachment hooks are the fail-stop fallback used by `Context::drop`.
pub trait ContextAttachment {
    /// Stops new work before native teardown begins.
    fn quiesce(
        &self,
        _context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        Ok(())
    }

    /// Releases renderer resources before platform windows are destroyed.
    fn release_renderer_resources(
        &self,
        _context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        Ok(())
    }

    /// Releases platform windows before the native Context is destroyed.
    fn release_platform_windows(
        &self,
        _context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        Ok(())
    }

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
    owner: NonNull<Context>,
    phase: ContextAttachmentPhase,
    renderer_texture_reset_active: Cell<bool>,
    _exclusive_owner: PhantomData<&'a mut Context>,
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
    fn new<'owner>(
        owner: &'owner mut Context,
        phase: ContextAttachmentPhase,
    ) -> ContextTeardown<'owner> {
        ContextTeardown {
            owner: NonNull::from(owner),
            phase,
            renderer_texture_reset_active: Cell::new(false),
            _exclusive_owner: PhantomData,
        }
    }

    fn state(&self) -> &ContextState {
        // SAFETY: `run_pre_destroy_phase` creates this capability from its exclusive Context
        // borrow and does not access that Context again until the capability is dropped.
        unsafe { self.owner.as_ref().state.as_ref() }
    }

    /// Returns the Context identity being torn down.
    pub fn id(&self) -> ContextId {
        self.state().id()
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
            self.state().lifecycle(),
            ContextLifecycle::Dropping,
            "ContextTeardown used outside pre-destroy teardown"
        );
        let raw = self.state().raw_during_teardown();
        assert!(
            !raw.is_null(),
            "ContextTeardown used after native Context destruction"
        );

        binding::with_bound_context(raw, f)
    }

    /// Release a renderer's complete GPU texture map and reset its native bindings atomically.
    ///
    /// This transaction is available only from [`ContextAttachmentPhase::RendererResources`].
    /// The Context first validates that `consumer` is its matching idle renderer generation. It
    /// then runs `release`, and resets Context-owned texture bindings only when `release` returns
    /// `Ok(())`. A failed preflight, a failed release, a panic, or a reentrant call leaves those
    /// native bindings unchanged. The closure receives no Context access.
    /// On success, the returned count is the number of Context-owned bindings invalidated.
    ///
    /// Attachment hooks are the fail-stop fallback used by `Context::drop`; concrete renderer
    /// shutdown APIs should continue to expose their retryable backend errors before deferring
    /// ownership to the Context.
    pub fn with_renderer_texture_reset(
        &self,
        consumer: &RendererConsumer,
        release: impl FnOnce() -> Result<(), ContextAttachmentTeardownError>,
    ) -> Result<usize, ContextAttachmentTeardownError> {
        if self.phase != ContextAttachmentPhase::RendererResources {
            return Err(ContextAttachmentTeardownError::new(format!(
                "renderer texture reset requires the RendererResources phase, not {:?}",
                self.phase
            )));
        }
        if self.state().lifecycle() != ContextLifecycle::Dropping {
            return Err(ContextAttachmentTeardownError::new(
                "renderer texture reset requires active Context teardown",
            ));
        }
        if self.renderer_texture_reset_active.replace(true) {
            return Err(ContextAttachmentTeardownError::new(
                "renderer texture reset cannot be reentered",
            ));
        }
        let _active = RendererTextureResetInvocation {
            active: &self.renderer_texture_reset_active,
        };

        // SAFETY: `ContextTeardown` owns the exclusive Context borrow for this entire hook. No
        // mutable Context reference crosses `release`, and the active flag rejects recursive
        // attempts to create another reset transaction through this capability.
        let watermark = unsafe { &mut *self.owner.as_ptr() }
            .prepare_renderer_texture_reset_during_teardown(consumer)
            .map_err(|error| {
                ContextAttachmentTeardownError::new(format!(
                    "renderer texture reset preflight failed: {error}"
                ))
            })?;

        release()?;

        // SAFETY: the preflight's mutable borrow ended before `release` ran. This is the same
        // exclusive Context owner, the phase and lifecycle were validated above, and reentrancy
        // remains blocked until the commit completes.
        let invalidated = unsafe { &mut *self.owner.as_ptr() }
            .commit_renderer_texture_reset_during_teardown(watermark);
        Ok(invalidated)
    }

    #[cfg(test)]
    pub(super) fn as_raw_for_test(&self) -> *mut crate::sys::ImGuiContext {
        self.state().raw_during_teardown()
    }
}

struct RendererTextureResetInvocation<'a> {
    active: &'a Cell<bool>,
}

impl Drop for RendererTextureResetInvocation<'_> {
    fn drop(&mut self) {
        self.active.set(false);
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

    /// Leave the attachment under Context ownership until Context teardown.
    ///
    /// Backend owners use this when their own `Drop` implementation cannot safely enter native
    /// teardown without an explicit mutable Context. The Context retains the attachment and runs
    /// its normal phased teardown before destroying the native Context.
    pub fn defer_to_context(mut self) {
        self.control = Weak::new();
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
    owner: &mut Context,
    phase: ContextAttachmentPhase,
) -> bool {
    let context = ContextTeardown::new(owner, phase);
    let mut completed = true;
    for control in controls {
        let Some(attachment) = control.attachment.borrow().clone() else {
            continue;
        };
        let result = catch_unwind(AssertUnwindSafe(|| match phase {
            ContextAttachmentPhase::Quiesce => attachment.quiesce(&context),
            ContextAttachmentPhase::RendererResources => {
                attachment.release_renderer_resources(&context)
            }
            ContextAttachmentPhase::PlatformWindows => {
                attachment.release_platform_windows(&context)
            }
        }));
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                completed = false;
                std::mem::forget(error);
            }
            Err(payload) => {
                completed = false;
                // A panic payload may itself panic when dropped. Context teardown is now
                // fail-stop, so retain it until the caller aborts instead of risking a nested
                // unwind mid-phase.
                std::mem::forget(payload);
            }
        }
    }
    completed
}

pub(super) fn run_post_destroy(
    controls: Vec<Rc<AttachmentControl>>,
    context_id: ContextId,
) -> bool {
    let context = ContextDestroyed { id: context_id };
    let mut completed = true;
    for control in controls {
        if let Some(attachment) = control.attachment.borrow().clone() {
            if let Err(payload) =
                catch_unwind(AssertUnwindSafe(|| attachment.context_destroyed(context)))
            {
                completed = false;
                std::mem::forget(payload);
            }
        }
        control.state.set(AttachmentState::Complete);
        let attachment = control.attachment.borrow_mut().take();
        if let Err(payload) = catch_unwind(AssertUnwindSafe(move || drop(attachment))) {
            completed = false;
            std::mem::forget(payload);
        }
    }
    completed
}

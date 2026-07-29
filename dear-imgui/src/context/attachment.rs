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

/// Failure while entering or leaving an explicit platform-window teardown transaction.
///
/// Unlike [`ContextAttachmentTeardownError`], this error is returned to the caller of
/// [`crate::Context::destroy_platform_windows`] before or after its native operation. It does not
/// use Context-drop's fail-stop policy because the caller still owns a live Context.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum ContextPlatformWindowTeardownError {
    /// The Context is already being dropped.
    #[error("Dear ImGui context teardown is in progress")]
    ContextDropping,
    /// A platform-window teardown transaction attempted to re-enter itself.
    #[error("platform-window teardown cannot be reentered")]
    Reentrant,
    /// The active platform attachment rejected the transaction before native teardown began.
    #[error("platform attachment rejected platform-window teardown: {0}")]
    AttachmentPreflight(#[source] ContextAttachmentTeardownError),
    /// The active platform attachment failed after native teardown completed.
    #[error("platform attachment could not complete platform-window teardown: {0}")]
    AttachmentPostflight(#[source] ContextAttachmentTeardownError),
    /// The active platform attachment panicked before native teardown began.
    #[error("platform attachment panicked before platform-window teardown")]
    BeginPanicked,
    /// The active platform attachment panicked after native teardown completed.
    #[error("platform attachment panicked after platform-window teardown")]
    EndPanicked,
}

/// Type-erased lifecycle hooks owned by a Context.
///
/// Hooks must be idempotent. If a hook panics, the remaining attachments in that phase are still
/// notified, then the process aborts before a later destructive phase can violate resource
/// ordering. Explicit backend shutdown APIs remain responsible for reporting retryable errors;
/// attachment hooks are the fail-stop fallback used by `Context::drop`.
pub trait ContextAttachment {
    /// Validates and prepares a normal [`crate::Context::destroy_platform_windows`] call.
    ///
    /// Only the active platform attachment receives this hook. The passed capability may bind the
    /// target Context for immediate native inspection, but intentionally does not expose a mutable
    /// `Context` reference. Returning an error prevents native teardown from starting.
    fn begin_platform_window_teardown(
        &self,
        _context: &ContextPlatformWindowTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        Ok(())
    }

    /// Completes a normal [`crate::Context::destroy_platform_windows`] call.
    ///
    /// This runs only when [`Self::begin_platform_window_teardown`] succeeded and native teardown
    /// returned normally. Implementations should restore any temporary callback state and record
    /// the new native baseline before returning.
    fn end_platform_window_teardown(
        &self,
        _context: &ContextPlatformWindowTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        Ok(())
    }

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

/// Failure to explicitly detach a live Context attachment lease.
///
/// A failed detach leaves both the attachment and lease active. Platform backends must release
/// renderer dependencies first and use [`Context::prepare_platform_attachment_release`] for
/// transactional native teardown.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum ContextAttachmentDetachError {
    /// Another transaction has reserved this platform attachment generation for release.
    #[error("platform attachment release is already in progress")]
    ReleaseInProgress,
    /// Renderer resources still depend on platform-owned native handles.
    #[error("the platform attachment cannot be detached while a renderer attachment is active")]
    RendererActive,
}

/// Failure to prepare an explicit platform attachment release.
///
/// Platform backends must complete this preflight before closing a frame, destroying native
/// windows, or clearing callback state. The returned permit keeps the exact attachment generation
/// reserved until the backend either commits detachment or abandons the transaction.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum ContextPlatformAttachmentReleaseError {
    /// Context-owned teardown has already started.
    #[error("Dear ImGui context teardown is in progress")]
    ContextDropping,
    /// The supplied attachment generation has already detached.
    #[error("the platform attachment generation is no longer active")]
    AttachmentInactive,
    /// The supplied attachment does not own the platform role.
    #[error("the supplied attachment does not own the platform role")]
    NotPlatform,
    /// The supplied attachment is not this Context's active platform generation.
    #[error("the supplied attachment is not the active platform generation for this Context")]
    PlatformGenerationMismatch,
    /// Another release transaction already reserves this platform generation.
    #[error("platform attachment release is already in progress")]
    ReleaseInProgress,
    /// Renderer resources still depend on platform-owned native handles.
    #[error("the platform attachment cannot be released while a renderer attachment is active")]
    RendererActive,
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

/// Phase-limited capability passed around a normal platform-window teardown transaction.
///
/// The Context remains alive throughout this scope. It exists so a platform backend can prepare
/// callback state for native teardown without receiving unrestricted mutable Context access.
pub struct ContextPlatformWindowTeardown<'a> {
    state: &'a ContextState,
    _exclusive_owner: PhantomData<&'a mut Context>,
}

impl fmt::Debug for ContextPlatformWindowTeardown<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContextPlatformWindowTeardown")
            .field("id", &self.id())
            .finish_non_exhaustive()
    }
}

impl<'a> ContextPlatformWindowTeardown<'a> {
    #[cfg(feature = "multi-viewport")]
    pub(super) fn new(state: &'a ContextState) -> Self {
        Self {
            state,
            _exclusive_owner: PhantomData,
        }
    }

    /// Returns the Context identity whose platform windows are being torn down.
    pub fn id(&self) -> ContextId {
        self.state.id()
    }

    /// Runs a closure while the target Context is current.
    ///
    /// The capability remains valid only for the observer hook currently executing. It does not
    /// provide a mutable [`Context`] reference, so backend callbacks cannot recursively enter an
    /// unrelated Context operation while native window teardown is in progress.
    pub fn with_bound_context<R>(&self, f: impl FnOnce() -> R) -> R {
        assert_eq!(
            self.state.lifecycle(),
            ContextLifecycle::Alive,
            "ContextPlatformWindowTeardown used outside a live Context"
        );
        let raw = self.state.raw_during_teardown();
        assert!(
            !raw.is_null(),
            "ContextPlatformWindowTeardown used after native Context destruction"
        );
        binding::with_bound_context(raw, f)
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
    ReleasePrepared,
    Teardown,
    Complete,
    Detached,
}

#[derive(Default)]
struct AttachmentRoleState {
    renderer_active: Cell<bool>,
}

pub(super) struct AttachmentControl {
    marker: TypeId,
    role: ContextAttachmentRole,
    state: Cell<AttachmentState>,
    attachment: RefCell<Option<Rc<dyn ContextAttachment>>>,
    roles: Rc<AttachmentRoleState>,
}

impl AttachmentControl {
    fn detach(&self) -> Result<bool, ContextAttachmentDetachError> {
        match self.state.get() {
            AttachmentState::Active => {}
            AttachmentState::ReleasePrepared => {
                return Err(ContextAttachmentDetachError::ReleaseInProgress);
            }
            AttachmentState::Teardown | AttachmentState::Complete | AttachmentState::Detached => {
                return Ok(false);
            }
        }
        if self.role == ContextAttachmentRole::Platform && self.roles.renderer_active.get() {
            return Err(ContextAttachmentDetachError::RendererActive);
        }
        self.state.set(AttachmentState::Detached);
        self.attachment.borrow_mut().take();
        if self.role == ContextAttachmentRole::Renderer {
            self.roles.renderer_active.set(false);
        }
        Ok(true)
    }

    fn prepare_platform_release(&self) {
        debug_assert_eq!(self.role, ContextAttachmentRole::Platform);
        debug_assert_eq!(self.state.get(), AttachmentState::Active);
        self.state.set(AttachmentState::ReleasePrepared);
    }

    fn abandon_platform_release(&self) {
        if self.state.get() == AttachmentState::ReleasePrepared {
            self.state.set(AttachmentState::Active);
        }
    }

    fn commit_platform_release(&self) {
        debug_assert_eq!(self.role, ContextAttachmentRole::Platform);
        debug_assert_eq!(self.state.get(), AttachmentState::ReleasePrepared);
        debug_assert!(!self.roles.renderer_active.get());
        self.state.set(AttachmentState::Detached);
        self.attachment.borrow_mut().take();
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
///
/// A platform attachment with an active renderer dependency remains Context-owned instead of
/// detaching out of order.
#[derive(Debug)]
#[must_use = "retain the lease for explicit detach, or defer cleanup to Context teardown"]
pub struct ContextAttachmentLease {
    control: Weak<AttachmentControl>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl ContextAttachmentLease {
    /// Returns a non-owning identity for this exact attachment generation.
    ///
    /// Backends may retain the handle in shared runtime state. Unlike the lease, dropping a handle
    /// never detaches the attachment.
    pub fn handle(&self) -> ContextAttachmentHandle {
        ContextAttachmentHandle {
            control: self.control.clone(),
            _not_send_or_sync: PhantomData,
        }
    }

    /// Detaches the attachment if it is still active and has no dependency blocking release.
    ///
    /// `Ok(true)` reports the transition from attached to detached. `Ok(false)` means Context
    /// teardown or an earlier release already claimed the attachment. An error leaves the lease
    /// attached: platform backends must not destroy native state after such a failure. Explicit
    /// platform shutdown should use [`Context::prepare_platform_attachment_release`] so native
    /// teardown and lease release share one transaction.
    pub fn detach(&mut self) -> Result<bool, ContextAttachmentDetachError> {
        self.control
            .upgrade()
            .map_or(Ok(false), |control| control.detach())
    }

    /// Returns whether the attachment is still active.
    pub fn is_attached(&self) -> bool {
        self.control.upgrade().is_some_and(|control| {
            matches!(
                control.state.get(),
                AttachmentState::Active | AttachmentState::ReleasePrepared
            )
        })
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

/// Non-owning identity for one exact Context attachment generation.
///
/// Handles are cloneable so related runtime objects can prove which platform attachment they use.
/// They cannot detach the attachment and do not keep it alive after the Context releases it.
#[derive(Clone, Debug)]
pub struct ContextAttachmentHandle {
    control: Weak<AttachmentControl>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl ContextAttachmentHandle {
    /// Returns whether this attachment generation remains active or reserved by a release permit.
    pub fn is_attached(&self) -> bool {
        self.control.upgrade().is_some_and(|control| {
            matches!(
                control.state.get(),
                AttachmentState::Active | AttachmentState::ReleasePrepared
            )
        })
    }

    /// Returns whether an active renderer attachment still depends on this platform generation.
    ///
    /// This is a conservative Drop-path diagnostic. Explicit platform shutdown must use
    /// [`Context::prepare_platform_attachment_release`] so the check and native cleanup share one
    /// exclusive transaction.
    pub fn has_active_renderer_dependency(&self) -> bool {
        self.control.upgrade().is_some_and(|control| {
            control.role == ContextAttachmentRole::Platform
                && matches!(
                    control.state.get(),
                    AttachmentState::Active | AttachmentState::ReleasePrepared
                )
                && control.roles.renderer_active.get()
        })
    }
}

/// Exclusive permit for an explicit platform attachment release transaction.
///
/// Preparing the permit proves that no renderer attachment still depends on the exact platform
/// generation. Use [`Self::context_mut`] for any frame normalization or native cleanup, then call
/// [`Self::commit`] only after the platform attachment has been fully released. Dropping an
/// uncommitted permit restores the attachment to its active state so shutdown can be retried.
/// Renderer registration remains unavailable while the permit reserves the platform generation.
#[must_use = "dropping the permit abandons platform detachment and keeps the attachment active"]
pub struct ContextPlatformAttachmentRelease<'a> {
    context: &'a mut Context,
    control: Rc<AttachmentControl>,
    committed: bool,
}

impl fmt::Debug for ContextPlatformAttachmentRelease<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ContextPlatformAttachmentRelease")
            .field("attachment", &self.control)
            .field("committed", &self.committed)
            .finish_non_exhaustive()
    }
}

impl<'a> ContextPlatformAttachmentRelease<'a> {
    pub(super) fn new(context: &'a mut Context, control: Rc<AttachmentControl>) -> Self {
        Self {
            context,
            control,
            committed: false,
        }
    }

    /// Returns the exclusively borrowed Context after platform-release preflight succeeded.
    pub fn context_mut(&mut self) -> &mut Context {
        self.context
    }

    /// Commits detachment of the exact platform attachment generation.
    ///
    /// This operation is infallible because the permit exclusively borrows the Context and keeps
    /// the attachment control reserved against ordinary lease detachment.
    pub fn commit(mut self) {
        self.control.commit_platform_release();
        self.committed = true;
    }
}

impl Drop for ContextPlatformAttachmentRelease<'_> {
    fn drop(&mut self) {
        if !self.committed {
            self.control.abandon_platform_release();
        }
    }
}

#[derive(Default)]
pub(super) struct AttachmentRegistry {
    controls: Vec<Rc<AttachmentControl>>,
    roles: Rc<AttachmentRoleState>,
    tearing_down: bool,
    platform_window_teardown_active: Cell<bool>,
}

impl fmt::Debug for AttachmentRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AttachmentRegistry")
            .field("controls", &self.controls)
            .field("tearing_down", &self.tearing_down)
            .field(
                "platform_window_teardown_active",
                &self.platform_window_teardown_active.get(),
            )
            .finish()
    }
}

impl AttachmentRegistry {
    pub(super) fn preflight_register<Marker: 'static>(
        &self,
        lifecycle: ContextLifecycle,
        role: ContextAttachmentRole,
    ) -> Result<(), ContextAttachmentError> {
        if lifecycle != ContextLifecycle::Alive || self.tearing_down {
            return Err(ContextAttachmentError::ContextDropping);
        }

        let marker = TypeId::of::<Marker>();
        if self.controls.iter().any(|control| {
            control.marker == marker && control.state.get() != AttachmentState::Detached
        }) {
            return Err(ContextAttachmentError::DuplicateAttachment);
        }
        if role == ContextAttachmentRole::Renderer
            && !self.role_is_operational(ContextAttachmentRole::Platform)
        {
            return Err(ContextAttachmentError::MissingPlatform);
        }
        if role != ContextAttachmentRole::Extension && self.role_is_active(role) {
            return Err(ContextAttachmentError::RoleOccupied(role));
        }
        Ok(())
    }

    pub(super) fn register<Marker: 'static>(
        &mut self,
        lifecycle: ContextLifecycle,
        role: ContextAttachmentRole,
        attachment: Rc<dyn ContextAttachment>,
    ) -> Result<ContextAttachmentLease, ContextAttachmentError> {
        self.preflight_register::<Marker>(lifecycle, role)?;
        self.controls
            .retain(|control| control.state.get() != AttachmentState::Detached);
        let marker = TypeId::of::<Marker>();

        let control = Rc::new(AttachmentControl {
            marker,
            role,
            state: Cell::new(AttachmentState::Active),
            attachment: RefCell::new(Some(attachment)),
            roles: Rc::clone(&self.roles),
        });
        if role == ContextAttachmentRole::Renderer {
            self.roles.renderer_active.set(true);
        }
        let lease = ContextAttachmentLease {
            control: Rc::downgrade(&control),
            _not_send_or_sync: PhantomData,
        };
        self.controls.push(control);
        Ok(lease)
    }

    fn role_is_active(&self, role: ContextAttachmentRole) -> bool {
        self.controls.iter().any(|control| {
            control.role == role
                && matches!(
                    control.state.get(),
                    AttachmentState::Active | AttachmentState::ReleasePrepared
                )
        })
    }

    fn role_is_operational(&self, role: ContextAttachmentRole) -> bool {
        self.controls
            .iter()
            .any(|control| control.role == role && control.state.get() == AttachmentState::Active)
    }

    pub(super) fn prepare_platform_release(
        &self,
        handle: &ContextAttachmentHandle,
    ) -> Result<Rc<AttachmentControl>, ContextPlatformAttachmentReleaseError> {
        if self.tearing_down {
            return Err(ContextPlatformAttachmentReleaseError::ContextDropping);
        }
        let control = handle
            .control
            .upgrade()
            .ok_or(ContextPlatformAttachmentReleaseError::AttachmentInactive)?;
        if control.role != ContextAttachmentRole::Platform {
            return Err(ContextPlatformAttachmentReleaseError::NotPlatform);
        }
        match control.state.get() {
            AttachmentState::Active => {}
            AttachmentState::ReleasePrepared => {
                return Err(ContextPlatformAttachmentReleaseError::ReleaseInProgress);
            }
            AttachmentState::Teardown | AttachmentState::Complete | AttachmentState::Detached => {
                return Err(ContextPlatformAttachmentReleaseError::AttachmentInactive);
            }
        }
        let owns_active_generation = self.controls.iter().any(|candidate| {
            Rc::ptr_eq(candidate, &control)
                && candidate.role == ContextAttachmentRole::Platform
                && candidate.state.get() == AttachmentState::Active
        });
        if !owns_active_generation {
            return Err(ContextPlatformAttachmentReleaseError::PlatformGenerationMismatch);
        }
        if self.roles.renderer_active.get() {
            return Err(ContextPlatformAttachmentReleaseError::RendererActive);
        }
        control.prepare_platform_release();
        Ok(control)
    }

    #[cfg(feature = "multi-viewport")]
    pub(super) fn begin_platform_window_teardown(
        &self,
        context: &ContextPlatformWindowTeardown<'_>,
    ) -> Result<PlatformWindowTeardownInvocation<'_>, ContextPlatformWindowTeardownError> {
        if self.tearing_down {
            return Err(ContextPlatformWindowTeardownError::ContextDropping);
        }
        if self.platform_window_teardown_active.get() {
            return Err(ContextPlatformWindowTeardownError::Reentrant);
        }
        self.platform_window_teardown_active.set(true);
        let invocation = PlatformWindowTeardownInvocation {
            attachment: self
                .controls
                .iter()
                .find(|control| {
                    control.role == ContextAttachmentRole::Platform
                        && matches!(
                            control.state.get(),
                            AttachmentState::Active | AttachmentState::ReleasePrepared
                        )
                })
                .and_then(|control| control.attachment.borrow().clone()),
            active: &self.platform_window_teardown_active,
        };
        invocation.begin(context)?;
        Ok(invocation)
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

#[cfg(feature = "multi-viewport")]
pub(super) struct PlatformWindowTeardownInvocation<'a> {
    attachment: Option<Rc<dyn ContextAttachment>>,
    active: &'a Cell<bool>,
}

#[cfg(feature = "multi-viewport")]
impl PlatformWindowTeardownInvocation<'_> {
    fn begin(
        &self,
        context: &ContextPlatformWindowTeardown<'_>,
    ) -> Result<(), ContextPlatformWindowTeardownError> {
        let Some(attachment) = &self.attachment else {
            return Ok(());
        };
        match catch_unwind(AssertUnwindSafe(|| {
            attachment.begin_platform_window_teardown(context)
        })) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(ContextPlatformWindowTeardownError::AttachmentPreflight(
                error,
            )),
            Err(payload) => {
                // A panic payload may panic when dropped. The transaction is rejected before any
                // native teardown begins, so retaining it is preferable to a nested unwind.
                std::mem::forget(payload);
                Err(ContextPlatformWindowTeardownError::BeginPanicked)
            }
        }
    }

    pub(super) fn finish(
        self,
        context: &ContextPlatformWindowTeardown<'_>,
    ) -> Result<(), ContextPlatformWindowTeardownError> {
        let Some(attachment) = &self.attachment else {
            return Ok(());
        };
        match catch_unwind(AssertUnwindSafe(|| {
            attachment.end_platform_window_teardown(context)
        })) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(ContextPlatformWindowTeardownError::AttachmentPostflight(
                error,
            )),
            Err(payload) => {
                // Native teardown completed, but the caller still receives a recoverable Rust
                // error rather than unwinding through an FFI boundary.
                std::mem::forget(payload);
                Err(ContextPlatformWindowTeardownError::EndPanicked)
            }
        }
    }
}

#[cfg(feature = "multi-viewport")]
impl Drop for PlatformWindowTeardownInvocation<'_> {
    fn drop(&mut self) {
        self.active.set(false);
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

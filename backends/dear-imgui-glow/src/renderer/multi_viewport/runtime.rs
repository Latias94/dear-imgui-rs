use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::fmt;
use std::rc::Rc;

use dear_imgui_rs::render::{ReconciledFrame, RenderedFrame};
use dear_imgui_rs::{
    Context, ContextAttachment, ContextAttachmentError, ContextAttachmentLease,
    ContextAttachmentRole, ContextAttachmentTeardownError, ContextBinding, ContextBindingError,
    ContextDestroyed, ContextId, ContextTeardown, Id, TextureFormat, TextureId,
};
use thiserror::Error;

use super::callbacks::{
    claim_callbacks, detect_callback_drift, preflight_callbacks, release_callbacks,
    revoke_renderer_viewport_capability_if_owned,
};
use super::registry::{preflight_runtime, register_runtime, unregister_runtime};
use crate::{GlowRenderer, InitError, RenderError};

struct GlowRendererAttachmentMarker;

/// Failure to attach or operate an owning Glow multi-viewport runtime.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum GlowViewportError {
    /// The Dear ImGui Context rejected the renderer attachment.
    #[error(transparent)]
    Attachment(#[from] ContextAttachmentError),
    /// The originating Context can no longer be entered normally.
    #[error(transparent)]
    Context(#[from] ContextBindingError),
    /// The underlying renderer operation failed.
    #[error(transparent)]
    Renderer(#[from] RenderError),
    /// A renderer-owned legacy texture operation failed.
    #[error(transparent)]
    Texture(#[from] InitError),
    /// Existing external-context renderers do not own a verifiable GL capability.
    #[error(
        "Glow multi-viewport requires a renderer created with an owned or shared glow::Context"
    )]
    ExternalContextUnsupported,
    /// The renderer and runtime Context identities differ.
    #[error("Glow viewport runtime belongs to Context {expected:?}, not {actual:?}")]
    ContextMismatch {
        expected: ContextId,
        actual: ContextId,
    },
    /// A callback entry was not running under this runtime's Context.
    #[error("the current Dear ImGui Context does not match Glow runtime Context {expected:?}")]
    BoundContextMismatch { expected: ContextId },
    /// The platform attachment has not advertised multi-viewport support.
    #[error("Glow multi-viewport requires an attached multi-viewport platform runtime")]
    PlatformBackendUnavailable,
    /// A platform callback required to make and present GL viewport contexts is absent.
    #[error("required ImGuiPlatformIO callback `{callback}` is not installed")]
    PlatformCallbackUnavailable { callback: &'static str },
    /// PlatformIO itself is unavailable for the currently bound Context.
    #[error("the bound Dear ImGui Context has no PlatformIO")]
    PlatformIoUnavailable,
    /// Another renderer already owns part of the renderer callback table.
    #[error("ImGuiPlatformIO callback `{callback}` is already owned by another renderer")]
    RendererCallbackOccupied { callback: &'static str },
    /// Another renderer already advertises renderer-owned multi-viewport support.
    #[error("ImGui backend flag `RENDERER_HAS_VIEWPORTS` is already owned by another renderer")]
    RendererViewportCapabilityOccupied,
    /// The renderer-owned multi-viewport capability disappeared while attached.
    #[error("Glow renderer backend flag `RENDERER_HAS_VIEWPORTS` was removed while attached")]
    RendererViewportCapabilityLost,
    /// A callback claimed by this runtime was replaced while attached.
    #[error("Glow renderer callback `{callback}` was replaced while the runtime was attached")]
    RendererCallbackReplaced { callback: &'static str },
    /// The registry already contains a live runtime for this Context.
    #[error("a Glow viewport runtime is already attached to this Context")]
    RuntimeAlreadyAttached,
    /// The owning runtime has already shut down or begun Context-owned teardown.
    #[error("the Glow viewport runtime is no longer attached")]
    RuntimeDetached,
    /// The renderer is already mutably borrowed by another runtime entry.
    #[error("Glow renderer runtime is already active in `{callback}`")]
    CallbackReentered { callback: &'static str },
    /// A callback panic was contained at the C ABI boundary.
    #[error("Glow renderer callback `{callback}` panicked")]
    CallbackPanicked { callback: &'static str },
    /// Dear ImGui passed an invalid viewport to the renderer callback.
    #[error("Glow renderer callback received a null viewport")]
    InvalidViewport,
    /// A frame trace is already collecting events for this runtime.
    #[error("a Glow viewport frame trace is already active")]
    FrameTraceAlreadyActive,
}

/// Renderer callbacks that completed successfully during one platform-window pump.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GlowViewportFrameReport {
    rendered_viewports: Vec<Id>,
}

impl GlowViewportFrameReport {
    /// Returns the secondary viewport IDs whose draw data completed Glow rendering.
    pub fn rendered_viewports(&self) -> &[Id] {
        &self.rendered_viewports
    }
}

#[derive(Debug)]
struct ActiveFrameTrace {
    rendered_viewports: HashSet<Id>,
}

#[derive(Debug, Default)]
struct FrameTraceState {
    active: Option<ActiveFrameTrace>,
}

/// Scoped collector for one Glow secondary-viewport render pass.
///
/// Dropping this guard without calling [`Self::finish`] aborts the report and releases the runtime
/// for the next frame. A runtime accepts at most one live trace, so callback events cannot be
/// assigned to overlapping frames.
#[must_use = "keep the trace alive through the platform-window pump, then call finish"]
pub struct GlowViewportFrameTrace<'runtime> {
    control: &'runtime RuntimeControl,
    finished: bool,
}

impl fmt::Debug for GlowViewportFrameTrace<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GlowViewportFrameTrace")
            .field("context", &self.control.binding.id())
            .field("finished", &self.finished)
            .finish()
    }
}

impl GlowViewportFrameTrace<'_> {
    /// Finishes this frame trace and returns only callbacks that completed successfully.
    pub fn finish(mut self) -> GlowViewportFrameReport {
        let report = self.control.finish_frame_trace();
        self.finished = true;
        report
    }
}

impl Drop for GlowViewportFrameTrace<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.control.abort_frame_trace();
        }
    }
}

/// Transactional attachment failure that returns the renderer unchanged.
pub struct GlowViewportAttachError {
    error: GlowViewportError,
    renderer: Box<GlowRenderer>,
}

impl GlowViewportAttachError {
    fn new(error: GlowViewportError, renderer: GlowRenderer) -> Self {
        Self {
            error,
            renderer: Box::new(renderer),
        }
    }

    /// Returns the reason attachment failed.
    pub fn error(&self) -> &GlowViewportError {
        &self.error
    }

    /// Returns the renderer so the caller can retry, use it for one viewport, or destroy it.
    pub fn into_renderer(self) -> GlowRenderer {
        *self.renderer
    }

    /// Returns both the typed failure and the unchanged renderer.
    pub fn into_parts(self) -> (GlowViewportError, GlowRenderer) {
        (self.error, *self.renderer)
    }
}

impl fmt::Debug for GlowViewportAttachError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GlowViewportAttachError")
            .field("error", &self.error)
            .field("renderer", &"returned to caller")
            .finish()
    }
}

impl fmt::Display for GlowViewportAttachError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for GlowViewportAttachError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RuntimeState {
    Constructing,
    Attached,
    ShuttingDown,
    Detached,
    ResourceDropped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CallbackState {
    Unclaimed,
    Claimed,
    Released,
}

enum ShutdownAction<'a> {
    Quiesce,
    Explicit(&'a mut Context),
}

pub(super) struct RuntimeControl {
    context_raw: *mut dear_imgui_rs::sys::ImGuiContext,
    binding: ContextBinding,
    state: Cell<RuntimeState>,
    renderer: RefCell<Option<Box<GlowRenderer>>>,
    gl: RefCell<Option<Rc<glow::Context>>>,
    attachment: RefCell<Option<ContextAttachmentLease>>,
    callback_state: Cell<CallbackState>,
    faults: RefCell<RuntimeFaults>,
    frame_trace: RefCell<FrameTraceState>,
    #[cfg(test)]
    panic_next_callback: Cell<bool>,
    #[cfg(test)]
    transitions: RefCell<Vec<&'static str>>,
}

#[derive(Default)]
struct RuntimeFaults {
    terminal: Option<GlowViewportError>,
    non_terminal: Option<GlowViewportError>,
}

impl RuntimeFaults {
    fn record_terminal(&mut self, fault: GlowViewportError) {
        if self.terminal.is_none() {
            self.terminal = Some(fault);
        }
    }

    fn record_non_terminal(&mut self, fault: GlowViewportError) {
        if self.non_terminal.is_none() {
            self.non_terminal = Some(fault);
        }
    }

    fn take_next(&mut self) -> Option<GlowViewportError> {
        self.terminal.take().or_else(|| self.non_terminal.take())
    }
}

impl fmt::Debug for RuntimeControl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeControl")
            .field("context", &self.binding.id())
            .field("state", &self.state.get())
            .field("has_renderer", &self.renderer.borrow().is_some())
            .field("callback_state", &self.callback_state.get())
            .finish_non_exhaustive()
    }
}

impl RuntimeControl {
    fn new(context: &Context, renderer: GlowRenderer, gl: Rc<glow::Context>) -> Self {
        Self {
            context_raw: context.as_raw(),
            binding: context.binding(),
            state: Cell::new(RuntimeState::Constructing),
            renderer: RefCell::new(Some(Box::new(renderer))),
            gl: RefCell::new(Some(gl)),
            attachment: RefCell::new(None),
            callback_state: Cell::new(CallbackState::Unclaimed),
            faults: RefCell::new(RuntimeFaults::default()),
            frame_trace: RefCell::new(FrameTraceState::default()),
            #[cfg(test)]
            panic_next_callback: Cell::new(false),
            #[cfg(test)]
            transitions: RefCell::new(Vec::new()),
        }
    }

    pub(super) fn context_raw(&self) -> *mut dear_imgui_rs::sys::ImGuiContext {
        self.context_raw
    }

    pub(super) fn binding(&self) -> &ContextBinding {
        &self.binding
    }

    #[cfg(test)]
    pub(super) fn state(&self) -> RuntimeState {
        self.state.get()
    }

    pub(super) fn is_callback_accessible(&self) -> bool {
        self.state.get() == RuntimeState::Attached
            && self.callback_state.get() != CallbackState::Released
    }

    pub(super) fn should_detect_callback_drift(&self) -> bool {
        self.state.get() == RuntimeState::Attached
            && self.callback_state.get() == CallbackState::Claimed
    }

    pub(super) fn callback_released(&self) -> bool {
        self.callback_state.get() == CallbackState::Released
    }

    pub(super) fn mark_callback_claimed(&self) {
        self.callback_state.set(CallbackState::Claimed);
    }

    pub(super) fn mark_callback_released(&self) {
        self.callback_state.set(CallbackState::Released);
        unregister_runtime(self.binding.id());
    }

    fn set_state(&self, state: RuntimeState) {
        #[cfg(test)]
        let previous = self.state.replace(state);
        #[cfg(not(test))]
        self.state.set(state);
        #[cfg(test)]
        if previous != state {
            match state {
                RuntimeState::ShuttingDown => self.transitions.borrow_mut().push("ShuttingDown"),
                RuntimeState::Detached => self.transitions.borrow_mut().push("Detached"),
                RuntimeState::ResourceDropped => {
                    self.transitions.borrow_mut().push("ResourceDropped");
                }
                RuntimeState::Constructing | RuntimeState::Attached => {}
            }
        }
    }

    fn begin_shutdown(&self) {
        if matches!(
            self.state.get(),
            RuntimeState::Constructing | RuntimeState::Attached
        ) {
            self.set_state(RuntimeState::ShuttingDown);
        }
    }

    fn mark_detached(&self) {
        if !matches!(
            self.state.get(),
            RuntimeState::Detached | RuntimeState::ResourceDropped
        ) {
            self.set_state(RuntimeState::Detached);
        }
        unregister_runtime(self.binding.id());
    }

    pub(super) fn record_fault(&self, fault: GlowViewportError) {
        self.faults.borrow_mut().record_non_terminal(fault);
    }

    pub(super) fn record_dependency_fault(&self, fault: GlowViewportError) {
        revoke_renderer_viewport_capability_if_owned(self);
        self.faults.borrow_mut().record_terminal(fault);
        self.begin_shutdown();
    }

    fn begin_frame_trace(&self) -> Result<GlowViewportFrameTrace<'_>, GlowViewportError> {
        if self.frame_trace.borrow().active.is_some() {
            return Err(GlowViewportError::FrameTraceAlreadyActive);
        }
        self.ensure_entry()?;
        {
            let mut trace = self.frame_trace.borrow_mut();
            if trace.active.is_some() {
                return Err(GlowViewportError::FrameTraceAlreadyActive);
            }
            trace.active = Some(ActiveFrameTrace {
                rendered_viewports: HashSet::new(),
            });
        }
        Ok(GlowViewportFrameTrace {
            control: self,
            finished: false,
        })
    }

    fn finish_frame_trace(&self) -> GlowViewportFrameReport {
        let active = {
            let mut trace = self.frame_trace.borrow_mut();
            trace
                .active
                .take()
                .expect("a live Glow frame-trace guard owns the active trace")
        };
        let mut rendered_viewports = active.rendered_viewports.into_iter().collect::<Vec<_>>();
        rendered_viewports.sort_unstable_by_key(|id| id.raw());
        GlowViewportFrameReport { rendered_viewports }
    }

    fn abort_frame_trace(&self) {
        self.frame_trace.borrow_mut().active = None;
    }

    pub(super) fn record_rendered_viewport(&self, viewport_id: dear_imgui_rs::sys::ImGuiID) {
        if let Some(active) = self.frame_trace.borrow_mut().active.as_mut() {
            active.rendered_viewports.insert(Id::from(viewport_id));
        }
    }

    fn record_renderer_operational_fault(&self, error: RenderError) {
        let terminal = matches!(
            &error,
            RenderError::RendererStateDrift { .. }
                | RenderError::RendererCallbackReplaced { .. }
                | RenderError::RendererCapabilityDrift { .. }
        );
        let fault = GlowViewportError::Renderer(error);
        if terminal {
            self.record_dependency_fault(fault);
        } else {
            self.record_fault(fault);
        }
    }

    fn detect_and_take_fault(&self) -> Option<GlowViewportError> {
        detect_callback_drift(self);
        self.faults.borrow_mut().take_next()
    }

    fn ensure_context(&self, context: &Context) -> Result<(), GlowViewportError> {
        if context.id() == self.binding.id() {
            Ok(())
        } else {
            Err(GlowViewportError::ContextMismatch {
                expected: self.binding.id(),
                actual: context.id(),
            })
        }
    }

    fn ensure_entry(&self) -> Result<(), GlowViewportError> {
        if let Some(fault) = self.detect_and_take_fault() {
            return Err(fault);
        }
        if self.state.get() == RuntimeState::Attached {
            Ok(())
        } else {
            Err(GlowViewportError::RuntimeDetached)
        }
    }

    fn finish_entry(&self) -> Result<(), GlowViewportError> {
        self.detect_and_take_fault().map_or(Ok(()), Err)
    }

    fn with_renderer_mut<R>(
        &self,
        callback: impl FnOnce(&mut GlowRenderer) -> Result<R, GlowViewportError>,
    ) -> Result<R, GlowViewportError> {
        self.ensure_entry()?;
        let result = {
            let mut renderer = self.renderer.try_borrow_mut().map_err(|_| {
                GlowViewportError::CallbackReentered {
                    callback: "Rust runtime entry",
                }
            })?;
            let renderer = renderer
                .as_deref_mut()
                .ok_or(GlowViewportError::RuntimeDetached)?;
            renderer.ensure_operational()?;
            callback(renderer)
        }?;
        self.finish_entry()?;
        Ok(result)
    }

    fn with_renderer<R>(
        &self,
        callback: impl FnOnce(&GlowRenderer) -> R,
    ) -> Result<R, GlowViewportError> {
        self.ensure_entry()?;
        let result = {
            let renderer =
                self.renderer
                    .try_borrow()
                    .map_err(|_| GlowViewportError::CallbackReentered {
                        callback: "Rust runtime entry",
                    })?;
            let renderer = renderer
                .as_deref()
                .ok_or(GlowViewportError::RuntimeDetached)?;
            callback(renderer)
        };
        self.finish_entry()?;
        Ok(result)
    }

    pub(super) fn with_renderer_callback(
        &self,
        callback_name: &'static str,
        callback: impl FnOnce(&mut GlowRenderer, &glow::Context) -> Result<(), RenderError>,
    ) {
        let gl = self.gl.borrow().as_ref().cloned();
        let Some(gl) = gl else {
            self.record_fault(GlowViewportError::RuntimeDetached);
            return;
        };
        let Ok(mut renderer) = self.renderer.try_borrow_mut() else {
            self.record_fault(GlowViewportError::CallbackReentered {
                callback: callback_name,
            });
            return;
        };
        let Some(renderer) = renderer.as_deref_mut() else {
            self.record_fault(GlowViewportError::RuntimeDetached);
            return;
        };
        if let Err(error) = renderer.ensure_operational() {
            self.record_renderer_operational_fault(error);
            return;
        }
        if let Err(error) = callback(renderer, &gl) {
            self.record_fault(GlowViewportError::Renderer(error));
        }
    }

    fn release_renderer_explicit(&self, context: &mut Context) -> Result<(), GlowViewportError> {
        if self.renderer.borrow().is_none() {
            self.begin_shutdown();
            let callback_result = release_callbacks(self);
            self.gl.borrow_mut().take();
            self.mark_detached();
            self.set_state(RuntimeState::ResourceDropped);
            return callback_result;
        }
        let gl = self
            .gl
            .borrow()
            .as_ref()
            .cloned()
            .ok_or(GlowViewportError::RuntimeDetached)?;
        let mut renderer_slot =
            self.renderer
                .try_borrow_mut()
                .map_err(|_| GlowViewportError::CallbackReentered {
                    callback: "GlowViewportRuntime::shutdown",
                })?;
        let renderer = renderer_slot
            .as_deref_mut()
            .ok_or(GlowViewportError::RuntimeDetached)?;

        // Preparing the reset permit and releasing resources happens before this runtime mutates
        // its callback table. A retryable failure therefore leaves every Context-visible runtime
        // publication intact.
        renderer.destroy_resources_and_reset(&gl, context)?;
        self.begin_shutdown();
        let callback_result = release_callbacks(self);
        renderer.unconfigure_imgui_context(context);
        let renderer = renderer_slot.take();
        drop(renderer);
        self.gl.borrow_mut().take();
        self.mark_detached();
        self.set_state(RuntimeState::ResourceDropped);
        callback_result
    }

    fn release_renderer_during_context_teardown(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        if self.renderer.borrow().is_none() {
            self.gl.borrow_mut().take();
            self.set_state(RuntimeState::ResourceDropped);
            return Ok(());
        }
        let gl = self
            .gl
            .borrow()
            .as_ref()
            .cloned()
            .ok_or_else(|| context_teardown_error(GlowViewportError::RuntimeDetached))?;
        let mut renderer = self.renderer.try_borrow_mut().map_err(|_| {
            context_teardown_error(GlowViewportError::CallbackReentered {
                callback: "Context renderer-resource teardown",
            })
        })?;
        let renderer_ref = renderer
            .as_deref_mut()
            .ok_or_else(|| context_teardown_error(GlowViewportError::RuntimeDetached))?;
        let consumer = renderer_ref.renderer_consumer.take().ok_or_else(|| {
            ContextAttachmentTeardownError::new(
                "Glow renderer-resource teardown lost its renderer consumer",
            )
        })?;
        let reset = context.with_renderer_texture_reset(&consumer, || {
            renderer_ref
                .destroy_for_context_teardown(&gl)
                .map_err(|error| context_teardown_error(GlowViewportError::Renderer(error)))
        });
        if let Err(error) = reset {
            renderer_ref.renderer_consumer = Some(consumer);
            return Err(error);
        }
        drop(consumer);
        let renderer = renderer.take();
        drop(renderer);
        self.gl.borrow_mut().take();
        self.set_state(RuntimeState::ResourceDropped);
        Ok(())
    }

    fn shutdown_once(&self, action: ShutdownAction<'_>) -> Result<(), GlowViewportError> {
        if self.state.get() == RuntimeState::ResourceDropped {
            self.detach_attachment();
            return Ok(());
        }
        match action {
            ShutdownAction::Quiesce => {
                self.begin_shutdown();
                release_callbacks(self)
            }
            ShutdownAction::Explicit(context) => {
                let renderer_result = self.release_renderer_explicit(context);
                if self.state.get() == RuntimeState::ResourceDropped {
                    self.detach_attachment();
                }
                renderer_result
            }
        }
    }

    fn owner_dropped(&self) {
        if self.state.get() == RuntimeState::ResourceDropped {
            return;
        }
        self.defer_attachment_to_context();
    }

    fn store_attachment(&self, attachment: ContextAttachmentLease) {
        self.attachment.borrow_mut().replace(attachment);
    }

    fn detach_attachment(&self) {
        if let Some(mut attachment) = self.attachment.borrow_mut().take() {
            let _ = attachment
                .detach()
                .expect("a renderer attachment cannot have a platform release dependency");
        }
    }

    fn defer_attachment_to_context(&self) {
        if let Some(attachment) = self.attachment.borrow_mut().take() {
            attachment.defer_to_context();
        }
    }

    fn recover_renderer(&self) -> GlowRenderer {
        self.gl.borrow_mut().take();
        *self
            .renderer
            .borrow_mut()
            .take()
            .expect("failed Glow runtime construction lost its renderer")
    }

    fn mark_context_destroyed(&self) {
        unregister_runtime(self.binding.id());
        let gl = self.gl.borrow().as_ref().cloned();
        if let (Some(gl), Ok(mut renderer)) = (gl, self.renderer.try_borrow_mut()) {
            if let Some(renderer) = renderer.as_deref_mut() {
                let _ = renderer.destroy_after_context_destroyed(&gl);
            }
            renderer.take();
        }
        self.gl.borrow_mut().take();
        self.attachment.borrow_mut().take();
        self.set_state(RuntimeState::ResourceDropped);
    }

    #[cfg(test)]
    pub(super) fn borrow_renderer_for_test(
        &self,
    ) -> std::cell::RefMut<'_, Option<Box<GlowRenderer>>> {
        self.renderer.borrow_mut()
    }

    #[cfg(test)]
    pub(super) fn has_renderer_for_test(&self) -> bool {
        self.renderer.borrow().is_some()
    }

    #[cfg(test)]
    pub(super) fn renderer_address_for_test(&self) -> *const GlowRenderer {
        self.renderer
            .borrow()
            .as_deref()
            .map_or(std::ptr::null(), std::ptr::from_ref)
    }

    #[cfg(test)]
    pub(super) fn panic_next_callback_for_test(&self) {
        self.panic_next_callback.set(true);
    }

    #[cfg(test)]
    pub(super) fn callback_panic_pending_for_test(&self) -> bool {
        self.panic_next_callback.get()
    }

    #[cfg(test)]
    pub(super) fn maybe_panic_callback_for_test(&self) {
        assert!(
            !self.panic_next_callback.replace(false),
            "injected Glow viewport callback panic"
        );
    }

    #[cfg(test)]
    pub(super) fn transition_log_for_test(&self) -> Vec<&'static str> {
        self.transitions.borrow().clone()
    }
}

impl ContextAttachment for RuntimeControl {
    fn quiesce(&self, context: &ContextTeardown<'_>) -> Result<(), ContextAttachmentTeardownError> {
        context.with_bound_context(|| {
            let pending = self.detect_and_take_fault();
            let shutdown = self.shutdown_once(ShutdownAction::Quiesce);
            first_error([pending, shutdown.err()]).map_err(context_teardown_error)
        })
    }

    fn release_renderer_resources(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        context.with_bound_context(|| {
            self.begin_shutdown();
            let callback_error = release_callbacks(self).err().map(context_teardown_error);
            self.mark_detached();
            let renderer_error = self.release_renderer_during_context_teardown(context).err();
            callback_error.or(renderer_error).map_or(Ok(()), Err)
        })
    }

    fn context_destroyed(&self, _context: ContextDestroyed) {
        self.mark_context_destroyed();
    }
}

/// Owning Glow renderer runtime for Dear ImGui multi-viewport support.
///
/// The runtime consumes the renderer into stable boxed storage and shares one lifecycle control
/// with the Context renderer attachment. Moving this wrapper never moves callback-visible state.
pub struct GlowViewportRuntime {
    control: Rc<RuntimeControl>,
}

impl fmt::Debug for GlowViewportRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GlowViewportRuntime")
            .field("control", &self.control)
            .finish()
    }
}

impl GlowViewportRuntime {
    /// Transactionally attaches a renderer under an explicit platform GL-context contract.
    ///
    /// Renderers created by [`GlowRenderer::with_external_context`] are intentionally rejected:
    /// the runtime cannot prove that an independently supplied capability created their objects.
    /// Use [`GlowRenderer::with_shared_context`] when the Glow function table is shared.
    ///
    /// # Platform GL contract
    ///
    /// Callback-table preflight validates lifecycle structure only. It does not establish or prove
    /// an OS-level OpenGL capability.
    ///
    /// # Safety
    ///
    /// The platform runtime must create every secondary viewport OpenGL context in the renderer's
    /// share group and make the corresponding context current before invoking
    /// `Platform_RenderWindow`. A compatible share-group context must remain current whenever a
    /// runtime method performs OpenGL work, during explicit shutdown, when this runtime is dropped,
    /// and during Context teardown. This lets renderer resources be deleted in the ordered
    /// renderer-resource phase. Violating these requirements may issue OpenGL operations against
    /// the wrong native context.
    pub unsafe fn attach(
        context: &mut Context,
        mut renderer: GlowRenderer,
    ) -> Result<Self, GlowViewportAttachError> {
        if let Err(error) = renderer.ensure_operational() {
            return Err(GlowViewportAttachError::new(error.into(), renderer));
        }
        let gl = match renderer.gl_context().cloned() {
            Some(gl) => gl,
            None => {
                return Err(GlowViewportAttachError::new(
                    GlowViewportError::ExternalContextUnsupported,
                    renderer,
                ));
            }
        };
        if let Err(error) = renderer.ensure_context_matches(context) {
            return Err(GlowViewportAttachError::new(error.into(), renderer));
        }
        if let Err(error) = preflight_callbacks(context) {
            return Err(GlowViewportAttachError::new(error, renderer));
        }
        if let Err(error) = preflight_runtime(context.id()) {
            return Err(GlowViewportAttachError::new(error, renderer));
        }

        let control = Rc::new(RuntimeControl::new(context, renderer, gl));
        let attachment = match context.register_attachment::<GlowRendererAttachmentMarker>(
            ContextAttachmentRole::Renderer,
            Rc::clone(&control) as Rc<dyn ContextAttachment>,
        ) {
            Ok(attachment) => attachment,
            Err(error) => {
                let renderer = control.recover_renderer();
                return Err(GlowViewportAttachError::new(error.into(), renderer));
            }
        };
        control.store_attachment(attachment);
        register_runtime(&control);
        claim_callbacks(&control, context);
        control.set_state(RuntimeState::Attached);
        Ok(Self { control })
    }

    /// Returns and clears the oldest callback or ownership fault.
    pub fn poll_fault(&self) -> Result<(), GlowViewportError> {
        self.control.detect_and_take_fault().map_or(Ok(()), Err)
    }

    /// Begins an instance-bound trace for one secondary platform-window render pass.
    ///
    /// Keep the returned guard alive while calling
    /// [`Self::render_with_platform_windows_reconciled`], restore the main native GL context, and
    /// then call [`GlowViewportFrameTrace::finish`]. The report contains only renderer callbacks
    /// whose Glow draw completed successfully. This diagnostic trace does not replace
    /// [`Self::poll_fault`].
    pub fn begin_frame_trace(&self) -> Result<GlowViewportFrameTrace<'_>, GlowViewportError> {
        self.control.begin_frame_trace()
    }

    /// Prepares renderer device objects for a new frame.
    pub fn new_frame(&self) -> Result<(), GlowViewportError> {
        self.control
            .with_renderer_mut(|renderer| renderer.new_frame().map_err(Into::into))
    }

    /// Consumes and renders one Context-owned frame.
    pub fn render(&self, frame: RenderedFrame<'_>) -> Result<(), GlowViewportError> {
        self.render_reconciled(frame).map(drop)
    }

    /// Renders one main-viewport frame and returns texture-reconciliation proof.
    pub fn render_reconciled<'frame>(
        &self,
        frame: RenderedFrame<'frame>,
    ) -> Result<ReconciledFrame<'frame>, GlowViewportError> {
        self.control
            .with_renderer_mut(|renderer| renderer.render_reconciled(frame).map_err(Into::into))
    }

    /// Renders the main viewport, then completes every secondary platform viewport.
    ///
    /// This follows Dear ImGui's OpenGL multi-viewport ordering while retaining the Context render
    /// lease across the native platform-window callbacks. The caller remains responsible for
    /// restoring its main native GL context and calling [`Self::poll_fault`] before presenting the
    /// main window. Delaying fault propagation until after context restoration keeps teardown on a
    /// known GL capability even when a native callback fails.
    pub fn render_with_platform_windows_reconciled<'frame>(
        &self,
        mut frame: RenderedFrame<'frame>,
    ) -> Result<ReconciledFrame<'frame>, GlowViewportError> {
        self.control.with_renderer_mut(|renderer| {
            renderer.render_borrowed(&mut frame).map_err(Into::into)
        })?;
        frame.update_and_render_platform_windows_default();
        frame
            .into_reconciled()
            .map_err(RenderError::from)
            .map_err(Into::into)
    }

    /// Runs a read-only, non-escaping renderer inspection.
    pub fn with_renderer<R>(
        &self,
        callback: impl FnOnce(&GlowRenderer) -> R,
    ) -> Result<R, GlowViewportError> {
        self.control.with_renderer(callback)
    }

    /// Configures the clear color used for secondary viewports.
    pub fn set_viewport_clear_color(&self, color: [f32; 4]) -> Result<(), GlowViewportError> {
        self.control.with_renderer_mut(|renderer| {
            renderer.set_viewport_clear_color(color);
            Ok(())
        })
    }

    /// Enables or disables framebuffer sRGB during Glow rendering.
    pub fn set_framebuffer_srgb_enabled(&self, enabled: bool) -> Result<(), GlowViewportError> {
        self.control
            .with_renderer_mut(|renderer| Ok(renderer.set_framebuffer_srgb_enabled(enabled)?))
    }

    /// Overrides the vertex-color gamma used by the renderer.
    pub fn set_color_gamma_override(&self, gamma: Option<f32>) -> Result<(), GlowViewportError> {
        self.control.with_renderer_mut(|renderer| {
            renderer.set_color_gamma_override(gamma);
            Ok(())
        })
    }

    /// Registers a renderer-owned legacy texture through the runtime's verified GL capability.
    pub fn register_texture(
        &self,
        width: u32,
        height: u32,
        format: TextureFormat,
        data: &[u8],
    ) -> Result<TextureId, GlowViewportError> {
        self.control.with_renderer_mut(|renderer| {
            renderer
                .register_texture(width, height, format, data)
                .map_err(Into::into)
        })
    }

    /// Updates a renderer-owned legacy texture.
    pub fn update_texture(
        &self,
        texture: TextureId,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Result<(), GlowViewportError> {
        self.control.with_renderer_mut(|renderer| {
            renderer
                .update_texture(texture, width, height, data)
                .map_err(Into::into)
        })
    }

    /// Explicitly shuts down renderer callbacks and GPU resources.
    ///
    /// This operation is idempotent and reports deferred callback faults. It intentionally does
    /// not return the consumed renderer because its consumer and GPU resources are terminal. A GL
    /// context from the renderer's share group must be current for this call.
    pub fn shutdown(&mut self, context: &mut Context) -> Result<(), GlowViewportError> {
        self.control.ensure_context(context)?;
        let pending = self.control.detect_and_take_fault();
        let binding = self.control.binding.clone();
        let result = binding
            .try_with_bound_context(|| {
                self.control
                    .shutdown_once(ShutdownAction::Explicit(context))
            })
            .map_err(Into::into)
            .and_then(|result| result);
        match (pending, result) {
            (Some(fault), Err(shutdown_error)) => {
                self.control.record_fault(shutdown_error);
                Err(fault)
            }
            (Some(fault), Ok(())) => Err(fault),
            (None, result) => result,
        }
    }

    #[cfg(test)]
    pub(super) fn renderer_address_for_test(&self) -> *const GlowRenderer {
        self.control.renderer_address_for_test()
    }

    #[cfg(test)]
    pub(super) fn state_for_test(&self) -> RuntimeState {
        self.control.state()
    }

    #[cfg(test)]
    pub(super) fn transition_log_for_test(&self) -> Vec<&'static str> {
        self.control.transition_log_for_test()
    }

    #[cfg(test)]
    pub(super) fn control_for_test(&self) -> Rc<RuntimeControl> {
        Rc::clone(&self.control)
    }

    #[cfg(test)]
    pub(super) fn panic_next_callback_for_test(&self) {
        self.control.panic_next_callback_for_test();
    }
}

impl Drop for GlowViewportRuntime {
    fn drop(&mut self) {
        self.control.owner_dropped();
    }
}

fn first_error<const N: usize>(
    errors: [Option<GlowViewportError>; N],
) -> Result<(), GlowViewportError> {
    errors.into_iter().flatten().next().map_or(Ok(()), Err)
}

fn context_teardown_error(error: GlowViewportError) -> ContextAttachmentTeardownError {
    ContextAttachmentTeardownError::new(format!("Glow viewport teardown failed: {error}"))
}

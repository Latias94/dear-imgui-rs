use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;

use dear_imgui_rs::render::{ReconciledFrame, RenderedFrame};
use dear_imgui_rs::{
    Context, ContextAttachment, ContextAttachmentError, ContextAttachmentLease,
    ContextAttachmentRole, ContextAttachmentTeardownError, ContextBinding, ContextBindingError,
    ContextDestroyed, ContextId, ContextLifecycle, ContextTeardown,
};
use thiserror::Error;

use super::callbacks::{
    claim_callbacks, destroy_renderer_viewport_resources, detect_runtime_contract_drift,
    preflight_callbacks, preflight_renderer_viewport_resources, release_callbacks,
    revoke_renderer_viewport_capability_if_owned,
};
use super::registry::{
    GlobalHandles, drop_orphaned_viewport_data, preflight_runtime, register_runtime,
    renderer_globals, unregister_runtime,
};
use super::trace::{FrameTraceState, WgpuViewportFrameTraceReport};
use crate::{ExternalTextureId, GammaMode, RendererError, WgpuRenderer};

struct WgpuRendererAttachmentMarker;

/// Failure to attach or operate an owning WGPU multi-viewport runtime.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WgpuViewportError {
    /// The Dear ImGui Context rejected the renderer attachment.
    #[error(transparent)]
    Attachment(#[from] ContextAttachmentError),
    /// The originating Context can no longer be entered normally.
    #[error(transparent)]
    Context(#[from] ContextBindingError),
    /// The underlying WGPU renderer operation failed.
    #[error(transparent)]
    Renderer(#[from] RendererError),
    /// The renderer and runtime Context identities differ.
    #[error("WGPU viewport runtime belongs to Context {expected:?}, not {actual:?}")]
    ContextMismatch {
        expected: ContextId,
        actual: ContextId,
    },
    /// A callback entry was not running under this runtime's Context.
    #[error("the current Dear ImGui Context does not match WGPU runtime Context {expected:?}")]
    BoundContextMismatch { expected: ContextId },
    /// The renderer has not been initialized with GPU backend data.
    #[error("WGPU renderer is not initialized")]
    RendererNotInitialized,
    /// The renderer was initialized for another Dear ImGui Context.
    #[error("WGPU renderer is bound to a different Dear ImGui Context")]
    RendererContextMismatch,
    /// The renderer's bound Dear ImGui Context no longer exists.
    #[error("the Dear ImGui Context bound to this WGPU renderer has been dropped")]
    RendererContextDropped,
    /// Per-window surfaces require the WGPU instance used by the renderer.
    #[error("WGPU multi-viewport requires WgpuInitInfo::with_instance")]
    MissingInstance,
    /// Surface capability negotiation requires the WGPU adapter used by the renderer.
    #[error("WGPU multi-viewport requires WgpuInitInfo::with_adapter")]
    MissingAdapter,
    /// Renderer callbacks require an attached platform backend that supports viewports.
    #[error("WGPU multi-viewport requires an attached multi-viewport platform runtime")]
    PlatformBackendUnavailable,
    /// The typed platform owner supplied to the renderer belongs to another Context.
    #[error("WGPU viewport platform owner belongs to Context {actual:?}, not {expected:?}")]
    PlatformOwnerContextMismatch {
        expected: ContextId,
        actual: ContextId,
    },
    /// The typed Winit platform owner rejected renderer attachment.
    #[cfg(feature = "multi-viewport-winit")]
    #[error(transparent)]
    WinitPlatformOwner(#[from] dear_imgui_winit::WinitPlatformError),
    /// The typed SDL3 platform owner rejected renderer attachment.
    #[cfg(feature = "multi-viewport-sdl3")]
    #[error(transparent)]
    Sdl3PlatformOwner(#[from] dear_imgui_sdl3::Sdl3BackendError),
    /// A required platform callback is absent.
    #[error("required ImGuiPlatformIO callback `{callback}` is not installed")]
    PlatformCallbackUnavailable { callback: &'static str },
    /// PlatformIO itself is unavailable for the currently bound Context.
    #[error("the bound Dear ImGui Context has no PlatformIO")]
    PlatformIoUnavailable,
    /// The platform runtime has not published the main native window handle.
    #[error("the attached platform runtime has no main viewport window handle")]
    MainViewportHandleUnavailable,
    /// Another renderer already owns one renderer callback slot.
    #[error("ImGuiPlatformIO callback `{callback}` is already owned by another renderer")]
    RendererCallbackOccupied { callback: &'static str },
    /// Another renderer already advertises multi-viewport support.
    #[error("another renderer already advertises RENDERER_HAS_VIEWPORTS")]
    RendererViewportCapabilityOccupied,
    /// This runtime's renderer capability bit was cleared while it remained attached.
    #[error("WGPU renderer backend flag RENDERER_HAS_VIEWPORTS was removed while attached")]
    RendererViewportCapabilityLost,
    /// A callback claimed by this runtime was replaced while attached.
    #[error("WGPU renderer callback `{callback}` was replaced while the runtime was attached")]
    RendererCallbackReplaced { callback: &'static str },
    /// A secondary viewport already contains renderer-owned user data.
    #[error("a secondary viewport already has RendererUserData owned by another backend")]
    RendererUserDataOccupied,
    /// A callback observed non-null renderer data that is absent from this runtime's sidecar.
    #[error("WGPU callback `{callback}` observed foreign or unregistered RendererUserData")]
    RendererUserDataOwnershipLost { callback: &'static str },
    /// Existing platform windows would miss the renderer create callback.
    #[error("secondary platform windows already exist; destroy them before attaching WGPU")]
    PlatformWindowsAlreadyCreated,
    /// The aggregate size callback cannot be bridged safely by this Dear ImGui artifact.
    #[error("dear-imgui-sys was built without PlatformIO aggregate ABI hooks")]
    AggregateCallbackHooksUnavailable,
    /// The registry already contains a live runtime for this Context.
    #[error("a WGPU viewport runtime is already attached to this Context")]
    RuntimeAlreadyAttached,
    /// The owning runtime has shut down or Context-owned teardown has started.
    #[error("the WGPU viewport runtime is no longer attached")]
    RuntimeDetached,
    /// The renderer is already mutably borrowed by another runtime entry.
    #[error("WGPU renderer runtime is already active in `{callback}`")]
    CallbackReentered { callback: &'static str },
    /// A callback panic was contained at the C ABI boundary.
    #[error("WGPU renderer callback `{callback}` panicked")]
    CallbackPanicked { callback: &'static str },
    /// Dear ImGui passed an invalid viewport to a renderer callback.
    #[error("WGPU renderer callback `{callback}` received a null viewport")]
    InvalidViewport { callback: &'static str },
    /// Creating or configuring a viewport surface failed.
    #[error("WGPU viewport surface operation `{operation}` failed")]
    SurfaceOperationFailed { operation: &'static str },
    /// The renderer's target format is unavailable with its required secondary-surface encoding.
    #[error(
        "WGPU render target format {format:?} is unavailable for secondary viewports in {color_space}"
    )]
    UnsupportedSurfaceFormat {
        format: wgpu::TextureFormat,
        color_space: &'static str,
    },
    /// The renderer's multisample count cannot describe a WGPU render attachment.
    #[error("WGPU viewport attachments require a non-zero multisample count, received {count}")]
    InvalidMultisampleCount { count: u32 },
    /// A renderer attachment format does not support the configured sample count.
    #[error(
        "WGPU {attachment} format {format:?} does not support RENDER_ATTACHMENT at sample count {sample_count}"
    )]
    UnsupportedViewportAttachment {
        attachment: &'static str,
        format: wgpu::TextureFormat,
        sample_count: u32,
    },
    /// Surface acquisition returned a terminal result.
    #[error("WGPU viewport surface acquisition was rejected: {event}")]
    SurfaceRejected { event: &'static str },
    /// Native multi-viewport surfaces are unavailable on this target.
    #[error("WGPU native multi-viewport rendering is unavailable on this target")]
    UnsupportedTarget,
    /// A frame trace is already active for this runtime.
    #[error("a WGPU secondary-viewport frame trace is already active")]
    FrameTraceAlreadyActive,
}

/// Transactional attachment failure that returns the renderer unchanged.
pub struct WgpuViewportAttachError {
    error: WgpuViewportError,
    renderer: Box<WgpuRenderer>,
}

impl WgpuViewportAttachError {
    pub(crate) fn new(error: WgpuViewportError, renderer: WgpuRenderer) -> Self {
        Self {
            error,
            renderer: Box::new(renderer),
        }
    }

    /// Returns the reason attachment failed.
    pub fn error(&self) -> &WgpuViewportError {
        &self.error
    }

    /// Returns the renderer so the caller can retry, use it for one viewport, or destroy it.
    pub fn into_renderer(self) -> WgpuRenderer {
        *self.renderer
    }

    /// Returns both the typed failure and the unchanged renderer.
    pub fn into_parts(self) -> (WgpuViewportError, WgpuRenderer) {
        (self.error, *self.renderer)
    }
}

impl fmt::Debug for WgpuViewportAttachError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WgpuViewportAttachError")
            .field("error", &self.error)
            .field("renderer", &"returned to caller")
            .finish()
    }
}

impl fmt::Display for WgpuViewportAttachError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for WgpuViewportAttachError {
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
    renderer: RefCell<Option<Box<WgpuRenderer>>>,
    globals: RefCell<Option<GlobalHandles>>,
    attachment: RefCell<Option<ContextAttachmentLease>>,
    callback_state: Cell<CallbackState>,
    faults: RefCell<RuntimeFaults>,
    frame_trace: RefCell<FrameTraceState>,
    #[cfg(test)]
    panic_next_callback: Cell<bool>,
    #[cfg(test)]
    fail_next_viewport_cleanup: Cell<bool>,
    #[cfg(test)]
    transitions: RefCell<Vec<&'static str>>,
}

#[derive(Default)]
struct RuntimeFaults {
    terminal: Option<WgpuViewportError>,
    non_terminal: Option<WgpuViewportError>,
}

impl RuntimeFaults {
    fn record_terminal(&mut self, fault: WgpuViewportError) {
        if self.terminal.is_none() {
            self.terminal = Some(fault);
        }
    }

    fn record_non_terminal(&mut self, fault: WgpuViewportError) {
        if self.non_terminal.is_none() {
            self.non_terminal = Some(fault);
        }
    }

    fn has_pending(&self) -> bool {
        self.terminal.is_some() || self.non_terminal.is_some()
    }

    fn take_next(&mut self) -> Option<WgpuViewportError> {
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
    fn new(context: &Context, renderer: WgpuRenderer, globals: Option<GlobalHandles>) -> Self {
        Self {
            context_raw: context.as_raw(),
            binding: context.binding(),
            state: Cell::new(RuntimeState::Constructing),
            renderer: RefCell::new(Some(Box::new(renderer))),
            globals: RefCell::new(globals),
            attachment: RefCell::new(None),
            callback_state: Cell::new(CallbackState::Unclaimed),
            faults: RefCell::new(RuntimeFaults::default()),
            frame_trace: RefCell::new(FrameTraceState::default()),
            #[cfg(test)]
            panic_next_callback: Cell::new(false),
            #[cfg(test)]
            fail_next_viewport_cleanup: Cell::new(false),
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

    pub(super) fn globals(&self) -> Option<GlobalHandles> {
        self.globals.borrow().clone()
    }

    pub(super) fn is_callback_accessible(&self) -> bool {
        self.state.get() == RuntimeState::Attached
            && self.callback_state.get() != CallbackState::Released
    }

    pub(super) fn is_cleanup_callback_accessible(&self) -> bool {
        matches!(
            self.state.get(),
            RuntimeState::Attached | RuntimeState::ShuttingDown
        ) && self.callback_state.get() == CallbackState::Claimed
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

    pub(super) fn begin_shutdown(&self) {
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

    pub(super) fn record_fault(&self, fault: WgpuViewportError) {
        self.faults.borrow_mut().record_non_terminal(fault);
    }

    fn record_terminal_fault(&self, fault: WgpuViewportError) {
        self.faults.borrow_mut().record_terminal(fault);
    }

    fn begin_frame_trace(&self) -> Result<(), WgpuViewportError> {
        self.ensure_entry()?;
        if self.frame_trace.borrow_mut().begin() {
            Ok(())
        } else {
            Err(WgpuViewportError::FrameTraceAlreadyActive)
        }
    }

    fn finish_frame_trace(&self) -> WgpuViewportFrameTraceReport {
        self.frame_trace.borrow_mut().finish()
    }

    fn abort_frame_trace(&self) {
        self.frame_trace.borrow_mut().abort();
    }

    pub(super) fn record_viewport_render_submitted(&self, viewport_id: dear_imgui_rs::Id) {
        self.frame_trace
            .borrow_mut()
            .record_render_submitted(viewport_id);
    }

    pub(super) fn record_viewport_present_submitted(&self, viewport_id: dear_imgui_rs::Id) {
        self.frame_trace
            .borrow_mut()
            .record_present_submitted(viewport_id);
    }

    pub(super) fn record_runtime_contract_fault(&self, fault: WgpuViewportError) {
        let _ = self.binding.try_with_bound_context(|| {
            revoke_renderer_viewport_capability_if_owned(self);
        });
        self.record_terminal_fault(fault);
        self.begin_shutdown();
    }

    /// Returns whether this runtime can still prove a WGPU core renderer publication on the
    /// bound Context. A reentrant mutable borrow is not proof of ownership, so it preserves the
    /// shared capability bit until a later teardown can inspect the exact publications.
    pub(super) fn owns_core_renderer_publication_bound(&self) -> bool {
        let Ok(renderer) = self.renderer.try_borrow() else {
            return false;
        };
        renderer
            .as_deref()
            .is_some_and(WgpuRenderer::owns_context_publication_bound)
    }

    pub(super) fn record_entry_fault(&self, fault: WgpuViewportError) {
        if matches!(
            &fault,
            WgpuViewportError::Renderer(RendererError::RendererStateDrift { .. })
                | WgpuViewportError::RendererUserDataOwnershipLost { .. }
                | WgpuViewportError::CallbackPanicked { .. }
                | WgpuViewportError::SurfaceRejected { .. }
        ) {
            self.record_runtime_contract_fault(fault);
        } else {
            self.record_fault(fault);
        }
    }

    pub(super) fn core_renderer_contract_fault(&self) -> Option<WgpuViewportError> {
        let renderer = match self.renderer.try_borrow() {
            Ok(renderer) => renderer,
            Err(_) => {
                return Some(WgpuViewportError::CallbackReentered {
                    callback: "renderer contract validation",
                });
            }
        };
        let renderer = renderer
            .as_deref()
            .ok_or(WgpuViewportError::RuntimeDetached);
        match renderer {
            Ok(renderer) => renderer.ensure_renderer_contract().err().map(Into::into),
            Err(error) => Some(error),
        }
    }

    fn detect_and_take_fault(&self) -> Option<WgpuViewportError> {
        detect_runtime_contract_drift(self);
        self.faults.borrow_mut().take_next()
    }

    fn ensure_context(&self, context: &Context) -> Result<(), WgpuViewportError> {
        if context.id() == self.binding.id() {
            Ok(())
        } else {
            Err(WgpuViewportError::ContextMismatch {
                expected: self.binding.id(),
                actual: context.id(),
            })
        }
    }

    fn ensure_entry(&self) -> Result<(), WgpuViewportError> {
        if let Some(fault) = self.detect_and_take_fault() {
            return Err(fault);
        }
        if self.state.get() == RuntimeState::Attached {
            Ok(())
        } else {
            Err(WgpuViewportError::RuntimeDetached)
        }
    }

    fn finish_entry(&self) -> Result<(), WgpuViewportError> {
        self.detect_and_take_fault().map_or(Ok(()), Err)
    }

    fn with_renderer_mut<R>(
        &self,
        callback: impl FnOnce(&mut WgpuRenderer) -> Result<R, WgpuViewportError>,
    ) -> Result<R, WgpuViewportError> {
        self.ensure_entry()?;
        let result = {
            let mut renderer = self.renderer.try_borrow_mut().map_err(|_| {
                WgpuViewportError::CallbackReentered {
                    callback: "Rust runtime entry",
                }
            })?;
            let renderer = renderer
                .as_deref_mut()
                .ok_or(WgpuViewportError::RuntimeDetached)?;
            callback(renderer)
        };
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                if matches!(
                    &error,
                    WgpuViewportError::Renderer(RendererError::RendererStateDrift { .. })
                ) {
                    self.record_runtime_contract_fault(error);
                    return Err(self
                        .detect_and_take_fault()
                        .unwrap_or(WgpuViewportError::RuntimeDetached));
                }
                return Err(error);
            }
        };
        self.finish_entry()?;
        Ok(result)
    }

    fn with_renderer<R>(
        &self,
        callback: impl FnOnce(&WgpuRenderer) -> R,
    ) -> Result<R, WgpuViewportError> {
        self.ensure_entry()?;
        let result = {
            let renderer =
                self.renderer
                    .try_borrow()
                    .map_err(|_| WgpuViewportError::CallbackReentered {
                        callback: "Rust runtime entry",
                    })?;
            let renderer = renderer
                .as_deref()
                .ok_or(WgpuViewportError::RuntimeDetached)?;
            callback(renderer)
        };
        self.finish_entry()?;
        Ok(result)
    }

    pub(super) fn with_renderer_callback(
        &self,
        callback_name: &'static str,
        callback: impl FnOnce(&mut WgpuRenderer, &GlobalHandles) -> Result<(), WgpuViewportError>,
    ) {
        detect_runtime_contract_drift(self);
        if self.state.get() != RuntimeState::Attached || self.faults.borrow().has_pending() {
            return;
        }
        let Ok(mut renderer) = self.renderer.try_borrow_mut() else {
            self.record_fault(WgpuViewportError::CallbackReentered {
                callback: callback_name,
            });
            return;
        };
        let Some(renderer) = renderer.as_deref_mut() else {
            self.record_fault(WgpuViewportError::RuntimeDetached);
            return;
        };
        let Some(globals) = self.globals() else {
            self.record_fault(WgpuViewportError::RuntimeDetached);
            return;
        };
        if let Err(error) = callback(renderer, &globals) {
            self.record_entry_fault(error);
        }
    }

    fn preflight_renderer_shutdown(&self, context: &mut Context) -> Result<(), WgpuViewportError> {
        let renderer =
            self.renderer
                .try_borrow()
                .map_err(|_| WgpuViewportError::CallbackReentered {
                    callback: "WGPU viewport runtime shutdown preflight",
                })?;
        renderer
            .as_deref()
            .ok_or(WgpuViewportError::RuntimeDetached)?
            .preflight_shutdown(context)
            .map_err(Into::into)
    }

    fn release_renderer_explicit(&self, context: &mut Context) -> Result<(), WgpuViewportError> {
        if self.renderer.borrow().is_none() {
            self.globals.borrow_mut().take();
            self.set_state(RuntimeState::ResourceDropped);
            return Ok(());
        }
        let mut renderer =
            self.renderer
                .try_borrow_mut()
                .map_err(|_| WgpuViewportError::CallbackReentered {
                    callback: "WGPU viewport runtime shutdown",
                })?;
        renderer
            .as_deref_mut()
            .ok_or(WgpuViewportError::RuntimeDetached)?
            .shutdown(context)?;
        let renderer = renderer.take();
        drop(renderer);
        self.globals.borrow_mut().take();
        self.set_state(RuntimeState::ResourceDropped);
        Ok(())
    }

    fn release_renderer_during_context_teardown(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        if self.state.get() == RuntimeState::ResourceDropped {
            return Ok(());
        }

        // Do not acquire the Context reset transaction until every visible sidecar still proves
        // exact ownership. This is read-only, so either failure leaves callback publication,
        // renderer resources, and the consumer intact for Context's fail-stop path.
        preflight_renderer_viewport_resources(self).map_err(|error| {
            let message = error.to_string();
            self.record_fault(error);
            ContextAttachmentTeardownError::new(message)
        })?;

        if self.renderer.borrow().is_none() {
            self.mark_detached();
            self.globals.borrow_mut().take();
            self.set_state(RuntimeState::ResourceDropped);
            return Ok(());
        }

        let mut renderer_slot = self.renderer.try_borrow_mut().map_err(|_| {
            ContextAttachmentTeardownError::new(
                "WGPU renderer was reentered during Context renderer-resource teardown",
            )
        })?;
        let renderer = renderer_slot.as_deref_mut().ok_or_else(|| {
            ContextAttachmentTeardownError::new(
                "WGPU renderer disappeared during Context renderer-resource teardown",
            )
        })?;

        renderer.shutdown_during_context_teardown(context, || {
            self.begin_shutdown();
            destroy_renderer_viewport_resources(self).map_err(|error| {
                let message = error.to_string();
                self.record_fault(error);
                ContextAttachmentTeardownError::new(message)
            })?;
            release_callbacks(self).map_err(|error| {
                let message = error.to_string();
                self.record_fault(error);
                ContextAttachmentTeardownError::new(message)
            })?;
            self.mark_detached();
            Ok(())
        })?;

        let renderer = renderer_slot.take();
        drop(renderer);
        self.globals.borrow_mut().take();
        self.set_state(RuntimeState::ResourceDropped);
        Ok(())
    }

    fn shutdown_once(&self, mut action: ShutdownAction<'_>) -> Result<(), WgpuViewportError> {
        if self.state.get() == RuntimeState::ResourceDropped {
            self.detach_attachment();
            return Ok(());
        }

        // An explicit shutdown is retryable. Validate that no live frame or detached snapshot
        // prevents the renderer reset before mutating viewport sidecars, surfaces, callbacks, or
        // runtime state.
        if let ShutdownAction::Explicit(context) = &mut action {
            self.preflight_renderer_shutdown(context)?;
        }

        // A foreign write to one sidecar makes the active DestroyWindow callback the only safe
        // reclaim path. Verify every reachable slot before changing the runtime state, dropping
        // any sidecar, or clearing callback publication.
        preflight_renderer_viewport_resources(self)?;

        self.begin_shutdown();
        let viewport_result = if matches!(action, ShutdownAction::Quiesce) {
            Ok(())
        } else {
            destroy_renderer_viewport_resources(self)
        };
        let mut viewport_error = viewport_result.err();
        if matches!(action, ShutdownAction::Explicit(_))
            && let Some(error) = viewport_error.take()
        {
            return Err(error);
        }
        let callback_result = release_callbacks(self);

        match action {
            ShutdownAction::Quiesce => callback_result,
            ShutdownAction::Explicit(context) => {
                self.mark_detached();
                let renderer_result = self.release_renderer_explicit(context);
                if self.state.get() == RuntimeState::ResourceDropped {
                    self.detach_attachment();
                }
                first_error([viewport_error, callback_result.err(), renderer_result.err()])
            }
        }
    }

    fn owner_dropped(&self) {
        if self.state.get() == RuntimeState::ResourceDropped {
            return;
        }
        match self.binding.lifecycle() {
            // Drop lacks an exclusive `&mut Context`, so it cannot prepare and commit the
            // renderer-texture reset transaction. Keep the renderer, sidecars, callback table,
            // and attachment alive for Context's ordered terminal teardown.
            ContextLifecycle::Alive => self.defer_attachment_to_context(),
            // Context already owns this attachment and is currently executing its teardown
            // phases. Entering native code here would violate that ordering.
            ContextLifecycle::Dropping => {}
            // Context teardown normally calls `context_destroyed` before the wrapper can be
            // released. This idempotent fallback touches only Rust-owned allocations and never
            // tries to make a destroyed native Context current.
            ContextLifecycle::NativeDestroyed => self.mark_context_destroyed(),
            _ => {}
        }
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

    fn recover_renderer(&self) -> WgpuRenderer {
        self.globals.borrow_mut().take();
        *self
            .renderer
            .borrow_mut()
            .take()
            .expect("failed WGPU runtime construction lost its renderer")
    }

    fn mark_context_destroyed(&self) {
        unregister_runtime(self.binding.id());
        // Native viewport slots are no longer touched after Context destruction, but the sidecar
        // still owns every remaining renderer allocation.
        drop_orphaned_viewport_data(self.binding.id());
        if self.renderer.borrow().is_some() {
            let mut renderer = self.renderer.borrow_mut();
            if let Some(renderer) = renderer.as_deref_mut() {
                renderer.shutdown_after_context_destroyed();
            }
            renderer.take();
            self.globals.borrow_mut().take();
        }
        self.attachment.borrow_mut().take();
        self.set_state(RuntimeState::ResourceDropped);
    }

    #[cfg(test)]
    pub(super) fn state(&self) -> RuntimeState {
        self.state.get()
    }

    #[cfg(test)]
    pub(super) fn borrow_renderer_for_test(
        &self,
    ) -> std::cell::RefMut<'_, Option<Box<WgpuRenderer>>> {
        self.renderer.borrow_mut()
    }

    #[cfg(test)]
    pub(super) fn has_renderer_for_test(&self) -> bool {
        self.renderer.borrow().is_some()
    }

    #[cfg(test)]
    pub(super) fn renderer_address_for_test(&self) -> *const WgpuRenderer {
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
    pub(super) fn maybe_panic_callback_for_test(&self) {
        assert!(
            !self.panic_next_callback.replace(false),
            "injected WGPU viewport callback panic"
        );
    }

    #[cfg(test)]
    pub(super) fn fail_next_viewport_cleanup_for_test(&self) {
        self.fail_next_viewport_cleanup.set(true);
    }

    #[cfg(test)]
    pub(super) fn take_viewport_cleanup_failure_for_test(&self) -> bool {
        self.fail_next_viewport_cleanup.replace(false)
    }

    #[cfg(test)]
    pub(super) fn transition_log_for_test(&self) -> Vec<&'static str> {
        self.transitions.borrow().clone()
    }
}

impl ContextAttachment for RuntimeControl {
    fn quiesce(&self, context: &ContextTeardown<'_>) -> Result<(), ContextAttachmentTeardownError> {
        context.with_bound_context(|| {
            self.shutdown_once(ShutdownAction::Quiesce)
                .map_err(|error| {
                    let message = error.to_string();
                    self.record_fault(error);
                    ContextAttachmentTeardownError::new(message)
                })
        })
    }

    fn release_renderer_resources(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        context.with_bound_context(|| self.release_renderer_during_context_teardown(context))
    }

    fn context_destroyed(&self, _context: ContextDestroyed) {
        self.mark_context_destroyed();
    }
}

/// Backend-local owning runtime shared by the Winit and SDL3 typed wrappers.
pub(crate) struct OwningViewportRuntime {
    control: Rc<RuntimeControl>,
}

/// A non-nestable trace scope for one secondary-viewport rendering pass.
///
/// Call [`Self::finish`] before acquiring or presenting the application's main surface to obtain
/// a report that proves which secondary surfaces completed renderer submission and presentation
/// within this scope. Dropping the guard discards the partial trace.
#[must_use = "finish the frame trace to obtain its report"]
pub struct WgpuViewportFrameTraceGuard<'runtime> {
    control: &'runtime RuntimeControl,
    active: bool,
}

impl WgpuViewportFrameTraceGuard<'_> {
    /// Ends the trace and returns its normalized, same-scope submission evidence.
    pub fn finish(mut self) -> WgpuViewportFrameTraceReport {
        let report = self.control.finish_frame_trace();
        self.active = false;
        report
    }
}

impl Drop for WgpuViewportFrameTraceGuard<'_> {
    fn drop(&mut self) {
        if self.active {
            self.control.abort_frame_trace();
        }
    }
}

impl fmt::Debug for OwningViewportRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OwningViewportRuntime")
            .field("control", &self.control)
            .finish()
    }
}

impl OwningViewportRuntime {
    pub(crate) fn begin_frame_trace(
        &self,
    ) -> Result<WgpuViewportFrameTraceGuard<'_>, WgpuViewportError> {
        self.control.begin_frame_trace()?;
        Ok(WgpuViewportFrameTraceGuard {
            control: self.control.as_ref(),
            active: true,
        })
    }

    pub(crate) fn attach(
        context: &mut Context,
        renderer: WgpuRenderer,
    ) -> Result<Self, WgpuViewportAttachError> {
        // The Context-owned attachment is the first ownership gate. In particular, a wrapper
        // that was dropped without explicit shutdown leaves this runtime alive for Context
        // teardown, so a replacement must be rejected without inspecting or mutating its
        // renderer argument.
        if let Err(error) = preflight_runtime(context.id()) {
            return Err(WgpuViewportAttachError::new(error, renderer));
        }
        if let Err(error) = renderer.ensure_context_matches(context) {
            let error = match error {
                RendererError::ContextDropped => WgpuViewportError::RendererContextDropped,
                RendererError::ContextMismatch => WgpuViewportError::RendererContextMismatch,
                other => WgpuViewportError::Renderer(other),
            };
            return Err(WgpuViewportAttachError::new(error, renderer));
        }
        if let Err(error) = renderer.ensure_renderer_contract() {
            return Err(WgpuViewportAttachError::new(error.into(), renderer));
        }
        let globals = match renderer_globals(&renderer) {
            Ok(globals) => globals,
            Err(error) => return Err(WgpuViewportAttachError::new(error, renderer)),
        };
        Self::attach_preflighted(context, renderer, Some(globals))
    }

    fn attach_preflighted(
        context: &mut Context,
        renderer: WgpuRenderer,
        globals: Option<GlobalHandles>,
    ) -> Result<Self, WgpuViewportAttachError> {
        if let Err(error) = preflight_callbacks(context) {
            return Err(WgpuViewportAttachError::new(error, renderer));
        }
        if let Err(error) = preflight_runtime(context.id()) {
            return Err(WgpuViewportAttachError::new(error, renderer));
        }

        if let Err(error) = renderer.ensure_renderer_contract() {
            return Err(WgpuViewportAttachError::new(error.into(), renderer));
        }
        let control = Rc::new(RuntimeControl::new(context, renderer, globals));
        let attachment = match context.register_attachment::<WgpuRendererAttachmentMarker>(
            ContextAttachmentRole::Renderer,
            Rc::clone(&control) as Rc<dyn ContextAttachment>,
        ) {
            Ok(attachment) => attachment,
            Err(error) => {
                let renderer = control.recover_renderer();
                return Err(WgpuViewportAttachError::new(error.into(), renderer));
            }
        };
        control.store_attachment(attachment);
        register_runtime(&control);
        claim_callbacks(&control, context);
        control.set_state(RuntimeState::Attached);
        Ok(Self { control })
    }

    #[cfg(test)]
    pub(super) fn attach_for_test(
        context: &mut Context,
        mut renderer: WgpuRenderer,
    ) -> Result<Self, WgpuViewportAttachError> {
        if let Err(error) = preflight_runtime(context.id()) {
            return Err(WgpuViewportAttachError::new(error, renderer));
        }
        if renderer.context_state.is_none() {
            let (flags, _) = match WgpuRenderer::configure_imgui_context(context) {
                Ok(configured) => configured,
                Err(error) => {
                    return Err(WgpuViewportAttachError::new(error.into(), renderer));
                }
            };
            if let Err(error) = renderer.bind_context(context, flags) {
                return Err(WgpuViewportAttachError::new(error.into(), renderer));
            }
            renderer.renderer_consumer = match context.create_renderer_consumer() {
                Ok(consumer) => Some(consumer),
                Err(error) => {
                    return Err(WgpuViewportAttachError::new(
                        RendererError::from(error).into(),
                        renderer,
                    ));
                }
            };
        }
        Self::attach_preflighted(context, renderer, None)
    }

    pub(crate) fn poll_fault(&self) -> Result<(), WgpuViewportError> {
        self.control.detect_and_take_fault().map_or(Ok(()), Err)
    }

    pub(crate) fn new_frame(&self) -> Result<(), WgpuViewportError> {
        self.control
            .with_renderer_mut(|renderer| renderer.new_frame().map_err(Into::into))
    }

    pub(crate) fn reconcile_frame(
        &self,
        frame: &mut RenderedFrame<'_>,
    ) -> Result<(), WgpuViewportError> {
        self.control
            .with_renderer_mut(|renderer| renderer.reconcile_frame(frame).map_err(Into::into))
    }

    pub(crate) fn render(
        &self,
        frame: RenderedFrame<'_>,
        render_pass: &mut wgpu::RenderPass<'_>,
    ) -> Result<(), WgpuViewportError> {
        self.control
            .with_renderer_mut(|renderer| renderer.render(frame, render_pass).map_err(Into::into))
    }

    pub(crate) fn render_reconciled<'frame>(
        &self,
        frame: RenderedFrame<'frame>,
        render_pass: &mut wgpu::RenderPass<'_>,
    ) -> Result<ReconciledFrame<'frame>, WgpuViewportError> {
        self.control.with_renderer_mut(|renderer| {
            renderer
                .render_reconciled(frame, render_pass)
                .map_err(Into::into)
        })
    }

    pub(crate) fn render_context(
        &self,
        context: &mut Context,
        render_pass: &mut wgpu::RenderPass<'_>,
    ) -> Result<(), WgpuViewportError> {
        self.control.with_renderer_mut(|renderer| {
            renderer
                .render_context(context, render_pass)
                .map_err(Into::into)
        })
    }

    pub(crate) fn render_with_fb_size(
        &self,
        frame: RenderedFrame<'_>,
        render_pass: &mut wgpu::RenderPass<'_>,
        width: u32,
        height: u32,
    ) -> Result<(), WgpuViewportError> {
        self.control.with_renderer_mut(|renderer| {
            renderer
                .render_with_fb_size(frame, render_pass, width, height)
                .map_err(Into::into)
        })
    }

    pub(crate) fn render_with_fb_size_reconciled<'frame>(
        &self,
        frame: RenderedFrame<'frame>,
        render_pass: &mut wgpu::RenderPass<'_>,
        width: u32,
        height: u32,
    ) -> Result<ReconciledFrame<'frame>, WgpuViewportError> {
        self.control.with_renderer_mut(|renderer| {
            renderer
                .render_with_fb_size_reconciled(frame, render_pass, width, height)
                .map_err(Into::into)
        })
    }

    pub(crate) fn render_context_with_fb_size(
        &self,
        context: &mut Context,
        render_pass: &mut wgpu::RenderPass<'_>,
        width: u32,
        height: u32,
    ) -> Result<(), WgpuViewportError> {
        self.control.with_renderer_mut(|renderer| {
            renderer
                .render_context_with_fb_size(context, render_pass, width, height)
                .map_err(Into::into)
        })
    }

    pub(crate) fn invalidate_device_objects(
        &self,
        context: &mut Context,
    ) -> Result<(), WgpuViewportError> {
        self.control.with_renderer_mut(|renderer| {
            renderer
                .invalidate_device_objects(context)
                .map_err(Into::into)
        })
    }

    pub(crate) fn with_renderer<R>(
        &self,
        callback: impl FnOnce(&WgpuRenderer) -> R,
    ) -> Result<R, WgpuViewportError> {
        self.control.with_renderer(callback)
    }

    pub(crate) fn set_gamma_mode(&self, mode: GammaMode) -> Result<(), WgpuViewportError> {
        self.control.with_renderer_mut(|renderer| {
            renderer.set_gamma_mode(mode);
            Ok(())
        })
    }

    pub(crate) fn set_viewport_clear_color(
        &self,
        color: wgpu::Color,
    ) -> Result<(), WgpuViewportError> {
        self.control.with_renderer_mut(|renderer| {
            renderer.set_viewport_clear_color(color);
            Ok(())
        })
    }

    pub(crate) fn register_external_texture(
        &self,
        view: &wgpu::TextureView,
    ) -> Result<ExternalTextureId, WgpuViewportError> {
        self.control.with_renderer_mut(|renderer| {
            renderer.register_external_texture(view).map_err(Into::into)
        })
    }

    pub(crate) fn update_external_texture(
        &self,
        texture: ExternalTextureId,
        view: &wgpu::TextureView,
    ) -> Result<(), WgpuViewportError> {
        self.control.with_renderer_mut(|renderer| {
            renderer
                .update_external_texture(texture, view)
                .map_err(Into::into)
        })
    }

    pub(crate) fn unregister_external_texture(
        &self,
        texture: ExternalTextureId,
    ) -> Result<(), WgpuViewportError> {
        self.control.with_renderer_mut(|renderer| {
            renderer
                .unregister_external_texture(texture)
                .map_err(Into::into)
        })
    }

    pub(crate) fn shutdown(&mut self, context: &mut Context) -> Result<(), WgpuViewportError> {
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
    pub(super) fn renderer_address_for_test(&self) -> *const WgpuRenderer {
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

    #[cfg(test)]
    pub(super) fn fail_next_viewport_cleanup_for_test(&self) {
        self.control.fail_next_viewport_cleanup_for_test();
    }
}

impl Drop for OwningViewportRuntime {
    fn drop(&mut self) {
        self.control.owner_dropped();
    }
}

fn first_error<const N: usize>(
    errors: [Option<WgpuViewportError>; N],
) -> Result<(), WgpuViewportError> {
    errors.into_iter().flatten().next().map_or(Ok(()), Err)
}

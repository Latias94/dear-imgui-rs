use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::fmt;
use std::rc::Rc;

use dear_imgui_rs::render::RenderedFrame;
use dear_imgui_rs::{
    Context, ContextAttachment, ContextAttachmentError, ContextAttachmentLease,
    ContextAttachmentRole, ContextBinding, ContextBindingError, ContextDestroyed, ContextId,
    ContextTeardown, TextureFormat, TextureId,
};
use thiserror::Error;

use super::callbacks::{
    claim_callbacks, detect_callback_drift, preflight_callbacks, release_callbacks,
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
    /// A known platform runtime cannot establish the required per-window GL context contract.
    #[error(
        "platform backend `{backend}` does not provide Glow-compatible per-window GL context switching"
    )]
    PlatformGlContextUnsupported { backend: String },
    /// A platform callback required to make and present GL viewport contexts is absent.
    #[error("required ImGuiPlatformIO callback `{callback}` is not installed")]
    PlatformCallbackUnavailable { callback: &'static str },
    /// PlatformIO itself is unavailable for the currently bound Context.
    #[error("the bound Dear ImGui Context has no PlatformIO")]
    PlatformIoUnavailable,
    /// Another renderer already owns part of the renderer callback table.
    #[error("ImGuiPlatformIO callback `{callback}` is already owned by another renderer")]
    RendererCallbackOccupied { callback: &'static str },
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

enum ShutdownAction<'a> {
    Quiesce,
    Explicit(&'a mut Context),
    BestEffort,
    ContextResources,
}

pub(super) struct RuntimeControl {
    context_raw: *mut dear_imgui_rs::sys::ImGuiContext,
    binding: ContextBinding,
    state: Cell<RuntimeState>,
    renderer: RefCell<Option<Box<GlowRenderer>>>,
    gl: RefCell<Option<Rc<glow::Context>>>,
    attachment: RefCell<Option<ContextAttachmentLease>>,
    callback_claimed: Cell<bool>,
    callback_released: Cell<bool>,
    prior_backend_flags: dear_imgui_rs::BackendFlags,
    faults: RefCell<VecDeque<GlowViewportError>>,
    #[cfg(test)]
    panic_next_callback: Cell<bool>,
    #[cfg(test)]
    transitions: RefCell<Vec<&'static str>>,
}

impl fmt::Debug for RuntimeControl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeControl")
            .field("context", &self.binding.id())
            .field("state", &self.state.get())
            .field("has_renderer", &self.renderer.borrow().is_some())
            .field("callback_claimed", &self.callback_claimed.get())
            .field("callback_released", &self.callback_released.get())
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
            callback_claimed: Cell::new(false),
            callback_released: Cell::new(false),
            prior_backend_flags: context.io().backend_flags(),
            faults: RefCell::new(VecDeque::new()),
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

    pub(super) fn prior_backend_flags(&self) -> dear_imgui_rs::BackendFlags {
        self.prior_backend_flags
    }

    pub(super) fn is_callback_accessible(&self) -> bool {
        self.state.get() == RuntimeState::Attached && !self.callback_released.get()
    }

    pub(super) fn should_detect_callback_drift(&self) -> bool {
        self.state.get() == RuntimeState::Attached
            && self.callback_claimed.get()
            && !self.callback_released.get()
    }

    pub(super) fn callback_released(&self) -> bool {
        self.callback_released.get()
    }

    pub(super) fn mark_callback_claimed(&self) {
        self.callback_claimed.set(true);
        self.callback_released.set(false);
    }

    pub(super) fn mark_callback_released(&self) {
        self.callback_released.set(true);
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
        if self.faults.borrow().is_empty() {
            self.faults.borrow_mut().push_back(fault);
        }
    }

    pub(super) fn record_callback_replaced(&self, callback: &'static str) {
        self.record_fault(GlowViewportError::RendererCallbackReplaced { callback });
        self.begin_shutdown();
    }

    fn detect_and_take_fault(&self) -> Option<GlowViewportError> {
        detect_callback_drift(self);
        self.faults.borrow_mut().pop_front()
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
        if let Err(error) = callback(renderer, &gl) {
            self.record_fault(GlowViewportError::Renderer(error));
        }
    }

    fn release_renderer_explicit(&self, context: &mut Context) -> Result<(), GlowViewportError> {
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
            .ok_or(GlowViewportError::RuntimeDetached)?;
        let mut renderer =
            self.renderer
                .try_borrow_mut()
                .map_err(|_| GlowViewportError::CallbackReentered {
                    callback: "GlowViewportRuntime::shutdown",
                })?;
        renderer
            .as_deref_mut()
            .ok_or(GlowViewportError::RuntimeDetached)?
            .destroy(&gl, context)?;
        let renderer = renderer.take();
        drop(renderer);
        self.gl.borrow_mut().take();
        self.set_state(RuntimeState::ResourceDropped);
        Ok(())
    }

    fn release_renderer_without_context_reset(&self) -> Result<(), GlowViewportError> {
        if self.renderer.borrow().is_none() {
            self.gl.borrow_mut().take();
            self.set_state(RuntimeState::ResourceDropped);
            return Ok(());
        }
        self.mark_detached();
        let gl = self
            .gl
            .borrow()
            .as_ref()
            .cloned()
            .ok_or(GlowViewportError::RuntimeDetached)?;
        let mut renderer =
            self.renderer
                .try_borrow_mut()
                .map_err(|_| GlowViewportError::CallbackReentered {
                    callback: "Context renderer-resource teardown",
                })?;
        if let Some(renderer) = renderer.as_deref_mut() {
            renderer.destroy_gpu_resources_only(&gl);
        }
        let renderer = renderer.take();
        drop(renderer);
        self.gl.borrow_mut().take();
        self.set_state(RuntimeState::ResourceDropped);
        Ok(())
    }

    fn shutdown_once(&self, action: ShutdownAction<'_>) -> Result<(), GlowViewportError> {
        if self.state.get() == RuntimeState::ResourceDropped {
            if !matches!(action, ShutdownAction::ContextResources) {
                self.detach_attachment();
            }
            return Ok(());
        }
        self.begin_shutdown();
        let callback_result = release_callbacks(self);
        match action {
            ShutdownAction::Quiesce => callback_result,
            ShutdownAction::Explicit(context) => {
                self.mark_detached();
                let renderer_result = self.release_renderer_explicit(context);
                if self.state.get() == RuntimeState::ResourceDropped {
                    self.detach_attachment();
                }
                first_error([callback_result.err(), renderer_result.err()])
            }
            ShutdownAction::BestEffort => {
                self.mark_detached();
                let renderer_result = self.release_renderer_without_context_reset();
                if self.state.get() == RuntimeState::ResourceDropped {
                    self.detach_attachment();
                }
                first_error([callback_result.err(), renderer_result.err()])
            }
            ShutdownAction::ContextResources => {
                self.mark_detached();
                let renderer_result = self.release_renderer_without_context_reset();
                first_error([callback_result.err(), renderer_result.err()])
            }
        }
    }

    fn owner_dropped(&self) {
        if self.state.get() == RuntimeState::ResourceDropped {
            return;
        }
        let _ = self.binding.try_with_bound_context(|| {
            if let Err(error) = self.shutdown_once(ShutdownAction::BestEffort) {
                self.record_fault(error);
            }
        });
    }

    fn store_attachment(&self, attachment: ContextAttachmentLease) {
        self.attachment.borrow_mut().replace(attachment);
    }

    fn detach_attachment(&self) {
        if let Some(mut attachment) = self.attachment.borrow_mut().take() {
            attachment.detach();
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
        if self.renderer.borrow().is_some() {
            let _ = self.release_renderer_without_context_reset();
        }
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
    fn quiesce(&self, context: &ContextTeardown<'_>) {
        context.with_bound_context(|| {
            if let Err(error) = self.shutdown_once(ShutdownAction::Quiesce) {
                self.record_fault(error);
            }
        });
    }

    fn release_renderer_resources(&self, context: &ContextTeardown<'_>) {
        context.with_bound_context(|| {
            if let Err(error) = self.shutdown_once(ShutdownAction::ContextResources) {
                self.record_fault(error);
            }
        });
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
    /// Transactionally attaches a renderer that already owns a live GL capability.
    ///
    /// Renderers created by [`GlowRenderer::with_external_context`] are intentionally rejected:
    /// the runtime cannot prove that an independently supplied capability created their objects.
    /// Use [`GlowRenderer::with_shared_context`] when the Glow function table is shared.
    ///
    /// # Platform GL contract
    ///
    /// The registered platform runtime must create every viewport GL context in the renderer's
    /// share group and make that context current from `Platform_RenderWindow`. Callback-table
    /// preflight proves ownership structure only; it cannot prove an OS-level GL share group. The
    /// window-only Winit runtime is rejected explicitly. A compatible share-group context must
    /// also be current during explicit runtime shutdown and Context teardown so renderer resources
    /// can be deleted in the ordered renderer-resource phase.
    pub fn attach(
        context: &mut Context,
        renderer: GlowRenderer,
    ) -> Result<Self, GlowViewportAttachError> {
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

    /// Prepares renderer device objects for a new frame.
    pub fn new_frame(&self) -> Result<(), GlowViewportError> {
        self.control
            .with_renderer_mut(|renderer| renderer.new_frame().map_err(Into::into))
    }

    /// Consumes and renders one Context-owned frame.
    pub fn render(&self, frame: RenderedFrame<'_>) -> Result<(), GlowViewportError> {
        self.control
            .with_renderer_mut(|renderer| renderer.render(frame).map_err(Into::into))
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
        self.control.with_renderer_mut(|renderer| {
            renderer.set_framebuffer_srgb_enabled(enabled);
            Ok(())
        })
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

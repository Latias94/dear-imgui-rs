use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::fmt;
use std::rc::Rc;

use dear_imgui_rs::render::RenderedFrame;
use dear_imgui_rs::{
    Context, ContextAttachment, ContextAttachmentError, ContextAttachmentLease,
    ContextAttachmentRole, ContextBinding, ContextBindingError, ContextDestroyed, ContextId,
    ContextTeardown, TextureId,
};
use thiserror::Error;

use super::callbacks::{
    claim_callbacks, destroy_renderer_viewport_resources, detect_callback_drift,
    preflight_callbacks, release_callbacks,
};
use super::registry::{
    GlobalHandles, preflight_runtime, register_runtime, renderer_globals, take_viewport_data,
    unregister_runtime,
};
use crate::{GammaMode, RendererError, WgpuRenderer};

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
    /// A required platform callback is absent.
    #[error("required ImGuiPlatformIO callback `{callback}` is not installed")]
    PlatformCallbackUnavailable { callback: &'static str },
    /// The platform runtime has not published the main native window handle.
    #[error("the attached platform runtime has no main viewport window handle")]
    MainViewportHandleUnavailable,
    /// Another renderer already owns one renderer callback slot.
    #[error("ImGuiPlatformIO callback `{callback}` is already owned by another renderer")]
    RendererCallbackOccupied { callback: &'static str },
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
    /// Surface acquisition returned a terminal result.
    #[error("WGPU viewport surface acquisition was rejected: {event}")]
    SurfaceRejected { event: &'static str },
    /// Native multi-viewport surfaces are unavailable on this target.
    #[error("WGPU native multi-viewport rendering is unavailable on this target")]
    UnsupportedTarget,
}

/// Transactional attachment failure that returns the renderer unchanged.
pub struct WgpuViewportAttachError {
    error: WgpuViewportError,
    renderer: Box<WgpuRenderer>,
}

impl WgpuViewportAttachError {
    fn new(error: WgpuViewportError, renderer: WgpuRenderer) -> Self {
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
    renderer: RefCell<Option<Box<WgpuRenderer>>>,
    globals: RefCell<Option<GlobalHandles>>,
    attachment: RefCell<Option<ContextAttachmentLease>>,
    callback_claimed: Cell<bool>,
    callback_released: Cell<bool>,
    prior_backend_flags: dear_imgui_rs::BackendFlags,
    renderer_flags_added: dear_imgui_rs::BackendFlags,
    faults: RefCell<VecDeque<WgpuViewportError>>,
    #[cfg(test)]
    panic_next_callback: Cell<bool>,
    #[cfg(test)]
    fail_next_viewport_cleanup: Cell<bool>,
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
    fn new(
        context: &Context,
        renderer: WgpuRenderer,
        globals: Option<GlobalHandles>,
        renderer_flags_added: dear_imgui_rs::BackendFlags,
    ) -> Self {
        Self {
            context_raw: context.as_raw(),
            binding: context.binding(),
            state: Cell::new(RuntimeState::Constructing),
            renderer: RefCell::new(Some(Box::new(renderer))),
            globals: RefCell::new(globals),
            attachment: RefCell::new(None),
            callback_claimed: Cell::new(false),
            callback_released: Cell::new(false),
            prior_backend_flags: context.io().backend_flags(),
            renderer_flags_added,
            faults: RefCell::new(VecDeque::new()),
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

    pub(super) fn prior_backend_flags(&self) -> dear_imgui_rs::BackendFlags {
        self.prior_backend_flags
    }

    pub(super) fn globals(&self) -> Option<GlobalHandles> {
        self.globals.borrow().clone()
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
        if self.faults.borrow().is_empty() {
            self.faults.borrow_mut().push_back(fault);
        }
    }

    pub(super) fn record_callback_replaced(&self, callback: &'static str) {
        self.record_fault(WgpuViewportError::RendererCallbackReplaced { callback });
        self.begin_shutdown();
    }

    fn detect_and_take_fault(&self) -> Option<WgpuViewportError> {
        detect_callback_drift(self);
        self.faults.borrow_mut().pop_front()
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
        }?;
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
            self.record_fault(error);
        }
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

    fn release_renderer_without_context_reset(&self) -> Result<(), WgpuViewportError> {
        if self.renderer.borrow().is_none() {
            self.globals.borrow_mut().take();
            self.set_state(RuntimeState::ResourceDropped);
            return Ok(());
        }
        let mut renderer =
            self.renderer
                .try_borrow_mut()
                .map_err(|_| WgpuViewportError::CallbackReentered {
                    callback: "Context renderer-resource teardown",
                })?;
        if let Some(renderer) = renderer.as_deref_mut() {
            renderer.shutdown_without_context_reset();
        }
        let renderer = renderer.take();
        drop(renderer);
        self.globals.borrow_mut().take();
        self.set_state(RuntimeState::ResourceDropped);
        Ok(())
    }

    fn shutdown_once(&self, action: ShutdownAction<'_>) -> Result<(), WgpuViewportError> {
        if self.state.get() == RuntimeState::ResourceDropped {
            if !matches!(action, ShutdownAction::ContextResources) {
                self.detach_attachment();
            }
            return Ok(());
        }

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
            ShutdownAction::BestEffort => {
                self.mark_detached();
                let renderer_result = self.release_renderer_without_context_reset();
                if self.state.get() == RuntimeState::ResourceDropped {
                    self.clear_bound_renderer_configuration();
                    self.detach_attachment();
                }
                first_error([viewport_error, callback_result.err(), renderer_result.err()])
            }
            ShutdownAction::ContextResources => {
                self.mark_detached();
                let renderer_result = self.release_renderer_without_context_reset();
                first_error([viewport_error, callback_result.err(), renderer_result.err()])
            }
        }
    }

    fn clear_bound_renderer_configuration(&self) {
        let io = unsafe { dear_imgui_rs::sys::igGetIO_Nil() };
        let platform_io = unsafe { dear_imgui_rs::sys::igGetPlatformIO_Nil() };
        if io.is_null() || platform_io.is_null() {
            return;
        }
        let platform_io =
            unsafe { dear_imgui_rs::platform_io::PlatformIo::from_raw_mut(platform_io) };
        let renderer_name = unsafe {
            (!(*io).BackendRendererName.is_null())
                .then(|| std::ffi::CStr::from_ptr((*io).BackendRendererName))
        };
        let renderer_name_is_ours = WgpuRenderer::renderer_name_is_ours(renderer_name);
        let draw_callbacks_are_ours = WgpuRenderer::owned_draw_callbacks_match(platform_io);
        unsafe {
            if renderer_name_is_ours {
                (*io).BackendRendererName = std::ptr::null();
            }
            if renderer_name_is_ours && draw_callbacks_are_ours {
                (*io).BackendFlags &= !self.renderer_flags_added.bits();
            }
        }
        WgpuRenderer::clear_owned_draw_callbacks(platform_io);
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
        for pointer in take_viewport_data(self.binding.id()) {
            // The registry is the allocation ownership sidecar; native viewport pointers are no
            // longer touched after Context destruction.
            drop(unsafe { Box::from_raw(pointer) });
        }
        if self.renderer.borrow().is_some() {
            let _ = self.release_renderer_without_context_reset();
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

/// Backend-local owning runtime shared by the Winit and SDL3 typed wrappers.
pub(crate) struct OwningViewportRuntime {
    control: Rc<RuntimeControl>,
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
    pub(crate) fn attach(
        context: &mut Context,
        renderer: WgpuRenderer,
    ) -> Result<Self, WgpuViewportAttachError> {
        if let Err(error) = renderer.ensure_context_matches(context) {
            let error = match error {
                RendererError::ContextDropped => WgpuViewportError::RendererContextDropped,
                RendererError::ContextMismatch => WgpuViewportError::RendererContextMismatch,
                other => WgpuViewportError::Renderer(other),
            };
            return Err(WgpuViewportAttachError::new(error, renderer));
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

        let renderer_flags_added = match renderer.renderer_flags_added() {
            Ok(flags) => flags,
            Err(error) => {
                return Err(WgpuViewportAttachError::new(error.into(), renderer));
            }
        };
        let control = Rc::new(RuntimeControl::new(
            context,
            renderer,
            globals,
            renderer_flags_added,
        ));
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
        if renderer.context_binding.is_none() {
            if let Err(error) = renderer.bind_context(context, dear_imgui_rs::BackendFlags::empty())
            {
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

    pub(crate) fn render(
        &self,
        frame: RenderedFrame<'_>,
        render_pass: &mut wgpu::RenderPass<'_>,
    ) -> Result<(), WgpuViewportError> {
        self.control
            .with_renderer_mut(|renderer| renderer.render(frame, render_pass).map_err(Into::into))
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
        texture: &wgpu::Texture,
        view: &wgpu::TextureView,
    ) -> Result<TextureId, WgpuViewportError> {
        self.control
            .with_renderer_mut(|renderer| Ok(renderer.register_external_texture(texture, view)))
    }

    pub(crate) fn unregister_texture(&self, texture: TextureId) -> Result<(), WgpuViewportError> {
        self.control.with_renderer_mut(|renderer| {
            renderer.unregister_texture(texture);
            Ok(())
        })
    }

    pub(crate) fn register_external_texture_with_sampler(
        &self,
        texture: &wgpu::Texture,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> Result<TextureId, WgpuViewportError> {
        self.control.with_renderer_mut(|renderer| {
            Ok(renderer.register_external_texture_with_sampler(texture, view, sampler))
        })
    }

    pub(crate) fn update_external_texture_view(
        &self,
        texture: TextureId,
        view: &wgpu::TextureView,
    ) -> Result<bool, WgpuViewportError> {
        self.control
            .with_renderer_mut(|renderer| Ok(renderer.update_external_texture_view(texture, view)))
    }

    pub(crate) fn update_external_texture_sampler(
        &self,
        texture: TextureId,
        sampler: &wgpu::Sampler,
    ) -> Result<bool, WgpuViewportError> {
        self.control.with_renderer_mut(|renderer| {
            Ok(renderer.update_external_texture_sampler(texture, sampler))
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

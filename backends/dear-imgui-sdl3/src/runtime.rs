use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::rc::{Rc, Weak};

use dear_imgui_rs::RendererConsumer;
#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
use dear_imgui_rs::render::{TextureFeedback, TextureRequest};
use dear_imgui_rs::{
    Context, ContextAttachment, ContextAttachmentLease, ContextAttachmentRole,
    ContextAttachmentTeardownError, ContextBinding, ContextDestroyed, ContextLifecycle,
    ContextTeardown, TextureData, sys,
};

use crate::callback_ownership::{
    PlatformCallbackOwnership, PlatformClaimBaseline, RendererCallbackOwnership,
    RendererShutdownRestore, SDL_PLATFORM_RESERVED_FLAGS, SDL_RENDERER_RESERVED_FLAGS,
    ViewportPlatformState, preflight_platform_claim, restore_baseline_after_failed_initialization,
};
use crate::core::{Sdl3BackendError, Sdl3OpenGlViewportSwapInterval, shutdown_platform_impl};
#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
use crate::renderer_textures::RendererTextureStore;

struct Sdl3PlatformAttachmentMarker;
struct Sdl3RendererAttachmentMarker;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RuntimeState {
    Attached,
    ShuttingDown,
    Detached,
    ResourceDropped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PlatformGraphicsKind {
    Other,
    OpenGl,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeRendererKind {
    None,
    #[cfg(feature = "opengl3-renderer")]
    OpenGl3,
    #[cfg(feature = "sdlrenderer3-renderer")]
    SdlRenderer3,
    #[cfg(feature = "sdlgpu3-renderer")]
    SdlGpu3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeFault {
    CallbackReplaced(&'static str),
    PlatformStateReplaced(&'static str),
    RendererCallbackReplaced(&'static str),
    RendererStateReplaced(&'static str),
    CallbackPanicked(&'static str),
    ForeignPlatformUserData,
    ViewportCreationFailed,
    ViewportOpenGlStateCaptureFailed,
    ViewportOpenGlContextFailed,
    ViewportOpenGlSwapIntervalFailed,
    ViewportOpenGlStateRestoreFailed,
    ViewportOpenGlRenderContextFailed,
    ViewportOpenGlSwapFailed,
    ViewportSdlGpuClaimFailed,
    ViewportSdlGpuConfigureFailed,
    ViewportSdlGpuCommandBufferFailed,
    ViewportSdlGpuSwapchainFailed,
    ViewportSdlGpuRenderPassFailed,
    ViewportSdlGpuSubmitFailed,
    NativeBridgeProtocolFailed,
}

impl RuntimeFault {
    fn into_error(self) -> Sdl3BackendError {
        match self {
            Self::CallbackReplaced(callback) => {
                Sdl3BackendError::PlatformCallbackReplaced { callback }
            }
            Self::PlatformStateReplaced(field) => Sdl3BackendError::PlatformStateReplaced { field },
            Self::RendererCallbackReplaced(callback) => {
                Sdl3BackendError::RendererCallbackReplaced { callback }
            }
            Self::RendererStateReplaced(field) => Sdl3BackendError::RendererStateReplaced { field },
            Self::CallbackPanicked(callback) => {
                Sdl3BackendError::PlatformCallbackPanicked { callback }
            }
            Self::ForeignPlatformUserData => Sdl3BackendError::ForeignPlatformUserData,
            Self::ViewportCreationFailed => Sdl3BackendError::ViewportCreationFailed,
            Self::ViewportOpenGlStateCaptureFailed => {
                Sdl3BackendError::ViewportOpenGlStateCaptureFailed
            }
            Self::ViewportOpenGlContextFailed => Sdl3BackendError::ViewportOpenGlContextFailed,
            Self::ViewportOpenGlSwapIntervalFailed => {
                Sdl3BackendError::ViewportOpenGlSwapIntervalFailed
            }
            Self::ViewportOpenGlStateRestoreFailed => {
                Sdl3BackendError::ViewportOpenGlStateRestoreFailed
            }
            Self::ViewportOpenGlRenderContextFailed => {
                Sdl3BackendError::ViewportOpenGlRenderContextFailed
            }
            Self::ViewportOpenGlSwapFailed => Sdl3BackendError::ViewportOpenGlSwapFailed,
            Self::ViewportSdlGpuClaimFailed => Sdl3BackendError::ViewportSdlGpuClaimFailed,
            Self::ViewportSdlGpuConfigureFailed => Sdl3BackendError::ViewportSdlGpuConfigureFailed,
            Self::ViewportSdlGpuCommandBufferFailed => {
                Sdl3BackendError::ViewportSdlGpuCommandBufferFailed
            }
            Self::ViewportSdlGpuSwapchainFailed => Sdl3BackendError::ViewportSdlGpuSwapchainFailed,
            Self::ViewportSdlGpuRenderPassFailed => {
                Sdl3BackendError::ViewportSdlGpuRenderPassFailed
            }
            Self::ViewportSdlGpuSubmitFailed => Sdl3BackendError::ViewportSdlGpuSubmitFailed,
            Self::NativeBridgeProtocolFailed => Sdl3BackendError::NativeBridgeProtocolFailed,
        }
    }
}

struct NativeLifecycle {
    renderer_shutdown: Option<Rc<dyn Fn()>>,
    renderer_device_objects_destroy: Option<Rc<dyn Fn()>>,
    renderer_texture_update: Option<Rc<dyn Fn(&mut TextureData)>>,
    platform_shutdown: Rc<dyn Fn()>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReleaseState {
    Pending,
    InProgress,
    Released,
}

impl ReleaseState {
    fn is_released(self) -> bool {
        self == Self::Released
    }
}

struct ReleaseGuard<'a> {
    state: &'a Cell<ReleaseState>,
}

impl<'a> ReleaseGuard<'a> {
    fn begin(state: &'a Cell<ReleaseState>) -> Option<Self> {
        match state.get() {
            ReleaseState::Pending => {
                state.set(ReleaseState::InProgress);
                Some(Self { state })
            }
            ReleaseState::InProgress | ReleaseState::Released => None,
        }
    }

    fn commit(self) {
        self.state.set(ReleaseState::Released);
    }
}

impl Drop for ReleaseGuard<'_> {
    fn drop(&mut self) {
        if self.state.get() == ReleaseState::InProgress {
            self.state.set(ReleaseState::Pending);
        }
    }
}

impl fmt::Debug for NativeLifecycle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeLifecycle")
            .field("has_renderer", &self.renderer_shutdown.is_some())
            .field(
                "has_renderer_device_objects_destroy",
                &self.renderer_device_objects_destroy.is_some(),
            )
            .field(
                "has_renderer_texture_update",
                &self.renderer_texture_update.is_some(),
            )
            .finish_non_exhaustive()
    }
}

pub(super) struct RuntimeControl {
    binding: ContextBinding,
    state: Cell<RuntimeState>,
    platform_initialized: Cell<bool>,
    renderer_initialized: Cell<bool>,
    renderer_release: Cell<ReleaseState>,
    platform_release: Cell<ReleaseState>,
    callback_teardown_active: Cell<bool>,
    platform_io_key: Cell<usize>,
    platform_graphics: PlatformGraphicsKind,
    gl_viewport_swap_interval: Cell<Sdl3OpenGlViewportSwapInterval>,
    native_renderer: NativeRendererKind,
    lifecycle: NativeLifecycle,
    callbacks: RefCell<Option<PlatformCallbackOwnership>>,
    renderer_callbacks: RefCell<Option<RendererCallbackOwnership>>,
    renderer_shutdown_restore: RefCell<Option<RendererShutdownRestore>>,
    renderer_consumer: RefCell<Option<RendererConsumer>>,
    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    renderer_textures: RefCell<RendererTextureStore>,
    owned_viewports: RefCell<HashMap<usize, ViewportPlatformState>>,
    owned_renderer_viewports: RefCell<HashMap<usize, *mut std::ffi::c_void>>,
    deferred_platform_viewport_restores: RefCell<HashMap<usize, ViewportPlatformState>>,
    deferred_renderer_viewport_restores: RefCell<HashMap<usize, *mut std::ffi::c_void>>,
    failed_viewports: RefCell<HashSet<usize>>,
    faults: RefCell<VecDeque<RuntimeFault>>,
    reported_replacements: RefCell<HashSet<&'static str>>,
    revoked_capabilities: Cell<i32>,
    foreign_capabilities: Cell<i32>,
    #[cfg(test)]
    phase_log: RefCell<Vec<&'static str>>,
}

impl fmt::Debug for RuntimeControl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeControl")
            .field("context", &self.binding.id())
            .field("state", &self.state.get())
            .field("platform_initialized", &self.platform_initialized.get())
            .field("renderer_initialized", &self.renderer_initialized.get())
            .field("renderer_released", &self.renderer_released())
            .field("platform_released", &self.platform_released())
            .finish_non_exhaustive()
    }
}

impl RuntimeControl {
    fn new_with_backend(
        context: &Context,
        renderer_shutdown: Option<Rc<dyn Fn()>>,
        renderer_device_objects_destroy: Option<Rc<dyn Fn()>>,
        renderer_texture_update: Option<Rc<dyn Fn(&mut TextureData)>>,
        platform_shutdown: Rc<dyn Fn()>,
        platform_graphics: PlatformGraphicsKind,
        native_renderer: NativeRendererKind,
    ) -> Self {
        Self {
            binding: context.binding(),
            state: Cell::new(RuntimeState::Attached),
            platform_initialized: Cell::new(false),
            renderer_initialized: Cell::new(false),
            renderer_release: Cell::new(if renderer_shutdown.is_none() {
                ReleaseState::Released
            } else {
                ReleaseState::Pending
            }),
            platform_release: Cell::new(ReleaseState::Pending),
            callback_teardown_active: Cell::new(false),
            platform_io_key: Cell::new(0),
            platform_graphics,
            gl_viewport_swap_interval: Cell::new(Sdl3OpenGlViewportSwapInterval::Immediate),
            native_renderer,
            lifecycle: NativeLifecycle {
                renderer_shutdown,
                renderer_device_objects_destroy,
                renderer_texture_update,
                platform_shutdown,
            },
            callbacks: RefCell::new(None),
            renderer_callbacks: RefCell::new(None),
            renderer_shutdown_restore: RefCell::new(None),
            renderer_consumer: RefCell::new(None),
            #[cfg(any(
                feature = "opengl3-renderer",
                feature = "sdlrenderer3-renderer",
                feature = "sdlgpu3-renderer"
            ))]
            renderer_textures: RefCell::new(RendererTextureStore::default()),
            owned_viewports: RefCell::new(HashMap::new()),
            owned_renderer_viewports: RefCell::new(HashMap::new()),
            deferred_platform_viewport_restores: RefCell::new(HashMap::new()),
            deferred_renderer_viewport_restores: RefCell::new(HashMap::new()),
            failed_viewports: RefCell::new(HashSet::new()),
            faults: RefCell::new(VecDeque::new()),
            reported_replacements: RefCell::new(HashSet::new()),
            revoked_capabilities: Cell::new(0),
            foreign_capabilities: Cell::new(0),
            #[cfg(test)]
            phase_log: RefCell::new(Vec::new()),
        }
    }

    pub(super) fn binding(&self) -> &ContextBinding {
        &self.binding
    }

    pub(super) fn install_renderer_consumer(&self, consumer: RendererConsumer) {
        let previous = self.renderer_consumer.borrow_mut().replace(consumer);
        assert!(
            previous.is_none(),
            "SDL3 runtime already owns a renderer consumer"
        );
    }

    fn take_renderer_consumer(&self) -> Option<RendererConsumer> {
        self.renderer_consumer.borrow_mut().take()
    }

    pub(super) fn state(&self) -> RuntimeState {
        self.state.get()
    }

    pub(super) fn expects_opengl(&self) -> bool {
        self.platform_graphics == PlatformGraphicsKind::OpenGl
    }

    pub(super) fn native_renderer(&self) -> NativeRendererKind {
        self.native_renderer
    }

    pub(super) fn native_gl_swap_interval(&self) -> (u32, i32) {
        self.gl_viewport_swap_interval.get().native_policy()
    }

    pub(super) fn set_gl_viewport_swap_interval(&self, policy: Sdl3OpenGlViewportSwapInterval) {
        debug_assert!(self.expects_opengl());
        self.gl_viewport_swap_interval.set(policy);
    }

    fn begin_shutdown(&self) {
        if self.state.get() == RuntimeState::Attached {
            self.state.set(RuntimeState::ShuttingDown);
        }
    }

    fn finish_shutdown(&self) {
        if self.renderer_released()
            && self.platform_released()
            && self.state.get() != RuntimeState::ResourceDropped
        {
            self.state.set(RuntimeState::Detached);
        }
    }

    fn renderer_released(&self) -> bool {
        self.renderer_release.get().is_released()
    }

    fn platform_released(&self) -> bool {
        self.platform_release.get().is_released()
    }

    fn release_renderer_device_objects_bound(&self) -> Result<(), Sdl3BackendError> {
        if !self.renderer_initialized.get() {
            return Ok(());
        }
        #[cfg(any(
            feature = "opengl3-renderer",
            feature = "sdlrenderer3-renderer",
            feature = "sdlgpu3-renderer"
        ))]
        self.destroy_uninstalled_renderer_textures_bound()?;
        if let Some(destroy) = &self.lifecycle.renderer_device_objects_destroy {
            destroy();
        }
        #[cfg(any(
            feature = "opengl3-renderer",
            feature = "sdlrenderer3-renderer",
            feature = "sdlgpu3-renderer"
        ))]
        self.forget_textures_destroyed_by_upstream();
        Ok(())
    }

    fn release_renderer_bound(&self) -> Result<bool, Sdl3BackendError> {
        if self.renderer_released() {
            return Ok(true);
        }
        let Some(release) = ReleaseGuard::begin(&self.renderer_release) else {
            return Ok(false);
        };
        #[cfg(test)]
        self.phase_log.borrow_mut().push("renderer");
        #[cfg(any(
            feature = "opengl3-renderer",
            feature = "sdlrenderer3-renderer",
            feature = "sdlgpu3-renderer"
        ))]
        if self.renderer_initialized.get() {
            self.destroy_uninstalled_renderer_textures_bound()?;
        }
        let restore = if let Some(restore) = self.renderer_shutdown_restore.borrow_mut().take() {
            let prepare_result = self
                .renderer_callbacks
                .borrow()
                .as_ref()
                .map(|callbacks| unsafe { callbacks.switch_from_platform_to_native_shutdown() })
                .transpose();
            if let Err(error) = prepare_result {
                self.renderer_shutdown_restore.borrow_mut().replace(restore);
                return Err(error);
            }
            Some(restore)
        } else {
            self.renderer_callbacks
                .borrow()
                .as_ref()
                .map(|callbacks| unsafe { callbacks.prepare_native_shutdown(self) })
                .transpose()?
        };
        let shutdown_result = if self.renderer_initialized.get()
            && let Some(shutdown) = &self.lifecycle.renderer_shutdown
        {
            catch_unwind(AssertUnwindSafe(|| shutdown()))
        } else {
            Ok(())
        };
        if let Some(restore) = restore {
            let restore_result = unsafe {
                self.renderer_callbacks
                    .borrow()
                    .as_ref()
                    .expect("initialized SDL3 renderer lost its callback claim")
                    .restore_after_shutdown(restore)
            };
            if restore_result.is_err() {
                self.record_platform_state_replaced("renderer callback shutdown state");
            }
        }
        if let Err(payload) = shutdown_result {
            resume_unwind(payload);
        }
        #[cfg(any(
            feature = "opengl3-renderer",
            feature = "sdlrenderer3-renderer",
            feature = "sdlgpu3-renderer"
        ))]
        self.renderer_textures
            .borrow_mut()
            .forget_destroyed_by_upstream();
        release.commit();
        self.renderer_initialized.set(false);
        self.renderer_callbacks.borrow_mut().take();
        self.owned_renderer_viewports.borrow_mut().clear();
        self.finish_shutdown();
        Ok(true)
    }

    fn release_platform_bound(&self) -> Result<(), Sdl3BackendError> {
        if self.platform_released() {
            self.finish_shutdown();
            return Ok(());
        }
        let Some(release) = ReleaseGuard::begin(&self.platform_release) else {
            return Ok(());
        };
        #[cfg(test)]
        self.phase_log.borrow_mut().push("platform");

        if !self.platform_initialized.get() {
            release.commit();
            self.finish_shutdown();
            return Ok(());
        }

        let renderer_restore = self
            .renderer_callbacks
            .borrow()
            .as_ref()
            .map(|callbacks| unsafe { callbacks.prepare_platform_shutdown(self) })
            .transpose()?;
        let restore = {
            let callbacks = self.callbacks.borrow();
            match callbacks
                .as_ref()
                .map(|callbacks| unsafe { callbacks.prepare_shutdown(self) })
                .transpose()
            {
                Ok(restore) => restore,
                Err(error) => {
                    if let Some(renderer_restore) = renderer_restore {
                        let _ = self
                            .renderer_callbacks
                            .borrow()
                            .as_ref()
                            .map(|callbacks| unsafe {
                                callbacks.restore_after_shutdown(renderer_restore)
                            });
                    }
                    return Err(error);
                }
            }
        };
        let main_viewport = unsafe { sys::igGetMainViewport() };
        if !main_viewport.is_null() {
            if let Some(restore) = restore.as_ref() {
                self.defer_platform_viewport_restore(main_viewport, restore.main_viewport());
            }
        }
        self.callback_teardown_active.set(true);
        struct CallbackTeardownGuard<'a>(&'a Cell<bool>);
        impl Drop for CallbackTeardownGuard<'_> {
            fn drop(&mut self) {
                self.0.set(false);
            }
        }
        let callback_guard = CallbackTeardownGuard(&self.callback_teardown_active);
        let shutdown_result = catch_unwind(AssertUnwindSafe(|| {
            (self.lifecycle.platform_shutdown)();
        }));
        drop(callback_guard);
        self.restore_deferred_viewport_state();

        if let Err(payload) = shutdown_result {
            if let Some(restore) = restore {
                let callbacks = self.callbacks.borrow();
                let _ = unsafe {
                    callbacks
                        .as_ref()
                        .expect("initialized SDL3 runtime lost its callback claim")
                        .restore_after_shutdown(restore)
                };
            }
            if let Some(renderer_restore) = renderer_restore {
                let _ = self
                    .renderer_callbacks
                    .borrow()
                    .as_ref()
                    .map(|callbacks| unsafe { callbacks.restore_after_shutdown(renderer_restore) });
            }
            resume_unwind(payload);
        }

        // Native shutdown is the irreversible boundary: never call it twice,
        // even if restoring foreign callback state reports an error.
        release.commit();
        self.platform_initialized.set(false);
        let restore_result = if let Some(restore) = restore {
            let callbacks = self.callbacks.borrow();
            unsafe {
                callbacks
                    .as_ref()
                    .expect("initialized SDL3 runtime lost its callback claim")
                    .restore_after_shutdown(restore)
            }
        } else {
            Ok(())
        };
        unregister_runtime(self.platform_io_key.replace(0));
        self.callbacks.borrow_mut().take();
        self.owned_viewports.borrow_mut().clear();
        self.failed_viewports.borrow_mut().clear();
        if let Some(renderer_restore) = renderer_restore {
            self.renderer_shutdown_restore
                .borrow_mut()
                .replace(renderer_restore);
        }
        self.finish_shutdown();
        restore_result
    }

    fn release_renderer_explicit(&self) -> Result<(), Sdl3BackendError> {
        if self.renderer_released() {
            return Ok(());
        }
        let result = self.binding.try_with_bound_context(|| {
            catch_unwind(AssertUnwindSafe(|| self.release_renderer_bound()))
        })?;
        match result {
            Ok(Ok(true)) => Ok(()),
            Ok(Ok(false)) => Err(Sdl3BackendError::ShutdownInProgress {
                phase: "renderer resources",
            }),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(Sdl3BackendError::ShutdownPanicked {
                phase: "renderer resources",
            }),
        }
    }

    fn release_platform_explicit(&self) -> Result<(), Sdl3BackendError> {
        if self.platform_released() {
            self.finish_shutdown();
            return Ok(());
        }
        let result = self.binding.try_with_bound_context(|| {
            catch_unwind(AssertUnwindSafe(|| self.release_platform_bound()))
        })?;
        result.unwrap_or(Err(Sdl3BackendError::ShutdownPanicked {
            phase: "platform windows",
        }))
    }

    fn shutdown_native_explicit(&self) -> Result<(), Sdl3BackendError> {
        self.begin_shutdown();
        let platform_result = self.release_platform_explicit();
        let platform_retry_result = if self.platform_released() {
            Ok(())
        } else {
            self.release_platform_explicit()
        };
        let renderer_result = if self.platform_released() {
            self.release_renderer_explicit()
        } else {
            Ok(())
        };
        let renderer_retry_result = if self.platform_released() && !self.renderer_released() {
            self.release_renderer_explicit()
        } else {
            Ok(())
        };
        first_error([
            platform_result.err(),
            platform_retry_result.err(),
            renderer_result.err(),
            renderer_retry_result.err(),
        ])
    }

    fn shutdown_bound_for_attachment(&self) -> Result<(), Sdl3BackendError> {
        let platform_result = match catch_unwind(AssertUnwindSafe(|| self.release_platform_bound()))
        {
            Ok(result) => result,
            Err(_) => Err(Sdl3BackendError::ShutdownPanicked {
                phase: "platform windows",
            }),
        };
        let renderer_result = if self.platform_released() {
            match catch_unwind(AssertUnwindSafe(|| self.release_renderer_bound())) {
                Ok(Ok(true)) => Ok(()),
                Ok(Ok(false)) => Err(Sdl3BackendError::ShutdownInProgress {
                    phase: "renderer resources",
                }),
                Ok(Err(error)) => Err(error),
                Err(_) => Err(Sdl3BackendError::ShutdownPanicked {
                    phase: "renderer resources",
                }),
            }
        } else {
            Ok(())
        };
        first_error([platform_result.err(), renderer_result.err()])
    }

    fn detect_callback_replacements(&self) {
        if self.state.get() != RuntimeState::Attached || !self.platform_initialized.get() {
            return;
        }
        let _ = self.binding.try_with_bound_context(|| {
            if let Some(callbacks) = self.callbacks.borrow().as_ref() {
                unsafe { callbacks.detect_replacements(self) };
            }
            if let Some(callbacks) = self.renderer_callbacks.borrow().as_ref() {
                unsafe { callbacks.detect_replacements(self) };
            }
        });
    }

    pub(super) fn poll_fault(&self) -> Result<(), Sdl3BackendError> {
        self.detect_callback_replacements();
        match self.faults.borrow_mut().pop_front() {
            Some(fault) => Err(fault.into_error()),
            None => Ok(()),
        }
    }

    fn take_pending_fault(&self) -> Option<Sdl3BackendError> {
        self.detect_callback_replacements();
        self.faults
            .borrow_mut()
            .pop_front()
            .map(RuntimeFault::into_error)
    }

    pub(super) fn ensure_entry(&self, context: &Context) -> Result<(), Sdl3BackendError> {
        self.ensure_context(context)?;
        self.ensure_bound_entry()
    }

    pub(super) fn ensure_bound_entry(&self) -> Result<(), Sdl3BackendError> {
        self.request_failed_viewport_closes();
        self.poll_fault()?;
        if self.state.get() != RuntimeState::Attached {
            return Err(Sdl3BackendError::RuntimeDetached);
        }
        Ok(())
    }

    pub(super) fn finish_entry(&self) -> Result<(), Sdl3BackendError> {
        self.poll_fault()
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    pub(super) fn process_texture_requests(
        &self,
        requests: &[TextureRequest],
        request_epoch: u64,
    ) -> Result<Vec<TextureFeedback>, Sdl3BackendError> {
        let update_texture = self
            .lifecycle
            .renderer_texture_update
            .as_ref()
            .expect("initialized SDL3 renderer has no texture updater");
        self.renderer_textures
            .borrow_mut()
            .process_requests(requests, request_epoch, |texture| update_texture(texture))
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    pub(super) fn mark_textures_reconciled(&self, requests: &[TextureRequest]) {
        self.renderer_textures
            .borrow_mut()
            .mark_reconciled(requests);
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    pub(super) fn prune_destroyed_textures(&self, completion_watermark: u64) {
        self.renderer_textures
            .borrow_mut()
            .prune_destroyed(completion_watermark);
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    pub(super) fn clear_destroyed_textures(&self) {
        self.renderer_textures.borrow_mut().clear_destroyed();
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    pub(super) fn forget_textures_destroyed_by_upstream(&self) {
        self.renderer_textures
            .borrow_mut()
            .forget_destroyed_by_upstream();
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    pub(super) fn destroy_uninstalled_renderer_textures_bound(
        &self,
    ) -> Result<(), Sdl3BackendError> {
        let Some(update_texture) = self.lifecycle.renderer_texture_update.as_ref() else {
            return Ok(());
        };
        self.renderer_textures
            .borrow_mut()
            .destroy_uninstalled(|texture| update_texture(texture))
    }

    pub(super) fn ensure_context(&self, context: &Context) -> Result<(), Sdl3BackendError> {
        let expected = self.binding.id();
        let actual = context.id();
        if expected != actual {
            return Err(Sdl3BackendError::ContextMismatch { expected, actual });
        }
        Ok(())
    }

    pub(super) fn original_create_window(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)> {
        self.callbacks
            .borrow()
            .as_ref()
            .and_then(PlatformCallbackOwnership::original_create_window)
    }

    pub(super) fn original_platform_callback<R: Copy>(
        &self,
        select: impl FnOnce(&sys::ImGuiPlatformIO) -> R,
    ) -> Option<R> {
        self.callbacks
            .borrow()
            .as_ref()
            .map(|callbacks| callbacks.select_original(select))
    }

    pub(super) fn original_destroy_window(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)> {
        self.callbacks
            .borrow()
            .as_ref()
            .and_then(PlatformCallbackOwnership::original_destroy_window)
    }

    pub(super) fn original_render_window(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut std::ffi::c_void)> {
        self.callbacks
            .borrow()
            .as_ref()
            .and_then(PlatformCallbackOwnership::original_render_window)
    }

    pub(super) fn original_swap_buffers(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut std::ffi::c_void)> {
        self.callbacks
            .borrow()
            .as_ref()
            .and_then(PlatformCallbackOwnership::original_swap_buffers)
    }

    pub(super) fn original_renderer_create_window(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)> {
        self.renderer_callbacks
            .borrow()
            .as_ref()
            .and_then(RendererCallbackOwnership::original_create_window)
    }

    pub(super) fn original_renderer_destroy_window(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)> {
        self.renderer_callbacks
            .borrow()
            .as_ref()
            .and_then(RendererCallbackOwnership::original_destroy_window)
    }

    pub(super) fn original_renderer_render_window(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut std::ffi::c_void)> {
        self.renderer_callbacks
            .borrow()
            .as_ref()
            .and_then(RendererCallbackOwnership::original_render_window)
    }

    pub(super) fn original_renderer_set_window_size(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2_c)> {
        self.renderer_callbacks
            .borrow()
            .as_ref()
            .and_then(RendererCallbackOwnership::original_set_window_size)
    }

    pub(super) fn original_renderer_swap_buffers(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut std::ffi::c_void)> {
        self.renderer_callbacks
            .borrow()
            .as_ref()
            .and_then(RendererCallbackOwnership::original_swap_buffers)
    }

    pub(super) fn validate_renderer_ownership_bound(&self) -> bool {
        if self.native_renderer == NativeRendererKind::None {
            return true;
        }
        let callbacks = self.renderer_callbacks.borrow();
        let Some(callbacks) = callbacks.as_ref() else {
            self.record_renderer_state_replaced("renderer callback ownership");
            return false;
        };
        unsafe { callbacks.detect_replacements(self) }
    }

    pub(super) fn validate_platform_ownership_bound(&self) -> bool {
        let callbacks = self.callbacks.borrow();
        let Some(callbacks) = callbacks.as_ref() else {
            self.record_platform_state_replaced("platform callback ownership");
            return false;
        };
        unsafe { callbacks.detect_replacements(self) }
    }

    pub(super) fn callback_teardown_active(&self) -> bool {
        self.callback_teardown_active.get()
    }

    pub(super) fn refresh_platform_monitors_bound(&self) {
        if let Some(callbacks) = self.callbacks.borrow().as_ref() {
            unsafe { callbacks.refresh_owned_monitors() };
        }
    }

    pub(super) fn remember_owned_viewport(
        &self,
        viewport: *mut sys::ImGuiViewport,
        state: ViewportPlatformState,
    ) {
        self.owned_viewports
            .borrow_mut()
            .insert(viewport as usize, state);
    }

    pub(super) fn take_owned_viewport(
        &self,
        viewport: *mut sys::ImGuiViewport,
    ) -> Option<ViewportPlatformState> {
        self.owned_viewports
            .borrow_mut()
            .remove(&(viewport as usize))
    }

    pub(super) fn owned_viewport(
        &self,
        viewport: *mut sys::ImGuiViewport,
    ) -> Option<ViewportPlatformState> {
        self.owned_viewports
            .borrow()
            .get(&(viewport as usize))
            .copied()
    }

    pub(super) fn remember_owned_renderer_viewport(
        &self,
        viewport: *mut sys::ImGuiViewport,
        user_data: *mut std::ffi::c_void,
    ) {
        if !viewport.is_null() && !user_data.is_null() {
            self.owned_renderer_viewports
                .borrow_mut()
                .insert(viewport as usize, user_data);
        }
    }

    pub(super) fn owned_renderer_viewport(
        &self,
        viewport: *mut sys::ImGuiViewport,
    ) -> Option<*mut std::ffi::c_void> {
        self.owned_renderer_viewports
            .borrow()
            .get(&(viewport as usize))
            .copied()
    }

    pub(super) fn forget_owned_renderer_viewport(
        &self,
        viewport: *mut sys::ImGuiViewport,
    ) -> Option<*mut std::ffi::c_void> {
        self.owned_renderer_viewports
            .borrow_mut()
            .remove(&(viewport as usize))
    }

    pub(super) fn defer_platform_viewport_restore(
        &self,
        viewport: *mut sys::ImGuiViewport,
        state: ViewportPlatformState,
    ) {
        if !viewport.is_null() {
            self.deferred_platform_viewport_restores
                .borrow_mut()
                .insert(viewport as usize, state);
        }
    }

    pub(super) fn defer_renderer_viewport_restore(
        &self,
        viewport: *mut sys::ImGuiViewport,
        user_data: *mut std::ffi::c_void,
    ) {
        if !viewport.is_null() {
            self.deferred_renderer_viewport_restores
                .borrow_mut()
                .insert(viewport as usize, user_data);
        }
    }

    fn restore_deferred_viewport_state(&self) {
        let platform = self
            .deferred_platform_viewport_restores
            .borrow_mut()
            .drain()
            .collect::<Vec<_>>();
        let renderer = self
            .deferred_renderer_viewport_restores
            .borrow_mut()
            .drain()
            .collect::<Vec<_>>();
        for (viewport, state) in platform {
            let viewport = viewport as *mut sys::ImGuiViewport;
            if !viewport.is_null() {
                unsafe { state.restore(viewport) };
            }
        }
        for (viewport, user_data) in renderer {
            let viewport = viewport as *mut sys::ImGuiViewport;
            if !viewport.is_null() {
                unsafe { (*viewport).RendererUserData = user_data };
            }
        }
    }

    pub(super) fn mark_viewport_failed(&self, viewport: *mut sys::ImGuiViewport) {
        if viewport.is_null() {
            return;
        }
        self.failed_viewports.borrow_mut().insert(viewport as usize);
        unsafe {
            (*viewport).PlatformRequestClose = true;
            (*viewport).DrawData = std::ptr::null_mut();
        }
    }

    pub(super) fn viewport_failed(&self, viewport: *mut sys::ImGuiViewport) -> bool {
        !viewport.is_null()
            && self
                .failed_viewports
                .borrow()
                .contains(&(viewport as usize))
    }

    pub(super) fn forget_failed_viewport(&self, viewport: *mut sys::ImGuiViewport) -> bool {
        !viewport.is_null()
            && self
                .failed_viewports
                .borrow_mut()
                .remove(&(viewport as usize))
    }

    fn request_failed_viewport_closes(&self) {
        let viewports = self
            .failed_viewports
            .borrow()
            .iter()
            .copied()
            .collect::<Vec<_>>();
        let _ = self.binding.try_with_bound_context(|| {
            for viewport in viewports {
                let viewport = viewport as *mut sys::ImGuiViewport;
                if !viewport.is_null() {
                    unsafe {
                        (*viewport).PlatformRequestClose = true;
                        (*viewport).DrawData = std::ptr::null_mut();
                    }
                }
            }
        });
    }

    fn record_fault(&self, fault: RuntimeFault) {
        if !self.faults.borrow().contains(&fault) {
            self.faults.borrow_mut().push_back(fault);
        }
    }

    pub(super) fn record_callback_replaced(&self, callback: &'static str) {
        if self.reported_replacements.borrow_mut().insert(callback) {
            self.record_fault(RuntimeFault::CallbackReplaced(callback));
        }
        self.revoke_capabilities(SDL_PLATFORM_RESERVED_FLAGS);
        self.begin_shutdown();
    }

    pub(super) fn record_renderer_callback_replaced(&self, callback: &'static str) {
        if self.reported_replacements.borrow_mut().insert(callback) {
            self.record_fault(RuntimeFault::RendererCallbackReplaced(callback));
        }
        self.revoke_capabilities(SDL_RENDERER_RESERVED_FLAGS);
        self.begin_shutdown();
    }

    pub(super) fn record_platform_state_replaced(&self, field: &'static str) {
        if self.reported_replacements.borrow_mut().insert(field) {
            self.record_fault(RuntimeFault::PlatformStateReplaced(field));
        }
        self.revoke_capabilities(SDL_PLATFORM_RESERVED_FLAGS);
        self.begin_shutdown();
    }

    pub(super) fn record_renderer_state_replaced(&self, field: &'static str) {
        if self.reported_replacements.borrow_mut().insert(field) {
            self.record_fault(RuntimeFault::RendererStateReplaced(field));
        }
        self.revoke_capabilities(SDL_RENDERER_RESERVED_FLAGS);
        self.begin_shutdown();
    }

    /// Retain renderer capability bits only after a complete foreign renderer publication was
    /// observed. A single callback or core-field replacement is an incomplete takeover, so its
    /// untagged capability bits must stay revoked after SDL releases its own renderer.
    pub(super) fn preserve_complete_foreign_renderer_capabilities(&self, flags: i32) {
        self.mark_capabilities_foreign(SDL_RENDERER_RESERVED_FLAGS);
        unsafe {
            let io = sys::igGetIO_Nil();
            if !io.is_null() {
                (*io).BackendFlags = ((*io).BackendFlags & !SDL_RENDERER_RESERVED_FLAGS)
                    | (flags & SDL_RENDERER_RESERVED_FLAGS);
            }
        }
    }

    /// Retain platform capability bits only after the entire platform publication has moved to a
    /// foreign owner. Individual callback, userdata, or viewport-field replacements are partial
    /// takeovers and must leave SDL's capability bits revoked.
    pub(super) fn preserve_complete_foreign_platform_capabilities(&self, flags: i32) {
        self.mark_capabilities_foreign(SDL_PLATFORM_RESERVED_FLAGS);
        unsafe {
            let io = sys::igGetIO_Nil();
            if !io.is_null() {
                (*io).BackendFlags = ((*io).BackendFlags & !SDL_PLATFORM_RESERVED_FLAGS)
                    | (flags & SDL_PLATFORM_RESERVED_FLAGS);
            }
        }
    }

    pub(super) fn record_callback_panicked(&self, callback: &'static str) {
        self.record_fault(RuntimeFault::CallbackPanicked(callback));
        if callback.starts_with("Renderer_") {
            self.revoke_capabilities(SDL_RENDERER_RESERVED_FLAGS);
        } else {
            self.revoke_capabilities(SDL_PLATFORM_RESERVED_FLAGS);
        }
        self.begin_shutdown();
    }

    pub(super) fn record_foreign_platform_user_data(&self) {
        self.record_fault(RuntimeFault::ForeignPlatformUserData);
        self.revoke_capabilities(SDL_PLATFORM_RESERVED_FLAGS);
        self.begin_shutdown();
    }

    fn mark_capabilities_foreign(&self, mask: i32) {
        self.foreign_capabilities
            .set(self.foreign_capabilities.get() | mask);
    }

    fn revoke_capabilities(&self, mask: i32) {
        self.revoked_capabilities
            .set(self.revoked_capabilities.get() | mask);
        if self.capabilities_are_foreign(mask) {
            return;
        }
        unsafe {
            let io = sys::igGetIO_Nil();
            if !io.is_null() {
                (*io).BackendFlags &= !mask;
            }
        }
    }

    pub(super) fn capabilities_were_revoked(&self, mask: i32) -> bool {
        self.revoked_capabilities.get() & mask == mask
    }

    pub(super) fn capabilities_are_foreign(&self, mask: i32) -> bool {
        self.foreign_capabilities.get() & mask != 0
    }

    pub(super) fn record_viewport_creation_failed(&self) {
        self.record_fault(RuntimeFault::ViewportCreationFailed);
    }

    pub(super) fn record_native_faults(&self, faults: u64) {
        const GL_SHARE_CAPTURE: u64 = 1 << 0;
        const GL_SHARE_SET: u64 = 1 << 1;
        const GL_MAIN_CONTEXT: u64 = 1 << 2;
        const GL_MAIN_SWAP_INTERVAL: u64 = 1 << 3;
        const GL_CREATE_CONTEXT: u64 = 1 << 4;
        const GL_SET_SWAP_INTERVAL: u64 = 1 << 5;
        const GL_RESTORE_CONTEXT: u64 = 1 << 6;
        const GL_RESTORE_SHARE: u64 = 1 << 7;
        const GL_RENDER_CONTEXT: u64 = 1 << 8;
        const GL_SWAP_CONTEXT: u64 = 1 << 9;
        const GL_SWAP_WINDOW: u64 = 1 << 10;
        const SDLGPU_CLAIM: u64 = 1 << 11;
        const SDLGPU_CONFIGURE: u64 = 1 << 12;
        const NATIVE_PROTOCOL: u64 = 1 << 13;
        const SDLGPU_COMMAND_BUFFER: u64 = 1 << 14;
        const SDLGPU_SWAPCHAIN: u64 = 1 << 15;
        const SDLGPU_RENDER_PASS: u64 = 1 << 16;
        const SDLGPU_SUBMIT: u64 = 1 << 17;

        if faults & GL_SHARE_CAPTURE != 0 {
            self.record_fault(RuntimeFault::ViewportOpenGlStateCaptureFailed);
        }
        if faults & (GL_MAIN_CONTEXT | GL_CREATE_CONTEXT) != 0 {
            self.record_fault(RuntimeFault::ViewportOpenGlContextFailed);
        }
        if faults & (GL_MAIN_SWAP_INTERVAL | GL_SET_SWAP_INTERVAL) != 0 {
            self.record_fault(RuntimeFault::ViewportOpenGlSwapIntervalFailed);
        }
        if faults & (GL_SHARE_SET | GL_RESTORE_CONTEXT | GL_RESTORE_SHARE) != 0 {
            self.record_fault(RuntimeFault::ViewportOpenGlStateRestoreFailed);
        }
        if faults & GL_RENDER_CONTEXT != 0 {
            self.record_fault(RuntimeFault::ViewportOpenGlRenderContextFailed);
        }
        if faults & (GL_SWAP_CONTEXT | GL_SWAP_WINDOW) != 0 {
            self.record_fault(RuntimeFault::ViewportOpenGlSwapFailed);
        }
        if faults & SDLGPU_CLAIM != 0 {
            self.record_fault(RuntimeFault::ViewportSdlGpuClaimFailed);
        }
        if faults & SDLGPU_CONFIGURE != 0 {
            self.record_fault(RuntimeFault::ViewportSdlGpuConfigureFailed);
        }
        if faults & NATIVE_PROTOCOL != 0 {
            self.record_fault(RuntimeFault::NativeBridgeProtocolFailed);
        }
        if faults & SDLGPU_COMMAND_BUFFER != 0 {
            self.record_fault(RuntimeFault::ViewportSdlGpuCommandBufferFailed);
        }
        if faults & SDLGPU_SWAPCHAIN != 0 {
            self.record_fault(RuntimeFault::ViewportSdlGpuSwapchainFailed);
        }
        if faults & SDLGPU_RENDER_PASS != 0 {
            self.record_fault(RuntimeFault::ViewportSdlGpuRenderPassFailed);
        }
        if faults & SDLGPU_SUBMIT != 0 {
            self.record_fault(RuntimeFault::ViewportSdlGpuSubmitFailed);
        }
    }

    #[cfg(test)]
    pub(super) fn record_viewport_opengl_context_failed_for_test(&self) {
        self.record_fault(RuntimeFault::ViewportOpenGlContextFailed);
    }

    fn context_destroyed(&self) {
        unregister_runtime(self.platform_io_key.replace(0));
        self.callbacks.borrow_mut().take();
        self.renderer_callbacks.borrow_mut().take();
        self.renderer_shutdown_restore.borrow_mut().take();
        self.renderer_consumer.borrow_mut().take();
        self.owned_viewports.borrow_mut().clear();
        self.owned_renderer_viewports.borrow_mut().clear();
        self.deferred_platform_viewport_restores
            .borrow_mut()
            .clear();
        self.deferred_renderer_viewport_restores
            .borrow_mut()
            .clear();
        self.failed_viewports.borrow_mut().clear();
        self.platform_initialized.set(false);
        self.renderer_initialized.set(false);
        self.renderer_release.set(ReleaseState::Released);
        self.platform_release.set(ReleaseState::Released);
        self.state.set(RuntimeState::Detached);
    }

    fn mark_owner_dropped(&self) {
        if self.state.get() == RuntimeState::Detached {
            self.state.set(RuntimeState::ResourceDropped);
        }
    }

    #[cfg(test)]
    fn phase_log(&self) -> Vec<&'static str> {
        self.phase_log.borrow().clone()
    }

    fn accepts_current_callback(&self) -> bool {
        if !self.platform_initialized.get() {
            return false;
        }
        match (self.state.get(), self.binding.lifecycle()) {
            (RuntimeState::Attached, ContextLifecycle::Alive) => true,
            (RuntimeState::ShuttingDown, ContextLifecycle::Alive | ContextLifecycle::Dropping) => {
                self.callback_teardown_active.get()
            }
            _ => false,
        }
    }
}

struct PlatformAttachment {
    control: Rc<RuntimeControl>,
}

impl ContextAttachment for PlatformAttachment {
    fn quiesce(
        &self,
        _context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        self.control.begin_shutdown();
        Ok(())
    }

    fn release_platform_windows(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        self.control.begin_shutdown();
        context
            .with_bound_context(|| self.control.shutdown_bound_for_attachment())
            .map_err(|error| {
                ContextAttachmentTeardownError::new(format!(
                    "SDL3 Context teardown could not safely release native resources: {error}"
                ))
            })
    }

    fn context_destroyed(&self, _context: ContextDestroyed) {
        self.control.context_destroyed();
    }
}

struct RendererAttachment {
    control: Rc<RuntimeControl>,
}

impl ContextAttachment for RendererAttachment {
    fn release_renderer_resources(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        self.control.begin_shutdown();
        let Some(consumer) = self.control.take_renderer_consumer() else {
            if self.control.lifecycle.renderer_texture_update.is_some() {
                return Err(ContextAttachmentTeardownError::new(
                    "SDL3 renderer-resource teardown lost its renderer consumer",
                ));
            }
            return Ok(());
        };

        // OpenGL3 and SDLGPU3 call DestroyPlatformWindows() from full shutdown. Destroy only
        // their device objects here, then keep the callback tables alive until the platform phase.
        let reset = context.with_bound_context(|| {
            context.with_renderer_texture_reset(&consumer, || {
                self.control
                    .release_renderer_device_objects_bound()
                    .map_err(|error| {
                        ContextAttachmentTeardownError::new(format!(
                            "SDL3 Context teardown could not release renderer device objects: {error}"
                        ))
                    })
            })
        });
        if let Err(error) = reset {
            self.control.install_renderer_consumer(consumer);
            return Err(error);
        }
        #[cfg(any(
            feature = "opengl3-renderer",
            feature = "sdlrenderer3-renderer",
            feature = "sdlgpu3-renderer"
        ))]
        self.control.clear_destroyed_textures();
        Ok(())
    }
}

pub(super) struct RuntimeRegistration {
    control: Rc<RuntimeControl>,
    baseline: Option<PlatformClaimBaseline>,
    platform_attachment: Option<ContextAttachmentLease>,
    renderer_attachment: Option<ContextAttachmentLease>,
}

impl fmt::Debug for RuntimeRegistration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeRegistration")
            .field("control", &self.control)
            .field("native_initialization_pending", &self.baseline.is_some())
            .field("platform_attached", &self.platform_attachment.is_some())
            .field("renderer_attached", &self.renderer_attachment.is_some())
            .finish()
    }
}

impl RuntimeRegistration {
    pub(super) fn set_gl_viewport_swap_interval(&mut self, policy: Sdl3OpenGlViewportSwapInterval) {
        self.control.set_gl_viewport_swap_interval(policy);
    }

    pub(super) fn prepare_with_backend(
        context: &mut Context,
        renderer_shutdown: Option<fn()>,
        renderer_device_objects_destroy: Option<fn()>,
        renderer_texture_update: Option<fn(&mut TextureData)>,
        platform_graphics: PlatformGraphicsKind,
        native_renderer: NativeRendererKind,
    ) -> Result<Self, Sdl3BackendError> {
        let baseline = preflight_platform_claim(context, native_renderer)?;
        let renderer_shutdown = renderer_shutdown.map(|shutdown| Rc::new(shutdown) as Rc<dyn Fn()>);
        let renderer_device_objects_destroy =
            renderer_device_objects_destroy.map(|destroy| Rc::new(destroy) as Rc<dyn Fn()>);
        let renderer_texture_update =
            renderer_texture_update.map(|update| Rc::new(update) as Rc<dyn Fn(&mut TextureData)>);
        let control = Rc::new(RuntimeControl::new_with_backend(
            context,
            renderer_shutdown,
            renderer_device_objects_destroy,
            renderer_texture_update,
            Rc::new(shutdown_platform_impl),
            platform_graphics,
            native_renderer,
        ));
        let mut platform_attachment = context.register_attachment::<Sdl3PlatformAttachmentMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(PlatformAttachment {
                control: Rc::clone(&control),
            }),
        )?;
        let renderer_attachment = if control.lifecycle.renderer_shutdown.is_some() {
            match context.register_attachment::<Sdl3RendererAttachmentMarker>(
                ContextAttachmentRole::Renderer,
                Rc::new(RendererAttachment {
                    control: Rc::clone(&control),
                }),
            ) {
                Ok(attachment) => Some(attachment),
                Err(error) => {
                    platform_attachment.detach();
                    return Err(error.into());
                }
            }
        } else {
            None
        };

        Ok(Self {
            control,
            baseline: Some(baseline),
            platform_attachment: Some(platform_attachment),
            renderer_attachment,
        })
    }

    pub(super) fn finish_native_initialization(
        &mut self,
        context: &Context,
    ) -> Result<(), Sdl3BackendError> {
        self.control.ensure_context(context)?;
        let baseline = self
            .baseline
            .as_ref()
            .expect("SDL3 native initialization was already completed")
            .snapshot();
        let renderer_baseline = baseline.snapshot();
        self.control.platform_initialized.set(true);
        self.control
            .renderer_initialized
            .set(self.control.lifecycle.renderer_shutdown.is_some());
        let claim_result = self.control.binding.try_with_bound_context(|| unsafe {
            let platform = PlatformCallbackOwnership::claim(&self.control, baseline)?;
            let renderer = RendererCallbackOwnership::claim(&self.control, &renderer_baseline)?;
            Ok::<_, Sdl3BackendError>((platform, renderer))
        });
        match claim_result {
            Ok(Ok((ownership, renderer_ownership))) => {
                self.baseline.take();
                self.control.callbacks.borrow_mut().replace(ownership);
                if let Some(renderer_ownership) = renderer_ownership {
                    self.control
                        .renderer_callbacks
                        .borrow_mut()
                        .replace(renderer_ownership);
                }
                Ok(())
            }
            Ok(Err(error)) => {
                self.rollback_claim_failure();
                Err(error)
            }
            Err(error) => {
                self.rollback_claim_failure();
                Err(error.into())
            }
        }
    }

    fn rollback_claim_failure(&mut self) {
        let baseline = self.baseline.take();
        self.control.begin_shutdown();
        let _ = self.control.binding.try_with_bound_context(|| {
            let _ = self.control.release_platform_bound();
            if self.control.platform_released() {
                let _ = self.control.release_renderer_bound();
            }
            if let Some(baseline) = baseline {
                unsafe { restore_baseline_after_failed_initialization(baseline) };
            }
        });
        self.detach_attachments();
    }

    pub(super) fn native_initialization_failed(&mut self) {
        if let Some(baseline) = self.baseline.take() {
            let _ = self.control.binding.try_with_bound_context(|| unsafe {
                restore_baseline_after_failed_initialization(baseline)
            });
        }
        self.control.platform_initialized.set(false);
        self.control.renderer_initialized.set(false);
        self.control.take_renderer_consumer();
        self.control.renderer_release.set(ReleaseState::Released);
        self.control.platform_release.set(ReleaseState::Released);
        self.control.state.set(RuntimeState::Detached);
        self.detach_attachments();
    }

    pub(super) fn control(&self) -> &RuntimeControl {
        &self.control
    }

    pub(super) fn poll_fault(&self) -> Result<(), Sdl3BackendError> {
        self.control.poll_fault()
    }

    pub(super) fn normalize_open_frame_for_shutdown(
        &self,
        context: &mut Context,
    ) -> Result<(), Sdl3BackendError> {
        self.control.ensure_context(context)?;
        if self.control.state() == RuntimeState::Attached {
            context.end_frame();
        }
        Ok(())
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    pub(super) fn destroy_renderer_device_objects(
        &self,
        context: &mut Context,
        destroy_device_objects: impl FnOnce(),
    ) -> Result<(), Sdl3BackendError> {
        self.control.ensure_entry(context)?;
        let consumer_guard = self.control.renderer_consumer.borrow();
        let consumer = consumer_guard
            .as_ref()
            .expect("initialized SDL3 renderer lost its renderer consumer");
        let reset = context.prepare_renderer_texture_reset(consumer)?;

        self.control.binding.try_with_bound_context(|| {
            self.control.destroy_uninstalled_renderer_textures_bound()?;
            destroy_device_objects();
            self.control.forget_textures_destroyed_by_upstream();
            Ok::<(), Sdl3BackendError>(())
        })??;

        let _ = reset.commit();
        drop(consumer_guard);
        self.control.clear_destroyed_textures();
        self.control.finish_entry()
    }

    pub(super) fn shutdown_platform(
        &mut self,
        context: &mut Context,
    ) -> Result<(), Sdl3BackendError> {
        self.normalize_open_frame_for_shutdown(context)?;
        let pending = self.control.take_pending_fault();
        let shutdown_result = self.control.shutdown_native_explicit();
        if matches!(self.control.state(), RuntimeState::Detached) {
            self.detach_attachments();
        }
        first_error([pending, shutdown_result.err()])
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    pub(super) fn shutdown_renderer(
        &mut self,
        context: &mut Context,
    ) -> Result<(), Sdl3BackendError> {
        self.normalize_open_frame_for_shutdown(context)?;
        if matches!(
            self.control.state(),
            RuntimeState::Detached | RuntimeState::ResourceDropped
        ) {
            let pending = self.control.take_pending_fault();
            self.detach_attachments();
            return first_error([pending, None]);
        }

        let consumer_guard =
            (!self.control.renderer_released()).then(|| self.control.renderer_consumer.borrow());
        let mut reset = match consumer_guard.as_ref() {
            Some(consumer) => {
                let consumer = consumer
                    .as_ref()
                    .expect("initialized SDL3 renderer lost its renderer consumer");
                match context.prepare_renderer_texture_reset(consumer) {
                    Ok(reset) => Some(reset),
                    Err(error) => return Err(error.into()),
                }
            }
            None => None,
        };
        let pending = self.control.take_pending_fault();
        let shutdown_result = self.control.shutdown_native_explicit();
        let renderer_released = self.control.renderer_released();
        if renderer_released {
            if let Some(reset) = reset.take() {
                let _ = reset.commit();
            }
        }
        drop(reset);
        drop(consumer_guard);
        if renderer_released {
            self.control.take_renderer_consumer();
            self.control.clear_destroyed_textures();
        }
        if matches!(self.control.state(), RuntimeState::Detached) {
            self.detach_attachments();
        }
        first_error([pending, shutdown_result.err()])
    }

    fn detach_attachments(&mut self) {
        if let Some(mut renderer) = self.renderer_attachment.take() {
            renderer.detach();
        }
        if let Some(mut platform) = self.platform_attachment.take() {
            platform.detach();
        }
    }

    fn defer_attachments_to_context(&mut self) {
        if let Some(renderer) = self.renderer_attachment.take() {
            renderer.defer_to_context();
        }
        if let Some(platform) = self.platform_attachment.take() {
            platform.defer_to_context();
        }
    }
}

impl Drop for RuntimeRegistration {
    fn drop(&mut self) {
        self.defer_attachments_to_context();
        self.control.mark_owner_dropped();
    }
}

fn first_error<const N: usize>(
    errors: [Option<Sdl3BackendError>; N],
) -> Result<(), Sdl3BackendError> {
    errors.into_iter().flatten().next().map_or(Ok(()), Err)
}

thread_local! {
    static RUNTIMES: RefCell<HashMap<usize, Weak<RuntimeControl>>> = RefCell::new(HashMap::new());
}

pub(super) fn register_runtime(control: &Rc<RuntimeControl>) {
    let key = unsafe { sys::igGetPlatformIO_Nil() as usize };
    control.platform_io_key.set(key);
    RUNTIMES.with(|runtimes| {
        runtimes.borrow_mut().insert(key, Rc::downgrade(control));
    });
}

pub(super) fn unregister_runtime(key: usize) {
    if key == 0 {
        return;
    }
    RUNTIMES.with(|runtimes| {
        runtimes.borrow_mut().remove(&key);
    });
}

pub(super) fn with_current_runtime<R>(callback: impl FnOnce(&RuntimeControl) -> R) -> Option<R> {
    let key = unsafe { sys::igGetPlatformIO_Nil() as usize };
    RUNTIMES.with(|runtimes| {
        let control = runtimes.borrow().get(&key).cloned()?.upgrade()?;
        control
            .accepts_current_callback()
            .then(|| callback(&control))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "sdlgpu3-renderer")]
    use crate::callback_ownership::finish_sdlgpu_renderer_create;
    use crate::callback_ownership::{
        create_window_callback_for_test, destroy_window_callback_for_test,
        render_window_callback_for_test, swap_buffers_callback_for_test,
    };
    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    use crate::callback_ownership::{
        renderer_render_window_callback_for_test, renderer_set_window_size_callback_for_test,
    };

    const OWNED_BACKEND_DATA: usize = 0x101;
    const FOREIGN_BACKEND_DATA: usize = 0x102;
    const OWNED_PLATFORM_DATA: usize = 0x201;
    const FOREIGN_PLATFORM_DATA: usize = 0x202;
    const OWNED_VIEWPORT_DATA: usize = 0x301;
    const FOREIGN_VIEWPORT_DATA: usize = 0x302;
    const OWNED_VIEWPORT_HANDLE: usize = 0x401;
    const FOREIGN_VIEWPORT_HANDLE: usize = 0x402;
    const FOREIGN_VIEWPORT_HANDLE_RAW: usize = 0x403;
    static OWNED_BACKEND_NAME: &[u8] = b"SDL3-test\0";
    static FOREIGN_BACKEND_NAME: &[u8] = b"foreign-test\0";
    static FOREIGN_CLIPBOARD_TEXT: &[u8] = b"foreign clipboard\0";

    thread_local! {
        static DESTROY_OBSERVED_USER_DATA: Cell<usize> = const { Cell::new(0) };
        static PLATFORM_RENDER_COUNT: Cell<usize> = const { Cell::new(0) };
        static PLATFORM_SWAP_COUNT: Cell<usize> = const { Cell::new(0) };
        static RENDERER_RENDER_COUNT: Cell<usize> = const { Cell::new(0) };
        static RENDERER_SET_SIZE_COUNT: Cell<usize> = const { Cell::new(0) };
        static OWNED_RENDERER_DESTROY_COUNT: Cell<usize> = const { Cell::new(0) };
        static FOREIGN_RENDERER_DESTROY_COUNT: Cell<usize> = const { Cell::new(0) };
        static RENDERER_DESTROY_OBSERVED_USER_DATA: Cell<usize> = const { Cell::new(0) };
    }

    unsafe extern "C" fn synthetic_create_window(viewport: *mut sys::ImGuiViewport) {
        if let Some(viewport) = unsafe { viewport.as_mut() } {
            viewport.PlatformUserData = OWNED_VIEWPORT_DATA as *mut _;
            viewport.PlatformHandle = OWNED_VIEWPORT_HANDLE as *mut _;
        }
    }

    unsafe extern "C" fn synthetic_destroy_window(viewport: *mut sys::ImGuiViewport) {
        if let Some(viewport) = unsafe { viewport.as_mut() } {
            DESTROY_OBSERVED_USER_DATA
                .with(|observed| observed.set(viewport.PlatformUserData as usize));
            viewport.PlatformUserData = std::ptr::null_mut();
            viewport.PlatformHandle = std::ptr::null_mut();
            viewport.PlatformHandleRaw = std::ptr::null_mut();
        }
    }

    unsafe extern "C" fn foreign_create_window(_viewport: *mut sys::ImGuiViewport) {}

    unsafe extern "C" fn foreign_destroy_window(_viewport: *mut sys::ImGuiViewport) {}

    unsafe extern "C" fn foreign_get_clipboard_text(
        _context: *mut sys::ImGuiContext,
    ) -> *const std::ffi::c_char {
        FOREIGN_CLIPBOARD_TEXT.as_ptr().cast()
    }

    unsafe extern "C" fn foreign_set_clipboard_text(
        _context: *mut sys::ImGuiContext,
        _text: *const std::ffi::c_char,
    ) {
    }

    unsafe extern "C" fn foreign_platform_render_window(
        _viewport: *mut sys::ImGuiViewport,
        _argument: *mut std::ffi::c_void,
    ) {
    }

    unsafe extern "C" fn foreign_platform_swap_buffers(
        _viewport: *mut sys::ImGuiViewport,
        _argument: *mut std::ffi::c_void,
    ) {
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    unsafe extern "C" fn synthetic_renderer_render_window(
        _viewport: *mut sys::ImGuiViewport,
        _argument: *mut std::ffi::c_void,
    ) {
        RENDERER_RENDER_COUNT.with(|count| count.set(count.get() + 1));
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    unsafe extern "C" fn synthetic_renderer_set_window_size(
        _viewport: *mut sys::ImGuiViewport,
        _size: sys::ImVec2_c,
    ) {
        RENDERER_SET_SIZE_COUNT.with(|count| count.set(count.get() + 1));
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    unsafe extern "C" fn foreign_renderer_render_window(
        _viewport: *mut sys::ImGuiViewport,
        _argument: *mut std::ffi::c_void,
    ) {
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    unsafe extern "C" fn foreign_renderer_set_window_size(
        _viewport: *mut sys::ImGuiViewport,
        _size: sys::ImVec2_c,
    ) {
    }

    unsafe extern "C" fn synthetic_platform_render_window(
        _viewport: *mut sys::ImGuiViewport,
        _argument: *mut std::ffi::c_void,
    ) {
        PLATFORM_RENDER_COUNT.with(|count| count.set(count.get() + 1));
    }

    unsafe extern "C" fn synthetic_platform_swap_buffers(
        _viewport: *mut sys::ImGuiViewport,
        _argument: *mut std::ffi::c_void,
    ) {
        PLATFORM_SWAP_COUNT.with(|count| count.set(count.get() + 1));
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    unsafe extern "C" fn synthetic_renderer_destroy_window(viewport: *mut sys::ImGuiViewport) {
        OWNED_RENDERER_DESTROY_COUNT.with(|count| count.set(count.get() + 1));
        if let Some(viewport) = unsafe { viewport.as_mut() } {
            RENDERER_DESTROY_OBSERVED_USER_DATA
                .with(|observed| observed.set(viewport.RendererUserData as usize));
            viewport.RendererUserData = std::ptr::null_mut();
        }
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    unsafe extern "C" fn foreign_renderer_destroy_window(_viewport: *mut sys::ImGuiViewport) {
        FOREIGN_RENDERER_DESTROY_COUNT.with(|count| count.set(count.get() + 1));
    }

    unsafe extern "C" fn failing_create_window(_viewport: *mut sys::ImGuiViewport) {}

    unsafe extern "C" fn foreign_set_window_alpha(_viewport: *mut sys::ImGuiViewport, _alpha: f32) {
    }

    fn registration_with_lifecycle(
        context: &mut Context,
        renderer_shutdown: Option<Rc<dyn Fn()>>,
        platform_shutdown: Rc<dyn Fn()>,
    ) -> RuntimeRegistration {
        registration_with_backend_lifecycle(
            context,
            renderer_shutdown,
            platform_shutdown,
            PlatformGraphicsKind::Other,
            NativeRendererKind::None,
        )
    }

    fn registration_with_backend_lifecycle(
        context: &mut Context,
        renderer_shutdown: Option<Rc<dyn Fn()>>,
        platform_shutdown: Rc<dyn Fn()>,
        platform_graphics: PlatformGraphicsKind,
        native_renderer: NativeRendererKind,
    ) -> RuntimeRegistration {
        registration_with_backend_lifecycle_and_texture_update(
            context,
            renderer_shutdown,
            None,
            None,
            platform_shutdown,
            platform_graphics,
            native_renderer,
        )
    }

    fn registration_with_backend_lifecycle_and_texture_update(
        context: &mut Context,
        renderer_shutdown: Option<Rc<dyn Fn()>>,
        renderer_device_objects_destroy: Option<Rc<dyn Fn()>>,
        renderer_texture_update: Option<Rc<dyn Fn(&mut TextureData)>>,
        platform_shutdown: Rc<dyn Fn()>,
        platform_graphics: PlatformGraphicsKind,
        native_renderer: NativeRendererKind,
    ) -> RuntimeRegistration {
        let baseline = preflight_platform_claim(context, native_renderer).unwrap();
        let control = Rc::new(RuntimeControl::new_with_backend(
            context,
            renderer_shutdown,
            renderer_device_objects_destroy,
            renderer_texture_update,
            platform_shutdown,
            platform_graphics,
            native_renderer,
        ));
        let platform_attachment = context
            .register_attachment::<Sdl3PlatformAttachmentMarker>(
                ContextAttachmentRole::Platform,
                Rc::new(PlatformAttachment {
                    control: Rc::clone(&control),
                }),
            )
            .unwrap();
        let renderer_attachment = control.lifecycle.renderer_shutdown.as_ref().map(|_| {
            context
                .register_attachment::<Sdl3RendererAttachmentMarker>(
                    ContextAttachmentRole::Renderer,
                    Rc::new(RendererAttachment {
                        control: Rc::clone(&control),
                    }),
                )
                .unwrap()
        });
        RuntimeRegistration {
            control,
            baseline: Some(baseline),
            platform_attachment: Some(platform_attachment),
            renderer_attachment,
        }
    }

    fn test_registration(
        context: &mut Context,
        renderer_count: Rc<Cell<usize>>,
        platform_count: Rc<Cell<usize>>,
    ) -> RuntimeRegistration {
        let registration = registration_with_lifecycle(
            context,
            Some({
                let renderer_count = Rc::clone(&renderer_count);
                Rc::new(move || renderer_count.set(renderer_count.get() + 1))
            }),
            {
                let platform_count = Rc::clone(&platform_count);
                Rc::new(move || platform_count.set(platform_count.get() + 1))
            },
        );
        registration.control.platform_initialized.set(true);
        registration.control.renderer_initialized.set(true);
        registration
    }

    fn synthetic_claimed_registration(
        context: &mut Context,
        platform_count: Rc<Cell<usize>>,
        observed_backend_data: Rc<Cell<usize>>,
        observed_main_viewport_data: Rc<Cell<usize>>,
        create_window: unsafe extern "C" fn(*mut sys::ImGuiViewport),
    ) -> RuntimeRegistration {
        synthetic_claimed_registration_with_renderer(
            context,
            None,
            None,
            platform_count,
            observed_backend_data,
            observed_main_viewport_data,
            create_window,
        )
    }

    fn synthetic_claimed_registration_with_renderer(
        context: &mut Context,
        renderer_shutdown: Option<Rc<dyn Fn()>>,
        platform_shutdown_hook: Option<Rc<dyn Fn()>>,
        platform_count: Rc<Cell<usize>>,
        observed_backend_data: Rc<Cell<usize>>,
        observed_main_viewport_data: Rc<Cell<usize>>,
        create_window: unsafe extern "C" fn(*mut sys::ImGuiViewport),
    ) -> RuntimeRegistration {
        let mut registration = registration_with_lifecycle(
            context,
            renderer_shutdown,
            Rc::new({
                let platform_count = Rc::clone(&platform_count);
                let observed_backend_data = Rc::clone(&observed_backend_data);
                let observed_main_viewport_data = Rc::clone(&observed_main_viewport_data);
                let platform_shutdown_hook = platform_shutdown_hook.clone();
                move || unsafe {
                    platform_count.set(platform_count.get() + 1);
                    if let Some(hook) = &platform_shutdown_hook {
                        hook();
                    }
                    let io = sys::igGetIO_Nil();
                    let platform_io = sys::igGetPlatformIO_Nil();
                    let main_viewport = sys::igGetMainViewport();
                    observed_backend_data.set((*io).BackendPlatformUserData as usize);
                    observed_main_viewport_data.set((*main_viewport).PlatformUserData as usize);

                    sys::ImGuiPlatformIO_ClearPlatformHandlers(platform_io);
                    (*io).BackendPlatformUserData = std::ptr::null_mut();
                    (*io).BackendPlatformName = std::ptr::null();
                    (*io).BackendFlags &= !SDL_PLATFORM_RESERVED_FLAGS;
                    (*main_viewport).PlatformUserData = std::ptr::null_mut();
                    (*main_viewport).PlatformHandle = std::ptr::null_mut();
                    (*main_viewport).PlatformHandleRaw = std::ptr::null_mut();
                }
            }),
        );

        context.binding().with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            let main_viewport = sys::igGetMainViewport();
            (*io).BackendPlatformUserData = OWNED_BACKEND_DATA as *mut _;
            (*io).BackendPlatformName = OWNED_BACKEND_NAME.as_ptr().cast();
            (*io).BackendFlags |= sys::ImGuiBackendFlags_HasMouseCursors as i32
                | sys::ImGuiBackendFlags_HasSetMousePos as i32
                | sys::ImGuiBackendFlags_PlatformHasViewports as i32
                | sys::ImGuiBackendFlags_HasParentViewport as i32;
            (*platform_io).Platform_CreateWindow = Some(create_window);
            (*platform_io).Platform_DestroyWindow = Some(synthetic_destroy_window);
            (*platform_io).Platform_RenderWindow = Some(synthetic_platform_render_window);
            (*platform_io).Platform_SwapBuffers = Some(synthetic_platform_swap_buffers);
            (*platform_io).Platform_ClipboardUserData = OWNED_PLATFORM_DATA as *mut _;
            (*main_viewport).PlatformUserData = OWNED_VIEWPORT_DATA as *mut _;
            (*main_viewport).PlatformHandle = OWNED_VIEWPORT_HANDLE as *mut _;
        });

        let baseline = registration.baseline.take().unwrap();
        let ownership = context.binding().with_bound_context(|| unsafe {
            PlatformCallbackOwnership::claim(&registration.control, baseline).unwrap()
        });
        registration
            .control
            .callbacks
            .borrow_mut()
            .replace(ownership);
        registration.control.platform_initialized.set(true);
        registration
            .control
            .renderer_initialized
            .set(registration.control.lifecycle.renderer_shutdown.is_some());
        registration
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    fn synthetic_renderer_registration(context: &mut Context) -> RuntimeRegistration {
        #[cfg(feature = "opengl3-renderer")]
        let native_renderer = NativeRendererKind::OpenGl3;
        #[cfg(all(not(feature = "opengl3-renderer"), feature = "sdlrenderer3-renderer"))]
        let native_renderer = NativeRendererKind::SdlRenderer3;
        #[cfg(all(
            not(feature = "opengl3-renderer"),
            not(feature = "sdlrenderer3-renderer"),
            feature = "sdlgpu3-renderer"
        ))]
        let native_renderer = NativeRendererKind::SdlGpu3;

        let mut registration = registration_with_backend_lifecycle(
            context,
            Some(Rc::new(|| unsafe {
                let io = sys::igGetIO_Nil();
                let platform_io = sys::igGetPlatformIO_Nil();
                sys::ImGuiPlatformIO_ClearRendererHandlers(platform_io);
                (*io).BackendRendererUserData = std::ptr::null_mut();
                (*io).BackendRendererName = std::ptr::null();
                (*io).BackendFlags &= !SDL_RENDERER_RESERVED_FLAGS;
            })),
            Rc::new(|| unsafe {
                let io = sys::igGetIO_Nil();
                let platform_io = sys::igGetPlatformIO_Nil();
                let main_viewport = sys::igGetMainViewport();
                sys::ImGuiPlatformIO_ClearPlatformHandlers(platform_io);
                (*io).BackendPlatformUserData = std::ptr::null_mut();
                (*io).BackendPlatformName = std::ptr::null();
                (*io).BackendFlags &= !SDL_PLATFORM_RESERVED_FLAGS;
                (*main_viewport).PlatformUserData = std::ptr::null_mut();
                (*main_viewport).PlatformHandle = std::ptr::null_mut();
                (*main_viewport).PlatformHandleRaw = std::ptr::null_mut();
            }),
            PlatformGraphicsKind::Other,
            native_renderer,
        );

        context.binding().with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            let main_viewport = sys::igGetMainViewport();
            (*io).BackendPlatformUserData = OWNED_BACKEND_DATA as *mut _;
            (*io).BackendPlatformName = OWNED_BACKEND_NAME.as_ptr().cast();
            (*io).BackendRendererUserData = OWNED_BACKEND_DATA as *mut _;
            (*io).BackendRendererName = OWNED_BACKEND_NAME.as_ptr().cast();
            (*io).BackendFlags |= sys::ImGuiBackendFlags_HasMouseCursors as i32
                | sys::ImGuiBackendFlags_HasSetMousePos as i32
                | sys::ImGuiBackendFlags_PlatformHasViewports as i32
                | sys::ImGuiBackendFlags_HasParentViewport as i32
                | sys::ImGuiBackendFlags_RendererHasViewports as i32;
            (*platform_io).Platform_CreateWindow = Some(synthetic_create_window);
            (*platform_io).Platform_DestroyWindow = Some(synthetic_destroy_window);
            (*platform_io).Renderer_RenderWindow = Some(synthetic_renderer_render_window);
            (*platform_io).Renderer_DestroyWindow = Some(synthetic_renderer_destroy_window);
            (*platform_io).Renderer_SetWindowSize = Some(synthetic_renderer_set_window_size);
            (*main_viewport).PlatformUserData = OWNED_VIEWPORT_DATA as *mut _;
            (*main_viewport).PlatformHandle = OWNED_VIEWPORT_HANDLE as *mut _;
        });

        let baseline = registration.baseline.take().unwrap();
        let renderer_baseline = baseline.snapshot();
        let (platform, renderer) = context.binding().with_bound_context(|| unsafe {
            (
                PlatformCallbackOwnership::claim(&registration.control, baseline).unwrap(),
                RendererCallbackOwnership::claim(&registration.control, &renderer_baseline)
                    .unwrap()
                    .unwrap(),
            )
        });
        registration
            .control
            .callbacks
            .borrow_mut()
            .replace(platform);
        registration
            .control
            .renderer_callbacks
            .borrow_mut()
            .replace(renderer);
        registration.control.platform_initialized.set(true);
        registration.control.renderer_initialized.set(true);
        registration
    }

    fn registry_contains(key: usize) -> bool {
        RUNTIMES.with(|runtimes| runtimes.borrow().contains_key(&key))
    }

    #[test]
    fn callback_registry_routes_each_current_context_to_its_own_runtime() {
        let _guard = crate::tests::test_guard();
        let mut context_a = Context::create();
        let runtime_a = synthetic_claimed_registration(
            &mut context_a,
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            synthetic_create_window,
        );
        let id_a = context_a.id();
        let key_a = runtime_a.control.platform_io_key.get();
        assert_eq!(
            with_current_runtime(|control| control.binding.id()),
            Some(id_a)
        );

        let suspended_a = context_a.suspend();
        let mut context_b = Context::create();
        let mut runtime_b = synthetic_claimed_registration(
            &mut context_b,
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            synthetic_create_window,
        );
        let id_b = context_b.id();
        let key_b = runtime_b.control.platform_io_key.get();
        assert_ne!(key_a, key_b);
        assert_eq!(
            with_current_runtime(|control| control.binding.id()),
            Some(id_b)
        );

        let suspended_b = context_b.suspend();
        let mut context_a = suspended_a.activate().expect("Context A should reactivate");
        assert_eq!(
            with_current_runtime(|control| control.binding.id()),
            Some(id_a)
        );
        let mut runtime_a = runtime_a;
        runtime_a.shutdown_platform(&mut context_a).unwrap();
        assert!(!registry_contains(key_a));
        assert!(registry_contains(key_b));
        drop(context_a);

        let mut context_b = suspended_b.activate().expect("Context B should reactivate");
        assert_eq!(
            with_current_runtime(|control| control.binding.id()),
            Some(id_b)
        );
        runtime_b.shutdown_platform(&mut context_b).unwrap();
        assert!(!registry_contains(key_b));
    }

    struct TeardownPhaseObserver {
        renderer_count: Rc<Cell<usize>>,
        platform_count: Rc<Cell<usize>>,
        renderer_phase_counts: Rc<Cell<(usize, usize)>>,
        platform_phase_counts: Rc<Cell<(usize, usize)>>,
    }

    impl ContextAttachment for TeardownPhaseObserver {
        fn release_renderer_resources(
            &self,
            _context: &ContextTeardown<'_>,
        ) -> Result<(), ContextAttachmentTeardownError> {
            self.renderer_phase_counts
                .set((self.renderer_count.get(), self.platform_count.get()));
            Ok(())
        }

        fn release_platform_windows(
            &self,
            _context: &ContextTeardown<'_>,
        ) -> Result<(), ContextAttachmentTeardownError> {
            self.platform_phase_counts
                .set((self.renderer_count.get(), self.platform_count.get()));
            Ok(())
        }
    }

    struct TeardownPhaseObserverMarker;

    #[test]
    fn context_first_shutdown_runs_each_phase_once_in_order() {
        let _guard = crate::tests::test_guard();
        let renderer_count = Rc::new(Cell::new(0));
        let platform_count = Rc::new(Cell::new(0));
        let mut context = Context::create();
        let runtime = test_registration(
            &mut context,
            Rc::clone(&renderer_count),
            Rc::clone(&platform_count),
        );
        let renderer_phase_counts = Rc::new(Cell::new((usize::MAX, usize::MAX)));
        let platform_phase_counts = Rc::new(Cell::new((usize::MAX, usize::MAX)));
        let _observer = context
            .register_attachment::<TeardownPhaseObserverMarker>(
                ContextAttachmentRole::Extension,
                Rc::new(TeardownPhaseObserver {
                    renderer_count: Rc::clone(&renderer_count),
                    platform_count: Rc::clone(&platform_count),
                    renderer_phase_counts: Rc::clone(&renderer_phase_counts),
                    platform_phase_counts: Rc::clone(&platform_phase_counts),
                }),
            )
            .unwrap();
        let control = Rc::clone(&runtime.control);

        drop(context);

        assert_eq!(renderer_phase_counts.get(), (0, 0));
        assert_eq!(platform_phase_counts.get(), (1, 1));
        assert_eq!(renderer_count.get(), 1);
        assert_eq!(platform_count.get(), 1);
        assert_eq!(control.phase_log(), ["platform", "renderer"]);
        assert_eq!(control.state(), RuntimeState::Detached);
        drop(runtime);
        assert_eq!(control.state(), RuntimeState::ResourceDropped);
        assert_eq!(renderer_count.get(), 1);
        assert_eq!(platform_count.get(), 1);
    }

    #[test]
    fn wrapper_first_and_repeated_shutdown_are_idempotent_after_move() {
        let _guard = crate::tests::test_guard();
        let renderer_count = Rc::new(Cell::new(0));
        let platform_count = Rc::new(Cell::new(0));
        let mut context = Context::create();
        let runtime = test_registration(
            &mut context,
            Rc::clone(&renderer_count),
            Rc::clone(&platform_count),
        );
        let control_address = Rc::as_ptr(&runtime.control);
        let mut slot = Some(runtime);
        let mut moved = slot.take().unwrap();
        assert_eq!(Rc::as_ptr(&moved.control), control_address);

        moved.shutdown_platform(&mut context).unwrap();
        moved.shutdown_platform(&mut context).unwrap();

        assert_eq!(renderer_count.get(), 1);
        assert_eq!(platform_count.get(), 1);
        assert_eq!(moved.control.phase_log(), ["platform", "renderer"]);
        drop(moved);
        drop(context);
        assert_eq!(renderer_count.get(), 1);
        assert_eq!(platform_count.get(), 1);
    }

    #[test]
    fn platform_shutdown_keeps_platform_destroy_callbacks_live() {
        let _guard = crate::tests::test_guard();
        let mut context = Context::create();
        let viewport = Rc::new(RefCell::new(sys::ImGuiViewport::default()));
        let platform_shutdown_hook: Rc<dyn Fn()> = {
            let viewport = Rc::clone(&viewport);
            Rc::new(move || unsafe {
                destroy_window_callback_for_test(&mut *viewport.borrow_mut());
            })
        };
        let platform_count = Rc::new(Cell::new(0));
        let observed_backend_data = Rc::new(Cell::new(0));
        let observed_main_viewport_data = Rc::new(Cell::new(0));
        let mut runtime = synthetic_claimed_registration_with_renderer(
            &mut context,
            Some(Rc::new(|| {})),
            Some(platform_shutdown_hook),
            Rc::clone(&platform_count),
            observed_backend_data,
            observed_main_viewport_data,
            synthetic_create_window,
        );

        context.binding().with_bound_context(|| unsafe {
            create_window_callback_for_test(&mut *viewport.borrow_mut());
        });
        assert_eq!(
            viewport.borrow().PlatformUserData as usize,
            OWNED_VIEWPORT_DATA
        );

        runtime.shutdown_platform(&mut context).unwrap();

        assert!(viewport.borrow().PlatformUserData.is_null());
        assert!(viewport.borrow().PlatformHandle.is_null());
        assert_eq!(platform_count.get(), 1);
    }

    #[test]
    fn explicit_shutdown_normalizes_an_open_frame_before_native_release() {
        let _guard = crate::tests::test_guard();
        let frame_open_during_platform_shutdown = Rc::new(Cell::new(true));
        let renderer_count = Rc::new(Cell::new(0));
        let platform_count = Rc::new(Cell::new(0));
        let mut context = Context::create();
        let mut runtime = registration_with_lifecycle(
            &mut context,
            Some({
                let renderer_count = Rc::clone(&renderer_count);
                Rc::new(move || renderer_count.set(renderer_count.get() + 1))
            }),
            {
                let platform_count = Rc::clone(&platform_count);
                let frame_open = Rc::clone(&frame_open_during_platform_shutdown);
                Rc::new(move || unsafe {
                    platform_count.set(platform_count.get() + 1);
                    let context = sys::igGetCurrentContext();
                    frame_open.set(!context.is_null() && (*context).WithinFrameScope);
                })
            },
        );
        runtime.control.platform_initialized.set(true);
        runtime.control.renderer_initialized.set(true);
        context.prepare_frame(dear_imgui_rs::FramePrepareOptions::new(
            [320.0, 240.0],
            1.0 / 60.0,
        ));
        let _ = context.font_atlas().build();
        context.frame().text("close before SDL teardown");

        runtime.shutdown_platform(&mut context).unwrap();

        assert!(!frame_open_during_platform_shutdown.get());
        assert_eq!(
            context.frame_lifecycle_state(),
            dear_imgui_rs::FrameLifecycleState::Idle
        );
        assert_eq!(renderer_count.get(), 1);
        assert_eq!(platform_count.get(), 1);

        context.prepare_frame(dear_imgui_rs::FramePrepareOptions::new(
            [320.0, 240.0],
            1.0 / 60.0,
        ));
        context.frame().text("context remains reusable");
        assert!(context.end_frame());
    }

    #[test]
    fn wrapper_drop_defers_each_phase_to_context_teardown() {
        let _guard = crate::tests::test_guard();
        let renderer_count = Rc::new(Cell::new(0));
        let platform_count = Rc::new(Cell::new(0));
        let mut context = Context::create();
        let runtime = test_registration(
            &mut context,
            Rc::clone(&renderer_count),
            Rc::clone(&platform_count),
        );
        let control = Rc::clone(&runtime.control);

        drop(runtime);

        assert_eq!(renderer_count.get(), 0);
        assert_eq!(platform_count.get(), 0);
        assert!(control.phase_log().is_empty());
        assert_eq!(control.state(), RuntimeState::Attached);
        drop(context);
        assert_eq!(renderer_count.get(), 1);
        assert_eq!(platform_count.get(), 1);
        assert_eq!(control.phase_log(), ["platform", "renderer"]);
        assert_eq!(control.state(), RuntimeState::Detached);
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    #[test]
    fn wrapper_drop_keeps_uninstalled_texture_proxies_alive_until_context_teardown() {
        let _guard = crate::tests::test_guard();
        let destroy_count = Rc::new(Cell::new(0));
        let device_destroy_count = Rc::new(Cell::new(0));
        let renderer_count = Rc::new(Cell::new(0));
        let platform_count = Rc::new(Cell::new(0));
        let mut context = Context::create();
        let texture = dear_imgui_rs::render::SnapshotTextureId::FontAtlas {
            context: context.id(),
            stamp: 1,
            generation: 1,
        };
        let runtime = registration_with_backend_lifecycle_and_texture_update(
            &mut context,
            Some({
                let renderer_count = Rc::clone(&renderer_count);
                Rc::new(move || renderer_count.set(renderer_count.get() + 1))
            }),
            Some({
                let device_destroy_count = Rc::clone(&device_destroy_count);
                Rc::new(move || device_destroy_count.set(device_destroy_count.get() + 1))
            }),
            Some({
                let destroy_count = Rc::clone(&destroy_count);
                Rc::new(move |texture: &mut TextureData| {
                    if texture.status() == dear_imgui_rs::TextureStatus::WantDestroy {
                        destroy_count.set(destroy_count.get() + 1);
                        unsafe {
                            texture.set_status(dear_imgui_rs::TextureStatus::Destroyed);
                        }
                    }
                })
            }),
            {
                let platform_count = Rc::clone(&platform_count);
                Rc::new(move || platform_count.set(platform_count.get() + 1))
            },
            PlatformGraphicsKind::Other,
            NativeRendererKind::None,
        );
        runtime
            .control
            .renderer_textures
            .borrow_mut()
            .insert_uninstalled_for_test(texture);
        runtime.control.platform_initialized.set(true);
        runtime.control.renderer_initialized.set(true);
        runtime
            .control
            .install_renderer_consumer(context.create_renderer_consumer().unwrap());

        drop(runtime);
        assert_eq!(destroy_count.get(), 0);
        assert_eq!(device_destroy_count.get(), 0);
        drop(context);

        assert_eq!(destroy_count.get(), 1);
        assert_eq!(device_destroy_count.get(), 1);
        assert_eq!(renderer_count.get(), 1);
        assert_eq!(platform_count.get(), 1);
    }

    #[test]
    fn deferred_owner_can_finish_fallible_teardown_before_context_drop() {
        let _guard = crate::tests::test_guard();
        let renderer_count = Rc::new(Cell::new(0));
        let platform_count = Rc::new(Cell::new(0));
        let mut context = Context::create();
        let runtime = registration_with_lifecycle(
            &mut context,
            Some({
                let renderer_count = Rc::clone(&renderer_count);
                Rc::new(move || {
                    let attempt = renderer_count.get() + 1;
                    renderer_count.set(attempt);
                    if attempt == 1 {
                        panic!("synthetic renderer shutdown failure");
                    }
                })
            }),
            {
                let platform_count = Rc::clone(&platform_count);
                Rc::new(move || platform_count.set(platform_count.get() + 1))
            },
        );
        runtime.control.platform_initialized.set(true);
        runtime.control.renderer_initialized.set(true);
        let control = Rc::clone(&runtime.control);

        drop(runtime);

        assert_eq!(renderer_count.get(), 0);
        assert_eq!(platform_count.get(), 0);
        control.begin_shutdown();
        let first = context
            .binding()
            .with_bound_context(|| control.shutdown_bound_for_attachment());
        assert!(matches!(
            first,
            Err(Sdl3BackendError::ShutdownPanicked {
                phase: "renderer resources"
            })
        ));
        context
            .binding()
            .with_bound_context(|| control.shutdown_bound_for_attachment())
            .unwrap();
        drop(context);

        assert_eq!(renderer_count.get(), 2);
        assert_eq!(platform_count.get(), 1);
        assert_eq!(control.phase_log(), ["platform", "renderer", "renderer"]);
        assert_eq!(control.state(), RuntimeState::Detached);
    }

    #[test]
    fn explicit_shutdown_reports_renderer_panic_after_completing_cleanup() {
        let _guard = crate::tests::test_guard();
        let renderer_count = Rc::new(Cell::new(0));
        let platform_count = Rc::new(Cell::new(0));
        let mut context = Context::create();
        let mut runtime = registration_with_lifecycle(
            &mut context,
            Some({
                let renderer_count = Rc::clone(&renderer_count);
                Rc::new(move || {
                    let attempt = renderer_count.get() + 1;
                    renderer_count.set(attempt);
                    if attempt == 1 {
                        panic!("synthetic explicit renderer failure");
                    }
                })
            }),
            {
                let platform_count = Rc::clone(&platform_count);
                Rc::new(move || platform_count.set(platform_count.get() + 1))
            },
        );
        runtime.control.platform_initialized.set(true);
        runtime.control.renderer_initialized.set(true);

        assert!(matches!(
            runtime.shutdown_platform(&mut context),
            Err(Sdl3BackendError::ShutdownPanicked {
                phase: "renderer resources"
            })
        ));
        assert_eq!(renderer_count.get(), 2);
        assert_eq!(platform_count.get(), 1);
        assert_eq!(runtime.control.state(), RuntimeState::Detached);
        runtime.poll_fault().unwrap();
        runtime.shutdown_platform(&mut context).unwrap();
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    #[test]
    fn renderer_shutdown_retries_before_texture_cleanup_after_platform_release() {
        let _guard = crate::tests::test_guard();
        let renderer_count = Rc::new(Cell::new(0));
        let platform_count = Rc::new(Cell::new(0));
        let mut context = Context::create();
        let consumer = context.create_renderer_consumer().unwrap();
        let mut runtime = registration_with_lifecycle(
            &mut context,
            Some({
                let renderer_count = Rc::clone(&renderer_count);
                Rc::new(move || {
                    let attempt = renderer_count.get() + 1;
                    renderer_count.set(attempt);
                    if attempt == 1 {
                        panic!("synthetic composite renderer failure");
                    }
                })
            }),
            {
                let platform_count = Rc::clone(&platform_count);
                Rc::new(move || platform_count.set(platform_count.get() + 1))
            },
        );
        runtime.control.platform_initialized.set(true);
        runtime.control.renderer_initialized.set(true);
        runtime.control.install_renderer_consumer(consumer);

        let result = runtime.shutdown_renderer(&mut context);

        assert!(matches!(
            result,
            Err(Sdl3BackendError::ShutdownPanicked {
                phase: "renderer resources"
            })
        ));
        assert_eq!(renderer_count.get(), 2);
        assert_eq!(platform_count.get(), 1);
        assert_eq!(runtime.control.state(), RuntimeState::Detached);
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    #[test]
    fn device_object_destroy_validates_texture_reset_before_native_destruction() {
        let _guard = crate::tests::test_guard();
        let native_destroy_count = Rc::new(Cell::new(0));
        let mut context = Context::create();
        context.io_mut().set_display_size([128.0, 128.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .io_mut()
            .set_backend_flags(dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES);
        let consumer = context.create_renderer_consumer().unwrap();
        let mut runtime = registration_with_lifecycle(&mut context, None, Rc::new(|| {}));
        let snapshot = context.begin_frame().render_snapshot(&consumer).unwrap();
        runtime.control.install_renderer_consumer(consumer);

        let result = runtime.destroy_renderer_device_objects(&mut context, {
            let native_destroy_count = Rc::clone(&native_destroy_count);
            move || native_destroy_count.set(native_destroy_count.get() + 1)
        });

        assert!(matches!(
            result,
            Err(Sdl3BackendError::RendererConsumer(
                dear_imgui_rs::render::RendererConsumerError::OutstandingEpochs { count: 1 }
            ))
        ));
        assert_eq!(native_destroy_count.get(), 0);
        assert_eq!(runtime.control.state(), RuntimeState::Attached);

        drop(snapshot);
        runtime
            .destroy_renderer_device_objects(&mut context, {
                let native_destroy_count = Rc::clone(&native_destroy_count);
                move || native_destroy_count.set(native_destroy_count.get() + 1)
            })
            .unwrap();
        assert_eq!(native_destroy_count.get(), 1);

        runtime.shutdown_platform(&mut context).unwrap();
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    #[test]
    fn renderer_shutdown_validates_texture_reset_before_native_teardown() {
        let _guard = crate::tests::test_guard();
        let renderer_count = Rc::new(Cell::new(0));
        let platform_count = Rc::new(Cell::new(0));
        let mut context = Context::create();
        context.io_mut().set_display_size([128.0, 128.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .io_mut()
            .set_backend_flags(dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES);
        let consumer = context.create_renderer_consumer().unwrap();
        let mut runtime = registration_with_lifecycle(
            &mut context,
            Some({
                let renderer_count = Rc::clone(&renderer_count);
                Rc::new(move || renderer_count.set(renderer_count.get() + 1))
            }),
            {
                let platform_count = Rc::clone(&platform_count);
                Rc::new(move || platform_count.set(platform_count.get() + 1))
            },
        );
        runtime.control.platform_initialized.set(true);
        runtime.control.renderer_initialized.set(true);
        let snapshot = context.begin_frame().render_snapshot(&consumer).unwrap();
        runtime.control.install_renderer_consumer(consumer);

        let result = runtime.shutdown_renderer(&mut context);

        assert!(matches!(
            result,
            Err(Sdl3BackendError::RendererConsumer(
                dear_imgui_rs::render::RendererConsumerError::OutstandingEpochs { count: 1 }
            ))
        ));
        assert_eq!(renderer_count.get(), 0);
        assert_eq!(platform_count.get(), 0);
        assert_eq!(runtime.control.state(), RuntimeState::Attached);
        assert_eq!(
            runtime.control.platform_release.get(),
            ReleaseState::Pending
        );
        assert_eq!(
            runtime.control.renderer_release.get(),
            ReleaseState::Pending
        );
        assert!(runtime.platform_attachment.is_some());
        assert!(runtime.renderer_attachment.is_some());

        drop(snapshot);
        runtime.shutdown_renderer(&mut context).unwrap();
        assert_eq!(renderer_count.get(), 1);
        assert_eq!(platform_count.get(), 1);
        assert_eq!(runtime.control.state(), RuntimeState::Detached);
    }

    #[test]
    fn attachment_teardown_reports_renderer_panic_without_hidden_retry() {
        let _guard = crate::tests::test_guard();
        let renderer_count = Rc::new(Cell::new(0));
        let platform_count = Rc::new(Cell::new(0));
        let mut context = Context::create();
        let runtime = registration_with_lifecycle(
            &mut context,
            Some({
                let renderer_count = Rc::clone(&renderer_count);
                Rc::new(move || {
                    let attempt = renderer_count.get() + 1;
                    renderer_count.set(attempt);
                    if attempt == 1 {
                        panic!("synthetic renderer teardown failure");
                    }
                })
            }),
            {
                let platform_count = Rc::clone(&platform_count);
                Rc::new(move || platform_count.set(platform_count.get() + 1))
            },
        );
        runtime.control.platform_initialized.set(true);
        runtime.control.renderer_initialized.set(true);
        let control = Rc::clone(&runtime.control);

        control.begin_shutdown();
        let first = context
            .binding()
            .with_bound_context(|| control.shutdown_bound_for_attachment());
        assert!(matches!(
            first,
            Err(Sdl3BackendError::ShutdownPanicked {
                phase: "renderer resources"
            })
        ));
        assert_eq!(renderer_count.get(), 1);
        assert_eq!(platform_count.get(), 1);
        assert_eq!(control.phase_log(), ["platform", "renderer"]);
        assert_eq!(control.state(), RuntimeState::ShuttingDown);

        context
            .binding()
            .with_bound_context(|| control.shutdown_bound_for_attachment())
            .unwrap();
        assert_eq!(renderer_count.get(), 2);
        assert_eq!(control.phase_log(), ["platform", "renderer", "renderer"]);
        assert_eq!(control.state(), RuntimeState::Detached);
        drop(context);
        drop(runtime);
        assert_eq!(control.state(), RuntimeState::ResourceDropped);
    }

    #[test]
    fn platform_phase_does_not_drop_renderer_global_state() {
        let _guard = crate::tests::test_guard();
        let renderer_count = Rc::new(Cell::new(0));
        let platform_count = Rc::new(Cell::new(0));
        let mut context = Context::create();
        let runtime = test_registration(
            &mut context,
            Rc::clone(&renderer_count),
            Rc::clone(&platform_count),
        );

        runtime.control.begin_shutdown();
        context
            .binding()
            .with_bound_context(|| runtime.control.release_platform_bound().unwrap());

        assert_eq!(runtime.control.phase_log(), ["platform"]);
        assert_eq!(renderer_count.get(), 0);
        assert_eq!(platform_count.get(), 1);

        let _ = context
            .binding()
            .with_bound_context(|| runtime.control.release_renderer_bound());
        assert_eq!(runtime.control.phase_log(), ["platform", "renderer"]);
        assert_eq!(renderer_count.get(), 1);
    }

    #[test]
    fn owned_callback_state_restores_baseline_without_dangling_native_data() {
        let _guard = crate::tests::test_guard();
        let mut context = Context::create();
        let baseline_clipboard = context.binding().with_bound_context(|| unsafe {
            let platform_io = sys::igGetPlatformIO_Nil();
            (
                (*platform_io).Platform_GetClipboardTextFn,
                (*platform_io).Platform_ClipboardUserData,
            )
        });
        let platform_count = Rc::new(Cell::new(0));
        let observed_backend_data = Rc::new(Cell::new(0));
        let observed_main_viewport_data = Rc::new(Cell::new(0));
        let mut runtime = synthetic_claimed_registration(
            &mut context,
            Rc::clone(&platform_count),
            Rc::clone(&observed_backend_data),
            Rc::clone(&observed_main_viewport_data),
            synthetic_create_window,
        );
        let registry_key = runtime.control.platform_io_key.get();
        assert!(registry_contains(registry_key));

        runtime.shutdown_platform(&mut context).unwrap();

        assert_eq!(platform_count.get(), 1);
        assert_eq!(observed_backend_data.get(), OWNED_BACKEND_DATA);
        assert_eq!(observed_main_viewport_data.get(), OWNED_VIEWPORT_DATA);
        assert_eq!(runtime.control.platform_io_key.get(), 0);
        assert!(!registry_contains(registry_key));
        assert!(with_current_runtime(|_| ()).is_none());
        context.binding().with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            let main_viewport = sys::igGetMainViewport();
            assert!((*io).BackendPlatformUserData.is_null());
            assert!((*io).BackendPlatformName.is_null());
            assert!((*platform_io).Platform_CreateWindow.is_none());
            assert_eq!(
                (*platform_io).Platform_ClipboardUserData,
                baseline_clipboard.1
            );
            match (
                (*platform_io).Platform_GetClipboardTextFn,
                baseline_clipboard.0,
            ) {
                (Some(actual), Some(expected)) => assert!(std::ptr::fn_addr_eq(actual, expected)),
                (None, None) => {}
                _ => panic!("clipboard callback baseline was not restored"),
            }
            assert!((*main_viewport).PlatformUserData.is_null());
            assert!((*main_viewport).PlatformHandle.is_null());
        });
    }

    #[test]
    fn platform_service_override_does_not_revoke_viewport_ownership() {
        let _guard = crate::tests::test_guard();
        let mut context = Context::create();
        let baseline_clipboard = context.binding().with_bound_context(|| unsafe {
            let platform_io = sys::igGetPlatformIO_Nil();
            (
                (*platform_io).Platform_GetClipboardTextFn,
                (*platform_io).Platform_SetClipboardTextFn,
                (*platform_io).Platform_ClipboardUserData,
            )
        });
        let mut runtime = synthetic_claimed_registration(
            &mut context,
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            synthetic_create_window,
        );

        context.binding().with_bound_context(|| unsafe {
            let platform_io = sys::igGetPlatformIO_Nil();
            (*platform_io).Platform_GetClipboardTextFn = Some(foreign_get_clipboard_text);
            (*platform_io).Platform_SetClipboardTextFn = Some(foreign_set_clipboard_text);
            (*platform_io).Platform_ClipboardUserData = FOREIGN_PLATFORM_DATA as *mut _;
        });

        let mut viewport = sys::ImGuiViewport::default();
        context.binding().with_bound_context(|| unsafe {
            create_window_callback_for_test(&mut viewport);
            destroy_window_callback_for_test(&mut viewport);
        });
        runtime.poll_fault().unwrap();
        runtime.shutdown_platform(&mut context).unwrap();

        context.binding().with_bound_context(|| unsafe {
            let platform_io = sys::igGetPlatformIO_Nil();
            assert!(std::ptr::fn_addr_eq(
                (*platform_io).Platform_GetClipboardTextFn.unwrap(),
                foreign_get_clipboard_text
                    as unsafe extern "C" fn(*mut sys::ImGuiContext) -> *const std::ffi::c_char,
            ));
            assert!(std::ptr::fn_addr_eq(
                (*platform_io).Platform_SetClipboardTextFn.unwrap(),
                foreign_set_clipboard_text
                    as unsafe extern "C" fn(*mut sys::ImGuiContext, *const std::ffi::c_char),
            ));
            assert_eq!(
                (*platform_io).Platform_ClipboardUserData as usize,
                FOREIGN_PLATFORM_DATA
            );

            (*platform_io).Platform_GetClipboardTextFn = baseline_clipboard.0;
            (*platform_io).Platform_SetClipboardTextFn = baseline_clipboard.1;
            (*platform_io).Platform_ClipboardUserData = baseline_clipboard.2;
        });
    }

    #[test]
    fn context_first_claimed_runtime_unregisters_callback_registry() {
        let _guard = crate::tests::test_guard();
        let mut context = Context::create();
        let platform_count = Rc::new(Cell::new(0));
        let observed_backend_data = Rc::new(Cell::new(0));
        let observed_main_viewport_data = Rc::new(Cell::new(0));
        let runtime = synthetic_claimed_registration(
            &mut context,
            Rc::clone(&platform_count),
            Rc::clone(&observed_backend_data),
            Rc::clone(&observed_main_viewport_data),
            synthetic_create_window,
        );
        let control = Rc::clone(&runtime.control);
        let registry_key = control.platform_io_key.get();

        drop(context);

        assert_eq!(platform_count.get(), 1);
        assert_eq!(observed_backend_data.get(), OWNED_BACKEND_DATA);
        assert_eq!(observed_main_viewport_data.get(), OWNED_VIEWPORT_DATA);
        assert!(!registry_contains(registry_key));
        assert_eq!(control.platform_io_key.get(), 0);
        assert_eq!(control.state(), RuntimeState::Detached);
        drop(runtime);
        assert_eq!(control.state(), RuntimeState::ResourceDropped);
    }

    #[test]
    fn failed_viewport_creation_is_reported_on_next_rust_entry() {
        let _guard = crate::tests::test_guard();
        let mut context = Context::create();
        let platform_count = Rc::new(Cell::new(0));
        let observed_backend_data = Rc::new(Cell::new(0));
        let observed_main_viewport_data = Rc::new(Cell::new(0));
        let mut runtime = synthetic_claimed_registration(
            &mut context,
            Rc::clone(&platform_count),
            observed_backend_data,
            observed_main_viewport_data,
            failing_create_window,
        );
        let mut viewport = sys::ImGuiViewport::default();

        context
            .binding()
            .with_bound_context(|| unsafe { create_window_callback_for_test(&mut viewport) });

        assert!(viewport.PlatformRequestClose);
        assert!(matches!(
            runtime.poll_fault(),
            Err(Sdl3BackendError::ViewportCreationFailed)
        ));
        runtime.shutdown_platform(&mut context).unwrap();
        assert_eq!(platform_count.get(), 1);
    }

    #[test]
    fn failed_viewport_opengl_context_is_reported_on_next_rust_entry() {
        let _guard = crate::tests::test_guard();
        let mut context = Context::create();
        let runtime = registration_with_lifecycle(&mut context, None, Rc::new(|| {}));

        runtime
            .control
            .record_viewport_opengl_context_failed_for_test();

        assert!(matches!(
            runtime.poll_fault(),
            Err(Sdl3BackendError::ViewportOpenGlContextFailed)
        ));
    }

    #[test]
    fn foreign_callback_and_user_data_replacements_survive_shutdown() {
        let _guard = crate::tests::test_guard();
        let mut context = Context::create();
        let baseline_clipboard_user_data = context.binding().with_bound_context(|| unsafe {
            (*sys::igGetPlatformIO_Nil()).Platform_ClipboardUserData
        });
        let platform_count = Rc::new(Cell::new(0));
        let observed_backend_data = Rc::new(Cell::new(0));
        let observed_main_viewport_data = Rc::new(Cell::new(0));
        let mut runtime = synthetic_claimed_registration(
            &mut context,
            Rc::clone(&platform_count),
            Rc::clone(&observed_backend_data),
            Rc::clone(&observed_main_viewport_data),
            synthetic_create_window,
        );
        let registry_key = runtime.control.platform_io_key.get();

        context.binding().with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            let main_viewport = sys::igGetMainViewport();
            (*io).BackendPlatformUserData = FOREIGN_BACKEND_DATA as *mut _;
            (*io).BackendPlatformName = FOREIGN_BACKEND_NAME.as_ptr().cast();
            (*platform_io).Platform_CreateWindow = Some(foreign_create_window);
            (*platform_io).Platform_ClipboardUserData = FOREIGN_PLATFORM_DATA as *mut _;
            (*main_viewport).PlatformUserData = FOREIGN_VIEWPORT_DATA as *mut _;
            (*main_viewport).PlatformHandle = FOREIGN_VIEWPORT_HANDLE as *mut _;
        });

        assert!(matches!(
            runtime.shutdown_platform(&mut context),
            Err(Sdl3BackendError::PlatformCallbackReplaced {
                callback: "Platform_CreateWindow"
            })
        ));

        assert_eq!(platform_count.get(), 1);
        assert_eq!(observed_backend_data.get(), OWNED_BACKEND_DATA);
        assert_eq!(observed_main_viewport_data.get(), OWNED_VIEWPORT_DATA);
        assert!(!registry_contains(registry_key));
        context.binding().with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            let main_viewport = sys::igGetMainViewport();
            assert_eq!((*io).BackendPlatformUserData as usize, FOREIGN_BACKEND_DATA);
            assert_eq!(
                (*io).BackendPlatformName,
                FOREIGN_BACKEND_NAME.as_ptr().cast()
            );
            assert_eq!(
                (*platform_io).Platform_ClipboardUserData as usize,
                FOREIGN_PLATFORM_DATA
            );
            assert!(std::ptr::fn_addr_eq(
                (*platform_io).Platform_CreateWindow.unwrap(),
                foreign_create_window as unsafe extern "C" fn(*mut sys::ImGuiViewport)
            ));
            assert_eq!(
                (*main_viewport).PlatformUserData as usize,
                FOREIGN_VIEWPORT_DATA
            );
            assert_eq!(
                (*main_viewport).PlatformHandle as usize,
                FOREIGN_VIEWPORT_HANDLE
            );
        });
        assert!(matches!(
            runtime.poll_fault(),
            Err(Sdl3BackendError::PlatformStateReplaced {
                field: "Platform_ClipboardUserData"
            })
        ));
        assert!(matches!(
            runtime.poll_fault(),
            Err(Sdl3BackendError::PlatformStateReplaced {
                field: "BackendPlatformUserData"
            })
        ));
        assert!(matches!(
            runtime.poll_fault(),
            Err(Sdl3BackendError::PlatformStateReplaced {
                field: "BackendPlatformName"
            })
        ));
        let fault = runtime.poll_fault();
        assert!(
            matches!(fault, Err(Sdl3BackendError::ForeignPlatformUserData)),
            "unexpected fourth platform ownership fault: {fault:?}"
        );
        assert!(matches!(
            runtime.poll_fault(),
            Err(Sdl3BackendError::PlatformStateReplaced {
                field: "MainViewport.PlatformHandle"
            })
        ));
        runtime.poll_fault().unwrap();

        // The synthetic foreign owner now performs its own shutdown before the
        // Context is dropped.
        context.binding().with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            let main_viewport = sys::igGetMainViewport();
            (*io).BackendPlatformUserData = std::ptr::null_mut();
            (*io).BackendPlatformName = std::ptr::null();
            (*platform_io).Platform_CreateWindow = None;
            (*platform_io).Platform_ClipboardUserData = baseline_clipboard_user_data;
            (*main_viewport).PlatformUserData = std::ptr::null_mut();
            (*main_viewport).PlatformHandle = std::ptr::null_mut();
        });
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    #[test]
    fn callback_only_renderer_drift_revokes_reserved_capabilities_without_erasing_foreign_callback()
    {
        let _guard = crate::tests::test_guard();
        let mut context = Context::create();
        let mut registration = synthetic_renderer_registration(&mut context);

        context.binding().with_bound_context(|| unsafe {
            (*sys::igGetPlatformIO_Nil()).Renderer_RenderWindow =
                Some(foreign_renderer_render_window);
        });

        assert!(matches!(
            registration.poll_fault(),
            Err(Sdl3BackendError::RendererCallbackReplaced {
                callback: "Renderer_RenderWindow"
            })
        ));
        context.binding().with_bound_context(|| unsafe {
            assert_eq!(
                (*sys::igGetIO_Nil()).BackendFlags & SDL_RENDERER_RESERVED_FLAGS,
                0,
                "a partial callback replacement must revoke SDL renderer capabilities"
            );
        });

        let shutdown = registration.shutdown_platform(&mut context);
        assert!(matches!(
            shutdown,
            Ok(()) | Err(Sdl3BackendError::RendererCallbackReplaced { .. })
        ));
        context.binding().with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            assert_eq!((*io).BackendFlags & SDL_RENDERER_RESERVED_FLAGS, 0);
            assert!((*io).BackendRendererUserData.is_null());
            assert!((*io).BackendRendererName.is_null());
            assert!(std::ptr::fn_addr_eq(
                (*platform_io).Renderer_RenderWindow.unwrap(),
                foreign_renderer_render_window
                    as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut std::ffi::c_void),
            ));

            sys::ImGuiPlatformIO_ClearRendererHandlers(platform_io);
        });
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    #[test]
    fn core_identity_only_renderer_drift_revokes_reserved_capabilities_without_erasing_foreign_identity()
     {
        let _guard = crate::tests::test_guard();
        let mut context = Context::create();
        let mut registration = synthetic_renderer_registration(&mut context);

        context.binding().with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            (*io).BackendRendererUserData = FOREIGN_BACKEND_DATA as *mut _;
            (*io).BackendRendererName = FOREIGN_BACKEND_NAME.as_ptr().cast();
        });

        assert!(matches!(
            registration.poll_fault(),
            Err(Sdl3BackendError::RendererStateReplaced {
                field: "BackendRendererUserData"
            })
        ));
        context.binding().with_bound_context(|| unsafe {
            assert_eq!(
                (*sys::igGetIO_Nil()).BackendFlags & SDL_RENDERER_RESERVED_FLAGS,
                0,
                "foreign core identity alone is not a complete renderer takeover"
            );
        });

        let shutdown = registration.shutdown_platform(&mut context);
        assert!(matches!(
            shutdown,
            Ok(()) | Err(Sdl3BackendError::RendererStateReplaced { .. })
        ));
        context.binding().with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            assert_eq!((*io).BackendFlags & SDL_RENDERER_RESERVED_FLAGS, 0);
            assert_eq!((*io).BackendRendererUserData as usize, FOREIGN_BACKEND_DATA);
            assert_eq!(
                (*io).BackendRendererName,
                FOREIGN_BACKEND_NAME.as_ptr().cast()
            );

            (*io).BackendRendererUserData = std::ptr::null_mut();
            (*io).BackendRendererName = std::ptr::null();
        });
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    #[test]
    fn complete_renderer_takeover_preserves_its_capabilities_and_callbacks() {
        let _guard = crate::tests::test_guard();
        let mut context = Context::create();
        let mut registration = synthetic_renderer_registration(&mut context);
        let foreign_flags = context.binding().with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            (*platform_io).Renderer_RenderWindow = Some(foreign_renderer_render_window);
            (*platform_io).Renderer_DestroyWindow = Some(foreign_renderer_destroy_window);
            (*platform_io).Renderer_SetWindowSize = Some(foreign_renderer_set_window_size);
            (*io).BackendRendererUserData = FOREIGN_BACKEND_DATA as *mut _;
            (*io).BackendRendererName = FOREIGN_BACKEND_NAME.as_ptr().cast();
            (*io).BackendFlags & SDL_RENDERER_RESERVED_FLAGS
        });

        assert!(matches!(
            registration.poll_fault(),
            Err(Sdl3BackendError::RendererCallbackReplaced { .. })
        ));
        context.binding().with_bound_context(|| unsafe {
            assert_eq!(
                (*sys::igGetIO_Nil()).BackendFlags & SDL_RENDERER_RESERVED_FLAGS,
                foreign_flags,
                "a complete foreign renderer takeover retains its own capability publication"
            );
        });

        let shutdown = registration.shutdown_platform(&mut context);
        assert!(matches!(
            shutdown,
            Ok(()) | Err(Sdl3BackendError::RendererCallbackReplaced { .. })
        ));
        context.binding().with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            assert_eq!(
                (*io).BackendFlags & SDL_RENDERER_RESERVED_FLAGS,
                foreign_flags
            );
            assert_eq!((*io).BackendRendererUserData as usize, FOREIGN_BACKEND_DATA);
            assert_eq!(
                (*io).BackendRendererName,
                FOREIGN_BACKEND_NAME.as_ptr().cast()
            );
            assert!(std::ptr::fn_addr_eq(
                (*platform_io).Renderer_RenderWindow.unwrap(),
                foreign_renderer_render_window
                    as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut std::ffi::c_void),
            ));
            assert!(std::ptr::fn_addr_eq(
                (*platform_io).Renderer_DestroyWindow.unwrap(),
                foreign_renderer_destroy_window as unsafe extern "C" fn(*mut sys::ImGuiViewport),
            ));
            assert!(std::ptr::fn_addr_eq(
                (*platform_io).Renderer_SetWindowSize.unwrap(),
                foreign_renderer_set_window_size
                    as unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2_c),
            ));

            sys::ImGuiPlatformIO_ClearRendererHandlers(platform_io);
            (*io).BackendRendererUserData = std::ptr::null_mut();
            (*io).BackendRendererName = std::ptr::null();
            (*io).BackendFlags &= !SDL_RENDERER_RESERVED_FLAGS;
        });
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    #[test]
    fn native_renderer_shutdown_preserves_foreign_callback_and_backend_replacements() {
        let _guard = crate::tests::test_guard();
        RENDERER_RENDER_COUNT.with(|count| count.set(0));
        RENDERER_SET_SIZE_COUNT.with(|count| count.set(0));
        OWNED_RENDERER_DESTROY_COUNT.with(|count| count.set(0));
        FOREIGN_RENDERER_DESTROY_COUNT.with(|count| count.set(0));
        RENDERER_DESTROY_OBSERVED_USER_DATA.with(|observed| observed.set(0));
        let mut context = Context::create();
        let renderer_shutdown_count = Rc::new(Cell::new(0));
        let renderer_observed_owned_state = Rc::new(Cell::new(false));

        #[cfg(feature = "opengl3-renderer")]
        let native_renderer = NativeRendererKind::OpenGl3;
        #[cfg(all(not(feature = "opengl3-renderer"), feature = "sdlrenderer3-renderer"))]
        let native_renderer = NativeRendererKind::SdlRenderer3;
        #[cfg(all(
            not(feature = "opengl3-renderer"),
            not(feature = "sdlrenderer3-renderer"),
            feature = "sdlgpu3-renderer"
        ))]
        let native_renderer = NativeRendererKind::SdlGpu3;

        let mut registration = registration_with_backend_lifecycle(
            &mut context,
            Some({
                let count = Rc::clone(&renderer_shutdown_count);
                let observed = Rc::clone(&renderer_observed_owned_state);
                Rc::new(move || unsafe {
                    count.set(count.get() + 1);
                    let io = sys::igGetIO_Nil();
                    let platform_io = sys::igGetPlatformIO_Nil();
                    let callback_is_owned =
                        (*platform_io)
                            .Renderer_RenderWindow
                            .is_some_and(|callback| {
                                std::ptr::fn_addr_eq(
                                    callback,
                                    synthetic_renderer_render_window
                                        as unsafe extern "C" fn(
                                            *mut sys::ImGuiViewport,
                                            *mut std::ffi::c_void,
                                        ),
                                )
                            });
                    observed.set(
                        callback_is_owned
                            && (*io).BackendRendererUserData as usize == OWNED_BACKEND_DATA
                            && (*io).BackendFlags
                                & sys::ImGuiBackendFlags_RendererHasViewports as i32
                                != 0,
                    );
                    sys::ImGuiPlatformIO_ClearRendererHandlers(platform_io);
                    (*io).BackendRendererUserData = std::ptr::null_mut();
                    (*io).BackendRendererName = std::ptr::null();
                })
            }),
            Rc::new(|| unsafe {
                let io = sys::igGetIO_Nil();
                let platform_io = sys::igGetPlatformIO_Nil();
                let mut viewport = sys::ImGuiViewport::default();
                viewport.PlatformUserData = OWNED_VIEWPORT_DATA as *mut _;
                viewport.PlatformHandle = OWNED_VIEWPORT_HANDLE as *mut _;
                viewport.RendererUserData = FOREIGN_BACKEND_DATA as *mut _;
                let _ = with_current_runtime(|control| {
                    control.remember_owned_viewport(
                        &mut viewport,
                        ViewportPlatformState::capture(&viewport),
                    );
                    control.remember_owned_renderer_viewport(
                        &mut viewport,
                        OWNED_BACKEND_DATA as *mut _,
                    );
                });
                (*platform_io)
                    .Renderer_DestroyWindow
                    .expect("renderer destroy wrapper must remain installed")(
                    &mut viewport
                );
                sys::ImGuiPlatformIO_ClearPlatformHandlers(platform_io);
                (*io).BackendFlags &= !(sys::ImGuiBackendFlags_HasMouseCursors as i32);
            }),
            PlatformGraphicsKind::Other,
            native_renderer,
        );

        context.binding().with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            (*platform_io).Platform_CreateWindow = Some(synthetic_create_window);
            (*platform_io).Platform_DestroyWindow = Some(synthetic_destroy_window);
            (*platform_io).Renderer_RenderWindow = Some(synthetic_renderer_render_window);
            (*platform_io).Renderer_DestroyWindow = Some(synthetic_renderer_destroy_window);
            (*platform_io).Renderer_SetWindowSize = Some(synthetic_renderer_set_window_size);
            (*io).BackendPlatformUserData = OWNED_BACKEND_DATA as *mut _;
            (*io).BackendPlatformName = OWNED_BACKEND_NAME.as_ptr().cast();
            (*io).BackendRendererUserData = OWNED_BACKEND_DATA as *mut _;
            (*io).BackendRendererName = OWNED_BACKEND_NAME.as_ptr().cast();
            (*io).BackendFlags |= sys::ImGuiBackendFlags_RendererHasViewports as i32
                | sys::ImGuiBackendFlags_HasMouseCursors as i32;
        });

        let baseline = registration.baseline.take().unwrap();
        let renderer_baseline = baseline.snapshot();
        let (platform, renderer) = context.binding().with_bound_context(|| unsafe {
            (
                PlatformCallbackOwnership::claim(&registration.control, baseline).unwrap(),
                RendererCallbackOwnership::claim(&registration.control, &renderer_baseline)
                    .unwrap()
                    .unwrap(),
            )
        });
        registration
            .control
            .callbacks
            .borrow_mut()
            .replace(platform);
        registration
            .control
            .renderer_callbacks
            .borrow_mut()
            .replace(renderer);
        registration.control.platform_initialized.set(true);
        registration.control.renderer_initialized.set(true);

        let mut owned_viewport = sys::ImGuiViewport::default();
        owned_viewport.PlatformUserData = OWNED_VIEWPORT_DATA as *mut _;
        owned_viewport.PlatformHandle = OWNED_VIEWPORT_HANDLE as *mut _;
        owned_viewport.RendererUserData = OWNED_BACKEND_DATA as *mut _;
        registration
            .control
            .remember_owned_viewport(&mut owned_viewport, unsafe {
                ViewportPlatformState::capture(&owned_viewport)
            });
        registration
            .control
            .remember_owned_renderer_viewport(&mut owned_viewport, owned_viewport.RendererUserData);
        context.binding().with_bound_context(|| unsafe {
            renderer_set_window_size_callback_for_test(
                &mut owned_viewport,
                sys::ImVec2_c { x: 320.0, y: 240.0 },
            )
        });
        assert_eq!(RENDERER_SET_SIZE_COUNT.with(Cell::get), 1);

        context.binding().with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            (*platform_io).Renderer_RenderWindow = Some(foreign_renderer_render_window);
            (*platform_io).Renderer_DestroyWindow = Some(foreign_renderer_destroy_window);
            (*platform_io).Renderer_SetWindowSize = Some(foreign_renderer_set_window_size);
            (*platform_io).Renderer_SwapBuffers = Some(foreign_renderer_render_window);
            (*io).BackendRendererUserData = FOREIGN_BACKEND_DATA as *mut _;
            (*io).BackendRendererName = FOREIGN_BACKEND_NAME.as_ptr().cast();
        });

        let mut viewport = sys::ImGuiViewport::default();
        context.binding().with_bound_context(|| unsafe {
            renderer_render_window_callback_for_test(&mut viewport)
        });
        assert_eq!(RENDERER_RENDER_COUNT.with(Cell::get), 0);
        context.binding().with_bound_context(|| unsafe {
            let flags = (*sys::igGetIO_Nil()).BackendFlags;
            assert_ne!(
                flags & sys::ImGuiBackendFlags_RendererHasViewports as i32,
                0,
                "foreign renderer takeover must retain its published capability"
            );
            assert_ne!(flags & sys::ImGuiBackendFlags_HasMouseCursors as i32, 0);
        });

        assert!(matches!(
            registration.shutdown_platform(&mut context),
            Err(Sdl3BackendError::RendererCallbackReplaced {
                callback: "Renderer_DestroyWindow"
            })
        ));

        assert_eq!(renderer_shutdown_count.get(), 1);
        assert!(renderer_observed_owned_state.get());
        assert_eq!(OWNED_RENDERER_DESTROY_COUNT.with(Cell::get), 1);
        assert_eq!(FOREIGN_RENDERER_DESTROY_COUNT.with(Cell::get), 0);
        assert_eq!(
            RENDERER_DESTROY_OBSERVED_USER_DATA.with(Cell::get),
            OWNED_BACKEND_DATA
        );
        context.binding().with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            assert!(std::ptr::fn_addr_eq(
                (*platform_io).Renderer_RenderWindow.unwrap(),
                foreign_renderer_render_window
                    as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut std::ffi::c_void),
            ));
            assert!(std::ptr::fn_addr_eq(
                (*platform_io).Renderer_DestroyWindow.unwrap(),
                foreign_renderer_destroy_window as unsafe extern "C" fn(*mut sys::ImGuiViewport),
            ));
            assert!(std::ptr::fn_addr_eq(
                (*platform_io).Renderer_SwapBuffers.unwrap(),
                foreign_renderer_render_window
                    as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut std::ffi::c_void),
            ));
            assert_eq!((*io).BackendRendererUserData as usize, FOREIGN_BACKEND_DATA);
            assert_eq!(
                (*io).BackendRendererName,
                FOREIGN_BACKEND_NAME.as_ptr().cast()
            );
            assert_eq!(
                (*io).BackendFlags & sys::ImGuiBackendFlags_HasMouseCursors as i32,
                0,
                "renderer shutdown must not resurrect a platform bit cleared by native shutdown"
            );
            assert_ne!(
                (*io).BackendFlags & sys::ImGuiBackendFlags_RendererHasViewports as i32,
                0,
                "renderer shutdown must restore a foreign renderer capability snapshot"
            );
        });
        context.binding().with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            sys::ImGuiPlatformIO_ClearRendererHandlers(platform_io);
            (*io).BackendRendererUserData = std::ptr::null_mut();
            (*io).BackendRendererName = std::ptr::null();
            (*io).BackendFlags &= !SDL_RENDERER_RESERVED_FLAGS;
        });
        let mut faults = Vec::new();
        while let Err(fault) = registration.poll_fault() {
            faults.push(fault);
        }
        assert!(faults.iter().any(|fault| {
            matches!(
                fault,
                Sdl3BackendError::RendererCallbackReplaced {
                    callback: "Renderer_RenderWindow"
                }
            )
        }));
        assert!(faults.iter().any(|fault| {
            matches!(
                fault,
                Sdl3BackendError::RendererCallbackReplaced {
                    callback: "Renderer_SwapBuffers"
                }
            )
        }));
        assert!(faults.iter().any(|fault| {
            matches!(
                fault,
                Sdl3BackendError::RendererStateReplaced {
                    field: "BackendRendererUserData"
                }
            )
        }));
        assert!(faults.iter().any(|fault| {
            matches!(
                fault,
                Sdl3BackendError::RendererStateReplaced {
                    field: "BackendRendererName"
                }
            )
        }));
        assert!(!faults.iter().any(|fault| {
            matches!(
                fault,
                Sdl3BackendError::RendererStateReplaced {
                    field: "BackendFlags(renderer-owned bits)"
                }
            )
        }));
    }

    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    #[cfg(debug_assertions)]
    #[test]
    fn real_imgui_destroy_platform_windows_restores_foreign_state_after_all_callbacks() {
        let _guard = crate::tests::test_guard();
        let mut context = Context::create();
        let viewport_pointer = Rc::new(Cell::new(0_usize));

        #[cfg(feature = "opengl3-renderer")]
        let native_renderer = NativeRendererKind::OpenGl3;
        #[cfg(all(not(feature = "opengl3-renderer"), feature = "sdlrenderer3-renderer"))]
        let native_renderer = NativeRendererKind::SdlRenderer3;
        #[cfg(all(
            not(feature = "opengl3-renderer"),
            not(feature = "sdlrenderer3-renderer"),
            feature = "sdlgpu3-renderer"
        ))]
        let native_renderer = NativeRendererKind::SdlGpu3;

        let mut runtime = registration_with_backend_lifecycle(
            &mut context,
            Some(Rc::new(|| unsafe {
                let io = sys::igGetIO_Nil();
                let platform_io = sys::igGetPlatformIO_Nil();
                sys::ImGuiPlatformIO_ClearRendererHandlers(platform_io);
                (*io).BackendRendererUserData = std::ptr::null_mut();
                (*io).BackendRendererName = std::ptr::null();
            })),
            {
                let viewport_pointer = Rc::clone(&viewport_pointer);
                Rc::new(move || unsafe {
                    crate::core::ffi::dear_imgui_sdl3_destroy_platform_windows_for_test(
                        viewport_pointer.get() as *mut sys::ImGuiViewportP,
                    );
                    sys::ImGuiPlatformIO_ClearPlatformHandlers(sys::igGetPlatformIO_Nil());
                })
            },
            PlatformGraphicsKind::Other,
            native_renderer,
        );
        context.binding().with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            let main_viewport = sys::igGetMainViewport();
            (*platform_io).Platform_DestroyWindow = Some(synthetic_destroy_window);
            (*platform_io).Renderer_DestroyWindow = Some(synthetic_renderer_destroy_window);
            (*io).BackendPlatformUserData = OWNED_BACKEND_DATA as *mut _;
            (*io).BackendPlatformName = OWNED_BACKEND_NAME.as_ptr().cast();
            (*io).BackendRendererUserData = OWNED_BACKEND_DATA as *mut _;
            (*io).BackendRendererName = OWNED_BACKEND_NAME.as_ptr().cast();
            (*main_viewport).PlatformUserData = OWNED_VIEWPORT_DATA as *mut _;
            (*main_viewport).PlatformHandle = OWNED_VIEWPORT_HANDLE as *mut _;
            (*main_viewport).PlatformHandleRaw = OWNED_VIEWPORT_HANDLE as *mut _;
            (*main_viewport).PlatformWindowCreated = true;
        });
        let baseline = runtime.baseline.take().unwrap();
        let renderer_baseline = baseline.snapshot();
        let (platform, renderer) = context.binding().with_bound_context(|| unsafe {
            (
                PlatformCallbackOwnership::claim(&runtime.control, baseline).unwrap(),
                RendererCallbackOwnership::claim(&runtime.control, &renderer_baseline)
                    .unwrap()
                    .unwrap(),
            )
        });
        runtime.control.callbacks.borrow_mut().replace(platform);
        runtime
            .control
            .renderer_callbacks
            .borrow_mut()
            .replace(renderer);
        runtime.control.platform_initialized.set(true);
        runtime.control.renderer_initialized.set(true);
        context.binding().with_bound_context(|| unsafe {
            let main_viewport = sys::igGetMainViewport();
            (*main_viewport).PlatformUserData = FOREIGN_VIEWPORT_DATA as *mut _;
            (*main_viewport).PlatformHandle = FOREIGN_VIEWPORT_HANDLE as *mut _;
            (*main_viewport).PlatformHandleRaw = FOREIGN_VIEWPORT_HANDLE as *mut _;
            (*main_viewport).RendererUserData = FOREIGN_BACKEND_DATA as *mut _;
        });

        let mut viewport = Box::new(sys::ImGuiViewportP::default());
        let raw = &mut viewport._ImGuiViewport as *mut sys::ImGuiViewport;
        viewport_pointer.set((&mut *viewport) as *mut sys::ImGuiViewportP as usize);
        viewport._ImGuiViewport.PlatformUserData = OWNED_VIEWPORT_DATA as *mut _;
        viewport._ImGuiViewport.PlatformHandle = OWNED_VIEWPORT_HANDLE as *mut _;
        viewport._ImGuiViewport.RendererUserData = OWNED_BACKEND_DATA as *mut _;
        viewport._ImGuiViewport.PlatformWindowCreated = true;
        runtime
            .control
            .remember_owned_viewport(raw, unsafe { ViewportPlatformState::capture(raw) });
        runtime
            .control
            .remember_owned_renderer_viewport(raw, viewport._ImGuiViewport.RendererUserData);
        viewport._ImGuiViewport.PlatformUserData = FOREIGN_VIEWPORT_DATA as *mut _;
        viewport._ImGuiViewport.PlatformHandle = FOREIGN_VIEWPORT_HANDLE as *mut _;
        viewport._ImGuiViewport.RendererUserData = FOREIGN_BACKEND_DATA as *mut _;

        assert!(matches!(
            runtime.shutdown_platform(&mut context),
            Err(Sdl3BackendError::ForeignPlatformUserData)
        ));

        assert!(!viewport._ImGuiViewport.PlatformWindowCreated);
        context.binding().with_bound_context(|| unsafe {
            let main_viewport = sys::igGetMainViewport();
            assert_eq!(
                (*main_viewport).PlatformUserData as usize,
                FOREIGN_VIEWPORT_DATA
            );
            assert_eq!(
                (*main_viewport).PlatformHandle as usize,
                FOREIGN_VIEWPORT_HANDLE
            );
            assert_eq!(
                (*main_viewport).PlatformHandleRaw as usize,
                FOREIGN_VIEWPORT_HANDLE
            );
            assert_eq!(
                (*main_viewport).RendererUserData as usize,
                FOREIGN_BACKEND_DATA
            );
        });
        assert_eq!(
            viewport._ImGuiViewport.PlatformUserData as usize,
            FOREIGN_VIEWPORT_DATA
        );
        assert_eq!(
            viewport._ImGuiViewport.PlatformHandle as usize,
            FOREIGN_VIEWPORT_HANDLE
        );
        assert_eq!(
            viewport._ImGuiViewport.RendererUserData as usize,
            FOREIGN_BACKEND_DATA
        );
        context.binding().with_bound_context(|| unsafe {
            let main_viewport = sys::igGetMainViewport();
            (*main_viewport).PlatformUserData = std::ptr::null_mut();
            (*main_viewport).PlatformHandle = std::ptr::null_mut();
            (*main_viewport).PlatformHandleRaw = std::ptr::null_mut();
            (*main_viewport).RendererUserData = std::ptr::null_mut();
            (*main_viewport).PlatformWindowCreated = false;
        });
        let mut faults = Vec::new();
        while let Err(fault) = runtime.poll_fault() {
            faults.push(fault);
        }
        assert!(faults.iter().any(|fault| {
            matches!(
                fault,
                Sdl3BackendError::PlatformStateReplaced {
                    field: "MainViewport.PlatformHandle"
                }
            )
        }));
        assert!(
            faults
                .iter()
                .any(|fault| { matches!(fault, Sdl3BackendError::ForeignPlatformUserData) })
        );
        assert!(faults.iter().any(|fault| {
            matches!(
                fault,
                Sdl3BackendError::PlatformStateReplaced {
                    field: "Viewport.PlatformHandle"
                }
            )
        }));
        assert!(faults.iter().any(|fault| {
            matches!(
                fault,
                Sdl3BackendError::RendererStateReplaced {
                    field: "Viewport.RendererUserData"
                }
            )
        }));
    }

    #[test]
    fn destroy_callback_frees_owned_data_and_preserves_foreign_replacement() {
        let _guard = crate::tests::test_guard();
        DESTROY_OBSERVED_USER_DATA.with(|observed| observed.set(0));
        let mut context = Context::create();
        let platform_count = Rc::new(Cell::new(0));
        let observed_backend_data = Rc::new(Cell::new(0));
        let observed_main_viewport_data = Rc::new(Cell::new(0));
        let mut runtime = synthetic_claimed_registration(
            &mut context,
            Rc::clone(&platform_count),
            observed_backend_data,
            observed_main_viewport_data,
            synthetic_create_window,
        );
        let mut viewport = sys::ImGuiViewport::default();
        context
            .binding()
            .with_bound_context(|| unsafe { create_window_callback_for_test(&mut viewport) });
        assert_eq!(viewport.PlatformUserData as usize, OWNED_VIEWPORT_DATA);
        assert_eq!(viewport.PlatformHandle as usize, OWNED_VIEWPORT_HANDLE);
        viewport.PlatformUserData = FOREIGN_VIEWPORT_DATA as *mut _;
        viewport.PlatformHandle = FOREIGN_VIEWPORT_HANDLE as *mut _;

        context
            .binding()
            .with_bound_context(|| unsafe { destroy_window_callback_for_test(&mut viewport) });

        assert_eq!(
            DESTROY_OBSERVED_USER_DATA.with(Cell::get),
            OWNED_VIEWPORT_DATA
        );
        assert!(viewport.PlatformUserData.is_null());
        assert!(viewport.PlatformHandle.is_null());
        assert!(matches!(
            runtime.poll_fault(),
            Err(Sdl3BackendError::ForeignPlatformUserData)
        ));
        assert!(matches!(
            runtime.poll_fault(),
            Err(Sdl3BackendError::PlatformStateReplaced {
                field: "Viewport.PlatformHandle"
            })
        ));
        runtime.shutdown_platform(&mut context).unwrap();
        assert_eq!(viewport.PlatformUserData as usize, FOREIGN_VIEWPORT_DATA);
        assert_eq!(viewport.PlatformHandle as usize, FOREIGN_VIEWPORT_HANDLE);
        assert_eq!(platform_count.get(), 1);
    }

    #[test]
    fn platform_render_rejects_foreign_viewport_state_before_native_callback() {
        let _guard = crate::tests::test_guard();
        PLATFORM_RENDER_COUNT.with(|count| count.set(0));
        let mut context = Context::create();
        let platform_count = Rc::new(Cell::new(0));
        let mut runtime = synthetic_claimed_registration(
            &mut context,
            Rc::clone(&platform_count),
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            synthetic_create_window,
        );
        let mut viewport = sys::ImGuiViewport::default();
        context
            .binding()
            .with_bound_context(|| unsafe { create_window_callback_for_test(&mut viewport) });
        viewport.PlatformUserData = FOREIGN_VIEWPORT_DATA as *mut _;

        context
            .binding()
            .with_bound_context(|| unsafe { render_window_callback_for_test(&mut viewport) });

        assert_eq!(PLATFORM_RENDER_COUNT.with(Cell::get), 0);
        assert!(viewport.PlatformRequestClose);
        assert!(viewport.DrawData.is_null());
        assert!(matches!(
            runtime.poll_fault(),
            Err(Sdl3BackendError::ForeignPlatformUserData)
        ));

        context
            .binding()
            .with_bound_context(|| unsafe { destroy_window_callback_for_test(&mut viewport) });
        runtime.shutdown_platform(&mut context).unwrap();
        assert_eq!(platform_count.get(), 1);
    }

    #[test]
    fn platform_swap_rejects_foreign_viewport_state_before_native_callback() {
        let _guard = crate::tests::test_guard();
        PLATFORM_SWAP_COUNT.with(|count| count.set(0));
        let mut context = Context::create();
        let platform_count = Rc::new(Cell::new(0));
        let mut runtime = synthetic_claimed_registration(
            &mut context,
            Rc::clone(&platform_count),
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            synthetic_create_window,
        );
        let mut viewport = sys::ImGuiViewport::default();
        context
            .binding()
            .with_bound_context(|| unsafe { create_window_callback_for_test(&mut viewport) });
        viewport.PlatformHandle = FOREIGN_VIEWPORT_HANDLE as *mut _;

        context
            .binding()
            .with_bound_context(|| unsafe { swap_buffers_callback_for_test(&mut viewport) });

        assert_eq!(PLATFORM_SWAP_COUNT.with(Cell::get), 0);
        assert!(viewport.PlatformRequestClose);
        assert!(matches!(
            runtime.poll_fault(),
            Err(Sdl3BackendError::PlatformStateReplaced {
                field: "Viewport.PlatformHandle"
            })
        ));

        context
            .binding()
            .with_bound_context(|| unsafe { destroy_window_callback_for_test(&mut viewport) });
        runtime.shutdown_platform(&mut context).unwrap();
        assert_eq!(platform_count.get(), 1);
    }

    #[test]
    fn main_viewport_handle_drift_blocks_unrelated_direct_trampoline() {
        let _guard = crate::tests::test_guard();
        let mut context = Context::create();
        let mut runtime = synthetic_claimed_registration(
            &mut context,
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            synthetic_create_window,
        );
        let published_flags = context.binding().with_bound_context(|| unsafe {
            (*sys::igGetIO_Nil()).BackendFlags & SDL_PLATFORM_RESERVED_FLAGS
        });
        context.binding().with_bound_context(|| unsafe {
            (*sys::igGetMainViewport()).PlatformHandle = FOREIGN_VIEWPORT_HANDLE as *mut _;
        });
        let mut secondary = sys::ImGuiViewport::default();

        context
            .binding()
            .with_bound_context(|| unsafe { create_window_callback_for_test(&mut secondary) });

        assert!(secondary.PlatformUserData.is_null());
        assert_eq!(runtime.control.state(), RuntimeState::ShuttingDown);
        context.binding().with_bound_context(|| unsafe {
            assert_eq!(
                (*sys::igGetIO_Nil()).BackendFlags & SDL_PLATFORM_RESERVED_FLAGS,
                0,
                "a partial foreign drift must revoke SDL platform capabilities"
            );
        });
        assert!(matches!(
            runtime.poll_fault(),
            Err(Sdl3BackendError::PlatformStateReplaced {
                field: "MainViewport.PlatformHandle"
            })
        ));

        runtime.shutdown_platform(&mut context).unwrap();
        context.binding().with_bound_context(|| unsafe {
            let main_viewport = sys::igGetMainViewport();
            assert_eq!(
                (*main_viewport).PlatformHandle as usize,
                FOREIGN_VIEWPORT_HANDLE
            );
            (*main_viewport).PlatformHandle = std::ptr::null_mut();
            assert_eq!(
                (*sys::igGetIO_Nil()).BackendFlags & SDL_PLATFORM_RESERVED_FLAGS,
                0,
                "teardown must not restore capabilities after only one platform field changed"
            );
        });
        assert_ne!(published_flags, 0);
    }

    #[test]
    fn platform_name_only_drift_revokes_reserved_capabilities() {
        let _guard = crate::tests::test_guard();
        let mut context = Context::create();
        let mut runtime = synthetic_claimed_registration(
            &mut context,
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            synthetic_create_window,
        );
        context.binding().with_bound_context(|| unsafe {
            (*sys::igGetIO_Nil()).BackendPlatformName = FOREIGN_BACKEND_NAME.as_ptr().cast();
        });

        assert!(matches!(
            runtime.poll_fault(),
            Err(Sdl3BackendError::PlatformStateReplaced {
                field: "BackendPlatformName"
            })
        ));
        context.binding().with_bound_context(|| unsafe {
            assert_eq!(
                (*sys::igGetIO_Nil()).BackendFlags & SDL_PLATFORM_RESERVED_FLAGS,
                0,
                "a foreign name alone is not a complete platform takeover"
            );
        });

        runtime.shutdown_platform(&mut context).unwrap();
        context.binding().with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            assert_eq!((*io).BackendFlags & SDL_PLATFORM_RESERVED_FLAGS, 0);
            assert_eq!(
                (*io).BackendPlatformName,
                FOREIGN_BACKEND_NAME.as_ptr().cast()
            );
            (*io).BackendPlatformName = std::ptr::null();
        });
    }

    #[test]
    fn complete_foreign_platform_takeover_preserves_capability_flags() {
        let _guard = crate::tests::test_guard();
        let mut context = Context::create();
        let original_service_callbacks = context.binding().with_bound_context(|| unsafe {
            let platform_io = &*sys::igGetPlatformIO_Nil();
            (
                platform_io.Platform_GetClipboardTextFn,
                platform_io.Platform_SetClipboardTextFn,
                platform_io.Platform_OpenInShellFn,
                platform_io.Platform_SetImeDataFn,
            )
        });
        let mut runtime = synthetic_claimed_registration(
            &mut context,
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            synthetic_create_window,
        );
        let foreign_flags = context.binding().with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            let main_viewport = sys::igGetMainViewport();
            (*platform_io).Platform_GetClipboardTextFn = original_service_callbacks.0;
            (*platform_io).Platform_SetClipboardTextFn = original_service_callbacks.1;
            (*platform_io).Platform_OpenInShellFn = original_service_callbacks.2;
            (*platform_io).Platform_SetImeDataFn = original_service_callbacks.3;
            (*platform_io).Platform_CreateWindow = Some(foreign_create_window);
            (*platform_io).Platform_DestroyWindow = Some(foreign_destroy_window);
            (*platform_io).Platform_RenderWindow = Some(foreign_platform_render_window);
            (*platform_io).Platform_SwapBuffers = Some(foreign_platform_swap_buffers);
            (*platform_io).Platform_SetWindowAlpha = Some(foreign_set_window_alpha);
            (*platform_io).Platform_ClipboardUserData = FOREIGN_PLATFORM_DATA as *mut _;
            (*io).BackendPlatformUserData = FOREIGN_BACKEND_DATA as *mut _;
            (*io).BackendPlatformName = FOREIGN_BACKEND_NAME.as_ptr().cast();
            (*main_viewport).PlatformUserData = FOREIGN_VIEWPORT_DATA as *mut _;
            (*main_viewport).PlatformHandle = FOREIGN_VIEWPORT_HANDLE as *mut _;
            (*main_viewport).PlatformHandleRaw = FOREIGN_VIEWPORT_HANDLE_RAW as *mut _;
            (*io).BackendFlags =
                ((*io).BackendFlags & !SDL_PLATFORM_RESERVED_FLAGS) | SDL_PLATFORM_RESERVED_FLAGS;
            (*io).BackendFlags & SDL_PLATFORM_RESERVED_FLAGS
        });
        let mut secondary = sys::ImGuiViewport::default();

        context
            .binding()
            .with_bound_context(|| unsafe { create_window_callback_for_test(&mut secondary) });

        assert!(secondary.PlatformUserData.is_null());
        let observed_flags = context.binding().with_bound_context(|| unsafe {
            (*sys::igGetIO_Nil()).BackendFlags & SDL_PLATFORM_RESERVED_FLAGS
        });

        let shutdown = runtime.shutdown_platform(&mut context);
        let restored_flags = context.binding().with_bound_context(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            let main_viewport = sys::igGetMainViewport();
            let restored_flags = (*io).BackendFlags & SDL_PLATFORM_RESERVED_FLAGS;
            sys::ImGuiPlatformIO_ClearPlatformHandlers(platform_io);
            (*platform_io).Platform_ClipboardUserData = std::ptr::null_mut();
            (*io).BackendPlatformUserData = std::ptr::null_mut();
            (*io).BackendPlatformName = std::ptr::null();
            (*io).BackendFlags &= !SDL_PLATFORM_RESERVED_FLAGS;
            (*main_viewport).PlatformUserData = std::ptr::null_mut();
            (*main_viewport).PlatformHandle = std::ptr::null_mut();
            (*main_viewport).PlatformHandleRaw = std::ptr::null_mut();
            restored_flags
        });
        assert_eq!(
            observed_flags, foreign_flags,
            "detecting a foreign backend must not clear its published capabilities"
        );
        assert!(matches!(
            shutdown,
            Err(Sdl3BackendError::PlatformCallbackReplaced { .. })
        ));
        assert_eq!(
            restored_flags, foreign_flags,
            "shutdown must restore the foreign platform capability snapshot"
        );
    }

    #[test]
    fn foreign_write_to_reserved_empty_platform_slot_blocks_direct_trampoline() {
        let _guard = crate::tests::test_guard();
        let mut context = Context::create();
        let mut runtime = synthetic_claimed_registration(
            &mut context,
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            synthetic_create_window,
        );
        context.binding().with_bound_context(|| unsafe {
            let platform_io = sys::igGetPlatformIO_Nil();
            assert!((*platform_io).Platform_SetWindowAlpha.is_none());
            (*platform_io).Platform_SetWindowAlpha = Some(foreign_set_window_alpha);
        });
        let mut viewport = sys::ImGuiViewport::default();

        context
            .binding()
            .with_bound_context(|| unsafe { create_window_callback_for_test(&mut viewport) });

        assert!(viewport.PlatformUserData.is_null());
        assert_eq!(runtime.control.state(), RuntimeState::ShuttingDown);
        assert!(matches!(
            runtime.poll_fault(),
            Err(Sdl3BackendError::PlatformCallbackReplaced {
                callback: "Platform_SetWindowAlpha"
            })
        ));
        context.binding().with_bound_context(|| unsafe {
            assert_eq!(
                (*sys::igGetIO_Nil()).BackendFlags & SDL_PLATFORM_RESERVED_FLAGS,
                0,
                "filling one reserved callback slot must revoke SDL platform capabilities"
            );
        });
        runtime.shutdown_platform(&mut context).unwrap();
        context.binding().with_bound_context(|| unsafe {
            assert_eq!(
                (*sys::igGetIO_Nil()).BackendFlags & SDL_PLATFORM_RESERVED_FLAGS,
                0
            );
            (*sys::igGetPlatformIO_Nil()).Platform_SetWindowAlpha = None;
        });
    }

    #[test]
    fn callback_panic_latches_shutdown_after_the_fault_is_consumed() {
        let _guard = crate::tests::test_guard();
        let mut context = Context::create();
        let mut runtime = synthetic_claimed_registration(
            &mut context,
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(0)),
            synthetic_create_window,
        );

        runtime
            .control
            .record_callback_panicked("Platform_CreateWindow");

        assert_eq!(runtime.control.state(), RuntimeState::ShuttingDown);
        assert!(matches!(
            runtime.poll_fault(),
            Err(Sdl3BackendError::PlatformCallbackPanicked {
                callback: "Platform_CreateWindow"
            })
        ));
        assert!(matches!(
            runtime.control.ensure_bound_entry(),
            Err(Sdl3BackendError::RuntimeDetached)
        ));
        runtime.shutdown_platform(&mut context).unwrap();
    }

    #[cfg(feature = "sdlgpu3-renderer")]
    #[test]
    fn sdlgpu_create_failure_clears_upstream_sentinel_before_destroy_can_release_it() {
        let _guard = crate::tests::test_guard();
        for (fault, is_claim_failure) in [(1_u64 << 11, true), (1_u64 << 12, false)] {
            let mut context = Context::create();
            let mut runtime = synthetic_claimed_registration(
                &mut context,
                Rc::new(Cell::new(0)),
                Rc::new(Cell::new(0)),
                Rc::new(Cell::new(0)),
                synthetic_create_window,
            );
            let mut viewport = sys::ImGuiViewport::default();
            viewport.RendererUserData = OWNED_BACKEND_DATA as *mut _;

            runtime.control.record_native_faults(fault);
            unsafe { finish_sdlgpu_renderer_create(&runtime.control, &mut viewport, fault) };

            assert!(viewport.RendererUserData.is_null());
            assert!(
                runtime
                    .control
                    .owned_renderer_viewport(&mut viewport)
                    .is_none()
            );
            assert!(runtime.control.viewport_failed(&mut viewport));
            let error = runtime.poll_fault();
            if is_claim_failure {
                assert!(matches!(
                    error,
                    Err(Sdl3BackendError::ViewportSdlGpuClaimFailed)
                ));
            } else {
                assert!(matches!(
                    error,
                    Err(Sdl3BackendError::ViewportSdlGpuConfigureFailed)
                ));
            }
            runtime.shutdown_platform(&mut context).unwrap();
        }
    }

    #[cfg(feature = "sdlgpu3-renderer")]
    #[test]
    fn sdlgpu_secondary_render_faults_are_typed_and_close_the_viewport() {
        let _guard = crate::tests::test_guard();
        for fault in [1_u64 << 14, 1_u64 << 15, 1_u64 << 16, 1_u64 << 17] {
            let mut context = Context::create();
            let mut runtime = synthetic_claimed_registration(
                &mut context,
                Rc::new(Cell::new(0)),
                Rc::new(Cell::new(0)),
                Rc::new(Cell::new(0)),
                synthetic_create_window,
            );
            let mut viewport = sys::ImGuiViewport::default();

            runtime.control.record_native_faults(fault);
            runtime.control.mark_viewport_failed(&mut viewport);

            assert!(viewport.PlatformRequestClose);
            assert!(viewport.DrawData.is_null());
            let error = runtime.poll_fault();
            match fault {
                value if value == 1_u64 << 14 => assert!(matches!(
                    error,
                    Err(Sdl3BackendError::ViewportSdlGpuCommandBufferFailed)
                )),
                value if value == 1_u64 << 15 => assert!(matches!(
                    error,
                    Err(Sdl3BackendError::ViewportSdlGpuSwapchainFailed)
                )),
                value if value == 1_u64 << 16 => assert!(matches!(
                    error,
                    Err(Sdl3BackendError::ViewportSdlGpuRenderPassFailed)
                )),
                _ => assert!(matches!(
                    error,
                    Err(Sdl3BackendError::ViewportSdlGpuSubmitFailed)
                )),
            }
            runtime.shutdown_platform(&mut context).unwrap();
        }
    }
}

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(feature = "multi-viewport")]
use std::sync::{Arc, atomic::AtomicBool};

#[cfg(feature = "multi-viewport")]
use crate::callback_ownership::validate_platform_viewport_state;
use crate::callback_ownership::{
    PlatformCallbackOwnership, PlatformClaimBaseline, RendererCallbackOwnership,
    RendererShutdownRestore, SDL_PLATFORM_RESERVED_FLAGS, SDL_RENDERER_RESERVED_FLAGS,
    ViewportPlatformState, preflight_platform_claim, restore_baseline_after_failed_initialization,
};
#[cfg(feature = "multi-viewport")]
use crate::core::Sdl3VulkanSurfaceError;
use crate::core::{Sdl3BackendError, Sdl3OpenGlViewportSwapInterval, shutdown_platform_impl};
#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
use crate::renderer_textures::RendererTextureStore;
use dear_imgui_rs::RendererConsumer;
#[cfg(feature = "multi-viewport")]
use dear_imgui_rs::platform_io::Viewport;
#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
use dear_imgui_rs::render::{TextureFeedback, TextureRequest};
use dear_imgui_rs::{
    Context, ContextAttachment, ContextAttachmentLease, ContextAttachmentRole,
    ContextAttachmentTeardownError, ContextBinding, ContextDestroyed, ContextLifecycle,
    ContextTeardown, Id, TextureData, sys,
};

struct Sdl3PlatformAttachmentMarker;
struct Sdl3RendererAttachmentMarker;

static NEXT_PLATFORM_SESSION_GENERATION: AtomicU64 = AtomicU64::new(1);
static PLATFORM_SESSION_OWNER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct Sdl3PlatformSession {
    generation: u64,
}

impl Sdl3PlatformSession {
    fn acquire() -> Result<Self, Sdl3BackendError> {
        let generation = loop {
            let generation = NEXT_PLATFORM_SESSION_GENERATION.fetch_add(1, Ordering::Relaxed);
            if generation != 0 {
                break generation;
            }
        };
        PLATFORM_SESSION_OWNER
            .compare_exchange(0, generation, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| Sdl3BackendError::PlatformSessionOccupied)?;
        Ok(Self { generation })
    }
}

impl Drop for Sdl3PlatformSession {
    fn drop(&mut self) {
        let released = PLATFORM_SESSION_OWNER.compare_exchange(
            self.generation,
            0,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        debug_assert!(released.is_ok(), "SDL3 platform session owner changed");
    }
}

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
    Vulkan,
}

/// Native OpenGL platform callbacks that completed successfully during one viewport frame.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Sdl3OpenGlViewportFrameReport {
    context_activated_viewports: Vec<Id>,
    swapped_viewports: Vec<Id>,
}

impl Sdl3OpenGlViewportFrameReport {
    /// Returns secondary viewport IDs whose native render-context transaction completed.
    pub fn context_activated_viewports(&self) -> &[Id] {
        &self.context_activated_viewports
    }

    /// Returns secondary viewport IDs whose native swap transaction completed.
    pub fn swapped_viewports(&self) -> &[Id] {
        &self.swapped_viewports
    }
}

/// Failure to begin or finish an SDL3 OpenGL viewport frame trace.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Sdl3OpenGlViewportFrameTraceError {
    /// The underlying platform runtime rejected the operation.
    #[error(transparent)]
    Backend(#[from] Sdl3BackendError),
    /// The platform runtime was not initialized for OpenGL.
    #[error("SDL3 OpenGL viewport tracing requires an OpenGL platform runtime")]
    RequiresOpenGl,
    /// Another guard is already collecting the runtime's callback events.
    #[error("an SDL3 OpenGL viewport frame trace is already active")]
    AlreadyActive,
}

#[derive(Debug)]
struct ActiveOpenGlViewportFrameTrace {
    context_activated_viewports: HashSet<Id>,
    swapped_viewports: HashSet<Id>,
}

#[derive(Debug, Default)]
struct OpenGlViewportFrameTraceState {
    active: Option<ActiveOpenGlViewportFrameTrace>,
}

/// Scoped collector for one SDL3 OpenGL secondary-viewport platform pass.
///
/// Dropping this guard without calling [`Self::finish`] aborts its report. Each runtime accepts
/// only one live trace, and callback routing remains bound to that runtime's Context attachment.
#[must_use = "keep the trace alive through the platform-window pump, then call finish"]
pub struct Sdl3OpenGlViewportFrameTrace<'runtime> {
    control: &'runtime RuntimeControl,
    finished: bool,
}

impl fmt::Debug for Sdl3OpenGlViewportFrameTrace<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Sdl3OpenGlViewportFrameTrace")
            .field("context", &self.control.binding.id())
            .field("finished", &self.finished)
            .finish()
    }
}

impl Sdl3OpenGlViewportFrameTrace<'_> {
    /// Finishes this frame trace and returns only successful native transactions.
    ///
    /// The caller should restore the main OpenGL context before finishing the trace, then call
    /// [`Sdl3PlatformBackend::poll_fault`](crate::Sdl3PlatformBackend::poll_fault) before the main
    /// window is swapped.
    pub fn finish(mut self) -> Sdl3OpenGlViewportFrameReport {
        let report = self.control.finish_opengl_viewport_frame_trace();
        self.finished = true;
        report
    }
}

impl Drop for Sdl3OpenGlViewportFrameTrace<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.control.abort_opengl_viewport_frame_trace();
        }
    }
}

#[cfg(feature = "multi-viewport")]
#[derive(Debug, Default)]
struct VulkanSurfaceProviderState {
    leased: AtomicBool,
}

/// Exclusive capability for creating secondary Vulkan surfaces through one live SDL3 runtime.
///
/// The provider is intentionally neither `Clone` nor constructible by users. Its lifetime blocks
/// SDL platform shutdown, and each invocation validates the current Context, SDL callback owner,
/// and viewport sidecar immediately before entering the native callback.
#[cfg(feature = "multi-viewport")]
#[must_use = "keep the provider alive until the renderer has destroyed every SDL3 Vulkan surface"]
pub struct Sdl3VulkanSurfaceProvider {
    state: Arc<VulkanSurfaceProviderState>,
}

#[cfg(feature = "multi-viewport")]
impl fmt::Debug for Sdl3VulkanSurfaceProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Sdl3VulkanSurfaceProvider")
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "multi-viewport")]
impl Sdl3VulkanSurfaceProvider {
    /// Create a Vulkan surface for one SDL3-owned viewport.
    ///
    /// # Safety
    ///
    /// `vulkan_instance` must be a live `VkInstance` compatible with SDL3 and must outlive the
    /// returned surface. The caller must destroy the returned surface before dropping this
    /// provider. The viewport must belong to the provider's Dear ImGui Context, and that Context
    /// must be current on the calling thread.
    pub unsafe fn create_surface(
        &self,
        viewport: &mut Viewport,
        vulkan_instance: u64,
    ) -> Result<u64, Sdl3VulkanSurfaceError> {
        with_current_runtime(|control| {
            if !Arc::ptr_eq(&self.state, &control.vulkan_surface_provider)
                || !control.expects_vulkan()
            {
                return Err(Sdl3VulkanSurfaceError::OwnerUnavailable);
            }
            let entry = control.enter_bound()?;
            if !control.validate_platform_ownership_bound()
                || !unsafe { validate_platform_viewport_state(control, viewport.as_raw_mut()) }
            {
                entry.finish()?;
                return Err(Sdl3VulkanSurfaceError::OwnerUnavailable);
            }

            let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
            let callback = if platform_io.is_null() {
                None
            } else {
                unsafe { (*platform_io).Platform_CreateVkSurface }
            }
            .ok_or(Sdl3VulkanSurfaceError::CallbackUnavailable)?;
            let mut surface = 0;
            let code = unsafe {
                callback(
                    viewport.as_raw_mut(),
                    vulkan_instance,
                    std::ptr::null(),
                    &mut surface,
                )
            };
            if code != 0 || surface == 0 {
                return Err(Sdl3VulkanSurfaceError::CallbackFailed { code, surface });
            }
            entry.finish()?;
            Ok(surface)
        })
        .unwrap_or(Err(Sdl3VulkanSurfaceError::OwnerUnavailable))
    }
}

#[cfg(feature = "multi-viewport")]
impl Drop for Sdl3VulkanSurfaceProvider {
    fn drop(&mut self) {
        self.state.leased.store(false, Ordering::Release);
    }
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

type RendererTextureUpdate = Rc<dyn Fn(&mut TextureData)>;

struct NativeLifecycle {
    renderer_shutdown: Option<Rc<dyn Fn()>>,
    renderer_device_objects_destroy: Option<Rc<dyn Fn()>>,
    renderer_texture_update: Option<RendererTextureUpdate>,
    platform_shutdown: Rc<dyn Fn()>,
}

impl NativeLifecycle {
    fn new(
        renderer_shutdown: Option<Rc<dyn Fn()>>,
        renderer_device_objects_destroy: Option<Rc<dyn Fn()>>,
        renderer_texture_update: Option<RendererTextureUpdate>,
        platform_shutdown: Rc<dyn Fn()>,
    ) -> Self {
        Self {
            renderer_shutdown,
            renderer_device_objects_destroy,
            renderer_texture_update,
            platform_shutdown,
        }
    }
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

pub(super) struct RuntimeEntry<'runtime> {
    control: &'runtime RuntimeControl,
    finished: bool,
}

impl RuntimeEntry<'_> {
    pub(super) fn finish(mut self) -> Result<(), Sdl3BackendError> {
        self.finished = true;
        self.control.finish_entry()
    }
}

impl Drop for RuntimeEntry<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.control.inspect_abandoned_entry();
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
    #[cfg(feature = "multi-viewport")]
    vulkan_surface_provider: Arc<VulkanSurfaceProviderState>,
    gl_viewport_swap_interval: Cell<Sdl3OpenGlViewportSwapInterval>,
    native_renderer: NativeRendererKind,
    lifecycle: NativeLifecycle,
    callbacks: RefCell<Option<PlatformCallbackOwnership>>,
    renderer_callbacks: RefCell<Option<RendererCallbackOwnership>>,
    renderer_shutdown_restore: RefCell<Option<RendererShutdownRestore>>,
    renderer_consumer: RefCell<Option<RendererConsumer>>,
    platform_session: RefCell<Option<Sdl3PlatformSession>>,
    #[cfg(any(
        feature = "opengl3-renderer",
        feature = "sdlrenderer3-renderer",
        feature = "sdlgpu3-renderer"
    ))]
    renderer_textures: RefCell<RendererTextureStore>,
    owned_viewports: RefCell<HashMap<usize, ViewportPlatformState>>,
    owned_renderer_viewports: RefCell<HashMap<usize, *mut std::ffi::c_void>>,
    deferred_platform_viewports: RefCell<HashMap<usize, DeferredPlatformViewportState>>,
    deferred_renderer_viewports: RefCell<HashMap<usize, DeferredRendererViewportState>>,
    failed_viewports: RefCell<HashSet<usize>>,
    faults: RefCell<VecDeque<RuntimeFault>>,
    opengl_viewport_frame_trace: RefCell<OpenGlViewportFrameTraceState>,
    reported_replacements: RefCell<HashSet<&'static str>>,
    revoked_capabilities: Cell<i32>,
    foreign_capabilities: Cell<i32>,
    #[cfg(test)]
    phase_log: RefCell<Vec<&'static str>>,
}

#[derive(Clone, Copy)]
struct DeferredPlatformViewportState {
    id: sys::ImGuiID,
    state: ViewportPlatformState,
}

#[derive(Clone, Copy)]
struct DeferredRendererViewportState {
    id: sys::ImGuiID,
    user_data: *mut std::ffi::c_void,
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

mod control;

pub(super) use control::{RuntimeRegistration, register_runtime, with_current_runtime};

//! Shared WGPU multi-viewport renderer runtime.

use super::WgpuRenderer;
use dear_imgui_rs::internal::RawCast;
use dear_imgui_rs::platform_io::{PlatformIo, Viewport};
use dear_imgui_rs::{BackendFlags, Context, ViewportFlags};
use std::ffi::c_void;
use std::ops::{Deref, DerefMut};
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::thread::ThreadId;

#[cfg(all(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
compile_error!(
    "Features `multi-viewport-winit` and `multi-viewport-sdl3` are mutually exclusive; enable only one."
);

#[cfg(all(not(feature = "multi-viewport-winit"), feature = "multi-viewport-sdl3"))]
use super::multi_viewport_sdl3_adapter as platform_adapter;
#[cfg(feature = "multi-viewport-winit")]
use super::multi_viewport_winit_adapter as platform_adapter;

struct ViewportWgpuData {
    owner_context: usize,
    device: wgpu::Device,
    #[cfg(feature = "wgpu-30")]
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pending_frame: Option<wgpu::SurfaceTexture>,
    pending_reconfigure: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ViewportDataState {
    context: usize,
    pointer: usize,
}

struct ContextRendererState {
    context: usize,
    renderer: usize,
    borrowed: bool,
    thread: ThreadId,
    globals: Option<GlobalHandles>,
}

#[derive(Clone)]
struct GlobalHandles {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    #[cfg(feature = "wgpu-30")]
    queue: wgpu::Queue,
    render_target_format: wgpu::TextureFormat,
}

static RENDERERS: Mutex<Vec<ContextRendererState>> = Mutex::new(Vec::new());
static VIEWPORT_DATA: Mutex<Vec<ViewportDataState>> = Mutex::new(Vec::new());

/// Failure to enable WGPU multi-viewport renderer callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CallbackOwnershipError {
    /// Another renderer backend still owns at least one `Renderer_*` callback slot.
    #[error("ImGuiPlatformIO renderer callbacks are already owned by another backend")]
    RendererCallbacksOccupied,
    /// A secondary viewport already contains renderer-owned user data.
    #[error("a secondary viewport already has RendererUserData owned by another backend")]
    RendererUserDataOccupied,
    /// Existing platform windows would not receive the newly installed create callback.
    #[error(
        "secondary platform windows already exist; destroy them before enabling WGPU multi-viewport"
    )]
    PlatformWindowsAlreadyCreated,
    /// The current Dear ImGui artifact cannot safely bridge the aggregate size callback.
    #[error("dear-imgui-sys was built without PlatformIO aggregate ABI hooks")]
    AggregateCallbackHooksUnavailable,
    /// The renderer has not been initialized with GPU backend data.
    #[error("WGPU renderer is not initialized")]
    RendererNotInitialized,
    /// Per-window surfaces require the WGPU instance used by the renderer.
    #[error("WGPU multi-viewport requires WgpuInitInfo::with_instance")]
    MissingInstance,
    /// Surface capability negotiation requires the WGPU adapter used by the renderer.
    #[error("WGPU multi-viewport requires WgpuInitInfo::with_adapter")]
    MissingAdapter,
    /// Renderer callbacks require a platform backend that already supports viewports.
    #[error("the active platform backend does not advertise multi-viewport support")]
    PlatformBackendUnavailable,
    /// A callback is currently using the renderer registered for this context.
    #[error("cannot replace the WGPU renderer while a viewport callback is active")]
    RendererCallbackActive,
    /// Live secondary viewports still hold surfaces created from the previous renderer.
    #[error("cannot replace the WGPU renderer while live viewport surfaces exist")]
    LiveViewportRendererRebind,
    /// This context already has an active WGPU multi-viewport registration.
    #[error(
        "WGPU multi-viewport is already enabled for this context; shut it down before enabling again"
    )]
    AlreadyEnabled,
    /// One renderer instance cannot back multiple ImGui contexts.
    #[error("this WGPU renderer is already registered with another ImGui context")]
    RendererAlreadyRegistered,
    /// Native multi-viewport surfaces are unavailable on this target.
    #[error("WGPU native multi-viewport rendering is unavailable on this target")]
    UnsupportedTarget,
}

struct CurrentContextGuard {
    previous: *mut dear_imgui_rs::sys::ImGuiContext,
    target: *mut dear_imgui_rs::sys::ImGuiContext,
}

impl CurrentContextGuard {
    unsafe fn bind(target: *mut dear_imgui_rs::sys::ImGuiContext) -> Self {
        let previous = unsafe { dear_imgui_rs::sys::igGetCurrentContext() };
        if previous != target {
            unsafe { dear_imgui_rs::sys::igSetCurrentContext(target) };
        }
        Self { previous, target }
    }
}

impl Drop for CurrentContextGuard {
    fn drop(&mut self) {
        if self.previous != self.target {
            unsafe { dear_imgui_rs::sys::igSetCurrentContext(self.previous) };
        }
    }
}

fn current_context() -> *mut dear_imgui_rs::sys::ImGuiContext {
    unsafe { dear_imgui_rs::sys::igGetCurrentContext() }
}

fn renderer_globals(renderer: &WgpuRenderer) -> Result<GlobalHandles, CallbackOwnershipError> {
    #[cfg(target_arch = "wasm32")]
    return Err(CallbackOwnershipError::UnsupportedTarget);

    #[cfg(not(target_arch = "wasm32"))]
    {
        let backend = renderer
            .backend_data
            .as_ref()
            .ok_or(CallbackOwnershipError::RendererNotInitialized)?;
        Ok(GlobalHandles {
            instance: backend
                .instance
                .clone()
                .ok_or(CallbackOwnershipError::MissingInstance)?,
            adapter: backend
                .adapter
                .clone()
                .ok_or(CallbackOwnershipError::MissingAdapter)?,
            device: backend.device.clone(),
            #[cfg(feature = "wgpu-30")]
            queue: backend.queue.clone(),
            render_target_format: backend.render_target_format,
        })
    }
}

fn insert_renderer_state(
    context: *mut dear_imgui_rs::sys::ImGuiContext,
    renderer: *mut WgpuRenderer,
    globals: Option<GlobalHandles>,
) {
    let context = context as usize;
    let renderer = renderer as usize;
    let mut renderers = RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    debug_assert!(!renderers.iter().any(|state| state.context == context));
    renderers.push(ContextRendererState {
        context,
        renderer,
        borrowed: false,
        thread: std::thread::current().id(),
        globals,
    });
}

fn remove_renderer_state_for_context(context: *mut dear_imgui_rs::sys::ImGuiContext) -> bool {
    let context = context as usize;
    let mut removed = false;
    RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .retain(|state| {
            let keep = state.context != context;
            removed |= !keep;
            keep
        });
    removed
}

fn remove_renderer_state_for_renderer(renderer: *mut WgpuRenderer) {
    unsafe { &*renderer }
        .multi_viewport_active
        .store(false, Ordering::Release);
    let renderer = renderer as usize;
    RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .retain(|state| state.renderer != renderer);
}

fn has_renderer_state(context: *mut dear_imgui_rs::sys::ImGuiContext) -> bool {
    let context = context as usize;
    RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .iter()
        .any(|state| state.context == context)
}

fn validate_new_registration(
    context: *mut dear_imgui_rs::sys::ImGuiContext,
    renderer: *mut WgpuRenderer,
) -> Result<(), CallbackOwnershipError> {
    let context = context as usize;
    let renderer = renderer as usize;
    let renderers = RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let Some(state) = renderers.iter().find(|state| state.context == context) else {
        if renderers.iter().any(|state| state.renderer == renderer)
            || unsafe { &*(renderer as *const WgpuRenderer) }
                .multi_viewport_active
                .load(Ordering::Acquire)
        {
            return Err(CallbackOwnershipError::RendererAlreadyRegistered);
        }
        drop(renderers);
        return if has_viewport_data(context) {
            Err(CallbackOwnershipError::LiveViewportRendererRebind)
        } else {
            Ok(())
        };
    };
    if state.borrowed {
        return Err(CallbackOwnershipError::RendererCallbackActive);
    }
    drop(renderers);
    if has_viewport_data(context) {
        Err(CallbackOwnershipError::LiveViewportRendererRebind)
    } else {
        Err(CallbackOwnershipError::AlreadyEnabled)
    }
}

fn globals_for_current_context() -> Option<GlobalHandles> {
    let context = current_context() as usize;
    if context == 0 {
        return None;
    }
    RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .iter()
        .find(|state| state.context == context && state.thread == std::thread::current().id())
        .and_then(|state| state.globals.clone())
}

fn register_viewport_data(
    context: *mut dear_imgui_rs::sys::ImGuiContext,
    pointer: *mut ViewportWgpuData,
) {
    if pointer.is_null() {
        return;
    }
    let state = ViewportDataState {
        context: context as usize,
        pointer: pointer as usize,
    };
    let mut data = VIEWPORT_DATA
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if !data.contains(&state) {
        data.push(state);
    }
}

fn unregister_viewport_data(pointer: *mut ViewportWgpuData) {
    let pointer = pointer as usize;
    VIEWPORT_DATA
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .retain(|state| state.pointer != pointer);
}

fn has_viewport_data(context: usize) -> bool {
    VIEWPORT_DATA
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .iter()
        .any(|state| state.context == context)
}

fn owns_viewport_data(
    context: *mut dear_imgui_rs::sys::ImGuiContext,
    pointer: *mut ViewportWgpuData,
) -> bool {
    let expected = ViewportDataState {
        context: context as usize,
        pointer: pointer as usize,
    };
    !pointer.is_null()
        && VIEWPORT_DATA
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .contains(&expected)
}

unsafe fn viewport_data_pointer(viewport: &Viewport) -> Option<*mut ViewportWgpuData> {
    let context = current_context();
    let pointer = viewport.renderer_user_data().cast::<ViewportWgpuData>();
    if owns_viewport_data(context, pointer) {
        Some(pointer)
    } else {
        None
    }
}

unsafe fn destroy_viewport_data(
    context: *mut dear_imgui_rs::sys::ImGuiContext,
    viewport: &mut Viewport,
) {
    let pointer = viewport.renderer_user_data().cast::<ViewportWgpuData>();
    if !owns_viewport_data(context, pointer) {
        return;
    }
    unregister_viewport_data(pointer);
    viewport.set_renderer_user_data(std::ptr::null_mut());
    let data = unsafe { Box::from_raw(pointer) };
    debug_assert_eq!(data.owner_context, context as usize);
    drop(data);
}

fn unary_callback_matches(
    actual: Option<unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport)>,
    expected: unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport),
) -> bool {
    actual.is_some_and(|actual| std::ptr::fn_addr_eq(actual, expected))
}

fn render_callback_matches(
    actual: Option<unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport, *mut c_void)>,
    expected: unsafe extern "C" fn(*mut dear_imgui_rs::sys::ImGuiViewport, *mut c_void),
) -> bool {
    actual.is_some_and(|actual| std::ptr::fn_addr_eq(actual, expected))
}

fn callbacks_owned(platform_io: &PlatformIo) -> bool {
    unary_callback_matches(
        platform_io.renderer_create_window_raw(),
        renderer_create_window_sys,
    ) && unary_callback_matches(
        platform_io.renderer_destroy_window_raw(),
        renderer_destroy_window_sys,
    ) && platform_io.renderer_set_window_size_matches_pointer_callback(renderer_set_window_size_sys)
        && render_callback_matches(
            platform_io.renderer_render_window_raw(),
            renderer_render_window_sys,
        )
        && render_callback_matches(
            platform_io.renderer_swap_buffers_raw(),
            renderer_swap_buffers_sys,
        )
}

fn any_callback_owned(platform_io: &PlatformIo) -> bool {
    unary_callback_matches(
        platform_io.renderer_create_window_raw(),
        renderer_create_window_sys,
    ) || unary_callback_matches(
        platform_io.renderer_destroy_window_raw(),
        renderer_destroy_window_sys,
    ) || platform_io.renderer_set_window_size_matches_pointer_callback(renderer_set_window_size_sys)
        || render_callback_matches(
            platform_io.renderer_render_window_raw(),
            renderer_render_window_sys,
        )
        || render_callback_matches(
            platform_io.renderer_swap_buffers_raw(),
            renderer_swap_buffers_sys,
        )
}

fn claim_callbacks(
    platform_io: &mut PlatformIo,
    aggregate_hooks_available: bool,
) -> Result<(), CallbackOwnershipError> {
    if !platform_io.renderer_callbacks_are_empty() {
        if callbacks_owned(platform_io) {
            return Ok(());
        }
        return Err(CallbackOwnershipError::RendererCallbacksOccupied);
    }
    if !aggregate_hooks_available {
        return Err(CallbackOwnershipError::AggregateCallbackHooksUnavailable);
    }
    platform_io.set_renderer_create_window_raw(Some(renderer_create_window_sys));
    platform_io.set_renderer_destroy_window_raw(Some(renderer_destroy_window_sys));
    platform_io.set_renderer_set_window_size_raw(Some(renderer_set_window_size_sys));
    platform_io.set_renderer_render_window_raw(Some(renderer_render_window_sys));
    platform_io.set_renderer_swap_buffers_raw(Some(renderer_swap_buffers_sys));
    Ok(())
}

fn validate_secondary_viewports(
    states: &[(bool, *mut c_void)],
) -> Result<(), CallbackOwnershipError> {
    if states.iter().any(|(_, slot)| !slot.is_null()) {
        Err(CallbackOwnershipError::RendererUserDataOccupied)
    } else if states.iter().any(|(created, _)| *created) {
        Err(CallbackOwnershipError::PlatformWindowsAlreadyCreated)
    } else {
        Ok(())
    }
}

/// Enables WGPU multi-viewport renderer callbacks.
///
/// # Safety
///
/// `renderer` must remain at the same address until [`shutdown_multi_viewport_support`] completes.
/// All callbacks and renderer access must occur on the enabling thread. While enabled, callers must
/// not concurrently access, reinitialize, shut down, move, or drop the renderer, or replace any
/// viewport's `RendererUserData`.
pub unsafe fn enable(
    renderer: &mut WgpuRenderer,
    context: &mut Context,
) -> Result<(), CallbackOwnershipError> {
    let raw_context = context.as_raw();
    let _context_guard = unsafe { CurrentContextGuard::bind(raw_context) };
    let globals = renderer_globals(renderer)?;
    if !context
        .io()
        .backend_flags()
        .contains(BackendFlags::PLATFORM_HAS_VIEWPORTS)
    {
        return Err(CallbackOwnershipError::PlatformBackendUnavailable);
    }
    validate_new_registration(raw_context, renderer)?;
    let secondary_viewports = context
        .platform_io()
        .viewports_iter()
        .skip(1)
        .map(|viewport| {
            (
                viewport.platform_window_created(),
                viewport.renderer_user_data(),
            )
        })
        .collect::<Vec<_>>();
    validate_secondary_viewports(&secondary_viewports)?;
    claim_callbacks(
        context.platform_io_mut(),
        dear_imgui_rs::sys::HAS_PLATFORM_IO_AGGREGATE_HOOKS,
    )?;
    insert_renderer_state(raw_context, renderer, Some(globals));
    renderer
        .multi_viewport_active
        .store(true, Ordering::Release);
    let io = context.io_mut();
    io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_VIEWPORTS);
    Ok(())
}

pub(crate) fn clear_for_drop(renderer: *mut WgpuRenderer) {
    remove_renderer_state_for_renderer(renderer);
}

fn disable_after_platform_shutdown(context: &mut Context) {
    let raw_context = context.as_raw();
    let _context_guard = unsafe { CurrentContextGuard::bind(raw_context) };
    let had_state = has_renderer_state(raw_context);
    let registered_renderer = RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .iter()
        .find(|state| state.context == raw_context as usize)
        .map(|state| state.renderer as *mut WgpuRenderer);
    let platform_io = context.platform_io_mut();
    let had_owned_callbacks = any_callback_owned(platform_io);
    for viewport in platform_io.viewports_iter_mut() {
        unsafe { destroy_viewport_data(raw_context, viewport) };
    }
    if unary_callback_matches(
        platform_io.renderer_create_window_raw(),
        renderer_create_window_sys,
    ) {
        platform_io.set_renderer_create_window_raw(None);
    }
    if unary_callback_matches(
        platform_io.renderer_destroy_window_raw(),
        renderer_destroy_window_sys,
    ) {
        platform_io.set_renderer_destroy_window_raw(None);
    }
    platform_io.clear_renderer_set_window_size_if_pointer_callback(renderer_set_window_size_sys);
    if render_callback_matches(
        platform_io.renderer_render_window_raw(),
        renderer_render_window_sys,
    ) {
        platform_io.set_renderer_render_window_raw(None);
    }
    if render_callback_matches(
        platform_io.renderer_swap_buffers_raw(),
        renderer_swap_buffers_sys,
    ) {
        platform_io.set_renderer_swap_buffers_raw(None);
    }
    let renderer_callbacks_are_empty = platform_io.renderer_callbacks_are_empty();
    if let Some(renderer) = registered_renderer {
        unsafe { &*renderer }
            .multi_viewport_active
            .store(false, Ordering::Release);
    }
    remove_renderer_state_for_context(raw_context);
    if (had_state || had_owned_callbacks) && renderer_callbacks_are_empty {
        let io = context.io_mut();
        let mut flags = io.backend_flags();
        flags.remove(BackendFlags::RENDERER_HAS_VIEWPORTS);
        io.set_backend_flags(flags);
    }
}

/// Destroys secondary platform windows before releasing renderer state and callbacks.
pub fn shutdown_multi_viewport_support(context: &mut Context) {
    context.destroy_platform_windows();
    disable_after_platform_shutdown(context);
}

struct RendererBorrowGuard {
    context: usize,
    renderer: *mut WgpuRenderer,
}

unsafe fn borrow_renderer() -> Option<RendererBorrowGuard> {
    let context = current_context() as usize;
    if context == 0 {
        return None;
    }
    let mut renderers = RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let state = renderers
        .iter_mut()
        .find(|state| state.context == context && state.thread == std::thread::current().id())?;
    if state.renderer == 0 || state.borrowed {
        return None;
    }
    state.borrowed = true;
    Some(RendererBorrowGuard {
        context,
        renderer: state.renderer as *mut WgpuRenderer,
    })
}

impl Drop for RendererBorrowGuard {
    fn drop(&mut self) {
        let mut renderers = RENDERERS
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(state) = renderers
            .iter_mut()
            .find(|state| state.context == self.context && state.renderer == self.renderer as usize)
        {
            state.borrowed = false;
        }
    }
}

impl Deref for RendererBorrowGuard {
    type Target = WgpuRenderer;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.renderer }
    }
}

impl DerefMut for RendererBorrowGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.renderer }
    }
}

fn surface_config(
    globals: &GlobalHandles,
    surface: &wgpu::Surface<'static>,
    size: [u32; 2],
) -> Option<wgpu::SurfaceConfiguration> {
    let capabilities = surface.get_capabilities(&globals.adapter);
    if !capabilities.formats.contains(&globals.render_target_format) {
        eprintln!(
            "[wgpu-mv] surface does not support renderer format {:?}",
            globals.render_target_format
        );
        return None;
    }
    let present_mode = if capabilities
        .present_modes
        .contains(&wgpu::PresentMode::Fifo)
    {
        wgpu::PresentMode::Fifo
    } else {
        capabilities.present_modes.first().copied()?
    };
    let alpha_mode = if capabilities
        .alpha_modes
        .contains(&wgpu::CompositeAlphaMode::Opaque)
    {
        wgpu::CompositeAlphaMode::Opaque
    } else if capabilities
        .alpha_modes
        .contains(&wgpu::CompositeAlphaMode::Auto)
    {
        wgpu::CompositeAlphaMode::Auto
    } else {
        capabilities.alpha_modes.first().copied()?
    };
    Some(wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: globals.render_target_format,
        #[cfg(feature = "wgpu-30")]
        color_space: wgpu::SurfaceColorSpace::Auto,
        width: size[0].max(1),
        height: size[1].max(1),
        present_mode,
        alpha_mode,
        view_formats: vec![globals.render_target_format],
        desired_maximum_frame_latency: 1,
    })
}

unsafe fn create_viewport_data(
    context: *mut dear_imgui_rs::sys::ImGuiContext,
    viewport: &Viewport,
    globals: &GlobalHandles,
) -> Option<ViewportWgpuData> {
    let (surface, size) = unsafe { platform_adapter::create_surface(&globals.instance, viewport) }?;
    let config = surface_config(globals, &surface, size)?;
    surface.configure(&globals.device, &config);
    Some(ViewportWgpuData {
        owner_context: context as usize,
        device: globals.device.clone(),
        #[cfg(feature = "wgpu-30")]
        queue: globals.queue.clone(),
        surface,
        config,
        pending_frame: None,
        pending_reconfigure: false,
    })
}

unsafe fn reconfigure_surface(viewport: &Viewport, data: &mut ViewportWgpuData) -> bool {
    let Some(size) = (unsafe { platform_adapter::framebuffer_size(viewport) }) else {
        return false;
    };
    data.config.width = size[0].max(1);
    data.config.height = size[1].max(1);
    data.surface.configure(&data.device, &data.config);
    true
}

unsafe fn recreate_surface(
    viewport: &Viewport,
    data: &mut ViewportWgpuData,
    globals: &GlobalHandles,
) -> bool {
    let Some((surface, size)) =
        (unsafe { platform_adapter::create_surface(&globals.instance, viewport) })
    else {
        return false;
    };
    let Some(config) = surface_config(globals, &surface, size) else {
        return false;
    };
    surface.configure(&globals.device, &config);
    data.pending_frame = None;
    data.pending_reconfigure = false;
    data.device = globals.device.clone();
    #[cfg(feature = "wgpu-30")]
    {
        data.queue = globals.queue.clone();
    }
    data.surface = surface;
    data.config = config;
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SurfaceEvent {
    Success,
    Suboptimal,
    Outdated,
    Lost,
    Timeout,
    #[cfg(any(feature = "wgpu-29", feature = "wgpu-30", test))]
    Occluded,
    #[cfg(any(feature = "wgpu-29", feature = "wgpu-30", test))]
    Validation,
    #[cfg(any(feature = "wgpu-27", feature = "wgpu-28", test))]
    OutOfMemory,
    #[cfg(any(feature = "wgpu-27", feature = "wgpu-28", test))]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SurfaceAction {
    Render,
    RenderThenReconfigure,
    Reconfigure,
    Recreate,
    Skip,
    Reject,
}

const fn surface_action(event: SurfaceEvent) -> SurfaceAction {
    match event {
        SurfaceEvent::Success => SurfaceAction::Render,
        SurfaceEvent::Suboptimal => SurfaceAction::RenderThenReconfigure,
        SurfaceEvent::Outdated => SurfaceAction::Reconfigure,
        SurfaceEvent::Lost => SurfaceAction::Recreate,
        SurfaceEvent::Timeout => SurfaceAction::Skip,
        #[cfg(any(feature = "wgpu-29", feature = "wgpu-30", test))]
        SurfaceEvent::Occluded => SurfaceAction::Skip,
        #[cfg(any(feature = "wgpu-29", feature = "wgpu-30", test))]
        SurfaceEvent::Validation => SurfaceAction::Reject,
        #[cfg(any(feature = "wgpu-27", feature = "wgpu-28", test))]
        SurfaceEvent::OutOfMemory | SurfaceEvent::Other => SurfaceAction::Reject,
    }
}

unsafe fn handle_non_renderable_surface_event(
    event: SurfaceEvent,
    viewport: &Viewport,
    data: &mut ViewportWgpuData,
    globals: &GlobalHandles,
) {
    match surface_action(event) {
        SurfaceAction::Reconfigure => {
            let _ = unsafe { reconfigure_surface(viewport, data) };
        }
        SurfaceAction::Recreate => {
            let _ = unsafe { recreate_surface(viewport, data, globals) };
        }
        SurfaceAction::Skip => {}
        SurfaceAction::Reject => {
            eprintln!("[wgpu-mv] surface acquisition rejected: {event:?}");
        }
        SurfaceAction::Render | SurfaceAction::RenderThenReconfigure => {
            debug_assert!(false, "renderable surface event reached recovery path");
        }
    }
}

fn should_clear_viewport(flags: ViewportFlags) -> bool {
    !flags.contains(ViewportFlags::NO_RENDERER_CLEAR)
}

unsafe fn renderer_create_window(viewport: *mut Viewport) {
    let context = current_context();
    let Some(globals) = globals_for_current_context() else {
        return;
    };
    let viewport = unsafe { &mut *viewport };
    if !viewport.renderer_user_data().is_null() {
        return;
    }
    let Some(data) = (unsafe { create_viewport_data(context, viewport, &globals) }) else {
        return;
    };
    let pointer = Box::into_raw(Box::new(data));
    register_viewport_data(context, pointer);
    viewport.set_renderer_user_data(pointer.cast());
}

unsafe fn renderer_destroy_window(viewport: *mut Viewport) {
    unsafe { destroy_viewport_data(current_context(), &mut *viewport) };
}

unsafe fn renderer_set_window_size(viewport: *mut Viewport, size: dear_imgui_rs::sys::ImVec2) {
    let viewport = unsafe { &mut *viewport };
    let Some(pointer) = (unsafe { viewport_data_pointer(viewport) }) else {
        return;
    };
    let pixels = platform_adapter::logical_size_to_framebuffer(
        [size.x, size.y],
        viewport.framebuffer_scale(),
    );
    let data = unsafe { &mut *pointer };
    if data.config.width != pixels[0] || data.config.height != pixels[1] {
        data.config.width = pixels[0];
        data.config.height = pixels[1];
        data.surface.configure(&data.device, &data.config);
    }
}

unsafe fn renderer_render_window(viewport: *mut Viewport) {
    let Some(mut renderer) = (unsafe { borrow_renderer() }) else {
        return;
    };
    let Some(globals) = globals_for_current_context() else {
        return;
    };
    let Some(backend) = renderer.backend_data.as_ref() else {
        return;
    };
    let device = backend.device.clone();
    let queue = backend.queue.clone();
    let viewport = unsafe { &mut *viewport };
    let raw_draw_data = viewport.draw_data();
    if raw_draw_data.is_null() {
        return;
    }
    let Some(data_pointer) = (unsafe { viewport_data_pointer(viewport) }) else {
        return;
    };
    let data = unsafe { &mut *data_pointer };
    let draw_data = unsafe { dear_imgui_rs::render::DrawData::from_raw_mut(&mut *raw_draw_data) };

    #[cfg(any(feature = "wgpu-29", feature = "wgpu-30"))]
    let (frame, reconfigure_after_present) = match data.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(frame) => (
            frame,
            surface_action(SurfaceEvent::Success) == SurfaceAction::RenderThenReconfigure,
        ),
        wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (
            frame,
            surface_action(SurfaceEvent::Suboptimal) == SurfaceAction::RenderThenReconfigure,
        ),
        wgpu::CurrentSurfaceTexture::Outdated => {
            unsafe {
                handle_non_renderable_surface_event(
                    SurfaceEvent::Outdated,
                    viewport,
                    data,
                    &globals,
                )
            };
            return;
        }
        wgpu::CurrentSurfaceTexture::Lost => {
            unsafe {
                handle_non_renderable_surface_event(SurfaceEvent::Lost, viewport, data, &globals)
            };
            return;
        }
        wgpu::CurrentSurfaceTexture::Timeout => {
            unsafe {
                handle_non_renderable_surface_event(SurfaceEvent::Timeout, viewport, data, &globals)
            };
            return;
        }
        wgpu::CurrentSurfaceTexture::Occluded => {
            unsafe {
                handle_non_renderable_surface_event(
                    SurfaceEvent::Occluded,
                    viewport,
                    data,
                    &globals,
                )
            };
            return;
        }
        wgpu::CurrentSurfaceTexture::Validation => {
            unsafe {
                handle_non_renderable_surface_event(
                    SurfaceEvent::Validation,
                    viewport,
                    data,
                    &globals,
                )
            };
            return;
        }
    };

    #[cfg(any(feature = "wgpu-27", feature = "wgpu-28"))]
    let (frame, reconfigure_after_present) = match data.surface.get_current_texture() {
        Ok(frame) => {
            let event = if frame.suboptimal {
                SurfaceEvent::Suboptimal
            } else {
                SurfaceEvent::Success
            };
            (
                frame,
                surface_action(event) == SurfaceAction::RenderThenReconfigure,
            )
        }
        Err(wgpu::SurfaceError::Outdated) => {
            unsafe {
                handle_non_renderable_surface_event(
                    SurfaceEvent::Outdated,
                    viewport,
                    data,
                    &globals,
                )
            };
            return;
        }
        Err(wgpu::SurfaceError::Lost) => {
            unsafe {
                handle_non_renderable_surface_event(SurfaceEvent::Lost, viewport, data, &globals)
            };
            return;
        }
        Err(wgpu::SurfaceError::Timeout) => {
            unsafe {
                handle_non_renderable_surface_event(SurfaceEvent::Timeout, viewport, data, &globals)
            };
            return;
        }
        Err(wgpu::SurfaceError::OutOfMemory) => {
            unsafe {
                handle_non_renderable_surface_event(
                    SurfaceEvent::OutOfMemory,
                    viewport,
                    data,
                    &globals,
                )
            };
            return;
        }
        Err(wgpu::SurfaceError::Other) => {
            unsafe {
                handle_non_renderable_surface_event(SurfaceEvent::Other, viewport, data, &globals)
            };
            return;
        }
    };

    let view = frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("dear-imgui-wgpu::viewport-encoder"),
    });
    {
        let load = if should_clear_viewport(viewport.flags()) {
            wgpu::LoadOp::Clear(renderer.viewport_clear_color())
        } else {
            wgpu::LoadOp::Load
        };
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("dear-imgui-wgpu::viewport-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            #[cfg(any(feature = "wgpu-28", feature = "wgpu-29", feature = "wgpu-30"))]
            multiview_mask: None,
            timestamp_writes: None,
        });
        if let Err(error) = renderer.render_draw_data_with_fb_size_ex(
            draw_data,
            &mut render_pass,
            data.config.width,
            data.config.height,
            false,
            unsafe { dear_imgui_rs::sys::igGetPlatformIO_Nil() },
        ) {
            eprintln!("[wgpu-mv] viewport render failed: {error:?}");
            return;
        }
    }
    queue.submit(std::iter::once(encoder.finish()));
    data.pending_frame = Some(frame);
    data.pending_reconfigure = reconfigure_after_present;
}

unsafe fn renderer_swap_buffers(viewport: *mut Viewport) {
    let viewport = unsafe { &mut *viewport };
    let refreshed_size = unsafe { platform_adapter::framebuffer_size(viewport) };
    let Some(pointer) = (unsafe { viewport_data_pointer(viewport) }) else {
        return;
    };
    let data = unsafe { &mut *pointer };
    let Some(frame) = data.pending_frame.take() else {
        return;
    };
    #[cfg(feature = "wgpu-30")]
    data.queue.present(frame);
    #[cfg(not(feature = "wgpu-30"))]
    frame.present();
    if data.pending_reconfigure {
        if let Some(size) = refreshed_size {
            data.config.width = size[0].max(1);
            data.config.height = size[1].max(1);
        }
        data.surface.configure(&data.device, &data.config);
        data.pending_reconfigure = false;
    }
}

fn run_callback(name: &str, callback: impl FnOnce()) {
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(callback)).is_err() {
        eprintln!("[wgpu-mv] panic in {name}");
        std::process::abort();
    }
}

unsafe extern "C" fn renderer_create_window_sys(viewport: *mut dear_imgui_rs::sys::ImGuiViewport) {
    if viewport.is_null() {
        return;
    }
    run_callback("Renderer_CreateWindow", || unsafe {
        renderer_create_window(viewport.cast())
    });
}

unsafe extern "C" fn renderer_destroy_window_sys(viewport: *mut dear_imgui_rs::sys::ImGuiViewport) {
    if viewport.is_null() {
        return;
    }
    run_callback("Renderer_DestroyWindow", || unsafe {
        renderer_destroy_window(viewport.cast())
    });
}

// The pointer-based aggregate hook is intentional: passing ImVec2 by value is not ABI-compatible
// across every supported C++ MSVC target.
unsafe extern "C" fn renderer_set_window_size_sys(
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    size: *const dear_imgui_rs::sys::ImVec2,
) {
    if viewport.is_null() || size.is_null() {
        return;
    }
    run_callback("Renderer_SetWindowSize", || unsafe {
        renderer_set_window_size(viewport.cast(), *size)
    });
}

unsafe extern "C" fn renderer_render_window_sys(
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
    if viewport.is_null() {
        return;
    }
    run_callback("Renderer_RenderWindow", || unsafe {
        renderer_render_window(viewport.cast())
    });
}

unsafe extern "C" fn renderer_swap_buffers_sys(
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
    if viewport.is_null() {
        return;
    }
    run_callback("Renderer_SwapBuffers", || unsafe {
        renderer_swap_buffers(viewport.cast())
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr::NonNull;
    use std::sync::{Mutex as TestMutex, MutexGuard, OnceLock};

    fn lock_context() -> MutexGuard<'static, ()> {
        static GUARD: OnceLock<TestMutex<()>> = OnceLock::new();
        GUARD
            .get_or_init(|| TestMutex::new(()))
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    unsafe fn install_test_renderer(
        renderer: &mut WgpuRenderer,
        context: &mut Context,
    ) -> Result<(), CallbackOwnershipError> {
        let raw_context = context.as_raw();
        let _guard = unsafe { CurrentContextGuard::bind(raw_context) };
        validate_new_registration(raw_context, renderer)?;
        claim_callbacks(
            context.platform_io_mut(),
            dear_imgui_rs::sys::HAS_PLATFORM_IO_AGGREGATE_HOOKS,
        )?;
        insert_renderer_state(raw_context, renderer, None);
        renderer
            .multi_viewport_active
            .store(true, Ordering::Release);
        Ok(())
    }

    unsafe extern "C" fn unary_sentinel(_viewport: *mut dear_imgui_rs::sys::ImGuiViewport) {}

    unsafe extern "C" fn size_sentinel(
        _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
        _size: *const dear_imgui_rs::sys::ImVec2,
    ) {
    }

    unsafe extern "C" fn render_sentinel(
        _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
        _render_arg: *mut c_void,
    ) {
    }

    unsafe extern "C" fn platform_sentinel(
        _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
        _render_arg: *mut c_void,
    ) {
    }

    #[test]
    fn install_targets_passed_context_and_preserves_platform_slots() {
        let _lock = lock_context();
        let mut context_a = Context::create();
        let raw_a = context_a.as_raw();
        let platform_io_a = unsafe { dear_imgui_rs::sys::igGetPlatformIO_ContextPtr(raw_a) };
        unsafe {
            (*platform_io_a).Platform_RenderWindow = Some(platform_sentinel);
            (*platform_io_a).Platform_SwapBuffers = Some(platform_sentinel);
            dear_imgui_rs::sys::igSetCurrentContext(std::ptr::null_mut());
        }
        let context_b = Context::create();
        let raw_b = context_b.as_raw();
        let platform_io_b = unsafe { dear_imgui_rs::sys::igGetPlatformIO_ContextPtr(raw_b) };
        let mut renderer = Box::new(WgpuRenderer::empty());

        unsafe { install_test_renderer(&mut renderer, &mut context_a) }.unwrap();

        unsafe {
            assert_eq!(dear_imgui_rs::sys::igGetCurrentContext(), raw_b);
            assert!(callbacks_owned(context_a.platform_io()));
            assert!((*platform_io_b).Renderer_CreateWindow.is_none());
            assert!(render_callback_matches(
                (*platform_io_a).Platform_RenderWindow,
                platform_sentinel
            ));
            assert!(render_callback_matches(
                (*platform_io_a).Platform_SwapBuffers,
                platform_sentinel
            ));
        }

        disable_after_platform_shutdown(&mut context_a);
        unsafe {
            (*platform_io_a).Platform_RenderWindow = None;
            (*platform_io_a).Platform_SwapBuffers = None;
            dear_imgui_rs::sys::igSetCurrentContext(raw_a);
        }
        drop(context_a);
        unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw_b) };
        drop(context_b);
    }

    #[test]
    fn foreign_renderer_slots_are_rejected_without_mutation() {
        let _lock = lock_context();
        let mut context = Context::create();
        let platform_io = context.platform_io_mut();
        platform_io.set_renderer_create_window_raw(Some(unary_sentinel));
        platform_io.set_renderer_destroy_window_raw(Some(unary_sentinel));
        platform_io.set_renderer_set_window_size_raw(Some(size_sentinel));
        platform_io.set_renderer_render_window_raw(Some(render_sentinel));
        platform_io.set_renderer_swap_buffers_raw(Some(render_sentinel));
        let mut renderer = Box::new(WgpuRenderer::empty());

        assert_eq!(
            unsafe { install_test_renderer(&mut renderer, &mut context) },
            Err(CallbackOwnershipError::RendererCallbacksOccupied)
        );

        let platform_io = context.platform_io_mut();
        assert!(unary_callback_matches(
            platform_io.renderer_create_window_raw(),
            unary_sentinel
        ));
        assert!(unary_callback_matches(
            platform_io.renderer_destroy_window_raw(),
            unary_sentinel
        ));
        assert!(platform_io.renderer_set_window_size_matches_pointer_callback(size_sentinel));
        assert!(render_callback_matches(
            platform_io.renderer_render_window_raw(),
            render_sentinel
        ));
        assert!(render_callback_matches(
            platform_io.renderer_swap_buffers_raw(),
            render_sentinel
        ));
        platform_io.clear_renderer_handlers();
    }

    #[test]
    fn aggregate_hook_failure_does_not_install_partial_callbacks() {
        let _lock = lock_context();
        let mut context = Context::create();
        let raw = context.as_raw();
        assert_eq!(
            claim_callbacks(context.platform_io_mut(), false),
            Err(CallbackOwnershipError::AggregateCallbackHooksUnavailable)
        );
        assert!(context.platform_io().renderer_callbacks_are_empty());
        assert!(!has_renderer_state(raw));
    }

    #[test]
    fn foreign_renderer_user_data_preflight_is_transactional() {
        let _lock = lock_context();
        let context = Context::create();
        let foreign = 0x1234_usize as *mut c_void;

        assert_eq!(
            validate_secondary_viewports(&[(false, std::ptr::null_mut()), (false, foreign)]),
            Err(CallbackOwnershipError::RendererUserDataOccupied)
        );
        assert!(context.platform_io().renderer_callbacks_are_empty());
        assert!(!has_renderer_state(context.as_raw()));
    }

    #[test]
    fn existing_platform_window_preflight_is_transactional() {
        let _lock = lock_context();
        let context = Context::create();

        assert_eq!(
            validate_secondary_viewports(&[(true, std::ptr::null_mut())]),
            Err(CallbackOwnershipError::PlatformWindowsAlreadyCreated)
        );
        assert!(context.platform_io().renderer_callbacks_are_empty());
        assert!(!has_renderer_state(context.as_raw()));
    }

    #[test]
    fn public_enable_preflight_is_transactional() {
        let _lock = lock_context();
        let mut context = Context::create();
        let mut renderer = Box::new(WgpuRenderer::empty());
        let flags_before = context.io().backend_flags();

        assert_eq!(
            unsafe { enable(&mut renderer, &mut context) },
            Err(CallbackOwnershipError::RendererNotInitialized)
        );

        assert!(context.platform_io().renderer_callbacks_are_empty());
        assert_eq!(context.io().backend_flags(), flags_before);
        assert!(!has_renderer_state(context.as_raw()));
        assert!(!renderer.multi_viewport_active.load(Ordering::Acquire));
    }

    #[test]
    fn disable_preserves_callbacks_replaced_after_install() {
        let _lock = lock_context();
        let mut context = Context::create();
        let mut renderer = Box::new(WgpuRenderer::empty());
        unsafe { install_test_renderer(&mut renderer, &mut context) }.unwrap();
        let platform_io = context.platform_io_mut();
        platform_io.set_renderer_create_window_raw(Some(unary_sentinel));
        platform_io.set_renderer_destroy_window_raw(Some(unary_sentinel));
        platform_io.set_renderer_set_window_size_raw(Some(size_sentinel));
        platform_io.set_renderer_render_window_raw(Some(render_sentinel));
        platform_io.set_renderer_swap_buffers_raw(Some(render_sentinel));
        let io = context.io_mut();
        io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_VIEWPORTS);

        disable_after_platform_shutdown(&mut context);

        assert!(
            context
                .io()
                .backend_flags()
                .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
        );

        let platform_io = context.platform_io_mut();
        assert!(unary_callback_matches(
            platform_io.renderer_create_window_raw(),
            unary_sentinel
        ));
        assert!(unary_callback_matches(
            platform_io.renderer_destroy_window_raw(),
            unary_sentinel
        ));
        assert!(platform_io.renderer_set_window_size_matches_pointer_callback(size_sentinel));
        assert!(render_callback_matches(
            platform_io.renderer_render_window_raw(),
            render_sentinel
        ));
        assert!(render_callback_matches(
            platform_io.renderer_swap_buffers_raw(),
            render_sentinel
        ));
        platform_io.clear_renderer_handlers();
    }

    #[test]
    fn renderer_registry_is_context_local() {
        let _lock = lock_context();
        let mut context_a = Context::create();
        let raw_a = context_a.as_raw();
        let mut renderer_a = Box::new(WgpuRenderer::empty());
        unsafe { install_test_renderer(&mut renderer_a, &mut context_a) }.unwrap();
        unsafe { dear_imgui_rs::sys::igSetCurrentContext(std::ptr::null_mut()) };
        let mut context_b = Context::create();
        let raw_b = context_b.as_raw();
        let mut renderer_b = Box::new(WgpuRenderer::empty());
        unsafe { install_test_renderer(&mut renderer_b, &mut context_b) }.unwrap();

        unsafe {
            dear_imgui_rs::sys::igSetCurrentContext(raw_a);
            let borrowed = borrow_renderer().unwrap();
            assert_eq!(borrowed.renderer, (&mut *renderer_a) as *mut _);
            drop(borrowed);
            dear_imgui_rs::sys::igSetCurrentContext(raw_b);
            let borrowed = borrow_renderer().unwrap();
            assert_eq!(borrowed.renderer, (&mut *renderer_b) as *mut _);
            drop(borrowed);
            dear_imgui_rs::sys::igSetCurrentContext(std::ptr::null_mut());
            assert!(borrow_renderer().is_none());
        }

        unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw_b) };
        disable_after_platform_shutdown(&mut context_b);
        drop(context_b);
        unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw_a) };
        disable_after_platform_shutdown(&mut context_a);
    }

    #[test]
    fn nested_renderer_borrow_is_rejected_and_recovers_after_drop() {
        let _lock = lock_context();
        let mut context = Context::create();
        let raw = context.as_raw();
        let mut renderer = Box::new(WgpuRenderer::empty());
        unsafe { install_test_renderer(&mut renderer, &mut context) }.unwrap();
        unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw) };

        let first = unsafe { borrow_renderer() }.unwrap();
        assert!(unsafe { borrow_renderer() }.is_none());
        drop(first);
        assert!(unsafe { borrow_renderer() }.is_some());

        disable_after_platform_shutdown(&mut context);
    }

    #[test]
    fn renderer_rebind_is_rejected_while_callback_is_active() {
        let _lock = lock_context();
        let mut context = Context::create();
        let raw = context.as_raw();
        let mut first_renderer = Box::new(WgpuRenderer::empty());
        let mut next_renderer = Box::new(WgpuRenderer::empty());
        unsafe { install_test_renderer(&mut first_renderer, &mut context) }.unwrap();
        unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw) };
        let borrow = unsafe { borrow_renderer() }.unwrap();

        assert_eq!(
            unsafe { install_test_renderer(&mut next_renderer, &mut context) },
            Err(CallbackOwnershipError::RendererCallbackActive)
        );

        drop(borrow);
        disable_after_platform_shutdown(&mut context);
    }

    #[test]
    fn renderer_rebind_is_rejected_with_live_viewport_data() {
        let _lock = lock_context();
        let mut context = Context::create();
        let raw = context.as_raw();
        let mut first_renderer = Box::new(WgpuRenderer::empty());
        let mut next_renderer = Box::new(WgpuRenderer::empty());
        unsafe { install_test_renderer(&mut first_renderer, &mut context) }.unwrap();
        let pointer = NonNull::<ViewportWgpuData>::dangling().as_ptr();
        register_viewport_data(raw, pointer);

        assert_eq!(
            unsafe { install_test_renderer(&mut next_renderer, &mut context) },
            Err(CallbackOwnershipError::LiveViewportRendererRebind)
        );

        unregister_viewport_data(pointer);
        disable_after_platform_shutdown(&mut context);
    }

    #[test]
    fn repeated_registration_requires_full_shutdown() {
        let _lock = lock_context();
        let mut context = Context::create();
        let mut renderer = Box::new(WgpuRenderer::empty());
        unsafe { install_test_renderer(&mut renderer, &mut context) }.unwrap();

        assert_eq!(
            unsafe { install_test_renderer(&mut renderer, &mut context) },
            Err(CallbackOwnershipError::AlreadyEnabled)
        );

        disable_after_platform_shutdown(&mut context);
    }

    #[test]
    fn renderer_cannot_be_registered_with_two_contexts() {
        let _lock = lock_context();
        let mut context_a = Context::create();
        let raw_a = context_a.as_raw();
        let mut renderer = Box::new(WgpuRenderer::empty());
        unsafe { install_test_renderer(&mut renderer, &mut context_a) }.unwrap();
        unsafe { dear_imgui_rs::sys::igSetCurrentContext(std::ptr::null_mut()) };
        let mut context_b = Context::create();

        assert_eq!(
            unsafe { install_test_renderer(&mut renderer, &mut context_b) },
            Err(CallbackOwnershipError::RendererAlreadyRegistered)
        );
        assert!(context_b.platform_io().renderer_callbacks_are_empty());

        unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw_a) };
        disable_after_platform_shutdown(&mut context_a);
        drop(context_a);
        drop(context_b);
    }

    #[test]
    fn viewport_user_data_is_bound_to_its_owner_context() {
        let _lock = lock_context();
        let context_a = Context::create();
        let raw_a = context_a.as_raw();
        unsafe { dear_imgui_rs::sys::igSetCurrentContext(std::ptr::null_mut()) };
        let context_b = Context::create();
        let raw_b = context_b.as_raw();
        let pointer = NonNull::<ViewportWgpuData>::dangling().as_ptr();
        register_viewport_data(raw_a, pointer);
        let mut raw_viewport = dear_imgui_rs::sys::ImGuiViewport {
            RendererUserData: pointer.cast(),
            ..Default::default()
        };
        let viewport = unsafe { Viewport::from_raw_mut(&mut raw_viewport) };

        unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw_b) };
        assert!(unsafe { viewport_data_pointer(viewport) }.is_none());
        unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw_a) };
        assert_eq!(unsafe { viewport_data_pointer(viewport) }, Some(pointer));

        unregister_viewport_data(pointer);
        drop(context_a);
        unsafe { dear_imgui_rs::sys::igSetCurrentContext(raw_b) };
        drop(context_b);
    }

    #[test]
    fn destroy_ignores_foreign_renderer_user_data() {
        let _lock = lock_context();
        let context = Context::create();
        let raw = context.as_raw();
        let foreign = 0x1234_usize as *mut c_void;
        let mut raw_viewport = dear_imgui_rs::sys::ImGuiViewport {
            RendererUserData: foreign,
            ..Default::default()
        };
        unsafe {
            dear_imgui_rs::sys::igSetCurrentContext(raw);
            renderer_destroy_window(
                (&mut raw_viewport as *mut dear_imgui_rs::sys::ImGuiViewport).cast::<Viewport>(),
            );
        }
        assert_eq!(raw_viewport.RendererUserData, foreign);
    }

    #[test]
    fn disable_clears_runtime_state_and_renderer_capability() {
        let _lock = lock_context();
        let mut context = Context::create();
        let raw = context.as_raw();
        let mut renderer = Box::new(WgpuRenderer::empty());
        unsafe { install_test_renderer(&mut renderer, &mut context) }.unwrap();
        let io = context.io_mut();
        io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_VIEWPORTS);

        disable_after_platform_shutdown(&mut context);

        assert!(!has_renderer_state(raw));
        assert!(context.platform_io().renderer_callbacks_are_empty());
        assert!(!renderer.multi_viewport_active.load(Ordering::Acquire));
        assert!(
            !context
                .io()
                .backend_flags()
                .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
        );
    }

    #[test]
    fn renderer_lifecycle_mutation_is_blocked_until_runtime_shutdown() {
        let _lock = lock_context();
        let mut context = Context::create();
        let raw = context.as_raw();
        let mut renderer = Box::new(WgpuRenderer::empty());
        unsafe { install_test_renderer(&mut renderer, &mut context) }.unwrap();
        let io = context.io_mut();
        io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_VIEWPORTS);
        assert!(matches!(
            renderer.shutdown(),
            Err(crate::RendererError::MultiViewportActive)
        ));
        assert!(has_renderer_state(raw));
        disable_after_platform_shutdown(&mut context);
        assert!(!renderer.multi_viewport_active.load(Ordering::Acquire));
        assert!(renderer.ensure_multi_viewport_inactive().is_ok());
        assert!(renderer.shutdown().is_ok());
        assert!(!has_renderer_state(raw));
        assert!(
            !context
                .io()
                .backend_flags()
                .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
        );
    }

    #[test]
    fn surface_events_have_explicit_recovery_actions() {
        assert_eq!(surface_action(SurfaceEvent::Success), SurfaceAction::Render);
        assert_eq!(
            surface_action(SurfaceEvent::Suboptimal),
            SurfaceAction::RenderThenReconfigure
        );
        assert_eq!(
            surface_action(SurfaceEvent::Outdated),
            SurfaceAction::Reconfigure
        );
        assert_eq!(surface_action(SurfaceEvent::Lost), SurfaceAction::Recreate);
        assert_eq!(surface_action(SurfaceEvent::Timeout), SurfaceAction::Skip);
        assert_eq!(surface_action(SurfaceEvent::Occluded), SurfaceAction::Skip);
        assert_eq!(
            surface_action(SurfaceEvent::Validation),
            SurfaceAction::Reject
        );
        assert_eq!(
            surface_action(SurfaceEvent::OutOfMemory),
            SurfaceAction::Reject
        );
        assert_eq!(surface_action(SurfaceEvent::Other), SurfaceAction::Reject);
        assert!(should_clear_viewport(ViewportFlags::empty()));
        assert!(!should_clear_viewport(ViewportFlags::NO_RENDERER_CLEAR));
    }
}

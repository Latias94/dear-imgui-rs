use super::*;
use std::sync::Arc;

#[derive(Clone)]
pub(super) struct GlobalHandles {
    pub(super) entry: ash::Entry,
    pub(super) instance: ash::Instance,
    pub(super) physical_device: vk::PhysicalDevice,
    pub(super) present_queue: vk::Queue,
    pub(super) graphics_queue_family_index: u32,
    pub(super) present_queue_family_index: u32,
    pub(super) in_flight_frames: usize,
    pub(super) surface_adapter: Arc<dyn SurfaceAdapter>,
}

static RENDERERS: Mutex<Vec<ContextRendererState>> = Mutex::new(Vec::new());
static VIEWPORT_DATA: Mutex<Vec<ViewportDataRegistration>> = Mutex::new(Vec::new());

#[derive(Clone, Copy)]
struct ViewportDataRegistration {
    context: usize,
    data: usize,
}

struct ContextRendererState {
    pub(super) ctx: usize,
    pub(super) renderer: usize,
    borrowed: bool,
    global: Option<GlobalHandles>,
}

struct CurrentContextGuard {
    previous: *mut sys::ImGuiContext,
    target: *mut sys::ImGuiContext,
}

impl CurrentContextGuard {
    /// # Safety
    ///
    /// `target` must be null or a live context for the current thread. Neither it nor the context
    /// that is current on entry may be destroyed before the returned guard is dropped.
    unsafe fn bind(target: *mut sys::ImGuiContext) -> Self {
        // SAFETY: reading the current context does not dereference it; the caller owns both
        // contexts' lifetimes and thread-affinity while the guard is alive.
        let previous = unsafe { sys::igGetCurrentContext() };
        if previous != target {
            // SAFETY: the caller contract guarantees `target` remains live until guard drop.
            unsafe { sys::igSetCurrentContext(target) };
        }
        Self { previous, target }
    }
}

impl Drop for CurrentContextGuard {
    fn drop(&mut self) {
        if self.previous != self.target {
            // SAFETY: `bind` requires the previously current context to outlive this guard.
            unsafe { sys::igSetCurrentContext(self.previous) };
        }
    }
}

pub(super) fn insert_renderer_state(
    ctx: *mut sys::ImGuiContext,
    renderer: *mut AshRenderer,
    global: Option<GlobalHandles>,
) -> Result<(), CallbackOwnershipError> {
    if ctx.is_null() {
        return Err(CallbackOwnershipError::AlreadyEnabled);
    }

    let ctx = ctx as usize;
    let renderer = renderer as usize;
    let mut renderers = RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if renderers.iter().any(|entry| entry.ctx == ctx) {
        return Err(CallbackOwnershipError::AlreadyEnabled);
    }
    if renderers.iter().any(|entry| entry.renderer == renderer) {
        return Err(CallbackOwnershipError::RendererAlreadyRegistered);
    }

    renderers.push(ContextRendererState {
        ctx,
        renderer,
        borrowed: false,
        global,
    });
    Ok(())
}

pub(super) fn remove_renderer_state_for_context(ctx: *mut sys::ImGuiContext) {
    if ctx.is_null() {
        return;
    }

    let ctx = ctx as usize;
    RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .retain(|entry| entry.ctx != ctx);
}

pub(super) fn remove_renderer_state_for_renderer(renderer: *mut AshRenderer) {
    let renderer = renderer as usize;
    RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .retain(|entry| entry.renderer != renderer);
}

pub(super) fn register_viewport_data(ptr: *mut ViewportAshData) {
    if ptr.is_null() {
        return;
    }

    // SAFETY: this reads the current-context pointer without dereferencing it.
    let context = unsafe { sys::igGetCurrentContext() } as usize;
    if context == 0 {
        return;
    }
    let data = ptr as usize;
    let mut items = VIEWPORT_DATA
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if !items.iter().any(|item| item.data == data) {
        items.push(ViewportDataRegistration { context, data });
    }
}

pub(super) fn unregister_viewport_data(ptr: *mut ViewportAshData) {
    if ptr.is_null() {
        return;
    }

    let data = ptr as usize;
    VIEWPORT_DATA
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .retain(|entry| entry.data != data);
}

pub(super) fn is_ash_viewport_data(ptr: *mut ViewportAshData) -> bool {
    if ptr.is_null() {
        return false;
    }

    // SAFETY: this reads the current-context pointer without dereferencing it.
    let context = unsafe { sys::igGetCurrentContext() } as usize;
    let data = ptr as usize;
    VIEWPORT_DATA
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .iter()
        .any(|entry| entry.context == context && entry.data == data)
}

fn has_viewport_data_for_context(ctx: *mut sys::ImGuiContext) -> bool {
    let context = ctx as usize;
    context != 0
        && VIEWPORT_DATA
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .iter()
            .any(|entry| entry.context == context)
}

pub(super) fn global_handles() -> Option<GlobalHandles> {
    // SAFETY: this reads the current-context pointer without dereferencing it.
    let ctx = unsafe { sys::igGetCurrentContext() } as usize;
    if ctx == 0 {
        return None;
    }

    RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .iter()
        .find(|entry| entry.ctx == ctx)
        .and_then(|entry| entry.global.clone())
}

/// Failure to acquire the renderer callback table for Ash multi-viewport rendering.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CallbackOwnershipError {
    /// Another renderer backend still owns at least one `Renderer_*` callback slot.
    #[error("ImGuiPlatformIO renderer callbacks are already owned by another backend")]
    RendererCallbacksOccupied,

    /// An active runtime no longer owns the complete callback table required for teardown.
    #[error("Ash renderer callbacks were replaced while multi-viewport remained active")]
    RendererCallbacksReplaced,

    /// The current Dear ImGui artifact cannot safely bridge `Renderer_SetWindowSize`.
    #[error("dear-imgui-sys was built without PlatformIO aggregate ABI hooks")]
    AggregateCallbackHooksUnavailable,

    /// This context already has an active Ash viewport runtime.
    #[error("Ash multi-viewport rendering is already enabled for this ImGui context")]
    AlreadyEnabled,

    /// The same renderer cannot back callback registries for multiple contexts.
    #[error("this Ash renderer is already registered with another ImGui context")]
    RendererAlreadyRegistered,

    /// The configured physical device must be a valid non-null Vulkan handle.
    #[error("VulkanViewportConfig::physical_device must be non-null")]
    NullPhysicalDevice,

    /// The configured presentation queue must be a valid non-null Vulkan handle.
    #[error("VulkanViewportConfig::present_queue must be non-null")]
    NullPresentQueue,

    /// The configured graphics queue family does not exist on the physical device.
    #[error(
        "graphics queue family {queue_family_index} is out of range for {queue_family_count} queue families"
    )]
    GraphicsQueueFamilyOutOfRange {
        queue_family_index: u32,
        queue_family_count: usize,
    },

    /// The configured presentation queue family does not exist on the physical device.
    #[error(
        "present queue family {queue_family_index} is out of range for {queue_family_count} queue families"
    )]
    PresentQueueFamilyOutOfRange {
        queue_family_index: u32,
        queue_family_count: usize,
    },

    /// The configured graphics queue family cannot execute graphics commands.
    #[error("queue family {queue_family_index} does not support GRAPHICS commands")]
    GraphicsQueueFamilyUnsupported { queue_family_index: u32 },

    /// A configured queue family exposes no queues.
    #[error("queue family {queue_family_index} exposes no queues")]
    QueueFamilyEmpty { queue_family_index: u32 },

    /// Live secondary viewports still reference the current runtime.
    #[error("live Ash viewport resources must be destroyed before rebinding the renderer")]
    LiveViewportResources,

    /// SDL3 did not install its Vulkan surface callback.
    #[error("Platform_CreateVkSurface is not set by the SDL3 platform backend")]
    PlatformCreateVkSurfaceUnavailable,

    /// Renderer callbacks require a platform backend that already supports viewports.
    #[error("the active platform backend does not advertise multi-viewport support")]
    PlatformBackendUnavailable,

    /// An existing viewport is owned by another renderer backend.
    #[error("an existing viewport already has RendererUserData")]
    RendererUserDataOccupied,

    /// Existing platform windows would never receive the newly installed create callback.
    #[error(
        "secondary platform windows already exist; destroy them before enabling Ash multi-viewport"
    )]
    PlatformWindowsAlreadyCreated,

    /// The platform backend has not installed its required window lifecycle callbacks.
    #[error(
        "the platform backend must install Platform_CreateWindow and Platform_DestroyWindow before enabling the Ash viewport runtime"
    )]
    PlatformCallbacksUnavailable,

    /// The caller-provided main surface cannot support the configured viewport runtime.
    #[error(transparent)]
    SurfaceUnsupported(#[from] SurfaceSupportError),
}

/// Vulkan surface capability required by the viewport runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum SurfaceSupportError {
    #[error("the validation Vulkan surface must be non-null")]
    NullSurface,
    #[error("querying present support failed: {0:?}")]
    PresentSupportQuery(vk::Result),
    #[error("queue family {queue_family_index} cannot present to the validation surface")]
    PresentUnsupported { queue_family_index: u32 },
    #[error("querying surface capabilities failed: {0:?}")]
    CapabilitiesQuery(vk::Result),
    #[error("the surface does not support COLOR_ATTACHMENT swapchain images")]
    ColorAttachmentUnsupported,
    #[error("querying surface formats failed: {0:?}")]
    FormatsQuery(vk::Result),
    #[error("the surface reports no formats")]
    NoFormats,
    #[error("querying surface present modes failed: {0:?}")]
    PresentModesQuery(vk::Result),
    #[error("the surface reports no present modes")]
    NoPresentModes,
}

pub(super) struct SurfaceSupport {
    pub(super) capabilities: vk::SurfaceCapabilitiesKHR,
    pub(super) formats: Vec<vk::SurfaceFormatKHR>,
    pub(super) present_modes: Vec<vk::PresentModeKHR>,
}

pub(super) fn validate_vulkan_handles(
    physical_device: vk::PhysicalDevice,
    present_queue: vk::Queue,
) -> Result<(), CallbackOwnershipError> {
    if physical_device == vk::PhysicalDevice::null() {
        return Err(CallbackOwnershipError::NullPhysicalDevice);
    }
    if present_queue == vk::Queue::null() {
        return Err(CallbackOwnershipError::NullPresentQueue);
    }
    Ok(())
}

pub(super) fn validate_queue_family_selection(
    properties: &[vk::QueueFamilyProperties],
    graphics_queue_family_index: u32,
    present_queue_family_index: u32,
) -> Result<(), CallbackOwnershipError> {
    let Some(graphics) = properties.get(graphics_queue_family_index as usize) else {
        return Err(CallbackOwnershipError::GraphicsQueueFamilyOutOfRange {
            queue_family_index: graphics_queue_family_index,
            queue_family_count: properties.len(),
        });
    };
    let Some(present) = properties.get(present_queue_family_index as usize) else {
        return Err(CallbackOwnershipError::PresentQueueFamilyOutOfRange {
            queue_family_index: present_queue_family_index,
            queue_family_count: properties.len(),
        });
    };
    if graphics.queue_count == 0 {
        return Err(CallbackOwnershipError::QueueFamilyEmpty {
            queue_family_index: graphics_queue_family_index,
        });
    }
    if present.queue_count == 0 {
        return Err(CallbackOwnershipError::QueueFamilyEmpty {
            queue_family_index: present_queue_family_index,
        });
    }
    if !graphics.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
        return Err(CallbackOwnershipError::GraphicsQueueFamilyUnsupported {
            queue_family_index: graphics_queue_family_index,
        });
    }
    Ok(())
}

fn validate_vulkan_config(global: &GlobalHandles) -> Result<(), CallbackOwnershipError> {
    validate_vulkan_handles(global.physical_device, global.present_queue)?;
    // SAFETY: `GlobalHandles` can only be built by `enable_with_adapter`, whose safety contract
    // requires this physical device to be live and owned by `global.instance`.
    let queue_families = unsafe {
        global
            .instance
            .get_physical_device_queue_family_properties(global.physical_device)
    };
    validate_queue_family_selection(
        &queue_families,
        global.graphics_queue_family_index,
        global.present_queue_family_index,
    )
}

pub(super) fn query_surface_support(
    global: &GlobalHandles,
    surface: vk::SurfaceKHR,
) -> Result<SurfaceSupport, SurfaceSupportError> {
    if surface == vk::SurfaceKHR::null() {
        return Err(SurfaceSupportError::NullSurface);
    }

    let loader = khr_surface::Instance::new(&global.entry, &global.instance);
    // SAFETY: the enable contract requires the physical device, queue family, and validation
    // surface to be live handles created from this entry/instance pair.
    let present_supported = unsafe {
        loader.get_physical_device_surface_support(
            global.physical_device,
            global.present_queue_family_index,
            surface,
        )
    }
    .map_err(SurfaceSupportError::PresentSupportQuery)?;
    if !present_supported {
        return Err(SurfaceSupportError::PresentUnsupported {
            queue_family_index: global.present_queue_family_index,
        });
    }

    // SAFETY: the same-instance handle invariant described above remains valid for this query.
    let capabilities =
        unsafe { loader.get_physical_device_surface_capabilities(global.physical_device, surface) }
            .map_err(SurfaceSupportError::CapabilitiesQuery)?;
    if !capabilities
        .supported_usage_flags
        .contains(vk::ImageUsageFlags::COLOR_ATTACHMENT)
    {
        return Err(SurfaceSupportError::ColorAttachmentUnsupported);
    }

    // SAFETY: the physical device and surface are live and originate from `global.instance`.
    let formats =
        unsafe { loader.get_physical_device_surface_formats(global.physical_device, surface) }
            .map_err(SurfaceSupportError::FormatsQuery)?;
    if formats.is_empty() {
        return Err(SurfaceSupportError::NoFormats);
    }

    // SAFETY: the physical device and surface are live and originate from `global.instance`.
    let present_modes = unsafe {
        loader.get_physical_device_surface_present_modes(global.physical_device, surface)
    }
    .map_err(SurfaceSupportError::PresentModesQuery)?;
    if present_modes.is_empty() {
        return Err(SurfaceSupportError::NoPresentModes);
    }

    Ok(SurfaceSupport {
        capabilities,
        formats,
        present_modes,
    })
}

pub(super) fn has_renderer_state_for_context(ctx: *mut sys::ImGuiContext) -> bool {
    if ctx.is_null() {
        return false;
    }

    let ctx = ctx as usize;
    RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .iter()
        .any(|entry| entry.ctx == ctx)
}

fn validate_renderer_registration(
    ctx: *mut sys::ImGuiContext,
    renderer: *mut AshRenderer,
) -> Result<(), CallbackOwnershipError> {
    let ctx = ctx as usize;
    let renderer = renderer as usize;
    let renderers = RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if renderers
        .iter()
        .any(|entry| entry.renderer == renderer && entry.ctx != ctx)
    {
        Err(CallbackOwnershipError::RendererAlreadyRegistered)
    } else {
        Ok(())
    }
}

pub(super) fn unary_callback_matches(
    actual: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    expected: unsafe extern "C" fn(*mut sys::ImGuiViewport),
) -> bool {
    actual.is_some_and(|actual| std::ptr::fn_addr_eq(actual, expected))
}

pub(super) fn render_callback_matches(
    actual: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)>,
    expected: unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void),
) -> bool {
    actual.is_some_and(|actual| std::ptr::fn_addr_eq(actual, expected))
}

fn any_renderer_callback_owned(platform_io: &dear_imgui_rs::platform_io::PlatformIo) -> bool {
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

fn renderer_callbacks_owned(platform_io: &dear_imgui_rs::platform_io::PlatformIo) -> bool {
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

pub(super) fn try_install_renderer_callbacks(
    _ctx: *mut sys::ImGuiContext,
    platform_io: &mut dear_imgui_rs::platform_io::PlatformIo,
) -> Result<(), CallbackOwnershipError> {
    if !platform_io.renderer_callbacks_are_empty() {
        return Err(CallbackOwnershipError::RendererCallbacksOccupied);
    }

    if !sys::HAS_PLATFORM_IO_AGGREGATE_HOOKS {
        return Err(CallbackOwnershipError::AggregateCallbackHooksUnavailable);
    }

    platform_io.set_renderer_create_window_raw(Some(renderer_create_window_sys));
    platform_io.set_renderer_destroy_window_raw(Some(renderer_destroy_window_sys));
    platform_io.set_renderer_set_window_size_raw(Some(renderer_set_window_size_sys));
    platform_io.set_renderer_render_window_raw(Some(renderer_render_window_sys));
    platform_io.set_renderer_swap_buffers_raw(Some(renderer_swap_buffers_sys));
    Ok(())
}

pub(super) fn try_install_renderer_callbacks_after_preflight(
    ctx: *mut sys::ImGuiContext,
    platform_io: &mut dear_imgui_rs::platform_io::PlatformIo,
    preflight: impl FnOnce() -> Result<(), CallbackOwnershipError>,
) -> Result<(), CallbackOwnershipError> {
    if has_renderer_state_for_context(ctx) {
        if has_viewport_data_for_context(ctx) {
            return Err(CallbackOwnershipError::LiveViewportResources);
        }
        return Err(CallbackOwnershipError::AlreadyEnabled);
    }
    if has_viewport_data_for_context(ctx) {
        return Err(CallbackOwnershipError::LiveViewportResources);
    }
    if !platform_io.renderer_callbacks_are_empty() {
        return Err(CallbackOwnershipError::RendererCallbacksOccupied);
    }
    if !sys::HAS_PLATFORM_IO_AGGREGATE_HOOKS {
        return Err(CallbackOwnershipError::AggregateCallbackHooksUnavailable);
    }
    preflight()?;
    try_install_renderer_callbacks(ctx, platform_io)
}

pub(super) fn validate_platform_callbacks(
    platform_io: &dear_imgui_rs::platform_io::PlatformIo,
) -> Result<(), CallbackOwnershipError> {
    // SAFETY: `PlatformIo::as_raw` points to the table borrowed by `platform_io` for this call.
    let raw = unsafe { &*platform_io.as_raw() };
    if raw.Platform_CreateWindow.is_none() || raw.Platform_DestroyWindow.is_none() {
        return Err(CallbackOwnershipError::PlatformCallbacksUnavailable);
    }
    Ok(())
}

pub(super) fn validate_platform_backend(context: &Context) -> Result<(), CallbackOwnershipError> {
    if context
        .io()
        .backend_flags()
        .contains(BackendFlags::PLATFORM_HAS_VIEWPORTS)
    {
        Ok(())
    } else {
        Err(CallbackOwnershipError::PlatformBackendUnavailable)
    }
}

pub(super) fn validate_empty_renderer_user_data(
    slots: impl IntoIterator<Item = *mut c_void>,
) -> Result<(), CallbackOwnershipError> {
    if slots.into_iter().any(|slot| !slot.is_null()) {
        Err(CallbackOwnershipError::RendererUserDataOccupied)
    } else {
        Ok(())
    }
}

pub(super) fn validate_no_created_platform_windows(
    created: impl IntoIterator<Item = bool>,
) -> Result<(), CallbackOwnershipError> {
    if created.into_iter().any(|created| created) {
        Err(CallbackOwnershipError::PlatformWindowsAlreadyCreated)
    } else {
        Ok(())
    }
}

pub(super) fn clear_renderer_callbacks(platform_io: &mut dear_imgui_rs::platform_io::PlatformIo) {
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
    let _ = platform_io
        .clear_renderer_set_window_size_if_pointer_callback(renderer_set_window_size_sys);
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
}

/// # Safety
///
/// Every Vulkan handle in `config` must be live, mutually compatible, and remain valid until the
/// viewport runtime is shut down. `renderer` must stay at a stable address, and callbacks must run
/// on the thread that owns `imgui_context`.
pub(crate) unsafe fn enable_with_adapter(
    renderer: &mut AshRenderer,
    imgui_context: &mut Context,
    config: VulkanViewportConfig,
    surface_adapter: Arc<dyn SurfaceAdapter>,
) -> Result<(), CallbackOwnershipError> {
    let context_raw = imgui_context.as_raw();
    // SAFETY: the mutable context borrow proves `context_raw` is live for the guard, and the caller
    // guarantees its thread-affinity and that the previously current context is not destroyed.
    let _context_guard = unsafe { CurrentContextGuard::bind(context_raw) };
    let renderer_raw = renderer as *mut _;

    let global = GlobalHandles {
        entry: config.entry,
        instance: config.instance,
        physical_device: config.physical_device,
        present_queue: config.present_queue,
        graphics_queue_family_index: config.graphics_queue_family_index,
        present_queue_family_index: config.present_queue_family_index,
        in_flight_frames: renderer.options.in_flight_frames.max(1),
        surface_adapter,
    };

    validate_renderer_registration(context_raw, renderer_raw)?;
    validate_platform_backend(imgui_context)?;
    validate_platform_callbacks(imgui_context.platform_io())?;
    validate_empty_renderer_user_data(
        imgui_context
            .platform_io()
            .viewports_iter()
            .map(Viewport::renderer_user_data),
    )?;
    validate_no_created_platform_windows(
        imgui_context
            .platform_io()
            .viewports_iter()
            .skip(1)
            .map(Viewport::platform_window_created),
    )?;
    try_install_renderer_callbacks_after_preflight(
        context_raw,
        imgui_context.platform_io_mut(),
        || {
            validate_vulkan_config(&global)?;
            query_surface_support(&global, config.validation_surface)?;
            Ok(())
        },
    )?;

    if let Err(error) = insert_renderer_state(context_raw, renderer_raw, Some(global)) {
        clear_renderer_callbacks(imgui_context.platform_io_mut());
        return Err(error);
    }
    let io = imgui_context.io_mut();
    io.set_backend_flags(io.backend_flags() | dear_imgui_rs::BackendFlags::RENDERER_HAS_VIEWPORTS);
    Ok(())
}

pub(crate) fn clear_for_drop(renderer: *mut AshRenderer) {
    remove_renderer_state_for_renderer(renderer);
}

pub(crate) fn disable(imgui_context: &mut Context) -> Result<(), CallbackOwnershipError> {
    // SAFETY: the mutable context borrow keeps this context live; disable does not destroy either
    // it or the previously current context before the guard restores that pointer.
    let _context_guard = unsafe { CurrentContextGuard::bind(imgui_context.as_raw()) };

    if has_viewport_data_for_context(imgui_context.as_raw()) {
        return Err(CallbackOwnershipError::LiveViewportResources);
    }

    let had_state = has_renderer_state_for_context(imgui_context.as_raw());
    let had_owned_callbacks = any_renderer_callback_owned(imgui_context.platform_io());
    clear_renderer_callbacks(imgui_context.platform_io_mut());
    remove_renderer_state_for_context(imgui_context.as_raw());
    if (had_state || had_owned_callbacks)
        && imgui_context.platform_io().renderer_callbacks_are_empty()
    {
        let io = imgui_context.io_mut();
        io.set_backend_flags(io.backend_flags() & !BackendFlags::RENDERER_HAS_VIEWPORTS);
    }
    Ok(())
}

/// Convenience helper that destroys all platform windows and disables callbacks.
pub fn shutdown_multi_viewport_support(
    context: &mut Context,
) -> Result<(), CallbackOwnershipError> {
    // SAFETY: the mutable context borrow keeps this context live through platform-window teardown
    // and restoration of the previously current context.
    let _context_guard = unsafe { CurrentContextGuard::bind(context.as_raw()) };
    if !has_renderer_state_for_context(context.as_raw()) {
        return Ok(());
    }
    if !renderer_callbacks_owned(context.platform_io()) {
        return Err(CallbackOwnershipError::RendererCallbacksReplaced);
    }
    context.destroy_platform_windows();
    disable(context)
}

#[allow(unsafe_op_in_unsafe_fn)]
pub(super) unsafe fn borrow_renderer() -> Option<RendererBorrowGuard> {
    // SAFETY: this reads the current-context pointer without dereferencing it. Registry lookup and
    // the borrow flag validate ownership before any renderer pointer is dereferenced.
    let ctx = unsafe { sys::igGetCurrentContext() } as usize;
    if ctx == 0 {
        return None;
    }

    let mut renderers = RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let entry = renderers.iter_mut().find(|entry| entry.ctx == ctx)?;
    if entry.renderer == 0 {
        return None;
    }
    if entry.borrowed {
        eprintln!("[ash-mv] renderer already mutably borrowed; skipping callback");
        return None;
    }

    entry.borrowed = true;
    Some(RendererBorrowGuard {
        ctx,
        renderer: entry.renderer as *mut AshRenderer,
    })
}

pub(super) struct RendererBorrowGuard {
    pub(super) ctx: usize,
    pub(super) renderer: *mut AshRenderer,
}

impl Drop for RendererBorrowGuard {
    fn drop(&mut self) {
        let mut renderers = RENDERERS
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(entry) = renderers
            .iter_mut()
            .find(|entry| entry.ctx == self.ctx && entry.renderer == self.renderer as usize)
        {
            entry.borrowed = false;
        }
    }
}

impl std::ops::Deref for RendererBorrowGuard {
    type Target = AshRenderer;

    fn deref(&self) -> &Self::Target {
        // SAFETY: registration requires a stable renderer address, and this guard owns the
        // context-local callback borrow until `Drop` clears the registry flag.
        unsafe { &*self.renderer }
    }
}

impl std::ops::DerefMut for RendererBorrowGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: the registry permits only one active mutable renderer borrow for this context.
        unsafe { &mut *self.renderer }
    }
}

#[allow(unsafe_op_in_unsafe_fn)]
pub(super) unsafe fn viewport_user_data_mut(vpm: &mut Viewport) -> Option<&mut ViewportAshData> {
    let data = vpm.renderer_user_data();
    let data = data as *mut ViewportAshData;
    if !is_ash_viewport_data(data) {
        None
    } else {
        // SAFETY: the context-local registry proves this live pointer came from `Box::into_raw`,
        // and renderer callbacks serialize mutable access to viewport user data.
        Some(&mut *data)
    }
}

#[allow(unsafe_op_in_unsafe_fn)]
pub(super) unsafe fn take_viewport_data(vpm: &mut Viewport) -> Option<Box<ViewportAshData>> {
    let data = vpm.renderer_user_data() as *mut ViewportAshData;
    if !is_ash_viewport_data(data) {
        return None;
    }

    unregister_viewport_data(data);
    vpm.set_renderer_user_data(std::ptr::null_mut());
    // SAFETY: registry ownership proves `data` came from `Box::into_raw`; unregistering and
    // clearing the viewport slot transfer that allocation exactly once back into this Box.
    Some(Box::from_raw(data))
}

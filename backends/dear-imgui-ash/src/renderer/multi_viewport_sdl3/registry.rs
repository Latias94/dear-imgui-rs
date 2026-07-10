use super::*;

#[derive(Clone)]
pub(super) struct GlobalHandles {
    pub(super) entry: ash::Entry,
    pub(super) instance: ash::Instance,
    pub(super) physical_device: vk::PhysicalDevice,
    pub(super) present_queue: vk::Queue,
    pub(super) graphics_queue_family_index: u32,
    pub(super) present_queue_family_index: u32,
    pub(super) in_flight_frames: usize,
    pub(super) platform_create_vk_surface: PlatformCreateVkSurfaceFn,
}

static RENDERERS: Mutex<Vec<ContextRendererState>> = Mutex::new(Vec::new());
static VIEWPORT_DATA: Mutex<Vec<usize>> = Mutex::new(Vec::new());

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
    unsafe fn bind(target: *mut sys::ImGuiContext) -> Self {
        let previous = unsafe { sys::igGetCurrentContext() };
        if previous != target {
            unsafe { sys::igSetCurrentContext(target) };
        }
        Self { previous, target }
    }
}

impl Drop for CurrentContextGuard {
    fn drop(&mut self) {
        if self.previous != self.target {
            unsafe { sys::igSetCurrentContext(self.previous) };
        }
    }
}

pub(super) fn upsert_renderer_state(
    ctx: *mut sys::ImGuiContext,
    renderer: *mut AshRenderer,
    global: Option<GlobalHandles>,
) {
    if ctx.is_null() {
        return;
    }

    let ctx = ctx as usize;
    let renderer = renderer as usize;
    let mut renderers = RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if let Some(entry) = renderers.iter_mut().find(|entry| entry.ctx == ctx) {
        entry.renderer = renderer;
        entry.global = global;
        return;
    }

    renderers.push(ContextRendererState {
        ctx,
        renderer,
        borrowed: false,
        global,
    });
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

    let ptr = ptr as usize;
    let mut items = VIEWPORT_DATA
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if !items.contains(&ptr) {
        items.push(ptr);
    }
}

fn unregister_viewport_data(ptr: *mut ViewportAshData) {
    if ptr.is_null() {
        return;
    }

    let ptr = ptr as usize;
    VIEWPORT_DATA
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .retain(|entry| *entry != ptr);
}

fn is_ash_viewport_data(ptr: *mut ViewportAshData) -> bool {
    if ptr.is_null() {
        return false;
    }

    let ptr = ptr as usize;
    VIEWPORT_DATA
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .contains(&ptr)
}

pub(super) fn global_handles() -> Option<GlobalHandles> {
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

    /// The SDL3 platform backend did not provide its Vulkan surface callback.
    #[error("Platform_CreateVkSurface is not set by the SDL3 platform backend")]
    PlatformCreateVkSurfaceUnavailable,

    /// The current Dear ImGui artifact cannot safely bridge `Renderer_SetWindowSize`.
    #[error("dear-imgui-sys was built without PlatformIO aggregate ABI hooks")]
    AggregateCallbackHooksUnavailable,
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

fn renderer_callbacks_are_owned_by_ash(
    platform_io: &dear_imgui_rs::platform_io::PlatformIo,
) -> bool {
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
    ctx: *mut sys::ImGuiContext,
    platform_io: &mut dear_imgui_rs::platform_io::PlatformIo,
) -> Result<(), CallbackOwnershipError> {
    if !platform_io.renderer_callbacks_are_empty() {
        if has_renderer_state_for_context(ctx) && renderer_callbacks_are_owned_by_ash(platform_io) {
            return Ok(());
        }
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

/// Enable Vulkan multi-viewport (SDL3): installs renderer callbacks.
pub fn enable(
    renderer: &mut AshRenderer,
    imgui_context: &mut Context,
    entry: ash::Entry,
    instance: ash::Instance,
    physical_device: vk::PhysicalDevice,
    present_queue: vk::Queue,
    graphics_queue_family_index: u32,
    present_queue_family_index: u32,
) -> Result<(), CallbackOwnershipError> {
    let context_raw = imgui_context.as_raw();
    let _context_guard = unsafe { CurrentContextGuard::bind(context_raw) };

    let platform_create_vk_surface = {
        let platform_io = imgui_context.platform_io_mut();
        let callback = platform_io
            .platform_create_vk_surface_raw()
            .ok_or(CallbackOwnershipError::PlatformCreateVkSurfaceUnavailable)?;
        try_install_renderer_callbacks(context_raw, platform_io)?;
        callback
    };

    upsert_renderer_state(
        context_raw,
        renderer as *mut _,
        Some(GlobalHandles {
            entry,
            instance,
            physical_device,
            present_queue,
            graphics_queue_family_index,
            present_queue_family_index,
            in_flight_frames: renderer.options.in_flight_frames.max(1),
            platform_create_vk_surface,
        }),
    );
    Ok(())
}

pub(crate) fn clear_for_drop(renderer: *mut AshRenderer) {
    remove_renderer_state_for_renderer(renderer);
}

/// Disable multi-viewport callbacks and clear stored globals.
pub fn disable(imgui_context: &mut Context) {
    let _context_guard = unsafe { CurrentContextGuard::bind(imgui_context.as_raw()) };

    clear_renderer_callbacks(imgui_context.platform_io_mut());
    remove_renderer_state_for_context(imgui_context.as_raw());
}

/// Convenience helper that destroys all platform windows and disables callbacks.
pub fn shutdown_multi_viewport_support(context: &mut Context) {
    context.destroy_platform_windows();
    disable(context);
}

pub(super) unsafe fn borrow_renderer() -> Option<RendererBorrowGuard> {
    let ctx = unsafe { sys::igGetCurrentContext() } as usize;
    if ctx == 0 {
        return None;
    }

    let mut renderers = RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let Some(entry) = renderers.iter_mut().find(|entry| entry.ctx == ctx) else {
        return None;
    };
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
        unsafe { &*self.renderer }
    }
}

impl std::ops::DerefMut for RendererBorrowGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.renderer }
    }
}

#[allow(unsafe_op_in_unsafe_fn)]
pub(super) unsafe fn viewport_user_data_mut<'a>(
    vpm: &'a mut Viewport,
) -> Option<&'a mut ViewportAshData> {
    let data = vpm.renderer_user_data();
    let data = data as *mut ViewportAshData;
    if !is_ash_viewport_data(data) {
        None
    } else {
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
    Some(Box::from_raw(data))
}

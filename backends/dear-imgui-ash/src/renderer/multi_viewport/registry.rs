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
}

static RENDERERS: Mutex<Vec<ContextRendererState>> = Mutex::new(Vec::new());

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

/// Enable Vulkan multi-viewport (winit): installs renderer callbacks.
pub fn enable(
    renderer: &mut AshRenderer,
    imgui_context: &mut Context,
    entry: ash::Entry,
    instance: ash::Instance,
    physical_device: vk::PhysicalDevice,
    present_queue: vk::Queue,
    graphics_queue_family_index: u32,
    present_queue_family_index: u32,
) {
    let _context_guard = unsafe { CurrentContextGuard::bind(imgui_context.as_raw()) };

    unsafe {
        let platform_io = imgui_context.platform_io_mut();
        platform_io.set_renderer_create_window(Some(
            renderer_create_window as unsafe extern "C" fn(*mut Viewport),
        ));
        platform_io.set_renderer_destroy_window(Some(
            renderer_destroy_window as unsafe extern "C" fn(*mut Viewport),
        ));
        platform_io.set_renderer_set_window_size(Some(
            renderer_set_window_size
                as unsafe extern "C" fn(*mut Viewport, dear_imgui_rs::sys::ImVec2),
        ));
        platform_io.set_platform_render_window_raw(Some(platform_render_window_sys));
        platform_io.set_platform_swap_buffers_raw(Some(platform_swap_buffers_sys));
    }

    upsert_renderer_state(
        imgui_context.as_raw(),
        renderer as *mut _,
        Some(GlobalHandles {
            entry,
            instance,
            physical_device,
            present_queue,
            graphics_queue_family_index,
            present_queue_family_index,
            in_flight_frames: renderer.options.in_flight_frames.max(1),
        }),
    );
}

pub(crate) fn clear_for_drop(renderer: *mut AshRenderer) {
    remove_renderer_state_for_renderer(renderer);
}

/// Disable multi-viewport callbacks and clear stored globals.
pub fn disable(imgui_context: &mut Context) {
    let _context_guard = unsafe { CurrentContextGuard::bind(imgui_context.as_raw()) };

    unsafe {
        let platform_io = imgui_context.platform_io_mut();
        platform_io.set_renderer_create_window(None);
        platform_io.set_renderer_destroy_window(None);
        platform_io.set_renderer_set_window_size(None);
        platform_io.set_platform_render_window_raw(None);
        platform_io.set_platform_swap_buffers_raw(None);
    }
    remove_renderer_state_for_context(imgui_context.as_raw());
}

/// Convenience helper that destroys all platform windows and disables callbacks.
pub fn shutdown_multi_viewport_support(context: &mut Context) {
    context.destroy_platform_windows();
    disable(context);
}

#[allow(unsafe_op_in_unsafe_fn)]
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
    if data.is_null() {
        None
    } else {
        Some(&mut *(data as *mut ViewportAshData))
    }
}

// Multi-viewport support (Renderer_* callbacks and helpers)

use super::*;
use dear_imgui_rs::internal::RawCast;
use dear_imgui_rs::platform_io::Viewport;
use std::ffi::c_void;
use std::ops::{Deref, DerefMut};
use std::sync::Mutex;
#[cfg(not(target_arch = "wasm32"))]
use winit::window::Window;

/// Per-viewport WGPU data stored in ImGuiViewport::RendererUserData
struct ViewportWgpuData {
    pub device: wgpu::Device,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    pub pending_frame: Option<wgpu::SurfaceTexture>,
    pub pending_reconfigure: bool,
    // Last values we logged for this viewport (to avoid per-frame spam).
    #[cfg(feature = "mv-log")]
    pub last_log_display_size: [f32; 2],
    #[cfg(feature = "mv-log")]
    pub last_log_fb_scale: [f32; 2],
}

static RENDERERS: Mutex<Vec<ContextRendererState>> = Mutex::new(Vec::new());

struct ContextRendererState {
    ctx: usize,
    renderer: usize,
    borrowed: bool,
    global: Option<GlobalHandles>,
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

#[derive(Clone)]
struct GlobalHandles {
    instance: Option<wgpu::Instance>,
    adapter: Option<wgpu::Adapter>,
    device: wgpu::Device,
    render_target_format: wgpu::TextureFormat,
}

fn upsert_renderer_state(
    ctx: *mut dear_imgui_rs::sys::ImGuiContext,
    renderer: *mut WgpuRenderer,
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

fn remove_renderer_state_for_context(ctx: *mut dear_imgui_rs::sys::ImGuiContext) {
    if ctx.is_null() {
        return;
    }

    let ctx = ctx as usize;
    RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .retain(|entry| entry.ctx != ctx);
}

fn remove_renderer_state_for_renderer(renderer: *mut WgpuRenderer) {
    let renderer = renderer as usize;
    RENDERERS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .retain(|entry| entry.renderer != renderer);
}

fn global_handles() -> Option<GlobalHandles> {
    let ctx = unsafe { dear_imgui_rs::sys::igGetCurrentContext() } as usize;
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

/// Enable WGPU multi-viewport: set per-viewport callbacks and capture renderer pointer
pub fn enable(renderer: &mut WgpuRenderer, imgui_context: &mut Context) {
    let _context_guard = unsafe { CurrentContextGuard::bind(imgui_context.as_raw()) };

    // Expose callbacks through PlatformIO
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
        // Route rendering via platform raw callbacks to avoid typed trampolines
        platform_io.set_platform_render_window_raw(Some(platform_render_window_sys));
        platform_io.set_platform_swap_buffers_raw(Some(platform_swap_buffers_sys));
    }

    let global = renderer.backend_data.as_ref().map(|backend| GlobalHandles {
        instance: backend.instance.clone(),
        adapter: backend.adapter.clone(),
        device: backend.device.clone(),
        render_target_format: backend.render_target_format,
    });
    upsert_renderer_state(imgui_context.as_raw(), renderer as *mut _, global);
}

pub(crate) fn clear_for_drop(renderer: *mut WgpuRenderer) {
    remove_renderer_state_for_renderer(renderer);
}

/// Disable WGPU multi-viewport callbacks and clear stored globals.
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

/// Convenience helper that disables callbacks and destroys all platform windows.
pub fn shutdown_multi_viewport_support(context: &mut Context) {
    disable(context);
    context.destroy_platform_windows();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enable_targets_passed_context() {
        let mut ctx_a = Context::create();
        let raw_a = ctx_a.as_raw();
        let pio_a = unsafe { dear_imgui_rs::sys::igGetPlatformIO_ContextPtr(raw_a) };

        unsafe {
            dear_imgui_rs::sys::igSetCurrentContext(std::ptr::null_mut());
        }

        let ctx_b = Context::create();
        let raw_b = ctx_b.as_raw();
        let pio_b = unsafe { dear_imgui_rs::sys::igGetPlatformIO_ContextPtr(raw_b) };

        let mut renderer = WgpuRenderer::empty();
        enable(&mut renderer, &mut ctx_a);

        unsafe {
            assert_eq!(dear_imgui_rs::sys::igGetCurrentContext(), raw_b);
            assert!((*pio_a).Renderer_CreateWindow.is_some());
            assert!((*pio_a).Renderer_DestroyWindow.is_some());
            assert!((*pio_a).Renderer_SetWindowSize.is_some());
            assert!((*pio_a).Platform_RenderWindow.is_some());
            assert!((*pio_a).Platform_SwapBuffers.is_some());

            assert!((*pio_b).Renderer_CreateWindow.is_none());
            assert!((*pio_b).Renderer_DestroyWindow.is_none());
            assert!((*pio_b).Renderer_SetWindowSize.is_none());
            assert!((*pio_b).Platform_RenderWindow.is_none());
            assert!((*pio_b).Platform_SwapBuffers.is_none());
        }

        disable(&mut ctx_a);

        unsafe {
            assert_eq!(dear_imgui_rs::sys::igGetCurrentContext(), raw_b);
            assert!((*pio_a).Renderer_CreateWindow.is_none());
            assert!((*pio_a).Renderer_DestroyWindow.is_none());
            assert!((*pio_a).Renderer_SetWindowSize.is_none());
            assert!((*pio_a).Platform_RenderWindow.is_none());
            assert!((*pio_a).Platform_SwapBuffers.is_none());
        }

        unsafe {
            dear_imgui_rs::sys::igSetCurrentContext(raw_a);
        }
        drop(ctx_a);
        unsafe {
            dear_imgui_rs::sys::igSetCurrentContext(raw_b);
        }
        drop(ctx_b);
        drop(renderer);
    }

    #[test]
    fn renderer_state_is_context_local() {
        let mut ctx_a = Context::create();
        let raw_a = ctx_a.as_raw();
        let mut renderer_a = WgpuRenderer::empty();
        enable(&mut renderer_a, &mut ctx_a);

        unsafe {
            dear_imgui_rs::sys::igSetCurrentContext(std::ptr::null_mut());
        }

        let mut ctx_b = Context::create();
        let raw_b = ctx_b.as_raw();
        let mut renderer_b = WgpuRenderer::empty();
        enable(&mut renderer_b, &mut ctx_b);

        unsafe {
            dear_imgui_rs::sys::igSetCurrentContext(raw_a);
            {
                let borrowed = borrow_renderer().expect("renderer for context A");
                assert_eq!((&*borrowed.renderer) as *const _, &renderer_a as *const _);
            }

            dear_imgui_rs::sys::igSetCurrentContext(raw_b);
            {
                let borrowed = borrow_renderer().expect("renderer for context B");
                assert_eq!((&*borrowed.renderer) as *const _, &renderer_b as *const _);
            }
        }

        unsafe {
            dear_imgui_rs::sys::igSetCurrentContext(raw_b);
        }
        disable(&mut ctx_b);
        drop(ctx_b);
        drop(renderer_b);

        unsafe {
            dear_imgui_rs::sys::igSetCurrentContext(raw_a);
        }
        disable(&mut ctx_a);
        drop(ctx_a);
        drop(renderer_a);
    }
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn borrow_renderer() -> Option<RendererBorrowGuard> {
    let ctx = unsafe { dear_imgui_rs::sys::igGetCurrentContext() } as usize;
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
        eprintln!("[wgpu-mv] renderer already mutably borrowed; skipping callback");
        return None;
    }

    entry.borrowed = true;
    Some(RendererBorrowGuard {
        ctx,
        renderer: entry.renderer as *mut WgpuRenderer,
    })
}

/// Helper to get or create per-viewport user data
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn viewport_user_data_mut<'a>(vpm: &'a mut Viewport) -> Option<&'a mut ViewportWgpuData> {
    let data = vpm.renderer_user_data();
    if data.is_null() {
        None
    } else {
        Some(&mut *(data as *mut ViewportWgpuData))
    }
}

struct RendererBorrowGuard {
    ctx: usize,
    renderer: *mut WgpuRenderer,
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

/// Renderer: create per-viewport resources (surface + config)
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_create_window(vp: *mut Viewport) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        mvlog!("[wgpu-mv] Renderer_CreateWindow");

        let Some(global) = global_handles() else {
            return;
        };

        // Obtain window from platform handle
        let vpm = &mut *vp;
        let window_ptr = vpm.platform_handle();
        if window_ptr.is_null() {
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        let window: &Window = &*(window_ptr as *const Window);

        #[cfg(not(target_arch = "wasm32"))]
        let instance = match &global.instance {
            Some(i) => i.clone(),
            None => return, // cannot create surfaces without instance
        };

        #[cfg(not(target_arch = "wasm32"))]
        let surface: wgpu::Surface<'static> = {
            // SAFETY: the underlying OS window (from ImGui PlatformHandle) is guaranteed by the
            // platform backend to outlive the corresponding ImGui viewport, and Dear ImGui calls
            // `Renderer_DestroyWindow` before the platform destroys the window. Therefore the raw
            // window/display handles remain valid for the lifetime of this surface.
            let target = match wgpu::SurfaceTargetUnsafe::from_window(window) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("[wgpu-mv] create_surface handle error: {:?}", e);
                    return;
                }
            };
            match instance.create_surface_unsafe(target) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[wgpu-mv] create_surface error: {:?}", e);
                    return;
                }
            }
        };

        #[cfg(not(target_arch = "wasm32"))]
        let size = window.inner_size();
        #[cfg(not(target_arch = "wasm32"))]
        let width = size.width.max(1);
        #[cfg(not(target_arch = "wasm32"))]
        let height = size.height.max(1);

        #[cfg(not(target_arch = "wasm32"))]
        let config = {
            // Prefer the renderer's main format if the surface supports it; otherwise, bail out gracefully
            if let Some(adapter) = &global.adapter {
                let caps = surface.get_capabilities(adapter);
                let format = if caps.formats.contains(&global.render_target_format) {
                    global.render_target_format
                } else {
                    // If the main pipeline format isn't supported, we cannot render safely with this pipeline.
                    eprintln!(
                        "[wgpu-mv] Surface doesn't support pipeline format {:?}; supported: {:?}. Skipping configure.",
                        global.render_target_format, caps.formats
                    );
                    return;
                };
                let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
                    wgpu::PresentMode::Fifo
                } else {
                    // Fallback to first supported present mode
                    caps.present_modes
                        .get(0)
                        .cloned()
                        .unwrap_or(wgpu::PresentMode::Fifo)
                };
                let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
                    wgpu::CompositeAlphaMode::Opaque
                } else if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Auto) {
                    wgpu::CompositeAlphaMode::Auto
                } else {
                    caps.alpha_modes
                        .get(0)
                        .cloned()
                        .unwrap_or(wgpu::CompositeAlphaMode::Opaque)
                };
                wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format,
                    width,
                    height,
                    present_mode,
                    alpha_mode,
                    view_formats: vec![format],
                    desired_maximum_frame_latency: 1,
                }
            } else {
                // No adapter available: assume the same format as main and attempt configure (best-effort)
                wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format: global.render_target_format,
                    width,
                    height,
                    present_mode: wgpu::PresentMode::Fifo,
                    alpha_mode: wgpu::CompositeAlphaMode::Opaque,
                    view_formats: vec![global.render_target_format],
                    desired_maximum_frame_latency: 1,
                }
            }
        };

        #[cfg(not(target_arch = "wasm32"))]
        {
            // Configure with validated config
            surface.configure(&global.device, &config);
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let data = ViewportWgpuData {
                device: global.device.clone(),
                surface,
                config,
                pending_frame: None,
                pending_reconfigure: false,
                #[cfg(feature = "mv-log")]
                last_log_display_size: [0.0, 0.0],
                #[cfg(feature = "mv-log")]
                last_log_fb_scale: [0.0, 0.0],
            };
            vpm.set_renderer_user_data(Box::into_raw(Box::new(data)) as *mut c_void);
        }
    }));
    if res.is_err() {
        eprintln!("[wgpu-mv] panic in Renderer_CreateWindow");
        std::process::abort();
    }
}

/// Renderer: destroy per-viewport resources
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_destroy_window(vp: *mut Viewport) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        mvlog!("[wgpu-mv] Renderer_DestroyWindow");
        let vpm = &mut *vp;
        let data = vpm.renderer_user_data();
        if !data.is_null() {
            let _boxed: Box<ViewportWgpuData> = Box::from_raw(data as *mut ViewportWgpuData);
            vpm.set_renderer_user_data(std::ptr::null_mut());
        }
    }));
    if res.is_err() {
        eprintln!("[wgpu-mv] panic in Renderer_DestroyWindow");
        std::process::abort();
    }
}

/// Renderer: notify new size
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_set_window_size(
    vp: *mut Viewport,
    size: dear_imgui_rs::sys::ImVec2,
) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        mvlog!(
            "[wgpu-mv] Renderer_SetWindowSize to ({}, {})",
            size.x,
            size.y
        );
        let Some(global) = global_handles() else {
            return;
        };
        let vpm = &mut *vp;
        let scale = vpm.framebuffer_scale();
        if let Some(data) = viewport_user_data_mut(vpm) {
            // Winit multi-viewport uses logical screen coordinates for viewport sizes.
            // Convert to physical pixels for WGPU surfaces using framebuffer scale.
            let sx = if scale[0].is_finite() && scale[0] > 0.0 {
                scale[0]
            } else {
                1.0
            };
            let sy = if scale[1].is_finite() && scale[1] > 0.0 {
                scale[1]
            } else {
                1.0
            };
            let new_w = (size.x * sx).max(1.0).round() as u32;
            let new_h = (size.y * sy).max(1.0).round() as u32;
            if data.config.width != new_w || data.config.height != new_h {
                data.config.width = new_w;
                data.config.height = new_h;
                data.surface.configure(&global.device, &data.config);
            }
        }
    }));
    if res.is_err() {
        eprintln!("[wgpu-mv] panic in Renderer_SetWindowSize");
        std::process::abort();
    }
}

/// Renderer: render viewport draw data into its surface
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_render_window(vp: *mut Viewport, _render_arg: *mut c_void) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        let Some(mut renderer) = borrow_renderer() else {
            return;
        };
        // Clone device/queue to avoid borrowing renderer during render
        let (device, queue) = match renderer.backend_data.as_ref() {
            Some(b) => (b.device.clone(), b.queue.clone()),
            None => return,
        };
        // Obtain draw data for this viewport
        let vpm = &mut *vp;
        #[cfg(feature = "mv-log")]
        let vp_id = vpm.id();
        #[cfg(not(target_arch = "wasm32"))]
        let platform_handle = vpm.platform_handle();
        let raw_dd = vpm.draw_data();
        if raw_dd.is_null() {
            // No draw data for this viewport (e.g. minimized or empty)
            return;
        }
        // SAFETY: Dear ImGui guarantees that during RenderPlatformWindowsDefault, draw_data()
        // returns a valid ImDrawData pointer for each viewport being rendered.
        let draw_data: &dear_imgui_rs::render::DrawData =
            dear_imgui_rs::render::DrawData::from_raw(&*raw_dd);
        if let Some(data) = viewport_user_data_mut(vpm) {
            // Targeted debug log: only print on size/scale change or mismatch.
            #[cfg(feature = "mv-log")]
            {
                let disp = draw_data.display_size();
                let fb_scale = draw_data.framebuffer_scale();
                let expected_w = (disp[0] * fb_scale[0]).round().max(0.0) as u32;
                let expected_h = (disp[1] * fb_scale[1]).round().max(0.0) as u32;
                let cfg_w = data.config.width;
                let cfg_h = data.config.height;
                let mismatch = expected_w != cfg_w || expected_h != cfg_h;
                let disp_changed = (disp[0] - data.last_log_display_size[0]).abs() > 0.5
                    || (disp[1] - data.last_log_display_size[1]).abs() > 0.5;
                let scale_changed = (fb_scale[0] - data.last_log_fb_scale[0]).abs() > 0.01
                    || (fb_scale[1] - data.last_log_fb_scale[1]).abs() > 0.01;
                if mismatch || disp_changed || scale_changed {
                    mvlog!(
                        "[wgpu-mv] vp={} disp=({:.1},{:.1}) fb_scale=({:.2},{:.2}) expected_fb=({},{}) cfg_fb=({},{}) mismatch={}",
                        vp_id,
                        disp[0],
                        disp[1],
                        fb_scale[0],
                        fb_scale[1],
                        expected_w,
                        expected_h,
                        cfg_w,
                        cfg_h,
                        mismatch
                    );
                    data.last_log_display_size = disp;
                    data.last_log_fb_scale = fb_scale;
                }
            }

            let fb_w = data.config.width;
            let fb_h = data.config.height;
            // Acquire frame with basic recovery on Outdated/Lost/Timeout
            #[cfg(feature = "wgpu-29")]
            let (frame, reconfigure_after_present) = match data.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
                wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    // Reconfigure with current window size and retry next frame
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        if !platform_handle.is_null() {
                            let window: &winit::window::Window = &*(platform_handle as *const _);
                            let size = window.inner_size();
                            if size.width > 0 && size.height > 0 {
                                data.config.width = size.width;
                                data.config.height = size.height;
                                data.surface.configure(&device, &data.config);
                            }
                        }
                    }
                    return;
                }
                wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                    // Skip this frame silently
                    return;
                }
                wgpu::CurrentSurfaceTexture::Validation => {
                    eprintln!("[wgpu-mv] get_current_texture failed with a validation error");
                    return;
                }
            };
            #[cfg(any(feature = "wgpu-27", feature = "wgpu-28"))]
            let (frame, reconfigure_after_present) = match data.surface.get_current_texture() {
                Ok(frame) => (frame, false),
                Err(wgpu::SurfaceError::Outdated) | Err(wgpu::SurfaceError::Lost) => {
                    // Reconfigure with current window size and retry next frame
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        if !platform_handle.is_null() {
                            let window: &winit::window::Window = &*(platform_handle as *const _);
                            let size = window.inner_size();
                            if size.width > 0 && size.height > 0 {
                                data.config.width = size.width;
                                data.config.height = size.height;
                                data.surface.configure(&data.device, &data.config);
                            }
                        }
                    }
                    return;
                }
                Err(wgpu::SurfaceError::Timeout) => {
                    // Skip this frame silently
                    return;
                }
                Err(e) => {
                    eprintln!("[wgpu-mv] get_current_texture error: {:?}", e);
                    return;
                }
            };
            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            // Encode commands and render (catch panics to avoid crashing the whole app)
            let render_block = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("dear-imgui-wgpu::viewport-encoder"),
                });
                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("dear-imgui-wgpu::viewport-pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(renderer.viewport_clear_color()),
                                store: wgpu::StoreOp::Store,
                            },
                            depth_slice: None,
                        })],
                        depth_stencil_attachment: None,
                        occlusion_query_set: None,
                        #[cfg(any(feature = "wgpu-28", feature = "wgpu-29"))]
                        multiview_mask: None,
                        timestamp_writes: None,
                    });
                    // Reuse existing draw path with explicit framebuffer size override, but do
                    // not advance frame index: we already advanced it for the main window.
                    if let Err(e) = renderer.render_draw_data_with_fb_size_ex(
                        &draw_data,
                        &mut render_pass,
                        fb_w,
                        fb_h,
                        false,
                        dear_imgui_rs::sys::igGetPlatformIO_Nil(),
                    ) {
                        eprintln!("[wgpu-mv] render_draw_data(with_fb) error: {:?}", e);
                    }
                }
                queue.submit(std::iter::once(encoder.finish()));
            }));
            if render_block.is_err() {
                eprintln!(
                    "[wgpu-mv] panic during viewport render block; skipping present for this viewport"
                );
                return;
            }
            data.pending_frame = Some(frame);
            data.pending_reconfigure = reconfigure_after_present;
        }
    }));
    if res.is_err() {
        eprintln!("[wgpu-mv] panic in Renderer_RenderWindow");
        std::process::abort();
    }
}

/// Renderer: present frame for viewport surface
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `Viewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn renderer_swap_buffers(vp: *mut Viewport, _render_arg: *mut c_void) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        let vpm = &mut *vp;
        if let Some(data) = viewport_user_data_mut(vpm) {
            if let Some(frame) = data.pending_frame.take() {
                frame.present();
                if data.pending_reconfigure {
                    data.surface.configure(&data.device, &data.config);
                    data.pending_reconfigure = false;
                }
            }
        }
    }));
    if res.is_err() {
        eprintln!("[wgpu-mv] panic in Renderer_SwapBuffers");
        std::process::abort();
    }
}

// Raw sys-platform wrappers to avoid typed trampolines
///
/// # Safety
///
/// Called by Dear ImGui from C with a valid `ImGuiViewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn platform_render_window_sys(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    arg: *mut c_void,
) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        renderer_render_window(vp as *mut Viewport, arg);
    }));
    if res.is_err() {
        eprintln!("[wgpu-mv] panic in Platform_RenderWindow");
        std::process::abort();
    }
}

/// # Safety
///
/// Called by Dear ImGui from C with a valid `ImGuiViewport*` belonging to the current ImGui context.
#[allow(unsafe_op_in_unsafe_fn)]
pub unsafe extern "C" fn platform_swap_buffers_sys(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    arg: *mut c_void,
) {
    if vp.is_null() {
        return;
    }
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        renderer_swap_buffers(vp as *mut Viewport, arg);
    }));
    if res.is_err() {
        eprintln!("[wgpu-mv] panic in Platform_SwapBuffers");
        std::process::abort();
    }
}

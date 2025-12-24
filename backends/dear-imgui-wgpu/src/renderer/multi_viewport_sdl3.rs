// Multi-viewport support (SDL3 platform backend + WGPU renderer)
//
// This mirrors the existing winit multi-viewport renderer callbacks, but reads
// ImGuiViewport::PlatformHandle as an SDL_WindowID and creates per-viewport
// WGPU surfaces from SDL3 native window handles.

use super::*;
use dear_imgui_rs::internal::RawCast;
use dear_imgui_rs::platform_io::Viewport;
use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use super::sdl3_raw_window_handle::Sdl3SurfaceTarget;

#[cfg(target_arch = "wasm32")]
compile_error!("`multi-viewport-sdl3` is not supported on wasm32 targets.");

/// Per-viewport WGPU data stored in ImGuiViewport::RendererUserData
pub struct ViewportWgpuData {
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    pub pending_frame: Option<wgpu::SurfaceTexture>,
}

static RENDERER_PTR: AtomicUsize = AtomicUsize::new(0);
static GLOBAL: Mutex<Option<GlobalHandles>> = Mutex::new(None);

#[derive(Clone)]
struct GlobalHandles {
    instance: Option<wgpu::Instance>,
    adapter: Option<wgpu::Adapter>,
    device: wgpu::Device,
    render_target_format: wgpu::TextureFormat,
}

/// Enable WGPU multi-viewport: set per-viewport callbacks and capture renderer pointer.
pub fn enable(renderer: &mut WgpuRenderer, imgui_context: &mut Context) {
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

    RENDERER_PTR.store(renderer as *mut _ as usize, Ordering::SeqCst);

    if let Some(backend) = renderer.backend_data.as_ref() {
        let mut g = GLOBAL.lock().unwrap_or_else(|poison| poison.into_inner());
        *g = Some(GlobalHandles {
            instance: backend.instance.clone(),
            adapter: backend.adapter.clone(),
            device: backend.device.clone(),
            render_target_format: backend.render_target_format,
        });
    }
}

/// Disable WGPU multi-viewport callbacks and clear stored globals (SDL3 platform).
pub fn disable(imgui_context: &mut Context) {
    unsafe {
        let platform_io = imgui_context.platform_io_mut();
        platform_io.set_renderer_create_window(None);
        platform_io.set_renderer_destroy_window(None);
        platform_io.set_renderer_set_window_size(None);
        platform_io.set_platform_render_window_raw(None);
        platform_io.set_platform_swap_buffers_raw(None);
    }
    RENDERER_PTR.store(0, Ordering::SeqCst);
    let mut g = GLOBAL.lock().unwrap_or_else(|poison| poison.into_inner());
    *g = None;
}

/// Convenience helper that disables callbacks and destroys all platform windows.
pub fn shutdown_multi_viewport_support(context: &mut Context) {
    disable(context);
    context.destroy_platform_windows();
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn get_renderer<'a>() -> Option<&'a mut WgpuRenderer> {
    let ptr = RENDERER_PTR.load(Ordering::SeqCst) as *mut WgpuRenderer;
    ptr.as_mut()
}

unsafe fn viewport_user_data_mut<'a>(vp: *mut Viewport) -> Option<&'a mut ViewportWgpuData> {
    let vpm = unsafe { &mut *vp };
    let data = vpm.renderer_user_data();
    if data.is_null() {
        None
    } else {
        Some(unsafe { &mut *(data as *mut ViewportWgpuData) })
    }
}

fn sdl_window_id_from_viewport(vp: &Viewport) -> Option<sdl3_sys::video::SDL_WindowID> {
    let handle = vp.platform_handle();
    if handle.is_null() {
        None
    } else {
        Some(handle as usize as sdl3_sys::video::SDL_WindowID)
    }
}

fn clamp_i32_pixels_to_u32(pixels: i32) -> u32 {
    if pixels > 0 { pixels as u32 } else { 1 }
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
        mvlog!("[wgpu-mv-sdl3] Renderer_CreateWindow");

        let global = {
            let g = GLOBAL.lock().unwrap_or_else(|poison| poison.into_inner());
            match g.as_ref() {
                Some(g) => g.clone(),
                None => return,
            }
        };

        let vpm = &mut *vp;
        let window_id = match sdl_window_id_from_viewport(vpm) {
            Some(id) => id,
            None => return,
        };

        let target = match Sdl3SurfaceTarget::from_window_id(window_id) {
            Some(t) => t,
            None => return,
        };

        let instance = match &global.instance {
            Some(i) => i.clone(),
            None => return,
        };

        let surface: wgpu::Surface<'static> = {
            // SAFETY: the underlying SDL window and display remain valid for the lifetime of the
            // corresponding ImGui viewport, and Dear ImGui calls `Renderer_DestroyWindow` before
            // the platform destroys the window. Therefore the raw handles remain valid until this
            // surface is dropped.
            let surface_target = match wgpu::SurfaceTargetUnsafe::from_window(&target) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("[wgpu-mv-sdl3] create_surface handle error: {:?}", e);
                    return;
                }
            };
            match instance.create_surface_unsafe(surface_target) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[wgpu-mv-sdl3] create_surface error: {:?}", e);
                    return;
                }
            }
        };

        // Query initial pixel size from SDL3.
        let mut w: i32 = 0;
        let mut h: i32 = 0;
        let raw_window = target.raw_window();
        let ok = sdl3_sys::video::SDL_GetWindowSizeInPixels(raw_window, &mut w, &mut h);
        if !ok {
            return;
        }
        let width = clamp_i32_pixels_to_u32(w);
        let height = clamp_i32_pixels_to_u32(h);

        let config = if let Some(adapter) = &global.adapter {
            let caps = surface.get_capabilities(adapter);
            let format = if caps.formats.contains(&global.render_target_format) {
                global.render_target_format
            } else {
                eprintln!(
                    "[wgpu-mv-sdl3] Surface doesn't support pipeline format {:?}; supported: {:?}. Skipping configure.",
                    global.render_target_format, caps.formats
                );
                return;
            };
            let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
                wgpu::PresentMode::Fifo
            } else {
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
        };

        surface.configure(&global.device, &config);

        let data = ViewportWgpuData {
            surface,
            config,
            pending_frame: None,
        };
        vpm.set_renderer_user_data(Box::into_raw(Box::new(data)) as *mut c_void);
    }));
    if res.is_err() {
        eprintln!("[wgpu-mv-sdl3] panic in Renderer_CreateWindow");
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
        mvlog!("[wgpu-mv-sdl3] Renderer_DestroyWindow");
        let vpm = &mut *vp;
        let data = vpm.renderer_user_data();
        if !data.is_null() {
            let _boxed: Box<ViewportWgpuData> = Box::from_raw(data as *mut ViewportWgpuData);
            vpm.set_renderer_user_data(std::ptr::null_mut());
        }
    }));
    if res.is_err() {
        eprintln!("[wgpu-mv-sdl3] panic in Renderer_DestroyWindow");
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
            "[wgpu-mv-sdl3] Renderer_SetWindowSize to ({}, {})",
            size.x,
            size.y
        );
        let global = {
            let g = GLOBAL.lock().unwrap_or_else(|poison| poison.into_inner());
            match g.as_ref() {
                Some(g) => g.clone(),
                None => return,
            }
        };
        if let Some(data) = viewport_user_data_mut(vp) {
            let vpm_ref = &*vp;
            let scale = vpm_ref.framebuffer_scale();
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
        eprintln!("[wgpu-mv-sdl3] panic in Renderer_SetWindowSize");
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
        mvlog!("[wgpu-mv-sdl3] Renderer_RenderWindow");

        let Some(renderer) = get_renderer() else {
            return;
        };
        let (device, queue) = match renderer.backend_data.as_ref() {
            Some(b) => (b.device.clone(), b.queue.clone()),
            None => return,
        };

        let vpm = &mut *vp;
        let raw_dd = vpm.draw_data();
        if raw_dd.is_null() {
            return;
        }
        // SAFETY: Dear ImGui guarantees `raw_dd` is valid during render callbacks.
        let draw_data: &dear_imgui_rs::render::DrawData =
            dear_imgui_rs::render::DrawData::from_raw(&*raw_dd);

        if let Some(data) = viewport_user_data_mut(vp) {
            let frame = match data.surface.get_current_texture() {
                Ok(f) => f,
                Err(wgpu::SurfaceError::Outdated) | Err(wgpu::SurfaceError::Lost) => {
                    // Reconfigure from actual SDL3 pixel size and retry next frame.
                    if let Some(window_id) = sdl_window_id_from_viewport(&*vp) {
                        if let Some(target) = Sdl3SurfaceTarget::from_window_id(window_id) {
                            let raw_window = target.raw_window();
                            let mut w: i32 = 0;
                            let mut h: i32 = 0;
                            if sdl3_sys::video::SDL_GetWindowSizeInPixels(
                                raw_window, &mut w, &mut h,
                            ) {
                                let w = clamp_i32_pixels_to_u32(w);
                                let h = clamp_i32_pixels_to_u32(h);
                                data.config.width = w;
                                data.config.height = h;
                                data.surface.configure(&device, &data.config);
                            }
                        }
                    }
                    return;
                }
                Err(wgpu::SurfaceError::Timeout) => return,
                Err(e) => {
                    eprintln!("[wgpu-mv-sdl3] get_current_texture error: {:?}", e);
                    return;
                }
            };

            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

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
                        multiview_mask: None,
                        timestamp_writes: None,
                    });

                    if let Some(vd) = viewport_user_data_mut(vp) {
                        let fb_w = vd.config.width;
                        let fb_h = vd.config.height;
                        if let Err(e) = renderer.render_draw_data_with_fb_size_ex(
                            &draw_data,
                            &mut render_pass,
                            fb_w,
                            fb_h,
                            false,
                        ) {
                            eprintln!("[wgpu-mv-sdl3] render_draw_data(with_fb) error: {:?}", e);
                        }
                    } else if let Err(e) = renderer.render_draw_data(&draw_data, &mut render_pass) {
                        eprintln!("[wgpu-mv-sdl3] render_draw_data error: {:?}", e);
                    }
                }
                queue.submit(std::iter::once(encoder.finish()));
            }));

            if render_block.is_err() {
                eprintln!(
                    "[wgpu-mv-sdl3] panic during viewport render block; skipping present for this viewport"
                );
                return;
            }

            data.pending_frame = Some(frame);
        }
    }));
    if res.is_err() {
        eprintln!("[wgpu-mv-sdl3] panic in Renderer_RenderWindow");
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
        mvlog!("[wgpu-mv-sdl3] Renderer_SwapBuffers");
        if let Some(data) = viewport_user_data_mut(vp) {
            if let Some(frame) = data.pending_frame.take() {
                frame.present();
            }
        }
    }));
    if res.is_err() {
        eprintln!("[wgpu-mv-sdl3] panic in Renderer_SwapBuffers");
        std::process::abort();
    }
}

/// Raw sys-platform wrapper to avoid typed trampolines.
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
        eprintln!("[wgpu-mv-sdl3] panic in Platform_RenderWindow");
        std::process::abort();
    }
}

/// Raw sys-platform wrapper to avoid typed trampolines.
///
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
        eprintln!("[wgpu-mv-sdl3] panic in Platform_SwapBuffers");
        std::process::abort();
    }
}

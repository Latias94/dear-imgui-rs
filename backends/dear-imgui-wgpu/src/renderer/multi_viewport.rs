// Multi-viewport support (Renderer_* callbacks and helpers)

use super::*;
use dear_imgui_rs::internal::RawCast;
use dear_imgui_rs::platform_io::Viewport;
use std::ffi::c_void;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(not(target_arch = "wasm32"))]
use winit::window::Window;

/// Per-viewport WGPU data stored in ImGuiViewport::RendererUserData
pub struct ViewportWgpuData {
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    pub pending_frame: Option<wgpu::SurfaceTexture>,
}

static RENDERER_PTR: AtomicUsize = AtomicUsize::new(0);
static GLOBAL: OnceLock<GlobalHandles> = OnceLock::new();

struct GlobalHandles {
    instance: Option<wgpu::Instance>,
    adapter: Option<wgpu::Adapter>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    render_target_format: wgpu::TextureFormat,
}

/// Enable WGPU multi-viewport: set per-viewport callbacks and capture renderer pointer
pub fn enable(renderer: &mut WgpuRenderer, imgui_context: &mut Context) {
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

    // Store self pointer for callbacks
    RENDERER_PTR.store(renderer as *mut _ as usize, Ordering::SeqCst);

    // Also store global handles so creation/resizing callbacks don't rely on renderer pointer stability
    if let Some(backend) = renderer.backend_data.as_ref() {
        let _ = GLOBAL.set(GlobalHandles {
            instance: backend.instance.clone(),
            adapter: backend.adapter.clone(),
            device: backend.device.clone(),
            queue: backend.queue.clone(),
            render_target_format: backend.render_target_format,
        });
    }
}

unsafe fn get_renderer<'a>() -> &'a mut WgpuRenderer {
    let ptr = RENDERER_PTR.load(Ordering::SeqCst) as *mut WgpuRenderer;
    &mut *ptr
}

/// Helper to get or create per-viewport user data
unsafe fn viewport_user_data_mut<'a>(vp: *mut Viewport) -> Option<&'a mut ViewportWgpuData> {
    let vpm = &mut *vp;
    let data = vpm.renderer_user_data();
    if data.is_null() {
        None
    } else {
        Some(&mut *(data as *mut ViewportWgpuData))
    }
}

/// Renderer: create per-viewport resources (surface + config)
pub unsafe extern "C" fn renderer_create_window(vp: *mut Viewport) {
    if vp.is_null() {
        return;
    }
    mvlog!("[wgpu-mv] Renderer_CreateWindow");

    let global = match GLOBAL.get() {
        Some(g) => g,
        None => return,
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
    let surface = match instance.create_surface(window) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[wgpu-mv] create_surface error: {:?}", e);
            return;
        }
    };
    // Extend surface lifetime to 'static by tying it to backend-owned instance
    let surface: wgpu::Surface<'static> = std::mem::transmute(surface);

    #[cfg(not(target_arch = "wasm32"))]
    let size = window.inner_size();
    #[cfg(not(target_arch = "wasm32"))]
    let width = size.width.max(1);
    #[cfg(not(target_arch = "wasm32"))]
    let height = size.height.max(1);

    #[cfg(not(target_arch = "wasm32"))]
    let mut config = {
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
            surface,
            config,
            pending_frame: None,
        };
        vpm.set_renderer_user_data(Box::into_raw(Box::new(data)) as *mut c_void);
    }
}

/// Renderer: destroy per-viewport resources
pub unsafe extern "C" fn renderer_destroy_window(vp: *mut Viewport) {
    if vp.is_null() {
        return;
    }
    mvlog!("[wgpu-mv] Renderer_DestroyWindow");
    let vpm = &mut *vp;
    let data = vpm.renderer_user_data();
    if !data.is_null() {
        let _boxed: Box<ViewportWgpuData> = Box::from_raw(data as *mut ViewportWgpuData);
        vpm.set_renderer_user_data(std::ptr::null_mut());
    }
}

/// Renderer: notify new size
pub unsafe extern "C" fn renderer_set_window_size(
    vp: *mut Viewport,
    size: dear_imgui_rs::sys::ImVec2,
) {
    if vp.is_null() {
        return;
    }
    mvlog!(
        "[wgpu-mv] Renderer_SetWindowSize to ({}, {})",
        size.x,
        size.y
    );
    let global = match GLOBAL.get() {
        Some(g) => g,
        None => return,
    };
    if let Some(data) = viewport_user_data_mut(vp) {
        // Convert ImGui logical size to physical pixels using framebuffer scale
        let vpm_ref = &*vp;
        let scale = vpm_ref.framebuffer_scale();
        let new_w = (size.x * scale[0]).max(1.0) as u32;
        let new_h = (size.y * scale[1]).max(1.0) as u32;
        if data.config.width != new_w || data.config.height != new_h {
            data.config.width = new_w;
            data.config.height = new_h;
            data.surface.configure(&global.device, &data.config);
        }
    }
}

/// Renderer: render viewport draw data into its surface
pub unsafe extern "C" fn renderer_render_window(vp: *mut Viewport, _render_arg: *mut c_void) {
    if vp.is_null() {
        return;
    }
    mvlog!("[wgpu-mv] Renderer_RenderWindow");
    let renderer = match (get_renderer() as *mut WgpuRenderer).as_mut() {
        Some(r) => r,
        None => return,
    };
    // Clone device/queue to avoid borrowing renderer during render
    let (device, queue) = match renderer.backend_data.as_ref() {
        Some(b) => (b.device.clone(), b.queue.clone()),
        None => return,
    };
    // Obtain draw data for this viewport
    let vpm = unsafe { &mut *vp };
    let raw_dd = vpm.draw_data();
    if raw_dd.is_null() {
        // No draw data for this viewport (e.g. minimized or empty)
        return;
    }
    // SAFETY: Dear ImGui guarantees that during RenderPlatformWindowsDefault, draw_data()
    // returns a valid ImDrawData pointer for each viewport being rendered.
    let draw_data: &dear_imgui_rs::render::DrawData =
        unsafe { dear_imgui_rs::render::DrawData::from_raw(&*raw_dd) };
    mvlog!(
        "[wgpu-mv] draw_data: valid={} lists={} fb_scale=({:.2},{:.2}) disp=({:.1},{:.1})",
        draw_data.valid(),
        draw_data.draw_lists_count(),
        draw_data.framebuffer_scale()[0],
        draw_data.framebuffer_scale()[1],
        draw_data.display_size()[0],
        draw_data.display_size()[1]
    );

    mvlog!("[wgpu-mv] retrieving viewport user data");
    if let Some(data) = unsafe { viewport_user_data_mut(vp) } {
        mvlog!("[wgpu-mv] have viewport user data; acquiring surface frame");
        // Acquire frame with basic recovery on Outdated/Lost/Timeout
        let frame = match data.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Outdated) | Err(wgpu::SurfaceError::Lost) => {
                // Reconfigure with current window size and retry next frame
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let vpm_ref = &*vp;
                    let window_ptr = vpm_ref.platform_handle();
                    if !window_ptr.is_null() {
                        let window: &winit::window::Window = &*(window_ptr as *const _);
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
            Err(wgpu::SurfaceError::Timeout) => {
                // Skip this frame silently
                return;
            }
            Err(e) => {
                eprintln!("[wgpu-mv] get_current_texture error: {:?}", e);
                return;
            }
        };
        mvlog!("[wgpu-mv] acquired frame; creating view");
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        // Encode commands and render (catch panics to avoid crashing the whole app)
        mvlog!("[wgpu-mv] creating command encoder");
        let render_block = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("dear-imgui-wgpu::viewport-encoder"),
            });
            mvlog!("[wgpu-mv] begin_render_pass start");
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
                    timestamp_writes: None,
                });
                mvlog!("[wgpu-mv] begin_render_pass ok");
                mvlog!("[wgpu-mv] about to render_draw_data for viewport");
                // Reuse existing draw path with explicit framebuffer size override, but do
                // not advance frame index: we already advanced it for the main window.
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
                        eprintln!("[wgpu-mv] render_draw_data(with_fb) error: {:?}", e);
                    }
                } else if let Err(e) = renderer.render_draw_data(&draw_data, &mut render_pass) {
                    eprintln!("[wgpu-mv] render_draw_data error: {:?}", e);
                }
                mvlog!("[wgpu-mv] finished render_draw_data");
            }
            mvlog!("[wgpu-mv] submitting queue");
            queue.submit(std::iter::once(encoder.finish()));
            mvlog!("[wgpu-mv] submit ok");
        }));
        if render_block.is_err() {
            eprintln!(
                "[wgpu-mv] panic during viewport render block; skipping present for this viewport"
            );
            return;
        }
        data.pending_frame = Some(frame);
        mvlog!("[wgpu-mv] submitted and stored pending frame");
    }
}

/// Renderer: present frame for viewport surface
pub unsafe extern "C" fn renderer_swap_buffers(vp: *mut Viewport, _render_arg: *mut c_void) {
    if vp.is_null() {
        return;
    }
    mvlog!("[wgpu-mv] Renderer_SwapBuffers");
    if let Some(data) = viewport_user_data_mut(vp) {
        if let Some(frame) = data.pending_frame.take() {
            frame.present();
        }
    }
}

// Raw sys-platform wrappers to avoid typed trampolines
pub unsafe extern "C" fn platform_render_window_sys(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    arg: *mut c_void,
) {
    if vp.is_null() {
        return;
    }
    renderer_render_window(vp as *mut Viewport, arg);
}

pub unsafe extern "C" fn platform_swap_buffers_sys(
    vp: *mut dear_imgui_rs::sys::ImGuiViewport,
    arg: *mut c_void,
) {
    if vp.is_null() {
        return;
    }
    renderer_swap_buffers(vp as *mut Viewport, arg);
}

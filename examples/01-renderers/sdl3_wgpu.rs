//! SDL3 + WGPU renderer example (single window, no multi-viewport).
//!
//! This demonstrates driving Dear ImGui with:
//! - SDL3 for window + events
//! - WGPU for rendering
//! - Official SDL3 platform backend (via `dear-imgui-sdl3`)
//! - Rust WGPU renderer backend (`dear-imgui-wgpu`)
//!
//! Run with:
//!   cargo run -p dear-imgui-examples --bin sdl3_wgpu

use std::error::Error;
use std::time::Instant;

use dear_imgui_rs::{Condition, ConfigFlags, Context};
use dear_imgui_sdl3 as imgui_sdl3_backend;
use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer};
use sdl3::event::Event;
use sdl3::keyboard::Keycode;
use sdl3::video::{SwapInterval, WindowPos};

fn main() -> Result<(), Box<dyn Error>> {
    // Initialize SDL3 (video + events).
    let sdl = sdl3::init()?;
    let video = sdl.video()?;

    // Create a basic window (no GL context needed for WGPU).
    let main_scale = video
        .get_primary_display()?
        .get_content_scale()
        .unwrap_or(1.0);

    let mut window = video
        .window(
            "Dear ImGui SDL3 + WGPU",
            (1200.0 * main_scale) as u32,
            (720.0 * main_scale) as u32,
        )
        .resizable()
        .high_pixel_density()
        .build()
        .map_err(|e| format!("failed to create SDL3 window: {e}"))?;
    window.set_position(WindowPos::Centered, WindowPos::Centered);

    // Disable vsync at SDL level (WGPU present mode controls timing).
    let _ = video.gl_set_swap_interval(SwapInterval::Immediate);

    // Initialize WGPU instance, surface, device and queue.
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    });

    // SAFETY: SDL3 window handle is valid for the duration of the surface.
    let surface = unsafe {
        instance.create_surface_unsafe(
            wgpu::SurfaceTargetUnsafe::from_window(&window)
                .expect("failed to create SurfaceTarget from SDL3 window"),
        )?
    };

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .expect("failed to find suitable WGPU adapter");

    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;

    // Configure surface using the window's backing pixel size.
    let (width, height) = window.size_in_pixels();
    let caps = surface.get_capabilities(&adapter);
    let preferred_srgb = [
        wgpu::TextureFormat::Bgra8UnormSrgb,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ];
    let format = preferred_srgb
        .iter()
        .cloned()
        .find(|f| caps.formats.contains(f))
        .unwrap_or(caps.formats[0]);

    let mut surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width,
        height,
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &surface_config);

    // Dear ImGui context.
    let mut imgui = Context::create();
    imgui.set_ini_filename(None::<String>)?;

    // Enable basic navigation and scale style/fonts using the display scale.
    {
        let io = imgui.io_mut();
        let mut flags = io.config_flags();
        flags.insert(ConfigFlags::NAV_ENABLE_KEYBOARD);
        flags.insert(ConfigFlags::NAV_ENABLE_GAMEPAD);
        io.set_config_flags(flags);

        let style = imgui.style_mut();
        style.set_font_scale_dpi(main_scale);
    }

    // Initialize SDL3 platform backend (for "other" renderer).
    imgui_sdl3_backend::init_for_other(&mut imgui, &window)?;

    // Initialize WGPU renderer backend.
    let init_info = WgpuInitInfo::new(device.clone(), queue.clone(), surface_config.format);
    let mut renderer = WgpuRenderer::new(init_info, &mut imgui)?;

    let mut last_frame = Instant::now();
    let mut show_demo = true;

    loop {
        // Handle events (both for ImGui via SDL3 backend and our own logic).
        while let Some(raw) = imgui_sdl3_backend::sdl3_poll_event_ll() {
            // Feed ImGui SDL3 backend.
            let _ = imgui_sdl3_backend::process_sys_event(&raw);

            // Convert to high-level Event for application logic.
            let event = Event::from_ll(raw);
            match event {
                Event::Quit { .. } => {
                    imgui_sdl3_backend::shutdown(&mut imgui);
                    return Ok(());
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    imgui_sdl3_backend::shutdown(&mut imgui);
                    return Ok(());
                }
                Event::Window {
                    win_event: sdl3::event::WindowEvent::PixelSizeChanged(_, _),
                    window_id,
                    ..
                } if window_id == window.id() => {
                    // Reconfigure WGPU surface when window pixel size changes.
                    let (w, h) = window.size_in_pixels();
                    if w > 0 && h > 0 {
                        surface_config.width = w;
                        surface_config.height = h;
                        surface.configure(&device, &surface_config);
                    }
                }
                _ => {}
            }
        }

        // Update delta time.
        let now = Instant::now();
        let dt = (now - last_frame).as_secs_f32();
        last_frame = now;
        imgui.io_mut().set_delta_time(dt);

        // Start a new ImGui frame.
        imgui_sdl3_backend::sdl3_new_frame(&mut imgui);
        let ui = imgui.frame();

        // Basic UI: show demo window and a small control window.
        ui.window("SDL3 + WGPU")
            .size([400.0, 200.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Dear ImGui running on SDL3 + WGPU");
                ui.separator();
                ui.checkbox("Show demo window", &mut show_demo);

                ui.text(format!(
                    "Application average {:.3} ms/frame ({:.1} FPS)",
                    1000.0 / ui.io().framerate(),
                    ui.io().framerate()
                ));
            });

        if show_demo {
            ui.show_demo_window(&mut show_demo);
        }

        let draw_data = imgui.render();

        // Acquire next frame from the WGPU surface.
        let frame = match surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                surface.configure(&device, &surface_config);
                continue;
            }
            Err(wgpu::SurfaceError::Timeout) => {
                continue;
            }
            Err(e) => {
                imgui_sdl3_backend::shutdown(&mut imgui);
                return Err(Box::new(e));
            }
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Record ImGui draw calls into WGPU command buffer.
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("sdl3_wgpu_encoder"),
        });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sdl3_wgpu_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.12,
                            b: 0.15,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            renderer.new_frame()?;
            renderer.render_draw_data_with_fb_size(
                draw_data,
                &mut rpass,
                surface_config.width,
                surface_config.height,
            )?;
        }

        queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }
}

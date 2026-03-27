use std::ffi::{c_char, c_int};
use std::time::Instant;

use dear_imgui_rs::{Condition, ConfigFlags, Context};
use dear_imgui_sdl3::{self as imgui_sdl3_backend, GamepadMode};
use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer};
use pollster::block_on;
use sdl3::event::Event;
use sdl3::keyboard::Keycode;

fn run_inner(_argc: c_int, _argv: *mut *mut c_char) -> Result<c_int, Box<dyn std::error::Error>> {
    imgui_sdl3_backend::enable_native_ime_ui();

    let sdl = sdl3::init()?;
    let video = sdl.video()?;
    let main_scale = video
        .get_primary_display()?
        .get_content_scale()
        .unwrap_or(1.0);

    let window = video
        .window(
            "Dear ImGui iOS SDL3 Smoke",
            (1200.0 * main_scale) as u32,
            (720.0 * main_scale) as u32,
        )
        .high_pixel_density()
        .build()
        .map_err(|err| format!("failed to create SDL3 window: {err}"))?;

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });

    let surface = create_surface::create_surface(&instance, &window)
        .map_err(|err| format!("failed to create WGPU surface from SDL3 window: {err}"))?;

    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .expect("failed to find suitable WGPU adapter for SDL3 iOS smoke");

    let required_limits = wgpu::Limits::downlevel_defaults()
        .using_resolution(adapter.limits())
        .using_alignment(adapter.limits());
    let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits,
        ..Default::default()
    }))?;

    let (width, height) = window.size_in_pixels();
    let caps = surface.get_capabilities(&adapter);
    let preferred_srgb = [
        wgpu::TextureFormat::Bgra8UnormSrgb,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ];
    let format = preferred_srgb
        .iter()
        .copied()
        .find(|candidate| caps.formats.contains(candidate))
        .unwrap_or(caps.formats[0]);

    let mut surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: width.max(1),
        height: height.max(1),
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &surface_config);

    let mut imgui = Context::create();
    imgui.set_ini_filename(None::<String>)?;
    {
        let io = imgui.io_mut();
        let mut flags = io.config_flags();
        flags.insert(ConfigFlags::NAV_ENABLE_KEYBOARD);
        flags.insert(ConfigFlags::NAV_ENABLE_GAMEPAD);
        io.set_config_flags(flags);

        let style = imgui.style_mut();
        style.set_font_scale_dpi(main_scale);
    }

    imgui_sdl3_backend::init_for_other(&mut imgui, &window)?;
    imgui_sdl3_backend::set_gamepad_mode(GamepadMode::AutoAll);

    let init_info = WgpuInitInfo::new(device.clone(), queue.clone(), surface_config.format);
    let mut renderer = WgpuRenderer::new(init_info, &mut imgui)?;

    let mut last_frame = Instant::now();
    let mut show_demo_window = true;
    let mut tap_count = 0u32;
    let mut input_text = String::from(
        "Tap here and focus the input field to validate the SDL3 iOS soft keyboard path.",
    );
    let mut clear_color = wgpu::Color {
        r: 0.09,
        g: 0.12,
        b: 0.17,
        a: 1.0,
    };

    loop {
        while let Some(raw) = imgui_sdl3_backend::sdl3_poll_event_ll() {
            let _ = imgui_sdl3_backend::process_sys_event(&raw);
            let event = Event::from_ll(raw);
            match event {
                Event::Quit { .. } => {
                    imgui_sdl3_backend::shutdown(&mut imgui);
                    return Ok(0);
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    imgui_sdl3_backend::shutdown(&mut imgui);
                    return Ok(0);
                }
                Event::Window {
                    win_event: sdl3::event::WindowEvent::PixelSizeChanged(_, _),
                    window_id,
                    ..
                } if window_id == window.id() => {
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

        let now = Instant::now();
        let dt = (now - last_frame).as_secs_f32();
        last_frame = now;
        imgui.io_mut().set_delta_time(dt);

        imgui_sdl3_backend::sdl3_new_frame(&mut imgui);
        let ui = imgui.frame();

        ui.window("Dear ImGui iOS SDL3 Smoke")
            .size([460.0, 300.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Reference path: SDL3 + WGPU");
                ui.separator();
                ui.text(format!(
                    "Framebuffer: {} x {}",
                    surface_config.width, surface_config.height
                ));
                ui.text(format!("FPS: {:.1}", ui.io().framerate()));

                if ui.button("Count Tap") {
                    tap_count = tap_count.saturating_add(1);
                }
                ui.same_line();
                ui.text(format!("Taps: {tap_count}"));

                ui.input_text("Input", &mut input_text).build();

                let mut color = [
                    clear_color.r as f32,
                    clear_color.g as f32,
                    clear_color.b as f32,
                    clear_color.a as f32,
                ];
                if ui.color_edit4("Clear Color", &mut color) {
                    clear_color.r = color[0] as f64;
                    clear_color.g = color[1] as f64;
                    clear_color.b = color[2] as f64;
                    clear_color.a = color[3] as f64;
                }

                ui.text_wrapped(
                    "This route keeps SDL3 packaging and the app entry point in the host application.",
                );
            });

        if show_demo_window {
            ui.show_demo_window(&mut show_demo_window);
        }

        let draw_data = imgui.render();
        let (frame, reconfigure_after_present) = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                surface.configure(&device, &surface_config);
                continue;
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                continue;
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                imgui_sdl3_backend::shutdown(&mut imgui);
                return Err("surface acquisition failed with a WGPU validation error".into());
            }
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("dear_imgui_ios_sdl3_smoke_encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("dear_imgui_ios_sdl3_smoke_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            renderer.new_frame()?;
            renderer.render_draw_data_with_fb_size(
                draw_data,
                &mut render_pass,
                surface_config.width,
                surface_config.height,
            )?;
        }

        queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        if reconfigure_after_present {
            surface.configure(&device, &surface_config);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dear_imgui_ios_sdl3_smoke_main(argc: c_int, argv: *mut *mut c_char) -> c_int {
    match run_inner(argc, argv) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("Dear ImGui iOS SDL3 smoke failed: {err}");
            1
        }
    }
}

mod create_surface {
    use sdl3::video::Window;
    use wgpu::rwh::{HasDisplayHandle, HasWindowHandle};

    struct SyncWindow<'a>(&'a Window);

    unsafe impl<'a> Send for SyncWindow<'a> {}
    unsafe impl<'a> Sync for SyncWindow<'a> {}

    impl<'a> HasWindowHandle for SyncWindow<'a> {
        fn window_handle(&self) -> Result<wgpu::rwh::WindowHandle<'_>, wgpu::rwh::HandleError> {
            self.0.window_handle()
        }
    }

    impl<'a> HasDisplayHandle for SyncWindow<'a> {
        fn display_handle(&self) -> Result<wgpu::rwh::DisplayHandle<'_>, wgpu::rwh::HandleError> {
            self.0.display_handle()
        }
    }

    pub fn create_surface<'a>(
        instance: &wgpu::Instance,
        window: &'a Window,
    ) -> Result<wgpu::Surface<'a>, String> {
        instance
            .create_surface(SyncWindow(window))
            .map_err(|err| err.to_string())
    }
}

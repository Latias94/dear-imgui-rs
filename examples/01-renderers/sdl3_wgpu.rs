//! SDL3 + WGPU renderer example (single window, no multi-viewport).
//!
//! This demonstrates driving Dear ImGui with:
//! - SDL3 for window + events;
//! - WGPU for rendering;
//! - the official SDL3 platform backend via `dear-imgui-sdl3`;
//! - the Rust WGPU renderer backend, `dear-imgui-wgpu`.
//!
//! Run with:
//! `cargo run -p dear-imgui-examples --bin sdl3_wgpu`
//!
//! For native multi-viewport, see `sdl3_wgpu_multi_viewport.rs` and run it with the
//! `sdl3-wgpu-multi-viewport` feature.

use std::cell::RefCell;
use std::error::Error;
use std::time::Instant;

use dear_imgui_examples::sdl3_callbacks::{
    Sdl3CallbackEventHandoff, configure_main_callback_rate, requests_exit,
};
use dear_imgui_rs::{Condition, ConfigFlags, Context};
use dear_imgui_sdl3::{self as imgui_sdl3_backend, GamepadMode, Sdl3PlatformBackend};
use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer};
use sdl3::video::{SwapInterval, WindowPos};
use sdl3_main::{AppResult, AppResultWithState, MainThreadData, app_impl};

struct WgpuApp {
    events: Sdl3CallbackEventHandoff,
    main: MainThreadData<RefCell<MainData>>,
}

struct MainData {
    renderer: WgpuRenderer,
    sdl3_backend: Sdl3PlatformBackend,
    imgui: Context,
    surface: wgpu::Surface<'static>,
    queue: wgpu::Queue,
    device: wgpu::Device,
    _adapter: wgpu::Adapter,
    _instance: wgpu::Instance,
    surface_config: wgpu::SurfaceConfiguration,
    window: sdl3::video::Window,
    _video: sdl3::VideoSubsystem,
    _sdl: sdl3::Sdl,
    last_frame: Instant,
    show_demo: bool,
}

impl WgpuApp {
    fn new() -> Result<Self, Box<dyn Error>> {
        configure_main_callback_rate();
        imgui_sdl3_backend::enable_native_ime_ui();

        let sdl = sdl3::init()?;
        let video = sdl.video()?;
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
            .map_err(|error| format!("failed to create SDL3 window: {error}"))?;
        window.set_position(WindowPos::Centered, WindowPos::Centered);
        let _ = video.gl_set_swap_interval(SwapInterval::Immediate);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        // SAFETY: the retained SDL3 window stays valid until after this surface is dropped.
        let surface = unsafe {
            instance.create_surface_unsafe(
                wgpu::SurfaceTargetUnsafe::from_display_and_window(&window, &window)
                    .expect("failed to create SurfaceTarget from SDL3 window"),
            )?
        };
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            apply_limit_buckets: false,
            force_fallback_adapter: false,
        }))
        .expect("failed to find suitable WGPU adapter");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;

        let (width, height) = window.size_in_pixels();
        let capabilities = surface.get_capabilities(&adapter);
        let preferred_srgb = [
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ];
        let format = preferred_srgb
            .iter()
            .copied()
            .find(|format| capabilities.formats.contains(format))
            .unwrap_or(capabilities.formats[0]);
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width,
            height,
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
            imgui.style_mut().set_font_scale_dpi(main_scale);
        }
        // SAFETY: the retained window outlives explicit platform shutdown.
        let mut sdl3_backend = unsafe { Sdl3PlatformBackend::init_for_other(&mut imgui, &window)? };
        sdl3_backend.set_gamepad_mode(&mut imgui, GamepadMode::AutoAll)?;
        let renderer = WgpuRenderer::new(
            WgpuInitInfo::new(device.clone(), queue.clone(), surface_config.format),
            &mut imgui,
        )?;

        Ok(Self {
            events: Sdl3CallbackEventHandoff::default(),
            main: MainThreadData::assert_new(RefCell::new(MainData {
                renderer,
                sdl3_backend,
                imgui,
                surface,
                queue,
                device,
                _adapter: adapter,
                _instance: instance,
                surface_config,
                window,
                _video: video,
                _sdl: sdl,
                last_frame: Instant::now(),
                show_demo: true,
            })),
        })
    }

    fn process_events(&self) -> AppResult {
        let mut events = self.events.drain();
        let mut main_guard = self.main.assert_get().borrow_mut();
        let main = &mut *main_guard;
        while let Some(event) = events.pop() {
            let backend_result = event.with_imgui_event(|raw| match raw {
                // SAFETY: the callback handoff reconstructs the active union variant and owns
                // every pointer payload for the duration of this closure.
                Some(raw) => unsafe { main.sdl3_backend.process_raw_event(&mut main.imgui, raw) },
                None => Ok(false),
            });
            if let Err(error) = backend_result {
                eprintln!("SDL3 backend event processing failed: {error}");
                return AppResult::Failure;
            }
            if requests_exit(&event, main.window.id()) {
                return AppResult::Success;
            }
            if event.is_pixel_size_changed_for(main.window.id()) {
                Self::reconfigure_surface(main);
            }
        }
        AppResult::Continue
    }

    fn reconfigure_surface(main: &mut MainData) {
        let (width, height) = main.window.size_in_pixels();
        if width > 0 && height > 0 {
            main.surface_config.width = width;
            main.surface_config.height = height;
            main.surface.configure(&main.device, &main.surface_config);
        }
    }

    fn render(&self) -> Result<(), Box<dyn Error>> {
        let mut main_guard = self.main.assert_get().borrow_mut();
        let main = &mut *main_guard;
        let now = Instant::now();
        main.imgui
            .io_mut()
            .set_delta_time((now - main.last_frame).as_secs_f32());
        main.last_frame = now;
        main.sdl3_backend.new_frame(&mut main.imgui)?;
        let ui = main.imgui.frame();
        ui.window("SDL3 + WGPU")
            .size([400.0, 200.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Dear ImGui running on SDL3 + WGPU");
                ui.separator();
                ui.checkbox("Show demo window", &mut main.show_demo);
                ui.text("Gamepad: SDL3 backend in AutoAll mode (all controllers merged)");
                ui.text(format!(
                    "Application average {:.3} ms/frame ({:.1} FPS)",
                    1000.0 / ui.io().framerate(),
                    ui.io().framerate()
                ));
            });
        if main.show_demo {
            ui.show_demo_window(&mut main.show_demo);
        }
        let draw_data = main.imgui.render();

        let (frame, reconfigure_after_present) = match main.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                drop(draw_data);
                Self::reconfigure_surface(main);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                drop(draw_data);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("surface acquisition failed with a WGPU validation error".into());
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = main
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("sdl3_wgpu_encoder"),
            });
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
                multiview_mask: None,
            });
            main.renderer.new_frame()?;
            main.renderer.render_with_fb_size(
                draw_data,
                &mut render_pass,
                main.surface_config.width,
                main.surface_config.height,
            )?;
        }
        main.queue.submit(std::iter::once(encoder.finish()));
        main.queue.present(frame);
        if reconfigure_after_present {
            Self::reconfigure_surface(main);
        }
        Ok(())
    }

    fn shutdown(&self) {
        let mut main_guard = self.main.assert_get().borrow_mut();
        let main = &mut *main_guard;
        if let Err(error) = main.renderer.shutdown(&mut main.imgui) {
            eprintln!("WGPU renderer shutdown failed: {error}");
        }
        if let Err(error) = main.sdl3_backend.shutdown(&mut main.imgui) {
            eprintln!("SDL3 platform backend shutdown failed: {error}");
        }
    }
}

#[app_impl]
impl WgpuApp {
    fn app_init() -> AppResultWithState<Box<Self>> {
        match Self::new() {
            Ok(app) => AppResultWithState::Continue(Box::new(app)),
            Err(error) => {
                eprintln!("failed to initialize SDL3 WGPU example: {error}");
                AppResultWithState::Failure(None)
            }
        }
    }

    fn app_iterate(&self) -> AppResult {
        let event_result = self.process_events();
        if event_result != AppResult::Continue {
            return event_result;
        }
        match self.render() {
            Ok(()) => AppResult::Continue,
            Err(error) => {
                eprintln!("SDL3 WGPU frame failed: {error}");
                AppResult::Failure
            }
        }
    }

    fn app_event(&self, raw: &sdl3::sys::events::SDL_Event) -> AppResult {
        self.events.push(raw);
        AppResult::Continue
    }

    fn app_quit(state: Option<&Self>) {
        if let Some(app) = state {
            app.shutdown();
        }
    }
}

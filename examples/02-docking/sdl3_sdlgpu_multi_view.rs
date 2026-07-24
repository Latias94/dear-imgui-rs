use dear_imgui_rs::{Condition, ConfigFlags, Context};
use dear_imgui_sdl3::{self as imgui_sdl3_backend, SdlGpu3InitInfo, SdlGpu3RendererBackend};
use sdl3::event::{Event, WindowEvent};
use sdl3::gpu::{PresentMode, ShaderFormat, SwapchainComposition};
use sdl3::keyboard::Keycode;
use sdl3::pixels::Color;
use sdl3::{Sdl, VideoSubsystem};
use sdl3_main::{AppResult, AppResultWithState, MainThreadData, app_impl};
use std::error::Error;
use std::sync::Mutex;

fn select_present_mode(mailbox_supported: bool) -> PresentMode {
    if mailbox_supported {
        PresentMode::Mailbox
    } else {
        PresentMode::Vsync
    }
}

fn low_latency_present_mode(gpu: &sdl3::gpu::Device, window: &sdl3::video::Window) -> PresentMode {
    // The device claims `window` before this query. Mailbox avoids presenting stale queued frames.
    let mailbox_supported = unsafe {
        sdl3::sys::gpu::SDL_WindowSupportsGPUPresentMode(
            gpu.raw(),
            window.raw(),
            sdl3::sys::gpu::SDL_GPUPresentMode::MAILBOX,
        )
    };
    select_present_mode(mailbox_supported)
}

struct SdlGpuApp {
    main: MainThreadData<MainData>,
}

struct MainData {
    sdl3_backend: SdlGpu3RendererBackend,
    imgui: Context,
    gpu: sdl3::gpu::Device,
    window: sdl3::video::Window,
    _video: VideoSubsystem,
    _sdl: Sdl,
    show_demo: bool,
    show_debug: bool,
    show_about: bool,
}

impl SdlGpuApp {
    fn new() -> Result<Self, Box<dyn Error>> {
        // Enable native IME UI before creating any SDL3 windows (recommended for IME-heavy locales).
        imgui_sdl3_backend::enable_native_ime_ui();
        // Bound the nonblocking callback loop without overriding an application/user choice.
        sdl3::hint::set_with_priority("SDL_MAIN_CALLBACK_RATE", "120", &sdl3::hint::Hint::Default);

        let sdl_ctx = sdl3::init()?;
        let video = sdl_ctx.video()?;

        let main_scale = video
            .get_primary_display()?
            .get_content_scale()
            .unwrap_or(1.0);

        let mut window = video
            .window(
                "Dear ImGui + SDL3 + SDL3GPU (multi-viewport)",
                (800.0 * main_scale) as u32,
                (600.0 * main_scale) as u32,
            )
            .resizable()
            .hidden()
            .high_pixel_density()
            .build()
            .map_err(|e| format!("failed to create SDL3 window: {e}"))?;

        window.show();

        let gpu = sdl3::gpu::Device::new(
            ShaderFormat::SPIRV | ShaderFormat::DXIL | ShaderFormat::MSL | ShaderFormat::METALLIB,
            true,
        )?
        .with_window(&window)?;

        let mut imgui = Context::create();

        {
            let io = imgui.io_mut();
            let mut flags = io.config_flags();
            flags.insert(ConfigFlags::DOCKING_ENABLE);
            flags.insert(ConfigFlags::VIEWPORTS_ENABLE);
            io.set_config_flags(flags);
        }

        // Basic style scaling using the window's display scale.
        let window_scale = window.display_scale();
        imgui.style_mut().set_font_scale_dpi(window_scale);

        let present_mode = low_latency_present_mode(&gpu, &window);
        gpu.set_swapchain_parameters(&window, present_mode, SwapchainComposition::Sdr)?;

        // SAFETY: `window` and `gpu` are stored with the backend and outlive explicit shutdown.
        let mut init_info = SdlGpu3InitInfo::from_window(&gpu, &window);
        init_info.present_mode = present_mode;
        let sdl3_backend = unsafe { SdlGpu3RendererBackend::init(&mut imgui, &window, init_info)? };

        Ok(Self {
            main: MainThreadData::assert_new(MainData {
                sdl3_backend,
                imgui,
                gpu,
                window,
                _video: video,
                _sdl: sdl_ctx,
                show_demo: false,
                show_debug: false,
                show_about: false,
            }),
        })
    }

    fn render(&mut self) -> Result<(), Box<dyn Error>> {
        let main = self.main.assert_get_mut();
        if main.window.is_minimized() {
            sdl3::timer::delay(10);
            return Ok(());
        }

        main.sdl3_backend.new_frame(&mut main.imgui)?;
        let ui = main.imgui.frame();

        ui.window("SDL3 + IMGUI")
            .size([400.0, 200.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Dear ImGui running on SDL3 + SDL_GPU");
                ui.separator();
                ui.checkbox("Show demo window", &mut main.show_demo);
                ui.checkbox("Show debug log window", &mut main.show_debug);
                ui.checkbox("Show about window", &mut main.show_about);
            });

        if main.show_demo {
            ui.show_demo_window(&mut main.show_demo);
        }
        if main.show_debug {
            ui.show_debug_log_window(&mut main.show_debug);
        }
        if main.show_about {
            ui.show_about_window(&mut main.show_about);
        }

        let frame = main.imgui.render();
        let is_minimized = frame
            .draw_data()
            .display_size()
            .into_iter()
            .any(|size| size <= 0.0);
        let mut draw_cmd = main.gpu.acquire_command_buffer()?;
        // Win32 invokes AppIterate from its modal move/resize timer. A blocking acquire here
        // stalls window messages, so callback-driven applications must use the nonblocking path.
        let swap_chain = match draw_cmd.acquire_swapchain_texture(&main.window) {
            Ok(swap_chain) => swap_chain,
            Err(error) => {
                draw_cmd.cancel();
                return Err(error.into());
            }
        };

        let main_render = if let Some(swap_chain) = swap_chain.filter(|_| !is_minimized) {
            let target_info = sdl3::gpu::ColorTargetInfo::default()
                .with_texture(&swap_chain)
                .with_clear_color(Color::RGB(0, 255, 255))
                .with_load_op(sdl3::gpu::LoadOp::CLEAR)
                .with_store_op(sdl3::gpu::StoreOp::STORE);
            (|| -> Result<(), Box<dyn Error>> {
                // SAFETY: this command buffer belongs to the device used to initialize the backend.
                let prepared = unsafe { main.sdl3_backend.prepare_render(frame, &draw_cmd)? };
                let mut render_pass =
                    main.gpu
                        .begin_render_pass(&draw_cmd, &[target_info], None)?;

                // SAFETY: this pass belongs to the same device and remains active for this call.
                let render_result = unsafe { prepared.render(&mut render_pass) };
                main.gpu.end_render_pass(render_pass);
                render_result?;
                Ok(())
            })()
        } else {
            drop(frame);
            Ok(())
        };
        if let Err(error) = main_render {
            draw_cmd.cancel();
            return Err(error);
        }

        let io_flags = main.imgui.io().config_flags();
        if io_flags.contains(ConfigFlags::VIEWPORTS_ENABLE) {
            main.imgui.update_platform_windows();
            main.imgui.render_platform_windows_default();
        }

        draw_cmd.submit()?;

        Ok(())
    }

    fn process_event(&mut self, raw: &sdl3::sys::events::SDL_Event) -> AppResult {
        let Some(main_thread) = sdl3_main::MainThreadToken::get() else {
            return AppResult::Continue;
        };
        let main = self.main.get_mut(main_thread);

        if let Err(error) = main.sdl3_backend.process_event(&mut main.imgui, raw) {
            eprintln!("SDL3 backend event processing failed: {error}");
            return AppResult::Failure;
        }

        match Event::from_ll(*raw) {
            Event::Quit { .. }
            | Event::KeyDown {
                keycode: Some(Keycode::Escape),
                ..
            } => AppResult::Success,
            Event::Window {
                window_id,
                win_event: WindowEvent::CloseRequested,
                ..
            } if window_id == main.window.id() => AppResult::Success,
            _ => AppResult::Continue,
        }
    }

    fn shutdown(&mut self) {
        let main = self.main.assert_get_mut();
        // SAFETY: all command buffers are submitted, and `gpu` remains live for this call.
        let gpu_idle = unsafe { sdl3::sys::gpu::SDL_WaitForGPUIdle(main.gpu.raw()) };
        if !gpu_idle {
            eprintln!("SDL_WaitForGPUIdle failed: {}", sdl3::get_error());
        }
        let backend_shutdown = main.sdl3_backend.shutdown(&mut main.imgui);
        if let Err(error) = &backend_shutdown {
            eprintln!("SDL3 backend shutdown failed: {error}");
        }
        if gpu_idle && backend_shutdown.is_ok() {
            // SAFETY: `window` is the main window claimed by this idle GPU device, and the
            // backend has released all secondary platform windows.
            unsafe {
                sdl3::sys::gpu::SDL_ReleaseWindowFromGPUDevice(main.gpu.raw(), main.window.raw());
            }
        }
    }
}

#[app_impl]
impl SdlGpuApp {
    fn app_init() -> AppResultWithState<Box<Mutex<Self>>> {
        match Self::new() {
            Ok(app) => AppResultWithState::Continue(Box::new(Mutex::new(app))),
            Err(error) => {
                eprintln!("failed to initialize SDL3 SDLGPU example: {error}");
                AppResultWithState::Failure(None)
            }
        }
    }

    fn app_iterate(&mut self) -> AppResult {
        match self.render() {
            Ok(()) => AppResult::Continue,
            Err(error) => {
                eprintln!("SDL3 SDLGPU frame failed: {error}");
                AppResult::Failure
            }
        }
    }

    fn app_event(&mut self, raw: &sdl3::sys::events::SDL_Event) -> AppResult {
        self.process_event(raw)
    }

    fn app_quit(state: Option<&mut Self>) {
        if let Some(app) = state {
            app.shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mailbox_is_preferred_when_supported() {
        assert_eq!(select_present_mode(true), PresentMode::Mailbox);
        assert_eq!(select_present_mode(false), PresentMode::Vsync);
    }
}

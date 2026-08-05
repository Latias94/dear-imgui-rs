use dear_imgui_examples::sdl3_callbacks::{
    Sdl3CallbackEventHandoff, configure_main_callback_rate, requests_exit,
};
use dear_imgui_rs::{Condition, ConfigFlags, Context};
use dear_imgui_sdl3::{self as imgui_sdl3_backend, SdlGpu3InitInfo, SdlGpu3RendererBackend};
use sdl3::gpu::{PresentMode, ShaderFormat, SwapchainComposition};
use sdl3::pixels::Color;
use sdl3::{Sdl, VideoSubsystem};
use sdl3_main::{AppResult, AppResultWithState, MainThreadData, app_impl};
use std::cell::RefCell;
use std::error::Error;

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

fn secondary_viewport_present_mode() -> PresentMode {
    // On D3D12, SDL claims new viewport windows with VSync before applying the requested mode.
    // Switching the newly claimed viewport to Mailbox waits for the shared queue to drain, which
    // turns a normal viewport drag into a synchronous UI stall. The primary window keeps Mailbox.
    PresentMode::Vsync
}

struct SdlGpuApp {
    main: MainThreadData<RefCell<MainData>>,
    events: Sdl3CallbackEventHandoff,
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
        configure_main_callback_rate();

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
        let window_scale = window.display_scale();
        let window_scale = if window_scale.is_finite() && window_scale > 0.0 {
            window_scale
        } else {
            1.0
        };

        {
            let io = imgui.io_mut();
            let mut flags = io.config_flags();
            flags.insert(ConfigFlags::DOCKING_ENABLE);
            flags.insert(ConfigFlags::VIEWPORTS_ENABLE);
            io.set_config_flags(flags);
            io.set_config_dpi_scale_fonts(true);
            io.set_config_dpi_scale_viewports(true);
        }

        let style = imgui.style_mut();
        style.scale_all_sizes(window_scale);
        style.set_font_scale_dpi(window_scale);

        let present_mode = low_latency_present_mode(&gpu, &window);
        gpu.set_swapchain_parameters(&window, present_mode, SwapchainComposition::Sdr)?;

        // SAFETY: `window` and `gpu` are stored with the backend and outlive explicit shutdown.
        let mut init_info = SdlGpu3InitInfo::from_window(&gpu, &window);
        init_info.present_mode = secondary_viewport_present_mode();
        let sdl3_backend = unsafe { SdlGpu3RendererBackend::init(&mut imgui, &window, init_info)? };

        Ok(Self {
            main: MainThreadData::assert_new(RefCell::new(MainData {
                sdl3_backend,
                imgui,
                gpu,
                window,
                _video: video,
                _sdl: sdl_ctx,
                show_demo: false,
                show_debug: false,
                show_about: false,
            })),
            events: Sdl3CallbackEventHandoff::default(),
        })
    }

    fn iterate(&self) -> Result<AppResult, Box<dyn Error>> {
        let mut events = self.events.drain();
        let mut main_guard = self.main.assert_get().borrow_mut();
        let main = &mut *main_guard;
        while let Some(event) = events.pop() {
            event.with_imgui_event(|raw| -> Result<(), Box<dyn Error>> {
                if let Some(raw) = raw {
                    // SAFETY: the callback handoff reconstructs the active union variant and owns
                    // every pointer payload for the duration of this closure.
                    let _ = unsafe { main.sdl3_backend.process_raw_event(&mut main.imgui, raw)? };
                }
                Ok(())
            })?;
            if requests_exit(&event, main.window.id()) {
                return Ok(AppResult::Success);
            }
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

        let viewports_enabled = main
            .imgui
            .io()
            .config_flags()
            .contains(ConfigFlags::VIEWPORTS_ENABLE);
        let main_window_minimized = main.window.is_minimized();
        let mut frame = main.imgui.render();
        let is_minimized = frame
            .draw_data()
            .display_size()
            .into_iter()
            .any(|size| size <= 0.0);

        // Texture reconciliation and the secondary-window pump cannot depend on the main
        // swapchain. Detached viewports remain interactive while the main window is minimized or
        // temporarily lacks a presentable image.
        main.sdl3_backend.reconcile_frame(&mut frame)?;
        if viewports_enabled {
            frame.update_and_render_platform_windows_default();
            main.sdl3_backend.poll_fault()?;
        }
        if main_window_minimized || is_minimized {
            drop(frame);
            sdl3::timer::delay(10);
            return Ok(AppResult::Continue);
        }

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
            // A swapchain texture may have been acquired, so SDL requires this command buffer to
            // be submitted even after render preparation or rendering failed. The closure above
            // ends any pass it begins before returning its error.
            if let Err(submit_error) = draw_cmd.submit() {
                eprintln!(
                    "failed to submit an SDLGPU command buffer after render failure: {submit_error}"
                );
            }
            return Err(error);
        }

        draw_cmd.submit()?;

        Ok(AppResult::Continue)
    }

    fn shutdown(&self) {
        let mut main_guard = self.main.assert_get().borrow_mut();
        let main = &mut *main_guard;
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
    fn app_init() -> AppResultWithState<Box<Self>> {
        match Self::new() {
            Ok(app) => AppResultWithState::Continue(Box::new(app)),
            Err(error) => {
                eprintln!("failed to initialize SDL3 SDLGPU example: {error}");
                AppResultWithState::Failure(None)
            }
        }
    }

    fn app_iterate(&self) -> AppResult {
        match self.iterate() {
            Ok(result) => result,
            Err(error) => {
                eprintln!("SDL3 SDLGPU frame failed: {error}");
                AppResult::Failure
            }
        }
    }

    fn app_event(&self, raw: &sdl3::sys::events::SDL_Event) -> AppResult {
        // SAFETY: SDL supplies a valid event whose transient payload remains live for this call.
        unsafe { self.events.push_from_callback(raw) };
        AppResult::Continue
    }

    fn app_quit(state: Option<&Self>) {
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

    #[test]
    fn secondary_viewports_stay_on_vsync() {
        assert_eq!(secondary_viewport_present_mode(), PresentMode::Vsync);
    }
}

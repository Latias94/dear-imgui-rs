use std::error::Error;
use sdl3::event::{Event, WindowEvent};
use sdl3::gpu::{PresentMode, ShaderFormat, SwapchainComposition};
use sdl3::keyboard::Keycode;
use sdl3::pixels::Color;
use dear_imgui_rs::{Condition, ConfigFlags, Context};
use dear_imgui_sdl3::{self as imgui_sdl3_backend, SdlGpu3RendererBackend};

fn main() -> Result<(), Box<dyn Error>> {
    // Enable VSYNC on Renderer
    sdl3::hint::set(sdl3::hint::names::RENDER_VSYNC, "1");

    // Enable native IME UI before creating any SDL3 windows (recommended for IME-heavy locales).
    imgui_sdl3_backend::enable_native_ime_ui();

    // Initialize SDL3
    let sdl_ctx = sdl3::init()?;
    let video = sdl_ctx.video()?;

    let main_scale = video
        .get_primary_display().unwrap()
        .get_content_scale()
        .unwrap_or(1.0);

    let mut window = video
        .window(
            "Dear ImGui + SDL3 + SDL3GPU (multi-viewport)",
            (800.0 * main_scale) as u32,
            (600.0 * main_scale) as u32,
        )
        .opengl()
        .resizable()
        .hidden()
        .high_pixel_density()
        .build()
        .map_err(|e| format!("failed to create SDL3 window: {e}"))?;

    window.show();

    let gpu = sdl3::gpu::Device::new(
        ShaderFormat::SPIRV | ShaderFormat::DXIL | ShaderFormat::DXBC | ShaderFormat::METALLIB,
        true,
    )?.with_window(&window)?;

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
    {
        let style = imgui.style_mut();
        style.set_font_scale_dpi(window_scale);
    }

    gpu.set_swapchain_parameters(
        &window, PresentMode::Vsync, SwapchainComposition::Sdr)?;

    let mut sdl3_backend = SdlGpu3RendererBackend::init(
        &mut imgui, &window, &gpu)?;

    let mut show_demo = false;
    let mut show_debug = false;
    let mut show_about = false;

    'running: loop {
        //input
        while let Some(raw) = imgui_sdl3_backend::sdl3_poll_event_ll() {
            if sdl3_backend.process_event(&mut imgui, &raw) {
                //event was processed by imgui... we could shortcut the loop here
            }
            let event = Event::from_ll(raw);
            match event {
                Event::Quit { .. } | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    break 'running;
                }
                Event::Window { timestamp: _, window_id: _, win_event } => {
                    match win_event
                    {
                        WindowEvent::CloseRequested => {
                            break 'running;
                        }
                        _ => (),
                    }
                }
                _ => (),
            }
        }

        sdl3_backend.new_frame(&mut imgui);
        let ui = imgui.frame();

        ui.window("SDL3 + IMGUI")
            .size([400.0, 200.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Dear ImGui running on SDL3 + SDL_Renderer");
                ui.separator();
                ui.checkbox("Show demo window", &mut show_demo);
                ui.checkbox("Show debug log window", &mut show_debug);
                ui.checkbox("Show about window", &mut show_about);
            });

        if show_demo {
            ui.show_demo_window(&mut show_demo);
        }
        if show_debug {
            ui.show_debug_log_window(&mut show_debug);
        }
        if show_about {
            ui.show_about_window(&mut show_about);
        }

        //update/render
        let draw_data = imgui.render();

        let mut draw_cmd = gpu.acquire_command_buffer()?;

        if let Ok(swap_chain) = draw_cmd.wait_and_acquire_swapchain_texture(&window) {
            let target_info = sdl3::gpu::ColorTargetInfo::default()
                .with_texture(&swap_chain)
                .with_clear_color(Color::RGB(0, 255, 255))
                .with_load_op(sdl3::gpu::LoadOp::CLEAR)
                .with_store_op(sdl3::gpu::StoreOp::STORE);

            sdl3_backend.prepare_render(draw_data, &mut draw_cmd);

            let mut render_pass = gpu.begin_render_pass(&draw_cmd,
                                                        &[target_info], None)?;

            sdl3_backend.render(draw_data, &mut draw_cmd, &mut render_pass);

            gpu.end_render_pass(render_pass);

            draw_cmd.submit().expect("TODO: panic message");

            let io_flags = imgui.io().config_flags();
            if io_flags.contains(ConfigFlags::VIEWPORTS_ENABLE) {
                imgui.update_platform_windows();
                imgui.render_platform_windows_default();
            }
        } else {
            draw_cmd.cancel();
        }


    }

    Ok(())
}
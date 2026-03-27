//! SDL3 + SDLRenderer3 renderer example
//!
//! Run with:
//!   cargo run -p dear-imgui-examples --bin sdl3_sdlrenderer --features sdl3-sdlrenderer3

use std::error::Error;

use dear_imgui_rs::{Condition, Context};
use dear_imgui_sdl3 as imgui_sdl3_backend;

use sdl3::event::Event;
use sdl3::keyboard::Keycode;
use sdl3::pixels::Color;

fn main() -> Result<(), Box<dyn Error>> {
    // Enable VSYNC on Renderer
    sdl3::hint::set(sdl3::hint::names::RENDER_VSYNC, "1");

    // Enable native IME UI before creating any SDL3 windows (recommended for IME-heavy locales).
    imgui_sdl3_backend::enable_native_ime_ui();

    // Initialize SDL3
    let sdl_ctx = sdl3::init()?;
    let video = sdl_ctx.video()?;

    let main_scale = video.get_primary_display()?.get_content_scale().unwrap_or(1.0);
    let window = video
        .window("SDL Test", (1200.0 * main_scale) as u32, (720.0 * main_scale) as u32)
        .position_centered()
        .resizable()
        .high_pixel_density()
        .build()?;

    // Create renderer/canvas
    let mut canvas = window.into_canvas();

    canvas.set_draw_color(Color::RGB(0, 255, 255));
    canvas.clear();
    canvas.present();

    //IMGUI Context
    let mut imgui = Context::create();
    imgui.set_ini_filename(None::<String>)?;
    imgui.set_log_filename(None::<String>)?;

    imgui_sdl3_backend::init_for_canvas(&mut imgui, canvas.window(), &canvas)?;

    let mut show_demo = false;
    let mut show_debug = false;
    let mut show_about = false;

    'running: loop {
        canvas.clear();

        //input
        while let Some(raw) = imgui_sdl3_backend::sdl3_poll_event_ll() {
            if imgui_sdl3_backend::process_sys_event(&raw) {
                //event was processed by imgui... we coudlshortcut the loop here
            }
            let event = Event::from_ll(raw);
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    break 'running;
                }
                _ => (),
            }
        }

        imgui_sdl3_backend::canvas_new_frame(&mut imgui);
        let ui = imgui.frame();

        ui.window("SDL3 + IMGUI").size([400.0, 200.0], Condition::FirstUseEver).build(|| {
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
        imgui_sdl3_backend::canvas_render(&draw_data, &canvas);

        canvas.present();
    }

    println!("Shutting down...");
    //make sure we shutdown imgui context before we exit!
    imgui_sdl3_backend::shutdown_for_canvas(&mut imgui);

    Ok(())
}

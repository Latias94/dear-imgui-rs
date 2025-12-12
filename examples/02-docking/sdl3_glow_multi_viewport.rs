//! SDL3 + Glow multi-viewport example.
//!
//! This is an experimental example showing how to drive Dear ImGui using:
//! - SDL3 window + GL context for platform and multi-viewport management
//! - Rust Glow renderer backend (`dear-imgui-glow`)
//! - the high-level `dear-imgui-rs` API.
//! - Does not use the official OpenGL3 renderer (`dear-imgui-sdl3/opengl3-renderer`).
//!
//! Run with:
//!   cargo run -p dear-imgui-examples --bin sdl3_glow_multi_viewport \
//!       --features multi-viewport,sdl3-platform

use std::error::Error;
use std::time::Instant;

use dear_imgui_glow::{GlowRenderer, multi_viewport as glow_mvp};
use dear_imgui_rs::{Condition, ConfigFlags, Context};
use dear_imgui_sdl3 as imgui_sdl3_backend;
use glow::HasContext;
use sdl3::event::Event;
use sdl3::keyboard::Keycode;
use sdl3::video::{GLProfile, SwapInterval, WindowPos};

fn main() -> Result<(), Box<dyn Error>> {
    // Enable multi-viewport at runtime.
    const ENABLE_VIEWPORTS: bool = true;

    let sdl = sdl3::init()?;
    let video = sdl.video()?;

    let gl_attr = video.gl_attr();
    gl_attr.set_context_version(3, 2);
    gl_attr.set_context_profile(GLProfile::Core);
    gl_attr.set_depth_size(0);

    // Determine a reasonable initial window size based on display scale.
    let main_scale = video
        .get_primary_display()?
        .get_content_scale()
        .unwrap_or(1.0);

    let mut window = video
        .window(
            "Dear ImGui + SDL3 + Glow (multi-viewport)",
            (800.0 * main_scale) as u32,
            (600.0 * main_scale) as u32,
        )
        .opengl()
        .resizable()
        .hidden()
        .high_pixel_density()
        .build()
        .map_err(|e| format!("failed to create SDL3 window: {e}"))?;

    let gl_context = window
        .gl_create_context()
        .map_err(|e| format!("SDL_GL_CreateContext failed: {e}"))?;
    window
        .gl_make_current(&gl_context)
        .map_err(|e| format!("SDL_GL_MakeCurrent failed: {e}"))?;
    let _ = video.gl_set_swap_interval(SwapInterval::VSync);
    window.set_position(WindowPos::Centered, WindowPos::Centered);
    window.show();

    // Create a glow context for rendering.
    let gl = unsafe { create_glow_context(&video) };

    // Build ImGui context.
    let mut imgui = Context::create();
    {
        let io = imgui.io_mut();
        let mut flags = io.config_flags();
        flags.insert(ConfigFlags::DOCKING_ENABLE);
        if ENABLE_VIEWPORTS {
            flags.insert(ConfigFlags::VIEWPORTS_ENABLE);
        }
        io.set_config_flags(flags);
    }

    // Initialize SDL3 platform backend only (no C++ OpenGL3 renderer).
    imgui_sdl3_backend::init_platform_for_opengl(&mut imgui, &window, &gl_context)?;

    // Basic style scaling using the window's display scale.
    let window_scale = window.display_scale();
    {
        let style = imgui.style_mut();
        style.set_font_scale_dpi(window_scale);
    }

    // Initialize Glow renderer and enable multi-viewport callbacks.
    let mut renderer = GlowRenderer::new(gl, &mut imgui)?;
    if ENABLE_VIEWPORTS {
        glow_mvp::enable(&mut renderer, &mut imgui);
    }

    let mut last_frame = Instant::now();

    'main: loop {
        // 1) Pump events: low-level SDL_Event for ImGui backend + high-level Event for us.
        while let Some(raw) = imgui_sdl3_backend::sdl3_poll_event_ll() {
            // Feed the SDL3 backend.
            let _ = imgui_sdl3_backend::process_sys_event(&raw);

            // Convert to high-level Event for our own logic.
            let event = Event::from_ll(raw);
            match event {
                Event::Quit { .. } => break 'main,
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'main,
                Event::Window {
                    win_event: sdl3::event::WindowEvent::CloseRequested,
                    window_id,
                    ..
                } if window_id == window.id() => break 'main,
                _ => {}
            }
        }

        // 2) Update delta time.
        let now = Instant::now();
        let dt = (now - last_frame).as_secs_f32();
        last_frame = now;
        imgui.io_mut().set_delta_time(dt);

        // 3) Start a new ImGui frame.
        imgui_sdl3_backend::sdl3_new_frame(&mut imgui);
        let ui = imgui.frame();

        // Create a dockspace over the main viewport so there is always content.
        ui.dockspace_over_main_viewport();

        ui.window("Main")
            .size([420.0, 260.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("SDL3 + Glow + Dear ImGui multi-viewport");
                ui.separator();
                ui.text("Drag this window outside the main viewport to spawn OS windows.");
            });

        // 4) Render ImGui main viewport.
        let draw_data = imgui.render();

        // Clear the main framebuffer with Glow.
        unsafe {
            let (w, h) = window.size_in_pixels();
            let gl = renderer.gl_context().unwrap().clone();
            gl.viewport(0, 0, w as i32, h as i32);
            gl.clear_color(0.1, 0.12, 0.15, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
        }

        renderer.new_frame()?;
        renderer.render(draw_data)?;

        // 5) Optionally render additional platform windows when multi-viewport is enabled.
        if ENABLE_VIEWPORTS {
            let io_flags = imgui.io().config_flags();
            if io_flags.contains(ConfigFlags::VIEWPORTS_ENABLE) {
                imgui.update_platform_windows();
                imgui.render_platform_windows_default();
                // Restore main window GL context after ImGui has rendered secondary viewports.
                let _ = window.gl_make_current(&gl_context);
            }
        }

        // Present the main window.
        window.gl_swap_window();
    }

    if ENABLE_VIEWPORTS {
        glow_mvp::shutdown_multi_viewport_support(&mut imgui);
    }
    imgui_sdl3_backend::shutdown(&mut imgui);
    Ok(())
}

/// Create a glow context from an SDL3 `VideoSubsystem`.
///
/// # Safety
///
/// Call this only after there is a current OpenGL context for the thread.
unsafe fn create_glow_context(video: &sdl3::VideoSubsystem) -> glow::Context {
    use std::ffi::c_void;

    glow::Context::from_loader_function(|name| {
        video
            .gl_get_proc_address(name)
            .map(|f| f as *const c_void)
            .unwrap_or(std::ptr::null())
    })
}

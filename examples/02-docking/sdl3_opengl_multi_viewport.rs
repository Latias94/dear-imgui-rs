//! SDL3 + OpenGL3 multi-viewport example.
//!
//! This is an experimental example showing how to drive Dear ImGui using:
//! - SDL3 window + GL context
//! - official C++ backends: imgui_impl_sdl3.cpp + imgui_impl_opengl3.cpp
//! - the high-level `dear-imgui-rs` API.
//!
//! Run with:
//!   cargo run -p dear-imgui-examples --bin sdl3_opengl_multi_viewport --features multi-viewport

use std::error::Error;
use std::time::Instant;

use dear_imgui_rs::{Condition, ConfigFlags, Context, TextureId};
use dear_imgui_sdl3 as imgui_sdl3_backend;
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
            "Dear ImGui + SDL3 + OpenGL3 (multi-viewport)",
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

    // Optional: create a glow context for clearing the main framebuffer.
    let gl = unsafe { create_glow_context(&video) };

    // Create a simple OpenGL texture that we can show inside an ImGui window.
    // This demonstrates how to bridge your own GL rendering with ImGui, and the
    // texture will keep working even when the window is dragged to another viewport.
    let game_tex = unsafe { create_game_texture(&gl) };
    // On native GL, `glow::Texture` is a small integer handle wrapper (NativeTexture).
    // We treat the underlying GL name (NonZeroU32) as a legacy ImTextureID.
    let game_tex_name = game_tex.0.get(); // NonZeroU32 -> u32
    let game_tex_id = TextureId::from(game_tex_name as u64);

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

    // Initialize SDL3 + OpenGL3 backends (C++ side).
    imgui_sdl3_backend::init_for_opengl(&mut imgui, &window, &gl_context, "#version 150")?;

    // Basic style scaling using the window's display scale.
    let window_scale = window.display_scale();
    {
        let style = imgui.style_mut();
        style.set_font_scale_dpi(window_scale);
    }

    let mut last_frame = Instant::now();

    'main: loop {
        // 1) Pump events: low-level SDL_Event for ImGui backend + high-level Event for us.
        while let Some(raw) = imgui_sdl3_backend::sdl3_poll_event_ll() {
            // Feed the official SDL3 backend.
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
        imgui_sdl3_backend::new_frame(&mut imgui);
        let ui = imgui.frame();

        // Create a dockspace over the main viewport so there is always content.
        ui.dockspace_over_main_viewport();

        // Simple test window that you can tear out as a separate OS window.
        ui.window("Main")
            .size([420.0, 260.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("SDL3 + OpenGL3 + Dear ImGui multi-viewport");
                ui.separator();
                ui.text("Drag this window outside the main viewport to spawn OS windows.");
            });

        // "Game View" window showing a static OpenGL texture. You can drag this
        // window into other viewports (OS windows), and the same texture will be
        // rendered there via the official OpenGL backend.
        ui.window("Game View")
            .size([420.0, 420.0], Condition::FirstUseEver)
            .build(|| {
                let avail = ui.content_region_avail();
                let side = avail[0].min(avail[1]).max(64.0);
                ui.text("OpenGL texture rendered via ImGui Image:");
                ui.image(game_tex_id, [side, side]);
            });

        // 4) Render ImGui.
        let draw_data = imgui.render();

        // Clear the main framebuffer using glow (optional but recommended).
        unsafe {
            use glow::HasContext;

            let (w, h) = window.size_in_pixels();
            gl.viewport(0, 0, w as i32, h as i32);
            gl.clear_color(0.1, 0.12, 0.15, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
        }

        imgui_sdl3_backend::render(draw_data);

        // 5) Optionally render additional platform windows when multi-viewport is enabled.
        if ENABLE_VIEWPORTS {
            let io_flags = imgui.io().config_flags();
            if io_flags.contains(ConfigFlags::VIEWPORTS_ENABLE) {
                imgui.update_platform_windows();
                imgui.render_platform_windows_default();
                let _ = window.gl_make_current(&gl_context);
            }
        }

        // Present the main window.
        window.gl_swap_window();
    }

    imgui_sdl3_backend::shutdown_for_opengl(&mut imgui);
    Ok(())
}

/// Create a glow context from an SDL3 `VideoSubsystem`.
///
/// # Safety
///
/// Call this only after there is a current OpenGL context for the thread.
unsafe fn create_glow_context(video: &sdl3::VideoSubsystem) -> glow::Context {
    use std::ffi::c_void;

    unsafe {
        glow::Context::from_loader_function(|name| {
            video
                .gl_get_proc_address(name)
                .map(|f| f as *const c_void)
                .unwrap_or(std::ptr::null())
        })
    }
}

/// Create a simple gradient texture using raw OpenGL calls via glow.
///
/// This texture can be used as an ImGui `TextureId` (legacy path) with the
/// official `imgui_impl_opengl3` backend by casting the GL texture handle
/// into an integer.
unsafe fn create_game_texture(gl: &glow::Context) -> glow::Texture {
    use glow::HasContext;

    const WIDTH: i32 = 256;
    const HEIGHT: i32 = 256;

    // Generate pixel data on the CPU (simple gradient).
    let mut pixels = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let r = (x as f32 / WIDTH as f32 * 255.0) as u8;
            let g = (y as f32 / HEIGHT as f32 * 255.0) as u8;
            let b = (((x + y) as f32 / (WIDTH + HEIGHT) as f32) * 255.0) as u8;
            pixels.extend_from_slice(&[r, g, b, 255]);
        }
    }

    // Create and upload the texture.
    let tex = gl.create_texture().expect("failed to create GL texture");
    unsafe {
        gl.bind_texture(glow::TEXTURE_2D, Some(tex));
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_S,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_T,
            glow::CLAMP_TO_EDGE as i32,
        );

        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA8 as i32,
            WIDTH,
            HEIGHT,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(Some(&pixels)),
        );

        gl.bind_texture(glow::TEXTURE_2D, None);
    }
    tex
}

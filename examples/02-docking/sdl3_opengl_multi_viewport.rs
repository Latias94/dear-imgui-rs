//! SDL3 + OpenGL3 multi-viewport example.
//!
//! This experimental example drives Dear ImGui using:
//! - an SDL3 window and OpenGL context;
//! - the official `imgui_impl_sdl3.cpp` and `imgui_impl_opengl3.cpp` backends;
//! - the high-level `dear-imgui-rs` API.
//!
//! It requires the `dear-imgui-sdl3` `opengl3-renderer` feature.
//!
//! Run with:
//! `cargo run -p dear-imgui-examples --bin sdl3_opengl_multi_viewport --features multi-viewport,sdl3-opengl3`

use std::cell::RefCell;
use std::error::Error;
use std::time::Instant;

use dear_imgui_examples::sdl3_callbacks::{
    Sdl3CallbackEventHandoff, configure_main_callback_rate, requests_exit,
};
use dear_imgui_rs::{Condition, ConfigFlags, Context, TextureId};
use dear_imgui_sdl3::{self as imgui_sdl3_backend, GamepadMode, Sdl3OpenGl3Backend};
use sdl3::video::{GLProfile, SwapInterval, WindowPos};
use sdl3_main::{AppResult, AppResultWithState, MainThreadData, app_impl};

const ENABLE_VIEWPORTS: bool = true;

struct OpenGlApp {
    events: Sdl3CallbackEventHandoff,
    main: MainThreadData<RefCell<MainData>>,
}

struct MainData {
    sdl3_backend: Sdl3OpenGl3Backend,
    imgui: Context,
    game_tex: glow::Texture,
    gl: glow::Context,
    gl_context: sdl3::video::GLContext,
    window: sdl3::video::Window,
    _video: sdl3::VideoSubsystem,
    _sdl: sdl3::Sdl,
    last_frame: Instant,
}

impl OpenGlApp {
    fn new() -> Result<Self, Box<dyn Error>> {
        configure_main_callback_rate();
        imgui_sdl3_backend::enable_native_ime_ui();

        let sdl = sdl3::init()?;
        let video = sdl.video()?;

        let gl_attr = video.gl_attr();
        gl_attr.set_context_version(3, 2);
        gl_attr.set_context_profile(GLProfile::Core);
        gl_attr.set_depth_size(0);

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
            .map_err(|error| format!("failed to create SDL3 window: {error}"))?;

        let gl_context = window
            .gl_create_context()
            .map_err(|error| format!("SDL_GL_CreateContext failed: {error}"))?;
        window
            .gl_make_current(&gl_context)
            .map_err(|error| format!("SDL_GL_MakeCurrent failed: {error}"))?;
        let _ = video.gl_set_swap_interval(SwapInterval::VSync);
        window.set_position(WindowPos::Centered, WindowPos::Centered);
        window.show();

        // SAFETY: the window's OpenGL context is current on this thread.
        let gl = unsafe { create_glow_context(&video) };
        // SAFETY: the glow context was created from the current OpenGL context.
        let game_tex = unsafe { create_game_texture(&gl) };

        let mut imgui = Context::create();
        let main_scale = window.display_scale();
        let main_scale = if main_scale.is_finite() && main_scale > 0.0 {
            main_scale
        } else {
            1.0
        };
        {
            let io = imgui.io_mut();
            let mut flags = io.config_flags();
            flags.insert(ConfigFlags::DOCKING_ENABLE);
            if ENABLE_VIEWPORTS {
                flags.insert(ConfigFlags::VIEWPORTS_ENABLE);
            }
            io.set_config_flags(flags);
            io.set_config_dpi_scale_fonts(true);
            io.set_config_dpi_scale_viewports(true);
        }
        {
            let style = imgui.style_mut();
            style.scale_all_sizes(main_scale);
            style.set_font_scale_dpi(main_scale);
        }

        // SAFETY: the window and GL context are retained until explicit backend shutdown.
        let mut sdl3_backend =
            unsafe { Sdl3OpenGl3Backend::init(&mut imgui, &window, &gl_context, "#version 150")? };
        sdl3_backend.set_gamepad_mode(&mut imgui, GamepadMode::AutoAll)?;

        Ok(Self {
            events: Sdl3CallbackEventHandoff::default(),
            main: MainThreadData::assert_new(RefCell::new(MainData {
                sdl3_backend,
                imgui,
                game_tex,
                gl,
                gl_context,
                window,
                _video: video,
                _sdl: sdl,
                last_frame: Instant::now(),
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
        }
        AppResult::Continue
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
        ui.dockspace_over_main_viewport();

        ui.window("Main")
            .size([420.0, 260.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("SDL3 + OpenGL3 + Dear ImGui multi-viewport");
                ui.separator();
                ui.text("Drag this window outside the main viewport to spawn OS windows.");
                ui.text("Gamepad: SDL3 backend in AutoAll mode (all controllers merged)");
            });

        let game_tex_id = TextureId::from(main.game_tex.0.get());
        ui.window("Game View")
            .size([420.0, 420.0], Condition::FirstUseEver)
            .build(|| {
                let avail = ui.content_region_avail();
                let side = avail[0].min(avail[1]).max(64.0);
                ui.text("OpenGL texture rendered via ImGui Image:");
                ui.image(game_tex_id, [side, side]);
            });

        let draw_data = main.imgui.render();
        unsafe {
            use glow::HasContext;

            let (width, height) = main.window.size_in_pixels();
            main.gl.viewport(0, 0, width as i32, height as i32);
            main.gl.clear_color(0.1, 0.12, 0.15, 1.0);
            main.gl.clear(glow::COLOR_BUFFER_BIT);
        }
        main.sdl3_backend.render(draw_data)?;

        if ENABLE_VIEWPORTS
            && main
                .imgui
                .io()
                .config_flags()
                .contains(ConfigFlags::VIEWPORTS_ENABLE)
        {
            main.imgui.update_platform_windows();
            main.imgui.render_platform_windows_default();
            main.window.gl_make_current(&main.gl_context)?;
        }
        main.window.gl_swap_window();
        Ok(())
    }

    fn shutdown(&self) {
        let mut main_guard = self.main.assert_get().borrow_mut();
        let main = &mut *main_guard;
        if let Err(error) = main.sdl3_backend.shutdown(&mut main.imgui) {
            eprintln!("SDL3 OpenGL backend shutdown failed: {error}");
        }
    }
}

#[app_impl]
impl OpenGlApp {
    fn app_init() -> AppResultWithState<Box<Self>> {
        match Self::new() {
            Ok(app) => AppResultWithState::Continue(Box::new(app)),
            Err(error) => {
                eprintln!("failed to initialize SDL3 OpenGL example: {error}");
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
                eprintln!("SDL3 OpenGL frame failed: {error}");
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
                .map(|function| function as *const c_void)
                .unwrap_or(std::ptr::null())
        })
    }
}

/// Create the gradient texture displayed by the game-view window.
///
/// # Safety
///
/// The supplied context must be current for the calling thread.
unsafe fn create_game_texture(gl: &glow::Context) -> glow::Texture {
    use glow::HasContext;

    const WIDTH: i32 = 256;
    const HEIGHT: i32 = 256;

    let mut pixels = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let red = (x as f32 / WIDTH as f32 * 255.0) as u8;
            let green = (y as f32 / HEIGHT as f32 * 255.0) as u8;
            let blue = (((x + y) as f32 / (WIDTH + HEIGHT) as f32) * 255.0) as u8;
            pixels.extend_from_slice(&[red, green, blue, 255]);
        }
    }

    let texture = unsafe { gl.create_texture() }.expect("failed to create GL texture");
    unsafe {
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
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
    texture
}

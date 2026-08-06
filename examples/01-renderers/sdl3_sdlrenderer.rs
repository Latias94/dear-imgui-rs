//! SDL3 + SDLRenderer3 renderer example
//!
//! Run with:
//!   cargo run -p dear-imgui-examples --bin sdl3_sdlrenderer --features sdl3-sdlrenderer3

use std::cell::RefCell;
use std::error::Error;

use dear_imgui_examples::sdl3_callbacks::{
    Sdl3CallbackEventHandoff, configure_main_callback_rate, requests_exit,
};
use dear_imgui_rs::{Condition, Context};
use dear_imgui_sdl3::{self as imgui_sdl3_backend, Sdl3RendererBackend};
use sdl3::pixels::Color;
use sdl3::{Sdl, VideoSubsystem};
use sdl3_main::{AppResult, AppResultWithState, MainThreadData, app_impl};

struct SdlRendererApp {
    main: MainThreadData<RefCell<MainData>>,
    events: Sdl3CallbackEventHandoff,
}

struct MainData {
    sdl3_backend: Sdl3RendererBackend,
    imgui: Context,
    canvas: sdl3::render::Canvas<sdl3::video::Window>,
    _video: VideoSubsystem,
    _sdl: Sdl,
    show_demo: bool,
    show_debug: bool,
    show_about: bool,
}

impl SdlRendererApp {
    fn new() -> Result<Self, Box<dyn Error>> {
        configure_main_callback_rate();
        sdl3::hint::set(sdl3::hint::names::RENDER_VSYNC, "1");
        imgui_sdl3_backend::enable_native_ime_ui();

        let sdl_ctx = sdl3::init()?;
        let video = sdl_ctx.video()?;
        let main_scale = video
            .get_primary_display()?
            .get_content_scale()
            .unwrap_or(1.0);
        let window = video
            .window(
                "SDL Test",
                (1200.0 * main_scale) as u32,
                (720.0 * main_scale) as u32,
            )
            .position_centered()
            .resizable()
            .high_pixel_density()
            .build()?;

        let mut canvas = window.into_canvas();
        canvas.set_draw_color(Color::RGB(0, 255, 255));
        canvas.clear();
        canvas.present();

        let mut imgui = Context::create();
        imgui.set_ini_filename(None::<String>)?;
        imgui.set_log_filename(None::<String>)?;

        // SAFETY: `canvas` owns the Window and SDL_Renderer through shutdown or Context teardown.
        let sdl3_backend =
            unsafe { Sdl3RendererBackend::init(&mut imgui, canvas.window(), &canvas)? };

        Ok(Self {
            main: MainThreadData::assert_new(RefCell::new(MainData {
                sdl3_backend,
                imgui,
                canvas,
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
            if requests_exit(&event, main.canvas.window().id()) {
                return Ok(AppResult::Success);
            }
        }

        main.canvas.clear();
        main.sdl3_backend.new_frame(&mut main.imgui)?;
        let ui = main.imgui.frame();

        ui.window("SDL3 + IMGUI")
            .size([400.0, 200.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Dear ImGui running on SDL3 + SDL_Renderer");
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

        let pending_frame = main.imgui.render(main.sdl3_backend.consumer());
        main.sdl3_backend.render(pending_frame, &main.canvas)?;
        main.canvas.present();

        Ok(AppResult::Continue)
    }

    fn shutdown(&self) {
        let mut main_guard = self.main.assert_get().borrow_mut();
        let main = &mut *main_guard;
        if let Err(error) = main.sdl3_backend.shutdown(&mut main.imgui) {
            eprintln!("SDL3 SDLRenderer backend shutdown failed: {error}");
        }
    }
}

#[app_impl]
impl SdlRendererApp {
    fn app_init() -> AppResultWithState<Box<Self>> {
        match Self::new() {
            Ok(app) => AppResultWithState::Continue(Box::new(app)),
            Err(error) => {
                eprintln!("failed to initialize SDL3 SDLRenderer example: {error}");
                AppResultWithState::Failure(None)
            }
        }
    }

    fn app_iterate(&self) -> AppResult {
        match self.iterate() {
            Ok(result) => result,
            Err(error) => {
                eprintln!("SDL3 SDLRenderer frame failed: {error}");
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

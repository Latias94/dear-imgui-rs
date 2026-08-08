//! SDL3 + Glow multi-viewport example.
//!
//! This example drives Dear ImGui using:
//! - SDL3 for the main window, input, and secondary platform windows;
//! - the Rust Glow renderer backend (`dear-imgui-glow`);
//! - the high-level `dear-imgui-rs` API.
//!
//! It does not use the official OpenGL3 renderer from `dear-imgui-sdl3`.
//!
//! Run with:
//! ```text
//! cargo run -p dear-imgui-examples --bin sdl3_glow_multi_viewport \
//!     --features sdl3-glow-multi-viewport
//! ```

// The shared lifecycle also exposes evidence consumed only by the private runtime probe.
#[allow(dead_code)]
#[path = "../support/sdl3_glow_multi_viewport_runtime.rs"]
mod sdl3_glow_multi_viewport_runtime;

use sdl3_glow_multi_viewport_runtime::{GlowApp, InteractiveScenario};
use sdl3_main::{AppResult, AppResultWithState, app_impl};

struct ExampleApp(GlowApp<InteractiveScenario>);

#[app_impl]
impl ExampleApp {
    fn app_init() -> AppResultWithState<Box<Self>> {
        match GlowApp::new(InteractiveScenario) {
            Ok(app) => AppResultWithState::Continue(Box::new(Self(app))),
            Err(error) => {
                eprintln!("failed to initialize SDL3 Glow example: {error}");
                AppResultWithState::Failure(None)
            }
        }
    }

    fn app_iterate(&self) -> AppResult {
        self.0.iterate()
    }

    fn app_event(&self, raw: &sdl3::sys::events::SDL_Event) -> AppResult {
        // SAFETY: SDL supplies a valid event whose transient payload remains live for this call.
        unsafe { self.0.queue_event(raw) };
        AppResult::Continue
    }

    fn app_quit(state: Option<&Self>) {
        if let Some(app) = state {
            app.0.shutdown();
        }
    }
}

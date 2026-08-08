//! SDL3 callback-mode helpers shared by the examples.
//!
//! Event payload ownership belongs to `dear-imgui-sdl3`; this module contains only the
//! application-level callback cadence and exit policy used by the examples.

pub use dear_imgui_sdl3::{Sdl3CallbackEvent, Sdl3CallbackEventHandoff};

/// Prefer a responsive frame cadence while a native platform modal loop drives callbacks.
pub fn configure_main_callback_rate() {
    sdl3::hint::set_with_priority(
        sdl3::hint::names::MAIN_CALLBACK_RATE,
        "120",
        &sdl3::hint::Hint::Default,
    );
}

/// Whether an example should leave its callback loop after this event.
pub fn requests_exit(event: &Sdl3CallbackEvent, main_window_id: u32) -> bool {
    event.requests_exit(main_window_id)
}

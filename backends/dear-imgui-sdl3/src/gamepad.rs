use super::*;

/// Gamepad handling mode used by the SDL3 backend.
///
/// This controls how many SDL3 gamepads are opened and merged into ImGui's
/// gamepad input state.
#[derive(Copy, Clone, Debug)]
pub enum GamepadMode {
    /// Automatically open the first available gamepad (Dear ImGui default).
    AutoFirst,
    /// Automatically open all available gamepads and merge their state.
    AutoAll,
}

pub(crate) fn set_gamepad_mode(mode: GamepadMode) {
    unsafe {
        match mode {
            GamepadMode::AutoFirst => ffi::ImGui_ImplSDL3_SetGamepadMode_AutoFirst_Rust(),
            GamepadMode::AutoAll => ffi::ImGui_ImplSDL3_SetGamepadMode_AutoAll_Rust(),
        }
    }
}

pub(crate) unsafe fn set_gamepad_mode_manual(gamepads: &[*mut sdl3_sys::gamepad::SDL_Gamepad]) {
    unsafe {
        ffi::ImGui_ImplSDL3_SetGamepadMode_Manual_Rust(gamepads.as_ptr(), gamepads.len() as i32);
    }
}

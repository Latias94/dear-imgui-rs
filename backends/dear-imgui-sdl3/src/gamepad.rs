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

/// Configure how the SDL3 backend handles gamepads.
///
/// Call this after backend initialization if you want a mode other than the
/// default `AutoFirst`.
///
/// This thin compatibility helper operates on Dear ImGui's current context.
/// Prefer [`set_gamepad_mode_for_context`] or an RAII backend owner in
/// multi-context code.
pub fn set_gamepad_mode(mode: GamepadMode) {
    unsafe {
        match mode {
            GamepadMode::AutoFirst => ffi::ImGui_ImplSDL3_SetGamepadMode_AutoFirst_Rust(),
            GamepadMode::AutoAll => ffi::ImGui_ImplSDL3_SetGamepadMode_AutoAll_Rust(),
        }
    }
}

/// Configure how the SDL3 backend handles gamepads for a specific context.
pub fn set_gamepad_mode_for_context(imgui: &mut Context, mode: GamepadMode) {
    with_context(imgui, "set_gamepad_mode_for_context()", || {
        set_gamepad_mode(mode);
    });
}

/// Configure SDL3 backend to use manual gamepad selection.
///
/// This thin compatibility helper operates on Dear ImGui's current context.
/// Prefer [`set_gamepad_mode_manual_for_context`] or an RAII backend owner in
/// multi-context code.
///
/// # Safety
///
/// - The caller must ensure every pointer in `gamepads` is a valid, opened `SDL_Gamepad`.
/// - The caller is responsible for keeping those gamepads alive for the duration of ImGui usage.
/// - The slice itself is only read during this call; the backend copies the pointers.
pub unsafe fn set_gamepad_mode_manual(gamepads: &[*mut sdl3_sys::gamepad::SDL_Gamepad]) {
    unsafe {
        ffi::ImGui_ImplSDL3_SetGamepadMode_Manual_Rust(gamepads.as_ptr(), gamepads.len() as i32);
    }
}

/// Configure SDL3 backend to use manual gamepad selection for a specific context.
///
/// # Safety
///
/// - The caller must ensure every pointer in `gamepads` is a valid, opened `SDL_Gamepad`.
/// - The caller is responsible for keeping those gamepads alive for the duration of ImGui usage.
/// - The slice itself is only read during this call; the backend copies the pointers.
pub unsafe fn set_gamepad_mode_manual_for_context(
    imgui: &mut Context,
    gamepads: &[*mut sdl3_sys::gamepad::SDL_Gamepad],
) {
    with_context(imgui, "set_gamepad_mode_manual_for_context()", || unsafe {
        set_gamepad_mode_manual(gamepads);
    });
}

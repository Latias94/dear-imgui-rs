use super::*;

/// Gamepad handling mode used by the SDL3 backend.
///
/// This controls how many SDL3 gamepads are opened and merged into ImGui's
/// gamepad input state.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum GamepadMode {
    /// Automatically open the first available gamepad (Dear ImGui default).
    AutoFirst,
    /// Automatically open all available gamepads and merge their state.
    AutoAll,
}

/// Mouse capture policy used by the SDL3 platform backend.
///
/// Mouse capture keeps drag coordinates updating after the pointer leaves an SDL window. The
/// upstream backend defaults to [`EnabledAfterDrag`](Self::EnabledAfterDrag) on X11,
/// [`Enabled`](Self::Enabled) on other capable desktop drivers, and
/// [`Disabled`](Self::Disabled) when the active SDL video driver cannot provide global mouse
/// state and capture.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MouseCaptureMode {
    /// Capture as soon as any mouse button is held.
    Enabled,
    /// Wait until Dear ImGui recognizes a drag before capturing the mouse.
    ///
    /// This is the upstream X11 default because a debugger break while capture is active can
    /// otherwise leave the desktop pointer temporarily captured.
    EnabledAfterDrag,
    /// Disable capture and immediately release any capture owned by the backend.
    Disabled,
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

pub(crate) fn set_mouse_capture_mode(mode: MouseCaptureMode) {
    unsafe {
        match mode {
            MouseCaptureMode::Enabled => ffi::ImGui_ImplSDL3_SetMouseCaptureMode_Enabled_Rust(),
            MouseCaptureMode::EnabledAfterDrag => {
                ffi::ImGui_ImplSDL3_SetMouseCaptureMode_EnabledAfterDrag_Rust()
            }
            MouseCaptureMode::Disabled => ffi::ImGui_ImplSDL3_SetMouseCaptureMode_Disabled_Rust(),
        }
    }
}

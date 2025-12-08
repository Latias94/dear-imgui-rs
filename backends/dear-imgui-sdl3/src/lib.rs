//! SDL3 + OpenGL backend bindings for `dear-imgui-rs`.
//!
//! This crate is a thin, opinionated wrapper around the official C++ SDL3
//! platform backend (`imgui_impl_sdl3.cpp`) and OpenGL3 renderer backend
//! (`imgui_impl_opengl3.cpp`) compiled from the upstream Dear ImGui tree
//! used by `dear-imgui-sys`.
//!
//! The intent is to provide a simple, safe-ish API that:
//! - plugs into an existing `dear-imgui-rs::Context`
//! - integrates with an SDL3 window and OpenGL context
//! - supports Dear ImGui multi-viewport via the official backend behavior.

use std::ffi::{CString, c_char, c_void};

use dear_imgui_rs::{Context, TextureData, render::DrawData};
use dear_imgui_sys as sys;
use sdl3::video::{GLContext, Window};
use sdl3_sys::events::SDL_Event;

/// FFI bindings to the C wrappers defined in `wrapper.cpp`.
mod ffi {
    use super::*;

    unsafe extern "C" {
        pub fn ImGui_ImplSDL3_InitForOpenGL_Rust(
            window: *mut sdl3_sys::video::SDL_Window,
            sdl_gl_context: *mut c_void,
        ) -> bool;
        pub fn ImGui_ImplSDL3_InitForOther_Rust(window: *mut sdl3_sys::video::SDL_Window) -> bool;
        pub fn ImGui_ImplSDL3_Shutdown_Rust();
        pub fn ImGui_ImplSDL3_NewFrame_Rust();
        pub fn ImGui_ImplSDL3_ProcessEvent_Rust(event: *const SDL_Event) -> bool;

        pub fn ImGui_ImplOpenGL3_Init_Rust(glsl_version: *const c_char) -> bool;
        pub fn ImGui_ImplOpenGL3_Shutdown_Rust();
        pub fn ImGui_ImplOpenGL3_NewFrame_Rust();
        pub fn ImGui_ImplOpenGL3_RenderDrawData_Rust(draw_data: *mut sys::ImDrawData);
        pub fn ImGui_ImplOpenGL3_UpdateTexture_Rust(tex: *mut sys::ImTextureData);

        pub fn ImGui_ImplSDL3_SetGamepadMode_AutoFirst_Rust();
        pub fn ImGui_ImplSDL3_SetGamepadMode_AutoAll_Rust();
    }
}

/// Errors that can occur when setting up the SDL3 + OpenGL backend.
#[derive(Debug, thiserror::Error)]
pub enum Sdl3BackendError {
    #[error("ImGui_ImplSDL3_InitForOpenGL returned false")]
    Sdl3InitFailed,
    #[error("ImGui_ImplOpenGL3_Init returned false")]
    OpenGlInitFailed,
    #[error("Invalid GLSL version string")]
    InvalidGlslVersion,
}

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
/// Call this after [`init_for_opengl`] or [`init_for_other`] if you want a
/// mode other than the default `AutoFirst`.
pub fn set_gamepad_mode(mode: GamepadMode) {
    unsafe {
        match mode {
            GamepadMode::AutoFirst => ffi::ImGui_ImplSDL3_SetGamepadMode_AutoFirst_Rust(),
            GamepadMode::AutoAll => ffi::ImGui_ImplSDL3_SetGamepadMode_AutoAll_Rust(),
        }
    }
}

/// Enable native IME UI for SDL3 (recommended on IME-heavy platforms).
///
/// This should be called before creating any SDL3 windows so that the
/// underlying backend can display the OS IME UI correctly.
pub fn enable_native_ime_ui() {
    // Best-effort: ignore return value; missing hints are not fatal.
    let _ = sdl3::hint::set("SDL_HINT_IME_SHOW_UI", "1");
}

/// Initialize the Dear ImGui SDL3 + OpenGL3 backends.
///
/// This assumes that:
/// - a `dear_imgui_rs::Context` already exists;
/// - `window` has an active OpenGL context (`gl_context`);
/// - the same context will be current when rendering.
///
/// `glsl_version` should be a GLSL version string such as `"#version 150"`.
pub fn init_for_opengl(
    _imgui: &mut Context,
    window: &Window,
    gl_context: &GLContext,
    glsl_version: &str,
) -> Result<(), Sdl3BackendError> {
    let glsl = CString::new(glsl_version).map_err(|_| Sdl3BackendError::InvalidGlslVersion)?;

    let sdl_window = window.raw();
    let sdl_gl = unsafe { gl_context.raw() };

    unsafe {
        if !ffi::ImGui_ImplSDL3_InitForOpenGL_Rust(sdl_window, sdl_gl as *mut c_void) {
            return Err(Sdl3BackendError::Sdl3InitFailed);
        }
        if !ffi::ImGui_ImplOpenGL3_Init_Rust(glsl.as_ptr()) {
            return Err(Sdl3BackendError::OpenGlInitFailed);
        }
    }

    Ok(())
}

/// Initialize only the Dear ImGui SDL3 *platform* backend for an OpenGL context.
///
/// This is useful when you want to use a Rust renderer (e.g. `dear-imgui-glow`)
/// instead of the official C++ OpenGL3 renderer. It:
/// - configures the SDL3 platform backend (including multi-viewport support);
/// - does **not** initialize `imgui_impl_opengl3`.
///
/// This assumes that:
/// - a `dear_imgui_rs::Context` already exists;
/// - `window` has an active OpenGL context (`gl_context`);
/// - the same context will be current when rendering.
pub fn init_platform_for_opengl(
    _imgui: &mut Context,
    window: &Window,
    gl_context: &GLContext,
) -> Result<(), Sdl3BackendError> {
    let sdl_window = window.raw();
    let sdl_gl = unsafe { gl_context.raw() };

    unsafe {
        if !ffi::ImGui_ImplSDL3_InitForOpenGL_Rust(sdl_window, sdl_gl as *mut c_void) {
            return Err(Sdl3BackendError::Sdl3InitFailed);
        }
    }

    Ok(())
}

/// Initialize the Dear ImGui SDL3 platform backend only.
///
/// This is useful when using a non-OpenGL renderer (e.g. WGPU) and only
/// want SDL3 to drive the platform layer.
///
/// This assumes that:
/// - a `dear_imgui_rs::Context` already exists;
/// - `window` is a valid SDL3 window handle.
pub fn init_for_other(_imgui: &mut Context, window: &Window) -> Result<(), Sdl3BackendError> {
    let sdl_window = window.raw();

    unsafe {
        if !ffi::ImGui_ImplSDL3_InitForOther_Rust(sdl_window) {
            return Err(Sdl3BackendError::Sdl3InitFailed);
        }
    }

    Ok(())
}

/// Shutdown the SDL3 + OpenGL3 backends.
///
/// Call this before destroying the ImGui context or the SDL3 window.
pub fn shutdown_for_opengl(_imgui: &mut Context) {
    unsafe {
        ffi::ImGui_ImplOpenGL3_Shutdown_Rust();
        ffi::ImGui_ImplSDL3_Shutdown_Rust();
    }
}

/// Shutdown the SDL3 platform backend only.
///
/// This is the counterpart to [`init_for_other`] and should be called before
/// destroying the ImGui context when using a non-OpenGL renderer (e.g. WGPU).
pub fn shutdown(_imgui: &mut Context) {
    unsafe {
        ffi::ImGui_ImplSDL3_Shutdown_Rust();
    }
}

/// Begin a new ImGui frame for SDL3 + OpenGL.
///
/// Call this before `imgui.frame()`.
pub fn new_frame(_imgui: &mut Context) {
    unsafe {
        ffi::ImGui_ImplOpenGL3_NewFrame_Rust();
        ffi::ImGui_ImplSDL3_NewFrame_Rust();
    }
}

/// Begin a new ImGui frame for SDL3 platform backend only.
///
/// This is intended for non-OpenGL renderers such as WGPU.
pub fn sdl3_new_frame(_imgui: &mut Context) {
    unsafe {
        ffi::ImGui_ImplSDL3_NewFrame_Rust();
    }
}

/// Poll the next SDL3 event as a low-level `SDL_Event`.
///
/// This mirrors the C++ SDL3 examples and is useful when you want to feed both
/// Dear ImGui and your own event handling from the same low-level event stream.
pub fn sdl3_poll_event_ll() -> Option<SDL_Event> {
    let mut raw = std::mem::MaybeUninit::<SDL_Event>::uninit();
    let has_event = unsafe { sdl3_sys::events::SDL_PollEvent(raw.as_mut_ptr()) };
    if has_event {
        Some(unsafe { raw.assume_init() })
    } else {
        None
    }
}

/// Process a single low-level SDL3 event with ImGui's SDL3 backend.
///
/// Returns `true` if Dear ImGui consumed the event.
pub fn process_sys_event(event: &SDL_Event) -> bool {
    unsafe { ffi::ImGui_ImplSDL3_ProcessEvent_Rust(event) }
}

/// Render Dear ImGui draw data using the OpenGL3 backend.
///
/// This assumes an OpenGL context is current.
pub fn render(draw_data: &DrawData) {
    // Render main viewport
    unsafe {
        let raw = draw_data as *const DrawData as *mut sys::ImDrawData;
        ffi::ImGui_ImplOpenGL3_RenderDrawData_Rust(raw);
    }
}

/// Update a single ImGui texture using the OpenGL3 backend.
///
/// This is an advanced helper that delegates to `ImGui_ImplOpenGL3_UpdateTexture`.
pub fn update_texture(tex: &mut TextureData) {
    unsafe {
        let raw = tex as *mut TextureData as *mut sys::ImTextureData;
        ffi::ImGui_ImplOpenGL3_UpdateTexture_Rust(raw);
    }
}

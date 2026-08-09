//! SDL3 OpenGL helpers whose Rust SDL wrapper currently drops native failure results.

use sdl3::video::Window;

/// Swap an OpenGL window and preserve SDL's native success or failure result.
pub fn swap_window(window: &Window) -> Result<(), sdl3::Error> {
    // SAFETY: `Window::raw()` is valid for the lifetime of the borrowed SDL window.
    if unsafe { sdl3::sys::video::SDL_GL_SwapWindow(window.raw()) } {
        Ok(())
    } else {
        Err(sdl3::get_error())
    }
}

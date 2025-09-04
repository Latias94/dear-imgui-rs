use std::marker::PhantomData;
use std::ptr::NonNull;
use std::sync::Mutex;

use crate::error::{ImGuiError, Result};
use crate::fonts::FontAtlas;
use crate::frame::Frame;
use crate::io::Io;
use crate::style::Style;
use dear_imgui_sys as sys;

// Global context lock to ensure thread safety
static CONTEXT_LOCK: Mutex<()> = Mutex::new(());

/// Dear ImGui context
///
/// This is the main entry point for Dear ImGui. A context manages all the state
/// for Dear ImGui and must be created before any other Dear ImGui operations.
///
/// # Example
///
/// ```rust,no_run
/// use dear_imgui::Context;
///
/// let mut ctx = Context::new().unwrap();
/// let mut frame = ctx.frame();
/// // Use the frame to build UI...
/// ```
pub struct Context {
    raw: NonNull<sys::ImGuiContext>,
    _marker: PhantomData<*mut sys::ImGuiContext>,
}

impl Context {
    /// Create a new Dear ImGui context
    ///
    /// # Errors
    ///
    /// Returns `ImGuiError::ContextCreationFailed` if the context could not be created.
    pub fn new() -> Result<Self> {
        let _guard = CONTEXT_LOCK.lock().unwrap();

        let raw = unsafe {
            let ptr = sys::ImGui_CreateContext(std::ptr::null_mut());
            NonNull::new(ptr).ok_or(ImGuiError::ContextCreationFailed)?
        };

        // Initialize IO with default values
        unsafe {
            sys::ImGui_SetCurrentContext(raw.as_ptr());
            let io = sys::ImGui_GetIO();
            (*io).DisplaySize.x = 800.0;
            (*io).DisplaySize.y = 600.0;
            (*io).DisplayFramebufferScale.x = 1.0;
            (*io).DisplayFramebufferScale.y = 1.0;
            (*io).DeltaTime = 1.0 / 60.0; // 60 FPS

            // Build font atlas
            let fonts = (*io).Fonts;
            let mut pixels: *mut u8 = std::ptr::null_mut();
            let mut width: i32 = 0;
            let mut height: i32 = 0;
            let mut bytes_per_pixel: i32 = 0;

            // Get texture data to build the font atlas
            (*fonts).GetTexDataAsRGBA32(
                &mut pixels as *mut *mut u8,
                &mut width as *mut i32,
                &mut height as *mut i32,
                &mut bytes_per_pixel as *mut i32,
            );

            // Set a dummy texture ID (normally this would be set by the renderer)
            (*fonts).__bindgen_anon_1.TexID = sys::ImTextureRef {
                _TexData: std::ptr::null_mut(),
                _TexID: 1, // Dummy texture ID
            };
        }

        Ok(Context {
            raw,
            _marker: PhantomData,
        })
    }

    /// Begin a new frame
    ///
    /// This method starts a new Dear ImGui frame and returns a `Frame` object
    /// that can be used to build the UI for this frame.
    ///
    /// The frame will automatically end when the returned `Frame` is dropped.
    pub fn frame(&mut self) -> Frame<'_> {
        unsafe {
            sys::ImGui_SetCurrentContext(self.raw.as_ptr());
            sys::ImGui_NewFrame();
        }

        Frame::new(self)
    }

    /// Get access to the font atlas
    pub fn fonts(&mut self) -> FontAtlas {
        unsafe {
            let io = sys::ImGui_GetIO();
            FontAtlas::from_raw((*io).Fonts)
        }
    }

    /// Get immutable access to the IO system
    pub fn io(&self) -> Io {
        unsafe { Io::from_raw(sys::ImGui_GetIO()) }
    }

    /// Get mutable access to the IO system
    pub fn io_mut(&mut self) -> Io {
        unsafe { Io::from_raw(sys::ImGui_GetIO()) }
    }

    /// Get immutable access to the style system
    pub fn style(&self) -> Style {
        unsafe { Style::from_raw(sys::ImGui_GetStyle()) }
    }

    /// Get mutable access to the style system
    pub fn style_mut(&mut self) -> Style {
        unsafe { Style::from_raw(sys::ImGui_GetStyle()) }
    }

    /// Get the raw ImGui context pointer
    ///
    /// # Safety
    ///
    /// This is unsafe because it returns a raw pointer that could be used
    /// to violate memory safety. Only use this if you need to call Dear ImGui
    /// functions that are not yet wrapped by this library.
    pub unsafe fn raw(&self) -> *mut sys::ImGuiContext {
        self.raw.as_ptr()
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        let _guard = CONTEXT_LOCK.lock().unwrap();
        unsafe {
            sys::ImGui_DestroyContext(self.raw.as_ptr());
        }
    }
}

// Context can be sent between threads, but Dear ImGui is not thread-safe,
// so we don't implement Sync
unsafe impl Send for Context {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let _ctx = Context::new().unwrap();
    }

    #[test]
    fn test_frame_creation() {
        let mut ctx = Context::new().unwrap();
        let _frame = ctx.frame();
    }
}

use parking_lot::ReentrantMutex;
use std::cell::UnsafeCell;
use std::ffi::{CStr, CString};
use std::ops::Drop;
use std::path::PathBuf;
use std::ptr;

use crate::fonts::{Font, FontAtlas};
use crate::io::Io;
// use crate::style::Style;
use crate::sys;
use crate::ui::Ui;

/// An imgui context.
///
/// A context needs to be created to access most library functions. Due to current Dear ImGui
/// design choices, at most one active Context can exist at any time. This limitation will likely
/// be removed in a future Dear ImGui version.
///
/// If you need more than one context, you can use suspended contexts. As long as only one context
/// is active at a time, it's possible to have multiple independent contexts.
///
/// # Examples
///
/// Creating a new active context:
/// ```
/// let ctx = dear_imgui::Context::create();
/// // ctx is dropped naturally when it goes out of scope, which deactivates and destroys the
/// // context
/// ```
///
/// Never try to create an active context when another one is active:
///
/// ```should_panic
/// let ctx1 = dear_imgui::Context::create();
///
/// let ctx2 = dear_imgui::Context::create(); // PANIC
/// ```
#[derive(Debug)]
pub struct Context {
    raw: *mut sys::ImGuiContext,
    ini_filename: Option<CString>,
    log_filename: Option<CString>,
    platform_name: Option<CString>,
    renderer_name: Option<CString>,
    ui: crate::Ui,
}

// This mutex needs to be used to guard all public functions that can affect the underlying
// Dear ImGui active context
static CTX_MUTEX: ReentrantMutex<()> = parking_lot::const_reentrant_mutex(());

fn clear_current_context() {
    // TODO: Implement once FFI is working
    // unsafe {
    //     sys::igSetCurrentContext(ptr::null_mut());
    // }
}

fn no_current_context() -> bool {
    // TODO: Implement once FFI is working
    // let ctx = unsafe { sys::igGetCurrentContext() };
    // ctx.is_null()
    true // Placeholder
}

impl Context {
    /// Creates a new active Dear ImGui context.
    ///
    /// # Panics
    ///
    /// Panics if an active context already exists
    pub fn create() -> Context {
        let _guard = CTX_MUTEX.lock();

        if !no_current_context() {
            panic!("A Dear ImGui context is already active");
        }

        // Create the actual ImGui context
        let raw = unsafe { sys::ImGui_CreateContext(ptr::null_mut()) };
        if raw.is_null() {
            panic!("Failed to create Dear ImGui context");
        }

        // Set it as the current context
        unsafe {
            sys::ImGui_SetCurrentContext(raw);
        }

        Context {
            raw,
            ini_filename: None,
            log_filename: None,
            platform_name: None,
            renderer_name: None,
            ui: crate::Ui::new(),
        }
    }

    /// Returns a mutable reference to the active context's IO object
    pub fn io_mut(&mut self) -> &mut Io {
        let _guard = CTX_MUTEX.lock();
        Io::from_raw()
    }

    // /// Returns a reference to the active context's style
    // pub fn style(&self) -> &Style {
    //     let _guard = CTX_MUTEX.lock();
    //     unsafe {
    //         let style_ptr = sys::igGetStyle();
    //         &*(style_ptr as *const Style)
    //     }
    // }

    // /// Returns a mutable reference to the active context's style
    // pub fn style_mut(&mut self) -> &mut Style {
    //     let _guard = CTX_MUTEX.lock();
    //     unsafe {
    //         let style_ptr = sys::igGetStyle();
    //         &mut *(style_ptr as *mut Style)
    //     }
    // }

    /// Get access to the IO structure
    pub fn io(&self) -> &crate::io::Io {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            let io_ptr = sys::ImGui_GetIO();
            &*(io_ptr as *const crate::io::Io)
        }
    }

    /// Get access to the Style structure
    pub fn style(&self) -> crate::style::Style {
        crate::style::Style::from_raw()
    }

    /// Creates a new frame and returns a Ui object for building the interface
    pub fn frame(&mut self) -> &mut crate::Ui {
        let _guard = CTX_MUTEX.lock();

        // Ensure font atlas is built before calling NewFrame
        unsafe {
            let io = sys::ImGui_GetIO();
            let fonts = (*io).Fonts;
            if !(*fonts).TexIsBuilt {
                // Build the font atlas using the main build function
                sys::ImFontAtlasBuildMain(fonts);

                // Mark the texture as built by setting a dummy texture ID
                (*fonts).TexIsBuilt = true;
            }

            sys::ImGui_NewFrame();
        }
        &mut self.ui
    }

    /// Create a new frame with a callback
    pub fn frame_with<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&crate::Ui) -> R,
    {
        let ui = self.frame();
        f(ui)
    }

    // TODO: Implement render once FFI is working
    // /// Renders the frame and returns draw data
    // pub fn render(&mut self) -> &sys::ImDrawData {
    //     let _guard = CTX_MUTEX.lock();
    //     unsafe {
    //         sys::igRender();
    //         &*sys::igGetDrawData()
    //     }
    // }

    /// Push a font onto the font stack
    ///
    /// This makes the given font the current font for subsequent text rendering.
    /// Must be paired with a call to `pop_font()`.
    #[doc(alias = "PushFont")]
    pub fn push_font(&mut self, font: &Font) {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            sys::ImGui_PushFont(font.raw(), 0.0);
        }
    }

    /// Pop a font from the font stack
    ///
    /// This restores the previous font. Must be paired with a call to `push_font()`.
    #[doc(alias = "PopFont")]
    pub fn pop_font(&mut self) {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            sys::ImGui_PopFont();
        }
    }

    /// Get the current font
    #[doc(alias = "GetFont")]
    pub fn current_font(&self) -> &Font {
        let _guard = CTX_MUTEX.lock();
        unsafe { Font::from_raw(sys::ImGui_GetFont()) }
    }

    /// Get the current font size
    #[doc(alias = "GetFontSize")]
    pub fn current_font_size(&self) -> f32 {
        let _guard = CTX_MUTEX.lock();
        unsafe { sys::ImGui_GetFontSize() }
    }

    /// Get the font atlas from the IO structure
    pub fn font_atlas(&self) -> &FontAtlas {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            let io = sys::ImGui_GetIO();
            let atlas_ptr = (*io).Fonts;
            &*(atlas_ptr as *const FontAtlas)
        }
    }

    /// Get a mutable reference to the font atlas from the IO structure
    pub fn font_atlas_mut(&mut self) -> &mut FontAtlas {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            let io = sys::ImGui_GetIO();
            let atlas_ptr = (*io).Fonts;
            &mut *(atlas_ptr as *mut FontAtlas)
        }
    }

    /// Returns a mutable reference to the font atlas (alias for font_atlas_mut)
    ///
    /// This provides compatibility with imgui-rs naming convention
    pub fn fonts(&mut self) -> &mut FontAtlas {
        self.font_atlas_mut()
    }

    // TODO: Implement these methods once FFI is working
    // /// Sets the INI filename for settings persistence
    // pub fn set_ini_filename<P: Into<PathBuf>>(&mut self, filename: Option<P>) {
    //     let _guard = CTX_MUTEX.lock();

    //     self.ini_filename = filename.map(|f| {
    //         CString::new(f.into().to_string_lossy().as_bytes())
    //             .expect("Invalid filename")
    //     });

    //     unsafe {
    //         let ptr = self.ini_filename
    //             .as_ref()
    //             .map(|s| s.as_ptr())
    //             .unwrap_or(ptr::null());
    //         (*sys::igGetIO()).IniFilename = ptr;
    //     }
    // }
}

impl Drop for Context {
    fn drop(&mut self) {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            if !self.raw.is_null() {
                if sys::ImGui_GetCurrentContext() == self.raw {
                    clear_current_context();
                }
                sys::ImGui_DestroyContext(self.raw);
            }
        }
    }
}

unsafe impl Send for Context {}
unsafe impl Sync for Context {}

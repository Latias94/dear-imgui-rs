use parking_lot::ReentrantMutex;
use std::cell::UnsafeCell;
use std::ffi::{CStr, CString};
use std::ops::Drop;
use std::path::PathBuf;
use std::ptr;

use crate::clipboard::{ClipboardBackend, ClipboardContext};
use crate::fonts::{Font, FontAtlas, SharedFontAtlas};
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
    shared_font_atlas: Option<SharedFontAtlas>,
    ini_filename: Option<CString>,
    log_filename: Option<CString>,
    platform_name: Option<CString>,
    renderer_name: Option<CString>,
    // We need to box this because we hand imgui a pointer to it,
    // and we don't want to deal with finding `clipboard_ctx`.
    // We also put it in an UnsafeCell since we're going to give
    // imgui a mutable pointer to it.
    clipboard_ctx: Box<UnsafeCell<ClipboardContext>>,
    ui: crate::Ui,
}

// This mutex needs to be used to guard all public functions that can affect the underlying
// Dear ImGui active context
static CTX_MUTEX: ReentrantMutex<()> = parking_lot::const_reentrant_mutex(());

fn clear_current_context() {
    unsafe {
        sys::ImGui_SetCurrentContext(ptr::null_mut());
    }
}

fn no_current_context() -> bool {
    let ctx = unsafe { sys::ImGui_GetCurrentContext() };
    ctx.is_null()
}

impl Context {
    /// Creates a new active Dear ImGui context.
    ///
    /// # Panics
    ///
    /// Panics if an active context already exists
    pub fn create() -> Context {
        Self::create_internal(None)
    }

    /// Creates a new active Dear ImGui context with a shared font atlas.
    ///
    /// # Panics
    ///
    /// Panics if an active context already exists
    pub fn create_with_shared_font_atlas(shared_font_atlas: SharedFontAtlas) -> Context {
        Self::create_internal(Some(shared_font_atlas))
    }

    fn create_internal(mut shared_font_atlas: Option<SharedFontAtlas>) -> Context {
        let _guard = CTX_MUTEX.lock();

        if !no_current_context() {
            panic!("A Dear ImGui context is already active");
        }

        let shared_font_atlas_ptr = match &mut shared_font_atlas {
            Some(atlas) => atlas.as_ptr_mut(),
            None => ptr::null_mut(),
        };

        // Create the actual ImGui context
        let raw = unsafe { sys::ImGui_CreateContext(shared_font_atlas_ptr) };
        if raw.is_null() {
            panic!("Failed to create Dear ImGui context");
        }

        // Set it as the current context
        unsafe {
            sys::ImGui_SetCurrentContext(raw);
        }

        Context {
            raw,
            shared_font_atlas,
            ini_filename: None,
            log_filename: None,
            platform_name: None,
            renderer_name: None,
            clipboard_ctx: Box::new(ClipboardContext::dummy().into()),
            ui: crate::Ui::new(),
        }
    }

    /// Returns a mutable reference to the active context's IO object
    pub fn io_mut(&mut self) -> &mut Io {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            let io_ptr = sys::ImGui_GetIO();
            &mut *(io_ptr as *mut Io)
        }
    }

    /// Get access to the IO structure
    pub fn io(&self) -> &crate::io::Io {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            let io_ptr = sys::ImGui_GetIO();
            &*(io_ptr as *const crate::io::Io)
        }
    }

    /// Get access to the Style structure
    pub fn style(&self) -> &crate::style::Style {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            let style_ptr = sys::ImGui_GetStyle();
            &*(style_ptr as *const crate::style::Style)
        }
    }

    /// Get mutable access to the Style structure
    pub fn style_mut(&mut self) -> &mut crate::style::Style {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            let style_ptr = sys::ImGui_GetStyle();
            &mut *(style_ptr as *mut crate::style::Style)
        }
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

    /// Renders the frame and returns a reference to the resulting draw data
    pub fn render(&mut self) -> &crate::draw::DrawData {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            sys::ImGui_Render();
            &*(sys::ImGui_GetDrawData() as *const crate::draw::DrawData)
        }
    }

    /// Sets the INI filename for settings persistence  
    pub fn set_ini_filename<P: Into<PathBuf>>(&mut self, filename: Option<P>) {
        let _guard = CTX_MUTEX.lock();

        self.ini_filename = filename.map(|f| {
            CString::new(f.into().to_string_lossy().as_bytes())
                .expect("Invalid filename")
        });

        unsafe {
            let io = sys::ImGui_GetIO();
            let ptr = self.ini_filename
                .as_ref()
                .map(|s| s.as_ptr())
                .unwrap_or(ptr::null());
            (*io).IniFilename = ptr;
        }
    }

    /// Sets the log filename  
    pub fn set_log_filename<P: Into<PathBuf>>(&mut self, filename: Option<P>) {
        let _guard = CTX_MUTEX.lock();

        self.log_filename = filename.map(|f| {
            CString::new(f.into().to_string_lossy().as_bytes())
                .expect("Invalid filename")
        });

        unsafe {
            let io = sys::ImGui_GetIO();
            let ptr = self.log_filename
                .as_ref()
                .map(|s| s.as_ptr())
                .unwrap_or(ptr::null());
            (*io).LogFilename = ptr;
        }
    }

    /// Sets the platform name
    pub fn set_platform_name<S: Into<String>>(&mut self, name: Option<S>) {
        let _guard = CTX_MUTEX.lock();

        self.platform_name = name.map(|n| {
            CString::new(n.into())
                .expect("Invalid platform name")
        });

        unsafe {
            let io = sys::ImGui_GetIO();
            let ptr = self.platform_name
                .as_ref()
                .map(|s| s.as_ptr())
                .unwrap_or(ptr::null());
            (*io).BackendPlatformName = ptr;
        }
    }

    /// Sets the renderer name  
    pub fn set_renderer_name<S: Into<String>>(&mut self, name: Option<S>) {
        let _guard = CTX_MUTEX.lock();

        self.renderer_name = name.map(|n| {
            CString::new(n.into())
                .expect("Invalid renderer name")
        });

        unsafe {
            let io = sys::ImGui_GetIO();
            let ptr = self.renderer_name
                .as_ref()
                .map(|s| s.as_ptr())
                .unwrap_or(ptr::null());
            (*io).BackendRendererName = ptr;
        }
    }

    /// Suspends this context so another context can be the active context
    pub fn suspend(self) -> SuspendedContext {
        let _guard = CTX_MUTEX.lock();
        assert!(
            self.is_current_context(),
            "context to be suspended is not the active context"
        );
        clear_current_context();
        SuspendedContext(self)
    }

    fn is_current_context(&self) -> bool {
        let ctx = unsafe { sys::ImGui_GetCurrentContext() };
        self.raw == ctx
    }

    /// Push a font onto the font stack
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

    /// Attempts to clone the interior shared font atlas **if it exists**.
    pub fn clone_shared_font_atlas(&mut self) -> Option<SharedFontAtlas> {
        self.shared_font_atlas.clone()
    }

    /// Sets the clipboard backend used for clipboard operations
    pub fn set_clipboard_backend<T: ClipboardBackend>(&mut self, backend: T) {
        let clipboard_ctx: Box<UnsafeCell<_>> = Box::new(ClipboardContext::new(backend).into());
        
        // Set the clipboard callbacks in the ImGui IO
        unsafe {
            let io = sys::ImGui_GetIO();
            (*io).SetClipboardTextFn = Some(crate::clipboard::set_clipboard_text);
            (*io).GetClipboardTextFn = Some(crate::clipboard::get_clipboard_text);
            (*io).ClipboardUserData = clipboard_ctx.get() as *mut _;
        }
        
        self.clipboard_ctx = clipboard_ctx;
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

/// A suspended Dear ImGui context
///
/// A suspended context retains its state, but is not usable without activating it first.
#[derive(Debug)]
pub struct SuspendedContext(Context);

impl SuspendedContext {
    /// Creates a new suspended Dear ImGui context
    pub fn create() -> Self {
        Self::create_internal(None)
    }

    /// Creates a new suspended Dear ImGui context with a shared font atlas
    pub fn create_with_shared_font_atlas(shared_font_atlas: SharedFontAtlas) -> Self {
        Self::create_internal(Some(shared_font_atlas))
    }

    fn create_internal(mut shared_font_atlas: Option<SharedFontAtlas>) -> Self {
        let _guard = CTX_MUTEX.lock();

        let shared_font_atlas_ptr = match &mut shared_font_atlas {
            Some(atlas) => atlas.as_ptr_mut(),
            None => ptr::null_mut(),
        };

        let raw = unsafe { sys::ImGui_CreateContext(shared_font_atlas_ptr) };
        if raw.is_null() {
            panic!("Failed to create Dear ImGui context");
        }

        let ctx = Context {
            raw,
            shared_font_atlas,
            ini_filename: None,
            log_filename: None,
            platform_name: None,
            renderer_name: None,
            clipboard_ctx: Box::new(ClipboardContext::dummy().into()),
            ui: crate::Ui::new(),
        };

        // If the context was activated during creation, deactivate it
        if ctx.is_current_context() {
            clear_current_context();
        }

        SuspendedContext(ctx)
    }

    /// Attempts to activate this suspended context
    /// 
    /// If there is no active context, this suspended context is activated and `Ok` is returned.
    /// If there is already an active context, nothing happens and `Err` is returned.
    pub fn activate(self) -> Result<Context, SuspendedContext> {
        let _guard = CTX_MUTEX.lock();
        if no_current_context() {
            unsafe {
                sys::ImGui_SetCurrentContext(self.0.raw);
            }
            Ok(self.0)
        } else {
            Err(self)
        }
    }
}

unsafe impl Send for Context {}
unsafe impl Sync for Context {}

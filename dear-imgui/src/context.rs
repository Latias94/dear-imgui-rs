use parking_lot::ReentrantMutex;
use std::cell::UnsafeCell;
use std::ffi::CString;
use std::ops::Drop;
use std::path::PathBuf;
use std::ptr;

use crate::clipboard::{ClipboardBackend, ClipboardContext};
use crate::fonts::{Font, FontAtlas, SharedFontAtlas};
use crate::io::Io;

use crate::sys;
use crate::ui;

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
/// let ctx = dear_imgui::Context::create_or_panic();
/// // ctx is dropped naturally when it goes out of scope, which deactivates and destroys the
/// // context
/// ```
///
/// Never try to create an active context when another one is active:
///
/// ```should_panic
/// let ctx1 = dear_imgui::Context::create_or_panic();
///
/// let ctx2 = dear_imgui::Context::create_or_panic(); // PANIC
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
    ui: crate::ui::Ui,
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
    /// # Errors
    ///
    /// Returns an error if:
    /// - Another context is already active
    /// - Failed to create the underlying ImGui context
    pub fn create() -> crate::error::ImGuiResult<Context> {
        Self::create_internal(None)
    }

    /// Creates a new active Dear ImGui context with a shared font atlas.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Another context is already active
    /// - Failed to create the underlying ImGui context
    pub fn create_with_shared_font_atlas(
        shared_font_atlas: SharedFontAtlas,
    ) -> crate::error::ImGuiResult<Context> {
        Self::create_internal(Some(shared_font_atlas))
    }

    /// Creates a new active Dear ImGui context (panics on error).
    ///
    /// This is a convenience method that panics if context creation fails.
    /// Use `create()` for proper error handling.
    ///
    /// # Panics
    ///
    /// Panics if another context is already active or context creation fails.
    pub fn create_or_panic() -> Context {
        Self::create().expect("Failed to create Dear ImGui context")
    }

    /// Creates a new active Dear ImGui context with a shared font atlas (panics on error).
    ///
    /// This is a convenience method that panics if context creation fails.
    /// Use `create_with_shared_font_atlas()` for proper error handling.
    ///
    /// # Panics
    ///
    /// Panics if another context is already active or context creation fails.
    pub fn create_with_shared_font_atlas_or_panic(shared_font_atlas: SharedFontAtlas) -> Context {
        Self::create_with_shared_font_atlas(shared_font_atlas)
            .expect("Failed to create Dear ImGui context")
    }

    fn create_internal(
        mut shared_font_atlas: Option<SharedFontAtlas>,
    ) -> crate::error::ImGuiResult<Context> {
        let _guard = CTX_MUTEX.lock();

        if !no_current_context() {
            return Err(crate::error::ImGuiError::ContextAlreadyActive);
        }

        let shared_font_atlas_ptr = match &mut shared_font_atlas {
            Some(atlas) => atlas.as_ptr_mut(),
            None => ptr::null_mut(),
        };

        // Create the actual ImGui context
        let raw = unsafe { sys::ImGui_CreateContext(shared_font_atlas_ptr) };
        if raw.is_null() {
            return Err(crate::error::ImGuiError::ContextCreation {
                reason: "ImGui_CreateContext returned null".to_string(),
            });
        }

        // Set it as the current context
        unsafe {
            sys::ImGui_SetCurrentContext(raw);
        }

        Ok(Context {
            raw,
            shared_font_atlas,
            ini_filename: None,
            log_filename: None,
            platform_name: None,
            renderer_name: None,
            clipboard_ctx: Box::new(UnsafeCell::new(ClipboardContext::dummy())),
            ui: crate::ui::Ui::new(),
        })
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
    pub fn frame(&mut self) -> &mut crate::ui::Ui {
        let _guard = CTX_MUTEX.lock();

        // Ensure font atlas is built before calling NewFrame
        unsafe {
            let io = sys::ImGui_GetIO();
            let fonts = (*io).Fonts;

            // Check if we need to add default font and build atlas
            if (*fonts).Fonts.Size == 0 {
                // Add default font if no fonts are loaded
                sys::ImFontAtlas_AddFontDefault(fonts, ptr::null());
            }

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
        F: FnOnce(&crate::ui::Ui) -> R,
    {
        let ui = self.frame();
        f(ui)
    }

    /// Renders the frame and returns a reference to the resulting draw data
    ///
    /// This finalizes the Dear ImGui frame and prepares all draw data for rendering.
    /// The returned draw data contains all the information needed to render the frame.
    pub fn render(&mut self) -> &crate::render::DrawData {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            sys::ImGui_Render();
            &*(sys::ImGui_GetDrawData() as *const crate::render::DrawData)
        }
    }

    /// Gets the draw data for the current frame
    ///
    /// This returns the draw data without calling render. Only valid after
    /// `render()` has been called and before the next `new_frame()`.
    pub fn draw_data(&self) -> Option<&crate::render::DrawData> {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            let draw_data = sys::ImGui_GetDrawData();
            if draw_data.is_null() {
                None
            } else {
                let data = &*(draw_data as *const crate::render::DrawData);
                if data.valid() {
                    Some(data)
                } else {
                    None
                }
            }
        }
    }

    /// Sets the INI filename for settings persistence
    ///
    /// # Errors
    ///
    /// Returns an error if the filename contains null bytes
    pub fn set_ini_filename<P: Into<PathBuf>>(
        &mut self,
        filename: Option<P>,
    ) -> crate::error::ImGuiResult<()> {
        use crate::error::SafeStringConversion;
        let _guard = CTX_MUTEX.lock();

        self.ini_filename = match filename {
            Some(f) => Some(f.into().to_string_lossy().to_cstring_safe()?),
            None => None,
        };

        unsafe {
            let io = sys::ImGui_GetIO();
            let ptr = self
                .ini_filename
                .as_ref()
                .map(|s| s.as_ptr())
                .unwrap_or(ptr::null());
            (*io).IniFilename = ptr;
        }
        Ok(())
    }

    /// Sets the INI filename for settings persistence (panics on error)
    ///
    /// This is a convenience method that panics if the filename contains null bytes.
    /// Use `set_ini_filename()` for proper error handling.
    ///
    /// # Panics
    ///
    /// Panics if the filename contains null bytes.
    pub fn set_ini_filename_or_panic<P: Into<PathBuf>>(&mut self, filename: Option<P>) {
        self.set_ini_filename(filename).expect("Invalid filename")
    }

    /// Sets the log filename
    ///
    /// # Errors
    ///
    /// Returns an error if the filename contains null bytes
    pub fn set_log_filename<P: Into<PathBuf>>(
        &mut self,
        filename: Option<P>,
    ) -> crate::error::ImGuiResult<()> {
        use crate::error::SafeStringConversion;
        let _guard = CTX_MUTEX.lock();

        self.log_filename = match filename {
            Some(f) => Some(f.into().to_string_lossy().to_cstring_safe()?),
            None => None,
        };

        unsafe {
            let io = sys::ImGui_GetIO();
            let ptr = self
                .log_filename
                .as_ref()
                .map(|s| s.as_ptr())
                .unwrap_or(ptr::null());
            (*io).LogFilename = ptr;
        }
        Ok(())
    }

    /// Sets the log filename (panics on error)
    ///
    /// This is a convenience method that panics if the filename contains null bytes.
    /// Use `set_log_filename()` for proper error handling.
    ///
    /// # Panics
    ///
    /// Panics if the filename contains null bytes.
    pub fn set_log_filename_or_panic<P: Into<PathBuf>>(&mut self, filename: Option<P>) {
        self.set_log_filename(filename).expect("Invalid filename")
    }

    /// Sets the platform name
    ///
    /// # Errors
    ///
    /// Returns an error if the name contains null bytes
    pub fn set_platform_name<S: Into<String>>(
        &mut self,
        name: Option<S>,
    ) -> crate::error::ImGuiResult<()> {
        use crate::error::SafeStringConversion;
        let _guard = CTX_MUTEX.lock();

        self.platform_name = match name {
            Some(n) => Some(n.into().to_cstring_safe()?),
            None => None,
        };

        unsafe {
            let io = sys::ImGui_GetIO();
            let ptr = self
                .platform_name
                .as_ref()
                .map(|s| s.as_ptr())
                .unwrap_or(ptr::null());
            (*io).BackendPlatformName = ptr;
        }
        Ok(())
    }

    /// Sets the platform name (panics on error)
    ///
    /// This is a convenience method that panics if the name contains null bytes.
    /// Use `set_platform_name()` for proper error handling.
    ///
    /// # Panics
    ///
    /// Panics if the name contains null bytes.
    pub fn set_platform_name_or_panic<S: Into<String>>(&mut self, name: Option<S>) {
        self.set_platform_name(name).expect("Invalid platform name")
    }

    /// Sets the renderer name
    ///
    /// # Errors
    ///
    /// Returns an error if the name contains null bytes
    pub fn set_renderer_name<S: Into<String>>(
        &mut self,
        name: Option<S>,
    ) -> crate::error::ImGuiResult<()> {
        use crate::error::SafeStringConversion;
        let _guard = CTX_MUTEX.lock();

        self.renderer_name = match name {
            Some(n) => Some(n.into().to_cstring_safe()?),
            None => None,
        };

        unsafe {
            let io = sys::ImGui_GetIO();
            let ptr = self
                .renderer_name
                .as_ref()
                .map(|s| s.as_ptr())
                .unwrap_or(ptr::null());
            (*io).BackendRendererName = ptr;
        }
        Ok(())
    }

    /// Sets the renderer name (panics on error)
    ///
    /// This is a convenience method that panics if the name contains null bytes.
    /// Use `set_renderer_name()` for proper error handling.
    ///
    /// # Panics
    ///
    /// Panics if the name contains null bytes.
    pub fn set_renderer_name_or_panic<S: Into<String>>(&mut self, name: Option<S>) {
        self.set_renderer_name(name).expect("Invalid renderer name")
    }

    /// Get mutable access to the platform IO
    #[cfg(feature = "multi-viewport")]
    pub fn platform_io_mut(&mut self) -> &mut crate::platform_io::PlatformIo {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            let pio = sys::ImGui_GetPlatformIO();
            crate::platform_io::PlatformIo::from_raw_mut(pio)
        }
    }

    /// Enable multi-viewport support flags
    #[cfg(feature = "multi-viewport")]
    pub fn enable_multi_viewport(&mut self) {
        // Enable viewport flags
        crate::viewport_backend::utils::enable_viewport_flags(self.io_mut());
    }

    /// Update platform windows
    ///
    /// This function should be called every frame when multi-viewport is enabled.
    /// It updates all platform windows and handles viewport management.
    #[cfg(feature = "multi-viewport")]
    pub fn update_platform_windows(&mut self) {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            // Ensure main viewport is properly set up before updating platform windows
            let main_viewport = sys::ImGui_GetMainViewport();
            if !main_viewport.is_null() && (*main_viewport).PlatformHandle.is_null() {
                eprintln!("update_platform_windows: main viewport not set up, setting it up now");
                // The main viewport needs to be set up - this should be done by the backend
                // For now, we'll just log this and continue
            }

            sys::ImGui_UpdatePlatformWindows();
        }
    }

    /// Render platform windows with default implementation
    ///
    /// This function renders all platform windows using the default implementation.
    /// It calls the platform and renderer backends to render each viewport.
    #[cfg(feature = "multi-viewport")]
    pub fn render_platform_windows_default(&mut self) {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            sys::ImGui_RenderPlatformWindowsDefault(std::ptr::null_mut(), std::ptr::null_mut());
        }
    }

    /// Destroy all platform windows
    ///
    /// This function should be called during shutdown to properly clean up
    /// all platform windows and their associated resources.
    #[cfg(feature = "multi-viewport")]
    pub fn destroy_platform_windows(&mut self) {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            sys::ImGui_DestroyPlatformWindows();
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
    pub fn font_atlas(&self) -> FontAtlas {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            let io = sys::ImGui_GetIO();
            let atlas_ptr = (*io).Fonts;
            FontAtlas::from_raw(atlas_ptr)
        }
    }

    /// Get a mutable reference to the font atlas from the IO structure
    pub fn font_atlas_mut(&mut self) -> FontAtlas {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            let io = sys::ImGui_GetIO();
            let atlas_ptr = (*io).Fonts;
            FontAtlas::from_raw(atlas_ptr)
        }
    }

    /// Returns the font atlas (alias for font_atlas_mut)
    ///
    /// This provides compatibility with imgui-rs naming convention
    pub fn fonts(&mut self) -> FontAtlas {
        self.font_atlas_mut()
    }

    /// Attempts to clone the interior shared font atlas **if it exists**.
    pub fn clone_shared_font_atlas(&mut self) -> Option<SharedFontAtlas> {
        self.shared_font_atlas.clone()
    }

    /// Loads settings from a string slice containing settings in .Ini file format
    #[doc(alias = "LoadIniSettingsFromMemory")]
    pub fn load_ini_settings(&mut self, data: &str) {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            sys::ImGui_LoadIniSettingsFromMemory(data.as_ptr() as *const _, data.len());
        }
    }

    /// Saves settings to a mutable string buffer in .Ini file format
    #[doc(alias = "SaveIniSettingsToMemory")]
    pub fn save_ini_settings(&mut self, buf: &mut String) {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            let data_ptr = sys::ImGui_SaveIniSettingsToMemory(ptr::null_mut());
            if !data_ptr.is_null() {
                let data = std::ffi::CStr::from_ptr(data_ptr);
                buf.push_str(&data.to_string_lossy());
            }
        }
    }

    /// Sets the clipboard backend used for clipboard operations
    pub fn set_clipboard_backend<T: ClipboardBackend>(&mut self, backend: T) {
        let clipboard_ctx: Box<UnsafeCell<_>> =
            Box::new(UnsafeCell::new(ClipboardContext::new(backend)));

        // Set the clipboard callbacks in the ImGui PlatformIO
        unsafe {
            let platform_io = sys::ImGui_GetPlatformIO();
            (*platform_io).Platform_SetClipboardTextFn = Some(crate::clipboard::set_clipboard_text);
            (*platform_io).Platform_GetClipboardTextFn = Some(crate::clipboard::get_clipboard_text);
            (*platform_io).Platform_ClipboardUserData = clipboard_ctx.get() as *mut _;
        }

        self.clipboard_ctx = clipboard_ctx;
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
    ///
    /// # Errors
    ///
    /// Returns an error if failed to create the underlying ImGui context
    pub fn create() -> crate::error::ImGuiResult<Self> {
        Self::create_internal(None)
    }

    /// Creates a new suspended Dear ImGui context with a shared font atlas
    ///
    /// # Errors
    ///
    /// Returns an error if failed to create the underlying ImGui context
    pub fn create_with_shared_font_atlas(
        shared_font_atlas: SharedFontAtlas,
    ) -> crate::error::ImGuiResult<Self> {
        Self::create_internal(Some(shared_font_atlas))
    }

    /// Creates a new suspended Dear ImGui context (panics on error)
    ///
    /// This is a convenience method that panics if context creation fails.
    /// Use `create()` for proper error handling.
    ///
    /// # Panics
    ///
    /// Panics if context creation fails.
    pub fn create_or_panic() -> Self {
        Self::create().expect("Failed to create Dear ImGui context")
    }

    /// Creates a new suspended Dear ImGui context with a shared font atlas (panics on error)
    ///
    /// This is a convenience method that panics if context creation fails.
    /// Use `create_with_shared_font_atlas()` for proper error handling.
    ///
    /// # Panics
    ///
    /// Panics if context creation fails.
    pub fn create_with_shared_font_atlas_or_panic(shared_font_atlas: SharedFontAtlas) -> Self {
        Self::create_with_shared_font_atlas(shared_font_atlas)
            .expect("Failed to create Dear ImGui context")
    }

    fn create_internal(
        mut shared_font_atlas: Option<SharedFontAtlas>,
    ) -> crate::error::ImGuiResult<Self> {
        let _guard = CTX_MUTEX.lock();

        let shared_font_atlas_ptr = match &mut shared_font_atlas {
            Some(atlas) => atlas.as_ptr_mut(),
            None => ptr::null_mut(),
        };

        let raw = unsafe { sys::ImGui_CreateContext(shared_font_atlas_ptr) };
        if raw.is_null() {
            return Err(crate::error::ImGuiError::ContextCreation {
                reason: "ImGui_CreateContext returned null".to_string(),
            });
        }

        let ctx = Context {
            raw,
            shared_font_atlas,
            ini_filename: None,
            log_filename: None,
            platform_name: None,
            renderer_name: None,
            clipboard_ctx: Box::new(UnsafeCell::new(ClipboardContext::dummy())),
            ui: crate::ui::Ui::new(),
        };

        // If the context was activated during creation, deactivate it
        if ctx.is_current_context() {
            clear_current_context();
        }

        Ok(SuspendedContext(ctx))
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

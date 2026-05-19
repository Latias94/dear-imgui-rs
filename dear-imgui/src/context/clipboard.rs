use crate::clipboard::{ClipboardBackend, ClipboardContext};
use crate::sys;

use super::Context;
use super::binding::{CTX_MUTEX, with_bound_context};

impl Context {
    /// Returns the current clipboard text, if available.
    ///
    /// This calls Dear ImGui's clipboard callbacks (configured via
    /// [`Context::set_clipboard_backend`]). When no backend is installed, this returns `None`.
    ///
    /// Note: returned data is copied into a new `String`.
    #[doc(alias = "GetClipboardText")]
    pub fn clipboard_text(&self) -> Option<String> {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            with_bound_context(self.raw, || {
                let ptr = sys::igGetClipboardText();
                if ptr.is_null() {
                    return None;
                }
                Some(std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned())
            })
        }
    }

    /// Sets the clipboard text.
    ///
    /// This calls Dear ImGui's clipboard callbacks (configured via
    /// [`Context::set_clipboard_backend`]). If no backend is installed, this is a no-op.
    ///
    /// Interior NUL bytes are sanitized to `?` to match other scratch-string helpers.
    #[doc(alias = "SetClipboardText")]
    pub fn set_clipboard_text(&self, text: impl AsRef<str>) {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            with_bound_context(self.raw, || {
                sys::igSetClipboardText(self.ui.scratch_txt(text.as_ref()));
            });
        }
    }

    /// Sets the clipboard backend used for clipboard operations
    pub fn set_clipboard_backend<T: ClipboardBackend>(&mut self, backend: T) {
        let _guard = CTX_MUTEX.lock();

        let clipboard_ctx = Box::new(ClipboardContext::new(backend));

        // On native/desktop targets, register clipboard callbacks in ImGui PlatformIO
        // so ImGui can call back into Rust for copy/paste.
        //
        // On wasm32 (import-style build), function pointers cannot safely cross the
        // module boundary between the Rust main module and the cimgui provider. We
        // therefore keep the backend alive on the Rust side but do not hook it into
        // ImGui's PlatformIO yet; clipboard integration for web will need a dedicated
        // design using JS bindings.
        #[cfg(not(target_arch = "wasm32"))]
        unsafe {
            let platform_io = sys::igGetPlatformIO_ContextPtr(self.raw);
            if platform_io.is_null() {
                panic!("Context::set_clipboard_backend() requires a valid ImGui context");
            }
            (*platform_io).Platform_SetClipboardTextFn = Some(crate::clipboard::set_clipboard_text);
            (*platform_io).Platform_GetClipboardTextFn = Some(crate::clipboard::get_clipboard_text);
            (*platform_io).Platform_ClipboardUserData =
                clipboard_ctx.as_ref() as *const ClipboardContext as *mut _;
        }

        self.clipboard_ctx = clipboard_ctx;
    }
}

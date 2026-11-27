use crate::clipboard::{ClipboardBackend, ClipboardContext};
use crate::ffi::RawBox;
use crate::sys;

use super::Context;
use super::binding::{CTX_MUTEX, with_bound_context};

impl Context {
    /// Returns the current clipboard text, if available.
    ///
    /// Copies the text returned by the current platform clipboard callback. This can be
    /// Dear ImGui's default implementation, the platform backend, or a backend installed via
    /// [`Context::set_clipboard_backend`]. Returns `None` if the callback or text is unavailable.
    #[doc(alias = "GetClipboardText")]
    pub fn clipboard_text(&self) -> Option<String> {
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
    /// Calls the current platform clipboard setter, including Dear ImGui's default or a
    /// replacement installed via [`Context::set_clipboard_backend`].
    ///
    /// Interior NUL bytes are sanitized to `?` to match other scratch-string helpers.
    #[doc(alias = "SetClipboardText")]
    pub fn set_clipboard_text(&self, text: impl AsRef<str>) {
        let text = text.as_ref();
        with_bound_context(self.raw, || {
            self.ui
                .with_scratch_c_string(text, |text| unsafe { sys::igSetClipboardText(text) });
        });
    }

    /// Sets the clipboard backend used for native clipboard operations.
    ///
    /// On WebAssembly the context retains the backend without installing its callbacks.
    pub fn set_clipboard_backend<T: ClipboardBackend>(&mut self, backend: T) {
        let _guard = CTX_MUTEX.lock();

        let clipboard_ctx = RawBox::new(ClipboardContext::new(backend));

        #[cfg(not(target_arch = "wasm32"))]
        unsafe {
            let platform_io = sys::igGetPlatformIO_ContextPtr(self.raw);
            if platform_io.is_null() {
                panic!("Context::set_clipboard_backend() requires a valid ImGui context");
            }
            (*platform_io).Platform_SetClipboardTextFn = Some(crate::clipboard::set_clipboard_text);
            (*platform_io).Platform_GetClipboardTextFn = Some(crate::clipboard::get_clipboard_text);
            (*platform_io).Platform_ClipboardUserData = clipboard_ctx.as_ptr().cast();
        }

        let previous = std::mem::replace(&mut self.clipboard_ctx, clipboard_ctx);
        drop(previous);
    }
}

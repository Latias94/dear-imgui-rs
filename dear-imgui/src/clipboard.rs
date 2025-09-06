use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use std::cell::UnsafeCell;

/// Trait for clipboard backends
pub trait ClipboardBackend: 'static {
    /// Returns the current clipboard contents as an owned string, or None if the
    /// clipboard is empty or inaccessible
    fn get(&mut self) -> Option<String>;
    /// Sets the clipboard contents to the given string slice.
    fn set(&mut self, value: &str);
}

pub(crate) struct ClipboardContext {
    backend: Box<dyn ClipboardBackend>,
    // This is needed to keep ownership of the value when the raw C callback is called
    last_value: CString,
}

impl ClipboardContext {
    /// Creates a new ClipboardContext
    pub fn new<T: ClipboardBackend>(backend: T) -> ClipboardContext {
        ClipboardContext {
            backend: Box::new(backend),
            last_value: CString::default(),
        }
    }

    /// Creates a dummy clipboard context that doesn't actually interact with the system clipboard
    pub fn dummy() -> ClipboardContext {
        Self {
            backend: Box::new(DummyClipboardBackend),
            last_value: CString::default(),
        }
    }
}

/// Non-functioning placeholder clipboard backend
pub struct DummyClipboardBackend;

impl ClipboardBackend for DummyClipboardBackend {
    fn get(&mut self) -> Option<String> {
        None
    }

    fn set(&mut self, _: &str) {
        // Do nothing
    }
}

/// C callback functions for Dear ImGui clipboard integration
pub(crate) unsafe extern "C" fn get_clipboard_text(user_data: *mut std::os::raw::c_void) -> *const c_char {
    let clipboard_ctx = &mut *(user_data as *mut ClipboardContext);
    
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        clipboard_ctx.backend.get()
    })) {
        Ok(Some(value)) => {
            // Convert to CString and store it
            match CString::new(value) {
                Ok(cstring) => {
                    clipboard_ctx.last_value = cstring;
                    clipboard_ctx.last_value.as_ptr()
                }
                Err(_) => ptr::null(),
            }
        }
        Ok(None) => ptr::null(),
        Err(_) => {
            // Panic occurred, return null
            ptr::null()
        }
    }
}

pub(crate) unsafe extern "C" fn set_clipboard_text(user_data: *mut std::os::raw::c_void, text: *const c_char) {
    if text.is_null() {
        return;
    }

    let clipboard_ctx = &mut *(user_data as *mut ClipboardContext);
    
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let text_str = CStr::from_ptr(text);
        if let Ok(text_str) = text_str.to_str() {
            clipboard_ctx.backend.set(text_str);
        }
    }));
}

impl From<ClipboardContext> for UnsafeCell<ClipboardContext> {
    fn from(ctx: ClipboardContext) -> Self {
        UnsafeCell::new(ctx)
    }
}
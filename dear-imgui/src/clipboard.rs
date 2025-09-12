use std::ffi::{CStr, CString};
use std::fmt;
use std::os::raw::c_char;
use std::ptr;

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
pub(crate) unsafe extern "C" fn get_clipboard_text(
    _user_data: *mut crate::sys::ImGuiContext,
) -> *const c_char {
    let result = std::panic::catch_unwind(|| {
        let user_data = unsafe { (*crate::sys::ImGui_GetPlatformIO()).Platform_ClipboardUserData };

        let ctx = &mut *(user_data as *mut ClipboardContext);
        match ctx.backend.get() {
            Some(text) => {
                ctx.last_value = CString::new(text).unwrap();
                ctx.last_value.as_ptr()
            }
            None => ptr::null(),
        }
    });
    result.unwrap_or_else(|_| {
        eprintln!("Clipboard getter panicked");
        std::process::abort();
    })
}

pub(crate) unsafe extern "C" fn set_clipboard_text(
    _user_data: *mut crate::sys::ImGuiContext,
    text: *const c_char,
) {
    let result = std::panic::catch_unwind(|| {
        let user_data = unsafe { (*crate::sys::ImGui_GetPlatformIO()).Platform_ClipboardUserData };

        let ctx = &mut *(user_data as *mut ClipboardContext);
        let text = CStr::from_ptr(text).to_owned();
        ctx.backend.set(text.to_str().unwrap());
    });
    result.unwrap_or_else(|_| {
        eprintln!("Clipboard setter panicked");
        std::process::abort();
    });
}

impl fmt::Debug for ClipboardContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClipboardContext")
            .field("backend", &(&(*self.backend) as *const _))
            .field("last_value", &self.last_value)
            .finish()
    }
}

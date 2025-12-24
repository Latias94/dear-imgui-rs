//! Clipboard integration
//!
//! Provides the `ClipboardBackend` trait and a small glue layer used by the
//! crate to hook Dear ImGui's clipboard callbacks. You can implement your own
//! backend and pass it to the context so copy/paste works in input widgets.
//!
use std::ffi::{CStr, CString};
use std::fmt;
use std::os::raw::c_char;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

static CLIPBOARD_BORROWED: AtomicBool = AtomicBool::new(false);

struct ClipboardBorrowGuard;

impl ClipboardBorrowGuard {
    fn try_new() -> Option<Self> {
        if CLIPBOARD_BORROWED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return None;
        }
        Some(Self)
    }
}

impl Drop for ClipboardBorrowGuard {
    fn drop(&mut self) {
        CLIPBOARD_BORROWED.store(false, Ordering::SeqCst);
    }
}

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
        let platform_io = unsafe { crate::sys::igGetPlatformIO_Nil() };
        if platform_io.is_null() {
            return ptr::null();
        }
        let user_data = unsafe { (*platform_io).Platform_ClipboardUserData };
        if user_data.is_null() {
            return ptr::null();
        }
        let Some(_borrow) = ClipboardBorrowGuard::try_new() else {
            return ptr::null();
        };

        let ctx = unsafe { &mut *(user_data as *mut ClipboardContext) };
        match ctx.backend.get() {
            Some(text) => {
                ctx.last_value = match CString::new(text) {
                    Ok(v) => v,
                    Err(e) => {
                        let mut bytes = e.into_vec();
                        for b in &mut bytes {
                            if *b == 0 {
                                *b = b'?';
                            }
                        }
                        CString::new(bytes).expect("sanitized clipboard text contained null byte")
                    }
                };
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
        let platform_io = unsafe { crate::sys::igGetPlatformIO_Nil() };
        if platform_io.is_null() {
            return;
        }
        let user_data = unsafe { (*platform_io).Platform_ClipboardUserData };
        if user_data.is_null() {
            return;
        }
        let Some(_borrow) = ClipboardBorrowGuard::try_new() else {
            return;
        };

        let ctx = unsafe { &mut *(user_data as *mut ClipboardContext) };
        if text.is_null() {
            ctx.backend.set("");
            return;
        }
        let text = unsafe { CStr::from_ptr(text) }.to_string_lossy();
        ctx.backend.set(text.as_ref());
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

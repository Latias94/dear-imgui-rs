//! Clipboard integration
//!
//! Provides the `ClipboardBackend` trait and a small glue layer used by the
//! crate to hook Dear ImGui's clipboard callbacks. You can implement your own
//! backend and pass it to the context so copy/paste works in input widgets.
//!
use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::fmt;
use std::os::raw::c_char;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

struct ClipboardBorrowGuard<'a> {
    borrowed: &'a AtomicBool,
}

impl<'a> ClipboardBorrowGuard<'a> {
    fn try_new(borrowed: &'a AtomicBool) -> Option<Self> {
        if borrowed
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return None;
        }
        Some(Self { borrowed })
    }
}

impl Drop for ClipboardBorrowGuard<'_> {
    fn drop(&mut self) {
        self.borrowed.store(false, Ordering::SeqCst);
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
    borrowed: AtomicBool,
    backend: RefCell<Box<dyn ClipboardBackend>>,
    // This is needed to keep ownership of the value when the raw C callback is called
    last_value: RefCell<CString>,
}

impl ClipboardContext {
    /// Creates a new ClipboardContext
    pub fn new<T: ClipboardBackend>(backend: T) -> ClipboardContext {
        ClipboardContext {
            borrowed: AtomicBool::new(false),
            backend: RefCell::new(Box::new(backend)),
            last_value: RefCell::new(CString::default()),
        }
    }

    /// Creates a dummy clipboard context that doesn't actually interact with the system clipboard
    pub fn dummy() -> ClipboardContext {
        Self {
            borrowed: AtomicBool::new(false),
            backend: RefCell::new(Box::new(DummyClipboardBackend)),
            last_value: RefCell::new(CString::default()),
        }
    }

    fn try_borrow(&self) -> Option<ClipboardBorrowGuard<'_>> {
        ClipboardBorrowGuard::try_new(&self.borrowed)
    }

    fn get(&self, _borrow: &ClipboardBorrowGuard<'_>) -> Option<String> {
        self.backend.borrow_mut().get()
    }

    fn set(&self, _borrow: &ClipboardBorrowGuard<'_>, value: &str) {
        self.backend.borrow_mut().set(value);
    }

    fn store_last_value(
        &self,
        _borrow: &ClipboardBorrowGuard<'_>,
        value: CString,
    ) -> *const c_char {
        let mut last_value = self.last_value.borrow_mut();
        *last_value = value;
        last_value.as_ptr()
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
unsafe fn platform_io_for_context(
    ctx: *mut crate::sys::ImGuiContext,
) -> *mut crate::sys::ImGuiPlatformIO {
    if ctx.is_null() {
        unsafe { crate::sys::igGetPlatformIO_Nil() }
    } else {
        unsafe { crate::sys::igGetPlatformIO_ContextPtr(ctx) }
    }
}

pub(crate) unsafe extern "C" fn get_clipboard_text(
    ctx: *mut crate::sys::ImGuiContext,
) -> *const c_char {
    let result = std::panic::catch_unwind(|| {
        let platform_io = unsafe { platform_io_for_context(ctx) };
        if platform_io.is_null() {
            return ptr::null();
        }
        let user_data = unsafe { (*platform_io).Platform_ClipboardUserData };
        if user_data.is_null() {
            return ptr::null();
        }
        let ctx = unsafe { &*(user_data as *const ClipboardContext) };
        let Some(borrow) = ctx.try_borrow() else {
            return ptr::null();
        };

        match ctx.get(&borrow) {
            Some(text) => {
                let last_value = match CString::new(text) {
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
                ctx.store_last_value(&borrow, last_value)
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
    ctx: *mut crate::sys::ImGuiContext,
    text: *const c_char,
) {
    let result = std::panic::catch_unwind(|| {
        let platform_io = unsafe { platform_io_for_context(ctx) };
        if platform_io.is_null() {
            return;
        }
        let user_data = unsafe { (*platform_io).Platform_ClipboardUserData };
        if user_data.is_null() {
            return;
        }
        let ctx = unsafe { &*(user_data as *const ClipboardContext) };
        let Some(borrow) = ctx.try_borrow() else {
            return;
        };

        if text.is_null() {
            ctx.set(&borrow, "");
            return;
        }
        let text = unsafe { CStr::from_ptr(text) }.to_string_lossy();
        ctx.set(&borrow, text.as_ref());
    });
    result.unwrap_or_else(|_| {
        eprintln!("Clipboard setter panicked");
        std::process::abort();
    });
}

impl fmt::Debug for ClipboardContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClipboardContext")
            .field("backend", &(&**self.backend.borrow() as *const _))
            .field("last_value", &self.last_value.borrow())
            .finish()
    }
}

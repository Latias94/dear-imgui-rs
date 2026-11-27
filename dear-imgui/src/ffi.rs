//! Owned allocations and callback boundaries for native code.

use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr::NonNull;

/// Run a native callback body and report a panic without unwinding through the C ABI.
///
/// Put callback setup and dispatch in `callback`. The fallback is returned even if panic
/// reporting or dropping a custom panic payload panics. This does not catch aborting panics.
#[doc(hidden)]
pub fn guard_native_callback<R>(
    fallback: R,
    callback: impl FnOnce() -> R,
    report_panic: impl FnOnce(),
) -> R {
    match catch_unwind(AssertUnwindSafe(callback)) {
        Ok(value) => value,
        Err(payload) => {
            discard_panic_payload(payload);
            if let Err(payload) = catch_unwind(AssertUnwindSafe(report_panic)) {
                discard_panic_payload(payload);
            }
            fallback
        }
    }
}

fn discard_panic_payload(payload: Box<dyn std::any::Any + Send>) {
    if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
        std::mem::forget(payload);
    }
}

/// Owns a boxed allocation through a raw pointer.
///
/// Moving this owner does not create a new Rust reference to the allocation. A pointer
/// registered with native code remains valid when the owner moves. Unregister the pointer and
/// finish all native accesses before dropping the owner.
///
/// ```compile_fail,E0599
/// let owner = dear_imgui_rs::ffi::RawBox::new(7u32);
/// let shared = owner.as_ref();
/// ```
#[derive(Debug)]
pub struct RawBox<T: ?Sized> {
    ptr: NonNull<T>,
}

impl<T> RawBox<T> {
    /// Allocates `value` on the heap.
    pub fn new(value: T) -> Self {
        Box::new(value).into()
    }
}

impl<T: ?Sized> RawBox<T> {
    /// Returns the pointer to the owned allocation.
    pub fn as_ptr(&self) -> *mut T {
        self.ptr.as_ptr()
    }

    /// Borrows the allocation through a shared reference.
    ///
    /// # Safety
    ///
    /// During the borrow, neither Rust nor native code may mutate the allocation other than
    /// through its interior-mutable fields, or hold an overlapping exclusive borrow.
    pub unsafe fn as_ref_unchecked(&self) -> &T {
        unsafe { self.ptr.as_ref() }
    }

    /// Borrows the allocation exclusively.
    ///
    /// # Safety
    ///
    /// The caller must prevent access through previously published pointers and other borrows
    /// for the lifetime of the returned reference.
    pub unsafe fn as_mut_unchecked(&mut self) -> &mut T {
        unsafe { self.ptr.as_mut() }
    }
}

impl<T: ?Sized> From<Box<T>> for RawBox<T> {
    fn from(value: Box<T>) -> Self {
        Self {
            ptr: NonNull::new(Box::into_raw(value)).expect("Box owns a non-null pointer"),
        }
    }
}

impl<T: ?Sized> Drop for RawBox<T> {
    fn drop(&mut self) {
        // SAFETY: this owner exclusively reclaims the original Box allocation.
        unsafe { drop(Box::from_raw(self.ptr.as_ptr())) };
    }
}

/// Owns an immutable NUL-terminated string published to native code.
///
/// The buffer stays at the same address when this owner moves. Native code must only read the
/// buffer and stop using its pointer before this owner drops.
#[derive(Debug)]
pub struct RawCString {
    ptr: NonNull<c_char>,
}

impl RawCString {
    /// Takes ownership of a C string without copying its buffer.
    pub fn new(value: CString) -> Self {
        Self {
            ptr: NonNull::new(value.into_raw()).expect("CString owns a non-null pointer"),
        }
    }

    /// Returns the native read-only C string pointer.
    pub fn as_ptr(&self) -> *const c_char {
        self.ptr.as_ptr()
    }

    /// Borrows the owned C string.
    pub fn as_c_str(&self) -> &CStr {
        // SAFETY: CString supplied the terminator and native recipients only read the buffer.
        unsafe { CStr::from_ptr(self.as_ptr()) }
    }
}

impl Drop for RawCString {
    fn drop(&mut self) {
        // SAFETY: this owner exclusively reclaims the original, unmodified CString allocation.
        unsafe { drop(CString::from_raw(self.ptr.as_ptr())) };
    }
}

#[cfg(test)]
mod tests {
    use super::guard_native_callback;
    use std::cell::Cell;
    use std::panic::panic_any;

    struct PanickingPayload;

    impl Drop for PanickingPayload {
        fn drop(&mut self) {
            panic!("drop of callback panic payload");
        }
    }

    #[test]
    fn native_callback_contains_panic_from_body_payload_and_fault_reporter() {
        let reported = Cell::new(false);
        let value = guard_native_callback(
            42,
            || panic_any(PanickingPayload),
            || {
                reported.set(true);
                panic_any(PanickingPayload);
            },
        );
        assert_eq!(value, 42);
        assert!(reported.get());
    }
}

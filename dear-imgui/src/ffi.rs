//! Owned allocations that can be published to native code.

use std::ffi::{CStr, CString, c_char};
use std::ptr::NonNull;

/// Owns a boxed allocation through a raw pointer.
///
/// Moving this owner does not create a new Rust reference to the allocation. A pointer
/// registered with native code remains valid when the owner moves. Unregister the pointer and
/// finish all native accesses before dropping the owner.
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

impl<T: ?Sized> AsRef<T> for RawBox<T> {
    fn as_ref(&self) -> &T {
        // SAFETY: the allocation lives for this borrow. Native code must respect shared access.
        unsafe { self.ptr.as_ref() }
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

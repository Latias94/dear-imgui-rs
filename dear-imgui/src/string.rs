//! String helpers (ImString and scratch buffers)
//!
//! Utilities for working with strings across the Rust <-> Dear ImGui FFI
//! boundary.
//!
//! - `ImString`: an owned, growable UTF-8 string that maintains a trailing
//!   NUL byte as required by C APIs. Useful for zero-copy text editing via
//!   ImGui callbacks.
//! - `UiBuffer`: an internal scratch buffer used by [`Ui`] methods to stage
//!   temporary C strings for widget labels and hints.
//!
//! Example (zero-copy text input with `ImString`):
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! let mut s = ImString::with_capacity(256);
//! if ui.input_text_imstr("Edit", &mut s).build() {
//!     // edited in-place, no extra copies
//! }
//! ```
//!
use std::borrow::Cow;
use std::cell::RefCell;
use std::fmt;
use std::ops::{Deref, Index, RangeFull};
use std::os::raw::c_char;
use std::str;

/// Internal buffer for UI string operations
#[derive(Debug)]
pub struct UiBuffer {
    pub buffer: Vec<u8>,
    pub max_len: usize,
}

impl UiBuffer {
    /// Creates a new buffer with the specified capacity
    pub const fn new(max_len: usize) -> Self {
        Self {
            buffer: Vec::new(),
            max_len,
        }
    }

    /// Internal method to push a single text to our scratch buffer.
    pub fn scratch_txt(&mut self, txt: impl AsRef<str>) -> *const std::os::raw::c_char {
        self.refresh_buffer();

        let start_of_substr = self.push(txt);
        unsafe { self.offset(start_of_substr) }
    }

    /// Internal method to push an option text to our scratch buffer.
    pub fn scratch_txt_opt(&mut self, txt: Option<impl AsRef<str>>) -> *const std::os::raw::c_char {
        match txt {
            Some(v) => self.scratch_txt(v),
            None => std::ptr::null(),
        }
    }

    /// Helper method, same as [`Self::scratch_txt`] but for two strings
    pub fn scratch_txt_two(
        &mut self,
        txt_0: impl AsRef<str>,
        txt_1: impl AsRef<str>,
    ) -> (*const std::os::raw::c_char, *const std::os::raw::c_char) {
        self.refresh_buffer();

        let first_offset = self.push(txt_0);
        let second_offset = self.push(txt_1);

        unsafe { (self.offset(first_offset), self.offset(second_offset)) }
    }

    /// Helper method, same as [`Self::scratch_txt`] but with one optional value
    pub fn scratch_txt_with_opt(
        &mut self,
        txt_0: impl AsRef<str>,
        txt_1: Option<impl AsRef<str>>,
    ) -> (*const std::os::raw::c_char, *const std::os::raw::c_char) {
        match txt_1 {
            Some(value) => self.scratch_txt_two(txt_0, value),
            None => (self.scratch_txt(txt_0), std::ptr::null()),
        }
    }

    /// Attempts to clear the buffer if it's over the maximum length allowed.
    /// This is to prevent us from making a giant vec over time.
    pub fn refresh_buffer(&mut self) {
        if self.buffer.len() > self.max_len {
            self.buffer.clear();
        }
    }

    /// Given a position, gives an offset from the start of the scratch buffer.
    ///
    /// # Safety
    /// This can return a pointer to undefined data if given a `pos >= self.buffer.len()`.
    /// This is marked as unsafe to reflect that.
    pub unsafe fn offset(&self, pos: usize) -> *const std::os::raw::c_char {
        unsafe { self.buffer.as_ptr().add(pos) as *const _ }
    }

    /// Pushes a new scratch sheet text and return the byte index where the sub-string
    /// starts.
    pub fn push(&mut self, txt: impl AsRef<str>) -> usize {
        assert!(!txt.as_ref().contains('\0'), "string contained null byte");
        let len = self.buffer.len();
        self.buffer.extend(txt.as_ref().as_bytes());
        self.buffer.push(b'\0');

        len
    }
}

thread_local! {
    static TLS_SCRATCH: RefCell<UiBuffer> = RefCell::new(UiBuffer::new(1024));
}

/// Creates a temporary, NUL-terminated C string pointer backed by a thread-local scratch buffer.
///
/// The returned pointer is only valid until the next call on the same thread.
pub(crate) fn tls_scratch_txt(txt: impl AsRef<str>) -> *const c_char {
    TLS_SCRATCH.with(|buf| buf.borrow_mut().scratch_txt(txt))
}

/// Calls `f` with a temporary, NUL-terminated C string pointer backed by a thread-local scratch buffer.
///
/// The pointer is only valid for the duration of the call (and will be overwritten by subsequent
/// scratch-string operations on the same thread).
pub fn with_scratch_txt<R>(txt: impl AsRef<str>, f: impl FnOnce(*const c_char) -> R) -> R {
    TLS_SCRATCH.with(|buf| {
        let mut buf = buf.borrow_mut();
        let ptr = buf.scratch_txt(txt);
        f(ptr)
    })
}

/// Same as [`tls_scratch_txt`] but returns two pointers that stay valid together.
pub(crate) fn tls_scratch_txt_two(
    txt_0: impl AsRef<str>,
    txt_1: impl AsRef<str>,
) -> (*const c_char, *const c_char) {
    TLS_SCRATCH.with(|buf| buf.borrow_mut().scratch_txt_two(txt_0, txt_1))
}

/// Calls `f` with two temporary, NUL-terminated C string pointers backed by a thread-local scratch buffer.
///
/// Both pointers are valid together for the duration of the call (and will be overwritten by
/// subsequent scratch-string operations on the same thread).
pub fn with_scratch_txt_two<R>(
    txt_0: impl AsRef<str>,
    txt_1: impl AsRef<str>,
    f: impl FnOnce(*const c_char, *const c_char) -> R,
) -> R {
    TLS_SCRATCH.with(|buf| {
        let mut buf = buf.borrow_mut();
        let (a, b) = buf.scratch_txt_two(txt_0, txt_1);
        f(a, b)
    })
}

/// Calls `f` with three temporary, NUL-terminated C string pointers backed by a thread-local scratch buffer.
///
/// All pointers are valid together for the duration of the call (and will be overwritten by
/// subsequent scratch-string operations on the same thread).
pub fn with_scratch_txt_three<R>(
    txt_0: impl AsRef<str>,
    txt_1: impl AsRef<str>,
    txt_2: impl AsRef<str>,
    f: impl FnOnce(*const c_char, *const c_char, *const c_char) -> R,
) -> R {
    TLS_SCRATCH.with(|buf| {
        let mut buf = buf.borrow_mut();
        buf.refresh_buffer();
        let o0 = buf.push(txt_0);
        let o1 = buf.push(txt_1);
        let o2 = buf.push(txt_2);
        unsafe { f(buf.offset(o0), buf.offset(o1), buf.offset(o2)) }
    })
}

/// A UTF-8 encoded, growable, implicitly nul-terminated string.
#[derive(Clone, Hash, Ord, Eq, PartialOrd, PartialEq)]
pub struct ImString(pub(crate) Vec<u8>);

impl ImString {
    /// Creates a new `ImString` from an existing string.
    pub fn new<T: Into<String>>(value: T) -> ImString {
        let value = value.into();
        assert!(!value.contains('\0'), "ImString contained null byte");
        unsafe {
            let mut s = ImString::from_utf8_unchecked(value.into_bytes());
            s.refresh_len();
            s
        }
    }

    /// Creates a new empty `ImString` with a particular capacity
    #[inline]
    pub fn with_capacity(capacity: usize) -> ImString {
        let mut v = Vec::with_capacity(capacity + 1);
        v.push(b'\0');
        ImString(v)
    }

    /// Converts a vector of bytes to a `ImString` without checking that the string contains valid
    /// UTF-8
    ///
    /// # Safety
    ///
    /// It is up to the caller to guarantee the vector contains valid UTF-8 and no null terminator.
    #[inline]
    pub unsafe fn from_utf8_unchecked(mut v: Vec<u8>) -> ImString {
        v.push(b'\0');
        ImString(v)
    }

    /// Converts a vector of bytes to a `ImString` without checking that the string contains valid
    /// UTF-8
    ///
    /// # Safety
    ///
    /// It is up to the caller to guarantee the vector contains valid UTF-8 and a null terminator.
    #[inline]
    pub unsafe fn from_utf8_with_nul_unchecked(v: Vec<u8>) -> ImString {
        ImString(v)
    }

    /// Truncates this `ImString`, removing all contents
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear();
        self.0.push(b'\0');
    }

    /// Appends the given character to the end of this `ImString`
    #[inline]
    pub fn push(&mut self, ch: char) {
        let mut buf = [0; 4];
        self.push_str(ch.encode_utf8(&mut buf));
    }

    /// Appends a given string slice to the end of this `ImString`
    #[inline]
    pub fn push_str(&mut self, string: &str) {
        assert!(!string.contains('\0'), "ImString contained null byte");
        self.0.pop();
        self.0.extend(string.bytes());
        self.0.push(b'\0');
        unsafe {
            self.refresh_len();
        }
    }

    /// Returns the capacity of this `ImString` in bytes
    #[inline]
    pub fn capacity(&self) -> usize {
        self.0.capacity() - 1
    }

    /// Returns the capacity of this `ImString` in bytes, including the implicit null byte
    #[inline]
    pub fn capacity_with_nul(&self) -> usize {
        self.0.capacity()
    }

    /// Ensures that the capacity of this `ImString` is at least `additional` bytes larger than the
    /// current length.
    ///
    /// The capacity may be increased by more than `additional` bytes.
    pub fn reserve(&mut self, additional: usize) {
        self.0.reserve(additional);
    }

    /// Ensures that the capacity of this `ImString` is at least `additional` bytes larger than the
    /// current length
    pub fn reserve_exact(&mut self, additional: usize) {
        self.0.reserve_exact(additional);
    }

    /// Returns a raw pointer to the underlying buffer
    #[inline]
    pub fn as_ptr(&self) -> *const c_char {
        self.0.as_ptr() as *const c_char
    }

    /// Returns a raw mutable pointer to the underlying buffer.
    ///
    /// If the underlying data is modified, `refresh_len` *must* be called afterwards.
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut c_char {
        self.0.as_mut_ptr() as *mut c_char
    }

    /// Refreshes the length of the string by searching for the null terminator
    ///
    /// # Safety
    ///
    /// This function is unsafe because it assumes the buffer contains valid UTF-8
    /// and has a null terminator somewhere within the allocated capacity.
    pub unsafe fn refresh_len(&mut self) {
        unsafe {
            // For now, we'll use a simple implementation without libc
            // In a real implementation, you'd want to use libc::strlen or similar
            let mut len = 0;
            let ptr = self.as_ptr() as *const u8;
            while *ptr.add(len) != 0 {
                len += 1;
            }
            self.0.set_len(len + 1);
        }
    }

    /// Returns the length of this `ImString` in bytes, excluding the null terminator
    pub fn len(&self) -> usize {
        self.0.len().saturating_sub(1)
    }

    /// Returns true if this `ImString` is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Converts to a string slice
    pub fn to_str(&self) -> &str {
        unsafe { str::from_utf8_unchecked(&self.0[..self.len()]) }
    }
}

impl Default for ImString {
    fn default() -> Self {
        ImString::with_capacity(0)
    }
}

impl fmt::Display for ImString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.to_str(), f)
    }
}

impl fmt::Debug for ImString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.to_str(), f)
    }
}

impl Deref for ImString {
    type Target = str;
    fn deref(&self) -> &str {
        self.to_str()
    }
}

impl AsRef<str> for ImString {
    fn as_ref(&self) -> &str {
        self.to_str()
    }
}

impl From<String> for ImString {
    fn from(s: String) -> ImString {
        ImString::new(s)
    }
}

impl From<&str> for ImString {
    fn from(s: &str) -> ImString {
        ImString::new(s)
    }
}

impl Index<RangeFull> for ImString {
    type Output = str;
    fn index(&self, _index: RangeFull) -> &str {
        self.to_str()
    }
}

/// Represents a borrowed string that can be either a Rust string slice or an ImString
pub type ImStr<'a> = Cow<'a, str>;

/// Creates an ImString from a string literal at compile time
#[macro_export]
macro_rules! im_str {
    ($e:expr) => {{ $crate::ImString::new($e) }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn ui_buffer_push_appends_nul() {
        let mut buf = UiBuffer::new(1024);
        let start = buf.push("abc");
        assert_eq!(start, 0);
        assert_eq!(&buf.buffer, b"abc\0");
    }

    #[test]
    #[should_panic(expected = "null byte")]
    fn ui_buffer_rejects_interior_nul() {
        let mut buf = UiBuffer::new(1024);
        let _ = buf.push("a\0b");
    }

    #[test]
    fn tls_scratch_txt_is_nul_terminated() {
        let ptr = tls_scratch_txt("hello");
        let s = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap();
        assert_eq!(s, "hello");
    }

    #[test]
    fn tls_scratch_txt_two_returns_two_valid_strings() {
        let (a_ptr, b_ptr) = tls_scratch_txt_two("a", "bcd");
        let a = unsafe { CStr::from_ptr(a_ptr) }.to_str().unwrap();
        let b = unsafe { CStr::from_ptr(b_ptr) }.to_str().unwrap();
        assert_eq!(a, "a");
        assert_eq!(b, "bcd");
    }

    #[test]
    #[should_panic(expected = "null byte")]
    fn imstring_new_rejects_interior_nul() {
        let _ = ImString::new("a\0b");
    }

    #[test]
    #[should_panic(expected = "null byte")]
    fn imstring_push_str_rejects_interior_nul() {
        let mut s = ImString::new("a");
        s.push_str("b\0c");
    }
}

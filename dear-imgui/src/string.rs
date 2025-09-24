use std::borrow::Cow;
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
        self.buffer.as_ptr().add(pos) as *const _
    }

    /// Pushes a new scratch sheet text and return the byte index where the sub-string
    /// starts.
    pub fn push(&mut self, txt: impl AsRef<str>) -> usize {
        let len = self.buffer.len();
        self.buffer.extend(txt.as_ref().as_bytes());
        self.buffer.push(b'\0');

        len
    }
}

/// A UTF-8 encoded, growable, implicitly nul-terminated string.
#[derive(Clone, Hash, Ord, Eq, PartialOrd, PartialEq)]
pub struct ImString(pub(crate) Vec<u8>);

impl ImString {
    /// Creates a new `ImString` from an existing string.
    pub fn new<T: Into<String>>(value: T) -> ImString {
        unsafe {
            let mut s = ImString::from_utf8_unchecked(value.into().into_bytes());
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
        // For now, we'll use a simple implementation without libc
        // In a real implementation, you'd want to use libc::strlen or similar
        let mut len = 0;
        let ptr = self.as_ptr() as *const u8;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        self.0.set_len(len + 1);
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

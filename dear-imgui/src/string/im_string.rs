use std::borrow::Cow;
use std::fmt;
use std::ops::{Deref, Index, RangeFull};
use std::os::raw::c_char;
use std::str;

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

    /// Ensures the internal buffer length matches the requested size (including the trailing NUL).
    ///
    /// This is primarily used to prepare the backing storage for C APIs that write into the buffer
    /// using an explicit `BufSize` parameter (e.g. `InputText`).
    pub(crate) fn ensure_buf_size(&mut self, buf_size: usize) {
        if self.0.len() < buf_size {
            self.0.resize(buf_size, 0);
        } else if self.0.len() > buf_size {
            self.0.truncate(buf_size);
            if let Some(last) = self.0.last_mut() {
                *last = 0;
            } else {
                self.0.push(0);
            }
        } else if let Some(last) = self.0.last_mut() {
            *last = 0;
        }
    }

    /// Refreshes the length of the string by searching for the null terminator
    ///
    /// # Safety
    ///
    /// This function is unsafe because it assumes the initialized bytes before the null terminator
    /// contain valid UTF-8.
    pub unsafe fn refresh_len(&mut self) {
        if let Some(pos) = self.0.iter().position(|&b| b == 0) {
            self.0.truncate(pos + 1);
        } else {
            self.0.push(0);
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

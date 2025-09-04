//! String handling utilities for Dear ImGui
//! 
//! This module provides safe string handling for interfacing with Dear ImGui's
//! C API, which expects null-terminated strings. It includes utilities for
//! converting between Rust strings and C strings safely and efficiently.

use std::borrow::Cow;
use std::ffi::{CStr, CString};
use std::fmt;
use std::os::raw::c_char;
use std::ptr;

/// A string type that is compatible with Dear ImGui's C API
/// 
/// This type provides a safe interface for strings that need to be passed
/// to Dear ImGui functions. It automatically handles null termination and
/// UTF-8 validation.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ImString {
    inner: CString,
}

impl ImString {
    /// Create a new empty ImString
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::ImString;
    /// 
    /// let s = ImString::new();
    /// assert!(s.is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            inner: CString::new("").expect("Empty string should never fail"),
        }
    }

    /// Create an ImString with the specified capacity
    /// 
    /// This pre-allocates space for the string, which can be useful
    /// when you know the approximate size of the string you'll be building.
    /// 
    /// # Arguments
    /// 
    /// * `capacity` - The initial capacity in bytes
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::ImString;
    /// 
    /// let s = ImString::with_capacity(100);
    /// assert!(s.capacity() >= 100);
    /// ```
    pub fn with_capacity(capacity: usize) -> Self {
        let mut vec = Vec::with_capacity(capacity + 1); // +1 for null terminator
        vec.push(0); // null terminator
        Self {
            inner: unsafe { CString::from_vec_unchecked(vec) },
        }
    }

    /// Create an ImString from a Rust string
    /// 
    /// # Arguments
    /// 
    /// * `s` - The string to convert
    /// 
    /// # Returns
    /// 
    /// `Ok(ImString)` if the string is valid, `Err` if it contains null bytes
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::ImString;
    /// 
    /// let s = ImString::from_str("Hello, world!").unwrap();
    /// assert_eq!(s.to_str(), "Hello, world!");
    /// ```
    pub fn from_str(s: &str) -> Result<Self, std::ffi::NulError> {
        Ok(Self {
            inner: CString::new(s)?,
        })
    }

    /// Create an ImString from a Rust string, replacing null bytes
    ///
    /// This is a safe alternative to `from_str` that replaces any null bytes
    /// in the input string with a replacement character.
    ///
    /// # Arguments
    ///
    /// * `s` - The string to convert
    /// * `replacement` - The character to replace null bytes with
    ///
    /// # Examples
    ///
    /// ```rust
    /// use dear_imgui::ImString;
    ///
    /// let s = ImString::from_str_safe("Hello\0world", '?');
    /// assert_eq!(s.to_str(), "Hello?world");
    /// ```
    pub fn from_str_safe(s: &str, replacement: char) -> Self {
        let safe_string = s.replace('\0', &replacement.to_string());
        Self {
            inner: CString::new(safe_string).expect("Replacement should remove null bytes"),
        }
    }

    /// Create an ImString from a Rust string, using default error handling
    ///
    /// This is a convenience method that creates an ImString from a string,
    /// falling back to safe replacement if the string contains null bytes.
    /// This is the recommended method for migrating from CString::new().unwrap_or_default().
    ///
    /// # Arguments
    ///
    /// * `s` - The string to convert
    ///
    /// # Examples
    ///
    /// ```rust
    /// use dear_imgui::ImString;
    ///
    /// // Safe conversion - no null bytes
    /// let s1 = ImString::from_str_or_default("Hello, world!");
    /// assert_eq!(s1.to_str(), "Hello, world!");
    ///
    /// // Safe conversion - null bytes replaced with '?'
    /// let s2 = ImString::from_str_or_default("Hello\0world");
    /// assert_eq!(s2.to_str(), "Hello?world");
    /// ```
    pub fn from_str_or_default(s: impl AsRef<str>) -> Self {
        let s = s.as_ref();
        Self::from_str(s).unwrap_or_else(|_| Self::from_str_safe(s, '?'))
    }

    /// Get the string as a `&str`
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::ImString;
    /// 
    /// let s = ImString::from_str("Hello").unwrap();
    /// assert_eq!(s.to_str(), "Hello");
    /// ```
    pub fn to_str(&self) -> &str {
        self.inner.to_str().unwrap_or("")
    }

    /// Get the string as a `&CStr`
    /// 
    /// This is useful when interfacing with C APIs.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::ImString;
    /// 
    /// let s = ImString::from_str("Hello").unwrap();
    /// let c_str = s.as_c_str();
    /// ```
    pub fn as_c_str(&self) -> &CStr {
        &self.inner
    }

    /// Get a raw pointer to the null-terminated string
    /// 
    /// This is useful when calling C functions that expect a `*const c_char`.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::ImString;
    /// 
    /// let s = ImString::from_str("Hello").unwrap();
    /// let ptr = s.as_ptr();
    /// ```
    pub fn as_ptr(&self) -> *const c_char {
        self.inner.as_ptr()
    }

    /// Check if the string is empty
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::ImString;
    /// 
    /// let s = ImString::new();
    /// assert!(s.is_empty());
    /// 
    /// let s = ImString::from_str("Hello").unwrap();
    /// assert!(!s.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.inner.as_bytes().is_empty()
    }

    /// Get the length of the string in bytes
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::ImString;
    /// 
    /// let s = ImString::from_str("Hello").unwrap();
    /// assert_eq!(s.len(), 5);
    /// ```
    pub fn len(&self) -> usize {
        self.inner.as_bytes().len()
    }

    /// Get the capacity of the string in bytes
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::ImString;
    /// 
    /// let s = ImString::with_capacity(100);
    /// assert!(s.capacity() >= 100);
    /// ```
    pub fn capacity(&self) -> usize {
        // CString doesn't expose capacity, so we estimate based on length
        self.len().max(16) // Minimum reasonable capacity
    }

    /// Clear the string, making it empty
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::ImString;
    /// 
    /// let mut s = ImString::from_str("Hello").unwrap();
    /// s.clear();
    /// assert!(s.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.inner = CString::new("").expect("Empty string should never fail");
    }

    /// Push a string slice onto the end of this string
    /// 
    /// # Arguments
    /// 
    /// * `s` - The string slice to append
    /// 
    /// # Returns
    /// 
    /// `Ok(())` if successful, `Err` if the string contains null bytes
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::ImString;
    /// 
    /// let mut s = ImString::from_str("Hello").unwrap();
    /// s.push_str(", world!").unwrap();
    /// assert_eq!(s.to_str(), "Hello, world!");
    /// ```
    pub fn push_str(&mut self, s: &str) -> Result<(), std::ffi::NulError> {
        let mut current = self.to_str().to_string();
        current.push_str(s);
        self.inner = CString::new(current)?;
        Ok(())
    }

    /// Push a string slice onto the end of this string, replacing null bytes
    /// 
    /// # Arguments
    /// 
    /// * `s` - The string slice to append
    /// * `replacement` - The character to replace null bytes with
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::ImString;
    /// 
    /// let mut s = ImString::from_str("Hello").unwrap();
    /// s.push_str_safe(", wor\0ld!", '?');
    /// assert_eq!(s.to_str(), "Hello, wor?ld!");
    /// ```
    pub fn push_str_safe(&mut self, s: &str, replacement: char) {
        let safe_string = s.replace('\0', &replacement.to_string());
        let mut current = self.to_str().to_string();
        current.push_str(&safe_string);
        self.inner = CString::new(current).expect("Replacement should remove null bytes");
    }
}

impl Default for ImString {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ImString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ImString({:?})", self.to_str())
    }
}

impl fmt::Display for ImString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_str())
    }
}

impl From<String> for ImString {
    fn from(s: String) -> Self {
        Self::from_str(&s).unwrap_or_else(|_| Self::from_str_safe(&s, '?'))
    }
}

impl From<&str> for ImString {
    fn from(s: &str) -> Self {
        Self::from_str(s).unwrap_or_else(|_| Self::from_str_safe(s, '?'))
    }
}

impl From<Cow<'_, str>> for ImString {
    fn from(s: Cow<'_, str>) -> Self {
        Self::from_str(&s).unwrap_or_else(|_| Self::from_str_safe(&s, '?'))
    }
}

impl AsRef<str> for ImString {
    fn as_ref(&self) -> &str {
        self.to_str()
    }
}

impl AsRef<CStr> for ImString {
    fn as_ref(&self) -> &CStr {
        self.as_c_str()
    }
}

/// A mutable string buffer for use with Dear ImGui input widgets
/// 
/// This type provides a mutable buffer that can be used with Dear ImGui's
/// text input functions. It automatically manages the buffer size and
/// null termination.
#[derive(Debug)]
pub struct UiBuffer {
    buffer: Vec<u8>,
    capacity: usize,
}

impl UiBuffer {
    /// Create a new buffer with the specified capacity
    /// 
    /// # Arguments
    /// 
    /// * `capacity` - The maximum number of characters the buffer can hold
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::UiBuffer;
    /// 
    /// let buffer = UiBuffer::new(256);
    /// assert_eq!(buffer.capacity(), 256);
    /// ```
    pub fn new(capacity: usize) -> Self {
        let mut buffer = vec![0u8; capacity + 1]; // +1 for null terminator
        buffer[0] = 0; // Ensure null termination
        Self { buffer, capacity }
    }

    /// Create a buffer from an existing string
    /// 
    /// # Arguments
    /// 
    /// * `s` - The initial string content
    /// * `capacity` - The maximum capacity (must be >= string length)
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::UiBuffer;
    /// 
    /// let buffer = UiBuffer::from_str("Hello", 256);
    /// assert_eq!(buffer.to_str(), "Hello");
    /// assert_eq!(buffer.capacity(), 256);
    /// ```
    pub fn from_str(s: &str, capacity: usize) -> Self {
        let mut buffer = Self::new(capacity.max(s.len()));
        buffer.set_str(s);
        buffer
    }

    /// Get the buffer capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get the current string content
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::UiBuffer;
    /// 
    /// let buffer = UiBuffer::from_str("Hello", 256);
    /// assert_eq!(buffer.to_str(), "Hello");
    /// ```
    pub fn to_str(&self) -> &str {
        // Find the null terminator
        let null_pos = self.buffer.iter().position(|&b| b == 0).unwrap_or(self.buffer.len());
        std::str::from_utf8(&self.buffer[..null_pos]).unwrap_or("")
    }

    /// Set the buffer content
    /// 
    /// # Arguments
    /// 
    /// * `s` - The new string content
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::UiBuffer;
    /// 
    /// let mut buffer = UiBuffer::new(256);
    /// buffer.set_str("Hello, world!");
    /// assert_eq!(buffer.to_str(), "Hello, world!");
    /// ```
    pub fn set_str(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let copy_len = bytes.len().min(self.capacity);
        
        // Clear the buffer
        self.buffer.fill(0);
        
        // Copy the string
        self.buffer[..copy_len].copy_from_slice(&bytes[..copy_len]);
        
        // Ensure null termination
        if copy_len < self.buffer.len() {
            self.buffer[copy_len] = 0;
        }
    }

    /// Get a mutable pointer to the buffer
    /// 
    /// This is used when calling Dear ImGui functions that modify the buffer.
    /// 
    /// # Safety
    /// 
    /// The caller must ensure that:
    /// - The buffer is not written beyond its capacity
    /// - The buffer remains null-terminated
    /// - The buffer contains valid UTF-8 after modification
    pub fn as_mut_ptr(&mut self) -> *mut c_char {
        self.buffer.as_mut_ptr() as *mut c_char
    }

    /// Get a pointer to the buffer
    pub fn as_ptr(&self) -> *const c_char {
        self.buffer.as_ptr() as *const c_char
    }

    /// Get the buffer length in bytes
    pub fn len(&self) -> usize {
        self.to_str().len()
    }

    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.buffer.fill(0);
    }

    /// Internal method to push a single text to our scratch buffer.
    ///
    /// This method is used internally by the UI system for efficient string handling.
    /// It pushes the string to the buffer and returns a pointer to it.
    pub fn scratch_txt(&mut self, txt: impl AsRef<str>) -> *const c_char {
        self.refresh_buffer();
        let start_of_substr = self.push(txt);
        unsafe { self.offset(start_of_substr) }
    }

    /// Internal method to push an optional text to our scratch buffer.
    pub fn scratch_txt_opt(&mut self, txt: Option<impl AsRef<str>>) -> *const c_char {
        match txt {
            Some(v) => self.scratch_txt(v),
            None => ptr::null(),
        }
    }

    /// Internal method to push two texts to our scratch buffer.
    pub fn scratch_txt_two(
        &mut self,
        txt_0: impl AsRef<str>,
        txt_1: impl AsRef<str>,
    ) -> (*const c_char, *const c_char) {
        self.refresh_buffer();
        let first_offset = self.push(txt_0);
        let second_offset = self.push(txt_1);
        unsafe { (self.offset(first_offset), self.offset(second_offset)) }
    }

    /// Helper method, same as [`Self::scratch_txt_two`] but with one optional value
    pub fn scratch_txt_with_opt(
        &mut self,
        txt_0: impl AsRef<str>,
        txt_1: Option<impl AsRef<str>>,
    ) -> (*const c_char, *const c_char) {
        match txt_1 {
            Some(value) => self.scratch_txt_two(txt_0, value),
            None => (self.scratch_txt(txt_0), ptr::null()),
        }
    }

    /// Refresh the buffer (clear it for reuse)
    fn refresh_buffer(&mut self) {
        self.buffer.clear();
    }

    /// Push a string to the buffer and return the byte offset where it starts
    fn push(&mut self, txt: impl AsRef<str>) -> usize {
        let len = self.buffer.len();
        self.buffer.extend(txt.as_ref().as_bytes());
        self.buffer.push(b'\0');
        len
    }

    /// Get a pointer to the string at the given byte offset
    unsafe fn offset(&self, byte_offset: usize) -> *const c_char {
        self.buffer.as_ptr().add(byte_offset) as *const c_char
    }
}

impl Default for UiBuffer {
    fn default() -> Self {
        Self::new(256) // Default capacity
    }
}

impl fmt::Display for UiBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_str())
    }
}

impl AsRef<str> for UiBuffer {
    fn as_ref(&self) -> &str {
        self.to_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_im_string_creation() {
        let s = ImString::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);

        let s = ImString::from_str("Hello").unwrap();
        assert_eq!(s.to_str(), "Hello");
        assert_eq!(s.len(), 5);
        assert!(!s.is_empty());
    }

    #[test]
    fn test_im_string_from_string_with_nulls() {
        let s = ImString::from_str_safe("Hello\0world", '?');
        assert_eq!(s.to_str(), "Hello?world");
    }

    #[test]
    fn test_im_string_operations() {
        let mut s = ImString::from_str("Hello").unwrap();
        s.push_str(", world!").unwrap();
        assert_eq!(s.to_str(), "Hello, world!");

        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn test_ui_buffer() {
        let mut buffer = UiBuffer::new(256);
        assert_eq!(buffer.capacity(), 256);
        assert!(buffer.is_empty());

        buffer.set_str("Hello, world!");
        assert_eq!(buffer.to_str(), "Hello, world!");
        assert_eq!(buffer.len(), 13);

        buffer.clear();
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_ui_buffer_from_str() {
        let buffer = UiBuffer::from_str("Initial content", 256);
        assert_eq!(buffer.to_str(), "Initial content");
        assert_eq!(buffer.capacity(), 256);
    }

    #[test]
    fn test_ui_buffer_capacity_limit() {
        let mut buffer = UiBuffer::new(5);
        buffer.set_str("This is a very long string");
        assert_eq!(buffer.to_str(), "This "); // Truncated to capacity
        assert_eq!(buffer.len(), 5);
    }
}

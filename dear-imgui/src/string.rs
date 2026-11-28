//! Scratch string buffers
//!
//! [`UiBuffer`] stages temporary C strings for widget labels and hints.
//!
//! The unused `ImStr` alias and allocating `im_str!` compatibility macro are intentionally absent:
//! ```compile_fail
//! let _: dear_imgui_rs::ImStr<'static> = std::borrow::Cow::Borrowed("text");
//! ```
//! ```compile_fail
//! let _ = dear_imgui_rs::im_str!("text");
//! ```

use std::cell::RefCell;
use std::mem::MaybeUninit;
use std::ops::Range;
use std::os::raw::c_char;
use std::ptr;

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
    ///
    /// Note: any interior NUL bytes (`'\0'`) will be replaced with `?` to preserve C string semantics.
    pub fn scratch_txt(&mut self, txt: impl AsRef<str>) -> *const c_char {
        self.refresh_buffer();

        let start_of_substr = self.push(txt);
        unsafe { self.offset(start_of_substr) }
    }

    /// Stages multiple string parts as one NUL-terminated C string.
    pub(crate) fn scratch_txt_concat(&mut self, parts: &[&str]) -> *const c_char {
        self.refresh_buffer();

        let start = self.buffer.len();
        for part in parts {
            self.extend_c_string_bytes(part.as_bytes());
        }
        self.buffer.push(0);
        unsafe { self.offset(start) }
    }

    /// Stages an explicit text range followed by a readable NUL sentinel.
    ///
    /// Unlike [`Self::scratch_txt`], this preserves interior NUL bytes because
    /// range-based Dear ImGui APIs receive the end pointer separately.
    pub fn scratch_txt_range(&mut self, txt: impl AsRef<str>) -> Range<*const c_char> {
        self.refresh_buffer();

        let start = self.buffer.len();
        self.buffer.extend_from_slice(txt.as_ref().as_bytes());
        let end = self.buffer.len();
        self.buffer.push(0);

        unsafe { self.offset(start)..self.offset(end) }
    }

    /// Internal method to push an option text to our scratch buffer.
    pub fn scratch_txt_opt(&mut self, txt: Option<impl AsRef<str>>) -> *const c_char {
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
    ) -> (*const c_char, *const c_char) {
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
    ) -> (*const c_char, *const c_char) {
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
    pub unsafe fn offset(&self, pos: usize) -> *const c_char {
        unsafe { self.buffer.as_ptr().add(pos) as *const _ }
    }

    /// Pushes a new scratch sheet text and return the byte index where the sub-string
    /// starts.
    pub fn push(&mut self, txt: impl AsRef<str>) -> usize {
        let txt = txt.as_ref();
        let len = self.buffer.len();
        self.extend_c_string_bytes(txt.as_bytes());
        self.buffer.push(b'\0');

        len
    }

    fn extend_c_string_bytes(&mut self, bytes: &[u8]) {
        if bytes.contains(&0) {
            self.buffer.extend(
                bytes
                    .iter()
                    .map(|&byte| if byte == 0 { b'?' } else { byte }),
            );
        } else {
            self.buffer.extend(bytes);
        }
    }
}

const SCRATCH_LIMIT: usize = 1024;

#[derive(Debug, Default)]
pub(crate) struct ScratchBuffers(RefCell<Vec<UiBuffer>>);

struct ScratchBuffer<'a> {
    pool: &'a ScratchBuffers,
    buffer: UiBuffer,
}

impl Drop for ScratchBuffer<'_> {
    fn drop(&mut self) {
        let buffer = std::mem::replace(&mut self.buffer, UiBuffer::new(SCRATCH_LIMIT));
        self.pool.0.borrow_mut().push(buffer);
    }
}

impl ScratchBuffers {
    #[inline]
    pub(crate) fn with_c_string<R>(&self, text: &str, f: impl FnOnce(*const c_char) -> R) -> R {
        let bytes = text.as_bytes();
        if bytes.len() < 64 {
            let mut buffer = MaybeUninit::<[u8; 64]>::uninit();
            let start = buffer.as_mut_ptr().cast::<u8>();
            // SAFETY: `bytes` fits with room for a terminator. Initialize every byte
            // that the C string can read, leaving the unused tail uninitialized.
            unsafe {
                if bytes.contains(&0) {
                    copy_replacing_nul(bytes, start);
                } else {
                    ptr::copy_nonoverlapping(bytes.as_ptr(), start, bytes.len());
                }
                start.add(bytes.len()).write(0);
            }
            return f(start.cast());
        }
        self.with_long_c_string(text, f)
    }

    #[inline(never)]
    fn with_long_c_string<R>(&self, text: &str, f: impl FnOnce(*const c_char) -> R) -> R {
        self.with(|buffer| f(buffer.scratch_txt(text)))
    }

    // The most recently returned buffer contains the caller's staged labels. Keep it off the
    // stack during user code so AsRef, Drop, or native callbacks can use another allocation.
    pub(crate) fn with<R>(&self, f: impl FnOnce(&mut UiBuffer) -> R) -> R {
        let buffer = self
            .0
            .borrow_mut()
            .pop()
            .unwrap_or_else(|| UiBuffer::new(SCRATCH_LIMIT));
        let mut scratch = ScratchBuffer { pool: self, buffer };
        f(&mut scratch.buffer)
    }
}

// The caller provides space for all of `bytes` without overlapping it.
#[cold]
unsafe fn copy_replacing_nul(bytes: &[u8], start: *mut u8) {
    for (index, &byte) in bytes.iter().enumerate() {
        unsafe { start.add(index).write(if byte == 0 { b'?' } else { byte }) };
    }
}

thread_local! {
    static TLS_SCRATCH: ScratchBuffers = const { ScratchBuffers(RefCell::new(Vec::new())) };
}

fn with_tls_scratch<R>(f: impl FnOnce(&mut UiBuffer) -> R) -> R {
    TLS_SCRATCH.with(|pool| pool.with(f))
}

pub(crate) fn with_scratch_txt_range<R>(
    text: &str,
    f: impl FnOnce(std::ops::Range<*const c_char>) -> R,
) -> R {
    with_tls_scratch(|buf| f(buf.scratch_txt_range(text)))
}

/// Calls `f` with a temporary, NUL-terminated C string pointer backed by a thread-local scratch buffer.
///
/// The pointer is valid throughout `f`, including nested scratch-string calls on this thread.
pub fn with_scratch_txt<R>(txt: impl AsRef<str>, f: impl FnOnce(*const c_char) -> R) -> R {
    with_tls_scratch(|buf| {
        let ptr = buf.scratch_txt(txt);
        f(ptr)
    })
}

/// Calls `f` with two temporary, NUL-terminated C string pointers backed by a thread-local scratch buffer.
///
/// Both pointers are valid throughout `f`, including nested scratch-string calls.
pub fn with_scratch_txt_two<R>(
    txt_0: impl AsRef<str>,
    txt_1: impl AsRef<str>,
    f: impl FnOnce(*const c_char, *const c_char) -> R,
) -> R {
    with_tls_scratch(|buf| {
        let (a, b) = buf.scratch_txt_two(txt_0, txt_1);
        f(a, b)
    })
}

/// Calls `f` with three temporary, NUL-terminated C string pointers backed by a thread-local scratch buffer.
///
/// All pointers are valid throughout `f`, including nested scratch-string calls.
pub fn with_scratch_txt_three<R>(
    txt_0: impl AsRef<str>,
    txt_1: impl AsRef<str>,
    txt_2: impl AsRef<str>,
    f: impl FnOnce(*const c_char, *const c_char, *const c_char) -> R,
) -> R {
    with_tls_scratch(|buf| {
        buf.refresh_buffer();
        let o0 = buf.push(txt_0);
        let o1 = buf.push(txt_1);
        let o2 = buf.push(txt_2);
        unsafe { f(buf.offset(o0), buf.offset(o1), buf.offset(o2)) }
    })
}

/// Calls `f` with a list of temporary, NUL-terminated C string pointers backed by a thread-local scratch buffer.
///
/// The pointers are valid throughout `f`, including nested scratch-string calls.
pub fn with_scratch_txt_slice<R>(txts: &[&str], f: impl FnOnce(&[*const c_char]) -> R) -> R {
    with_tls_scratch(|buf| {
        buf.refresh_buffer();

        let total_bytes: usize = txts.iter().map(|s| s.len() + 1).sum();
        buf.buffer.reserve(total_bytes);

        let mut offsets: Vec<usize> = Vec::with_capacity(txts.len());
        for &s in txts {
            offsets.push(buf.push(s));
        }

        let mut ptrs: Vec<*const c_char> = Vec::with_capacity(txts.len());
        for off in offsets {
            ptrs.push(unsafe { buf.offset(off) });
        }

        f(&ptrs)
    })
}

/// Calls `f` with a list of temporary, NUL-terminated C string pointers and one optional pointer backed by
/// a thread-local scratch buffer.
///
/// The pointers are valid throughout `f`, including nested scratch-string calls.
pub fn with_scratch_txt_slice_with_opt<R>(
    txts: &[&str],
    txt_opt: Option<&str>,
    f: impl FnOnce(&[*const c_char], *const c_char) -> R,
) -> R {
    with_tls_scratch(|buf| {
        buf.refresh_buffer();

        let total_bytes: usize = txts.iter().map(|s| s.len() + 1).sum::<usize>()
            + txt_opt.map(|s| s.len() + 1).unwrap_or(0);
        buf.buffer.reserve(total_bytes);

        let mut offsets: Vec<usize> = Vec::with_capacity(txts.len());
        for &s in txts {
            offsets.push(buf.push(s));
        }

        let opt_off = txt_opt.map(|s| buf.push(s));

        let mut ptrs: Vec<*const c_char> = Vec::with_capacity(txts.len());
        for off in offsets {
            ptrs.push(unsafe { buf.offset(off) });
        }

        let opt_ptr = match opt_off {
            Some(off) => unsafe { buf.offset(off) },
            None => std::ptr::null(),
        };

        f(&ptrs, opt_ptr)
    })
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
    fn ui_buffer_sanitizes_interior_nul() {
        let mut buf = UiBuffer::new(1024);
        let ptr = buf.scratch_txt("a\0b");
        let s = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap();
        assert_eq!(s, "a?b");
    }

    #[test]
    fn scoped_c_strings_sanitize_interior_nul_on_both_paths() {
        let scratch = ScratchBuffers::default();
        for prefix in [0, 64] {
            let label = format!("{}a\0b", "x".repeat(prefix));
            scratch.with_c_string(&label, |ptr| {
                let text = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap();
                assert_eq!(text, label.replace('\0', "?"));
            });
        }
    }

    #[test]
    fn scoped_c_strings_terminate_at_the_label_length() {
        let scratch = ScratchBuffers::default();
        for label in [String::new(), "é".repeat(31) + "x", "x".repeat(64)] {
            scratch.with_c_string(&label, |ptr| {
                scratch.with_c_string("nested", |nested| {
                    assert_eq!(unsafe { CStr::from_ptr(nested) }, c"nested");
                });
                // SAFETY: The callback keeps this label's C string live.
                assert_eq!(unsafe { CStr::from_ptr(ptr) }.to_bytes(), label.as_bytes());
            });
        }
    }

    #[test]
    fn ui_buffer_text_range_preserves_bytes_and_appends_readable_nul() {
        let mut buf = UiBuffer::new(1024);
        let range = buf.scratch_txt_range("a\0b#");
        let len = unsafe { range.end.offset_from(range.start) as usize };
        let bytes = unsafe { std::slice::from_raw_parts(range.start.cast::<u8>(), len + 1) };

        assert_eq!(bytes, b"a\0b#\0");
    }

    #[test]
    fn scoped_scratch_strings_survive_nested_queries_and_unwind() {
        with_scratch_txt("outer", |outer| {
            let result = std::panic::catch_unwind(|| {
                with_scratch_txt("inner", |inner| {
                    assert_eq!(unsafe { CStr::from_ptr(inner) }, c"inner");
                    panic!("leave nested scratch scope");
                });
            });
            assert!(result.is_err());
            with_scratch_txt("next", |next| {
                assert_eq!(unsafe { CStr::from_ptr(next) }, c"next");
            });
            assert_eq!(unsafe { CStr::from_ptr(outer) }, c"outer");
        });
    }

    #[test]
    fn with_scratch_txt_two_returns_two_valid_strings() {
        with_scratch_txt_two("a", "bcd", |a_ptr, b_ptr| {
            let a = unsafe { CStr::from_ptr(a_ptr) }.to_str().unwrap();
            let b = unsafe { CStr::from_ptr(b_ptr) }.to_str().unwrap();
            assert_eq!(a, "a");
            assert_eq!(b, "bcd");
        });
    }

    #[test]
    fn with_scratch_txt_slice_returns_sequential_pointers() {
        with_scratch_txt_slice(&["a", "bc", "def"], |ptrs| {
            assert_eq!(ptrs.len(), 3);

            let a = unsafe { CStr::from_ptr(ptrs[0]) }.to_str().unwrap();
            let b = unsafe { CStr::from_ptr(ptrs[1]) }.to_str().unwrap();
            let c = unsafe { CStr::from_ptr(ptrs[2]) }.to_str().unwrap();
            assert_eq!(a, "a");
            assert_eq!(b, "bc");
            assert_eq!(c, "def");

            let ab = (ptrs[1] as usize) - (ptrs[0] as usize);
            let bc = (ptrs[2] as usize) - (ptrs[1] as usize);
            assert_eq!(ab, "a".len() + 1);
            assert_eq!(bc, "bc".len() + 1);
        });
    }

    #[test]
    fn with_scratch_txt_slice_with_opt_returns_null_for_none() {
        with_scratch_txt_slice_with_opt(&["a", "bc"], None, |ptrs, opt_ptr| {
            assert_eq!(ptrs.len(), 2);
            assert!(opt_ptr.is_null());

            let a = unsafe { CStr::from_ptr(ptrs[0]) }.to_str().unwrap();
            let b = unsafe { CStr::from_ptr(ptrs[1]) }.to_str().unwrap();
            assert_eq!(a, "a");
            assert_eq!(b, "bc");
        });
    }

    #[test]
    fn with_scratch_txt_slice_with_opt_appends_opt_string() {
        with_scratch_txt_slice_with_opt(&["a", "bc"], Some("fmt"), |ptrs, opt_ptr| {
            assert_eq!(ptrs.len(), 2);
            assert!(!opt_ptr.is_null());

            let a = unsafe { CStr::from_ptr(ptrs[0]) }.to_str().unwrap();
            let b = unsafe { CStr::from_ptr(ptrs[1]) }.to_str().unwrap();
            let fmt = unsafe { CStr::from_ptr(opt_ptr) }.to_str().unwrap();
            assert_eq!(a, "a");
            assert_eq!(b, "bc");
            assert_eq!(fmt, "fmt");

            let ab = (ptrs[1] as usize) - (ptrs[0] as usize);
            let bf = (opt_ptr as usize) - (ptrs[1] as usize);
            assert_eq!(ab, "a".len() + 1);
            assert_eq!(bf, "bc".len() + 1);
        });
    }
}

use super::UiBuffer;
use std::cell::RefCell;
use std::os::raw::c_char;

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
            let mut buffer = [0; 64];
            for (slot, &byte) in buffer.iter_mut().zip(bytes) {
                *slot = if byte == 0 { b'?' } else { byte };
            }
            return f(buffer.as_ptr().cast());
        }
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

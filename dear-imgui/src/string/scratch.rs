use super::UiBuffer;
use std::cell::RefCell;
use std::os::raw::c_char;

thread_local! {
    static TLS_SCRATCH: RefCell<UiBuffer> = RefCell::new(UiBuffer::new(1024));
}

/// Creates a temporary, NUL-terminated C string pointer backed by a thread-local scratch buffer.
///
/// The returned pointer is only valid until the next scratch-string call on the same thread.
///
/// This API is **not re-entrant**: any nested call to `tls_scratch_txt`/`with_scratch_txt` (or `Ui::scratch_txt`)
/// on the same thread will overwrite the backing buffer and invalidate previously returned pointers.
pub(crate) fn tls_scratch_txt(txt: impl AsRef<str>) -> *const c_char {
    TLS_SCRATCH.with(|buf| buf.borrow_mut().scratch_txt(txt))
}

/// Calls `f` with a temporary, NUL-terminated C string pointer backed by a thread-local scratch buffer.
///
/// The pointer is only valid for the duration of the call (and will be overwritten by subsequent
/// scratch-string operations on the same thread). Like [`tls_scratch_txt`], this is not re-entrant.
pub fn with_scratch_txt<R>(txt: impl AsRef<str>, f: impl FnOnce(*const c_char) -> R) -> R {
    TLS_SCRATCH.with(|buf| {
        let mut buf = buf.borrow_mut();
        let ptr = buf.scratch_txt(txt);
        f(ptr)
    })
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

/// Calls `f` with a list of temporary, NUL-terminated C string pointers backed by a thread-local scratch buffer.
///
/// The pointers are only valid for the duration of the call (and will be overwritten by subsequent
/// scratch-string operations on the same thread).
pub fn with_scratch_txt_slice<R>(txts: &[&str], f: impl FnOnce(&[*const c_char]) -> R) -> R {
    TLS_SCRATCH.with(|buf| {
        let mut buf = buf.borrow_mut();
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
/// The returned pointers are only valid for the duration of the call (and will be overwritten by subsequent
/// scratch-string operations on the same thread).
pub fn with_scratch_txt_slice_with_opt<R>(
    txts: &[&str],
    txt_opt: Option<&str>,
    f: impl FnOnce(&[*const c_char], *const c_char) -> R,
) -> R {
    TLS_SCRATCH.with(|buf| {
        let mut buf = buf.borrow_mut();
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

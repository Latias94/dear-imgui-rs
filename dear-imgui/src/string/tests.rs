use super::*;
use std::ffi::CStr;

#[test]
fn im_string_ensure_buf_size_resizes_and_nul_terminates() {
    let mut s = ImString::new("abc");
    s.ensure_buf_size(16);
    assert_eq!(s.0.len(), 16);
    assert_eq!(&s.0[..3], b"abc");
    assert_eq!(s.0[3], 0);
    assert!(s.0[4..].iter().all(|&b| b == 0));
}

#[test]
fn im_string_refresh_len_does_not_scan_spare_capacity() {
    let mut v = vec![b'x'; 16];
    v[..4].copy_from_slice(b"abcd");
    v[10] = 0;
    v.truncate(4);

    let mut s = ImString(v);
    unsafe { s.refresh_len() };
    assert_eq!(s.to_str(), "abcd");
    assert_eq!(s.0.last().copied(), Some(0));
}

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

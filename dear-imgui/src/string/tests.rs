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

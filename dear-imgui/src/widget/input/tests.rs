use super::buffers::{finish_string_input_buffer, resize_string_input_buffer};
use super::validation::{validate_input_scalar_flags, validate_input_text_flags};
use crate::{InputTextFlags, sys};

#[test]
fn string_input_buffer_finish_truncates_at_nul() {
    let mut out = String::from("old");
    finish_string_input_buffer(&mut out, b"new\0ignored".to_vec());
    assert_eq!(out, "new");
}

#[test]
fn string_input_buffer_finish_replaces_invalid_utf8() {
    let mut out = String::new();
    finish_string_input_buffer(&mut out, vec![b'a', 0xff, b'b', 0]);
    assert_eq!(out, "a\u{fffd}b");
}

#[test]
fn string_input_buffer_finish_reuses_target_capacity() {
    let mut out = String::with_capacity(16);
    out.push_str("old");
    let cap = out.capacity();

    finish_string_input_buffer(&mut out, b"new\0ignored".to_vec());

    assert_eq!(out, "new");
    assert!(out.capacity() >= cap);
}

#[test]
fn string_input_buffer_finish_keeps_input_capacity() {
    let mut out = String::new();
    let mut buffer = Vec::with_capacity(64);
    buffer.extend_from_slice(b"new\0");
    let cap = buffer.capacity();

    finish_string_input_buffer(&mut out, buffer);

    assert_eq!(out, "new");
    assert!(out.capacity() >= cap);
}

#[test]
fn string_input_buffer_resize_updates_imgui_buffer_pointer() {
    let mut buffer = b"abc\0".to_vec();
    let mut data = sys::ImGuiInputTextCallbackData::default();
    data.Buf = buffer.as_mut_ptr().cast();
    data.BufSize = 32;

    let result = unsafe { resize_string_input_buffer(&mut buffer, data.BufSize, &mut data) };

    assert_eq!(result, 0);
    assert!(buffer.len() >= 32);
    assert_eq!(data.Buf, buffer.as_mut_ptr().cast());
    assert!(data.BufDirty);
}

#[test]
fn input_text_flags_reject_unsupported_bits_and_invalid_combinations() {
    let private_multiline = InputTextFlags::from_bits_retain(sys::ImGuiInputTextFlags_Multiline);
    assert!(
        std::panic::catch_unwind(|| {
            validate_input_text_flags("test", private_multiline, false)
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            validate_input_text_flags(
                "test",
                InputTextFlags::CALLBACK_COMPLETION | InputTextFlags::ALLOW_TAB_INPUT,
                false,
            )
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            validate_input_text_flags(
                "test",
                InputTextFlags::WORD_WRAP | InputTextFlags::PASSWORD,
                true,
            )
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            validate_input_text_flags("test", InputTextFlags::WORD_WRAP, false)
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            validate_input_text_flags("test", InputTextFlags::ELIDE_LEFT, true)
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            validate_input_text_flags("test", InputTextFlags::CALLBACK_HISTORY, true)
        })
        .is_err()
    );

    validate_input_text_flags(
        "test",
        InputTextFlags::WORD_WRAP | InputTextFlags::CALLBACK_CHAR_FILTER,
        true,
    );
    validate_input_text_flags(
        "test",
        InputTextFlags::ELIDE_LEFT | InputTextFlags::PASSWORD,
        false,
    );
}

#[test]
fn input_scalar_flags_reject_callback_and_enter_return_flags() {
    assert!(
        std::panic::catch_unwind(|| {
            validate_input_scalar_flags("test", InputTextFlags::ENTER_RETURNS_TRUE)
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            validate_input_scalar_flags("test", InputTextFlags::CALLBACK_EDIT)
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            validate_input_scalar_flags("test", InputTextFlags::CALLBACK_RESIZE)
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            validate_input_scalar_flags("test", InputTextFlags::WORD_WRAP)
        })
        .is_err()
    );

    validate_input_scalar_flags(
        "test",
        InputTextFlags::CHARS_DECIMAL
            | InputTextFlags::PARSE_EMPTY_REF_VAL
            | InputTextFlags::DISPLAY_EMPTY_REF_VAL,
    );
}

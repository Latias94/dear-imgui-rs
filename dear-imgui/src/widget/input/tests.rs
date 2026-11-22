use super::buffers::{StringInputBuffer, finish_string_input_buffer, resize_string_input_buffer};
use super::validation::{
    validate_input_multiline_flags, validate_input_scalar_flags, validate_input_text_flags,
};
use crate::{InputScalarFlags, InputTextFlags, InputTextMultilineFlags, sys};

#[test]
fn unchanged_string_input_reuses_its_allocation_across_frames() {
    let mut text = String::with_capacity(64);
    text.push_str("café");
    let allocation = text.as_ptr();
    let capacity = text.capacity();
    for _ in 0..3 {
        let input = StringInputBuffer::new(&mut text, Some(16));
        assert_eq!(input.bytes.as_ptr(), allocation);
        assert_eq!(input.bytes.len(), capacity);
        assert_eq!(&input.bytes[..6], "café\0".as_bytes());
    }
    assert_eq!(text, "café");
    assert_eq!(text.capacity(), capacity);
}

#[test]
fn growing_input_amortizes_the_capacity_hint_across_edits() {
    let mut text = String::new();
    let mut reused = false;
    for i in 0..16 {
        let capacity = text.capacity();
        {
            let mut input = StringInputBuffer::new(&mut text, Some(64));
            assert!(input.bytes.len() > i + 64);
            input.bytes[i] = b'x';
        }
        reused |= text.capacity() == capacity;
    }
    assert_eq!(text, "x".repeat(16));
    assert!(reused, "a capacity hint should amortize successive edits");
}

#[test]
fn input_widget_retains_its_labels_across_callback_scratch_queries() {
    let mut ctx = crate::Context::create();
    ctx.set_ini_filename::<std::path::PathBuf>(None).unwrap();
    ctx.io_mut().set_display_size([320.0, 240.0]);
    ctx.font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
    let ui = ctx.frame();
    let (label, _) = ui.scratch_txt_with_opt("name", Some("hint"));
    let scratch_allocation = || ui.with_scratch_buffer(|scratch| scratch.buffer.as_ptr());
    let labels = scratch_allocation();
    ui.run_input_widget(label, || {
        ui.get_id("callback item");
        assert_ne!(scratch_allocation(), labels);
    });
    assert_eq!(scratch_allocation(), labels);
}

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
fn input_flags_are_split_by_widget_domain() {
    let private_multiline =
        InputTextFlags::from_bits_retain(sys::ImGuiInputTextFlags_Multiline as i32);
    let multiline_only = InputTextFlags::from_bits_retain(sys::ImGuiInputTextFlags_WordWrap as i32);
    let single_line_only =
        InputTextMultilineFlags::from_bits_retain(sys::ImGuiInputTextFlags_ElideLeft as i32);
    let numeric_only =
        InputTextFlags::from_bits_retain(sys::ImGuiInputTextFlags_ParseEmptyRefVal as i32);
    let resize_callback =
        InputTextFlags::from_bits_retain(sys::ImGuiInputTextFlags_CallbackResize as i32);
    let callback_only =
        InputScalarFlags::from_bits_retain(sys::ImGuiInputTextFlags_CallbackEdit as i32);
    let multiline_completion = InputTextMultilineFlags::from_bits_retain(
        sys::ImGuiInputTextFlags_CallbackCompletion as i32,
    );
    let multiline_history =
        InputTextMultilineFlags::from_bits_retain(sys::ImGuiInputTextFlags_CallbackHistory as i32);
    let multiline_resize =
        InputTextMultilineFlags::from_bits_retain(sys::ImGuiInputTextFlags_CallbackResize as i32);
    let conflicting_single_line_callbacks =
        InputTextFlags::CALLBACK_COMPLETION | InputTextFlags::ALLOW_TAB_INPUT;

    assert!(
        std::panic::catch_unwind(|| validate_input_text_flags("test", private_multiline)).is_err()
    );
    assert!(
        std::panic::catch_unwind(|| validate_input_text_flags("test", multiline_only)).is_err()
    );
    assert!(std::panic::catch_unwind(|| validate_input_text_flags("test", numeric_only)).is_err());
    assert!(
        std::panic::catch_unwind(|| {
            validate_input_text_flags("test", conflicting_single_line_callbacks)
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| validate_input_text_flags("test", resize_callback)).is_err()
    );
    assert!(
        std::panic::catch_unwind(|| validate_input_multiline_flags("test", single_line_only))
            .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| validate_input_multiline_flags("test", multiline_completion))
            .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| validate_input_multiline_flags("test", multiline_history))
            .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| validate_input_multiline_flags("test", multiline_resize))
            .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| { validate_input_scalar_flags("test", callback_only) })
            .is_err()
    );

    validate_input_text_flags(
        "test",
        InputTextFlags::ELIDE_LEFT | InputTextFlags::PASSWORD | InputTextFlags::ENTER_RETURNS_TRUE,
    );
    validate_input_multiline_flags(
        "test",
        InputTextMultilineFlags::WORD_WRAP
            | InputTextMultilineFlags::CTRL_ENTER_FOR_NEW_LINE
            | InputTextMultilineFlags::ALLOW_TAB_INPUT,
    );
    validate_input_scalar_flags(
        "test",
        InputScalarFlags::CHARS_DECIMAL
            | InputScalarFlags::PARSE_EMPTY_REF_VAL
            | InputScalarFlags::DISPLAY_EMPTY_REF_VAL,
    );

    assert_eq!(
        InputTextFlags::ELIDE_LEFT.bits(),
        sys::ImGuiInputTextFlags_ElideLeft as i32
    );
    assert_eq!(
        InputTextMultilineFlags::WORD_WRAP.bits(),
        sys::ImGuiInputTextFlags_WordWrap as i32
    );
    assert_eq!(
        InputScalarFlags::PARSE_EMPTY_REF_VAL.bits(),
        sys::ImGuiInputTextFlags_ParseEmptyRefVal as i32
    );
}

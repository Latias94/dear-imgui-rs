use super::{
    HistoryDirection, InputTextCallbackHandler, PassthroughCallback, StringCallbackState,
    StringInputBuffer, TextCallbackData, finish_string_input_buffer, string_callback_router,
    validate_input_multiline_flags, validate_input_scalar_flags, validate_input_text_flags,
    with_string_input_buffer,
};
use crate::{InputScalarFlags, InputTextFlags, InputTextMultilineFlags, sys};
use std::borrow::Cow;

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
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = crate::Context::create();
    ctx.set_ini_filename::<std::path::PathBuf>(None).unwrap();
    ctx.io_mut().set_display_size([320.0, 240.0]);
    ctx.font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
    let ui = ctx.frame();
    ui.with_scratch_buffer(|scratch| {
        let (label, hint) = scratch.scratch_txt_two("name", "hint");
        ui.run_input_widget(label, || {
            ui.with_scratch_buffer(|nested| {
                let query = nested.scratch_txt("callback query".repeat(1024));
                assert_ne!(nested.buffer.as_ptr(), scratch.buffer.as_ptr());
                assert_ne!(query, label);
            });
            assert_eq!(unsafe { std::ffi::CStr::from_ptr(label) }, c"name");
            assert_eq!(unsafe { std::ffi::CStr::from_ptr(hint) }, c"hint");
        });
    });
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
fn input_flags_are_split_by_widget_domain() {
    let private_multiline = InputTextFlags::from_bits_retain(sys::ImGuiInputTextFlags_Multiline);
    let multiline_only = InputTextFlags::from_bits_retain(sys::ImGuiInputTextFlags_WordWrap);
    let single_line_only =
        InputTextMultilineFlags::from_bits_retain(sys::ImGuiInputTextFlags_ElideLeft);
    let numeric_only = InputTextFlags::from_bits_retain(sys::ImGuiInputTextFlags_ParseEmptyRefVal);
    let resize_callback = InputTextFlags::from_bits_retain(sys::ImGuiInputTextFlags_CallbackResize);
    let callback_only = InputScalarFlags::from_bits_retain(sys::ImGuiInputTextFlags_CallbackEdit);
    let multiline_completion =
        InputTextMultilineFlags::from_bits_retain(sys::ImGuiInputTextFlags_CallbackCompletion);
    let multiline_history =
        InputTextMultilineFlags::from_bits_retain(sys::ImGuiInputTextFlags_CallbackHistory);
    let multiline_resize =
        InputTextMultilineFlags::from_bits_retain(sys::ImGuiInputTextFlags_CallbackResize);
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
        sys::ImGuiInputTextFlags_ElideLeft
    );
    assert_eq!(
        InputTextMultilineFlags::WORD_WRAP.bits(),
        sys::ImGuiInputTextFlags_WordWrap
    );
    assert_eq!(
        InputScalarFlags::PARSE_EMPTY_REF_VAL.bits(),
        sys::ImGuiInputTextFlags_ParseEmptyRefVal
    );
}

#[derive(Default)]
struct RecordingHandler {
    char_filter_seen: Option<char>,
    history_seen: Option<HistoryDirection>,
}

impl InputTextCallbackHandler for RecordingHandler {
    fn char_filter(&mut self, character: char) -> Option<char> {
        self.char_filter_seen = Some(character);
        Some('z')
    }

    fn on_history(&mut self, direction: HistoryDirection, _data: TextCallbackData<'_>) {
        self.history_seen = Some(direction);
    }
}

#[test]
fn string_callback_router_dispatches_char_filter_and_history() {
    let mut buffer = b"abc\0".to_vec();
    let ptr = buffer.as_mut_ptr();
    let mut state = StringCallbackState {
        buffer: &raw mut buffer,
        handler: RecordingHandler::default(),
        panic: None,
    };
    let mut data = sys::ImGuiInputTextCallbackData {
        Buf: ptr.cast(),
        BufTextLen: 3,
        BufSize: 4,
        UserData: (&raw mut state).cast(),
        EventFlag: sys::ImGuiInputTextFlags_CallbackCharFilter,
        EventChar: 'a' as sys::ImWchar,
        ..Default::default()
    };

    assert_eq!(
        unsafe { string_callback_router::<RecordingHandler>(&mut data) },
        0
    );
    assert_eq!(state.handler.char_filter_seen, Some('a'));
    assert_eq!(data.EventChar, 'z' as sys::ImWchar);

    data.EventFlag = sys::ImGuiInputTextFlags_CallbackHistory;
    data.EventKey = sys::ImGuiKey_UpArrow;
    assert_eq!(
        unsafe { string_callback_router::<RecordingHandler>(&mut data) },
        0
    );
    assert_eq!(state.handler.history_seen, Some(HistoryDirection::Up));
}

#[test]
fn string_callback_router_skips_user_code_but_resizes_after_panic() {
    struct PanicOnCharacter(usize);

    impl InputTextCallbackHandler for PanicOnCharacter {
        fn char_filter(&mut self, _: char) -> Option<char> {
            self.0 += 1;
            panic!("character filter panicked");
        }
    }

    let mut buffer = b"abc\0".to_vec();
    let ptr = buffer.as_mut_ptr();
    let mut state = StringCallbackState {
        buffer: &raw mut buffer,
        handler: PanicOnCharacter(0),
        panic: None,
    };
    let mut data = sys::ImGuiInputTextCallbackData {
        Buf: ptr.cast(),
        BufTextLen: 3,
        BufSize: 4,
        UserData: (&raw mut state).cast(),
        EventFlag: sys::ImGuiInputTextFlags_CallbackCharFilter,
        EventChar: 'a' as sys::ImWchar,
        ..Default::default()
    };

    assert_eq!(
        unsafe { string_callback_router::<PanicOnCharacter>(&mut data) },
        1
    );
    assert_eq!(
        unsafe { string_callback_router::<PanicOnCharacter>(&mut data) },
        1
    );
    assert_eq!(state.handler.0, 1);

    data.EventFlag = sys::ImGuiInputTextFlags_CallbackResize;
    data.BufSize = 32;
    assert_eq!(
        unsafe { string_callback_router::<PanicOnCharacter>(&mut data) },
        0
    );
    assert_eq!(buffer.len(), 32);
    assert_eq!(data.Buf, buffer.as_mut_ptr().cast());
    assert_eq!(
        state.panic.unwrap().downcast_ref::<&str>().copied(),
        Some("character filter panicked")
    );
}

#[test]
fn invalid_resize_requests_keep_the_original_buffer() {
    let mut text = String::from("abc");
    with_string_input_buffer(
        &mut text,
        None,
        PassthroughCallback,
        |original, size, callback, userdata| {
            for requested in [-1, i32::MAX] {
                let mut data = sys::ImGuiInputTextCallbackData {
                    Buf: original,
                    BufTextLen: 3,
                    BufSize: requested,
                    UserData: userdata,
                    EventFlag: sys::ImGuiInputTextFlags_CallbackResize,
                    ..Default::default()
                };
                assert_eq!(unsafe { callback.unwrap()(&mut data) }, 0);
                assert_eq!(data.Buf, original);
                assert_eq!(data.BufSize, i32::try_from(size).unwrap());
                assert!(!data.BufDirty);
            }
            false
        },
    );
    assert_eq!(text, "abc");
}

struct DefaultHandler;
impl InputTextCallbackHandler for DefaultHandler {}

fn callback_data_for_text(text: &mut [u8]) -> sys::ImGuiInputTextCallbackData {
    let buffer_size = text.len();
    let text_len = buffer_size.saturating_sub(1) as i32;
    sys::ImGuiInputTextCallbackData {
        Buf: text.as_mut_ptr().cast(),
        BufTextLen: text_len,
        BufSize: buffer_size as i32,
        CursorPos: text_len,
        SelectionStart: 0,
        SelectionEnd: text_len,
        ..Default::default()
    }
}

#[test]
fn default_char_filter_keeps_character() {
    let mut handler = DefaultHandler;
    assert_eq!(handler.char_filter('x'), Some('x'));
}

#[test]
fn passthrough_char_filter_keeps_character() {
    let mut handler = PassthroughCallback;
    assert_eq!(handler.char_filter('x'), Some('x'));
}

#[test]
fn editable_callback_reads_native_text_before_caller_resize() {
    let mut text = *b"edited text\0";
    let mut data = callback_data_for_text(&mut text);
    let mut ctx = sys::ImGuiContext::default();
    ctx.InputTextState.TextA.Data = data.Buf;
    ctx.InputTextState.TextA.Size = data.BufSize;
    ctx.InputTextState.TextA.Capacity = data.BufSize;
    data.Ctx = &raw mut ctx;
    data.Flags = sys::ImGuiInputTextFlags_CallbackResize;
    data.BufSize = 4;

    let info = unsafe { TextCallbackData::new(&mut data) };
    assert_eq!(info.str(), Ok("edited text"));
    assert!(matches!(info.str_lossy(), Cow::Borrowed("edited text")));
}

#[test]
fn callback_text_exposes_invalid_utf8_without_panicking() {
    let mut text = [b'a', 0xff, b'b', 0];
    let mut data = callback_data_for_text(&mut text);
    let info = unsafe { TextCallbackData::new(&mut data) };

    assert_eq!(info.str().unwrap_err().valid_up_to(), 1);
    assert_eq!(info.str_lossy(), "a\u{fffd}b");
}

#[test]
#[should_panic(expected = "callback text must fit its buffer with a trailing NUL")]
fn callback_text_rejects_a_length_outside_its_buffer() {
    let mut text = *b"abc\0";
    let mut data = callback_data_for_text(&mut text);
    data.BufTextLen = data.BufSize;
    let info = unsafe { TextCallbackData::new(&mut data) };

    let _ = info.str();
}

#[test]
fn read_only_callback_limits_insertions_to_complete_chars_and_its_terminator() {
    let mut text = *b"ab\0\0\0\0";
    let mut data = callback_data_for_text(&mut text);
    data.BufTextLen = 2;
    data.Flags = sys::ImGuiInputTextFlags_CallbackResize | sys::ImGuiInputTextFlags_ReadOnly;
    let info = unsafe { TextCallbackData::new(&mut data) };
    let len = info.str().unwrap().len();
    assert_eq!(info.insertion_prefix("éé", len), "é");
    assert_eq!(info.insertion_prefix("a\0b", len), "a");
}

#[test]
fn text_callback_positions_accept_bounds() {
    let mut text = *b"abcd\0";
    let mut data = callback_data_for_text(&mut text);
    data.CursorPos = 4;
    data.SelectionStart = 1;
    data.SelectionEnd = 3;

    let info = unsafe { TextCallbackData::new(&mut data) };

    assert_eq!(info.cursor_pos(), 4);
    assert_eq!(info.selection_start(), 1);
    assert_eq!(info.selection_end(), 3);
    assert_eq!(info.selected(), "bc");
}

#[test]
#[should_panic(expected = "CursorPos exceeded BufTextLen")]
fn text_callback_cursor_pos_rejects_out_of_bounds() {
    let mut text = *b"abcd\0";
    let mut data = callback_data_for_text(&mut text);
    data.CursorPos = 5;

    let info = unsafe { TextCallbackData::new(&mut data) };
    let _ = info.cursor_pos();
}

#[test]
#[should_panic(expected = "CursorPos exceeded BufTextLen")]
fn text_callback_clear_selection_rejects_out_of_bounds_cursor() {
    let mut text = *b"abcd\0";
    let mut data = callback_data_for_text(&mut text);
    data.CursorPos = 5;

    let mut info = unsafe { TextCallbackData::new(&mut data) };
    info.clear_selection();
}

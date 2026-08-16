use dear_imgui_cte::{CteError, Position, SearchOptions, Selection, TextEditor};
use dear_imgui_rs::Context;

#[test]
fn empty_and_unicode_text_round_trip_as_owned_strings() {
    let context = Context::create();
    let mut editor = TextEditor::create(&context);

    editor.set_text("").unwrap();
    assert_eq!(editor.text().unwrap(), "");

    editor.set_text("你好, αβ🙂\n第二行").unwrap();
    let complete = editor.text().unwrap();
    let first_line = editor.line_text(0).unwrap();
    let prefix = editor
        .section_text(Selection::new(Position::new(0, 0), Position::new(0, 2)))
        .unwrap();
    assert_eq!(complete, "你好, αβ🙂\n第二行");
    assert_eq!(first_line, "你好, αβ🙂");
    assert_eq!(prefix, "你好");

    editor.set_text("replacement").unwrap();
    assert_eq!(complete, "你好, αβ🙂\n第二行");
    assert_eq!(first_line, "你好, αβ🙂");
}

#[test]
fn repeated_allocated_and_static_getters_do_not_invalidate_prior_results() {
    let context = Context::create();
    let mut editor = TextEditor::create(&context);
    editor.set_text("first\nsecond").unwrap();

    let complete_a = editor.text().unwrap();
    let line_a = editor.line_text(0).unwrap();
    let complete_b = editor.text().unwrap();
    let line_b = editor.line_text(1).unwrap();

    assert_eq!(complete_a, "first\nsecond");
    assert_eq!(complete_b, complete_a);
    assert_eq!(line_a, "first");
    assert_eq!(line_b, "second");
}

#[test]
fn invalid_strings_and_indices_fail_before_the_target_operation() {
    let context = Context::create();
    let mut editor = TextEditor::create(&context);
    editor.set_text("αβ\nline").unwrap();

    assert!(matches!(
        editor.set_text("bad\0text"),
        Err(CteError::InteriorNul { .. })
    ));
    assert!(matches!(
        editor.select_first_occurrence("bad\0query", SearchOptions::default()),
        Err(CteError::InteriorNul { .. })
    ));
    assert!(matches!(
        editor.line_text(99),
        Err(CteError::LineOutOfBounds { .. })
    ));
    assert!(matches!(
        editor.set_cursor(Position::new(0, 3)),
        Err(CteError::ColumnOutOfBounds { .. })
    ));
    assert!(matches!(
        editor.cursor_text(99),
        Err(CteError::CursorOutOfBounds { .. })
    ));
    assert!(matches!(
        editor.section_text(Selection::new(Position::new(1, 1), Position::new(0, 1))),
        Err(CteError::ReversedSelection { .. })
    ));

    assert_eq!(editor.text().unwrap(), "αβ\nline");
}

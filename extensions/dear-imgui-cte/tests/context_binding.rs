use dear_imgui_cte::{CteError, CteUiExt, Palette, Position, TextDiff, TextEditor};
use dear_imgui_rs::{Context, sys};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn prepare_frame(context: &mut Context) {
    context.io_mut().set_display_size([640.0, 480.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("headless CTE tests require the legacy font-atlas capability")
        .build();
}

#[test]
fn render_rejects_a_ui_from_another_context_before_ffi() {
    let context_a = Context::create();
    let mut editor = TextEditor::create(&context_a);

    let mut context_b = Context::create();
    prepare_frame(&mut context_b);
    let ui = context_b.frame();
    let error = ui
        .text_editor(&mut editor, "wrong context")
        .build()
        .unwrap_err();
    assert!(matches!(error, CteError::WrongContext { .. }));
    assert!(matches!(
        editor.document_to_screen(ui, Position::default()),
        Err(CteError::WrongContext { .. })
    ));
    drop(context_b.render_legacy());

    drop(context_b);
    drop(editor);
    drop(context_a);
}

#[test]
fn auxiliary_renderers_reject_a_ui_from_another_context_before_ffi() {
    let context_a = Context::create();
    let mut diff = TextDiff::create(&context_a);

    let mut context_b = Context::create();
    prepare_frame(&mut context_b);
    let ui = context_b.frame();
    assert!(matches!(
        ui.text_diff(&mut diff, "wrong context").build(),
        Err(CteError::WrongContext { .. })
    ));
    drop(context_b.render_legacy());

    drop(context_b);
    drop(diff);
    drop(context_a);
}

#[test]
fn editor_calls_restore_the_previous_current_context() {
    let context_a = Context::create();
    let mut editor = TextEditor::create(&context_a);
    editor.set_text("bound to A").unwrap();

    let context_b = Context::create();
    let raw_b = context_b.as_raw();
    assert_eq!(unsafe { sys::igGetCurrentContext() }, raw_b);
    assert_eq!(editor.text().unwrap(), "bound to A");
    assert_eq!(unsafe { sys::igGetCurrentContext() }, raw_b);

    drop(context_b);
    drop(editor);
    drop(context_a);
}

#[test]
fn calls_fail_closed_after_the_owner_context_is_destroyed() {
    let context = Context::create();
    let mut editor = TextEditor::create(&context);
    let mut diff = TextDiff::create(&context);
    drop(context);

    assert!(matches!(
        editor.text(),
        Err(CteError::ContextBinding { .. })
    ));
    assert!(matches!(
        editor.set_text("unused"),
        Err(CteError::ContextBinding { .. })
    ));
    assert!(matches!(
        editor.filter_lines(|_| panic!("a dead editor must not invoke the filter")),
        Err(CteError::ContextBinding { .. })
    ));
    assert!(matches!(
        editor.position_at_mouse([0.0, 0.0]),
        Err(CteError::ContextBinding { .. })
    ));
    assert!(matches!(
        diff.set_palette(&Palette::dark()),
        Err(CteError::ContextBinding { .. })
    ));

    let result = catch_unwind(AssertUnwindSafe(|| editor.line_count()));
    assert!(result.is_err());

    drop(editor);
}

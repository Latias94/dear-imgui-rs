use dear_imgui_cte::{CteError, CteUiExt, TextEditor};
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
    let suspended_a = context_a.suspend_or_panic();

    let mut context_b = Context::create();
    prepare_frame(&mut context_b);
    let ui = context_b.frame();
    let error = ui
        .text_editor(&mut editor, "wrong context")
        .build()
        .unwrap_err();
    assert!(matches!(error, CteError::WrongContext { .. }));
    drop(context_b.render_legacy());

    drop(context_b);
    let context_a = suspended_a.activate_or_panic();
    drop(editor);
    drop(context_a);
}

#[test]
fn editor_calls_restore_the_previous_current_context() {
    let context_a = Context::create();
    let mut editor = TextEditor::create(&context_a);
    editor.set_text("bound to A").unwrap();
    let suspended_a = context_a.suspend_or_panic();

    let context_b = Context::create();
    let raw_b = context_b.as_raw();
    assert_eq!(unsafe { sys::igGetCurrentContext() }, raw_b);
    assert_eq!(editor.text().unwrap(), "bound to A");
    assert_eq!(unsafe { sys::igGetCurrentContext() }, raw_b);

    drop(context_b);
    let context_a = suspended_a.activate_or_panic();
    drop(editor);
    drop(context_a);
}

#[test]
fn calls_fail_closed_after_the_owner_context_is_destroyed() {
    let context = Context::create();
    let editor = TextEditor::create(&context);
    drop(context);

    let result = catch_unwind(AssertUnwindSafe(|| editor.line_count()));
    assert!(result.is_err());

    // Drop intentionally leaks the native handle once the owner context is dead.
    drop(editor);
}

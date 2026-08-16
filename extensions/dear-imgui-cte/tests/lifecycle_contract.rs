use dear_imgui_cte::TextEditor;
use dear_imgui_rs::{Context, sys};

#[test]
fn normal_drop_binds_the_owner_and_restores_the_previous_context() {
    let context_a = Context::create();
    let editor = TextEditor::create(&context_a);
    let suspended_a = context_a.suspend_or_panic();

    let context_b = Context::create();
    let raw_b = context_b.as_raw();
    drop(editor);
    assert_eq!(unsafe { sys::igGetCurrentContext() }, raw_b);

    drop(context_b);
    drop(suspended_a.activate_or_panic());
}

#[test]
fn dead_owner_drop_does_not_switch_or_touch_another_context() {
    let context_a = Context::create();
    let editor = TextEditor::create(&context_a);
    let suspended_a = context_a.suspend_or_panic();
    drop(suspended_a);

    let context_b = Context::create();
    let raw_b = context_b.as_raw();
    drop(editor);
    assert_eq!(unsafe { sys::igGetCurrentContext() }, raw_b);
    drop(context_b);
}

#[test]
fn multiple_editors_have_independent_single_owner_lifecycles() {
    let context = Context::create();
    let first = TextEditor::create(&context);
    let second = TextEditor::create(&context);
    assert_ne!(unsafe { first.as_raw() }, unsafe { second.as_raw() });
    drop(first);
    drop(second);
    drop(context);
}

use dear_imgui_cte::{Language, TextEditor};
use dear_imgui_cte_sys as cte_sys;
use dear_imgui_rs::{Context, sys};
use std::{cell::Cell, rc::Rc};

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

struct DropToken(Rc<Cell<usize>>);

impl Drop for DropToken {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

struct NativeClearProbe {
    editor: *mut cte_sys::TextEditor,
    observed: Rc<Cell<Option<bool>>>,
}

impl Drop for NativeClearProbe {
    fn drop(&mut self) {
        self.observed.set(Some(!unsafe {
            cte_sys::TextEditor_HasLineDecorator(self.editor)
        }));
    }
}

#[test]
fn normal_drop_clears_native_callbacks_before_releasing_rust_state() {
    let context = Context::create();
    let mut editor = TextEditor::create(&context);
    let observed = Rc::new(Cell::new(None));
    let probe = NativeClearProbe {
        editor: unsafe { editor.as_raw() },
        observed: Rc::clone(&observed),
    };
    editor
        .set_line_decorator(1, move |_, _| {
            let _ = &probe;
        })
        .unwrap();

    drop(editor);
    assert_eq!(observed.get(), Some(true));
}

#[test]
fn dead_owner_drop_releases_rust_callbacks_without_touching_native_state() {
    let context = Context::create();
    let mut editor = TextEditor::create(&context);
    let drops = Rc::new(Cell::new(0));
    let token = DropToken(Rc::clone(&drops));
    editor
        .set_language_change_callback(move || {
            let _ = &token;
        })
        .unwrap();
    drop(context);

    drop(editor);
    assert_eq!(drops.get(), 1);
}

#[test]
fn dead_owner_drop_leaks_connected_trie_instead_of_dereferencing_its_editor() {
    let context = Context::create();
    let mut editor = TextEditor::create(&context);
    editor.set_language(Some(Language::Cpp));
    editor.enable_trie_autocomplete().unwrap();
    drop(context);

    drop(editor);
}

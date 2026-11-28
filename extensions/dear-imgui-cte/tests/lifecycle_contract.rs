use dear_imgui_cte::{Language, TextEditor};
use dear_imgui_cte_sys as cte_sys;
use dear_imgui_rs::{Context, sys};
use std::{cell::Cell, rc::Rc};

#[test]
fn normal_drop_binds_the_owner_and_restores_the_previous_context() {
    let context_a = Context::create();
    let editor = TextEditor::create(&context_a);

    let context_b = Context::create();
    let raw_b = context_b.as_raw();
    drop(editor);
    assert_eq!(unsafe { sys::igGetCurrentContext() }, raw_b);

    drop(context_b);
    drop(context_a);
}

#[test]
fn dead_owner_drop_preserves_another_current_context() {
    let context_a = Context::create();
    let editor = TextEditor::create(&context_a);
    drop(context_a);

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
fn drop_clears_native_callbacks_before_releasing_rust_state_in_either_owner_order() {
    for owner_first in [false, true] {
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
        if owner_first {
            drop(context);
        }
        drop(editor);
        assert_eq!(observed.get(), Some(true));
    }
}

#[test]
fn cte_owners_keep_allocator_thread_until_the_last_native_drop() {
    let context = Context::create();
    let diff = dear_imgui_cte::TextDiff::create(&context);
    let mut editor = TextEditor::create(&context);
    editor.set_language(Some(Language::Cpp));
    editor.enable_trie_autocomplete().unwrap();
    drop(context);

    let another_thread_can_acquire = || {
        std::thread::spawn(|| dear_imgui_rs::ContextThreadLease::acquire().is_ok())
            .join()
            .unwrap()
    };
    assert!(!another_thread_can_acquire());
    drop(editor);
    assert!(!another_thread_can_acquire());
    drop(diff);
    assert!(another_thread_can_acquire());
}

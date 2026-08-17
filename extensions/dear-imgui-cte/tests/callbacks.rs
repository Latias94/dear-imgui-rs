use dear_imgui_cte::{
    AutocompleteConfig, CteError, CteUiExt, Language, Position, Selection, TextChangeKind,
    TextEditor,
};
use dear_imgui_cte_sys as sys;
use dear_imgui_rs::{Context, FramePrepareOptions};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

fn render_context() -> Context {
    let mut context = Context::create();
    context.prepare_frame(FramePrepareOptions::new([640.0, 480.0], 1.0 / 60.0));
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("headless CTE tests require the legacy font-atlas capability")
        .build();
    context
}

fn render_editor(context: &mut Context, editor: &mut TextEditor, title: &str) {
    context.prepare_frame(FramePrepareOptions::new([640.0, 480.0], 1.0 / 60.0));
    let ui = context.frame();
    ui.text_editor(editor, title)
        .size([500.0, 280.0])
        .build()
        .unwrap();
    drop(context.render_legacy());
}

struct DropToken(Rc<Cell<usize>>);

impl Drop for DropToken {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn persistent_callbacks_are_isolated_replaced_and_dropped_once() {
    let context = Context::create();
    let mut first = TextEditor::create(&context);
    let mut second = TextEditor::create(&context);
    let first_calls = Rc::new(Cell::new(0));
    let second_calls = Rc::new(Cell::new(0));

    let calls = Rc::clone(&first_calls);
    first
        .set_language_change_callback(move || calls.set(calls.get() + 1))
        .unwrap();
    let calls = Rc::clone(&second_calls);
    second
        .set_language_change_callback(move || calls.set(calls.get() + 1))
        .unwrap();

    first.set_language(Some(Language::Cpp));
    assert_eq!(first_calls.get(), 1);
    assert_eq!(second_calls.get(), 0);
    second.set_language(Some(Language::Python));
    assert_eq!(first_calls.get(), 1);
    assert_eq!(second_calls.get(), 1);

    let drops = Rc::new(Cell::new(0));
    let token = DropToken(Rc::clone(&drops));
    first
        .set_language_change_callback(move || {
            let _ = &token;
        })
        .unwrap();
    assert_eq!(drops.get(), 0);

    let token = DropToken(Rc::clone(&drops));
    first
        .set_language_change_callback(move || {
            let _ = &token;
        })
        .unwrap();
    assert_eq!(drops.get(), 1);
    first.clear_language_change_callback().unwrap();
    assert_eq!(drops.get(), 2);

    let token = DropToken(Rc::clone(&drops));
    first
        .set_language_change_callback(move || {
            let _ = &token;
        })
        .unwrap();
    drop(first);
    assert_eq!(drops.get(), 3);
}

#[test]
fn one_editor_callback_can_use_an_independent_editor() {
    let context = Context::create();
    let mut source = TextEditor::create(&context);
    let target = Rc::new(RefCell::new(TextEditor::create(&context)));
    let target_for_callback = Rc::clone(&target);
    source
        .set_language_change_callback(move || {
            target_for_callback
                .borrow_mut()
                .set_text("updated from callback")
                .unwrap();
        })
        .unwrap();

    source.set_language(Some(Language::Cpp));
    assert_eq!(target.borrow().text().unwrap(), "updated from callback");
}

#[test]
fn transaction_change_and_render_callbacks_route_typed_events() {
    let mut context = render_context();
    let mut editor = TextEditor::create(&context);
    editor.set_text("alpha\nbeta").unwrap();

    let changes = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&changes);
    editor
        .set_transaction_callback(move |change| {
            recorded
                .borrow_mut()
                .push((change.kind, change.range, change.text.to_owned()));
        })
        .unwrap();
    editor
        .replace_section(
            Selection::new(Position::new(0, 5), Position::new(0, 5)),
            "!",
        )
        .unwrap();
    assert!(
        changes
            .borrow()
            .iter()
            .any(|(kind, _, text)| { *kind == TextChangeKind::Insert && text == "!" })
    );

    let change_calls = Rc::new(Cell::new(0));
    let calls = Rc::clone(&change_calls);
    editor
        .set_change_callback(Duration::ZERO, move || calls.set(calls.get() + 1))
        .unwrap();

    let decorator_calls = Rc::new(Cell::new(0));
    let calls = Rc::clone(&decorator_calls);
    editor
        .set_line_decorator(1, move |ui, event| {
            let _ = ui.context_id();
            assert!(event.glyph_size.into_iter().all(f32::is_finite));
            calls.set(calls.get() + 1);
        })
        .unwrap();

    let caret_calls = Rc::new(Cell::new(0));
    let calls = Rc::clone(&caret_calls);
    editor
        .set_custom_caret_callback(move |ui, event| {
            let _ = ui.context_id();
            assert!(event.glyph_position.into_iter().all(f32::is_finite));
            calls.set(calls.get() + 1);
        })
        .unwrap();

    editor.focus();
    for frame in 0..4 {
        render_editor(&mut context, &mut editor, &format!("callbacks##{frame}"));
        std::thread::sleep(Duration::from_millis(1));
    }

    assert!(change_calls.get() >= 1);
    assert!(decorator_calls.get() >= 1);
    assert!(caret_calls.get() >= 1);
}

#[test]
fn length_aware_identifier_and_filter_callbacks_preserve_utf8() {
    let mut context = render_context();
    let mut editor = TextEditor::create(&context);
    editor.set_text("int variable_name = 1;\n你好").unwrap();
    editor.set_language(Some(Language::Cpp));
    render_editor(&mut context, &mut editor, "identifiers");

    let mut identifiers = Vec::new();
    editor
        .for_each_identifier(|identifier| identifiers.push(identifier.to_owned()))
        .unwrap();
    assert!(identifiers.iter().any(|value| value == "variable_name"));

    editor.filter_lines(|line| format!("{line}!")).unwrap();
    assert_eq!(editor.text().unwrap(), "int variable_name = 1;!\n你好!");

    editor
        .select_region(Selection::new(Position::new(0, 0), Position::new(0, 3)))
        .unwrap();
    editor
        .filter_selections(|selection| selection.to_uppercase())
        .unwrap();
    assert!(editor.text().unwrap().starts_with("INT variable_name"));

    let before = editor.text().unwrap();
    assert!(matches!(
        editor.filter_lines(|_| "invalid\0output".to_owned()),
        Err(CteError::InteriorNul { .. })
    ));
    assert_eq!(editor.text().unwrap(), before);
}

#[test]
fn filters_validate_the_complete_batch_before_editing() {
    let context = Context::create();
    let mut editor = TextEditor::create(&context);
    editor.set_text("first\nsecond").unwrap();

    let calls = Cell::new(0);
    let result = editor.filter_lines(|line| {
        calls.set(calls.get() + 1);
        if line == "second" {
            "invalid\0output".to_owned()
        } else {
            format!("{line}!")
        }
    });
    assert!(matches!(result, Err(CteError::InteriorNul { .. })));
    assert_eq!(calls.get(), 2);
    assert_eq!(editor.text().unwrap(), "first\nsecond");

    editor
        .select_region(Selection::new(Position::new(0, 0), Position::new(1, 6)))
        .unwrap();
    assert!(matches!(
        editor.filter_selections(|selection| format!("{selection}!")),
        Err(CteError::InvalidValue { .. })
    ));
    assert_eq!(editor.text().unwrap(), "first\nsecond");
}

#[test]
fn invalid_callback_setup_drops_only_the_rejected_closure() {
    let mut context = render_context();
    let mut editor = TextEditor::create(&context);
    let old_calls = Rc::new(Cell::new(0));
    let calls = Rc::clone(&old_calls);
    editor
        .set_change_callback(Duration::ZERO, move || calls.set(calls.get() + 1))
        .unwrap();

    let rejected_drops = Rc::new(Cell::new(0));
    let token = DropToken(Rc::clone(&rejected_drops));
    let result =
        editor.set_change_callback(Duration::from_millis(i32::MAX as u64 + 1), move || {
            let _ = &token;
        });
    assert!(matches!(result, Err(CteError::InvalidValue { .. })));
    assert_eq!(rejected_drops.get(), 1);

    editor.set_text("changed").unwrap();
    for _ in 0..4 {
        render_editor(&mut context, &mut editor, "invalid-callback-setup");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(old_calls.get() >= 1);

    editor.clear_change_callback().unwrap();
}

#[test]
fn clear_callbacks_releases_every_registered_family_once() {
    let context = Context::create();
    let mut editor = TextEditor::create(&context);
    let drops = Rc::new(Cell::new(0));
    let language_calls = Rc::new(Cell::new(0));

    macro_rules! token {
        () => {
            DropToken(Rc::clone(&drops))
        };
    }

    let token = token!();
    editor
        .set_change_callback(Duration::ZERO, move || {
            let _ = &token;
        })
        .unwrap();
    let token = token!();
    editor
        .set_transaction_callback(move |_| {
            let _ = &token;
        })
        .unwrap();
    let token = token!();
    editor
        .set_line_decorator(1, move |_, _| {
            let _ = &token;
        })
        .unwrap();
    let token = token!();
    editor
        .set_custom_caret_callback(move |_, _| {
            let _ = &token;
        })
        .unwrap();
    let token = token!();
    editor
        .set_line_number_context_callback(move |_, _| {
            let _ = &token;
        })
        .unwrap();
    let token = token!();
    editor
        .set_text_context_callback(move |_, _| {
            let _ = &token;
        })
        .unwrap();
    let token = token!();
    editor
        .set_text_hover_callback(move |_, _| {
            let _ = &token;
        })
        .unwrap();
    let token = token!();
    let calls = Rc::clone(&language_calls);
    editor
        .set_language_change_callback(move || {
            let _ = &token;
            calls.set(calls.get() + 1);
        })
        .unwrap();
    let token = token!();
    editor
        .set_autocomplete(&AutocompleteConfig::new(), move |_| {
            let _ = &token;
        })
        .unwrap();

    let raw = unsafe { editor.as_raw() };
    assert!(unsafe { sys::TextEditor_HasLineDecorator(raw) });
    assert!(unsafe { sys::TextEditor_HasCustomCaretRenderer(raw) });
    assert!(unsafe { sys::TextEditor_HasLineNumberContextMenuCallback(raw) });
    assert!(unsafe { sys::TextEditor_HasTextContextMenuCallback(raw) });
    assert!(unsafe { sys::TextEditor_HasTextHoverCallback(raw) });

    editor.clear_callbacks().unwrap();
    assert_eq!(drops.get(), 9);
    assert!(!unsafe { sys::TextEditor_HasLineDecorator(raw) });
    assert!(!unsafe { sys::TextEditor_HasCustomCaretRenderer(raw) });
    assert!(!unsafe { sys::TextEditor_HasLineNumberContextMenuCallback(raw) });
    assert!(!unsafe { sys::TextEditor_HasTextContextMenuCallback(raw) });
    assert!(!unsafe { sys::TextEditor_HasTextHoverCallback(raw) });
    editor.set_language(Some(Language::Cpp));
    assert_eq!(language_calls.get(), 0);
    editor.clear_callbacks().unwrap();
    drop(editor);
    assert_eq!(drops.get(), 9);
}

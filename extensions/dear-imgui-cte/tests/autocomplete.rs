use dear_imgui_cte::{AutocompleteConfig, CteError, CteUiExt, Language, Position, TextEditor};
use dear_imgui_rs::{Context, FramePrepareOptions, Key, KeyChord, KeyMods};
use std::{cell::Cell, rc::Rc, time::Duration};

fn context() -> Context {
    let mut context = Context::create();
    context.prepare_frame(FramePrepareOptions::new([640.0, 480.0], 1.0 / 60.0));
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("headless CTE tests require the legacy font-atlas capability")
        .build();
    context
}

fn render(context: &mut Context, editor: &mut TextEditor, _frame: usize) {
    context.prepare_frame(FramePrepareOptions::new([640.0, 480.0], 1.0 / 60.0));
    let ui = context.frame();
    ui.text_editor(editor, "autocomplete")
        .size([500.0, 280.0])
        .build()
        .unwrap();
    drop(context.render_legacy());
}

fn request_ctrl_space(context: &mut Context, editor: &mut TextEditor, frame: usize) {
    context.io_mut().add_focus_event(true);
    context.io_mut().add_key_event(Key::ModCtrl, true);
    context.io_mut().add_key_event(Key::Space, true);
    render(context, editor, frame);
    context.io_mut().add_key_event(Key::Space, false);
    context.io_mut().add_key_event(Key::ModCtrl, false);
}

struct DropToken(Rc<Cell<usize>>);

impl Drop for DropToken {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn autocomplete_configuration_owns_replaces_and_clears_callbacks() {
    let context = Context::create();
    let mut editor = TextEditor::create(&context);
    let drops = Rc::new(Cell::new(0));
    let config = AutocompleteConfig::new()
        .trigger_on_typing(false)
        .trigger_on_shortcut(true)
        .trigger_in_comments(true)
        .trigger_in_strings(true)
        .auto_insert_single_suggestion(true)
        .trigger_delay(Duration::ZERO)
        .no_suggestions_label("Nothing found")
        .suggestion_width(40);

    let token = DropToken(Rc::clone(&drops));
    editor
        .set_autocomplete(&config, move |request| {
            let _ = &token;
            request.set_suggestions(["alpha", "你好"]).unwrap();
        })
        .unwrap();
    assert_eq!(drops.get(), 0);

    let token = DropToken(Rc::clone(&drops));
    editor
        .set_autocomplete(&config, move |_request| {
            let _ = &token;
        })
        .unwrap();
    assert_eq!(drops.get(), 1);
    editor.clear_autocomplete().unwrap();
    assert_eq!(drops.get(), 2);

    editor
        .set_autocomplete(&AutocompleteConfig::new().suggestion_width(0), |_| {})
        .unwrap();
    editor.clear_autocomplete().unwrap();
    assert!(matches!(
        editor.set_autocomplete(
            &AutocompleteConfig::new().no_suggestions_label("bad\0label"),
            |_| {}
        ),
        Err(CteError::InteriorNul { .. })
    ));
    assert!(matches!(
        editor.set_autocomplete_suggestions(["valid", "bad\0suggestion"]),
        Err(CteError::InteriorNul { .. })
    ));
    assert!(matches!(
        editor.set_autocomplete(
            &AutocompleteConfig::new().trigger_delay(Duration::from_millis(i64::MAX as u64 + 1)),
            |_| {}
        ),
        Err(CteError::InvalidValue { .. })
    ));
    editor
        .set_autocomplete_suggestions(["alpha", "你好", ""])
        .unwrap();
}

#[test]
fn trie_attachment_is_exclusive_replaceable_and_editor_owned() {
    let context = Context::create();
    let mut editor = TextEditor::create(&context);
    editor.set_text("alpha alphabet beta").unwrap();
    editor.set_language(Some(Language::Cpp));

    editor.set_change_callback(Duration::ZERO, || {}).unwrap();
    assert!(matches!(
        editor.enable_trie_autocomplete(),
        Err(CteError::CallbackConflict { .. })
    ));
    editor.clear_change_callback().unwrap();

    editor.enable_trie_autocomplete().unwrap();
    assert!(editor.trie_autocomplete().unwrap().is_connected().unwrap());
    assert!(matches!(
        editor.set_language_change_callback(|| {}),
        Err(CteError::CallbackConflict { .. })
    ));
    assert!(matches!(
        editor.set_autocomplete(&AutocompleteConfig::new(), |_| {}),
        Err(CteError::CallbackConflict { .. })
    ));

    editor.enable_trie_autocomplete().unwrap();
    assert!(editor.trie_autocomplete().unwrap().is_connected().unwrap());
    editor.disable_trie_autocomplete().unwrap();
    assert!(editor.trie_autocomplete().is_none());

    editor.set_language_change_callback(|| {}).unwrap();
    editor.clear_callbacks().unwrap();
    editor.enable_trie_autocomplete().unwrap();
    editor.clear_callbacks().unwrap();
    assert!(editor.trie_autocomplete().is_none());
}

#[test]
fn configured_autocomplete_callback_runs_from_the_render_loop() {
    let mut context = context();
    let mut editor = TextEditor::create(&context);
    editor.set_text("al").unwrap();
    editor.set_cursor(Position::new(0, 2)).unwrap();
    editor.set_language(Some(Language::Cpp));
    let calls = Rc::new(Cell::new(0));
    let callback_calls = Rc::clone(&calls);
    editor
        .set_autocomplete(
            &AutocompleteConfig::new()
                .trigger_on_typing(false)
                .trigger_delay(Duration::ZERO),
            move |request| {
                let call = callback_calls.get();
                callback_calls.set(call + 1);
                if call == 0 {
                    assert_eq!(request.search_term().unwrap(), "al");
                    assert_eq!(request.range().unwrap().start, Position::new(0, 0));
                    assert_eq!(request.context().unwrap().language, Some(Language::Cpp));
                }
                request.set_suggestions(["alpha", "alpine"]).unwrap();
            },
        )
        .unwrap();

    render(&mut context, &mut editor, 0);
    editor.focus();
    render(&mut context, &mut editor, 1);

    context.io_mut().add_focus_event(true);
    context.io_mut().add_key_event(Key::ModCtrl, true);
    context.io_mut().add_key_event(Key::Space, true);
    render(&mut context, &mut editor, 2);
    context.io_mut().add_key_event(Key::Space, false);
    context.io_mut().add_key_event(Key::ModCtrl, false);
    for frame in 3..7 {
        std::thread::sleep(Duration::from_millis(1));
        render(&mut context, &mut editor, frame);
    }

    assert!(calls.get() >= 1);
    context.io_mut().add_key_event(Key::Enter, true);
    render(&mut context, &mut editor, 7);
    context.io_mut().add_key_event(Key::Enter, false);
    render(&mut context, &mut editor, 8);
    assert_eq!(editor.text().unwrap(), "alpha");
}

#[test]
fn replacing_autocomplete_cancels_queued_activation() {
    let mut context = context();
    let mut editor = TextEditor::create(&context);
    editor.set_text("al").unwrap();
    editor.set_cursor(Position::new(0, 2)).unwrap();
    editor.set_language(Some(Language::Cpp));

    let config = AutocompleteConfig::new()
        .trigger_on_typing(false)
        .shortcut(KeyChord::new(Key::Space).with_mods(KeyMods::CTRL))
        .trigger_delay(Duration::from_millis(30));

    let old_calls = Rc::new(Cell::new(0));
    let calls = Rc::clone(&old_calls);
    editor
        .set_autocomplete(&config, move |request| {
            calls.set(calls.get() + 1);
            request.set_suggestions(["alpha"]).unwrap();
        })
        .unwrap();

    render(&mut context, &mut editor, 0);
    editor.focus();
    render(&mut context, &mut editor, 1);
    request_ctrl_space(&mut context, &mut editor, 2);

    let new_calls = Rc::new(Cell::new(0));
    let calls = Rc::clone(&new_calls);
    editor
        .set_autocomplete(&config, move |request| {
            calls.set(calls.get() + 1);
            request.set_suggestions(["alpine"]).unwrap();
        })
        .unwrap();

    std::thread::sleep(Duration::from_millis(40));
    render(&mut context, &mut editor, 3);

    assert_eq!(old_calls.get(), 0);
    assert_eq!(new_calls.get(), 0);
}

#[test]
fn trie_suggestions_can_be_accepted_and_disconnect_cleanly() {
    let mut context = context();
    let mut editor = TextEditor::create(&context);
    editor.set_text("alpha beta").unwrap();
    editor.set_language(Some(Language::Cpp));
    render(&mut context, &mut editor, 0);

    editor.enable_trie_autocomplete().unwrap();
    editor.set_text("al").unwrap();
    editor.set_cursor(Position::new(0, 2)).unwrap();
    editor.focus();
    render(&mut context, &mut editor, 1);

    context.io_mut().add_focus_event(true);
    context.io_mut().add_key_event(Key::ModCtrl, true);
    context.io_mut().add_key_event(Key::Space, true);
    render(&mut context, &mut editor, 2);
    context.io_mut().add_key_event(Key::Space, false);
    context.io_mut().add_key_event(Key::ModCtrl, false);
    std::thread::sleep(Duration::from_millis(225));
    render(&mut context, &mut editor, 3);

    context.io_mut().add_key_event(Key::Enter, true);
    render(&mut context, &mut editor, 4);
    context.io_mut().add_key_event(Key::Enter, false);
    render(&mut context, &mut editor, 5);

    assert_eq!(editor.text().unwrap(), "alpha");
    editor.disable_trie_autocomplete().unwrap();
    assert!(editor.trie_autocomplete().is_none());

    editor.set_text("al").unwrap();
    editor.set_cursor(Position::new(0, 2)).unwrap();
    editor.focus();
    render(&mut context, &mut editor, 6);
    context.io_mut().add_key_event(Key::ModCtrl, true);
    context.io_mut().add_key_event(Key::Space, true);
    render(&mut context, &mut editor, 7);
    context.io_mut().add_key_event(Key::Space, false);
    context.io_mut().add_key_event(Key::ModCtrl, false);
    std::thread::sleep(Duration::from_millis(225));
    render(&mut context, &mut editor, 8);
    context.io_mut().add_key_event(Key::Enter, true);
    render(&mut context, &mut editor, 9);
    context.io_mut().add_key_event(Key::Enter, false);
    render(&mut context, &mut editor, 10);
    assert_ne!(editor.text().unwrap(), "alpha");
}

#[test]
fn disabling_trie_autocomplete_cancels_queued_activation() {
    let mut context = context();
    let mut editor = TextEditor::create(&context);
    editor.set_text("alpha beta").unwrap();
    editor.set_language(Some(Language::Cpp));
    render(&mut context, &mut editor, 0);

    editor.enable_trie_autocomplete().unwrap();
    editor.set_text("al").unwrap();
    editor.set_cursor(Position::new(0, 2)).unwrap();
    editor.focus();
    render(&mut context, &mut editor, 1);
    request_ctrl_space(&mut context, &mut editor, 2);

    editor.disable_trie_autocomplete().unwrap();
    std::thread::sleep(Duration::from_millis(225));
    render(&mut context, &mut editor, 3);
    context.io_mut().add_key_event(Key::Enter, true);
    render(&mut context, &mut editor, 4);
    context.io_mut().add_key_event(Key::Enter, false);
    render(&mut context, &mut editor, 5);

    assert_ne!(editor.text().unwrap(), "alpha");
}

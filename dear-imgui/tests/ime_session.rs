use dear_imgui_rs::Context;

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn context() -> Context {
    let mut context = Context::create();
    context.io_mut().set_display_size([800.0, 600.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
    context
        .set_ini_filename::<std::path::PathBuf>(None)
        .unwrap();
    context
}

fn input_frame(context: &mut Context, label: &str, text: &mut String, activate: bool) {
    let ui = context.frame();
    ui.window("shared IME viewport")
        .size([500.0, 250.0], dear_imgui_rs::Condition::Always)
        .build(|| {
            if activate {
                ui.set_keyboard_focus_here();
            }
            ui.input_text(label, text).build();
        });
    let _ = context.render_legacy();
}

fn activate(context: &mut Context, label: &str, text: &mut String) {
    input_frame(context, label, text, true);
    input_frame(context, label, text, false);
}

#[test]
fn empty_preedit_preserves_owner_until_the_final_commit() {
    let _guard = test_guard();
    let mut context = context();
    let mut text = String::new();
    activate(&mut context, "input", &mut text);
    context.notify_ime_enabled(true);
    context.set_ime_preedit("中文", Some((3, 6))).unwrap();
    context.set_ime_preedit("", None).unwrap();
    assert!(!context.has_ime_preedit());
    assert!(context.has_ime_composition());

    assert!(context.commit_ime_text("中\n文\0"));
    assert!(!context.has_ime_composition());
    input_frame(&mut context, "input", &mut text, false);
    assert_eq!(text, "中文");
}

#[test]
fn changing_widgets_rejects_old_updates_and_commits_until_a_new_enabled_notice() {
    let _guard = test_guard();
    let mut context = context();
    let mut first = String::new();
    let mut second = String::new();
    activate(&mut context, "first", &mut first);
    context.notify_ime_enabled(true);
    context.set_ime_preedit("旧", Some((3, 3))).unwrap();
    let viewport = context.ime_session_viewport().unwrap();
    // Winit sends this immediately before commit; it must not forget the owner.
    context.set_ime_preedit("", None).unwrap();
    activate(&mut context, "second", &mut second);
    assert!(!context.has_ime_preedit());
    assert!(!context.has_ime_composition());

    let binding = context.binding();
    assert_eq!(binding.take_ime_reset_request(), Some(viewport));
    assert_eq!(binding.take_ime_reset_request(), None);
    context.set_ime_preedit("迟到", Some((6, 6))).unwrap();
    assert!(!context.has_ime_preedit());
    assert!(!context.commit_ime_text("迟到"));
    context.notify_ime_enabled(false);
    assert!(!context.commit_ime_text("仍然迟到"));

    context.notify_ime_enabled(true);
    context.set_ime_preedit("新", Some((3, 3))).unwrap();
    assert!(context.commit_ime_text("新"));
    input_frame(&mut context, "second", &mut second, false);
    assert!(first.is_empty());
    assert_eq!(second, "新");
}

#[test]
fn retiring_the_owner_viewport_cancels_even_an_empty_panel() {
    let _guard = test_guard();
    let mut context = context();
    let mut text = String::new();
    activate(&mut context, "input", &mut text);
    context.set_ime_preedit("old", Some((3, 3))).unwrap();
    let viewport = context.ime_session_viewport().unwrap();
    context.set_ime_preedit("", None).unwrap();
    let binding = context.binding();

    binding.clear_ime_preedit_for_viewport(dear_imgui_rs::Id::from(viewport.raw().wrapping_add(1)));
    assert!(context.has_ime_composition());
    binding.clear_ime_preedit_for_viewport(viewport);
    assert!(!context.commit_ime_text("old"));
    assert_eq!(binding.take_ime_reset_request(), Some(viewport));

    // A replacement platform can explicitly retire the old event source.
    binding.reset_ime_session();
    context.set_ime_preedit("new", Some((3, 3))).unwrap();
    assert!(context.has_ime_composition());
}

#[test]
fn explicit_cancellation_waits_for_native_reenable_without_committing() {
    let _guard = test_guard();
    let mut context = context();
    let mut text = String::new();
    activate(&mut context, "input", &mut text);
    context.set_ime_preedit("cancel", None).unwrap();
    context.clear_ime_preedit();
    assert!(!context.commit_ime_text("cancel"));
    context.set_ime_preedit("late", None).unwrap();
    assert!(!context.has_ime_preedit());

    context.notify_ime_enabled(true);
    context.set_ime_preedit("next", None).unwrap();
    assert!(context.has_ime_preedit());
    assert!(text.is_empty());
}

#[test]
fn custom_editors_can_receive_commits_without_core_input_text_state() {
    let _guard = test_guard();
    let mut context = context();
    context.notify_ime_enabled(true);
    let viewport = context.main_viewport().id();
    unsafe {
        let raw = &mut *context.as_raw();
        assert_eq!(raw.ActiveId, 0);
        raw.PlatformImeData.WantTextInput = true;
        raw.PlatformImeData.ViewportId = viewport.raw();
    }
    assert_eq!(context.text_input_viewport(), Some(viewport));
    context.set_ime_preedit("custom", None).unwrap();
    assert!(
        !context.has_ime_preedit(),
        "custom editors render their own composition"
    );
    assert!(context.commit_ime_text("文"));
    let _ = context.frame();
    unsafe {
        let characters = &(*context.as_raw()).IO.InputQueueCharacters;
        assert_eq!(characters.Size, 1);
        assert_eq!(*characters.Data, u32::from('文'));
    }
    context.end_frame();
}

#[test]
fn inactive_input_submitted_before_the_owner_does_not_cancel_composition() {
    let _guard = test_guard();
    let mut context = context();
    let mut first = String::new();
    let mut second = String::new();
    let mut frame = |context: &mut Context, activate: bool| {
        let ui = context.frame();
        ui.window("ordered inputs")
            .size([500.0, 250.0], dear_imgui_rs::Condition::Always)
            .build(|| {
                ui.input_text("first", &mut first).build();
                if activate {
                    ui.set_keyboard_focus_here();
                }
                ui.input_text("second", &mut second).build();
            });
        let _ = context.render_legacy();
    };
    frame(&mut context, true);
    frame(&mut context, false);
    context.notify_ime_enabled(true);
    context.set_ime_preedit("中文", Some((3, 6))).unwrap();
    for _ in 0..3 {
        frame(&mut context, false);
        assert!(context.has_ime_composition());
        assert_eq!(context.ime_preedit().unwrap().text(), "中文");
        assert_eq!(context.binding().take_ime_reset_request(), None);
    }
    assert!(first.is_empty());
    assert!(second.is_empty());
}

use dear_imgui_rs::{Context, ImePreeditError, sys};

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

fn input_frame(context: &mut Context, text: &mut String, activate: bool) -> usize {
    let ui = context.frame();
    ui.window("IME input")
        .position([40.0, 40.0], dear_imgui_rs::Condition::Always)
        .size([500.0, 250.0], dear_imgui_rs::Condition::Always)
        .build(|| {
            if activate {
                ui.set_keyboard_focus_here();
            }
            ui.input_text("text", text).build();
        });
    context.render_legacy().total_vtx_count()
}

#[test]
fn preedit_renders_without_editing_the_committed_buffer() {
    let _guard = test_guard();
    let mut context = context();
    let mut text = String::from("prefix");
    input_frame(&mut context, &mut text, true);
    let baseline = input_frame(&mut context, &mut text, false);
    context.set_ime_preedit("中文", Some((0, 3))).unwrap();
    assert_eq!(context.ime_preedit().unwrap().text(), "中文");
    assert_eq!(context.ime_preedit().unwrap().cursor_range(), Some((0, 3)));
    let composed = input_frame(&mut context, &mut text, false);
    assert!(
        composed > baseline,
        "preedit must emit visible draw geometry"
    );
    assert_eq!(text, "prefix", "composition is not committed text");
    assert!(
        !context.end_frame(),
        "closing an already rendered frame is a no-op"
    );
    assert!(
        context.has_ime_preedit(),
        "idempotent frame cleanup must keep composition"
    );

    context.set_ime_preedit("", None).unwrap();
    assert!(context.commit_ime_text("中文"));
    input_frame(&mut context, &mut text, false);
    assert!(context.ime_preedit().is_none());
    // Keyboard focus selected the original text; only the final commit replaces it.
    assert_eq!(text, "中文");
}

#[test]
fn preedit_validates_utf8_ranges_atomically_and_clears_on_focus_loss() {
    let _guard = test_guard();
    let mut context = context();
    let mut text = String::new();
    input_frame(&mut context, &mut text, true);
    input_frame(&mut context, &mut text, false);
    context.set_ime_preedit("中文", Some((3, 3))).unwrap();
    for cursor in [Some((1, 3)), Some((0, 7))] {
        assert_eq!(
            context.set_ime_preedit("中文", cursor),
            Err(ImePreeditError::InvalidCursorRange)
        );
        assert_eq!(context.ime_preedit().unwrap().cursor_range(), Some((3, 3)));
    }
    context.set_ime_preedit("中文", None).unwrap();
    assert_eq!(context.ime_preedit().unwrap().cursor_range(), None);
    context.set_ime_preedit("", None).unwrap();
    assert!(context.ime_preedit().is_none());
    context.set_ime_preedit("文", Some((3, 3))).unwrap();
    context.io_mut().add_focus_event(false);
    input_frame(&mut context, &mut text, false);
    assert!(context.ime_preedit().is_none());
    assert!(text.is_empty());
}

#[test]
fn preedit_follows_the_caret_and_cannot_move_to_a_different_widget() {
    let _guard = test_guard();
    let mut context = context();
    let mut text = String::new();
    input_frame(&mut context, &mut text, true);
    input_frame(&mut context, &mut text, false);
    let original = unsafe { (*context.as_raw()).PlatformImeData.InputPos };
    context.set_ime_preedit("abc", Some((3, 3))).unwrap();
    input_frame(&mut context, &mut text, false);
    let caret = unsafe { (*context.as_raw()).PlatformImeData.InputPos };
    assert!(caret.x > original.x);
    assert!(caret.y.is_finite());
    assert_eq!(text, "");

    for _ in 0..2 {
        let ui = context.frame();
        ui.window("other input").build(|| {
            ui.set_keyboard_focus_here();
            ui.input_text("other", &mut text).build();
        });
        let _ = context.render_legacy();
    }
    assert!(context.ime_preedit().is_none());
    assert_ne!(unsafe { (*context.as_raw()).ActiveId }, 0);
    assert_eq!(unsafe { sys::igGetCurrentContext() }, context.as_raw());
}

#[test]
fn abandoned_frame_cancels_composition() {
    let _guard = test_guard();
    let mut context = context();
    let mut text = String::new();
    input_frame(&mut context, &mut text, true);
    input_frame(&mut context, &mut text, false);
    context.set_ime_preedit("abc", Some((3, 3))).unwrap();
    let _ = context.frame();
    assert!(context.end_frame());
    assert!(!context.has_ime_preedit());
    input_frame(&mut context, &mut text, false);
    assert!(text.is_empty());

    context.notify_ime_enabled(true);
    context.set_ime_preedit("abc", None).unwrap();
    drop(context.begin_frame());
    assert!(!context.has_ime_preedit());
}

#[test]
fn scoped_font_and_density_are_preserved_after_the_font_scope_ends() {
    let _guard = test_guard();
    let mut context = Context::create();
    let consumer = context.create_detached_renderer_consumer().unwrap();
    context
        .io_mut()
        .set_backend_flags(dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES);
    context.io_mut().set_display_size([800.0, 600.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context
        .set_ini_filename::<std::path::PathBuf>(None)
        .unwrap();
    context
        .font_atlas()
        .add_font(&[dear_imgui_rs::FontSource::default_font_with_size(13.0)]);
    let custom = context
        .font_atlas()
        .add_font(&[dear_imgui_rs::FontSource::default_font_with_size(29.0)]);
    let mut text = String::new();
    let mut render = |context: &mut Context, activate: bool| {
        let mut font = std::ptr::null_mut();
        let mut anchor = 0.0;
        let mut width = 0.0;
        let ui = context.frame();
        ui.window("font input")
            .size([500.0, 250.0], dear_imgui_rs::Condition::Always)
            .build(|| {
                let _font = ui.push_font(custom);
                if activate {
                    ui.set_keyboard_focus_here();
                }
                ui.input_text("text", &mut text).build();
                unsafe {
                    let raw = sys::igGetCurrentContext();
                    font = (*raw).Font;
                    anchor = (*raw).PlatformImeData.InputPos.x;
                    width = sys::ImFont_CalcTextSizeA(
                        font,
                        29.0,
                        f32::MAX,
                        0.0,
                        c"WWW".as_ptr(),
                        std::ptr::null(),
                        std::ptr::null_mut(),
                    )
                    .x;
                }
            });
        unsafe {
            assert_ne!((*context.as_raw()).Font, font, "font scope must have ended");
            // Another viewport/font operation may have changed this by frame completion.
            (*font).CurrentRasterizerDensity = 2.0;
        }
        let snapshot = context.render_snapshot(&consumer).unwrap();
        assert!(!snapshot.texture_requests().is_empty());
        assert!(
            snapshot
                .draw_data()
                .draw_lists
                .iter()
                .any(|list| !list.vtx.is_empty())
        );
        assert_eq!(unsafe { (*font).CurrentRasterizerDensity }, 2.0);
        (anchor, width)
    };
    render(&mut context, true);
    render(&mut context, false);
    context.set_ime_preedit("WWW", Some((3, 3))).unwrap();
    let (anchor, width) = render(&mut context, false);
    let position = unsafe { (*context.as_raw()).PlatformImeData.InputPos };
    assert!((position.x - (anchor + 3.0 + width)).abs() < 0.1);
    assert!(context.has_ime_preedit());
}

#[test]
fn multiline_hint_and_password_inputs_render_transient_text() {
    let _guard = test_guard();
    for kind in 0..3 {
        let mut context = context();
        let mut text = String::new();
        let mut frame = |context: &mut Context, activate: bool| {
            let ui = context.frame();
            ui.window("input variants")
                .size([500.0, 250.0], dear_imgui_rs::Condition::Always)
                .build(|| {
                    if activate {
                        ui.set_keyboard_focus_here();
                    }
                    match kind {
                        0 => {
                            ui.input_text_multiline("text", &mut text, [400.0, 120.0])
                                .build();
                        }
                        1 => {
                            ui.input_text("text", &mut text).hint("hint").build();
                        }
                        _ => {
                            ui.input_text("text", &mut text).password(true).build();
                        }
                    }
                    // An inactive input must not overwrite the active input's presentation.
                    let mut other = String::new();
                    ui.input_text("inactive", &mut other).build();
                });
            context.render_legacy().total_vtx_count()
        };
        frame(&mut context, true);
        let baseline = frame(&mut context, false);
        context.set_ime_preedit("ab\ncd", Some((5, 1))).unwrap();
        assert!(
            context.has_ime_preedit(),
            "input variant {kind} must capture presentation"
        );
        assert!(frame(&mut context, false) > baseline);
        assert!(context.has_ime_preedit());
        assert!(text.is_empty());
    }
}

use crate::sys;

#[test]
fn metrics_counts_are_usize_and_reject_negative_raw_values() {
    let mut ctx = crate::Context::create();
    let io = ctx.io_mut();

    io.inner_mut().MetricsRenderVertices = 11;
    io.inner_mut().MetricsRenderIndices = 22;
    io.inner_mut().MetricsRenderWindows = 3;
    io.inner_mut().MetricsActiveWindows = 4;

    assert_eq!(io.metrics_render_vertices(), 11);
    assert_eq!(io.metrics_render_indices(), 22);
    assert_eq!(io.metrics_render_windows(), 3);
    assert_eq!(io.metrics_active_windows(), 4);

    io.inner_mut().MetricsRenderVertices = -1;
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = io.metrics_render_vertices();
        }))
        .is_err()
    );

    io.inner_mut().MetricsRenderVertices = 0;
    io.inner_mut().MetricsRenderIndices = -1;
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = io.metrics_render_indices();
        }))
        .is_err()
    );

    io.inner_mut().MetricsRenderIndices = 0;
    io.inner_mut().MetricsRenderWindows = -1;
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = io.metrics_render_windows();
        }))
        .is_err()
    );

    io.inner_mut().MetricsRenderWindows = 0;
    io.inner_mut().MetricsActiveWindows = -1;
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = io.metrics_active_windows();
        }))
        .is_err()
    );
}

#[test]
fn mouse_hovered_viewport_round_trips_through_io() {
    let mut ctx = crate::Context::create();
    let io = ctx.io_mut();
    let viewport_id = crate::Id::from(0x1234);

    io.set_mouse_hovered_viewport(viewport_id);

    assert_eq!(io.mouse_hovered_viewport(), viewport_id);
}

#[test]
fn mouse_click_timing_enforces_single_click_after_double_click() {
    let mut ctx = crate::Context::create();
    let io = ctx.io_mut();

    io.set_mouse_double_click_time(0.2);
    io.set_mouse_single_click_delay(0.4);

    assert_eq!(io.mouse_double_click_time(), 0.2);
    assert_eq!(io.mouse_single_click_delay(), 0.4);

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            io.set_mouse_double_click_time(0.4);
        }))
        .is_err()
    );
    assert_eq!(io.mouse_double_click_time(), 0.2);

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            io.set_mouse_single_click_delay(0.2);
        }))
        .is_err()
    );
    assert_eq!(io.mouse_single_click_delay(), 0.4);
}

#[test]
fn utf16_input_queues_only_scalars_and_replaces_unpaired_surrogates() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut context = crate::Context::create();
    for unit in [
        0xdc00, 0xd800, 0x0041, 0xd800, 0xd83d, 0xde00, 0xdbff, 0xdfff, 0xd800, 0, 0,
    ] {
        context.io_mut().add_input_character_utf16(unit);
    }

    let queue = unsafe {
        crate::internal::imvector_cast_ref::<sys::ImGuiInputEvent, _>(
            &(*context.as_raw()).InputEventsQueue,
        )
    };
    let characters: Vec<_> = queue
        .as_slice()
        .iter()
        .map(|event| {
            assert_eq!(event.Type, sys::ImGuiInputEventType_Text);
            unsafe { event.__bindgen_anon_1.Text.Char }
        })
        .collect();
    assert_eq!(
        characters,
        [0xfffd, 0xfffd, 0x0041, 0xfffd, 0x1f600, 0x10ffff, 0xfffd]
    );
    assert_eq!(context.io().inner().InputQueueSurrogate, 0);
}

#[test]
fn pending_utf16_input_follows_native_pause_focus_and_clear_behavior() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut context = crate::Context::create();
    context
        .set_ini_filename::<std::path::PathBuf>(None)
        .unwrap();
    context.io_mut().set_display_size([128.0, 128.0]);
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();

    let io = context.io_mut();
    io.add_input_character_utf16(0xd83d);
    io.set_app_accepting_events(false);
    io.add_input_character_utf16(0xd800);
    io.set_app_accepting_events(true);
    io.clear_events_queue();
    io.clear_input_keys();
    io.add_focus_event(false);

    let io = context.frame().io().inner();
    assert!(io.AppFocusLost);
    assert_eq!(io.InputQueueSurrogate, 0xd83d);
    assert_eq!(io.InputQueueCharacters.Size, 0);
    let _ = context.render_legacy();

    let io = context.io_mut();
    io.add_focus_event(true);
    io.add_input_character_utf16(0xde00);
    let io = context.frame().io().inner();
    assert_eq!(io.InputQueueSurrogate, 0);
    assert_eq!(io.InputQueueCharacters.Size, 1);
    assert_eq!(unsafe { *io.InputQueueCharacters.Data }, 0x1f600);
    let _ = context.render_legacy();
}

#[test]
fn non_modifier_key_reset_preserves_text_mouse_and_native_modifiers() {
    let mut context = crate::Context::create();
    context.io_mut().set_config_input_trickle_event_queue(false);
    context.io_mut().set_display_size([128.0, 128.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
    for macos_behaviors in [false, true] {
        context
            .io_mut()
            .set_config_macosx_behaviors(macos_behaviors);
        context.io_mut().add_key_event(crate::Key::ModCtrl, true);
        context.io_mut().add_key_event(crate::Key::LeftCtrl, true);
        context.io_mut().add_key_event(crate::Key::Backspace, true);
        context.io_mut().add_input_characters_utf8("甲");
        context
            .io_mut()
            .add_mouse_button_event(crate::input::MouseButton::Left, true);
        let _ = context.frame();
        assert!(context.io().inner().KeysData[key_data_index(crate::Key::Backspace)].Down);
        let modifiers = context.io().inner().KeyMods;
        assert_ne!(modifiers, 0);
        let characters = context.io().inner().InputQueueCharacters.Size;
        assert_eq!(characters, 1);
        // macOS maps Ctrl+left-click to right-click; preserve the native result.
        let mouse_down = context.io().inner().MouseDown;
        assert!(mouse_down.iter().any(|&down| down));

        context.io_mut().add_key_event(crate::Key::Delete, true);
        context.io_mut().add_key_event(crate::Key::ModShift, true);
        context.io_mut().add_input_characters_utf8("乙");
        context.io_mut().add_mouse_wheel_event([0.0, 1.0]);
        context.io_mut().clear_non_modifier_keys();
        assert!(!context.io().inner().KeysData[key_data_index(crate::Key::Backspace)].Down);
        assert_eq!(context.io().inner().KeyMods, modifiers);
        assert_eq!(context.io().inner().InputQueueCharacters.Size, characters);
        assert_eq!(context.io().inner().MouseDown, mouse_down);
        let _ = context.render_legacy();

        let ui = context.frame();
        assert!(!ui.is_key_down(crate::Key::Backspace));
        assert!(!ui.is_key_down(crate::Key::Delete));
        assert!(ui.is_key_down(if macos_behaviors {
            crate::Key::LeftSuper
        } else {
            crate::Key::LeftCtrl
        }));
        assert!(ui.io().key_shift());
        assert_eq!(ui.io().key_super(), macos_behaviors);
        assert_eq!(ui.io().key_ctrl(), !macos_behaviors);
        assert_eq!(ui.io().inner().MouseDown, mouse_down);
        assert_eq!(ui.io().inner().MouseWheel, 1.0);
        let characters = &ui.io().inner().InputQueueCharacters;
        assert_eq!(characters.Size, 1);
        assert_eq!(unsafe { *characters.Data }, '乙' as u32);
        let _ = context.render_legacy();
        context.io_mut().clear_input_keys();
        context.io_mut().clear_input_mouse();
    }
}

#[test]
fn extended_input_events_update_and_clear_context_state() {
    let mut ctx = crate::Context::create();
    ctx.io_mut().set_display_size([128.0, 128.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);
    ctx.font_atlas()
        .try_claim_legacy_renderer()
        .expect("legacy renderer font atlas should be available")
        .build();

    ctx.io_mut().set_app_accepting_events(true);
    ctx.io_mut().add_key_analog_event(crate::Key::A, true, 0.5);
    ctx.io_mut()
        .set_key_event_native_data(crate::Key::A, 65, 30, Some(65));
    assert!(ctx.frame().is_key_down(crate::Key::A));
    ctx.io_mut().clear_input_keys();
    assert!(!ctx.io().inner().KeysData[key_data_index(crate::Key::A)].Down);
    let _ = ctx.render_legacy();

    ctx.io_mut().add_key_event(crate::Key::A, true);
    ctx.io_mut().clear_events_queue();
    assert!(!ctx.frame().is_key_down(crate::Key::A));
    let _ = ctx.render_legacy();

    ctx.io_mut().set_app_accepting_events(false);
    ctx.io_mut().add_key_event(crate::Key::A, true);
    assert!(!ctx.frame().is_key_down(crate::Key::A));
    let _ = ctx.render_legacy();

    ctx.io_mut().set_app_accepting_events(true);
    ctx.io_mut().add_input_character_utf16(b'A' as u16);
    ctx.io_mut().add_input_characters_utf8("B界");
    let _ = ctx.frame();
    assert_eq!(ctx.io().inner().InputQueueCharacters.Size, 3);
    ctx.io_mut().clear_input_keys();
    assert_eq!(ctx.io().inner().InputQueueCharacters.Size, 0);

    ctx.io_mut().inner_mut().MouseDown[0] = true;
    ctx.io_mut().clear_input_mouse();
    assert!(!ctx.io().inner().MouseDown[0]);
    let _ = ctx.render_legacy();
}

fn key_data_index(key: crate::Key) -> usize {
    usize::try_from(key as sys::ImGuiKey - sys::ImGuiKey_NamedKey_BEGIN).unwrap()
}

#[test]
fn extended_input_events_validate_values_before_ffi() {
    let mut ctx = crate::Context::create();
    let io = ctx.io_mut();

    for value in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                io.add_key_analog_event(crate::Key::GamepadL2, true, value);
            }))
            .is_err()
        );
    }
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            io.set_key_event_native_data(crate::Key::ModCtrl, 0, 0, None);
        }))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            io.set_key_event_native_data(crate::Key::A, 0, 0, Some(512));
        }))
        .is_err()
    );
}

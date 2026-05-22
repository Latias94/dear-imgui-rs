use bevy_app::{App, PreUpdate};
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_input::ButtonState;
use bevy_input::keyboard::{Key as BevyKey, KeyCode, KeyboardFocusLost, KeyboardInput};
use bevy_input::mouse::{
    MouseButton as BevyMouseButton, MouseButtonInput, MouseScrollUnit, MouseWheel,
};
use bevy_input::touch::{TouchInput, TouchPhase};
use bevy_math::Vec2;
use bevy_window::{
    CursorLeft, CursorMoved, Ime, PrimaryWindow, Window, WindowFocused, WindowResized,
    WindowResolution, WindowScaleFactorChanged,
};
use dear_imgui_bevy::{
    ImguiContext, ImguiPlugin,
    input::{ImguiInputState, map_bevy_key_code},
};
use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn imgui_context_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn app_with_primary_window() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(ImguiPlugin::default());

    let mut window = Window {
        resolution: WindowResolution::new(1600, 1200),
        ..Default::default()
    };
    window.resolution.set_scale_factor(2.0);

    let primary = app.world_mut().spawn((window, PrimaryWindow)).id();
    prepare_imgui_context(&mut app);
    (app, primary)
}

fn prepare_imgui_context(app: &mut App) {
    let mut context = app
        .world_mut()
        .get_non_send_mut::<ImguiContext>()
        .expect("ImguiPlugin should install an ImGui context");
    let ctx = context.context_mut();
    ctx.io_mut().set_delta_time(1.0 / 60.0);
    ctx.io_mut().set_config_input_trickle_event_queue(false);
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
}

fn current_frame_input_chars() -> Vec<u32> {
    unsafe {
        let io = imgui::sys::igGetIO_Nil();
        let legacy_queue = &(*io).InputQueueCharacters;
        let mut chars = Vec::new();
        if legacy_queue.Size > 0 && !legacy_queue.Data.is_null() {
            chars.extend(
                std::slice::from_raw_parts(legacy_queue.Data, legacy_queue.Size as usize)
                    .iter()
                    .copied(),
            );
        }
        chars
    }
}

fn begin_frame_and_assert(app: &mut App, assert_ui: impl FnOnce(&imgui::Ui)) {
    let mut context = app
        .world_mut()
        .get_non_send_mut::<ImguiContext>()
        .expect("ImguiContext should exist");
    let frame = context.context_mut().begin_frame();
    assert_ui(frame.ui());
    let _ = frame.render();
}

fn run_input_systems(app: &mut App) {
    app.world_mut().run_schedule(PreUpdate);
}

fn key_input(
    window: Entity,
    key_code: KeyCode,
    logical_key: BevyKey,
    state: ButtonState,
    text: Option<&str>,
) -> KeyboardInput {
    KeyboardInput {
        key_code,
        logical_key,
        state,
        text: text.map(Into::into),
        repeat: false,
        window,
    }
}

#[test]
fn primary_window_input_maps_window_mouse_and_scroll_into_imgui_io() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary,
            position: Vec2::new(123.0, 45.0),
            delta: None,
        });
    app.world_mut()
        .resource_mut::<Messages<MouseButtonInput>>()
        .write(MouseButtonInput {
            button: BevyMouseButton::Left,
            state: ButtonState::Pressed,
            window: primary,
        });
    app.world_mut()
        .resource_mut::<Messages<MouseWheel>>()
        .write(MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 1.0,
            y: -2.0,
            window: primary,
            phase: TouchPhase::Moved,
        });

    run_input_systems(&mut app);

    {
        let context = app
            .world()
            .get_non_send::<ImguiContext>()
            .unwrap()
            .context();
        assert_eq!(context.io().display_size(), [800.0, 600.0]);
        assert_eq!(context.io().display_framebuffer_scale(), [2.0, 2.0]);
    }

    begin_frame_and_assert(&mut app, |ui| {
        assert_eq!(ui.mouse_pos(), [123.0, 45.0]);
        assert!(ui.is_mouse_down(imgui::MouseButton::Left));
        assert_eq!(ui.io().mouse_source(), imgui::MouseSource::Mouse);
        assert_eq!(ui.io().mouse_wheel_h(), 1.0);
        assert_eq!(ui.io().mouse_wheel(), -2.0);
    });
}

#[test]
fn input_keyboard_and_ime_messages_update_imgui_keys_modifiers_and_text_queue() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();

    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            primary,
            KeyCode::ControlLeft,
            BevyKey::Control,
            ButtonState::Pressed,
            None,
        ));
    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            primary,
            KeyCode::KeyA,
            BevyKey::Character("a".into()),
            ButtonState::Pressed,
            Some("a"),
        ));
    app.world_mut()
        .resource_mut::<Messages<Ime>>()
        .write(Ime::Commit {
            window: primary,
            value: "好!".to_owned(),
        });

    run_input_systems(&mut app);

    begin_frame_and_assert(&mut app, |ui| {
        let chars = current_frame_input_chars();
        assert!(chars.contains(&('a' as u32)));
        assert!(chars.contains(&('好' as u32)));
        assert!(chars.contains(&('!' as u32)));
        assert!(ui.is_key_down(imgui::Key::A));
    });
}

#[test]
fn input_ime_enable_commit_and_disable_preserves_explicit_ime_state() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();

    app.world_mut()
        .resource_mut::<Messages<Ime>>()
        .write(Ime::Enabled { window: primary });
    run_input_systems(&mut app);
    assert!(app.world().resource::<ImguiInputState>().ime_enabled());

    app.world_mut()
        .resource_mut::<Messages<Ime>>()
        .write(Ime::Commit {
            window: primary,
            value: "好".to_owned(),
        });
    run_input_systems(&mut app);
    assert!(
        app.world().resource::<ImguiInputState>().ime_enabled(),
        "committed text should not imply that the platform IME was disabled"
    );
    begin_frame_and_assert(&mut app, |_ui| {
        assert!(current_frame_input_chars().contains(&('好' as u32)));
    });

    app.world_mut()
        .resource_mut::<Messages<Ime>>()
        .write(Ime::Disabled { window: primary });
    run_input_systems(&mut app);
    assert!(!app.world().resource::<ImguiInputState>().ime_enabled());
}

#[test]
fn input_resize_dpi_and_cursor_leave_messages_update_imgui_io() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();

    app.world_mut()
        .resource_mut::<Messages<WindowResized>>()
        .write(WindowResized {
            window: primary,
            width: 1024.0,
            height: 768.0,
        });
    app.world_mut()
        .resource_mut::<Messages<WindowScaleFactorChanged>>()
        .write(WindowScaleFactorChanged {
            window: primary,
            scale_factor: 1.5,
        });
    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary,
            position: Vec2::new(8.0, 9.0),
            delta: None,
        });
    app.world_mut()
        .resource_mut::<Messages<CursorLeft>>()
        .write(CursorLeft { window: primary });

    run_input_systems(&mut app);

    {
        let context = app
            .world()
            .get_non_send::<ImguiContext>()
            .unwrap()
            .context();
        assert_eq!(context.io().display_size(), [1024.0, 768.0]);
        assert_eq!(context.io().display_framebuffer_scale(), [1.5, 1.5]);
    }

    begin_frame_and_assert(&mut app, |ui| {
        assert!(
            ui.mouse_pos()[0] < -1.0e30 && ui.mouse_pos()[1] < -1.0e30,
            "CursorLeft should move the Dear ImGui mouse position outside every window"
        );
    });
}

#[test]
fn input_focus_loss_releases_tracked_keyboard_and_mouse_state() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();

    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            primary,
            KeyCode::ControlLeft,
            BevyKey::Control,
            ButtonState::Pressed,
            None,
        ));
    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            primary,
            KeyCode::KeyA,
            BevyKey::Character("a".into()),
            ButtonState::Pressed,
            Some("a"),
        ));
    app.world_mut()
        .resource_mut::<Messages<MouseButtonInput>>()
        .write(MouseButtonInput {
            button: BevyMouseButton::Left,
            state: ButtonState::Pressed,
            window: primary,
        });
    run_input_systems(&mut app);

    begin_frame_and_assert(&mut app, |ui| {
        assert!(ui.is_key_down(imgui::Key::A));
        assert!(ui.is_mouse_down(imgui::MouseButton::Left));
    });

    app.world_mut()
        .resource_mut::<Messages<WindowFocused>>()
        .write(WindowFocused {
            window: primary,
            focused: false,
        });
    run_input_systems(&mut app);

    begin_frame_and_assert(&mut app, |ui| {
        assert!(!ui.is_key_down(imgui::Key::A));
        assert!(!ui.is_mouse_down(imgui::MouseButton::Left));
    });
}

#[test]
fn input_keyboard_focus_lost_releases_tracked_state_without_window_message() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();

    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            primary,
            KeyCode::KeyA,
            BevyKey::Character("a".into()),
            ButtonState::Pressed,
            None,
        ));
    app.world_mut()
        .resource_mut::<Messages<MouseButtonInput>>()
        .write(MouseButtonInput {
            button: BevyMouseButton::Right,
            state: ButtonState::Pressed,
            window: primary,
        });
    run_input_systems(&mut app);

    begin_frame_and_assert(&mut app, |ui| {
        assert!(ui.is_key_down(imgui::Key::A));
        assert!(ui.is_mouse_down(imgui::MouseButton::Right));
    });

    app.world_mut()
        .resource_mut::<Messages<KeyboardFocusLost>>()
        .write(KeyboardFocusLost);
    run_input_systems(&mut app);

    assert_eq!(
        app.world()
            .resource::<ImguiInputState>()
            .primary_window_focused(),
        Some(false)
    );
    begin_frame_and_assert(&mut app, |ui| {
        assert!(!ui.is_key_down(imgui::Key::A));
        assert!(!ui.is_mouse_down(imgui::MouseButton::Right));
    });
}

#[test]
fn input_touch_events_drive_first_active_finger_as_touchscreen_mouse() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();

    app.world_mut()
        .resource_mut::<Messages<TouchInput>>()
        .write(TouchInput {
            phase: TouchPhase::Started,
            position: Vec2::new(10.0, 20.0),
            window: primary,
            force: None,
            id: 7,
        });
    run_input_systems(&mut app);

    assert_eq!(
        app.world().resource::<ImguiInputState>().active_touch_id(),
        Some(7)
    );
    begin_frame_and_assert(&mut app, |ui| {
        assert_eq!(ui.mouse_pos(), [10.0, 20.0]);
        assert_eq!(ui.io().mouse_source(), imgui::MouseSource::TouchScreen);
        assert!(ui.is_mouse_down(imgui::MouseButton::Left));
    });

    app.world_mut()
        .resource_mut::<Messages<TouchInput>>()
        .write(TouchInput {
            phase: TouchPhase::Ended,
            position: Vec2::new(15.0, 25.0),
            window: primary,
            force: None,
            id: 7,
        });
    run_input_systems(&mut app);

    assert_eq!(
        app.world().resource::<ImguiInputState>().active_touch_id(),
        None
    );
    begin_frame_and_assert(&mut app, |ui| {
        assert_eq!(ui.mouse_pos(), [15.0, 25.0]);
        assert_eq!(ui.io().mouse_source(), imgui::MouseSource::TouchScreen);
        assert!(!ui.is_mouse_down(imgui::MouseButton::Left));
    });
}

#[test]
fn input_non_primary_window_messages_are_ignored() {
    let _guard = imgui_context_guard();
    let (mut app, _primary) = app_with_primary_window();
    let secondary = app.world_mut().spawn(Window::default()).id();

    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(key_input(
            secondary,
            KeyCode::KeyX,
            BevyKey::Character("x".into()),
            ButtonState::Pressed,
            Some("x"),
        ));
    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: secondary,
            position: Vec2::new(300.0, 400.0),
            delta: None,
        });
    run_input_systems(&mut app);

    begin_frame_and_assert(&mut app, |ui| {
        assert!(current_frame_input_chars().is_empty());
        assert!(!ui.is_key_down(imgui::Key::X));
        assert_ne!(ui.mouse_pos(), [300.0, 400.0]);
    });
}

#[test]
fn input_key_mapping_covers_modifiers_and_common_keys() {
    assert_eq!(
        map_bevy_key_code(KeyCode::ControlLeft),
        Some(imgui::Key::LeftCtrl)
    );
    assert_eq!(
        map_bevy_key_code(KeyCode::ShiftRight),
        Some(imgui::Key::RightShift)
    );
    assert_eq!(map_bevy_key_code(KeyCode::KeyZ), Some(imgui::Key::Z));
    assert_eq!(
        map_bevy_key_code(KeyCode::NumpadEnter),
        Some(imgui::Key::KeypadEnter)
    );
}

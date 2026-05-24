use bevy_app::{App, PreUpdate};
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_input::ButtonState;
use bevy_input::keyboard::{Key as BevyKey, KeyCode, KeyboardFocusLost, KeyboardInput};
use bevy_input::mouse::{
    MouseButton as BevyMouseButton, MouseButtonInput, MouseScrollUnit, MouseWheel,
};
use bevy_input::touch::{TouchInput, TouchPhase};
use bevy_math::{IVec2, Vec2};
use bevy_window::{
    CursorEntered, CursorIcon, CursorLeft, CursorMoved, CursorOptions, Ime, PrimaryWindow,
    SystemCursorIcon, Window, WindowFocused, WindowPosition, WindowResized, WindowResolution,
    WindowScaleFactorChanged,
};
use dear_imgui_bevy::{
    ImguiContext, ImguiFrameState, ImguiPlugin, ImguiPrimaryContextPass, ImguiViewportWindow,
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

fn request_text_cursor_and_ime(
    mut imgui_context: NonSendMut<ImguiContext>,
    frame_state: NonSend<ImguiFrameState>,
) {
    let ui = frame_state.ui().expect("Dear ImGui frame should be open");
    ui.set_mouse_cursor(Some(imgui::MouseCursor::TextInput));
    imgui_context
        .context_mut()
        .io_mut()
        .set_mouse_draw_cursor(false);

    let raw_context = imgui_context.context().as_raw();
    // SAFETY: This test owns the backend context resource and mutates the live frame's platform IME
    // data to simulate Dear ImGui output that normally comes from an active text widget.
    unsafe {
        let ime_data = &mut (*raw_context).PlatformImeData;
        ime_data.WantTextInput = true;
        ime_data.InputPos = imgui::sys::ImVec2_c { x: 222.0, y: 333.0 };
    }
}

fn request_text_cursor_and_secondary_viewport_ime(
    mut imgui_context: NonSendMut<ImguiContext>,
    frame_state: NonSend<ImguiFrameState>,
) {
    let ui = frame_state.ui().expect("Dear ImGui frame should be open");
    ui.set_mouse_cursor(Some(imgui::MouseCursor::TextInput));
    imgui_context
        .context_mut()
        .io_mut()
        .set_mouse_draw_cursor(false);

    let raw_context = imgui_context.context().as_raw();
    unsafe {
        let ime_data = &mut (*raw_context).PlatformImeData;
        ime_data.WantTextInput = true;
        ime_data.InputPos = imgui::sys::ImVec2_c { x: 144.0, y: 205.0 };
        ime_data.ViewportId = 0x501;
    }
}

fn request_primary_cursor_and_secondary_viewport_ime(
    mut imgui_context: NonSendMut<ImguiContext>,
    frame_state: NonSend<ImguiFrameState>,
) {
    let ui = frame_state.ui().expect("Dear ImGui frame should be open");
    ui.set_mouse_cursor(Some(imgui::MouseCursor::TextInput));
    imgui_context
        .context_mut()
        .io_mut()
        .set_mouse_draw_cursor(false);

    let raw_context = imgui_context.context().as_raw();
    unsafe {
        let ime_data = &mut (*raw_context).PlatformImeData;
        ime_data.WantTextInput = true;
        ime_data.InputPos = imgui::sys::ImVec2_c { x: 77.0, y: 88.0 };
        ime_data.ViewportId = 0x502;
    }
}

fn request_software_cursor(
    mut imgui_context: NonSendMut<ImguiContext>,
    frame_state: NonSend<ImguiFrameState>,
) {
    let ui = frame_state.ui().expect("Dear ImGui frame should be open");
    ui.set_mouse_cursor(Some(imgui::MouseCursor::Hand));
    imgui_context
        .context_mut()
        .io_mut()
        .set_mouse_draw_cursor(true);

    let raw_context = imgui_context.context().as_raw();
    // SAFETY: This test owns the backend context resource and mutates the live frame's platform IME
    // data to keep the assertion focused on cursor visibility.
    unsafe {
        let ime_data = &mut (*raw_context).PlatformImeData;
        ime_data.WantTextInput = false;
        ime_data.InputPos = imgui::sys::ImVec2_c { x: 0.0, y: 0.0 };
    }
}

fn request_hidden_cursor(
    mut imgui_context: NonSendMut<ImguiContext>,
    frame_state: NonSend<ImguiFrameState>,
) {
    let ui = frame_state.ui().expect("Dear ImGui frame should be open");
    ui.set_mouse_cursor(None);
    imgui_context
        .context_mut()
        .io_mut()
        .set_mouse_draw_cursor(false);
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
fn primary_window_input_reports_main_hovered_viewport_when_viewports_are_enabled() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();
    let main_viewport_id = {
        let mut context = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
        let context = context.context_mut();
        context
            .io_mut()
            .set_config_flags(imgui::ConfigFlags::VIEWPORTS_ENABLE);
        context.main_viewport().id()
    };

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
    run_input_systems(&mut app);

    begin_frame_and_assert(&mut app, |ui| {
        assert_eq!(ui.mouse_pos(), [123.0, 45.0]);
        assert_eq!(ui.io().mouse_hovered_viewport(), main_viewport_id);
        assert!(ui.is_mouse_down(imgui::MouseButton::Left));
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
fn input_invalid_window_metrics_are_sanitized_before_reaching_imgui_io() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();
    app.world_mut()
        .get_mut::<Window>(primary)
        .unwrap()
        .resolution
        .set_scale_factor(f32::NAN);

    app.world_mut()
        .resource_mut::<Messages<WindowResized>>()
        .write(WindowResized {
            window: primary,
            width: f32::NAN,
            height: -10.0,
        });
    app.world_mut()
        .resource_mut::<Messages<WindowScaleFactorChanged>>()
        .write(WindowScaleFactorChanged {
            window: primary,
            scale_factor: f64::INFINITY,
        });

    run_input_systems(&mut app);

    let context = app
        .world()
        .get_non_send::<ImguiContext>()
        .unwrap()
        .context();
    assert_eq!(context.io().display_size(), [0.0, 0.0]);
    assert_eq!(context.io().display_framebuffer_scale(), [1.0, 1.0]);
}

#[test]
fn input_platform_feedback_updates_primary_window_cursor_and_ime_state() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();
    app.add_systems(ImguiPrimaryContextPass, request_text_cursor_and_ime);

    app.update();

    let entity = app.world().entity(primary);
    assert!(
        entity.get::<CursorOptions>().unwrap().visible,
        "OS cursor should stay visible when Dear ImGui is not drawing a software cursor"
    );
    assert_eq!(
        entity.get::<CursorIcon>(),
        Some(&CursorIcon::System(SystemCursorIcon::Text))
    );
    let window = entity.get::<Window>().unwrap();
    assert!(window.ime_enabled);
    assert_eq!(window.ime_position, Vec2::new(222.0, 333.0));
}

#[test]
fn input_platform_feedback_updates_secondary_viewport_window_cursor_and_ime_state() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();
    let viewport_id = imgui::Id::from(0x501);
    let secondary = app
        .world_mut()
        .spawn((
            Window {
                position: WindowPosition::At(IVec2::new(100, 150)),
                resolution: WindowResolution::new(640, 480),
                ..Default::default()
            },
            ImguiViewportWindow { viewport_id },
        ))
        .id();
    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: secondary,
            position: Vec2::new(10.0, 20.0),
            delta: None,
        });
    app.add_systems(
        ImguiPrimaryContextPass,
        request_text_cursor_and_secondary_viewport_ime,
    );

    app.update();

    let primary_window = app.world().entity(primary).get::<Window>().unwrap();
    assert!(
        !primary_window.ime_enabled,
        "IME feedback for a secondary viewport should not be applied to the primary window"
    );

    let entity = app.world().entity(secondary);
    assert!(
        entity.get::<CursorOptions>().unwrap().visible,
        "OS cursor should stay visible when Dear ImGui is not drawing a software cursor"
    );
    assert_eq!(
        entity.get::<CursorIcon>(),
        Some(&CursorIcon::System(SystemCursorIcon::Text))
    );
    let window = entity.get::<Window>().unwrap();
    assert!(window.ime_enabled);
    assert_eq!(window.ime_position, Vec2::new(44.0, 55.0));
}

#[test]
fn input_platform_feedback_routes_cursor_independently_from_ime_viewport() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();
    let viewport_id = imgui::Id::from(0x502);
    let secondary = app
        .world_mut()
        .spawn((
            Window {
                resolution: WindowResolution::new(640, 480),
                ..Default::default()
            },
            ImguiViewportWindow { viewport_id },
        ))
        .id();
    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary,
            position: Vec2::new(11.0, 22.0),
            delta: None,
        });
    app.add_systems(
        ImguiPrimaryContextPass,
        request_primary_cursor_and_secondary_viewport_ime,
    );

    app.update();

    let primary_entity = app.world().entity(primary);
    assert_eq!(
        primary_entity.get::<CursorIcon>(),
        Some(&CursorIcon::System(SystemCursorIcon::Text)),
        "cursor feedback should follow the hovered Bevy window"
    );
    assert!(
        !primary_entity.get::<Window>().unwrap().ime_enabled,
        "IME feedback for a secondary viewport should not be applied to the primary window"
    );

    let secondary_entity = app.world().entity(secondary);
    assert!(
        secondary_entity.get::<CursorIcon>().is_none(),
        "IME viewport must not pull cursor feedback onto a non-hovered window"
    );
    let secondary_window = secondary_entity.get::<Window>().unwrap();
    assert!(secondary_window.ime_enabled);
    assert_eq!(secondary_window.ime_position, Vec2::new(77.0, 88.0));
}

#[test]
fn input_platform_feedback_hides_os_cursor_when_imgui_draws_software_cursor() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();
    app.world_mut()
        .entity_mut(primary)
        .insert(CursorIcon::from(SystemCursorIcon::Pointer));
    app.add_systems(ImguiPrimaryContextPass, request_software_cursor);

    app.update();

    let entity = app.world().entity(primary);
    assert!(
        !entity.get::<CursorOptions>().unwrap().visible,
        "OS cursor should be hidden while Dear ImGui draws the software cursor"
    );
    assert!(entity.get::<CursorIcon>().is_none());
    assert!(!entity.get::<Window>().unwrap().ime_enabled);
}

#[test]
fn input_platform_feedback_hides_os_cursor_when_imgui_requests_no_cursor() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();
    app.world_mut()
        .entity_mut(primary)
        .insert(CursorIcon::from(SystemCursorIcon::Pointer));
    app.add_systems(ImguiPrimaryContextPass, request_hidden_cursor);

    app.update();

    let entity = app.world().entity(primary);
    assert!(
        !entity.get::<CursorOptions>().unwrap().visible,
        "OS cursor should be hidden when Dear ImGui reports no cursor"
    );
    assert!(entity.get::<CursorIcon>().is_none());
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
fn input_focus_switch_between_viewport_windows_keeps_sticky_input_pressed() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();
    let secondary = app
        .world_mut()
        .spawn((
            Window::default(),
            ImguiViewportWindow {
                viewport_id: imgui::Id::from(0x560),
            },
        ))
        .id();

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
    app.world_mut()
        .resource_mut::<Messages<WindowFocused>>()
        .write(WindowFocused {
            window: secondary,
            focused: true,
        });
    run_input_systems(&mut app);

    let input_state = app.world().resource::<ImguiInputState>();
    assert_eq!(input_state.primary_window_focused(), Some(false));
    assert_eq!(input_state.focused_window(), Some(secondary));
    begin_frame_and_assert(&mut app, |ui| {
        assert!(
            ui.is_key_down(imgui::Key::A),
            "switching focus between mapped ImGui windows must not synthesize a global key release"
        );
        assert!(
            ui.is_mouse_down(imgui::MouseButton::Left),
            "switching focus between mapped ImGui windows must not synthesize a global mouse release"
        );
    });
}

#[test]
fn input_primary_focus_sync_does_not_blur_while_secondary_viewport_is_focused() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();
    app.world_mut().get_mut::<Window>(primary).unwrap().focused = true;
    let secondary = app
        .world_mut()
        .spawn((
            Window::default(),
            ImguiViewportWindow {
                viewport_id: imgui::Id::from(0x561),
            },
        ))
        .id();

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
            button: BevyMouseButton::Left,
            state: ButtonState::Pressed,
            window: primary,
        });
    run_input_systems(&mut app);

    app.world_mut()
        .resource_mut::<Messages<WindowFocused>>()
        .write(WindowFocused {
            window: secondary,
            focused: true,
        });
    run_input_systems(&mut app);

    app.world_mut().get_mut::<Window>(primary).unwrap().focused = false;
    run_input_systems(&mut app);

    let input_state = app.world().resource::<ImguiInputState>();
    assert_eq!(input_state.primary_window_focused(), Some(false));
    assert_eq!(input_state.focused_window(), Some(secondary));
    begin_frame_and_assert(&mut app, |ui| {
        assert!(
            ui.is_key_down(imgui::Key::A),
            "primary focus sync must not release keys while a secondary ImGui viewport is focused"
        );
        assert!(
            ui.is_mouse_down(imgui::MouseButton::Left),
            "primary focus sync must not release mouse buttons while a secondary ImGui viewport is focused"
        );
    });
}

#[test]
fn input_stale_focused_viewport_window_releases_sticky_input() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();
    app.world_mut().get_mut::<Window>(primary).unwrap().focused = true;
    let secondary = app
        .world_mut()
        .spawn((
            Window::default(),
            ImguiViewportWindow {
                viewport_id: imgui::Id::from(0x562),
            },
        ))
        .id();

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
            button: BevyMouseButton::Left,
            state: ButtonState::Pressed,
            window: primary,
        });
    run_input_systems(&mut app);

    app.world_mut()
        .resource_mut::<Messages<WindowFocused>>()
        .write(WindowFocused {
            window: primary,
            focused: false,
        });
    app.world_mut()
        .resource_mut::<Messages<WindowFocused>>()
        .write(WindowFocused {
            window: secondary,
            focused: true,
        });
    run_input_systems(&mut app);
    app.world_mut().get_mut::<Window>(primary).unwrap().focused = false;
    app.world_mut().despawn(secondary);
    run_input_systems(&mut app);

    let input_state = app.world().resource::<ImguiInputState>();
    assert_eq!(input_state.focused_window(), None);
    begin_frame_and_assert(&mut app, |ui| {
        assert!(
            !ui.is_key_down(imgui::Key::A),
            "destroying the focused secondary viewport must release sticky keys"
        );
        assert!(
            !ui.is_mouse_down(imgui::MouseButton::Left),
            "destroying the focused secondary viewport must release sticky mouse buttons"
        );
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
fn input_missing_primary_window_releases_tracked_state_and_clears_window_state() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();
    let main_viewport_id = {
        let mut context = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
        let context = context.context_mut();
        context
            .io_mut()
            .set_config_flags(imgui::ConfigFlags::VIEWPORTS_ENABLE);
        context.main_viewport().id()
    };

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary,
            position: Vec2::new(12.0, 34.0),
            delta: None,
        });
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
    app.world_mut()
        .resource_mut::<Messages<TouchInput>>()
        .write(TouchInput {
            phase: TouchPhase::Started,
            position: Vec2::new(56.0, 78.0),
            window: primary,
            force: None,
            id: 9,
        });
    app.world_mut()
        .resource_mut::<Messages<Ime>>()
        .write(Ime::Enabled { window: primary });
    run_input_systems(&mut app);

    assert_eq!(
        app.world()
            .resource::<ImguiInputState>()
            .mouse_hovered_window(),
        Some(primary)
    );
    assert_eq!(
        app.world().resource::<ImguiInputState>().active_touch_id(),
        Some(9)
    );
    assert!(app.world().resource::<ImguiInputState>().ime_enabled());
    begin_frame_and_assert(&mut app, |ui| {
        assert_eq!(ui.io().mouse_hovered_viewport(), main_viewport_id);
        assert!(ui.is_key_down(imgui::Key::A));
        assert!(ui.is_mouse_down(imgui::MouseButton::Right));
        assert!(ui.is_mouse_down(imgui::MouseButton::Left));
    });

    app.world_mut().despawn(primary);
    run_input_systems(&mut app);

    let input_state = app.world().resource::<ImguiInputState>();
    assert_eq!(input_state.primary_window_focused(), Some(false));
    assert_eq!(input_state.focused_window(), None);
    assert_eq!(input_state.mouse_hovered_window(), None);
    assert_eq!(input_state.active_touch_id(), None);
    assert!(!input_state.ime_enabled());
    begin_frame_and_assert(&mut app, |ui| {
        assert!(!ui.is_key_down(imgui::Key::A));
        assert!(!ui.is_mouse_down(imgui::MouseButton::Right));
        assert!(!ui.is_mouse_down(imgui::MouseButton::Left));
        assert!(
            ui.mouse_pos()[0] < -1.0e30 && ui.mouse_pos()[1] < -1.0e30,
            "removing the primary window must clear the ImGui mouse position"
        );
        assert_eq!(
            ui.io().mouse_hovered_viewport(),
            imgui::Id::from(0),
            "removing the primary window must clear the hovered viewport id"
        );
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
fn input_stale_touched_viewport_window_clears_touch_mouse_state() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();
    let viewport_id = imgui::Id::from(0x563);
    let secondary = app
        .world_mut()
        .spawn((
            Window {
                position: WindowPosition::At(IVec2::new(120, 180)),
                resolution: WindowResolution::new(640, 480),
                ..Default::default()
            },
            ImguiViewportWindow { viewport_id },
        ))
        .id();

    app.world_mut()
        .resource_mut::<Messages<TouchInput>>()
        .write(TouchInput {
            phase: TouchPhase::Started,
            position: Vec2::new(15.0, 25.0),
            window: secondary,
            force: None,
            id: 7,
        });
    run_input_systems(&mut app);
    begin_frame_and_assert(&mut app, |ui| {
        assert_eq!(ui.mouse_pos(), [15.0, 25.0]);
        assert_eq!(ui.io().mouse_source(), imgui::MouseSource::TouchScreen);
        assert!(ui.is_mouse_down(imgui::MouseButton::Left));
    });

    app.world_mut().despawn(secondary);
    run_input_systems(&mut app);

    let input_state = app.world().resource::<ImguiInputState>();
    assert_eq!(input_state.active_touch_id(), None);
    begin_frame_and_assert(&mut app, |ui| {
        assert!(
            ui.mouse_pos()[0] < -1.0e30 && ui.mouse_pos()[1] < -1.0e30,
            "destroying the touched secondary viewport must clear the ImGui mouse position"
        );
        assert!(!ui.is_mouse_down(imgui::MouseButton::Left));
        assert_eq!(ui.io().mouse_hovered_viewport(), imgui::Id::from(0));
    });

    assert!(app.world().get::<Window>(primary).is_some());
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
fn input_secondary_viewport_window_messages_use_imgui_platform_coordinates_when_viewports_are_enabled()
 {
    let _guard = imgui_context_guard();
    let (mut app, _primary) = app_with_primary_window();
    let viewport_id = imgui::Id::from(0x500);
    let secondary = app
        .world_mut()
        .spawn((
            Window {
                position: WindowPosition::At(IVec2::new(200, 300)),
                resolution: WindowResolution::new(640, 480),
                ..Default::default()
            },
            ImguiViewportWindow { viewport_id },
        ))
        .id();
    {
        let mut window = app.world_mut().get_mut::<Window>(secondary).unwrap();
        window.resolution.set_scale_factor(2.0);
    }
    {
        let mut context = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
        let io = context.context_mut().io_mut();
        io.set_config_flags(io.config_flags() | imgui::ConfigFlags::VIEWPORTS_ENABLE);
    }

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: secondary,
            position: Vec2::new(300.0, 400.0),
            delta: None,
        });
    app.world_mut()
        .resource_mut::<Messages<MouseButtonInput>>()
        .write(MouseButtonInput {
            button: BevyMouseButton::Right,
            state: ButtonState::Pressed,
            window: secondary,
        });
    app.world_mut()
        .resource_mut::<Messages<MouseWheel>>()
        .write(MouseWheel {
            unit: MouseScrollUnit::Pixel,
            x: -24.0,
            y: 24.0,
            window: secondary,
            phase: TouchPhase::Moved,
        });
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
        .resource_mut::<Messages<Ime>>()
        .write(Ime::Commit {
            window: secondary,
            value: "界".to_owned(),
        });
    app.world_mut()
        .resource_mut::<Messages<WindowFocused>>()
        .write(WindowFocused {
            window: secondary,
            focused: true,
        });
    run_input_systems(&mut app);

    begin_frame_and_assert(&mut app, |ui| {
        assert_eq!(ui.mouse_pos(), [400.0, 550.0]);
        assert_eq!(ui.io().mouse_hovered_viewport(), viewport_id);
        assert!(ui.is_mouse_down(imgui::MouseButton::Right));
        assert_eq!(ui.io().mouse_wheel_h(), -1.0);
        assert_eq!(ui.io().mouse_wheel(), 1.0);
        assert!(ui.is_key_down(imgui::Key::X));

        let chars = current_frame_input_chars();
        assert!(chars.contains(&('x' as u32)));
        assert!(chars.contains(&('界' as u32)));
    });
}

#[test]
fn input_cursor_left_from_previous_window_does_not_clear_new_hovered_viewport_position() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();
    let viewport_id = imgui::Id::from(0x550);
    let secondary = app
        .world_mut()
        .spawn((
            Window {
                position: WindowPosition::At(IVec2::new(200, 300)),
                resolution: WindowResolution::new(640, 480),
                ..Default::default()
            },
            ImguiViewportWindow { viewport_id },
        ))
        .id();
    {
        let mut context = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
        let io = context.context_mut().io_mut();
        io.set_config_flags(io.config_flags() | imgui::ConfigFlags::VIEWPORTS_ENABLE);
    }

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: primary,
            position: Vec2::new(10.0, 20.0),
            delta: None,
        });
    run_input_systems(&mut app);
    assert_eq!(
        app.world()
            .resource::<ImguiInputState>()
            .mouse_hovered_window(),
        Some(primary)
    );

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: secondary,
            position: Vec2::new(30.0, 40.0),
            delta: None,
        });
    app.world_mut()
        .resource_mut::<Messages<CursorLeft>>()
        .write(CursorLeft { window: primary });
    run_input_systems(&mut app);

    assert_eq!(
        app.world()
            .resource::<ImguiInputState>()
            .mouse_hovered_window(),
        Some(secondary)
    );
    begin_frame_and_assert(&mut app, |ui| {
        assert_eq!(ui.mouse_pos(), [230.0, 340.0]);
        assert_eq!(ui.io().mouse_hovered_viewport(), viewport_id);
    });
}

#[test]
fn input_stale_hovered_viewport_window_clears_imgui_mouse_hover() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();
    let viewport_id = imgui::Id::from(0x551);
    let secondary = app
        .world_mut()
        .spawn((
            Window {
                position: WindowPosition::At(IVec2::new(200, 300)),
                resolution: WindowResolution::new(640, 480),
                ..Default::default()
            },
            ImguiViewportWindow { viewport_id },
        ))
        .id();
    {
        let mut context = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
        let io = context.context_mut().io_mut();
        io.set_config_flags(io.config_flags() | imgui::ConfigFlags::VIEWPORTS_ENABLE);
    }

    app.world_mut()
        .resource_mut::<Messages<CursorMoved>>()
        .write(CursorMoved {
            window: secondary,
            position: Vec2::new(30.0, 40.0),
            delta: None,
        });
    run_input_systems(&mut app);
    begin_frame_and_assert(&mut app, |ui| {
        assert_eq!(ui.mouse_pos(), [230.0, 340.0]);
        assert_eq!(ui.io().mouse_hovered_viewport(), viewport_id);
    });

    app.world_mut().despawn(secondary);
    run_input_systems(&mut app);

    assert_eq!(
        app.world()
            .resource::<ImguiInputState>()
            .mouse_hovered_window(),
        None
    );
    begin_frame_and_assert(&mut app, |ui| {
        assert!(
            ui.mouse_pos()[0] < -1.0e30 && ui.mouse_pos()[1] < -1.0e30,
            "destroying the hovered secondary viewport must clear the ImGui mouse position"
        );
        assert_eq!(
            ui.io().mouse_hovered_viewport(),
            imgui::Id::from(0),
            "destroying the hovered secondary viewport must clear the hovered viewport id"
        );
    });

    assert!(app.world().get::<Window>(primary).is_some());
}

#[test]
fn input_cursor_entered_tracks_hovered_window_without_requiring_motion() {
    let _guard = imgui_context_guard();
    let (mut app, primary) = app_with_primary_window();

    app.world_mut()
        .resource_mut::<Messages<CursorEntered>>()
        .write(CursorEntered { window: primary });
    run_input_systems(&mut app);

    assert_eq!(
        app.world()
            .resource::<ImguiInputState>()
            .mouse_hovered_window(),
        Some(primary)
    );
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

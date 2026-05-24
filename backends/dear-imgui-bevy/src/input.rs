//! Window input mapping for the Bevy backend.
//!
//! This module maps the Bevy [`PrimaryWindow`] and any Dear ImGui secondary viewport windows into
//! one Dear ImGui context. It translates Bevy's window/input messages into Dear ImGui IO events
//! without consuming or rewriting Bevy's messages. Gameplay systems should use Dear ImGui's capture
//! flags as policy hints instead of expecting this backend to stop Bevy input propagation.

use crate::{ImguiContext, ImguiViewportWindow};
use bevy_app::{App, PreUpdate};
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy_ecs::system::SystemParam;
use bevy_input::ButtonState;
use bevy_input::keyboard::{KeyCode, KeyboardFocusLost, KeyboardInput};
use bevy_input::mouse::{
    MouseButton as BevyMouseButton, MouseButtonInput, MouseScrollUnit, MouseWheel,
};
use bevy_input::touch::{TouchInput, TouchPhase};
use bevy_math::Vec2;
use bevy_window::{
    CursorEntered, CursorIcon, CursorLeft, CursorMoved, Ime, PrimaryWindow, SystemCursorIcon,
    Window, WindowBackendScaleFactorChanged, WindowFocused, WindowPosition, WindowResized,
    WindowScaleFactorChanged,
};
use dear_imgui_rs as imgui;
use std::collections::HashSet;

const INVALID_MOUSE_POS: [f32; 2] = [-f32::MAX, -f32::MAX];

/// System set that injects Bevy window input into Dear ImGui IO.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImguiInputSystems;

/// Runtime state needed to map Bevy input streams into Dear ImGui events.
#[derive(Resource, Debug, Default)]
pub struct ImguiInputState {
    active_touch_id: Option<u64>,
    ime_enabled: bool,
    primary_window_focused: Option<bool>,
    focused_window: Option<Entity>,
    mouse_hovered_window: Option<Entity>,
    pressed_keys: HashSet<imgui::Key>,
    pressed_mouse_buttons: HashSet<imgui::MouseButton>,
}

impl ImguiInputState {
    /// Currently selected touch id for touch-to-mouse translation.
    #[must_use]
    pub fn active_touch_id(&self) -> Option<u64> {
        self.active_touch_id
    }

    /// Whether the last mapped-window IME message left IME enabled.
    #[must_use]
    pub fn ime_enabled(&self) -> bool {
        self.ime_enabled
    }

    /// Last focus state observed for the primary window.
    #[must_use]
    pub fn primary_window_focused(&self) -> Option<bool> {
        self.primary_window_focused
    }

    /// Last Bevy window entity reported as focused by the backend.
    #[must_use]
    pub fn focused_window(&self) -> Option<Entity> {
        self.focused_window
    }

    /// Last Bevy window entity reported as hovered by the OS mouse.
    #[must_use]
    pub fn mouse_hovered_window(&self) -> Option<Entity> {
        self.mouse_hovered_window
    }
}

/// Last-known Dear ImGui capture intent exposed as a Bevy resource.
///
/// Dear ImGui computes these flags while processing a frame. The backend records the latest values
/// seen in IO; game/editor systems can use them to decide whether to act on Bevy input, but the
/// backend itself does not remove or stop Bevy messages.
#[derive(Resource, Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct ImguiInputCapture {
    /// Dear ImGui wants mouse input.
    pub want_capture_mouse: bool,
    /// Dear ImGui wants mouse input, except when a popup close should be allowed through.
    pub want_capture_mouse_unless_popup_close: bool,
    /// Dear ImGui wants keyboard input.
    pub want_capture_keyboard: bool,
    /// Dear ImGui wants text input / IME.
    pub want_text_input: bool,
}

impl ImguiInputCapture {
    fn update_from_io(&mut self, io: &imgui::Io) {
        self.want_capture_mouse = io.want_capture_mouse();
        self.want_capture_mouse_unless_popup_close = io.want_capture_mouse_unless_popup_close();
        self.want_capture_keyboard = io.want_capture_keyboard();
        self.want_text_input = io.want_text_input();
    }
}

pub(crate) fn install_input_mapping(app: &mut App) {
    app.add_message::<WindowResized>()
        .add_message::<WindowScaleFactorChanged>()
        .add_message::<WindowBackendScaleFactorChanged>()
        .add_message::<WindowFocused>()
        .add_message::<CursorEntered>()
        .add_message::<CursorMoved>()
        .add_message::<CursorLeft>()
        .add_message::<Ime>()
        .add_message::<MouseButtonInput>()
        .add_message::<MouseWheel>()
        .add_message::<KeyboardInput>()
        .add_message::<KeyboardFocusLost>()
        .add_message::<TouchInput>()
        .init_resource::<ImguiInputState>()
        .init_resource::<ImguiInputCapture>()
        .add_systems(
            PreUpdate,
            primary_window_input_system.in_set(ImguiInputSystems),
        );
}

/// Translate primary-window Bevy messages into Dear ImGui IO events.
#[allow(clippy::too_many_arguments)]
pub fn primary_window_input_system(
    primary_window: Query<(Entity, &Window), With<PrimaryWindow>>,
    viewport_windows: Query<(Entity, &Window, &ImguiViewportWindow), Without<PrimaryWindow>>,
    mut imgui_context: NonSendMut<ImguiContext>,
    mut input_state: ResMut<ImguiInputState>,
    mut capture: ResMut<ImguiInputCapture>,
    mut messages: ImguiInputMessageReaders,
) {
    let Ok((primary_window_entity, window)) = primary_window.single() else {
        let context = imgui_context.context_mut();
        release_input_for_missing_primary_window(context, &mut input_state);
        *capture = ImguiInputCapture::default();
        discard_unread_messages(
            &mut messages.window_resized,
            &mut messages.window_scale_factor_changed,
            &mut messages.window_backend_scale_factor_changed,
            &mut messages.window_focused,
            &mut messages.cursor_entered,
            &mut messages.cursor_moved,
            &mut messages.cursor_left,
            &mut messages.mouse_button_input,
            &mut messages.mouse_wheel,
            &mut messages.keyboard_input,
            &mut messages.keyboard_focus_lost,
            &mut messages.touch_input,
            &mut messages.ime,
        );
        return;
    };

    let context = imgui_context.context_mut();
    sync_window_metrics(context, window);
    let primary_viewport_id = context.main_viewport().id();
    let primary_window = ImguiInputWindow {
        entity: primary_window_entity,
        position: window.position,
        scale_factor: window.scale_factor(),
        viewport_id: primary_viewport_id,
        is_primary: true,
    };
    prune_stale_window_state(
        context,
        &mut input_state,
        primary_window,
        &viewport_windows,
        window.focused,
    );

    for event in messages
        .window_resized
        .read()
        .filter(|event| event.window == primary_window_entity)
    {
        context
            .io_mut()
            .set_display_size(finite_non_negative_size([event.width, event.height]));
    }

    for event in messages
        .window_scale_factor_changed
        .read()
        .filter(|event| event.window == primary_window_entity)
    {
        set_framebuffer_scale(context, positive_finite_or(event.scale_factor as f32, 1.0));
    }

    for event in messages
        .window_backend_scale_factor_changed
        .read()
        .filter(|event| event.window == primary_window_entity)
    {
        set_framebuffer_scale(context, positive_finite_or(event.scale_factor as f32, 1.0));
    }

    let focus_events = messages
        .window_focused
        .read()
        .filter_map(|event| {
            imgui_window_for_event(event.window, primary_window, &viewport_windows)
                .map(|window| (window, event.focused))
        })
        .collect::<Vec<_>>();
    let keyboard_focus_lost = !messages.keyboard_focus_lost.is_empty();
    if keyboard_focus_lost {
        messages.keyboard_focus_lost.clear();
    }
    if focus_events.is_empty() && !keyboard_focus_lost {
        sync_initial_focus(context, &mut input_state, primary_window, window.focused);
    } else if input_state.primary_window_focused.is_none() {
        input_state.primary_window_focused = Some(window.focused);
    }
    apply_focus_events(context, &mut input_state, &focus_events);

    if keyboard_focus_lost {
        apply_focus_event(context, &mut input_state, primary_window, false);
    }

    for (_event, window) in messages.cursor_entered.read().filter_map(|event| {
        imgui_window_for_event(event.window, primary_window, &viewport_windows)
            .map(|window| (event, window))
    }) {
        input_state.mouse_hovered_window = Some(window.entity);
        let io = context.io_mut();
        io.add_mouse_source_event(imgui::MouseSource::Mouse);
        add_mouse_viewport_event(io, Some(window.viewport_id));
    }

    for event in messages.cursor_moved.read().filter_map(|event| {
        imgui_window_for_event(event.window, primary_window, &viewport_windows)
            .map(|window| (event, window))
    }) {
        let (event, window) = event;
        input_state.mouse_hovered_window = Some(window.entity);
        let mouse_pos = mouse_pos_for_window(context, window, event.position);
        let io = context.io_mut();
        io.add_mouse_source_event(imgui::MouseSource::Mouse);
        add_mouse_viewport_event(io, Some(window.viewport_id));
        io.add_mouse_pos_event(mouse_pos);
    }

    for window in messages
        .cursor_left
        .read()
        .filter_map(|event| imgui_window_for_event(event.window, primary_window, &viewport_windows))
    {
        if input_state
            .mouse_hovered_window
            .is_some_and(|entity| entity != window.entity)
        {
            continue;
        }
        input_state.mouse_hovered_window = None;
        let io = context.io_mut();
        io.add_mouse_source_event(imgui::MouseSource::Mouse);
        add_mouse_viewport_event(io, None);
        io.add_mouse_pos_event(INVALID_MOUSE_POS);
    }

    for event in messages.mouse_button_input.read().filter(|event| {
        imgui_window_for_event(event.window, primary_window, &viewport_windows).is_some()
    }) {
        if let Some(button) = map_bevy_mouse_button(event.button) {
            let pressed = event.state.is_pressed();
            if pressed {
                input_state.pressed_mouse_buttons.insert(button);
            } else {
                input_state.pressed_mouse_buttons.remove(&button);
            }
            let io = context.io_mut();
            io.add_mouse_source_event(imgui::MouseSource::Mouse);
            if let Some(window) =
                imgui_window_for_event(event.window, primary_window, &viewport_windows)
            {
                add_mouse_viewport_event(io, Some(window.viewport_id));
            }
            io.add_mouse_button_event(button, pressed);
        }
    }

    for event in messages.mouse_wheel.read().filter_map(|event| {
        imgui_window_for_event(event.window, primary_window, &viewport_windows)
            .map(|window| (event, window))
    }) {
        let (event, window) = event;
        let io = context.io_mut();
        io.add_mouse_source_event(imgui::MouseSource::Mouse);
        add_mouse_viewport_event(io, Some(window.viewport_id));
        io.add_mouse_wheel_event(normalize_wheel(event.unit, event.x, event.y));
    }

    for event in messages.keyboard_input.read().filter(|event| {
        imgui_window_for_event(event.window, primary_window, &viewport_windows).is_some()
    }) {
        apply_keyboard_input(context, &mut input_state, event);
    }

    for event in messages.touch_input.read().filter_map(|event| {
        imgui_window_for_event(event.window, primary_window, &viewport_windows)
            .map(|window| (event, window))
    }) {
        let (event, window) = event;
        apply_touch_input(context, &mut input_state, event, window);
    }

    for event in messages.ime.read() {
        let window = match event {
            Ime::Preedit { window, .. }
            | Ime::Commit { window, .. }
            | Ime::Enabled { window }
            | Ime::Disabled { window } => *window,
        };
        if imgui_window_for_event(window, primary_window, &viewport_windows).is_some() {
            apply_ime_event(context, &mut input_state, event);
        }
    }

    capture.update_from_io(context.io());
}

#[derive(SystemParam)]
pub struct ImguiInputMessageReaders<'w, 's> {
    window_resized: MessageReader<'w, 's, WindowResized>,
    window_scale_factor_changed: MessageReader<'w, 's, WindowScaleFactorChanged>,
    window_backend_scale_factor_changed: MessageReader<'w, 's, WindowBackendScaleFactorChanged>,
    window_focused: MessageReader<'w, 's, WindowFocused>,
    cursor_entered: MessageReader<'w, 's, CursorEntered>,
    cursor_moved: MessageReader<'w, 's, CursorMoved>,
    cursor_left: MessageReader<'w, 's, CursorLeft>,
    mouse_button_input: MessageReader<'w, 's, MouseButtonInput>,
    mouse_wheel: MessageReader<'w, 's, MouseWheel>,
    keyboard_input: MessageReader<'w, 's, KeyboardInput>,
    keyboard_focus_lost: MessageReader<'w, 's, KeyboardFocusLost>,
    touch_input: MessageReader<'w, 's, TouchInput>,
    ime: MessageReader<'w, 's, Ime>,
}

#[derive(Clone, Copy)]
struct ImguiInputWindow {
    entity: Entity,
    position: WindowPosition,
    scale_factor: f32,
    viewport_id: imgui::Id,
    is_primary: bool,
}

fn imgui_window_for_event(
    entity: Entity,
    primary_window: ImguiInputWindow,
    viewport_windows: &Query<(Entity, &Window, &ImguiViewportWindow), Without<PrimaryWindow>>,
) -> Option<ImguiInputWindow> {
    if entity == primary_window.entity {
        return Some(primary_window);
    }

    let Ok((entity, window, viewport_window)) = viewport_windows.get(entity) else {
        return None;
    };
    Some(ImguiInputWindow {
        entity,
        position: window.position,
        scale_factor: window.scale_factor(),
        viewport_id: viewport_window.viewport_id,
        is_primary: false,
    })
}

fn is_mapped_imgui_window(
    entity: Entity,
    primary_window: ImguiInputWindow,
    viewport_windows: &Query<(Entity, &Window, &ImguiViewportWindow), Without<PrimaryWindow>>,
) -> bool {
    entity == primary_window.entity || viewport_windows.get(entity).is_ok()
}

fn add_mouse_viewport_event(io: &mut imgui::Io, viewport_id: Option<imgui::Id>) {
    if !io
        .config_flags()
        .contains(imgui::ConfigFlags::VIEWPORTS_ENABLE)
    {
        return;
    }
    io.set_backend_flags(io.backend_flags() | imgui::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT);
    io.add_mouse_viewport_event(viewport_id.unwrap_or_default());
}

fn mouse_pos_for_window(
    context: &imgui::Context,
    window: ImguiInputWindow,
    local_pos: Vec2,
) -> [f32; 2] {
    let mut pos = [local_pos.x, local_pos.y];
    if !context
        .io()
        .config_flags()
        .contains(imgui::ConfigFlags::VIEWPORTS_ENABLE)
    {
        return pos;
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    if let Some(origin) = crate::viewport::window_client_origin_logical(
        window.entity,
        &window.position,
        window.scale_factor,
    ) {
        pos[0] += origin[0];
        pos[1] += origin[1];
        return pos;
    }

    let WindowPosition::At(window_pos) = window.position else {
        return pos;
    };
    let scale_factor = positive_finite_or(window.scale_factor, 1.0);
    pos[0] += window_pos.x as f32 / scale_factor;
    pos[1] += window_pos.y as f32 / scale_factor;
    pos
}

fn positive_finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

/// Convert a Bevy mouse button into Dear ImGui's button space.
#[must_use]
pub fn map_bevy_mouse_button(button: BevyMouseButton) -> Option<imgui::MouseButton> {
    match button {
        BevyMouseButton::Left => Some(imgui::MouseButton::Left),
        BevyMouseButton::Right => Some(imgui::MouseButton::Right),
        BevyMouseButton::Middle => Some(imgui::MouseButton::Middle),
        BevyMouseButton::Back => Some(imgui::MouseButton::Extra1),
        BevyMouseButton::Forward => Some(imgui::MouseButton::Extra2),
        BevyMouseButton::Other(_) => None,
    }
}

/// Convert a Dear ImGui mouse cursor into a Bevy window cursor icon.
#[must_use]
pub(crate) fn map_imgui_mouse_cursor(cursor: imgui::MouseCursor) -> Option<CursorIcon> {
    use imgui::MouseCursor as ImguiMouseCursor;

    let system_cursor = match cursor {
        ImguiMouseCursor::None => return None,
        ImguiMouseCursor::Arrow => SystemCursorIcon::Default,
        ImguiMouseCursor::TextInput => SystemCursorIcon::Text,
        ImguiMouseCursor::ResizeAll => SystemCursorIcon::Move,
        ImguiMouseCursor::ResizeNS => SystemCursorIcon::NsResize,
        ImguiMouseCursor::ResizeEW => SystemCursorIcon::EwResize,
        ImguiMouseCursor::ResizeNESW => SystemCursorIcon::NeswResize,
        ImguiMouseCursor::ResizeNWSE => SystemCursorIcon::NwseResize,
        ImguiMouseCursor::Hand => SystemCursorIcon::Pointer,
        ImguiMouseCursor::NotAllowed => SystemCursorIcon::NotAllowed,
    };

    Some(CursorIcon::from(system_cursor))
}

/// Convert a Bevy physical key code into Dear ImGui's key space.
#[must_use]
pub fn map_bevy_key_code(key_code: KeyCode) -> Option<imgui::Key> {
    use KeyCode as B;
    use imgui::Key as I;

    match key_code {
        B::Backquote => Some(I::GraveAccent),
        B::Backslash => Some(I::Backslash),
        B::BracketLeft => Some(I::LeftBracket),
        B::BracketRight => Some(I::RightBracket),
        B::Comma => Some(I::Comma),
        B::Digit0 => Some(I::Key0),
        B::Digit1 => Some(I::Key1),
        B::Digit2 => Some(I::Key2),
        B::Digit3 => Some(I::Key3),
        B::Digit4 => Some(I::Key4),
        B::Digit5 => Some(I::Key5),
        B::Digit6 => Some(I::Key6),
        B::Digit7 => Some(I::Key7),
        B::Digit8 => Some(I::Key8),
        B::Digit9 => Some(I::Key9),
        B::Equal => Some(I::Equal),
        B::IntlBackslash | B::IntlRo | B::IntlYen => Some(I::Oem102),
        B::KeyA => Some(I::A),
        B::KeyB => Some(I::B),
        B::KeyC => Some(I::C),
        B::KeyD => Some(I::D),
        B::KeyE => Some(I::E),
        B::KeyF => Some(I::F),
        B::KeyG => Some(I::G),
        B::KeyH => Some(I::H),
        B::KeyI => Some(I::I),
        B::KeyJ => Some(I::J),
        B::KeyK => Some(I::K),
        B::KeyL => Some(I::L),
        B::KeyM => Some(I::M),
        B::KeyN => Some(I::N),
        B::KeyO => Some(I::O),
        B::KeyP => Some(I::P),
        B::KeyQ => Some(I::Q),
        B::KeyR => Some(I::R),
        B::KeyS => Some(I::S),
        B::KeyT => Some(I::T),
        B::KeyU => Some(I::U),
        B::KeyV => Some(I::V),
        B::KeyW => Some(I::W),
        B::KeyX => Some(I::X),
        B::KeyY => Some(I::Y),
        B::KeyZ => Some(I::Z),
        B::Minus => Some(I::Minus),
        B::Period => Some(I::Period),
        B::Quote => Some(I::Apostrophe),
        B::Semicolon => Some(I::Semicolon),
        B::Slash => Some(I::Slash),
        B::AltLeft => Some(I::LeftAlt),
        B::AltRight => Some(I::RightAlt),
        B::Backspace | B::NumpadBackspace => Some(I::Backspace),
        B::CapsLock => Some(I::CapsLock),
        B::ContextMenu => Some(I::Menu),
        B::ControlLeft => Some(I::LeftCtrl),
        B::ControlRight => Some(I::RightCtrl),
        B::Enter => Some(I::Enter),
        B::SuperLeft | B::Meta => Some(I::LeftSuper),
        B::SuperRight => Some(I::RightSuper),
        B::ShiftLeft => Some(I::LeftShift),
        B::ShiftRight => Some(I::RightShift),
        B::Space => Some(I::Space),
        B::Tab => Some(I::Tab),
        B::Delete => Some(I::Delete),
        B::End => Some(I::End),
        B::Home => Some(I::Home),
        B::Insert => Some(I::Insert),
        B::PageDown => Some(I::PageDown),
        B::PageUp => Some(I::PageUp),
        B::ArrowDown => Some(I::DownArrow),
        B::ArrowLeft => Some(I::LeftArrow),
        B::ArrowRight => Some(I::RightArrow),
        B::ArrowUp => Some(I::UpArrow),
        B::NumLock => Some(I::NumLock),
        B::Numpad0 => Some(I::Keypad0),
        B::Numpad1 => Some(I::Keypad1),
        B::Numpad2 => Some(I::Keypad2),
        B::Numpad3 => Some(I::Keypad3),
        B::Numpad4 => Some(I::Keypad4),
        B::Numpad5 => Some(I::Keypad5),
        B::Numpad6 => Some(I::Keypad6),
        B::Numpad7 => Some(I::Keypad7),
        B::Numpad8 => Some(I::Keypad8),
        B::Numpad9 => Some(I::Keypad9),
        B::NumpadAdd => Some(I::KeypadAdd),
        B::NumpadDecimal | B::NumpadComma => Some(I::KeypadDecimal),
        B::NumpadDivide => Some(I::KeypadDivide),
        B::NumpadEnter => Some(I::KeypadEnter),
        B::NumpadEqual => Some(I::KeypadEqual),
        B::NumpadMultiply | B::NumpadStar => Some(I::KeypadMultiply),
        B::NumpadSubtract => Some(I::KeypadSubtract),
        B::Escape => Some(I::Escape),
        B::PrintScreen => Some(I::PrintScreen),
        B::ScrollLock => Some(I::ScrollLock),
        B::Pause => Some(I::Pause),
        B::F1 => Some(I::F1),
        B::F2 => Some(I::F2),
        B::F3 => Some(I::F3),
        B::F4 => Some(I::F4),
        B::F5 => Some(I::F5),
        B::F6 => Some(I::F6),
        B::F7 => Some(I::F7),
        B::F8 => Some(I::F8),
        B::F9 => Some(I::F9),
        B::F10 => Some(I::F10),
        B::F11 => Some(I::F11),
        B::F12 => Some(I::F12),
        _ => None,
    }
}

fn sync_window_metrics(context: &mut imgui::Context, window: &Window) {
    let io = context.io_mut();
    io.set_display_size(sanitized_window_display_size(window));
    io.set_display_framebuffer_scale(sanitized_window_framebuffer_scale(window));
}

fn set_framebuffer_scale(context: &mut imgui::Context, scale_factor: f32) {
    context
        .io_mut()
        .set_display_framebuffer_scale([scale_factor, scale_factor]);
}

pub(crate) fn sanitized_window_display_size(window: &Window) -> [f32; 2] {
    finite_non_negative_size([window.width(), window.height()])
}

pub(crate) fn sanitized_window_framebuffer_scale(window: &Window) -> [f32; 2] {
    let scale_factor = positive_finite_or(window.scale_factor(), 1.0);
    [scale_factor, scale_factor]
}

fn finite_non_negative_size(size: [f32; 2]) -> [f32; 2] {
    [
        if size[0].is_finite() && size[0] >= 0.0 {
            size[0]
        } else {
            0.0
        },
        if size[1].is_finite() && size[1] >= 0.0 {
            size[1]
        } else {
            0.0
        },
    ]
}

fn sync_initial_focus(
    context: &mut imgui::Context,
    state: &mut ImguiInputState,
    window: ImguiInputWindow,
    focused: bool,
) {
    if state.primary_window_focused == Some(focused) {
        return;
    }
    if !focused
        && state
            .focused_window
            .is_some_and(|entity| entity != window.entity)
    {
        if window.is_primary {
            state.primary_window_focused = Some(false);
        }
        return;
    }
    apply_focus_event(context, state, window, focused);
}

fn prune_stale_window_state(
    context: &mut imgui::Context,
    state: &mut ImguiInputState,
    primary_window: ImguiInputWindow,
    viewport_windows: &Query<(Entity, &Window, &ImguiViewportWindow), Without<PrimaryWindow>>,
    primary_focused: bool,
) {
    let focused_was_stale = state
        .focused_window
        .is_some_and(|entity| !is_mapped_imgui_window(entity, primary_window, viewport_windows));
    if focused_was_stale {
        state.focused_window = primary_focused.then_some(primary_window.entity);
        if !primary_focused {
            state.primary_window_focused = Some(false);
            context.io_mut().add_focus_event(false);
            release_sticky_input(context, state);
        }
    }

    if state
        .mouse_hovered_window
        .is_some_and(|entity| !is_mapped_imgui_window(entity, primary_window, viewport_windows))
    {
        state.mouse_hovered_window = None;
        let io = context.io_mut();
        io.add_mouse_source_event(imgui::MouseSource::Mouse);
        add_mouse_viewport_event(io, None);
        io.add_mouse_pos_event(INVALID_MOUSE_POS);
        clear_mouse_hovered_viewport(io);
    }
}

fn release_input_for_missing_primary_window(
    context: &mut imgui::Context,
    state: &mut ImguiInputState,
) {
    let had_focus = state.primary_window_focused != Some(false) || state.focused_window.is_some();
    let had_mouse_window = state.mouse_hovered_window.is_some();
    let had_pointer_input = had_mouse_window
        || !state.pressed_mouse_buttons.is_empty()
        || state.active_touch_id.is_some();

    if had_focus {
        context.io_mut().add_focus_event(false);
    }
    state.primary_window_focused = Some(false);
    state.focused_window = None;
    state.ime_enabled = false;

    if state.has_sticky_input() {
        release_sticky_input(context, state);
    }

    if had_pointer_input {
        state.mouse_hovered_window = None;
        let io = context.io_mut();
        io.add_mouse_source_event(imgui::MouseSource::Mouse);
        add_mouse_viewport_event(io, None);
        io.add_mouse_pos_event(INVALID_MOUSE_POS);
        clear_mouse_hovered_viewport(io);
    }
}

fn clear_mouse_hovered_viewport(io: &mut imgui::Io) {
    io.set_mouse_hovered_viewport(imgui::Id::from(0));
}

fn apply_focus_event(
    context: &mut imgui::Context,
    state: &mut ImguiInputState,
    window: ImguiInputWindow,
    focused: bool,
) {
    context.io_mut().add_focus_event(focused);
    if window.is_primary {
        state.primary_window_focused = Some(focused);
    }
    state.focused_window = focused.then_some(window.entity);
    if !focused {
        release_sticky_input(context, state);
    }
}

fn apply_focus_events(
    context: &mut imgui::Context,
    state: &mut ImguiInputState,
    events: &[(ImguiInputWindow, bool)],
) {
    if events.is_empty() {
        return;
    }

    let was_focused = state.focused_window.is_some();
    let mut focused_window = state.focused_window;
    for &(window, focused) in events {
        if window.is_primary {
            state.primary_window_focused = Some(focused);
        }
        if focused {
            focused_window = Some(window.entity);
        } else if focused_window == Some(window.entity) {
            focused_window = None;
        }
    }

    // Dear ImGui focus is context-wide: moving focus between mapped OS windows must not look like
    // an application blur, otherwise held keys/buttons are released during intra-app focus changes.
    match (was_focused, focused_window.is_some()) {
        (false, true) => context.io_mut().add_focus_event(true),
        (true, false) => {
            context.io_mut().add_focus_event(false);
            release_sticky_input(context, state);
        }
        _ => {}
    }
    state.focused_window = focused_window;
}

fn apply_keyboard_input(
    context: &mut imgui::Context,
    state: &mut ImguiInputState,
    event: &KeyboardInput,
) {
    let pressed = event.state == ButtonState::Pressed;

    if pressed && let Some(text) = &event.text {
        for ch in text.chars().filter(|ch| *ch != '\u{7f}') {
            context.io_mut().add_input_character(ch);
        }
    }

    if let Some(key) = map_bevy_key_code(event.key_code) {
        if pressed {
            state.pressed_keys.insert(key);
        } else {
            state.pressed_keys.remove(&key);
        }
        let io = context.io_mut();
        sync_modifier_events(io, state);
        io.add_key_event(key, pressed);
    }
}

fn sync_modifier_events(io: &mut imgui::Io, state: &ImguiInputState) {
    io.add_key_event(imgui::Key::ModCtrl, state.any_ctrl_down());
    io.add_key_event(imgui::Key::ModShift, state.any_shift_down());
    io.add_key_event(imgui::Key::ModAlt, state.any_alt_down());
    io.add_key_event(imgui::Key::ModSuper, state.any_super_down());
}

impl ImguiInputState {
    fn has_sticky_input(&self) -> bool {
        !self.pressed_keys.is_empty()
            || !self.pressed_mouse_buttons.is_empty()
            || self.active_touch_id.is_some()
    }

    fn any_ctrl_down(&self) -> bool {
        self.pressed_keys.contains(&imgui::Key::LeftCtrl)
            || self.pressed_keys.contains(&imgui::Key::RightCtrl)
    }

    fn any_shift_down(&self) -> bool {
        self.pressed_keys.contains(&imgui::Key::LeftShift)
            || self.pressed_keys.contains(&imgui::Key::RightShift)
    }

    fn any_alt_down(&self) -> bool {
        self.pressed_keys.contains(&imgui::Key::LeftAlt)
            || self.pressed_keys.contains(&imgui::Key::RightAlt)
    }

    fn any_super_down(&self) -> bool {
        self.pressed_keys.contains(&imgui::Key::LeftSuper)
            || self.pressed_keys.contains(&imgui::Key::RightSuper)
    }
}

fn apply_touch_input(
    context: &mut imgui::Context,
    state: &mut ImguiInputState,
    event: &TouchInput,
    window: ImguiInputWindow,
) {
    match event.phase {
        TouchPhase::Started => {
            if state.active_touch_id.is_none() {
                state.active_touch_id = Some(event.id);
                let mouse_pos = mouse_pos_for_window(context, window, event.position);
                let io = context.io_mut();
                io.add_mouse_source_event(imgui::MouseSource::TouchScreen);
                add_mouse_viewport_event(io, Some(window.viewport_id));
                io.add_mouse_pos_event(mouse_pos);
                io.add_mouse_button_event(imgui::MouseButton::Left, true);
            }
        }
        TouchPhase::Moved => {
            if state.active_touch_id == Some(event.id) {
                let mouse_pos = mouse_pos_for_window(context, window, event.position);
                let io = context.io_mut();
                io.add_mouse_source_event(imgui::MouseSource::TouchScreen);
                add_mouse_viewport_event(io, Some(window.viewport_id));
                io.add_mouse_pos_event(mouse_pos);
            }
        }
        TouchPhase::Ended | TouchPhase::Canceled => {
            if state.active_touch_id == Some(event.id) {
                state.active_touch_id = None;
                let mouse_pos = mouse_pos_for_window(context, window, event.position);
                let io = context.io_mut();
                io.add_mouse_source_event(imgui::MouseSource::TouchScreen);
                add_mouse_viewport_event(io, Some(window.viewport_id));
                io.add_mouse_pos_event(mouse_pos);
                io.add_mouse_button_event(imgui::MouseButton::Left, false);
            }
        }
    }
}

fn apply_ime_event(context: &mut imgui::Context, state: &mut ImguiInputState, event: &Ime) {
    match event {
        Ime::Commit { value, .. } => {
            for ch in value.chars().filter(|ch| !ch.is_control()) {
                context.io_mut().add_input_character(ch);
            }
        }
        Ime::Enabled { .. } => {
            state.ime_enabled = true;
        }
        Ime::Disabled { .. } => {
            state.ime_enabled = false;
        }
        Ime::Preedit { .. } => {}
    }
}

fn normalize_wheel(unit: MouseScrollUnit, x: f32, y: f32) -> [f32; 2] {
    match unit {
        MouseScrollUnit::Line => [x, y],
        MouseScrollUnit::Pixel => [pixel_wheel_step(x), pixel_wheel_step(y)],
    }
}

fn pixel_wheel_step(value: f32) -> f32 {
    match value.partial_cmp(&0.0) {
        Some(std::cmp::Ordering::Greater) => 1.0,
        Some(std::cmp::Ordering::Less) => -1.0,
        _ => 0.0,
    }
}

fn release_sticky_input(context: &mut imgui::Context, state: &mut ImguiInputState) {
    let io = context.io_mut();
    for key in state.pressed_keys.drain() {
        io.add_key_event(key, false);
    }
    io.add_key_event(imgui::Key::ModCtrl, false);
    io.add_key_event(imgui::Key::ModShift, false);
    io.add_key_event(imgui::Key::ModAlt, false);
    io.add_key_event(imgui::Key::ModSuper, false);

    for button in state.pressed_mouse_buttons.drain() {
        io.add_mouse_button_event(button, false);
    }

    if state.active_touch_id.take().is_some() {
        io.add_mouse_source_event(imgui::MouseSource::TouchScreen);
        io.add_mouse_button_event(imgui::MouseButton::Left, false);
    }
}

#[allow(clippy::too_many_arguments)]
fn discard_unread_messages(
    window_resized: &mut MessageReader<WindowResized>,
    window_scale_factor_changed: &mut MessageReader<WindowScaleFactorChanged>,
    window_backend_scale_factor_changed: &mut MessageReader<WindowBackendScaleFactorChanged>,
    window_focused: &mut MessageReader<WindowFocused>,
    cursor_entered: &mut MessageReader<CursorEntered>,
    cursor_moved: &mut MessageReader<CursorMoved>,
    cursor_left: &mut MessageReader<CursorLeft>,
    mouse_button_input: &mut MessageReader<MouseButtonInput>,
    mouse_wheel: &mut MessageReader<MouseWheel>,
    keyboard_input: &mut MessageReader<KeyboardInput>,
    keyboard_focus_lost: &mut MessageReader<KeyboardFocusLost>,
    touch_input: &mut MessageReader<TouchInput>,
    ime: &mut MessageReader<Ime>,
) {
    window_resized.clear();
    window_scale_factor_changed.clear();
    window_backend_scale_factor_changed.clear();
    window_focused.clear();
    cursor_entered.clear();
    cursor_moved.clear();
    cursor_left.clear();
    mouse_button_input.clear();
    mouse_wheel.clear();
    keyboard_input.clear();
    keyboard_focus_lost.clear();
    touch_input.clear();
    ime.clear();
}

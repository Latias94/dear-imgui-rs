use bevy_ecs::prelude::{Entity, NonSendMut, Query, ResMut, With, Without};
use bevy_input::ButtonState;
use bevy_input::keyboard::KeyboardInput;
use bevy_input::touch::{TouchInput, TouchPhase};
use bevy_window::{Ime, PrimaryWindow, Window};
use dear_imgui_rs as imgui;

use crate::viewport::ImguiViewportOwner;
use crate::{ContextId, ImguiContextError, ImguiContexts, ImguiViewportWindow};

use super::capture_api::ImguiInputCapture;
use super::common::{
    INVALID_MOUSE_POS, add_ime_text, add_keyboard_text, add_mouse_viewport_event,
    apply_modifier_events, clear_mouse_hovered_viewport, finite_non_negative_size,
    map_bevy_key_code, map_bevy_mouse_button, modifier_state, mouse_pos_for_window,
    normalize_wheel, positive_finite_or, release_sticky_keys_and_buttons, set_framebuffer_scale,
    sync_window_metrics,
};
use super::events::{ImguiInputMessageReaders, discard_all_unread_messages};
use super::state::{ImguiInputState, ImguiInputWindow};

/// Translate primary-window Bevy messages into Dear ImGui IO events.
#[allow(clippy::too_many_arguments)]
#[cfg(not(feature = "render"))]
pub(crate) fn primary_window_input_system(
    primary_window: Query<(Entity, &Window), With<PrimaryWindow>>,
    viewport_windows: Query<
        (Entity, &Window, &ImguiViewportWindow, &ImguiViewportOwner),
        Without<PrimaryWindow>,
    >,
    contexts: Option<NonSendMut<ImguiContexts>>,
    mut input_state: ResMut<ImguiInputState>,
    mut capture: ResMut<ImguiInputCapture>,
    mut messages: ImguiInputMessageReaders,
) {
    let Some(mut contexts) = contexts else {
        *input_state = ImguiInputState::default();
        *capture = ImguiInputCapture::default();
        discard_all_unread_messages(&mut messages);
        return;
    };
    let Some(primary_id) = contexts.primary_id() else {
        *capture = ImguiInputCapture::default();
        discard_all_unread_messages(&mut messages);
        return;
    };

    let result = contexts.configure(primary_id, |context| {
        translate_primary_window_input(
            &primary_window,
            &viewport_windows,
            primary_id,
            context,
            &mut input_state,
            &mut capture,
            &mut messages,
        );
    });
    match result {
        Ok(()) => {}
        Err(
            ImguiContextError::TeardownInProgress { .. } | ImguiContextError::UnknownContext { .. },
        ) => {
            *capture = ImguiInputCapture::default();
            discard_all_unread_messages(&mut messages);
        }
        Err(error) => {
            panic!("dear-imgui-bevy could not prepare primary Context input: {error}")
        }
    }
}

#[cfg(not(feature = "render"))]
fn translate_primary_window_input(
    primary_window: &Query<(Entity, &Window), With<PrimaryWindow>>,
    viewport_windows: &Query<
        (Entity, &Window, &ImguiViewportWindow, &ImguiViewportOwner),
        Without<PrimaryWindow>,
    >,
    primary_context: ContextId,
    context: &mut imgui::Context,
    input_state: &mut ImguiInputState,
    capture: &mut ImguiInputCapture,
    messages: &mut ImguiInputMessageReaders,
) {
    let Ok((primary_window_entity, window)) = primary_window.single() else {
        release_input_for_missing_primary_window(context, input_state);
        *capture = ImguiInputCapture::default();
        discard_all_unread_messages(messages);
        return;
    };

    sync_window_metrics(context, window);
    let primary_viewport_id = context.main_viewport().id();
    let primary_window = ImguiInputWindow {
        entity: primary_window_entity,
        position: window.position,
        scale_factor: window.scale_factor(),
        viewport_id: primary_viewport_id,
        context_id: primary_context,
        is_primary: true,
    };
    prune_stale_window_state(
        context,
        input_state,
        primary_window,
        viewport_windows,
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
            imgui_window_for_event(event.window, primary_window, viewport_windows)
                .map(|window| (window, event.focused))
        })
        .collect::<Vec<_>>();
    let keyboard_focus_lost = !messages.keyboard_focus_lost.is_empty();
    if keyboard_focus_lost {
        messages.keyboard_focus_lost.clear();
    }
    if focus_events.is_empty() && !keyboard_focus_lost {
        sync_initial_focus(context, input_state, primary_window, window.focused);
    } else if input_state.primary_window_focused.is_none() {
        input_state.primary_window_focused = Some(window.focused);
    }
    apply_focus_events(context, input_state, &focus_events);

    if keyboard_focus_lost {
        apply_focus_event(context, input_state, primary_window, false);
    }

    for (_event, window) in messages.cursor_entered.read().filter_map(|event| {
        imgui_window_for_event(event.window, primary_window, viewport_windows)
            .map(|window| (event, window))
    }) {
        input_state.mouse_hovered_window = Some(window.entity);
        let io = context.io_mut();
        io.add_mouse_source_event(imgui::MouseSource::Mouse);
        add_mouse_viewport_event(io, Some(window.viewport_id));
    }

    for event in messages.cursor_moved.read().filter_map(|event| {
        imgui_window_for_event(event.window, primary_window, viewport_windows)
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
        .filter_map(|event| imgui_window_for_event(event.window, primary_window, viewport_windows))
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
        imgui_window_for_event(event.window, primary_window, viewport_windows).is_some()
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
                imgui_window_for_event(event.window, primary_window, viewport_windows)
            {
                add_mouse_viewport_event(io, Some(window.viewport_id));
            }
            io.add_mouse_button_event(button, pressed);
        }
    }

    for event in messages.mouse_wheel.read().filter_map(|event| {
        imgui_window_for_event(event.window, primary_window, viewport_windows)
            .map(|window| (event, window))
    }) {
        let (event, window) = event;
        let io = context.io_mut();
        io.add_mouse_source_event(imgui::MouseSource::Mouse);
        add_mouse_viewport_event(io, Some(window.viewport_id));
        io.add_mouse_wheel_event(normalize_wheel(event.unit, event.x, event.y));
    }

    for event in messages.keyboard_input.read().filter(|event| {
        imgui_window_for_event(event.window, primary_window, viewport_windows).is_some()
    }) {
        apply_keyboard_input(context, input_state, event);
    }

    for event in messages.touch_input.read().filter_map(|event| {
        imgui_window_for_event(event.window, primary_window, viewport_windows)
            .map(|window| (event, window))
    }) {
        let (event, window) = event;
        apply_touch_input(context, input_state, event, window);
    }

    for event in messages.ime.read() {
        let window = match event {
            Ime::Preedit { window, .. }
            | Ime::Commit { window, .. }
            | Ime::Enabled { window }
            | Ime::Disabled { window } => *window,
        };
        if imgui_window_for_event(window, primary_window, viewport_windows).is_some() {
            apply_ime_event(context, input_state, event);
        }
    }

    capture.update_from_io(primary_context, primary_window_entity, context.io());
}

#[cfg(not(feature = "render"))]
fn imgui_window_for_event(
    entity: Entity,
    primary_window: ImguiInputWindow,
    viewport_windows: &Query<
        (Entity, &Window, &ImguiViewportWindow, &ImguiViewportOwner),
        Without<PrimaryWindow>,
    >,
) -> Option<ImguiInputWindow> {
    if entity == primary_window.entity {
        return Some(primary_window);
    }

    let Ok((entity, window, viewport_window, viewport_owner)) = viewport_windows.get(entity) else {
        return None;
    };
    if !viewport_owner.matches_window(viewport_window)
        || viewport_window.context_id() != primary_window.context_id
    {
        return None;
    }
    Some(ImguiInputWindow {
        entity,
        position: window.position,
        scale_factor: window.scale_factor(),
        viewport_id: viewport_window.viewport_id(),
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        desktop_origin: None,
        context_id: viewport_window.context_id(),
        is_primary: false,
    })
}

#[cfg(not(feature = "render"))]
fn is_mapped_imgui_window(
    entity: Entity,
    primary_window: ImguiInputWindow,
    viewport_windows: &Query<
        (Entity, &Window, &ImguiViewportWindow, &ImguiViewportOwner),
        Without<PrimaryWindow>,
    >,
) -> bool {
    entity == primary_window.entity
        || viewport_windows
            .get(entity)
            .is_ok_and(|(_, _, marker, owner)| {
                owner.matches_window(marker) && marker.context_id() == primary_window.context_id
            })
}

#[cfg(not(feature = "render"))]
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

#[cfg(not(feature = "render"))]
fn prune_stale_window_state(
    context: &mut imgui::Context,
    state: &mut ImguiInputState,
    primary_window: ImguiInputWindow,
    viewport_windows: &Query<
        (Entity, &Window, &ImguiViewportWindow, &ImguiViewportOwner),
        Without<PrimaryWindow>,
    >,
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

    if state
        .active_touch_window
        .is_some_and(|entity| !is_mapped_imgui_window(entity, primary_window, viewport_windows))
    {
        state.active_touch_id = None;
        state.active_touch_window = None;
        let io = context.io_mut();
        io.add_mouse_source_event(imgui::MouseSource::TouchScreen);
        io.add_mouse_pos_event(INVALID_MOUSE_POS);
        io.add_mouse_button_event(imgui::MouseButton::Left, false);
        add_mouse_viewport_event(io, None);
        clear_mouse_hovered_viewport(io);
    }
}

#[cfg(not(feature = "render"))]
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

#[cfg(not(feature = "render"))]
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

#[cfg(not(feature = "render"))]
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

#[cfg(not(feature = "render"))]
fn apply_keyboard_input(
    context: &mut imgui::Context,
    state: &mut ImguiInputState,
    event: &KeyboardInput,
) {
    let pressed = event.state == ButtonState::Pressed;

    if pressed && let Some(text) = &event.text {
        add_keyboard_text(context.io_mut(), text);
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

#[cfg(not(feature = "render"))]
fn sync_modifier_events(io: &mut imgui::Io, state: &ImguiInputState) {
    apply_modifier_events(io, modifier_state(&state.pressed_keys));
}

#[cfg(not(feature = "render"))]
impl ImguiInputState {
    fn has_sticky_input(&self) -> bool {
        !self.pressed_keys.is_empty()
            || !self.pressed_mouse_buttons.is_empty()
            || self.active_touch_id.is_some()
    }
}

#[cfg(not(feature = "render"))]
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
                state.active_touch_window = Some(window.entity);
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
                state.active_touch_window = Some(window.entity);
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
                state.active_touch_window = None;
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

#[cfg(not(feature = "render"))]
fn apply_ime_event(context: &mut imgui::Context, state: &mut ImguiInputState, event: &Ime) {
    match event {
        Ime::Commit { value, .. } => {
            add_ime_text(context.io_mut(), value);
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

#[cfg(not(feature = "render"))]
fn release_sticky_input(context: &mut imgui::Context, state: &mut ImguiInputState) {
    let io = context.io_mut();
    release_sticky_keys_and_buttons(
        io,
        &mut state.pressed_keys,
        &mut state.pressed_mouse_buttons,
    );

    if state.active_touch_id.take().is_some() {
        state.active_touch_window = None;
        io.add_mouse_source_event(imgui::MouseSource::TouchScreen);
        io.add_mouse_button_event(imgui::MouseButton::Left, false);
    }
}

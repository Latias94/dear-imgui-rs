use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::{Entity, NonSendMut, Query, Res, ResMut};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_ecs::system::NonSend;
use bevy_input::keyboard::KeyboardInput;
use bevy_input::mouse::MouseButton as BevyMouseButton;
use bevy_input::touch::{TouchInput, TouchPhase};
use bevy_math::{Rect, Vec2};
use bevy_window::{Ime, Window};
use dear_imgui_rs as imgui;

use crate::route::{ImguiInputPolicy, ImguiResolvedInputRoute, ImguiResolvedRoutes};
use crate::{ContextId, ImguiContextError, ImguiContexts};

use super::RoutedInputWindowComponents;
use super::capture_api::ImguiInputCapture;
use super::common::{
    INVALID_MOUSE_POS, add_ime_text, add_keyboard_text, add_mouse_viewport_event,
    apply_modifier_events, clear_mouse_hovered_viewport, finite_non_negative_size,
    map_bevy_key_code, map_bevy_mouse_button, normalize_wheel, positive_finite_or,
    release_sticky_keys_and_buttons, sanitized_window_display_size,
    sanitized_window_framebuffer_scale,
};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use super::event_ingest::collect_raw_winit_pointer_events;
use super::event_ingest::{OrderedPointerEvent, append_typed_pointer_event};
use super::events::{ImguiInputMessageReaders, discard_all_unread_messages};
use super::route::{
    ImguiFrameInput, ImguiFrameInputSlot, ImguiInputFrameMetrics, ImguiInputSlot,
    ImguiRoutedWindowState, RoutedInputState, RoutedInputTarget,
};
use super::state::{ImguiInputState, ImguiInputWindow};

#[cfg(feature = "render")]
#[allow(clippy::too_many_arguments)]
pub(super) fn routed_window_input_system(
    windows: Query<RoutedInputWindowComponents>,
    resolved_routes: Res<ImguiResolvedRoutes>,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))] viewport_bridge: NonSend<
        crate::ImguiViewportBridge,
    >,
    contexts: Option<NonSendMut<ImguiContexts>>,
    mut input_state: ResMut<ImguiInputState>,
    mut capture: ResMut<ImguiInputCapture>,
    mut frame_input: ResMut<ImguiFrameInputSlot>,
    mut messages: ImguiInputMessageReaders,
) {
    let Some(mut contexts) = contexts else {
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        crate::viewport::native_window::release_pointer_capture();
        *input_state = ImguiInputState::default();
        *capture = ImguiInputCapture::default();
        frame_input.publish(ImguiFrameInput::new(
            resolved_routes.render_epoch(),
            HashMap::new(),
            HashMap::new(),
        ));
        discard_all_unread_messages(&mut messages);
        return;
    };
    let Ok(primary_context) = contexts.primary_id() else {
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        crate::viewport::native_window::release_pointer_capture();
        *input_state = ImguiInputState::default();
        *capture = ImguiInputCapture::default();
        frame_input.publish(ImguiFrameInput::new(
            resolved_routes.render_epoch(),
            HashMap::new(),
            HashMap::new(),
        ));
        discard_all_unread_messages(&mut messages);
        return;
    };
    input_state.routed.primary_context = primary_context;
    input_state.routed.primary_window = windows
        .iter()
        .find_map(|(entity, _, primary_window, _, _)| primary_window.is_some().then_some(entity));

    let mut targets = Vec::new();
    for route in resolved_routes.input_routes().iter().copied() {
        let Ok((_, window, _, _, _)) = windows.get(route.host_window()) else {
            continue;
        };
        let target = routed_target_for_route(route, &resolved_routes, window);
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        {
            let mut target = target;
            if route_covers_window(route, window)
                && let Some(viewport_id) =
                    viewport_bridge.viewport_for_window(route.context_id(), route.host_window())
            {
                target.native_viewport = Some(ImguiInputWindow {
                    entity: route.host_window(),
                    scale_factor: window.scale_factor(),
                    viewport_id,
                    desktop_origin: viewport_bridge.viewport_desktop_origin_for_window(
                        route.context_id(),
                        route.host_window(),
                    ),
                });
            }
            targets.push(target);
        }
        #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
        targets.push(target);
    }

    let declared_slots = targets
        .iter()
        .map(|target| target.slot())
        .collect::<HashSet<_>>();
    let input_enabled_contexts = targets
        .iter()
        .filter(|target| !matches!(target.policy, ImguiInputPolicy::Disabled))
        .map(|target| target.context_id)
        .collect::<HashSet<_>>();
    for (entity, window, primary_window, viewport_window, viewport_owner) in &windows {
        let (Some(viewport_window), Some(viewport_owner)) = (viewport_window, viewport_owner)
        else {
            continue;
        };
        if !viewport_owner.matches_window(viewport_window) {
            continue;
        }
        let context_id = viewport_window.context_id();
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        if viewport_bridge.viewport_window(context_id, viewport_window.viewport_id())
            != Some(entity)
        {
            continue;
        }
        if primary_window.is_some()
            || !matches!(contexts.contains(context_id), Ok(true))
            || !input_enabled_contexts.contains(&context_id)
            || declared_slots.contains(&ImguiInputSlot {
                context_id,
                window: entity,
            })
        {
            continue;
        }
        targets.push(RoutedInputTarget {
            context_id,
            host_window: entity,
            logical_region: Rect::from_corners(
                Vec2::ZERO,
                Vec2::new(
                    finite_non_negative_size([window.width(), window.height()])[0],
                    finite_non_negative_size([window.width(), window.height()])[1],
                ),
            ),
            policy: ImguiInputPolicy::Exclusive { priority: i32::MAX },
            display_size: sanitized_window_display_size(window),
            framebuffer_scale: sanitized_window_framebuffer_scale(window),
            tracks_host_metrics: false,
            native_viewport: Some(ImguiInputWindow {
                #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                entity,
                #[cfg(any(
                    not(feature = "render"),
                    all(feature = "multi-viewport", not(target_arch = "wasm32"))
                ))]
                scale_factor: window.scale_factor(),
                viewport_id: viewport_window.viewport_id(),
                #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                desktop_origin: viewport_bridge
                    .viewport_desktop_origin_for_window(context_id, entity),
            }),
        });
    }

    let capture_routes = targets
        .iter()
        .map(|target| (target.context_id, target.host_window))
        .collect::<Vec<_>>();
    capture.begin_routes(primary_context, &capture_routes);

    let mut platform_feedback_hosts = HashMap::new();
    for target in targets
        .iter()
        .filter(|target| !matches!(target.policy, ImguiInputPolicy::Disabled))
    {
        platform_feedback_hosts
            .entry(target.context_id)
            .or_insert(target.host_window);
    }

    let mut unavailable_contexts = HashSet::new();
    release_stale_routed_input(
        &mut contexts,
        &mut input_state,
        &targets,
        &mut unavailable_contexts,
    );

    let mut context_metrics = HashMap::new();
    for target in targets
        .iter()
        .copied()
        .filter(|target| target.tracks_host_metrics || !target.is_native_viewport())
    {
        context_metrics.insert(
            target.context_id,
            ImguiInputFrameMetrics {
                host_window: target.host_window,
                display_size: target.display_size,
                framebuffer_scale: target.framebuffer_scale,
            },
        );
        configure_routed_context(
            &mut contexts,
            target.context_id,
            &mut unavailable_contexts,
            |context| sync_routed_metrics(context, target),
        );
    }

    let mut focus_updates = messages
        .window_focused
        .read()
        .map(|event| {
            let targets_for_window = if event.focused {
                focus_targets_for_window(&input_state, &targets, event.window)
            } else {
                Vec::new()
            };
            (event.window, targets_for_window)
        })
        .collect::<Vec<_>>();
    if !messages.keyboard_focus_lost.is_empty() {
        messages.keyboard_focus_lost.clear();
        focus_updates.extend(
            input_state
                .routed
                .focused_targets
                .keys()
                .copied()
                .map(|window| (window, Vec::new())),
        );
    }
    let focus_message_windows = focus_updates
        .iter()
        .map(|(window, _)| *window)
        .collect::<HashSet<_>>();
    for (window, window_state, primary_window, _, _) in &windows {
        if primary_window.is_none() || focus_message_windows.contains(&window) {
            continue;
        }
        if window_state.focused {
            if input_state.routed.focused_targets.contains_key(&window) {
                continue;
            }
            let targets_for_window = default_focus_targets(&targets, window);
            if !targets_for_window.is_empty() {
                focus_updates.push((window, targets_for_window));
            }
        } else if input_state.routed.focused_targets.contains_key(&window) {
            focus_updates.push((window, Vec::new()));
        }
    }
    replace_routed_focus_targets(
        &mut contexts,
        &mut input_state,
        &focus_updates,
        &mut unavailable_contexts,
    );

    for event in messages.window_resized.read() {
        apply_routed_resize(
            &mut contexts,
            &targets,
            event.window,
            [event.width, event.height],
            &mut context_metrics,
            &mut unavailable_contexts,
        );
    }
    for event in messages.window_scale_factor_changed.read() {
        apply_routed_scale_factor(
            &mut contexts,
            &targets,
            event.window,
            positive_finite_or(event.scale_factor as f32, 1.0),
            &mut context_metrics,
            &mut unavailable_contexts,
        );
    }
    for event in messages.window_backend_scale_factor_changed.read() {
        apply_routed_scale_factor(
            &mut contexts,
            &targets,
            event.window,
            positive_finite_or(event.scale_factor as f32, 1.0),
            &mut context_metrics,
            &mut unavailable_contexts,
        );
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    // Bevy captures each logical position while handling the Winit event, before later events in
    // the same batch can change the Window's effective scale factor.
    let typed_cursor_moved = messages.cursor_moved.read().cloned().collect::<Vec<_>>();

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    let (mut ordered_pointer_events, mut raw_pointer_duplicates) = collect_raw_winit_pointer_events(
        &mut messages.raw_winit_window,
        &windows,
        &mut input_state.routed.raw_window_scale_factors,
        &typed_cursor_moved,
    );
    #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
    let (mut ordered_pointer_events, mut raw_pointer_duplicates) = (Vec::new(), HashMap::new());

    for event in messages.cursor_entered.read() {
        append_typed_pointer_event(
            &mut ordered_pointer_events,
            &mut raw_pointer_duplicates,
            OrderedPointerEvent::Entered {
                window: event.window,
            },
        );
    }
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    let cursor_moved = typed_cursor_moved.iter();
    #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
    let cursor_moved = messages.cursor_moved.read();
    for event in cursor_moved {
        append_typed_pointer_event(
            &mut ordered_pointer_events,
            &mut raw_pointer_duplicates,
            OrderedPointerEvent::Moved {
                window: event.window,
                position: event.position,
                #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                native_position: None,
            },
        );
    }
    for event in messages.cursor_left.read() {
        append_typed_pointer_event(
            &mut ordered_pointer_events,
            &mut raw_pointer_duplicates,
            OrderedPointerEvent::Left {
                window: event.window,
            },
        );
    }
    for event in messages.mouse_button_input.read() {
        append_typed_pointer_event(
            &mut ordered_pointer_events,
            &mut raw_pointer_duplicates,
            OrderedPointerEvent::Button {
                window: event.window,
                button: event.button,
                state: event.state,
            },
        );
    }

    for event in ordered_pointer_events {
        apply_routed_pointer_event(
            &mut contexts,
            &mut input_state,
            &targets,
            event,
            &mut unavailable_contexts,
        );
    }

    for event in messages.mouse_wheel.read() {
        for target in refresh_routed_pointer_from_cached_position(
            &mut contexts,
            &mut input_state,
            &targets,
            event.window,
            &mut unavailable_contexts,
        ) {
            configure_routed_context(
                &mut contexts,
                target.context_id,
                &mut unavailable_contexts,
                |context| {
                    let viewport_id = target.viewport_id(context);
                    let io = context.io_mut();
                    io.add_mouse_source_event(imgui::MouseSource::Mouse);
                    add_mouse_viewport_event(io, Some(viewport_id));
                    io.add_mouse_wheel_event(normalize_wheel(event.unit, event.x, event.y));
                },
            );
        }
    }

    for event in messages.touch_input.read() {
        apply_routed_touch_input(
            &mut contexts,
            &mut input_state,
            &targets,
            event,
            &mut unavailable_contexts,
        );
    }

    for event in messages.keyboard_input.read() {
        for target in focused_targets_for_window(&input_state, &targets, event.window) {
            apply_routed_keyboard_input(
                &mut contexts,
                &mut input_state,
                target,
                event,
                &mut unavailable_contexts,
            );
        }
    }

    for event in messages.ime.read() {
        let window = ime_window(event);
        for target in focused_targets_for_window(&input_state, &targets, window) {
            apply_routed_ime_event(
                &mut contexts,
                &mut input_state,
                target,
                event,
                &mut unavailable_contexts,
            );
        }
    }

    let capture_contexts = capture_routes
        .iter()
        .map(|(context_id, _)| *context_id)
        .collect::<HashSet<_>>();
    for context_id in capture_contexts {
        configure_routed_context(
            &mut contexts,
            context_id,
            &mut unavailable_contexts,
            |context| capture.update_context(context_id, context.io()),
        );
    }
    prune_unavailable_routed_contexts(&mut input_state, &mut capture, &unavailable_contexts);
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    if !routed_has_pressed_mouse_buttons(&input_state) {
        crate::viewport::native_window::release_pointer_capture();
    }
    capture.finish_routes();
    frame_input.publish(ImguiFrameInput::new(
        resolved_routes.render_epoch(),
        context_metrics,
        platform_feedback_hosts,
    ));
}

#[cfg(feature = "render")]
fn routed_target_for_route(
    route: ImguiResolvedInputRoute,
    resolved_routes: &ImguiResolvedRoutes,
    host_window: &Window,
) -> RoutedInputTarget {
    let fallback_scale = sanitized_window_framebuffer_scale(host_window);
    let render_route = resolved_routes.render_route(route.context_id());
    let (display_size, framebuffer_scale) = render_route
        .map(|render_route| {
            let physical = render_route.physical_output_size();
            let scale_factor = positive_finite_or(render_route.target_info().scale_factor, 1.0);
            (
                [
                    physical.x as f32 / scale_factor,
                    physical.y as f32 / scale_factor,
                ],
                [scale_factor, scale_factor],
            )
        })
        .unwrap_or((sanitized_window_display_size(host_window), fallback_scale));
    RoutedInputTarget {
        context_id: route.context_id(),
        host_window: route.host_window(),
        logical_region: route.logical_region(),
        policy: route.policy(),
        display_size: finite_non_negative_size(display_size),
        framebuffer_scale,
        tracks_host_metrics: route.source().as_camera().is_some() || render_route.is_none(),
        native_viewport: None,
    }
}

#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
fn route_covers_window(route: ImguiResolvedInputRoute, window: &Window) -> bool {
    let region = route.logical_region();
    let size = sanitized_window_display_size(window);
    region.min == Vec2::ZERO
        && (region.max.x - size[0]).abs() <= 0.5
        && (region.max.y - size[1]).abs() <= 0.5
}

#[cfg(feature = "render")]
fn configure_routed_context(
    contexts: &mut ImguiContexts,
    context_id: ContextId,
    unavailable_contexts: &mut HashSet<ContextId>,
    operation: impl FnOnce(&mut imgui::Context),
) -> bool {
    if unavailable_contexts.contains(&context_id) {
        return false;
    }
    match contexts.configure(context_id, operation) {
        Ok(()) => true,
        Err(
            ImguiContextError::TeardownInProgress { .. } | ImguiContextError::UnknownContext { .. },
        ) => {
            unavailable_contexts.insert(context_id);
            false
        }
        Err(error) => panic!("dear-imgui-bevy could not prepare routed Context input: {error}"),
    }
}

#[cfg(feature = "render")]
fn sync_routed_metrics(context: &mut imgui::Context, target: RoutedInputTarget) {
    let io = context.io_mut();
    io.set_display_size(target.display_size);
    io.set_display_framebuffer_scale(target.framebuffer_scale);
}

#[cfg(feature = "render")]
fn pointer_targets_for_position(
    targets: &[RoutedInputTarget],
    host_window: Entity,
    position: Vec2,
) -> Vec<RoutedInputTarget> {
    let highest_exclusive = targets
        .iter()
        .filter(|target| target.host_window == host_window && target.contains(position))
        .filter_map(|target| target.policy.priority())
        .max();
    targets
        .iter()
        .copied()
        .filter(|target| target.host_window == host_window && target.contains(position))
        .filter(|target| match target.policy {
            ImguiInputPolicy::Exclusive { priority } => highest_exclusive == Some(priority),
            ImguiInputPolicy::Shared => true,
            ImguiInputPolicy::Disabled => false,
        })
        .collect()
}

#[cfg(feature = "render")]
fn default_pointer_targets(
    targets: &[RoutedInputTarget],
    host_window: Entity,
) -> Vec<RoutedInputTarget> {
    let exclusive_count = targets
        .iter()
        .filter(|target| {
            target.host_window == host_window
                && matches!(target.policy, ImguiInputPolicy::Exclusive { .. })
        })
        .count();
    targets
        .iter()
        .copied()
        .filter(|target| target.host_window == host_window)
        .filter(|target| {
            matches!(target.policy, ImguiInputPolicy::Shared)
                || (exclusive_count == 1
                    && matches!(target.policy, ImguiInputPolicy::Exclusive { .. }))
        })
        .collect()
}

#[cfg(feature = "render")]
fn targets_for_contexts(
    targets: &[RoutedInputTarget],
    host_window: Entity,
    context_ids: &[ContextId],
) -> Vec<RoutedInputTarget> {
    targets
        .iter()
        .copied()
        .filter(|target| {
            target.host_window == host_window && context_ids.contains(&target.context_id)
        })
        .collect()
}

#[cfg(feature = "render")]
fn pointer_targets_for_window_without_position(
    state: &ImguiInputState,
    targets: &[RoutedInputTarget],
    host_window: Entity,
) -> Vec<RoutedInputTarget> {
    if let Some(position) = state.routed.pointer_positions.get(&host_window) {
        return pointer_targets_for_position(targets, host_window, *position);
    }
    let pointer_contexts = state
        .routed
        .pointer_targets
        .get(&host_window)
        .filter(|contexts| !contexts.is_empty());
    pointer_contexts.map_or_else(
        || default_pointer_targets(targets, host_window),
        |contexts| targets_for_contexts(targets, host_window, contexts),
    )
}

#[cfg(feature = "render")]
fn refresh_routed_pointer_from_cached_position(
    contexts: &mut ImguiContexts,
    state: &mut ImguiInputState,
    targets: &[RoutedInputTarget],
    host_window: Entity,
    unavailable_contexts: &mut HashSet<ContextId>,
) -> Vec<RoutedInputTarget> {
    let Some(position) = state.routed.pointer_positions.get(&host_window).copied() else {
        return pointer_targets_for_window_without_position(state, targets, host_window);
    };
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    let native_position = state
        .routed
        .raw_native_pointer_positions
        .get(&host_window)
        .copied();
    let selected = pointer_targets_for_position(targets, host_window, position);
    replace_routed_pointer_targets(
        contexts,
        state,
        host_window,
        selected.iter().map(|target| target.context_id).collect(),
        unavailable_contexts,
    );
    for target in selected.iter().copied() {
        mark_routed_hovered(state, target, true);
        configure_routed_context(
            contexts,
            target.context_id,
            unavailable_contexts,
            |context| {
                #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                let position = if target.is_native_viewport() {
                    native_position.unwrap_or(position)
                } else {
                    position
                };
                add_routed_pointer_position(context, target, position, imgui::MouseSource::Mouse)
            },
        );
    }
    selected
}

#[cfg(feature = "render")]
fn apply_routed_pointer_event(
    contexts: &mut ImguiContexts,
    state: &mut ImguiInputState,
    targets: &[RoutedInputTarget],
    event: OrderedPointerEvent,
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    match event {
        OrderedPointerEvent::Entered { window } => {
            state.routed.pointer_outside_windows.remove(&window);
            clear_other_outside_window_pointers(contexts, state, window, unavailable_contexts);
            let selected = pointer_targets_for_window_without_position(state, targets, window);
            replace_routed_pointer_targets(
                contexts,
                state,
                window,
                selected.iter().map(|target| target.context_id).collect(),
                unavailable_contexts,
            );
            for target in selected {
                mark_routed_hovered(state, target, true);
                configure_routed_context(
                    contexts,
                    target.context_id,
                    unavailable_contexts,
                    |context| {
                        let viewport_id = target.viewport_id(context);
                        let io = context.io_mut();
                        io.add_mouse_source_event(imgui::MouseSource::Mouse);
                        add_mouse_viewport_event(io, Some(viewport_id));
                    },
                );
            }
        }
        OrderedPointerEvent::Moved {
            window,
            position,
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            native_position,
        } => {
            if !state.routed.pointer_outside_windows.contains(&window) {
                clear_other_outside_window_pointers(contexts, state, window, unavailable_contexts);
            }
            state.routed.pointer_positions.insert(window, position);
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            if let Some(native_position) = native_position {
                state
                    .routed
                    .raw_native_pointer_positions
                    .insert(window, native_position);
            } else {
                state.routed.raw_native_pointer_positions.remove(&window);
            }
            let pointer_targets = refresh_routed_pointer_from_cached_position(
                contexts,
                state,
                targets,
                window,
                unavailable_contexts,
            );
            #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
            let _ = pointer_targets;
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            if routed_window_has_pressed_mouse_buttons(state, window)
                && pointer_targets
                    .iter()
                    .any(|target| target.is_native_viewport())
            {
                crate::viewport::native_window::capture_pointer(window);
            }
        }
        OrderedPointerEvent::Left { window } => {
            state.routed.pointer_outside_windows.insert(window);
            if !routed_window_has_pressed_mouse_buttons(state, window) {
                clear_routed_window_pointer(contexts, state, window, unavailable_contexts);
            }
        }
        OrderedPointerEvent::Button {
            window,
            button,
            state: button_state,
        } => {
            let pointer_targets = refresh_routed_pointer_from_cached_position(
                contexts,
                state,
                targets,
                window,
                unavailable_contexts,
            );
            let button_targets = if button_state.is_pressed() {
                pointer_targets
            } else {
                pointer_or_sticky_button_targets(state, targets, window, button)
            };
            if button_state.is_pressed() {
                replace_routed_focus_targets(
                    contexts,
                    state,
                    &[(
                        window,
                        button_targets
                            .iter()
                            .map(|target| target.context_id)
                            .collect(),
                    )],
                    unavailable_contexts,
                );
            }
            if let Some(button) = map_bevy_mouse_button(button) {
                for target in button_targets.iter().copied() {
                    apply_routed_mouse_button(
                        contexts,
                        state,
                        target,
                        button,
                        button_state.is_pressed(),
                        unavailable_contexts,
                    );
                }
            }

            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            {
                if !routed_has_pressed_mouse_buttons(state) {
                    crate::viewport::native_window::release_pointer_capture();
                    clear_all_outside_window_pointers(contexts, state, unavailable_contexts);
                }
            }
        }
    }
}

#[cfg(feature = "render")]
fn routed_window_has_pressed_mouse_buttons(state: &ImguiInputState, window: Entity) -> bool {
    state.routed.windows.iter().any(|(slot, window_state)| {
        slot.window == window && !window_state.pressed_mouse_buttons.is_empty()
    })
}

#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
fn routed_has_pressed_mouse_buttons(state: &ImguiInputState) -> bool {
    state
        .routed
        .windows
        .values()
        .any(|window_state| !window_state.pressed_mouse_buttons.is_empty())
}

#[cfg(feature = "render")]
fn clear_routed_window_pointer(
    contexts: &mut ImguiContexts,
    state: &mut ImguiInputState,
    window: Entity,
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    state.routed.pointer_positions.remove(&window);
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    state.routed.raw_native_pointer_positions.remove(&window);
    replace_routed_pointer_targets(contexts, state, window, Vec::new(), unavailable_contexts);
}

#[cfg(feature = "render")]
fn clear_other_outside_window_pointers(
    contexts: &mut ImguiContexts,
    state: &mut ImguiInputState,
    active_window: Entity,
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    let stale_windows = state
        .routed
        .pointer_outside_windows
        .iter()
        .copied()
        .filter(|window| *window != active_window)
        .collect::<Vec<_>>();
    for window in stale_windows {
        state.routed.pointer_outside_windows.remove(&window);
        clear_routed_window_pointer(contexts, state, window, unavailable_contexts);
    }
}

#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
fn clear_all_outside_window_pointers(
    contexts: &mut ImguiContexts,
    state: &mut ImguiInputState,
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    let stale_windows = state
        .routed
        .pointer_outside_windows
        .drain()
        .collect::<Vec<_>>();
    for window in stale_windows {
        clear_routed_window_pointer(contexts, state, window, unavailable_contexts);
    }
}

#[cfg(feature = "render")]
fn default_focus_targets(targets: &[RoutedInputTarget], host_window: Entity) -> Vec<ContextId> {
    default_pointer_targets(targets, host_window)
        .into_iter()
        .map(|target| target.context_id)
        .collect()
}

#[cfg(feature = "render")]
fn focus_targets_for_window(
    state: &ImguiInputState,
    targets: &[RoutedInputTarget],
    host_window: Entity,
) -> Vec<ContextId> {
    state
        .routed
        .pointer_targets
        .get(&host_window)
        .filter(|contexts| !contexts.is_empty())
        .cloned()
        .unwrap_or_else(|| default_focus_targets(targets, host_window))
}

#[cfg(feature = "render")]
fn focused_targets_for_window(
    state: &ImguiInputState,
    targets: &[RoutedInputTarget],
    host_window: Entity,
) -> Vec<RoutedInputTarget> {
    state
        .routed
        .focused_targets
        .get(&host_window)
        .map_or_else(Vec::new, |contexts| {
            targets_for_contexts(targets, host_window, contexts)
        })
}

#[cfg(feature = "render")]
fn unique_contexts(contexts: Vec<ContextId>) -> Vec<ContextId> {
    let mut seen = HashSet::new();
    contexts
        .into_iter()
        .filter(|context_id| seen.insert(*context_id))
        .collect()
}

#[cfg(feature = "render")]
fn replace_routed_pointer_targets(
    contexts: &mut ImguiContexts,
    state: &mut ImguiInputState,
    host_window: Entity,
    next: Vec<ContextId>,
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    let next = unique_contexts(next);
    let previous = if next.is_empty() {
        state
            .routed
            .pointer_targets
            .remove(&host_window)
            .unwrap_or_default()
    } else {
        state
            .routed
            .pointer_targets
            .insert(host_window, next.clone())
            .unwrap_or_default()
    };
    for context_id in previous {
        if next.contains(&context_id) {
            continue;
        }
        let slot = ImguiInputSlot {
            context_id,
            window: host_window,
        };
        let hovered = state
            .routed
            .windows
            .get_mut(&slot)
            .is_some_and(|window_state| {
                let hovered = window_state.mouse_hovered;
                window_state.mouse_hovered = false;
                hovered
            });
        if state.routed.last_hovered.get(&context_id) == Some(&host_window) {
            state.routed.last_hovered.remove(&context_id);
        }
        let another_window_is_hovered =
            state
                .routed
                .windows
                .iter()
                .any(|(other_slot, window_state)| {
                    other_slot.context_id == context_id && window_state.mouse_hovered
                });
        if hovered && !another_window_is_hovered {
            configure_routed_context(
                contexts,
                context_id,
                unavailable_contexts,
                clear_routed_pointer,
            );
        }
    }
}

#[cfg(feature = "render")]
fn mark_routed_hovered(state: &mut ImguiInputState, target: RoutedInputTarget, hovered: bool) {
    let slot = target.slot();
    state.routed.windows.entry(slot).or_default().mouse_hovered = hovered;
    if hovered {
        state
            .routed
            .last_hovered
            .insert(target.context_id, target.host_window);
    } else if state.routed.last_hovered.get(&target.context_id) == Some(&target.host_window) {
        state.routed.last_hovered.remove(&target.context_id);
    }
}

#[cfg(feature = "render")]
fn add_routed_pointer_position(
    context: &mut imgui::Context,
    target: RoutedInputTarget,
    position: Vec2,
    source: imgui::MouseSource,
) {
    let mouse_position = target.map_position(context, position);
    let viewport_id = target.viewport_id(context);
    let io = context.io_mut();
    io.add_mouse_source_event(source);
    add_mouse_viewport_event(io, Some(viewport_id));
    io.add_mouse_pos_event(mouse_position);
}

#[cfg(feature = "render")]
fn clear_routed_pointer(context: &mut imgui::Context) {
    let io = context.io_mut();
    io.add_mouse_source_event(imgui::MouseSource::Mouse);
    add_mouse_viewport_event(io, None);
    io.add_mouse_pos_event(INVALID_MOUSE_POS);
    clear_mouse_hovered_viewport(io);
}

#[cfg(feature = "render")]
fn focused_context_ids(state: &RoutedInputState) -> HashSet<ContextId> {
    state.focused_targets.values().flatten().copied().collect()
}

#[cfg(feature = "render")]
fn replace_routed_focus_targets(
    contexts: &mut ImguiContexts,
    state: &mut ImguiInputState,
    updates: &[(Entity, Vec<ContextId>)],
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    if updates.is_empty() {
        return;
    }

    let before = focused_context_ids(&state.routed);
    for &(window, ref next) in updates {
        let next = unique_contexts(next.clone());
        if next.is_empty() {
            state.routed.focused_targets.remove(&window);
        } else {
            state.routed.focused_targets.insert(window, next);
        }
    }

    for window_state in state.routed.windows.values_mut() {
        window_state.focused = false;
    }
    state.routed.last_focused.clear();
    for (&window, context_ids) in &state.routed.focused_targets {
        for &context_id in context_ids {
            state
                .routed
                .windows
                .entry(ImguiInputSlot { context_id, window })
                .or_default()
                .focused = true;
            state.routed.last_focused.insert(context_id, window);
        }
    }

    let after = focused_context_ids(&state.routed);
    for context_id in before.difference(&after).copied() {
        configure_routed_context(contexts, context_id, unavailable_contexts, |context| {
            release_routed_context_input(context, state, context_id)
        });
    }
    for context_id in after.difference(&before).copied() {
        configure_routed_context(contexts, context_id, unavailable_contexts, |context| {
            context.io_mut().add_focus_event(true)
        });
    }
}

#[cfg(feature = "render")]
fn release_routed_context_input(
    context: &mut imgui::Context,
    state: &mut ImguiInputState,
    context_id: ContextId,
) {
    for (slot, window_state) in &mut state.routed.windows {
        if slot.context_id != context_id {
            continue;
        }
        release_routed_sticky_input(context, window_state);
        if window_state.mouse_hovered {
            window_state.mouse_hovered = false;
            clear_routed_pointer(context);
        }
        window_state.focused = false;
    }
    state.routed.last_focused.remove(&context_id);
    state.routed.last_hovered.remove(&context_id);
    context.io_mut().add_focus_event(false);
}

#[cfg(feature = "render")]
fn release_routed_sticky_input(context: &mut imgui::Context, state: &mut ImguiRoutedWindowState) {
    let io = context.io_mut();
    release_sticky_keys_and_buttons(
        io,
        &mut state.pressed_keys,
        &mut state.pressed_mouse_buttons,
    );
    if state.active_touch_id.take().is_some() {
        io.add_mouse_source_event(imgui::MouseSource::TouchScreen);
        io.add_mouse_button_event(imgui::MouseButton::Left, false);
    }
}

#[cfg(feature = "render")]
fn release_stale_routed_input(
    contexts: &mut ImguiContexts,
    state: &mut ImguiInputState,
    targets: &[RoutedInputTarget],
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    let valid_slots = targets
        .iter()
        .map(|target| target.slot())
        .collect::<HashSet<_>>();
    let before = focused_context_ids(&state.routed);
    let stale_slots = state
        .routed
        .windows
        .keys()
        .filter(|slot| !valid_slots.contains(slot))
        .copied()
        .collect::<Vec<_>>();
    for slot in stale_slots {
        let Some(mut window_state) = state.routed.windows.remove(&slot) else {
            continue;
        };
        state
            .routed
            .pointer_targets
            .entry(slot.window)
            .and_modify(|contexts| contexts.retain(|context_id| *context_id != slot.context_id));
        state
            .routed
            .focused_targets
            .entry(slot.window)
            .and_modify(|contexts| contexts.retain(|context_id| *context_id != slot.context_id));
        if state.routed.last_hovered.get(&slot.context_id) == Some(&slot.window) {
            state.routed.last_hovered.remove(&slot.context_id);
        }
        if state.routed.last_focused.get(&slot.context_id) == Some(&slot.window) {
            state.routed.last_focused.remove(&slot.context_id);
        }
        let had_pointer_input = window_state.mouse_hovered
            || !window_state.pressed_mouse_buttons.is_empty()
            || window_state.active_touch_id.is_some();
        configure_routed_context(contexts, slot.context_id, unavailable_contexts, |context| {
            release_routed_sticky_input(context, &mut window_state);
            if had_pointer_input {
                clear_routed_pointer(context);
            }
        });
    }
    state
        .routed
        .pointer_targets
        .retain(|_, contexts| !contexts.is_empty());
    state
        .routed
        .focused_targets
        .retain(|_, contexts| !contexts.is_empty());
    state
        .routed
        .pointer_outside_windows
        .retain(|window| targets.iter().any(|target| target.host_window == *window));

    let after = focused_context_ids(&state.routed);
    for context_id in before.difference(&after).copied() {
        configure_routed_context(contexts, context_id, unavailable_contexts, |context| {
            release_routed_context_input(context, state, context_id)
        });
    }
}

#[cfg(feature = "render")]
fn apply_routed_resize(
    contexts: &mut ImguiContexts,
    targets: &[RoutedInputTarget],
    host_window: Entity,
    size: [f32; 2],
    context_metrics: &mut HashMap<ContextId, ImguiInputFrameMetrics>,
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    for target in targets.iter().copied().filter(|target| {
        target.host_window == host_window
            && target.tracks_host_metrics
            && target.logical_region.min == Vec2::ZERO
    }) {
        let display_size = finite_non_negative_size(size);
        let metrics = ImguiInputFrameMetrics {
            host_window,
            display_size,
            framebuffer_scale: target.framebuffer_scale,
        };
        context_metrics.insert(target.context_id, metrics);
        configure_routed_context(
            contexts,
            target.context_id,
            unavailable_contexts,
            |context| {
                let io = context.io_mut();
                io.set_display_size(metrics.display_size);
                io.set_display_framebuffer_scale(metrics.framebuffer_scale);
            },
        );
    }
}

#[cfg(feature = "render")]
fn apply_routed_scale_factor(
    contexts: &mut ImguiContexts,
    targets: &[RoutedInputTarget],
    host_window: Entity,
    scale_factor: f32,
    context_metrics: &mut HashMap<ContextId, ImguiInputFrameMetrics>,
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    for target in targets
        .iter()
        .copied()
        .filter(|target| target.host_window == host_window && target.tracks_host_metrics)
    {
        let metrics = ImguiInputFrameMetrics {
            host_window,
            display_size: context_metrics
                .get(&target.context_id)
                .map_or(target.display_size, |metrics| metrics.display_size),
            framebuffer_scale: [scale_factor, scale_factor],
        };
        context_metrics.insert(target.context_id, metrics);
        configure_routed_context(
            contexts,
            target.context_id,
            unavailable_contexts,
            |context| {
                let io = context.io_mut();
                io.set_display_size(metrics.display_size);
                io.set_display_framebuffer_scale(metrics.framebuffer_scale);
            },
        );
    }
}

#[cfg(feature = "render")]
fn pointer_or_sticky_button_targets(
    state: &ImguiInputState,
    targets: &[RoutedInputTarget],
    host_window: Entity,
    button: BevyMouseButton,
) -> Vec<RoutedInputTarget> {
    let Some(button) = map_bevy_mouse_button(button) else {
        return Vec::new();
    };
    let mut context_ids = state
        .routed
        .pointer_targets
        .get(&host_window)
        .cloned()
        .unwrap_or_default();
    context_ids.extend(
        state
            .routed
            .windows
            .iter()
            .filter(|(_, window_state)| window_state.pressed_mouse_buttons.contains(&button))
            .map(|(slot, _)| slot.context_id),
    );
    unique_contexts(context_ids)
        .into_iter()
        .filter_map(|context_id| {
            targets
                .iter()
                .find(|target| target.context_id == context_id && target.host_window == host_window)
                .or_else(|| {
                    targets
                        .iter()
                        .find(|target| target.context_id == context_id)
                })
                .copied()
        })
        .collect()
}

#[cfg(feature = "render")]
fn apply_routed_mouse_button(
    contexts: &mut ImguiContexts,
    state: &mut ImguiInputState,
    target: RoutedInputTarget,
    button: imgui::MouseButton,
    pressed: bool,
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    if pressed {
        state
            .routed
            .windows
            .entry(target.slot())
            .or_default()
            .pressed_mouse_buttons
            .insert(button);
    } else {
        for (slot, window_state) in &mut state.routed.windows {
            if slot.context_id == target.context_id {
                window_state.pressed_mouse_buttons.remove(&button);
            }
        }
    }
    configure_routed_context(
        contexts,
        target.context_id,
        unavailable_contexts,
        |context| {
            let viewport_id = target.viewport_id(context);
            let io = context.io_mut();
            io.add_mouse_source_event(imgui::MouseSource::Mouse);
            add_mouse_viewport_event(io, Some(viewport_id));
            io.add_mouse_button_event(button, pressed);
        },
    );
}

#[cfg(feature = "render")]
fn active_touch_targets(
    state: &ImguiInputState,
    targets: &[RoutedInputTarget],
    window: Entity,
    touch_id: u64,
) -> Vec<RoutedInputTarget> {
    let context_ids = state
        .routed
        .windows
        .iter()
        .filter(|(slot, window_state)| {
            slot.window == window && window_state.active_touch_id == Some(touch_id)
        })
        .map(|(slot, _)| slot.context_id)
        .collect::<Vec<_>>();
    targets_for_contexts(targets, window, &context_ids)
}

#[cfg(feature = "render")]
fn apply_routed_touch_input(
    contexts: &mut ImguiContexts,
    state: &mut ImguiInputState,
    targets: &[RoutedInputTarget],
    event: &TouchInput,
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    let selected = match event.phase {
        TouchPhase::Started => pointer_targets_for_position(targets, event.window, event.position),
        TouchPhase::Moved | TouchPhase::Ended | TouchPhase::Canceled => {
            active_touch_targets(state, targets, event.window, event.id)
        }
    };
    for target in selected {
        let window_state = state.routed.windows.entry(target.slot()).or_default();
        match event.phase {
            TouchPhase::Started if window_state.active_touch_id.is_none() => {
                window_state.active_touch_id = Some(event.id);
                configure_routed_context(
                    contexts,
                    target.context_id,
                    unavailable_contexts,
                    |context| {
                        add_routed_pointer_position(
                            context,
                            target,
                            event.position,
                            imgui::MouseSource::TouchScreen,
                        );
                        context
                            .io_mut()
                            .add_mouse_button_event(imgui::MouseButton::Left, true);
                    },
                );
            }
            TouchPhase::Moved if window_state.active_touch_id == Some(event.id) => {
                configure_routed_context(
                    contexts,
                    target.context_id,
                    unavailable_contexts,
                    |context| {
                        add_routed_pointer_position(
                            context,
                            target,
                            event.position,
                            imgui::MouseSource::TouchScreen,
                        );
                    },
                );
            }
            TouchPhase::Ended | TouchPhase::Canceled
                if window_state.active_touch_id == Some(event.id) =>
            {
                window_state.active_touch_id = None;
                configure_routed_context(
                    contexts,
                    target.context_id,
                    unavailable_contexts,
                    |context| {
                        add_routed_pointer_position(
                            context,
                            target,
                            event.position,
                            imgui::MouseSource::TouchScreen,
                        );
                        context
                            .io_mut()
                            .add_mouse_button_event(imgui::MouseButton::Left, false);
                    },
                );
            }
            _ => {}
        }
    }
}

#[cfg(feature = "render")]
fn apply_routed_keyboard_input(
    contexts: &mut ImguiContexts,
    state: &mut ImguiInputState,
    target: RoutedInputTarget,
    event: &KeyboardInput,
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    let pressed = event.state.is_pressed();
    let key = map_bevy_key_code(event.key_code);
    if let Some(key) = key {
        if pressed {
            state
                .routed
                .windows
                .entry(target.slot())
                .or_default()
                .pressed_keys
                .insert(key);
        } else {
            for (slot, window_state) in &mut state.routed.windows {
                if slot.context_id == target.context_id {
                    window_state.pressed_keys.remove(&key);
                }
            }
        }
    }
    let modifiers = routed_context_modifiers(&state.routed, target.context_id);
    configure_routed_context(
        contexts,
        target.context_id,
        unavailable_contexts,
        |context| {
            let io = context.io_mut();
            if pressed && let Some(text) = &event.text {
                add_keyboard_text(io, text);
            }
            apply_modifier_events(io, modifiers);
            if let Some(key) = key {
                io.add_key_event(key, pressed);
            }
        },
    );
}

#[cfg(feature = "render")]
fn routed_context_modifiers(
    state: &RoutedInputState,
    context_id: ContextId,
) -> (bool, bool, bool, bool) {
    state
        .windows
        .iter()
        .filter(|(slot, _)| slot.context_id == context_id)
        .fold((false, false, false, false), |aggregate, (_, window)| {
            let modifiers = window.modifiers();
            (
                aggregate.0 || modifiers.0,
                aggregate.1 || modifiers.1,
                aggregate.2 || modifiers.2,
                aggregate.3 || modifiers.3,
            )
        })
}

#[cfg(feature = "render")]
fn ime_window(event: &Ime) -> Entity {
    match event {
        Ime::Preedit { window, .. }
        | Ime::Commit { window, .. }
        | Ime::Enabled { window }
        | Ime::Disabled { window } => *window,
    }
}

#[cfg(feature = "render")]
fn apply_routed_ime_event(
    contexts: &mut ImguiContexts,
    state: &mut ImguiInputState,
    target: RoutedInputTarget,
    event: &Ime,
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    match event {
        Ime::Enabled { .. } => {
            state
                .routed
                .windows
                .entry(target.slot())
                .or_default()
                .ime_enabled = true;
        }
        Ime::Disabled { .. } => {
            state
                .routed
                .windows
                .entry(target.slot())
                .or_default()
                .ime_enabled = false;
        }
        Ime::Preedit { .. } | Ime::Commit { .. } => {}
    }
    configure_routed_context(
        contexts,
        target.context_id,
        unavailable_contexts,
        |context| {
            if let Ime::Commit { value, .. } = event {
                add_ime_text(context.io_mut(), value);
            }
        },
    );
}

#[cfg(feature = "render")]
fn prune_unavailable_routed_contexts(
    state: &mut ImguiInputState,
    capture: &mut ImguiInputCapture,
    unavailable_contexts: &HashSet<ContextId>,
) {
    if unavailable_contexts.is_empty() {
        return;
    }
    state
        .routed
        .windows
        .retain(|slot, _| !unavailable_contexts.contains(&slot.context_id));
    state.routed.pointer_targets.retain(|_, contexts| {
        contexts.retain(|context_id| !unavailable_contexts.contains(context_id));
        !contexts.is_empty()
    });
    state.routed.focused_targets.retain(|_, contexts| {
        contexts.retain(|context_id| !unavailable_contexts.contains(context_id));
        !contexts.is_empty()
    });
    state
        .routed
        .last_focused
        .retain(|context_id, _| !unavailable_contexts.contains(context_id));
    state
        .routed
        .last_hovered
        .retain(|context_id, _| !unavailable_contexts.contains(context_id));
    for &context_id in unavailable_contexts {
        capture.remove_context(context_id);
    }
}

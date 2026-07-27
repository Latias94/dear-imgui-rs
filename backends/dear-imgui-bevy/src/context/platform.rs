#[cfg(feature = "render")]
use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::*;
use bevy_ecs::world::World;
use bevy_math::Vec2;
use bevy_window::{CursorIcon, CursorOptions, PrimaryWindow, Window, WindowPosition};
use dear_imgui_rs as imgui;

use crate::input::{ImguiInputState, map_imgui_mouse_cursor};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use crate::{ImguiViewportWindow, viewport::ImguiViewportOwner};

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_window::{Monitor, PrimaryMonitor};

#[cfg(feature = "render")]
#[derive(Resource, Default)]
pub(super) struct ImguiPlatformImeFeedback {
    previous_windows: HashSet<Entity>,
    requests: HashMap<Entity, ImguiPlatformImeRequest>,
}

#[cfg(feature = "render")]
#[derive(Clone, Copy, Default)]
struct ImguiPlatformImeRequest {
    wants_text_input: bool,
    input_position: Option<ImguiPlatformImePosition>,
}

#[cfg(feature = "render")]
#[derive(Clone, Copy)]
struct ImguiPlatformImePosition {
    position: [f32; 2],
    uses_desktop_coordinates: bool,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) fn prepare_context_platform_frame(
    world: &mut World,
    context_id: imgui::ContextId,
    context: &mut imgui::Context,
    host_window: Entity,
) -> Result<bool, crate::viewport::ImguiViewportBridgeError> {
    let Some(host_window_state) = world.get::<Window>(host_window).cloned() else {
        return Ok(false);
    };
    let Some(bridge) = world
        .get_non_send::<crate::ImguiViewportBridge>()
        .and_then(|bridge| bridge.context(context_id))
    else {
        return Ok(false);
    };
    if bridge.ecs_release_pending() {
        return Ok(false);
    }
    let (viewport_windows, marker_repairs) = {
        let mut query = world.query::<(
            Entity,
            &Window,
            Option<&ImguiViewportWindow>,
            &ImguiViewportOwner,
        )>();
        let mut viewport_windows = Vec::new();
        let mut marker_repairs = Vec::new();
        for (entity, window, marker, owner) in query.iter(world) {
            let Some((owner_context_id, viewport_id)) = owner.window_identity() else {
                continue;
            };
            if owner_context_id != context_id || bridge.viewport_window(viewport_id) != Some(entity)
            {
                continue;
            }
            if marker.is_none_or(|marker| !owner.matches_window(marker)) {
                marker_repairs.push((entity, viewport_id));
            }
            viewport_windows.push((entity, window.clone(), viewport_id));
        }
        (viewport_windows, marker_repairs)
    };
    let stale_markers = {
        let mut query =
            world.query::<(Entity, &ImguiViewportWindow, Option<&ImguiViewportOwner>)>();
        query
            .iter(world)
            .filter_map(|(entity, marker, owner)| {
                if marker.context_id() != context_id {
                    return None;
                }
                let is_projection_of_owner = owner.is_some_and(|owner| {
                    owner.matches_window(marker)
                        && bridge.viewport_window(marker.viewport_id()) == Some(entity)
                });
                (!is_projection_of_owner).then_some(entity)
            })
            .collect::<Vec<_>>()
    };
    // Public markers project private ownership and never grant it to another entity.
    for entity in stale_markers {
        world.entity_mut(entity).remove::<ImguiViewportWindow>();
    }
    for (entity, viewport_id) in marker_repairs {
        world
            .entity_mut(entity)
            .insert(ImguiViewportWindow::new(context_id, viewport_id));
    }
    let monitors = {
        let mut query = world.query::<(&Monitor, Option<&PrimaryMonitor>)>();
        crate::viewport::platform_monitors_from_bevy_monitors(
            query
                .iter(world)
                .map(|(monitor, primary)| (monitor.clone(), primary.is_some())),
        )
    };
    let viewport_feedback = viewport_windows
        .iter()
        .map(|(entity, window, viewport_id)| {
            (
                *entity,
                *viewport_id,
                crate::viewport::viewport_feedback_from_window(
                    *entity,
                    window,
                    bridge.viewport_feedback(*viewport_id),
                ),
            )
        })
        .collect::<Vec<_>>();
    crate::viewport::prepare_platform_viewports_for_frame(
        context,
        &bridge,
        host_window,
        &host_window_state,
        &monitors,
        viewport_feedback.into_iter(),
        true,
    )
    .map_err(crate::viewport::ImguiViewportBridgeError::CallbackOwnership)?;
    Ok(true)
}

pub(super) fn sync_context_platform_feedback(
    world: &mut World,
    context_id: imgui::ContextId,
    host_window: Option<Entity>,
    ui: &imgui::Ui,
) {
    debug_assert_eq!(ui.context_id(), context_id);
    let hovered_window = {
        #[cfg(feature = "render")]
        {
            world
                .get_resource::<ImguiInputState>()
                .and_then(|state| state.mouse_hovered_window_for_context(context_id))
        }
        #[cfg(not(feature = "render"))]
        {
            world
                .get_resource::<ImguiInputState>()
                .and_then(ImguiInputState::mouse_hovered_window)
        }
    };
    let Some(host_window) = host_window else {
        return;
    };
    let Some((host_entity, viewport_entities)) =
        host_window_and_viewport_entities(world, host_window, context_id)
    else {
        return;
    };

    let cursor_target = hovered_window
        .filter(|candidate| {
            host_entity == *candidate
                || viewport_entities
                    .iter()
                    .any(|(entity, _)| entity == candidate)
        })
        .or(Some(host_entity));
    let hide_os_cursor = ui.io().mouse_draw_cursor() || ui.mouse_cursor().is_none();
    let cursor_icon = (!hide_os_cursor)
        .then(|| ui.mouse_cursor().and_then(map_imgui_mouse_cursor))
        .flatten();
    let mut cursor_edits = Vec::new();

    {
        let mut query = world.query::<(Entity, &mut CursorOptions, Option<&mut CursorIcon>)>();
        for (entity, mut cursor_options, current_cursor_icon) in query.iter_mut(world) {
            if entity != host_entity
                && !viewport_entities
                    .iter()
                    .any(|(viewport_entity, _)| *viewport_entity == entity)
            {
                continue;
            }

            let owns_cursor = Some(entity) == cursor_target;
            cursor_options.visible = !owns_cursor || !hide_os_cursor;
            match (
                owns_cursor.then_some(cursor_icon.clone()).flatten(),
                current_cursor_icon,
            ) {
                (Some(desired), Some(mut current)) => *current = desired,
                (Some(desired), None) => cursor_edits.push(CursorEdit::Insert(entity, desired)),
                (None, Some(_)) => cursor_edits.push(CursorEdit::Remove(entity)),
                (None, None) => {}
            }
        }
    }

    for edit in cursor_edits {
        match edit {
            CursorEdit::Insert(entity, icon) => {
                world.entity_mut(entity).insert(icon);
            }
            CursorEdit::Remove(entity) => {
                world.entity_mut(entity).remove::<CursorIcon>();
            }
        }
    }
}

#[cfg(not(feature = "render"))]
pub(super) fn sync_primary_window_ime_feedback(
    world: &mut World,
    context_id: imgui::ContextId,
    context_raw: *mut imgui::sys::ImGuiContext,
) {
    let Some((primary_entity, viewport_entities)) =
        primary_window_and_viewport_entities(world, context_id)
    else {
        return;
    };
    // SAFETY: the serial driver retains the owning active Context and keeps its frame open for
    // this call. The raw pointer was captured immediately before `Context::frame()`.
    let ime_data = unsafe { &(*context_raw).PlatformImeData };
    let uses_desktop_coordinates = native_viewports_enabled_for_frame(context_raw);
    let ime_target = (ime_data.ViewportId != 0)
        .then_some(ime_data.ViewportId)
        .and_then(|viewport_id| {
            viewport_entities
                .iter()
                .find_map(|(entity, candidate)| (*candidate == viewport_id).then_some(*entity))
        })
        .unwrap_or(primary_entity);
    let ime_position = [ime_data.InputPos.x, ime_data.InputPos.y];

    let mut query = world.query::<(Entity, &mut Window, Option<&PrimaryWindow>)>();
    for (entity, mut window, primary_window) in query.iter_mut(world) {
        if primary_window.is_none()
            && !viewport_entities
                .iter()
                .any(|(viewport_entity, _)| *viewport_entity == entity)
        {
            continue;
        }
        let owns_ime = entity == ime_target;
        window.ime_enabled = owns_ime && ime_data.WantTextInput;
        if owns_ime {
            window.ime_position = ime_position_for_window(
                entity,
                &window,
                ime_position,
                primary_window.is_some(),
                uses_desktop_coordinates,
            );
        }
    }
}

#[cfg(feature = "render")]
pub(super) fn begin_platform_ime_feedback(world: &mut World) {
    world
        .resource_mut::<ImguiPlatformImeFeedback>()
        .requests
        .clear();
}

#[cfg(feature = "render")]
pub(super) fn record_context_platform_ime_feedback(
    world: &mut World,
    context_id: imgui::ContextId,
    host_window: Option<Entity>,
    context_raw: *mut imgui::sys::ImGuiContext,
) {
    // SAFETY: the serial driver retains the owning active Context and keeps its frame open for
    // this call. The raw pointer was captured immediately before `Context::frame()`.
    let ime_data = unsafe { &(*context_raw).PlatformImeData };
    let uses_desktop_coordinates = native_viewports_enabled_for_frame(context_raw);
    let mut windows = world
        .get_resource::<ImguiInputState>()
        .map(|state| state.context_window_focus_states(context_id))
        .unwrap_or_default();
    let host_and_viewports = host_window
        .and_then(|host_window| host_window_and_viewport_entities(world, host_window, context_id));

    if windows.is_empty() {
        if let Some((host_window, _)) = host_and_viewports.as_ref() {
            windows.push((*host_window, true));
        }
    }
    if ime_data.ViewportId != 0
        && let Some((_, viewport_entities)) = host_and_viewports.as_ref()
        && let Some(viewport_window) = viewport_entities.iter().find_map(|(entity, viewport_id)| {
            (*viewport_id == ime_data.ViewportId).then_some(*entity)
        })
    {
        windows.clear();
        windows.push((viewport_window, true));
    }

    let ime_position = [ime_data.InputPos.x, ime_data.InputPos.y];
    let mut feedback = world.resource_mut::<ImguiPlatformImeFeedback>();
    for (window, focused) in windows {
        let request = feedback.requests.entry(window).or_default();
        if focused && ime_data.WantTextInput {
            request.wants_text_input = true;
            request.input_position = Some(ImguiPlatformImePosition {
                position: ime_position,
                uses_desktop_coordinates,
            });
        }
    }
}

#[cfg(feature = "render")]
pub(super) fn finish_platform_ime_feedback(world: &mut World) {
    let (requests, previous_windows) = {
        let mut feedback = world.resource_mut::<ImguiPlatformImeFeedback>();
        let requests = std::mem::take(&mut feedback.requests);
        let previous_windows = std::mem::replace(
            &mut feedback.previous_windows,
            requests.keys().copied().collect(),
        );
        (requests, previous_windows)
    };
    let mut affected_windows = previous_windows;
    affected_windows.extend(requests.keys().copied());

    let mut query = world.query::<(Entity, &mut Window, Option<&PrimaryWindow>)>();
    for (entity, mut window, primary_window) in query.iter_mut(world) {
        if !affected_windows.contains(&entity) {
            continue;
        }
        let Some(request) = requests.get(&entity) else {
            window.ime_enabled = false;
            continue;
        };
        window.ime_enabled = request.wants_text_input;
        if let Some(ime_position) = request.input_position {
            window.ime_position = ime_position_for_window(
                entity,
                &window,
                ime_position.position,
                primary_window.is_some(),
                ime_position.uses_desktop_coordinates,
            );
        }
    }
}

#[cfg(not(feature = "render"))]
fn primary_window_and_viewport_entities(
    world: &mut World,
    context_id: imgui::ContextId,
) -> Option<(Entity, Vec<(Entity, u32)>)> {
    let mut query = world.query_filtered::<Entity, With<PrimaryWindow>>();
    let primary = query.single(world).ok()?;
    host_window_and_viewport_entities(world, primary, context_id)
}

fn host_window_and_viewport_entities(
    world: &mut World,
    host_window: Entity,
    context_id: imgui::ContextId,
) -> Option<(Entity, Vec<(Entity, u32)>)> {
    world.get::<Window>(host_window)?;
    let viewports = {
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        let viewports = {
            let mut viewports = Vec::new();
            let bridge = world
                .get_non_send::<crate::ImguiViewportBridge>()
                .and_then(|bridge| bridge.context(context_id));
            if let Some(bridge) = bridge {
                let mut query =
                    world.query_filtered::<(Entity, &ImguiViewportOwner), With<Window>>();
                for (entity, owner) in query.iter(world) {
                    if let Some((owner_context_id, viewport_id)) = owner.window_identity()
                        && owner_context_id == context_id
                        && bridge.viewport_window(viewport_id) == Some(entity)
                    {
                        viewports.push((entity, viewport_id.raw()));
                    }
                }
            }
            viewports
        };
        #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
        let viewports = {
            let _ = context_id;
            Vec::new()
        };
        viewports
    };
    Some((host_window, viewports))
}

enum CursorEdit {
    Insert(Entity, CursorIcon),
    Remove(Entity),
}

fn ime_position_for_window(
    _entity: Entity,
    window: &Window,
    ime_position: [f32; 2],
    is_primary: bool,
    uses_desktop_coordinates: bool,
) -> Vec2 {
    let mut position = Vec2::new(ime_position[0], ime_position[1]);
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    {
        if uses_desktop_coordinates {
            if let Some(client_position) = crate::viewport::desktop_to_window_client_logical(
                _entity,
                &window.position,
                window.scale_factor(),
                ime_position,
            ) {
                return Vec2::new(client_position[0], client_position[1]);
            }
        }
    }

    if uses_desktop_coordinates
        && !is_primary
        && let WindowPosition::At(window_position) = window.position
    {
        let scale_factor = crate::input::sanitized_window_framebuffer_scale(window)[0];
        position.x -= window_position.x as f32 / scale_factor;
        position.y -= window_position.y as f32 / scale_factor;
    }
    position
}

fn native_viewports_enabled_for_frame(context_raw: *mut imgui::sys::ImGuiContext) -> bool {
    // SAFETY: callers retain the owning active Context while platform feedback is collected.
    unsafe {
        (*context_raw).ConfigFlagsCurrFrame & imgui::ConfigFlags::VIEWPORTS_ENABLE.bits() != 0
    }
}

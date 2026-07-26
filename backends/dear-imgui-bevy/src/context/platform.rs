use bevy_ecs::prelude::*;
use bevy_ecs::world::World;
use bevy_math::Vec2;
use bevy_window::{CursorIcon, CursorOptions, PrimaryWindow, Window, WindowPosition};
use dear_imgui_rs as imgui;

use crate::ImguiViewportWindow;
use crate::input::{ImguiInputState, map_imgui_mouse_cursor};

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_window::{Monitor, PrimaryMonitor};

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) fn prepare_primary_platform_frame(world: &mut World, context: &mut imgui::Context) {
    let Some((primary_entity, primary_window)) = ({
        let mut query = world.query_filtered::<(Entity, &Window), With<PrimaryWindow>>();
        query
            .single(world)
            .ok()
            .map(|(entity, window)| (entity, window.clone()))
    }) else {
        return;
    };
    let viewport_windows = {
        let mut query = world
            .query_filtered::<(Entity, &Window, &ImguiViewportWindow), Without<PrimaryWindow>>();
        query
            .iter(world)
            .map(|(entity, window, viewport)| (entity, window.clone(), viewport.viewport_id))
            .collect::<Vec<_>>()
    };
    let monitors = {
        let mut query = world.query::<(&Monitor, Option<&PrimaryMonitor>)>();
        crate::viewport::platform_monitors_from_bevy_monitors(
            query
                .iter(world)
                .map(|(monitor, primary)| (monitor.clone(), primary.is_some())),
        )
    };
    let enable_viewports = world
        .get_resource::<crate::ImguiBackendStatus>()
        .is_some_and(|status| status.multi_viewport_supported);
    let Some(mut bridge) = world.get_non_send_mut::<crate::ImguiViewportBridge>() else {
        return;
    };
    if bridge.ecs_release_pending() {
        return;
    }
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
        &mut bridge,
        primary_entity,
        &primary_window,
        &monitors,
        viewport_feedback,
        enable_viewports,
    )
    .unwrap_or_else(|error| {
        panic!("dear-imgui-bevy viewport ownership changed before frame preparation: {error}")
    });
}

pub(super) fn sync_primary_window_platform_feedback(
    world: &mut World,
    ui: &imgui::Ui,
    context_raw: *mut imgui::sys::ImGuiContext,
) {
    let hovered_window = world
        .get_resource::<ImguiInputState>()
        .and_then(ImguiInputState::mouse_hovered_window);
    let Some((primary_entity, viewport_entities)) = ({
        let mut query =
            world.query::<(Entity, Option<&PrimaryWindow>, Option<&ImguiViewportWindow>)>();
        let mut primary = None;
        let mut viewports = Vec::new();
        for (entity, primary_window, viewport_window) in query.iter(world) {
            if primary_window.is_some() {
                primary = Some(entity);
            }
            if let Some(viewport_window) = viewport_window {
                viewports.push((entity, viewport_window.viewport_id.raw()));
            }
        }
        primary.map(|primary| (primary, viewports))
    }) else {
        return;
    };

    let cursor_target = hovered_window
        .filter(|candidate| {
            *candidate == primary_entity
                || viewport_entities
                    .iter()
                    .any(|(entity, _)| entity == candidate)
        })
        .unwrap_or(primary_entity);
    // SAFETY: the serial driver retains the owning active Context and keeps its frame open for
    // this call. The raw pointer was captured immediately before `Context::frame()`.
    let ime_data = unsafe { &(*context_raw).PlatformImeData };
    let ime_target = (ime_data.ViewportId != 0)
        .then_some(ime_data.ViewportId)
        .and_then(|viewport_id| {
            viewport_entities
                .iter()
                .find_map(|(entity, candidate)| (*candidate == viewport_id).then_some(*entity))
        })
        .unwrap_or(primary_entity);
    let ime_position = [ime_data.InputPos.x, ime_data.InputPos.y];
    let hide_os_cursor = ui.io().mouse_draw_cursor() || ui.mouse_cursor().is_none();
    let cursor_icon = (!hide_os_cursor)
        .then(|| ui.mouse_cursor().and_then(map_imgui_mouse_cursor))
        .flatten();
    let mut cursor_edits = Vec::new();

    {
        let mut query = world.query::<(
            Entity,
            &mut Window,
            &mut CursorOptions,
            Option<&mut CursorIcon>,
            Option<&PrimaryWindow>,
            Option<&ImguiViewportWindow>,
        )>();
        for (
            entity,
            mut window,
            mut cursor_options,
            current_cursor_icon,
            primary_window,
            viewport_window,
        ) in query.iter_mut(world)
        {
            if primary_window.is_none() && viewport_window.is_none() {
                continue;
            }

            let owns_cursor = entity == cursor_target;
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

            let owns_ime = entity == ime_target;
            window.ime_enabled = owns_ime && ime_data.WantTextInput;
            if owns_ime {
                window.ime_position = ime_position_for_window(
                    entity,
                    &window,
                    ime_position,
                    primary_window.is_some(),
                );
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

enum CursorEdit {
    Insert(Entity, CursorIcon),
    Remove(Entity),
}

fn ime_position_for_window(
    _entity: Entity,
    window: &Window,
    ime_position: [f32; 2],
    is_primary: bool,
) -> Vec2 {
    let mut position = Vec2::new(ime_position[0], ime_position[1]);
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    {
        if let Some(origin) = crate::viewport::window_client_origin_logical(
            _entity,
            &window.position,
            window.scale_factor(),
        ) {
            position.x -= origin[0];
            position.y -= origin[1];
            return position;
        }
    }

    if !is_primary && let WindowPosition::At(window_position) = window.position {
        let scale_factor = crate::input::sanitized_window_framebuffer_scale(window)[0];
        position.x -= window_position.x as f32 / scale_factor;
        position.y -= window_position.y as f32 / scale_factor;
    }
    position
}

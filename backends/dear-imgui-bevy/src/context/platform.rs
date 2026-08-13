#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use std::collections::BTreeSet;
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
use bevy_winit::WINIT_WINDOWS;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use dear_imgui_winit::native_support::{
    MonitorCollectionError, MonitorSnapshotSet, WorkAreaFallback, WorkAreaProvenance,
    collect_monitor_snapshot_set,
};

#[cfg(feature = "render")]
#[derive(Resource, Default)]
pub(super) struct ImguiPlatformFeedback {
    previous_ime_windows: HashSet<Entity>,
    ime_requests: HashMap<Entity, ImguiPlatformImeRequest>,
    previous_cursor_windows: HashSet<Entity>,
    cursor_requests: HashMap<Entity, ImguiPlatformCursorRequest>,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    native_monitor_diagnostics: Vec<crate::route::ImguiDiagnostic>,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    native_monitor_diagnostic_epoch: u64,
}

#[cfg(feature = "render")]
#[derive(Clone, Copy, Default)]
struct ImguiPlatformImeRequest {
    wants_text_input: bool,
    input_position: Option<ImguiPlatformImePosition>,
}

#[cfg(feature = "render")]
#[derive(Clone, Default)]
struct ImguiPlatformCursorRequest {
    visible: bool,
    icon: Option<CursorIcon>,
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
) -> Result<bool, crate::viewport::ImguiViewportRuntimeError> {
    let Some(host_window_state) = world.get::<Window>(host_window).cloned() else {
        return Ok(false);
    };
    let render_integration_installed = world
        .get_resource::<super::ImguiBackendRuntime>()
        .is_some_and(super::ImguiBackendRuntime::render_integration_installed);
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
            let Some((owner_context_id, instance_id)) = owner.window_identity() else {
                continue;
            };
            if owner_context_id != context_id
                || bridge.viewport_window_for_instance(instance_id) != Some(entity)
            {
                continue;
            }
            let Some(viewport_id) = bridge.viewport_id(instance_id) else {
                continue;
            };
            if marker.is_none_or(|marker| {
                !owner.matches_window(marker) || marker.viewport_id() != viewport_id
            }) {
                marker_repairs.push((entity, instance_id, viewport_id));
            }
            viewport_windows.push((entity, window.clone(), instance_id));
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
                        && bridge.viewport_window_for_instance(marker.instance_id()) == Some(entity)
                });
                (!is_projection_of_owner).then_some(entity)
            })
            .collect::<Vec<_>>()
    };
    // Public markers project private ownership and never grant it to another entity.
    for entity in stale_markers {
        world.entity_mut(entity).remove::<ImguiViewportWindow>();
    }
    for (entity, instance_id, viewport_id) in marker_repairs {
        world
            .entity_mut(entity)
            .insert(ImguiViewportWindow::new(instance_id, viewport_id));
    }
    // The ECS monitor list is not an exact proof of the host's native display source. Bevy-Winit
    // creates that mapping asynchronously, so a missing mapping is an explicit pending state and
    // must not be replaced with a guessed primary monitor or host-window rectangle.
    let (monitor_publication, monitor_diagnostics) = WINIT_WINDOWS.with_borrow(|windows| {
        let Some(host) = windows.get_window(host_window) else {
            return (None, Vec::new());
        };
        collect_native_monitor_publication(context_id, host)
    });
    world
        .resource_mut::<ImguiPlatformFeedback>()
        .native_monitor_diagnostics
        .extend(monitor_diagnostics);
    let viewport_feedback = viewport_windows
        .iter()
        .map(|(entity, window, instance_id)| {
            (
                *entity,
                *instance_id,
                crate::viewport::viewport_feedback_from_window(
                    *entity,
                    window,
                    bridge.viewport_feedback_for_instance(*instance_id),
                ),
            )
        })
        .collect::<Vec<_>>();
    #[cfg(test)]
    let desktop_position_support = world
        .get_resource::<crate::viewport::native_window::DesktopPositionSupportOverride>()
        .map_or_else(
            || crate::viewport::native_window::desktop_position_support(host_window),
            |support| support.0,
        );
    #[cfg(not(test))]
    let desktop_position_support =
        crate::viewport::native_window::desktop_position_support(host_window);
    world
        .resource_mut::<crate::ImguiNativeViewportSupport>()
        .set(context_id, desktop_position_support.into());
    crate::viewport::prepare_platform_viewports_for_frame(
        context,
        &bridge,
        host_window,
        &host_window_state,
        monitor_publication.as_ref(),
        viewport_feedback.into_iter(),
        crate::viewport::NativeViewportFrameSupport::new(
            render_integration_installed,
            desktop_position_support,
        ),
    )?;
    Ok(desktop_position_support
        != crate::viewport::native_window::DesktopPositionSupport::PendingWindow)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn collect_native_monitor_publication(
    context_id: imgui::ContextId,
    host: &winit::window::Window,
) -> (
    Option<crate::viewport::ImguiMonitorPublication>,
    Vec<crate::route::ImguiDiagnostic>,
) {
    let snapshot_set = match collect_monitor_snapshot_set(host) {
        Ok(snapshot_set) => snapshot_set,
        Err(error) => {
            return (
                None,
                vec![native_monitor_diagnostic(
                    context_id,
                    crate::route::ImguiDiagnosticKind::NativeMonitorCollectionFailed {
                        reason: native_monitor_collection_failure(error),
                    },
                )],
            );
        }
    };
    if snapshot_set.snapshots().is_empty() {
        return (
            None,
            vec![native_monitor_diagnostic(
                context_id,
                crate::route::ImguiDiagnosticKind::NativeMonitorCollectionFailed {
                    reason: crate::route::ImguiNativeMonitorFailure::EmptyCollection,
                },
            )],
        );
    }
    if snapshot_set.snapshots().len() > 1 && snapshot_set.primary_identity().is_none() {
        return (
            None,
            vec![native_monitor_diagnostic(
                context_id,
                crate::route::ImguiDiagnosticKind::NativeMonitorPrimaryUnproven,
            )],
        );
    }

    let mut diagnostics = native_monitor_snapshot_diagnostics(context_id, &snapshot_set);
    let publication = crate::viewport::monitor_publication_from_snapshot_set(snapshot_set);
    if publication.is_none() {
        diagnostics.push(native_monitor_diagnostic(
            context_id,
            crate::route::ImguiDiagnosticKind::NativeMonitorCollectionFailed {
                reason: crate::route::ImguiNativeMonitorFailure::ProjectionInvalid,
            },
        ));
    }
    (publication, diagnostics)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn native_monitor_snapshot_diagnostics(
    context_id: imgui::ContextId,
    snapshot_set: &MonitorSnapshotSet,
) -> Vec<crate::route::ImguiDiagnostic> {
    let mut diagnostics = Vec::new();
    let fallbacks = snapshot_set
        .snapshots()
        .iter()
        .filter_map(|snapshot| match snapshot.work_area_provenance() {
            WorkAreaProvenance::FullMain(reason) => Some(native_monitor_fallback(reason)),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    diagnostics.extend(fallbacks.into_iter().map(|reason| {
        native_monitor_diagnostic(
            context_id,
            crate::route::ImguiDiagnosticKind::NativeMonitorWorkAreaFallback { reason },
        )
    }));
    diagnostics
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn native_monitor_diagnostic(
    context_id: imgui::ContextId,
    kind: crate::route::ImguiDiagnosticKind,
) -> crate::route::ImguiDiagnostic {
    crate::route::ImguiDiagnostic::new(kind).with_context(context_id)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn native_monitor_collection_failure(
    error: MonitorCollectionError,
) -> crate::route::ImguiNativeMonitorFailure {
    use crate::route::ImguiNativeMonitorFailure as Failure;

    match error {
        MonitorCollectionError::MainFactsUnavailable { .. } => Failure::MainFactsUnavailable,
        MonitorCollectionError::InvalidMainGeometry { .. } => Failure::InvalidMainGeometry,
        MonitorCollectionError::InvalidScaleFactor { .. } => Failure::InvalidScaleFactor,
        MonitorCollectionError::InvalidWorkGeometry { .. } => Failure::InvalidWorkGeometry,
        _ => Failure::Unclassified,
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn native_monitor_fallback(fallback: WorkAreaFallback) -> crate::route::ImguiNativeMonitorFallback {
    use crate::route::ImguiNativeMonitorFallback as Fallback;

    match fallback {
        WorkAreaFallback::Wayland => Fallback::Wayland,
        WorkAreaFallback::UnsupportedWindowSystem => Fallback::UnsupportedWindowSystem,
        WorkAreaFallback::SourceUnavailable => Fallback::SourceUnavailable,
        WorkAreaFallback::InvalidNativeData => Fallback::InvalidNativeData,
        WorkAreaFallback::MainThreadUnavailable => Fallback::MainThreadUnavailable,
        WorkAreaFallback::AmbiguousDesktopScope => Fallback::AmbiguousDesktopScope,
        _ => Fallback::Unclassified,
    }
}

pub(super) fn sync_context_platform_feedback(
    world: &mut World,
    context_id: imgui::ContextId,
    host_window: Option<Entity>,
    ui: &imgui::Ui,
) {
    debug_assert_eq!(ui.context_id(), context_id);
    #[cfg(feature = "render")]
    {
        let Some(host_window) = host_window else {
            return;
        };
        let cursor_target = world.get_resource::<ImguiInputState>().and_then(|state| {
            state.platform_cursor_window_for_context(context_id, Some(host_window))
        });
        let Some(cursor_target) = cursor_target else {
            return;
        };
        let hide_os_cursor = ui.io().mouse_draw_cursor() || ui.mouse_cursor().is_none();
        let cursor_icon = (!hide_os_cursor)
            .then(|| ui.mouse_cursor().and_then(map_imgui_mouse_cursor))
            .flatten();
        world
            .resource_mut::<ImguiPlatformFeedback>()
            .cursor_requests
            .entry(cursor_target)
            .or_insert_with(|| ImguiPlatformCursorRequest {
                visible: !hide_os_cursor,
                icon: cursor_icon,
            });
    }

    #[cfg(not(feature = "render"))]
    {
        let Some(host_window) = host_window else {
            return;
        };
        let Some((host_entity, viewport_entities)) =
            host_window_and_viewport_entities(world, host_window, context_id)
        else {
            return;
        };
        let cursor_target = world
            .get_resource::<ImguiInputState>()
            .and_then(ImguiInputState::mouse_hovered_window)
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
pub(super) fn begin_platform_feedback(world: &mut World) {
    let mut feedback = world.resource_mut::<ImguiPlatformFeedback>();
    feedback.ime_requests.clear();
    feedback.cursor_requests.clear();
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    feedback.native_monitor_diagnostics.clear();
}

#[cfg(feature = "render")]
pub(super) fn record_context_platform_ime_feedback(
    world: &mut World,
    context_id: imgui::ContextId,
    host_window: Option<Entity>,
    input_enabled: bool,
    context_raw: *mut imgui::sys::ImGuiContext,
) {
    // SAFETY: the serial driver retains the owning active Context and keeps its frame open for
    // this call. The raw pointer was captured immediately before `Context::frame()`.
    let ime_data = unsafe { &(*context_raw).PlatformImeData };
    let uses_desktop_coordinates = native_viewports_enabled_for_frame(context_raw);
    if !input_enabled {
        return;
    }
    let mut windows = world
        .get_resource::<ImguiInputState>()
        .map(|state| state.context_window_focus_states(context_id))
        .unwrap_or_default();
    let host_and_viewports = host_window
        .and_then(|host_window| host_window_and_viewport_entities(world, host_window, context_id));

    if ime_data.ViewportId != 0
        && let Some(viewport_window) =
            host_and_viewports
                .as_ref()
                .and_then(|(_, viewport_entities)| {
                    viewport_entities.iter().find_map(|(entity, viewport_id)| {
                        (*viewport_id == ime_data.ViewportId).then_some(*entity)
                    })
                })
    {
        windows.clear();
        windows.push((viewport_window, true));
    }

    let ime_position = [ime_data.InputPos.x, ime_data.InputPos.y];
    let mut feedback = world.resource_mut::<ImguiPlatformFeedback>();
    for (window, focused) in windows {
        if focused && ime_data.WantTextInput {
            feedback
                .ime_requests
                .entry(window)
                .or_insert(ImguiPlatformImeRequest {
                    wants_text_input: true,
                    input_position: Some(ImguiPlatformImePosition {
                        position: ime_position,
                        uses_desktop_coordinates,
                    }),
                });
        }
    }
}

#[cfg(feature = "render")]
pub(super) fn finish_platform_feedback(world: &mut World) {
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    let (native_monitor_diagnostics, native_monitor_diagnostic_epoch) = {
        let mut feedback = world.resource_mut::<ImguiPlatformFeedback>();
        feedback.native_monitor_diagnostic_epoch =
            feedback.native_monitor_diagnostic_epoch.saturating_add(1);
        (
            std::mem::take(&mut feedback.native_monitor_diagnostics),
            feedback.native_monitor_diagnostic_epoch,
        )
    };
    let (ime_requests, previous_ime_windows, cursor_requests, previous_cursor_windows) = {
        let mut feedback = world.resource_mut::<ImguiPlatformFeedback>();
        let ime_requests = std::mem::take(&mut feedback.ime_requests);
        let previous_ime_windows = std::mem::replace(
            &mut feedback.previous_ime_windows,
            ime_requests.keys().copied().collect(),
        );
        let cursor_requests = std::mem::take(&mut feedback.cursor_requests);
        let previous_cursor_windows = std::mem::replace(
            &mut feedback.previous_cursor_windows,
            cursor_requests.keys().copied().collect(),
        );
        (
            ime_requests,
            previous_ime_windows,
            cursor_requests,
            previous_cursor_windows,
        )
    };
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    world.resource::<crate::route::ImguiDiagnostics>().replace(
        crate::route::ImguiDiagnosticOrigin::NativeMonitor,
        native_monitor_diagnostic_epoch,
        native_monitor_diagnostics,
    );
    let mut affected_cursor_windows = previous_cursor_windows;
    affected_cursor_windows.extend(cursor_requests.keys().copied());
    let mut cursor_edits = Vec::new();
    {
        let mut query = world.query::<(Entity, &mut CursorOptions, Option<&mut CursorIcon>)>();
        for (entity, mut cursor_options, current_cursor_icon) in query.iter_mut(world) {
            if !affected_cursor_windows.contains(&entity) {
                continue;
            }
            let request = cursor_requests.get(&entity);
            cursor_options.visible = request.is_none_or(|request| request.visible);
            match (
                request.and_then(|request| request.icon.clone()),
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

    let mut affected_ime_windows = previous_ime_windows;
    affected_ime_windows.extend(ime_requests.keys().copied());
    let mut query = world.query::<(Entity, &mut Window, Option<&PrimaryWindow>)>();
    for (entity, mut window, primary_window) in query.iter_mut(world) {
        if !affected_ime_windows.contains(&entity) {
            continue;
        }
        let Some(request) = ime_requests.get(&entity) else {
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
                    if let Some((owner_context_id, instance_id)) = owner.window_identity()
                        && owner_context_id == context_id
                        && bridge.viewport_window_for_instance(instance_id) == Some(entity)
                        && let Some(viewport_id) = bridge.viewport_id(instance_id)
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
        if uses_desktop_coordinates
            && let Some(client_position) = crate::viewport::desktop_to_window_client_logical(
                _entity,
                &window.position,
                window.scale_factor(),
                ime_position,
            )
        {
            return Vec2::new(client_position[0], client_position[1]);
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

#[cfg(all(test, feature = "multi-viewport", not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::route::{
        ImguiNativeMonitorFailure as Failure, ImguiNativeMonitorFallback as Fallback,
    };
    use dear_imgui_winit::native_support::RectValidationError;

    #[test]
    fn monitor_collection_failures_are_reduced_to_stable_public_reasons() {
        assert_eq!(
            native_monitor_collection_failure(MonitorCollectionError::MainFactsUnavailable {
                monitor: 3,
            }),
            Failure::MainFactsUnavailable,
        );
        assert_eq!(
            native_monitor_collection_failure(MonitorCollectionError::InvalidMainGeometry {
                monitor: 4,
                reason: RectValidationError::EdgeOverflow,
            }),
            Failure::InvalidMainGeometry,
        );
        assert_eq!(
            native_monitor_collection_failure(MonitorCollectionError::InvalidScaleFactor {
                monitor: 5,
            }),
            Failure::InvalidScaleFactor,
        );
        assert_eq!(
            native_monitor_collection_failure(MonitorCollectionError::InvalidWorkGeometry {
                monitor: 6,
            }),
            Failure::InvalidWorkGeometry,
        );
    }

    #[test]
    fn work_area_fallbacks_are_reduced_to_stable_public_reasons() {
        let cases = [
            (WorkAreaFallback::Wayland, Fallback::Wayland),
            (
                WorkAreaFallback::UnsupportedWindowSystem,
                Fallback::UnsupportedWindowSystem,
            ),
            (
                WorkAreaFallback::SourceUnavailable,
                Fallback::SourceUnavailable,
            ),
            (
                WorkAreaFallback::InvalidNativeData,
                Fallback::InvalidNativeData,
            ),
            (
                WorkAreaFallback::MainThreadUnavailable,
                Fallback::MainThreadUnavailable,
            ),
            (
                WorkAreaFallback::AmbiguousDesktopScope,
                Fallback::AmbiguousDesktopScope,
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(native_monitor_fallback(input), expected);
        }
    }
}

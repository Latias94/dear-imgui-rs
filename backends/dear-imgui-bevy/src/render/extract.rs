//! Main-world to render-world extraction.

use super::resources::ImguiViewportTarget;
use super::*;

pub(super) fn extract_imgui_bevy_textures(
    registry: Extract<Option<Res<crate::ImguiBevyTextures>>>,
    mut extracted: ResMut<ImguiExtractedBevyTextures>,
) {
    let textures = registry
        .as_ref()
        .map(|registry| registry.iter().collect::<Vec<_>>())
        .unwrap_or_default();
    extracted.replace(textures);
}

pub(super) fn extract_imgui_render_frame(
    mut extracted: ResMut<ImguiExtractedRenderFrame>,
    output: Extract<Res<crate::ImguiFrameOutput>>,
    snapshot_mailbox: Res<crate::context::ImguiFrameMailbox>,
    renderer_release: Res<ImguiRendererRelease>,
    backend_status: Extract<Res<ImguiBackendStatus>>,
    primary_window: Extract<Query<Entity, With<PrimaryWindow>>>,
    viewport_windows: Extract<Query<(Entity, &ImguiViewportWindow)>>,
    cameras: Extract<OverlayCameraQuery<'_>>,
) {
    if renderer_release.release_requested() {
        snapshot_mailbox.clear();
        extracted.clear(output.frame_index());
        return;
    }
    let Some((frame_index, snapshot)) = snapshot_mailbox.take() else {
        extracted.clear(output.frame_index());
        return;
    };
    let primary_window = primary_window.single().ok();
    let viewport_targets = if backend_status.multi_viewport_supported {
        collect_viewport_targets(viewport_windows.iter())
    } else {
        Vec::new()
    };
    let camera_targets = collect_camera_targets(primary_window, &viewport_targets, cameras.iter());
    extracted.replace(frame_index, snapshot, camera_targets);
}

/// Normalize every active overlay camera target, including secondary windows.
///
/// The primary window is only special when a camera target uses `WindowRef::Primary`; any camera
/// that already points at a specific window entity keeps that route intact.
fn collect_camera_targets<'w>(
    primary_window: Option<Entity>,
    viewport_targets: &[ImguiViewportTarget],
    cameras: impl Iterator<
        Item = (
            Entity,
            &'w Camera,
            &'w RenderTarget,
            Option<&'w ImguiOverlayCamera>,
            Option<&'w ImguiOverlayDisabled>,
        ),
    >,
) -> Vec<ImguiCameraTarget> {
    let targets = cameras
        .filter(|(_, camera, _, _, overlay_disabled)| {
            camera.is_active && overlay_disabled.is_none()
        })
        .filter_map(|(entity, camera, target, overlay_camera, _)| {
            target
                .normalize(primary_window)
                .map(|target| ImguiCameraTarget {
                    camera: entity,
                    order: camera.order,
                    viewport_id: viewport_id_for_target(&target, viewport_targets),
                    camera_viewport: camera.viewport.as_ref().map(ImguiCameraViewport::from),
                    target,
                    explicit: overlay_camera.is_some(),
                })
        })
        .collect::<Vec<_>>();
    let mut targets = select_overlay_camera_per_target(targets);
    targets.sort_by_key(|target| (target.order, target.camera));
    targets
}

fn select_overlay_camera_per_target(targets: Vec<ImguiCameraTarget>) -> Vec<ImguiCameraTarget> {
    let mut by_render_target = HashMap::<NormalizedRenderTarget, ImguiCameraTarget>::new();
    for target in targets {
        match by_render_target.entry(target.target.clone()) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(target);
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let current = entry.get();
                if overlay_target_precedence(&target) >= overlay_target_precedence(current) {
                    entry.insert(target);
                }
            }
        }
    }
    by_render_target.into_values().collect()
}

fn overlay_target_precedence(target: &ImguiCameraTarget) -> (bool, isize, Entity) {
    (target.explicit, target.order, target.camera)
}

fn collect_viewport_targets<'w>(
    viewport_windows: impl Iterator<Item = (Entity, &'w ImguiViewportWindow)>,
) -> Vec<ImguiViewportTarget> {
    viewport_windows
        .map(|(window, viewport_window)| ImguiViewportTarget {
            viewport_id: viewport_window.viewport_id,
            window,
        })
        .collect()
}

fn viewport_id_for_target(
    target: &NormalizedRenderTarget,
    viewport_targets: &[ImguiViewportTarget],
) -> Option<imgui::Id> {
    let NormalizedRenderTarget::Window(window) = target else {
        return None;
    };
    let entity = window.entity();
    viewport_targets
        .iter()
        .find(|target| target.window == entity)
        .map(|target| target.viewport_id)
}

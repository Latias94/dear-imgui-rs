use std::collections::HashSet;

use bevy_camera::{
    Camera, Camera2d, CameraOutputMode, ClearColorConfig, RenderTarget, visibility::RenderLayers,
};
use bevy_core_pipeline::Core2d;
use bevy_ecs::prelude::{Commands, Entity};
use bevy_render::camera::CameraRenderGraph;
use bevy_window::WindowRef;
use dear_imgui_rs as imgui;

use super::{
    ImguiViewportBridgeContext, ImguiViewportCamera, ImguiViewportOwner,
    protocol::ImguiViewportInstanceId,
};

pub(super) type ViewportCameraIdentity = (ImguiViewportInstanceId, Entity);

pub(super) fn cleanup_orphaned_viewport_cameras(
    ecs_commands: &mut Commands,
    bridge: &ImguiViewportBridgeContext,
    viewport_cameras: impl Iterator<Item = Entity>,
    scheduled_camera_despawns: &HashSet<Entity>,
) {
    let live_cameras = viewport_cameras.collect::<HashSet<_>>();
    let mapped_cameras = bridge.mapped_camera_entities();
    let orphaned_cameras = live_cameras
        .into_iter()
        .filter(|camera| {
            !mapped_cameras.contains(camera) && !scheduled_camera_despawns.contains(camera)
        })
        .collect::<Vec<_>>();
    for camera in orphaned_cameras {
        bridge.track_ecs_despawn(camera);
        ecs_commands.entity(camera).despawn();
    }
}

pub(super) struct ViewportCameraReconciliation<'a> {
    pub(super) live: &'a HashSet<ViewportCameraIdentity>,
    pub(super) recoverable: &'a HashSet<ViewportCameraIdentity>,
    pub(super) pending: &'a mut HashSet<ImguiViewportInstanceId>,
}

pub(super) fn ensure_viewport_camera(
    ecs_commands: &mut Commands,
    bridge: &ImguiViewportBridgeContext,
    instance_id: ImguiViewportInstanceId,
    window_entity: Entity,
    transparent: bool,
    flags: imgui::ViewportFlags,
    cameras: ViewportCameraReconciliation<'_>,
) {
    let Some(viewport_id) = bridge.viewport_id(instance_id) else {
        return;
    };
    if let Some(camera) = bridge.viewport_camera_for_instance(instance_id) {
        let camera_identity = (instance_id, camera);
        if cameras.live.contains(&camera_identity) || cameras.pending.contains(&instance_id) {
            return;
        }
        if cameras.recoverable.contains(&camera_identity) {
            cameras.pending.insert(instance_id);
            ecs_commands.entity(camera).insert((
                Camera2d,
                viewport_camera(transparent, flags),
                RenderTarget::Window(WindowRef::Entity(window_entity)),
                CameraRenderGraph::new(Core2d),
                RenderLayers::none(),
                ImguiViewportCamera::new(instance_id, viewport_id),
            ));
            return;
        }
        bridge.remove_viewport_camera(instance_id);
    }
    if !cameras.pending.insert(instance_id) {
        return;
    }

    let camera = ecs_commands
        .spawn((
            Camera2d,
            viewport_camera(transparent, flags),
            RenderTarget::Window(WindowRef::Entity(window_entity)),
            CameraRenderGraph::new(Core2d),
            RenderLayers::none(),
            ImguiViewportCamera::new(instance_id, viewport_id),
            ImguiViewportOwner::camera(instance_id),
        ))
        .id();
    bridge.set_viewport_camera(instance_id, camera);
}

pub(super) fn viewport_camera(transparent: bool, flags: imgui::ViewportFlags) -> Camera {
    let mut camera = Camera::default();
    let clear_color = if flags.contains(imgui::ViewportFlags::NO_RENDERER_CLEAR) {
        ClearColorConfig::None
    } else if transparent {
        ClearColorConfig::Custom(bevy_color::Color::NONE)
    } else {
        ClearColorConfig::Default
    };
    camera.output_mode = CameraOutputMode::Write {
        blend_state: None,
        clear_color,
    };
    camera
}

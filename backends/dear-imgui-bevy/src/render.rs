//! Render-world extraction data for the Bevy backend.
//!
//! BEVY-080 stops at the extraction boundary: it clones the thread-safe
//! [`FrameSnapshot`](dear_imgui_rs::render::snapshot::FrameSnapshot) produced by the main-world
//! lifecycle and associates it with the Bevy cameras that should receive ImGui overlay rendering.
//! It deliberately does not build WGPU pipelines or borrow raw ImGui draw data across worlds.

use bevy_app::App;
use bevy_camera::{Camera, NormalizedRenderTarget, RenderTarget};
use bevy_ecs::prelude::*;
use bevy_render::{Extract, ExtractSchedule, RenderApp};
use bevy_window::PrimaryWindow;
use dear_imgui_rs as imgui;

/// Marker proving the render feature is compiled in.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct RenderFeature;

/// Camera/render-target association for an extracted ImGui overlay frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImguiCameraTarget {
    /// Main-world camera entity that should receive the ImGui overlay.
    pub camera: Entity,
    /// Camera order, preserved so the renderer can match Bevy's camera ordering.
    pub order: isize,
    /// Normalized render target resolved from the camera and current primary window.
    pub target: NormalizedRenderTarget,
}

/// Render-side copy of the last completed primary ImGui frame.
#[derive(Resource, Clone, Debug, Default)]
pub struct ImguiExtractedRenderFrame {
    frame_index: Option<u64>,
    snapshot: Option<imgui::render::snapshot::FrameSnapshot>,
    camera_targets: Vec<ImguiCameraTarget>,
}

impl ImguiExtractedRenderFrame {
    /// Frame index copied from [`ImguiFrameOutput`].
    #[must_use]
    pub fn frame_index(&self) -> Option<u64> {
        self.frame_index
    }

    /// Snapshot copied from the main/UI world.
    #[must_use]
    pub fn snapshot(&self) -> Option<&imgui::render::snapshot::FrameSnapshot> {
        self.snapshot.as_ref()
    }

    /// Camera targets associated with the extracted snapshot.
    #[must_use]
    pub fn camera_targets(&self) -> &[ImguiCameraTarget] {
        &self.camera_targets
    }

    fn replace(
        &mut self,
        frame_index: u64,
        snapshot: imgui::render::snapshot::FrameSnapshot,
        camera_targets: Vec<ImguiCameraTarget>,
    ) {
        self.frame_index = Some(frame_index);
        self.snapshot = Some(snapshot);
        self.camera_targets = camera_targets;
    }

    fn clear(&mut self, frame_index: u64) {
        self.frame_index = (frame_index > 0).then_some(frame_index);
        self.snapshot = None;
        self.camera_targets.clear();
    }
}

#[derive(Resource, Default)]
struct ImguiRenderExtractionInstalled;

pub(crate) fn install_render_extraction(app: &mut App) {
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };

    if render_app
        .world()
        .contains_resource::<ImguiRenderExtractionInstalled>()
    {
        return;
    }

    render_app
        .init_resource::<ImguiExtractedRenderFrame>()
        .insert_resource(ImguiRenderExtractionInstalled)
        .add_systems(ExtractSchedule, extract_imgui_render_frame);
}

fn extract_imgui_render_frame(
    mut extracted: ResMut<ImguiExtractedRenderFrame>,
    output: Extract<Res<crate::ImguiFrameOutput>>,
    primary_window: Extract<Query<Entity, With<PrimaryWindow>>>,
    cameras: Extract<Query<(Entity, &Camera, &RenderTarget)>>,
) {
    let Some(snapshot) = output.snapshot().cloned() else {
        extracted.clear(output.frame_index());
        return;
    };
    let primary_window = primary_window.single().ok();
    let camera_targets = collect_camera_targets(primary_window, cameras.iter());
    extracted.replace(output.frame_index(), snapshot, camera_targets);
}

fn collect_camera_targets<'w>(
    primary_window: Option<Entity>,
    cameras: impl Iterator<Item = (Entity, &'w Camera, &'w RenderTarget)>,
) -> Vec<ImguiCameraTarget> {
    let mut targets = cameras
        .filter(|(_, camera, _)| camera.is_active)
        .filter_map(|(entity, camera, target)| {
            target
                .normalize(primary_window)
                .map(|target| ImguiCameraTarget {
                    camera: entity,
                    order: camera.order,
                    target,
                })
        })
        .collect::<Vec<_>>();
    targets.sort_by_key(|target| target.order);
    targets
}

//! Renderer startup, Context release, and snapshot completion in the render world.

use super::resources::{ImguiRenderDeviceState, ImguiRendererReleasePacket};
use super::*;

pub(super) fn initialize_imgui_gpu_resources(world: &mut World) {
    let (device_generation, recovering) = world.resource_mut::<ImguiRenderDeviceState>().advance();

    if recovering {
        let renderer_releases = world.resource::<ImguiRendererReleases>().clone();
        let context_ids = renderer_releases.registered_contexts();
        let snapshot_mailbox = world
            .resource::<crate::context::ImguiFrameMailbox>()
            .clone();
        for context_id in &context_ids {
            snapshot_mailbox.remove_context(*context_id);
        }
        {
            let mut extracted = world.resource_mut::<ImguiExtractedRenderFrame>();
            for context_id in &context_ids {
                extracted.remove_context(*context_id);
            }
        }
        world.resource_mut::<ImguiPreparedRenderFrame>().clear();

        let packets = world.resource_scope(|world, mut bindings: Mut<ImguiTextureBindGroups>| {
            let mut gpu_resources = world.resource_mut::<ImguiPipelineGpuResources>();
            context_ids
                .into_iter()
                .map(|context_id| {
                    let textures = bindings.take_managed_renderer_state(context_id);
                    let uniforms = gpu_resources.take_context(context_id);
                    (
                        context_id,
                        ImguiRendererReleasePacket::new(textures, uniforms),
                    )
                })
                .collect()
        });
        assert!(
            renderer_releases.begin_device_recovery(device_generation, packets),
            "Bevy renderer device generation did not advance during RenderStartup"
        );
    }

    world.insert_resource(ImguiGpuBuffers::default());
    world.insert_resource(ImguiTextureBindGroups::default());
    world.insert_resource(ImguiQueuedPipelines::default());
    world.insert_resource(SpecializedRenderPipelines::<ImguiRenderPipeline>::default());
    let gpu_resources = ImguiPipelineGpuResources::from_world(world);
    world.insert_resource(gpu_resources);
}

pub(super) fn release_imgui_renderer_resources(
    renderer_releases: Res<ImguiRendererReleases>,
    snapshot_mailbox: Res<crate::context::ImguiFrameMailbox>,
    mut extracted: ResMut<ImguiExtractedRenderFrame>,
    mut texture_bind_groups: ResMut<ImguiTextureBindGroups>,
    mut gpu_resources: Option<ResMut<ImguiPipelineGpuResources>>,
) {
    for (context_id, generation) in renderer_releases.requested_releases() {
        snapshot_mailbox.remove_context(context_id);
        extracted.remove_context(context_id);
        let textures = texture_bind_groups.take_managed_renderer_state(context_id);
        let uniforms = gpu_resources
            .as_deref_mut()
            .map(|resources| resources.take_context(context_id))
            .unwrap_or_default();
        let packet = ImguiRendererReleasePacket::new(textures, uniforms);
        assert!(
            renderer_releases.acknowledge_release(context_id, generation, packet),
            "Bevy renderer release acknowledgement generation changed during cleanup"
        );
    }
}

pub(super) fn commit_imgui_render_frame(
    snapshot_mailbox: Res<crate::context::ImguiFrameMailbox>,
    mut extracted: ResMut<ImguiExtractedRenderFrame>,
    mut texture_bind_groups: ResMut<ImguiTextureBindGroups>,
) {
    for (context_id, source) in extracted.commit_all() {
        snapshot_mailbox.record_snapshot_commit_error(context_id, source);
    }
    // Use Core's confirmed progress because submitting feedback does not advance the watermark.
    texture_bind_groups
        .prune_destroyed_managed_textures(|context_id| extracted.completion_watermark(context_id));
}

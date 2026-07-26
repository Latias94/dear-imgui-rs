//! Render-world extraction data for the Bevy backend.
//!
//! The main world moves one [`FrameSnapshot`](dear_imgui_rs::render::snapshot::FrameSnapshot)
//! through a bounded mailbox and associates it with the Bevy cameras that should receive ImGui
//! overlay rendering. The render world owns the snapshot until request-bound texture feedback is
//! committed, without borrowing raw ImGui draw data across worlds.

use bevy_app::App;
use bevy_asset::{Assets, Handle, uuid_handle};
use bevy_camera::{
    Camera, CameraMainTextureUsages, CameraOutputMode, ClearColor, ClearColorConfig,
    CompositingSpace, NormalizedRenderTarget, RenderTarget, Viewport,
};
use bevy_core_pipeline::{Core2d, Core2dSystems, Core3d, Core3dSystems, upscaling::upscaling};
use bevy_ecs::entity::ContainsEntity;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;
use bevy_ecs::system::SystemParam;
use bevy_image::Image;
use bevy_math::{UVec2, UVec4};
use bevy_mesh::VertexBufferLayout;
use bevy_render::{
    Extract, ExtractSchedule, GpuResourceAppExt, Render, RenderApp, RenderSystems,
    camera::ExtractedCamera,
    render_asset::RenderAssets,
    render_resource::{
        BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor, BindingResource,
        BindingType, BlendState, Buffer, BufferAddress, BufferBindingType, BufferDescriptor,
        BufferSize, BufferUsages, COPY_BUFFER_ALIGNMENT, CachedRenderPipelineId, ColorTargetState,
        ColorWrites, CommandEncoderDescriptor, Extent3d, FilterMode, FragmentState, IndexFormat,
        LoadOp, MipmapFilterMode, MultisampleState, Operations, Origin3d, PipelineCache,
        PrimitiveState, PrimitiveTopology, RawBufferVec, RenderPassColorAttachment,
        RenderPassDescriptor, RenderPipelineDescriptor, Sampler, SamplerBindingType,
        SamplerDescriptor, ShaderStages, SpecializedRenderPipeline, SpecializedRenderPipelines,
        StoreOp, TexelCopyBufferLayout, TexelCopyTextureInfo, Texture, TextureAspect,
        TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
        TextureView, TextureViewDescriptor, TextureViewDimension, VertexAttribute, VertexFormat,
        VertexState, VertexStepMode, WgpuFeatures,
    },
    renderer::{
        RenderContext, RenderDevice, RenderGraph, RenderGraphSystems, RenderQueue, ViewQuery,
    },
    texture::GpuImage,
    view::{ExtractedView, ExtractedWindows, Msaa, RetainedViewEntity, ViewTarget},
};
use bevy_shader::Shader;
use bevy_window::{PrimaryWindow, Window};
use bytemuck::{Pod, Zeroable};
use dear_imgui_rs as imgui;
use imgui::render::{DrawCmdSnapshot, DrawIdx, SnapshotTextureId, TextureBinding};
use std::collections::{HashMap, HashSet};
use std::mem::size_of;
use std::sync::{Arc, Mutex, MutexGuard};

pub use crate::texture::ImguiBevyTextures;
use crate::{ImguiBackendStatus, ImguiViewportCamera, ImguiViewportWindow};

mod extract;
mod pass;
mod pipeline;
mod plugin;
mod prepare;
mod resources;

pub use pipeline::{
    IMGUI_FRAGMENT_ENTRY_POINT, IMGUI_SHADER_HANDLE, IMGUI_SHADER_SOURCE, IMGUI_VERTEX_ENTRY_POINT,
    ImguiGpuVertex, ImguiPipelineKey, ImguiRenderPipeline, ImguiUniforms,
    imgui_vertex_buffer_layout,
};
pub use plugin::{ImguiRenderSystems, ImguiUiRenderOrder, RenderFeature};
pub(crate) use plugin::{
    clear_standard_draw_callbacks_if_owned, install_render_extraction,
    install_standard_draw_callbacks_for_context, render_integration_available,
    standard_draw_callback_conflict, standard_draw_callback_contract,
    standard_draw_callback_occupied,
};
pub(crate) use resources::ImguiRendererRelease;
pub use resources::{
    ImguiCameraTarget, ImguiCameraViewport, ImguiExtractedBevyTextures, ImguiExtractedRenderFrame,
    ImguiGpuBuffers, ImguiOverlayCamera, ImguiOverlayDisabled, ImguiPipelineGpuResources,
    ImguiPreparedDraw, ImguiPreparedRenderFrame, ImguiQueuedPipelines, ImguiSampler,
    ImguiScissorRect, ImguiTextureBindGroupError, ImguiTextureBindGroups,
};

type ViewportCameraQuery<'w> = Query<
    'w,
    'w,
    (
        Entity,
        &'w Camera,
        &'w RenderTarget,
        &'w bevy_render::camera::CameraRenderGraph,
        &'w ImguiViewportCamera,
    ),
>;

type ViewportWindowQuery<'w> =
    Query<'w, 'w, (&'w Window, &'w ImguiViewportWindow), With<ImguiViewportWindow>>;

const COPY_BYTES_PER_ROW_ALIGNMENT: u32 = 256;
const MANAGED_TEXTURE_NAMESPACE: u64 = 0x4000_0000_0000_0000;

#[cfg(test)]
mod tests {
    use super::plugin::{
        imgui_bevy_draw_callback_linear, imgui_bevy_draw_callback_nearest,
        imgui_bevy_draw_callback_reset,
    };
    use super::prepare::{
        convert_imgui_texture_pixels, convert_imgui_texture_update_rects,
        draw_indices_reference_out_of_bounds, intersect_scissor_with_camera_viewport,
        managed_texture_extent_supported, prepare_bevy_image_texture_bind_groups,
        prepare_draw_data, retain_extracted_bevy_image_bindings, scissor_for_render_pass,
        scissor_from_clip_rect, validate_texture_update_rect,
    };
    use super::resources::{ImguiTextureViewCompatibility, pad_index_buffer_for_copy_alignment};
    use super::*;
    use bevy_asset::AssetId;
    use bevy_ecs::schedule::ScheduleLabel;
    use bevy_render::{renderer::initialize_renderer, settings::WgpuSettings};
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    use bevy_window::Window;

    type RawDrawCallback =
        unsafe extern "C" fn(*const imgui::sys::ImDrawList, *const imgui::sys::ImDrawCmd);

    fn assert_fn_ptr_eq(actual: imgui::sys::ImDrawCallback, expected: RawDrawCallback) {
        assert_eq!(
            actual.map(|callback| std::ptr::fn_addr_eq(callback, expected) as u8),
            Some(1)
        );
    }

    fn managed_context() -> imgui::Context {
        let mut context = imgui::Context::create();
        context.io_mut().set_config_input_trickle_event_queue(false);
        context.prepare_frame(
            imgui::FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
        );
        let _ = context.font_atlas().build();
        let _ = context.set_ini_filename::<std::path::PathBuf>(None);
        context
    }

    fn managed_snapshot(
        context: &mut imgui::Context,
        consumer: &imgui::render::RendererConsumer,
        texture: Option<imgui::ManagedTextureId>,
    ) -> imgui::render::FrameSnapshot {
        context.prepare_frame(
            imgui::FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
        );
        let frame = context.begin_frame();
        if let Some(texture) = texture {
            frame.ui().image(texture, [16.0, 16.0]);
        }
        frame
            .render_snapshot(consumer)
            .expect("managed frame should produce a Context-owned snapshot")
    }

    fn user_texture_request(
        snapshot: &imgui::render::FrameSnapshot,
        texture: imgui::ManagedTextureId,
    ) -> &imgui::render::snapshot::TextureRequest {
        snapshot
            .texture_requests()
            .iter()
            .find(|request| request.texture() == imgui::render::SnapshotTextureId::User(texture))
            .expect("snapshot should contain the user texture request")
    }

    fn register_test_texture(context: &mut imgui::Context) -> imgui::ManagedTextureId {
        let mut texture = imgui::texture::OwnedTextureData::new();
        texture.create(imgui::TextureFormat::RGBA32, 1, 1);
        texture.set_data(&[255, 255, 255, 255]);
        context.register_texture(texture)
    }

    #[test]
    fn renderer_release_rejects_a_stale_generation_acknowledgement() {
        let release = ImguiRendererRelease::default();
        release.install();
        release.update_resources_live(true);
        assert!(!release.request_release());
        let generation = release.requested_generation().unwrap();

        assert!(!release.acknowledge_release(generation + 1));
        assert_eq!(release.requested_generation(), Some(generation));
        assert!(release.acknowledge_release(generation));
        assert!(release.request_release());
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    #[test]
    fn acknowledged_release_freezes_extra_updates_until_viewport_world_teardown_finishes() {
        let mut app = App::new();
        app.add_plugins(bevy_render::extract_plugin::ExtractPlugin::default());
        app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
        app.add_plugins(crate::ImguiPlugin::new(crate::ImguiBackendConfig {
            name: "release-freeze".to_owned(),
            docking: true,
            multi_viewport: true,
            viewport_window: Default::default(),
        }));
        app.world_mut().spawn((Window::default(), PrimaryWindow));
        let primary_id = app
            .world()
            .get_non_send::<crate::ImguiContexts>()
            .unwrap()
            .primary_id()
            .unwrap();
        app.world_mut()
            .get_non_send_mut::<crate::ImguiContexts>()
            .unwrap()
            .configure(primary_id, |context| {
                context.font_atlas().build();
            })
            .unwrap();
        let viewport_id = imgui::Id::from(0xA11);
        app.world_mut()
            .get_non_send_mut::<crate::ImguiViewportBridge>()
            .unwrap()
            .queue(crate::ImguiViewportCommand::Create(
                crate::ImguiViewportSnapshot {
                    id: viewport_id,
                    pos: [10.0, 20.0],
                    size: [320.0, 180.0],
                    dpi_scale: 1.0,
                    flags: imgui::ViewportFlags::IS_PLATFORM_WINDOW,
                },
            ));
        app.update();
        assert!(
            app.world()
                .get_non_send::<crate::ImguiViewportBridge>()
                .unwrap()
                .viewport_window(viewport_id)
                .is_some()
        );

        let release = app.world().resource::<ImguiRendererRelease>().clone();
        release.update_resources_live(true);
        assert!(matches!(
            app.world_mut()
                .get_non_send_mut::<crate::ImguiContexts>()
                .unwrap()
                .remove(primary_id),
            Err(crate::ImguiContextError::RemovalPending {
                context_id,
                reason: crate::context::ownership::ImguiContextIntoInnerErrorReason::RenderWorldReleasePending,
            }) if context_id == primary_id
        ));
        assert!(
            app.world()
                .get_non_send::<crate::ImguiContexts>()
                .unwrap()
                .contains(primary_id),
            "a pending removal must retain Context ownership for retry"
        );
        let generation = release.requested_generation().unwrap();
        assert!(release.acknowledge_release(generation));
        assert!(release.release_requested());

        let frame_index = app
            .world()
            .get_non_send::<crate::context::ImguiFrameState>()
            .unwrap()
            .frame_index();
        app.update();
        assert_eq!(
            app.world()
                .get_non_send::<crate::context::ImguiFrameState>()
                .unwrap()
                .frame_index(),
            frame_index,
            "an acknowledged release must not resume native frame production"
        );

        assert!(matches!(
            app.world_mut()
                .get_non_send_mut::<crate::ImguiContexts>()
                .unwrap()
                .remove(primary_id),
            Err(crate::ImguiContextError::RemovalPending {
                context_id,
                reason: crate::context::ownership::ImguiContextIntoInnerErrorReason::ViewportWorldReleasePending,
            }) if context_id == primary_id
        ));
        app.update();
        let context = app
            .world_mut()
            .get_non_send_mut::<crate::ImguiContexts>()
            .unwrap()
            .remove(primary_id)
            .expect("a frozen extra update must complete ECS release without reopening a frame");
        assert_eq!(context.id(), primary_id);
        assert!(
            !app.world()
                .get_non_send::<crate::ImguiContexts>()
                .unwrap()
                .contains(primary_id),
            "a completed retry must remove exactly the requested Context"
        );
    }

    #[test]
    fn managed_texture_tombstone_blocks_revival_until_renderer_reset() {
        let mut context = managed_context();
        let first = imgui::render::SnapshotTextureId::User(register_test_texture(&mut context));
        let second = imgui::render::SnapshotTextureId::User(register_test_texture(&mut context));
        let mut bindings = ImguiTextureBindGroups::default();

        let first_renderer_id = bindings.managed_texture_id(first);
        assert!(matches!(
            bindings.validate_external_texture_id(first_renderer_id),
            Err(ImguiTextureBindGroupError::ManagedTextureIdInUse { texture })
                if texture == first_renderer_id
        ));
        bindings.destroy_managed_texture(first, 4);
        bindings.destroy_managed_texture(first, 5);
        assert!(bindings.managed_texture_is_destroyed(first));
        assert!(!bindings.managed_texture_ids.contains_key(&first));
        assert!(
            !bindings
                .managed_texture_aliases
                .contains_key(&first_renderer_id)
        );
        assert_eq!(
            bindings.validate_external_texture_id(first_renderer_id),
            Ok(())
        );

        let released = bindings.take_managed_renderer_state();
        assert!(released.is_empty());
        assert!(!bindings.managed_texture_is_destroyed(first));
        let second_renderer_id = bindings.managed_texture_id(second);
        assert_ne!(second_renderer_id, first_renderer_id);
    }

    #[test]
    fn managed_texture_tombstones_prune_after_completed_epochs() {
        let mut context = managed_context();
        let mut bindings = ImguiTextureBindGroups::default();
        let mut ids = Vec::new();
        for _ in 0..128 {
            ids.push(imgui::render::SnapshotTextureId::User(
                register_test_texture(&mut context),
            ));
        }

        for (epoch, id) in ids.iter().copied().enumerate() {
            bindings.destroy_managed_texture(id, epoch as u64 + 1);
        }
        assert_eq!(bindings.destroyed_managed_textures.len(), ids.len());
        assert!(bindings.managed_texture_is_destroyed(ids[0]));

        bindings.prune_destroyed_managed_textures(127);
        assert_eq!(bindings.destroyed_managed_textures.len(), 1);
        assert!(bindings.managed_texture_is_destroyed(ids[127]));

        bindings.prune_destroyed_managed_textures(128);
        assert!(bindings.destroyed_managed_textures.is_empty());
        assert!(
            !bindings.managed_texture_is_destroyed(ids[0]),
            "a tombstone is retired only after its destroy epoch is complete"
        );
    }

    #[test]
    fn stale_create_and_update_uploads_cannot_revive_a_destroyed_texture() {
        let mut context = managed_context();
        let texture = imgui::render::SnapshotTextureId::User(register_test_texture(&mut context));
        let mut bindings = ImguiTextureBindGroups::default();
        let renderer_id = bindings.managed_texture_id(texture);
        assert!(!renderer_id.is_null());
        bindings.destroy_managed_texture(texture, 7);

        for operation in ["Create", "Update"] {
            assert!(
                !bindings.accepts_managed_texture_upload(texture),
                "a stale {operation} request must be rejected by the same gate used by the render system"
            );
            if bindings.accepts_managed_texture_upload(texture) {
                let _ = bindings.managed_texture_id(texture);
            }
            assert!(
                !bindings.managed_texture_ids.contains_key(&texture),
                "a stale {operation} request must not allocate a replacement renderer ID"
            );
            assert!(
                !bindings
                    .textures
                    .contains_key(&TextureBinding::Managed(texture)),
                "a stale {operation} request must not restore a managed GPU binding"
            );
        }

        bindings.prune_destroyed_managed_textures(6);
        assert!(!bindings.accepts_managed_texture_upload(texture));
        bindings.prune_destroyed_managed_textures(7);
        assert!(bindings.accepts_managed_texture_upload(texture));
    }

    #[test]
    fn context_teardown_waits_for_render_world_release_before_native_mutation() {
        let context = managed_context();
        let mut owner = crate::context::ownership::ContextOwner::new(context.suspend());
        let backend = crate::context::ownership::BackendAttachment {
            config: crate::ImguiBackendConfig::default(),
            render_integration_installed: true,
        };
        owner.preflight_renderer_admission(&backend).unwrap();
        owner.commit_renderer_admission(&backend);
        let renderer_texture = imgui::TextureId::new(0xCAFE);
        let texture = owner
            .try_with_active_renderer_context(false, |context, consumer| {
                let consumer = consumer.expect("renderer admission must install a consumer");
                let texture = register_test_texture(context);
                let snapshot = managed_snapshot(context, consumer, Some(texture));
                let feedback = user_texture_request(&snapshot, texture)
                    .uploaded(renderer_texture)
                    .unwrap();
                snapshot.commit([feedback]).unwrap();
                context.poll_snapshot_completions().unwrap();
                Ok::<_, ()>(texture)
            })
            .unwrap();
        let release = ImguiRendererRelease::default();
        release.install();
        release.update_resources_live(true);
        owner.attach_renderer_release(release.clone());

        let error = owner
            .try_detach_backend()
            .expect_err("live render-world resources must prevent Context teardown");
        assert_eq!(
            error,
            crate::context::ownership::ImguiContextIntoInnerErrorReason::RenderWorldReleasePending
        );
        owner
            .try_with_active_renderer_context(false, |context, _| {
                context
                    .with_texture(texture, |texture| {
                        assert_eq!(texture.status(), imgui::TextureStatus::OK);
                        assert_eq!(texture.texture_id(), renderer_texture);
                    })
                    .unwrap();
                Ok::<_, ()>(())
            })
            .unwrap();

        let generation = release.requested_generation().unwrap();
        assert!(release.acknowledge_release(generation));
        owner
            .try_detach_backend()
            .expect("acknowledged render-world release should allow teardown");
        let mut context = owner.into_suspended();
        context
            .try_with_active(|context| {
                context
                    .with_texture(texture, |texture| {
                        assert_eq!(texture.status(), imgui::TextureStatus::WantCreate);
                        assert!(texture.texture_id().is_null());
                    })
                    .unwrap();
                Ok::<_, ()>(())
            })
            .unwrap();
    }

    #[test]
    fn extracted_frame_commits_request_bound_create_update_and_destroy_feedback() {
        let mut context = managed_context();
        let consumer = context.create_renderer_consumer().unwrap();

        let mut texture_data = imgui::texture::OwnedTextureData::new();
        texture_data.create(imgui::TextureFormat::RGBA32, 1, 1);
        texture_data.set_data(&[255, 0, 255, 255]);
        let texture = context.register_texture(texture_data);
        let renderer_texture = imgui::TextureId::new(0xBEEF);
        let mut extracted = ImguiExtractedRenderFrame::default();

        let create = managed_snapshot(&mut context, &consumer, Some(texture));
        assert_eq!(
            user_texture_request(&create, texture).kind(),
            imgui::render::snapshot::TextureRequestKind::Create
        );
        let feedback = user_texture_request(&create, texture)
            .uploaded(renderer_texture)
            .unwrap();
        extracted.replace(1, create, 0, Vec::new());
        extracted.extend_texture_feedback([feedback]);
        extracted.commit();
        let progress = context.poll_snapshot_completions().unwrap();
        assert_eq!(progress.committed(), 1);
        assert_eq!(progress.feedback_applied(), 1);
        context
            .with_texture(texture, |texture| {
                assert_eq!(texture.status(), imgui::TextureStatus::OK);
                assert_eq!(texture.texture_id(), renderer_texture);
            })
            .unwrap();

        context
            .with_texture_mut(texture, |mut texture| {
                texture.set_data(&[0, 255, 0, 255]);
            })
            .unwrap();
        let update = managed_snapshot(&mut context, &consumer, Some(texture));
        assert_eq!(
            user_texture_request(&update, texture).kind(),
            imgui::render::snapshot::TextureRequestKind::Update
        );
        let feedback = user_texture_request(&update, texture)
            .uploaded(renderer_texture)
            .unwrap();
        extracted.replace(2, update, 0, Vec::new());
        extracted.extend_texture_feedback([feedback]);
        extracted.commit();
        let progress = context.poll_snapshot_completions().unwrap();
        assert_eq!(progress.committed(), 1);
        assert_eq!(progress.feedback_applied(), 1);

        context.remove_texture(texture).unwrap();
        let destroy = managed_snapshot(&mut context, &consumer, None);
        assert_eq!(
            user_texture_request(&destroy, texture).kind(),
            imgui::render::snapshot::TextureRequestKind::Destroy
        );
        let feedback = user_texture_request(&destroy, texture).destroyed().unwrap();
        extracted.replace(3, destroy, 0, Vec::new());
        extracted.extend_texture_feedback([feedback]);
        extracted.commit();
        let progress = context.poll_snapshot_completions().unwrap();
        assert_eq!(progress.committed(), 1);
        assert_eq!(progress.feedback_applied(), 1);
        assert!(context.with_texture(texture, |_| ()).is_err());
    }

    #[test]
    fn extracted_render_frame_is_move_only() {
        trait AmbiguousIfClone<Marker> {
            fn assert_not_clone() {}
        }
        impl<T: ?Sized> AmbiguousIfClone<()> for T {}
        struct Invalid;
        impl<T: Clone> AmbiguousIfClone<Invalid> for T {}

        let _ = <ImguiExtractedRenderFrame as AmbiguousIfClone<_>>::assert_not_clone;
    }

    #[test]
    fn replacing_an_extracted_frame_abandons_only_the_previous_epoch() {
        let mut context = managed_context();
        let consumer = context.create_renderer_consumer().unwrap();
        let first = managed_snapshot(&mut context, &consumer, None);
        let second = managed_snapshot(&mut context, &consumer, None);
        let mut extracted = ImguiExtractedRenderFrame::default();

        extracted.replace(1, first, 0, Vec::new());
        extracted.replace(2, second, 0, Vec::new());
        extracted.commit();

        let progress = context.poll_snapshot_completions().unwrap();
        assert_eq!(progress.abandoned(), 1);
        assert_eq!(progress.committed(), 1);
        assert_eq!(progress.watermark(), 2);
        extracted.commit();
        let progress = context.poll_snapshot_completions().unwrap();
        assert_eq!(progress.watermark(), 2);
        assert_eq!(progress.committed(), 0);
        assert_eq!(progress.abandoned(), 0);
        assert_eq!(progress.feedback_applied(), 0);
    }

    #[test]
    fn mailbox_epoch_jump_abandons_every_skipped_snapshot_before_committing_the_latest() {
        let mut context = managed_context();
        let consumer = context.create_renderer_consumer().unwrap();
        let first = managed_snapshot(&mut context, &consumer, None);
        let second = managed_snapshot(&mut context, &consumer, None);
        let third = managed_snapshot(&mut context, &consumer, None);
        assert_eq!(first.epoch().sequence(), 1);
        assert_eq!(second.epoch().sequence(), 2);
        assert_eq!(third.epoch().sequence(), 3);

        let mailbox = crate::context::ImguiFrameMailbox::default();
        let mut extracted = ImguiExtractedRenderFrame::default();
        mailbox.publish(1, first);
        mailbox.publish(2, second);
        let (frame_index, second) = mailbox.take().unwrap();
        extracted.replace(frame_index, second, 0, Vec::new());
        mailbox.publish(3, third);
        let (frame_index, third) = mailbox.take().unwrap();
        extracted.replace(frame_index, third, 0, Vec::new());
        extracted.commit();

        let progress = context.poll_snapshot_completions().unwrap();
        assert_eq!(progress.abandoned(), 2);
        assert_eq!(progress.committed(), 1);
        assert_eq!(progress.watermark(), 3);
        assert_eq!(extracted.completion_watermark(), 3);
    }

    #[test]
    fn dropping_an_extracted_frame_abandons_its_epoch_once() {
        let mut context = managed_context();
        let consumer = context.create_renderer_consumer().unwrap();
        let snapshot = managed_snapshot(&mut context, &consumer, None);
        let mut extracted = ImguiExtractedRenderFrame::default();
        extracted.replace(1, snapshot, 0, Vec::new());

        drop(extracted);

        let progress = context.poll_snapshot_completions().unwrap();
        assert_eq!(progress.abandoned(), 1);
        assert_eq!(progress.committed(), 0);
        assert_eq!(progress.watermark(), 1);
        let progress = context.poll_snapshot_completions().unwrap();
        assert_eq!(progress.watermark(), 1);
        assert_eq!(progress.committed(), 0);
        assert_eq!(progress.abandoned(), 0);
        assert_eq!(progress.feedback_applied(), 0);
    }

    #[test]
    fn texture_conversion_repackages_padded_rgba_rows() {
        let pixels = [
            1, 2, 3, 4, 9, 9, 9, 9, //
            5, 6, 7, 8, 8, 8, 8, 8,
        ];

        let (converted, row_pitch) =
            convert_imgui_texture_pixels(imgui::texture::TextureFormat::RGBA32, 1, 2, 8, &pixels)
                .expect("valid padded RGBA32 upload should convert");

        assert_eq!(row_pitch, 4);
        assert_eq!(converted, [1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn texture_conversion_expands_alpha8_to_white_rgba() {
        let pixels = [0, 128, 255, 64];

        let (converted, row_pitch) =
            convert_imgui_texture_pixels(imgui::texture::TextureFormat::Alpha8, 2, 2, 2, &pixels)
                .expect("valid Alpha8 upload should convert");

        assert_eq!(row_pitch, 8);
        assert_eq!(
            converted,
            [
                255, 255, 255, 0, 255, 255, 255, 128, //
                255, 255, 255, 255, 255, 255, 255, 64,
            ]
        );
    }

    #[test]
    fn index_buffer_upload_pads_to_copy_alignment() {
        let mut indices = RawBufferVec::new(BufferUsages::INDEX);
        indices.push(1);
        indices.push(2);
        indices.push(3);

        pad_index_buffer_for_copy_alignment(&mut indices);

        assert_eq!(indices.len(), 4);
        assert_eq!(indices.values(), &vec![1, 2, 3, 0]);
    }

    #[test]
    fn gamma_helper_uses_srgb_for_srgb_targets_and_compositing_space() {
        assert_eq!(
            ImguiUniforms::gamma_for_target(TextureFormat::Rgba8UnormSrgb, None),
            2.2
        );
        assert_eq!(
            ImguiUniforms::gamma_for_target(
                TextureFormat::Rgba8Unorm,
                Some(CompositingSpace::Srgb)
            ),
            2.2
        );
        assert_eq!(
            ImguiUniforms::gamma_for_target(TextureFormat::Rgba8Unorm, None),
            1.0
        );
        assert_eq!(
            ImguiUniforms::gamma_for_target(
                TextureFormat::Rgba8Unorm,
                Some(CompositingSpace::Linear)
            ),
            1.0
        );
    }

    #[test]
    fn render_installation_exposes_standard_sampler_callbacks() {
        let mut app = App::new();
        app.add_plugins(bevy_render::extract_plugin::ExtractPlugin::default());
        app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
        app.add_plugins(crate::ImguiPlugin::default());

        let primary_id = app
            .world()
            .get_non_send::<crate::ImguiContexts>()
            .expect("ImguiPlugin should install the Context registry")
            .primary_id()
            .expect("ImguiPlugin should install the primary Context");
        app.world_mut()
            .get_non_send_mut::<crate::ImguiContexts>()
            .unwrap()
            .configure(primary_id, |context| {
                let platform_io = context.platform_io();
                assert_fn_ptr_eq(
                    platform_io.draw_callback_reset_render_state_raw(),
                    imgui_bevy_draw_callback_reset,
                );
                assert_fn_ptr_eq(
                    platform_io.draw_callback_set_sampler_linear_raw(),
                    imgui_bevy_draw_callback_linear,
                );
                assert_fn_ptr_eq(
                    platform_io.draw_callback_set_sampler_nearest_raw(),
                    imgui_bevy_draw_callback_nearest,
                );
            })
            .unwrap();
    }

    #[test]
    fn standard_draw_callback_markers_keep_distinct_addresses_and_snapshot_classes() {
        let callbacks = [
            imgui_bevy_draw_callback_reset as RawDrawCallback,
            imgui_bevy_draw_callback_linear as RawDrawCallback,
            imgui_bevy_draw_callback_nearest as RawDrawCallback,
        ];
        for left in 0..callbacks.len() {
            for right in left + 1..callbacks.len() {
                assert!(
                    !std::ptr::fn_addr_eq(callbacks[left], callbacks[right]),
                    "standard callback markers must remain pairwise distinct after optimization"
                );
            }
        }

        let mut context = managed_context();
        install_standard_draw_callbacks_for_context(&mut context).unwrap();
        let consumer = context.create_renderer_consumer().unwrap();
        context.prepare_frame(
            imgui::FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
        );
        let frame = context.begin_frame();
        {
            let draw_list = frame.ui().get_background_draw_list();
            draw_list.add_draw_cmd();
            for callback in callbacks {
                unsafe { draw_list.add_callback(Some(callback), std::ptr::null_mut(), 0) };
            }
        }
        let snapshot = frame.render_snapshot(&consumer).unwrap();
        let classes = snapshot
            .draw_data()
            .draw_lists
            .iter()
            .flat_map(|list| list.commands.iter())
            .filter_map(|command| match command {
                DrawCmdSnapshot::ResetRenderState => Some("reset"),
                DrawCmdSnapshot::SetSamplerLinear => Some("linear"),
                DrawCmdSnapshot::SetSamplerNearest => Some("nearest"),
                DrawCmdSnapshot::Elements { .. } => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(classes, ["reset", "linear", "nearest"]);
        drop(snapshot);
        context.poll_snapshot_completions().unwrap();
        let _ = context
            .prepare_renderer_texture_reset(&consumer)
            .unwrap()
            .commit();
    }

    #[test]
    fn prepared_draws_preserve_standard_sampler_callback_state() {
        let camera = Entity::from_raw_u32(7).expect("test entity index should be valid");
        let draw = imgui::render::DrawDataSnapshot {
            display_pos: [0.0, 0.0],
            display_size: [32.0, 32.0],
            framebuffer_scale: [1.0, 1.0],
            draw_lists: vec![imgui::render::DrawListSnapshot {
                vtx: vec![
                    imgui::render::DrawVert::new([0.0, 0.0], [0.0, 0.0], 0xFFFF_FFFF),
                    imgui::render::DrawVert::new([1.0, 0.0], [1.0, 0.0], 0xFFFF_FFFF),
                    imgui::render::DrawVert::new([0.0, 1.0], [0.0, 1.0], 0xFFFF_FFFF),
                ],
                idx: vec![0, 1, 2],
                commands: vec![
                    DrawCmdSnapshot::SetSamplerNearest,
                    DrawCmdSnapshot::Elements {
                        count: 3,
                        clip_rect: [0.0, 0.0, 16.0, 16.0],
                        texture: TextureBinding::Legacy(imgui::TextureId::new(1)),
                        vtx_offset: 0,
                        idx_offset: 0,
                    },
                    DrawCmdSnapshot::ResetRenderState,
                    DrawCmdSnapshot::Elements {
                        count: 3,
                        clip_rect: [0.0, 0.0, 16.0, 16.0],
                        texture: TextureBinding::Legacy(imgui::TextureId::new(1)),
                        vtx_offset: 0,
                        idx_offset: 0,
                    },
                ],
            }],
        };
        let targets = [camera_target_for_test(camera, None)];

        let (_, _, draws, _) = prepare_draw_data(&draw, &[], &targets);

        assert_eq!(draws.len(), 2);
        assert_eq!(draws[0].sampler, ImguiSampler::Nearest);
        assert_eq!(draws[1].sampler, ImguiSampler::Linear);
    }

    #[test]
    fn prepared_draws_preserve_sampler_state_across_draw_lists() {
        let camera = Entity::from_raw_u32(8).expect("test entity index should be valid");
        let draw = imgui::render::DrawDataSnapshot {
            display_pos: [0.0, 0.0],
            display_size: [32.0, 32.0],
            framebuffer_scale: [1.0, 1.0],
            draw_lists: vec![
                imgui::render::DrawListSnapshot {
                    vtx: vec![
                        imgui::render::DrawVert::new([0.0, 0.0], [0.0, 0.0], 0xFFFF_FFFF),
                        imgui::render::DrawVert::new([1.0, 0.0], [1.0, 0.0], 0xFFFF_FFFF),
                        imgui::render::DrawVert::new([0.0, 1.0], [0.0, 1.0], 0xFFFF_FFFF),
                    ],
                    idx: vec![0, 1, 2],
                    commands: vec![
                        DrawCmdSnapshot::SetSamplerNearest,
                        DrawCmdSnapshot::Elements {
                            count: 3,
                            clip_rect: [0.0, 0.0, 16.0, 16.0],
                            texture: TextureBinding::Legacy(imgui::TextureId::new(1)),
                            vtx_offset: 0,
                            idx_offset: 0,
                        },
                    ],
                },
                imgui::render::DrawListSnapshot {
                    vtx: vec![
                        imgui::render::DrawVert::new([2.0, 0.0], [0.0, 0.0], 0xFFFF_FFFF),
                        imgui::render::DrawVert::new([3.0, 0.0], [1.0, 0.0], 0xFFFF_FFFF),
                        imgui::render::DrawVert::new([2.0, 1.0], [0.0, 1.0], 0xFFFF_FFFF),
                    ],
                    idx: vec![0, 1, 2],
                    commands: vec![DrawCmdSnapshot::Elements {
                        count: 3,
                        clip_rect: [0.0, 0.0, 16.0, 16.0],
                        texture: TextureBinding::Legacy(imgui::TextureId::new(1)),
                        vtx_offset: 0,
                        idx_offset: 0,
                    }],
                },
            ],
        };
        let targets = [camera_target_for_test(camera, None)];

        let (_, _, draws, _) = prepare_draw_data(&draw, &[], &targets);

        assert_eq!(draws.len(), 2);
        assert_eq!(draws[0].sampler, ImguiSampler::Nearest);
        assert_eq!(draws[1].sampler, ImguiSampler::Nearest);
    }

    #[test]
    fn prepared_draws_skip_commands_with_out_of_range_index_or_vertex_offsets() {
        let camera = Entity::from_raw_u32(9).expect("test entity index should be valid");
        let draw = imgui::render::DrawDataSnapshot {
            display_pos: [0.0, 0.0],
            display_size: [32.0, 32.0],
            framebuffer_scale: [1.0, 1.0],
            draw_lists: vec![imgui::render::DrawListSnapshot {
                vtx: vec![
                    imgui::render::DrawVert::new([0.0, 0.0], [0.0, 0.0], 0xFFFF_FFFF),
                    imgui::render::DrawVert::new([1.0, 0.0], [1.0, 0.0], 0xFFFF_FFFF),
                    imgui::render::DrawVert::new([0.0, 1.0], [0.0, 1.0], 0xFFFF_FFFF),
                ],
                idx: vec![0, 1, 2, 3, 1, 2],
                commands: vec![
                    DrawCmdSnapshot::Elements {
                        count: 1,
                        clip_rect: [0.0, 0.0, 16.0, 16.0],
                        texture: TextureBinding::Legacy(imgui::TextureId::new(1)),
                        vtx_offset: 0,
                        idx_offset: 6,
                    },
                    DrawCmdSnapshot::Elements {
                        count: 1,
                        clip_rect: [0.0, 0.0, 16.0, 16.0],
                        texture: TextureBinding::Legacy(imgui::TextureId::new(1)),
                        vtx_offset: 0,
                        idx_offset: 3,
                    },
                    DrawCmdSnapshot::Elements {
                        count: 3,
                        clip_rect: [0.0, 0.0, 16.0, 16.0],
                        texture: TextureBinding::Legacy(imgui::TextureId::new(1)),
                        vtx_offset: 4,
                        idx_offset: 3,
                    },
                    DrawCmdSnapshot::Elements {
                        count: 3,
                        clip_rect: [0.0, 0.0, 16.0, 16.0],
                        texture: TextureBinding::Legacy(imgui::TextureId::new(1)),
                        vtx_offset: 0,
                        idx_offset: 0,
                    },
                ],
            }],
        };
        let targets = [camera_target_for_test(camera, None)];

        let (_, _, draws, _) = prepare_draw_data(&draw, &[], &targets);

        assert_eq!(draws.len(), 1);
        assert_eq!(draws[0].index_range, 0..3);
        assert_eq!(draws[0].vertex_offset, 0);
    }

    #[test]
    fn draw_index_validation_rejects_absolute_indices_outside_uploaded_vertices() {
        assert!(!draw_indices_reference_out_of_bounds(&[0, 1, 2], 0, 3));
        assert!(!draw_indices_reference_out_of_bounds(&[0, 1, 2], 3, 6));
        assert!(draw_indices_reference_out_of_bounds(&[3], 0, 3));
        assert!(draw_indices_reference_out_of_bounds(&[0], 3, 3));
    }

    #[test]
    fn scissor_rejects_non_finite_or_invalid_display_rects() {
        let mut draw = imgui::render::DrawDataSnapshot {
            display_pos: [0.0, 0.0],
            display_size: [32.0, 32.0],
            framebuffer_scale: [1.0, 1.0],
            draw_lists: Vec::new(),
        };

        assert!(scissor_from_clip_rect(&draw, [0.0, 0.0, 16.0, 16.0]).is_some());
        assert!(scissor_from_clip_rect(&draw, [f32::NAN, 0.0, 16.0, 16.0]).is_none());
        assert!(scissor_from_clip_rect(&draw, [8.0, 0.0, 8.0, 16.0]).is_none());

        draw.display_size = [0.0, 32.0];
        assert!(scissor_from_clip_rect(&draw, [0.0, 0.0, 16.0, 16.0]).is_none());

        draw.display_size = [32.0, 32.0];
        draw.framebuffer_scale = [f32::INFINITY, 1.0];
        assert!(scissor_from_clip_rect(&draw, [0.0, 0.0, 16.0, 16.0]).is_none());

        draw.framebuffer_scale = [-1.0, 1.0];
        assert!(scissor_from_clip_rect(&draw, [0.0, 0.0, 16.0, 16.0]).is_none());
    }

    #[test]
    fn render_pass_scissor_intersects_draws_with_camera_viewport_without_scaling() {
        let scissor = intersect_scissor_with_camera_viewport(
            ImguiScissorRect {
                x: 320,
                y: 180,
                width: 640,
                height: 360,
            },
            ImguiCameraViewport {
                physical_position: [640, 0],
                physical_size: [640, 360],
            },
        )
        .expect("valid scissor should map into a valid camera viewport");

        assert_eq!(
            scissor,
            ImguiScissorRect {
                x: 640,
                y: 180,
                width: 320,
                height: 180,
            }
        );
    }

    #[test]
    fn render_pass_scissor_is_clamped_to_real_render_target_size() {
        let scissor = scissor_for_render_pass(
            &ImguiPreparedDraw {
                context_id: test_context_id(),
                route_epoch: 0,
                camera: Entity::from_raw_u32(13).expect("test entity index should be valid"),
                view: RetainedViewEntity::new(
                    Entity::from_raw_u32(13)
                        .expect("test entity index should be valid")
                        .into(),
                    None,
                    0,
                ),
                order: 0,
                camera_order: 0,
                camera_schedule: Core2d.intern(),
                target: NormalizedRenderTarget::Window(
                    bevy_window::WindowRef::Entity(
                        Entity::from_raw_u32(14).expect("test entity index should be valid"),
                    )
                    .normalize(None)
                    .expect("entity window target should normalize"),
                ),
                target_format: TextureFormat::Rgba8UnormSrgb,
                texture_usages: TextureUsages::RENDER_ATTACHMENT,
                msaa: Msaa::Off,
                physical_target_size: [570, 390],
                viewport_id: Some(imgui::Id::from(0x570)),
                texture: TextureBinding::Legacy(imgui::TextureId::new(1)),
                sampler: ImguiSampler::Linear,
                scissor: ImguiScissorRect {
                    x: 0,
                    y: 0,
                    width: 570,
                    height: 392,
                },
                framebuffer_size: [570, 392],
                camera_viewport: None,
                index_range: 0..3,
                vertex_offset: 0,
            },
            Some([570, 390]),
        )
        .expect("overlapping scissor should be clipped instead of rejected");

        assert_eq!(
            scissor,
            ImguiScissorRect {
                x: 0,
                y: 0,
                width: 570,
                height: 390,
            },
            "render pass scissors must never exceed the real WGPU target extent"
        );
    }

    #[test]
    fn camera_viewport_uniforms_use_logical_viewport_rect_without_scaling_imgui_coordinates() {
        let camera = Entity::from_raw_u32(12).expect("test entity index should be valid");
        let draw = imgui::render::DrawDataSnapshot {
            display_pos: [0.0, 0.0],
            display_size: [640.0, 360.0],
            framebuffer_scale: [2.0, 2.0],
            draw_lists: vec![draw_list_for_test()],
        };
        let target = ImguiCameraTarget {
            camera_viewport: Some(ImguiCameraViewport {
                physical_position: [640, 0],
                physical_size: [640, 720],
            }),
            ..camera_target_for_test(camera, None)
        };

        let (_, _, draws, uniforms_by_view) = prepare_draw_data(&draw, &[], &[target]);

        assert_eq!(draws.len(), 1);
        assert_eq!(
            draws[0].scissor,
            ImguiScissorRect {
                x: 0,
                y: 0,
                width: 32,
                height: 32,
            },
            "prepared draw scissors stay in source framebuffer coordinates"
        );
        assert_eq!(
            scissor_for_render_pass(&draws[0], None),
            None,
            "commands outside the camera viewport are clipped instead of scaled into it"
        );
        assert_eq!(
            uniforms_by_view
                .get(&RetainedViewEntity::new(camera.into(), None, 0))
                .copied(),
            Some(ImguiUniforms::from_display_rect(
                [320.0, 0.0],
                [320.0, 360.0]
            ))
        );
    }

    #[test]
    fn prepared_draws_skip_only_viewports_with_invalid_display_rects() {
        let primary_camera = Entity::from_raw_u32(10).expect("test entity index should be valid");
        let secondary_camera = Entity::from_raw_u32(11).expect("test entity index should be valid");
        let secondary_viewport = imgui::Id::from(0xBEEF);
        let draw = imgui::render::DrawDataSnapshot {
            display_pos: [0.0, 0.0],
            display_size: [f32::NAN, 32.0],
            framebuffer_scale: [1.0, 1.0],
            draw_lists: vec![draw_list_for_test()],
        };
        let viewports = [imgui::render::ViewportDrawDataSnapshot::new(
            secondary_viewport,
            false,
            imgui::render::DrawDataSnapshot {
                display_pos: [0.0, 0.0],
                display_size: [32.0, 32.0],
                framebuffer_scale: [1.0, 1.0],
                draw_lists: vec![draw_list_for_test()],
            },
        )];
        let targets = [
            camera_target_for_test(primary_camera, None),
            camera_target_for_test(secondary_camera, Some(secondary_viewport)),
        ];

        let (_, _, draws, uniforms_by_view) = prepare_draw_data(&draw, &viewports, &targets);

        assert_eq!(draws.len(), 1);
        assert_eq!(draws[0].camera, secondary_camera);
        assert!(!uniforms_by_view.contains_key(&RetainedViewEntity::new(
            primary_camera.into(),
            None,
            0
        )));
        assert!(uniforms_by_view.contains_key(&RetainedViewEntity::new(
            secondary_camera.into(),
            None,
            0
        )));
    }

    #[test]
    fn bevy_image_binding_tracking_prunes_unregistered_legacy_ids() {
        let mut texture_bind_groups = ImguiTextureBindGroups::default();
        let registered = TextureBinding::Legacy(imgui::TextureId::new(42));
        let still_active = TextureBinding::Legacy(imgui::TextureId::new(43));

        texture_bind_groups.bevy_image_bindings.insert(registered);
        texture_bind_groups.bevy_image_bindings.insert(still_active);

        texture_bind_groups.retain_bevy_image_bindings(&HashSet::from([still_active]));

        assert!(
            !texture_bind_groups
                .bevy_image_bindings
                .contains(&registered)
        );
        assert!(
            texture_bind_groups
                .bevy_image_bindings
                .contains(&still_active)
        );
    }

    #[test]
    fn extracted_bevy_image_binding_retention_does_not_require_gpu_images() {
        let mut extracted = ImguiExtractedBevyTextures::default();
        let mut texture_bind_groups = ImguiTextureBindGroups::default();
        let stale = TextureBinding::Legacy(imgui::TextureId::new(42));
        let still_active_id = imgui::TextureId::new(43);
        let still_active = TextureBinding::Legacy(still_active_id);

        texture_bind_groups.bevy_image_bindings.insert(stale);
        texture_bind_groups.bevy_image_bindings.insert(still_active);
        extracted.replace(vec![(still_active_id, AssetId::<Image>::default())]);

        retain_extracted_bevy_image_bindings(&extracted, &mut texture_bind_groups);

        assert!(!texture_bind_groups.bevy_image_bindings.contains(&stale));
        assert!(
            texture_bind_groups
                .bevy_image_bindings
                .contains(&still_active)
        );
    }

    #[test]
    fn bevy_image_sampling_compatibility_accepts_filterable_float_2d_views() {
        assert!(
            imgui_texture_view_compatibility(TextureFormat::Rgba8Unorm)
                .supports_imgui_sampling(WgpuFeatures::empty())
        );
        assert!(
            imgui_texture_view_compatibility(TextureFormat::Rgba8UnormSrgb)
                .supports_imgui_sampling(WgpuFeatures::empty())
        );
        assert!(
            imgui_texture_view_compatibility(TextureFormat::Rgba32Float)
                .supports_imgui_sampling(WgpuFeatures::FLOAT32_FILTERABLE)
        );
    }

    #[test]
    fn bevy_image_sampling_compatibility_rejects_views_that_cannot_match_imgui_layout() {
        let unsupported_cases = [
            imgui_texture_view_compatibility(TextureFormat::Rgba8Uint),
            imgui_texture_view_compatibility(TextureFormat::Rgba8Sint),
            imgui_texture_view_compatibility(TextureFormat::Depth32Float),
            imgui_texture_view_compatibility(TextureFormat::Rgba32Float),
            ImguiTextureViewCompatibility {
                texture_usage: TextureUsages::COPY_DST,
                ..imgui_texture_view_compatibility(TextureFormat::Rgba8Unorm)
            },
            ImguiTextureViewCompatibility {
                view_usage: Some(TextureUsages::COPY_SRC),
                ..imgui_texture_view_compatibility(TextureFormat::Rgba8Unorm)
            },
            ImguiTextureViewCompatibility {
                sample_count: 4,
                ..imgui_texture_view_compatibility(TextureFormat::Rgba8Unorm)
            },
            ImguiTextureViewCompatibility {
                texture_dimension: TextureDimension::D3,
                ..imgui_texture_view_compatibility(TextureFormat::Rgba8Unorm)
            },
            ImguiTextureViewCompatibility {
                depth_or_array_layers: 2,
                view_dimension: None,
                ..imgui_texture_view_compatibility(TextureFormat::Rgba8Unorm)
            },
            ImguiTextureViewCompatibility {
                depth_or_array_layers: 2,
                view_dimension: Some(TextureViewDimension::D2Array),
                ..imgui_texture_view_compatibility(TextureFormat::Rgba8Unorm)
            },
        ];

        for compatibility in unsupported_cases {
            assert!(
                !compatibility.supports_imgui_sampling(WgpuFeatures::empty()),
                "{compatibility:?} should not be bound to the fixed ImGui texture layout"
            );
        }
    }

    #[test]
    fn managed_texture_extent_validation_rejects_zero_or_device_oversized_textures() {
        assert!(managed_texture_extent_supported(1, 1, 2048));
        assert!(managed_texture_extent_supported(2048, 2048, 2048));
        assert!(!managed_texture_extent_supported(0, 1, 2048));
        assert!(!managed_texture_extent_supported(1, 0, 2048));
        assert!(!managed_texture_extent_supported(2049, 1, 2048));
        assert!(!managed_texture_extent_supported(1, 2049, 2048));
    }

    #[test]
    fn texture_update_rect_validation_rejects_empty_or_out_of_bounds_rects() {
        assert!(validate_texture_update_rect(
            64,
            32,
            imgui::TextureRect {
                x: 8,
                y: 4,
                w: 16,
                h: 8,
            },
        ));
        assert!(validate_texture_update_rect(
            64,
            32,
            imgui::TextureRect {
                x: 63,
                y: 31,
                w: 1,
                h: 1,
            },
        ));
        assert!(!validate_texture_update_rect(
            64,
            32,
            imgui::TextureRect {
                x: 8,
                y: 4,
                w: 0,
                h: 8,
            },
        ));
        assert!(!validate_texture_update_rect(
            64,
            32,
            imgui::TextureRect {
                x: 8,
                y: 4,
                w: 16,
                h: 0,
            },
        ));
        assert!(!validate_texture_update_rect(
            64,
            32,
            imgui::TextureRect {
                x: 63,
                y: 0,
                w: 2,
                h: 1,
            },
        ));
        assert!(!validate_texture_update_rect(
            64,
            32,
            imgui::TextureRect {
                x: 0,
                y: 31,
                w: 1,
                h: 2,
            },
        ));
    }

    #[test]
    fn texture_update_rect_conversion_requires_every_requested_rect_to_convert() {
        let valid = imgui::render::snapshot::TextureUploadRect {
            rect: imgui::TextureRect {
                x: 0,
                y: 0,
                w: 2,
                h: 1,
            },
            row_pitch: 8,
            data: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };

        let converted = convert_imgui_texture_update_rects(
            imgui::texture::TextureFormat::RGBA32,
            4,
            4,
            std::slice::from_ref(&valid),
        )
        .expect("valid update rect should convert");

        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0].origin, Origin3d { x: 0, y: 0, z: 0 });
        assert_eq!(converted[0].width, 2);
        assert_eq!(converted[0].height, 1);
        assert_eq!(converted[0].row_pitch, 8);
        assert_eq!(converted[0].pixels, valid.data);

        assert!(
            convert_imgui_texture_update_rects(imgui::texture::TextureFormat::RGBA32, 4, 4, &[],)
                .is_none(),
            "empty update lists must not acknowledge a texture update as complete"
        );

        let out_of_bounds = imgui::render::snapshot::TextureUploadRect {
            rect: imgui::TextureRect {
                x: 3,
                y: 0,
                w: 2,
                h: 1,
            },
            row_pitch: 8,
            data: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        assert!(
            convert_imgui_texture_update_rects(
                imgui::texture::TextureFormat::RGBA32,
                4,
                4,
                &[
                    imgui::render::snapshot::TextureUploadRect {
                        rect: imgui::TextureRect {
                            x: 0,
                            y: 0,
                            w: 2,
                            h: 1,
                        },
                        row_pitch: 8,
                        data: vec![1, 2, 3, 4, 5, 6, 7, 8],
                    },
                    out_of_bounds,
                ],
            )
            .is_none(),
            "one invalid rect should keep the whole texture update pending"
        );

        let short_row = imgui::render::snapshot::TextureUploadRect {
            rect: imgui::TextureRect {
                x: 0,
                y: 0,
                w: 2,
                h: 1,
            },
            row_pitch: 4,
            data: vec![1, 2, 3, 4],
        };
        assert!(
            convert_imgui_texture_update_rects(
                imgui::texture::TextureFormat::RGBA32,
                4,
                4,
                &[valid, short_row],
            )
            .is_none(),
            "one unconvertible rect should keep the whole texture update pending"
        );
    }

    #[test]
    #[ignore = "requires DEAR_IMGUI_BEVY_GPU_HARNESS=1 and a working native wgpu adapter"]
    fn bevy_image_texture_bind_groups_use_real_render_assets_when_gpu_harness_is_enabled() {
        if std::env::var_os("DEAR_IMGUI_BEVY_GPU_HARNESS").is_none() {
            return;
        }

        let RenderHarnessResources {
            render_device,
            pipeline_cache,
        } = initialize_render_harness_resources();
        let pipeline = ImguiRenderPipeline::default();
        let mut extracted = ImguiExtractedBevyTextures::default();
        let mut gpu_images = RenderAssets::<GpuImage>::default();
        let mut texture_bind_groups = ImguiTextureBindGroups::default();
        let texture_id = imgui::TextureId::new(42);
        let image_id = AssetId::<Image>::default();
        let binding = TextureBinding::Legacy(texture_id);

        extracted.replace(vec![(texture_id, image_id)]);
        gpu_images.insert(
            image_id,
            gpu_image(&render_device, TextureUsages::TEXTURE_BINDING),
        );

        prepare_bevy_image_texture_bind_groups(
            Some(&gpu_images),
            &extracted,
            &render_device,
            &pipeline_cache,
            &pipeline,
            &mut texture_bind_groups,
        );

        assert_eq!(texture_bind_groups.len(), 1);
        assert!(
            texture_bind_groups
                .get(&binding, ImguiSampler::Linear)
                .is_some(),
            "registered Bevy image handles should resolve to a real bind group"
        );

        gpu_images.remove(image_id);
        prepare_bevy_image_texture_bind_groups(
            Some(&gpu_images),
            &extracted,
            &render_device,
            &pipeline_cache,
            &pipeline,
            &mut texture_bind_groups,
        );
        assert!(
            texture_bind_groups.is_empty(),
            "missing RenderAssets<GpuImage> entries should remove stale bind groups"
        );

        gpu_images.insert(
            image_id,
            gpu_image(&render_device, TextureUsages::TEXTURE_BINDING),
        );
        extracted.replace(vec![(texture_id, image_id)]);
        prepare_bevy_image_texture_bind_groups(
            Some(&gpu_images),
            &extracted,
            &render_device,
            &pipeline_cache,
            &pipeline,
            &mut texture_bind_groups,
        );
        assert_eq!(texture_bind_groups.len(), 1);

        extracted.replace(Vec::new());
        prepare_bevy_image_texture_bind_groups(
            Some(&gpu_images),
            &extracted,
            &render_device,
            &pipeline_cache,
            &pipeline,
            &mut texture_bind_groups,
        );
        assert!(
            texture_bind_groups.is_empty(),
            "unregistered Bevy image handles should remove stale bind groups"
        );
    }

    #[test]
    #[ignore = "requires DEAR_IMGUI_BEVY_GPU_HARNESS=1 and a working native wgpu adapter"]
    fn bevy_image_texture_bind_groups_ignore_non_sampled_gpu_images_when_gpu_harness_is_enabled() {
        if std::env::var_os("DEAR_IMGUI_BEVY_GPU_HARNESS").is_none() {
            return;
        }

        let RenderHarnessResources {
            render_device,
            pipeline_cache,
        } = initialize_render_harness_resources();
        let pipeline = ImguiRenderPipeline::default();
        let mut extracted = ImguiExtractedBevyTextures::default();
        let mut gpu_images = RenderAssets::<GpuImage>::default();
        let mut texture_bind_groups = ImguiTextureBindGroups::default();
        let texture_id = imgui::TextureId::new(99);
        let image_id = AssetId::<Image>::default();
        let binding = TextureBinding::Legacy(texture_id);

        extracted.replace(vec![(texture_id, image_id)]);
        gpu_images.insert(image_id, gpu_image(&render_device, TextureUsages::COPY_DST));

        prepare_bevy_image_texture_bind_groups(
            Some(&gpu_images),
            &extracted,
            &render_device,
            &pipeline_cache,
            &pipeline,
            &mut texture_bind_groups,
        );

        assert_eq!(texture_bind_groups.len(), 0);
        assert!(
            texture_bind_groups
                .get(&binding, ImguiSampler::Linear)
                .is_none()
        );
    }

    struct RenderHarnessResources {
        render_device: RenderDevice,
        pipeline_cache: PipelineCache,
    }

    fn initialize_render_harness_resources() -> RenderHarnessResources {
        let settings = WgpuSettings::default();

        let resources = bevy_platform::future::block_on(initialize_renderer(
            settings
                .backends
                .expect("render harness should configure an explicit backend"),
            None,
            &settings,
        ));
        let render_device = resources.0.clone();
        let render_adapter = resources.3.clone();
        RenderHarnessResources {
            render_device: render_device.clone(),
            pipeline_cache: PipelineCache::new(render_device, render_adapter, true),
        }
    }

    fn imgui_texture_view_compatibility(format: TextureFormat) -> ImguiTextureViewCompatibility {
        ImguiTextureViewCompatibility {
            texture_usage: TextureUsages::TEXTURE_BINDING,
            view_usage: None,
            sample_count: 1,
            texture_dimension: TextureDimension::D2,
            depth_or_array_layers: 1,
            view_dimension: None,
            format,
            aspect: TextureAspect::All,
        }
    }

    fn gpu_image(render_device: &RenderDevice, usage: TextureUsages) -> GpuImage {
        let texture_descriptor = TextureDescriptor {
            label: Some("dear_imgui_bevy_harness_image"),
            size: Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage,
            view_formats: &[],
        };
        let texture = render_device.create_texture(&texture_descriptor);
        let texture_view = texture.create_view(&TextureViewDescriptor::default());
        let sampler = render_device.create_sampler(&SamplerDescriptor::default());
        GpuImage {
            texture,
            texture_view,
            sampler,
            texture_descriptor,
            texture_view_descriptor: None,
            had_data: true,
        }
    }

    fn draw_list_for_test() -> imgui::render::DrawListSnapshot {
        imgui::render::DrawListSnapshot {
            vtx: vec![
                imgui::render::DrawVert::new([0.0, 0.0], [0.0, 0.0], 0xFFFF_FFFF),
                imgui::render::DrawVert::new([1.0, 0.0], [1.0, 0.0], 0xFFFF_FFFF),
                imgui::render::DrawVert::new([0.0, 1.0], [0.0, 1.0], 0xFFFF_FFFF),
            ],
            idx: vec![0, 1, 2],
            commands: vec![DrawCmdSnapshot::Elements {
                count: 3,
                clip_rect: [0.0, 0.0, 16.0, 16.0],
                texture: TextureBinding::Legacy(imgui::TextureId::new(1)),
                vtx_offset: 0,
                idx_offset: 0,
            }],
        }
    }

    fn camera_target_for_test(camera: Entity, viewport_id: Option<imgui::Id>) -> ImguiCameraTarget {
        ImguiCameraTarget {
            context_id: test_context_id(),
            route_epoch: 0,
            camera,
            view: RetainedViewEntity::new(camera.into(), None, 0),
            order: 0,
            camera_order: 0,
            camera_schedule: Core2d.intern(),
            target: NormalizedRenderTarget::Window(
                bevy_window::WindowRef::Entity(camera)
                    .normalize(None)
                    .expect("entity window target should normalize"),
            ),
            target_format: TextureFormat::Rgba8UnormSrgb,
            texture_usages: TextureUsages::RENDER_ATTACHMENT,
            msaa: Msaa::Off,
            physical_target_size: [64, 64],
            viewport_id,
            camera_viewport: None,
        }
    }

    fn test_context_id() -> imgui::ContextId {
        imgui::SuspendedContext::create().id()
    }
}

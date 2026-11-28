use super::*;
use crate::render::texture::{
    ImguiTextureViewCompatibility, convert_imgui_texture_pixels,
    convert_imgui_texture_update_rects, managed_texture_extent_supported,
    prepare_bevy_image_texture_bind_groups, prepare_managed_texture_request,
};
use bevy_asset::AssetId;
use dear_imgui_backend_support::BackendError;

#[test]
fn atlas_tombstone_blocks_only_its_allocation_until_renderer_reset() {
    let context_id = test_context_id();
    let atlas = |stamp, generation| SnapshotTextureId::FontAtlas {
        context: context_id,
        stamp,
        generation,
    };
    let destroyed = atlas(1, 1);
    let mut bindings = ImguiTextureBindGroups::default();

    bindings.destroy_managed_texture(destroyed, 4);
    assert!(!bindings.accepts_managed_texture_upload(destroyed));
    assert!(bindings.accepts_managed_texture_upload(atlas(1, 2)));
    assert!(bindings.accepts_managed_texture_upload(atlas(2, 1)));

    let released = bindings.take_managed_renderer_state(context_id);
    assert!(released.is_empty());
    assert!(bindings.accepts_managed_texture_upload(destroyed));
}

#[test]
fn managed_texture_tombstones_prune_after_the_owning_context_completes_the_latest_destroy() {
    let mut context = managed_context();
    let owner = context.id();
    let early = SnapshotTextureId::User(register_test_texture(&mut context));
    let late = SnapshotTextureId::User(register_test_texture(&mut context));
    let unrelated = test_context_id();
    let mut bindings = ImguiTextureBindGroups::default();

    bindings.destroy_managed_texture(early, 3);
    bindings.destroy_managed_texture(late, 5);
    bindings.destroy_managed_texture(late, 4);
    bindings.prune_destroyed_managed_textures(|id| (id == unrelated).then_some(5));
    assert!(!bindings.accepts_managed_texture_upload(early));
    assert!(!bindings.accepts_managed_texture_upload(late));

    bindings.prune_destroyed_managed_textures(|id| (id == owner).then_some(4));
    assert!(bindings.accepts_managed_texture_upload(early));
    assert!(!bindings.accepts_managed_texture_upload(late));
    bindings.prune_destroyed_managed_textures(|id| (id == owner).then_some(5));
    assert!(bindings.accepts_managed_texture_upload(late));
}

#[test]
fn texture_conversion_borrows_padded_rgba_rows_through_the_last_pixel() {
    let pixels = [1, 2, 3, 4, 9, 9, 9, 9, 5, 6, 7, 8];

    let (converted, row_pitch) =
        convert_imgui_texture_pixels(imgui::texture::TextureFormat::RGBA32, [1, 2], 8, &pixels)
            .expect("valid padded RGBA32 upload should convert");

    assert_eq!(row_pitch, 8);
    assert!(matches!(converted, std::borrow::Cow::Borrowed(source) if source == pixels));
}

#[test]
fn texture_conversion_expands_alpha8_to_white_rgba() {
    let pixels = [0, 128, 255, 64];

    let (converted, row_pitch) =
        convert_imgui_texture_pixels(imgui::texture::TextureFormat::Alpha8, [2, 2], 2, &pixels)
            .expect("valid Alpha8 upload should convert");

    assert_eq!(row_pitch, 8);
    assert_eq!(
        converted.as_ref(),
        &[
            255, 255, 255, 0, 255, 255, 255, 128, //
            255, 255, 255, 255, 255, 255, 255, 64,
        ]
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
fn texture_update_rect_conversion_requires_every_requested_rect_to_convert() {
    let texture = SnapshotTextureId::FontAtlas {
        context: test_context_id(),
        stamp: 1,
        generation: 1,
    };
    let extent = [4, 4];
    let valid = imgui::render::snapshot::TextureUploadRect {
        rect: imgui::TextureRect {
            x: 2,
            y: 3,
            w: 2,
            h: 1,
        },
        row_pitch: 8,
        data: vec![1, 2, 3, 4, 5, 6, 7, 8],
    };
    let converted = convert_imgui_texture_update_rects(
        texture,
        imgui::TextureFormat::RGBA32,
        extent,
        std::slice::from_ref(&valid),
    )
    .expect("valid update rect should convert");
    assert_eq!(converted.len(), 1);
    assert_eq!(converted[0].origin, Origin3d { x: 2, y: 3, z: 0 });
    assert_eq!(converted[0].extent, [2, 1]);
    assert_eq!(converted[0].row_pitch, 8);
    assert_eq!(converted[0].pixels.as_ref(), valid.data.as_slice());
    drop(converted);

    let validate = |rects: &[imgui::render::snapshot::TextureUploadRect]| {
        convert_imgui_texture_update_rects(texture, imgui::TextureFormat::RGBA32, extent, rects)
            .map(drop)
    };
    assert!(matches!(
        validate(&[]),
        Err(BackendError::TextureUpload { .. })
    ));

    for rect in [
        imgui::TextureRect { x: 3, ..valid.rect },
        imgui::TextureRect { w: 0, ..valid.rect },
    ] {
        let invalid = imgui::render::snapshot::TextureUploadRect {
            rect,
            row_pitch: valid.row_pitch,
            data: valid.data.clone(),
        };
        assert!(matches!(
            validate(&[valid.clone(), invalid]),
            Err(BackendError::InvalidTextureRect { rect: rejected, texture_size })
                if rejected == rect && texture_size == extent
        ));
    }

    let short_row = imgui::render::snapshot::TextureUploadRect {
        row_pitch: 4,
        ..valid.clone()
    };
    assert!(matches!(
        validate(&[valid, short_row]),
        Err(BackendError::TextureUpload { binding: TextureBinding::Managed(id), .. }) if id == texture
    ));
}

#[test]
#[ignore = "requires DEAR_IMGUI_BEVY_GPU_HARNESS=1 and a working native wgpu adapter"]
fn managed_texture_bindings_and_aliases_are_released_per_context() {
    if std::env::var_os("DEAR_IMGUI_BEVY_GPU_HARNESS").is_none() {
        return;
    }

    let RenderHarnessResources {
        render_device,
        render_queue,
        pipeline_cache,
    } = initialize_render_harness_resources();
    let pipeline = ImguiRenderPipeline::default();
    let mut context_a = managed_context();
    let mut context_b = managed_context();
    let mut bindings = ImguiTextureBindGroups::default();
    let ((id_a, alias_a), (id_b, alias_b)) = {
        let mut upload_texture = |context: &mut imgui::Context| {
            let id = register_test_texture(context);
            let consumer = context.create_detached_renderer_consumer().unwrap();
            let frame = managed_snapshot(context, &consumer, Some(id));
            let uploaded = prepare_managed_texture_request(
                user_texture_request(&frame, id),
                frame.epoch().sequence(),
                &mut bindings,
                &render_device,
                &render_queue,
                &pipeline_cache,
                &pipeline,
            )
            .expect("a live managed texture should upload");
            let feedback = frame
                .texture_requests()
                .iter()
                .filter(|request| request.texture() != SnapshotTextureId::User(id))
                .map(imgui::render::snapshot::TextureRequest::retry)
                .chain(std::iter::once(uploaded))
                .collect::<Vec<_>>();
            frame.commit(feedback).unwrap();
            context.poll_snapshot_completions().unwrap();
            let alias = context
                .with_texture(id, |texture| texture.texture_id())
                .unwrap();
            (SnapshotTextureId::User(id), alias)
        };
        (
            upload_texture(&mut context_a),
            upload_texture(&mut context_b),
        )
    };
    assert_ne!(alias_a, alias_b);
    let group_a = bindings
        .get(&TextureBinding::Managed(id_a), ImguiSampler::Linear)
        .unwrap();
    assert_eq!(
        bindings.get(&TextureBinding::Legacy(alias_a), ImguiSampler::Linear),
        Some(group_a)
    );

    assert_eq!(
        bindings.take_managed_renderer_state(context_a.id()).len(),
        1
    );
    assert!(
        bindings
            .get(&TextureBinding::Managed(id_a), ImguiSampler::Linear)
            .is_none()
    );
    assert!(
        bindings
            .get(&TextureBinding::Legacy(alias_a), ImguiSampler::Linear)
            .is_none()
    );
    let group_b = bindings
        .get(&TextureBinding::Managed(id_b), ImguiSampler::Nearest)
        .unwrap();
    assert_eq!(
        bindings.get(&TextureBinding::Legacy(alias_b), ImguiSampler::Nearest),
        Some(group_b)
    );

    bindings.destroy_managed_texture(id_b, 1);
    assert!(
        bindings
            .get(&TextureBinding::Legacy(alias_b), ImguiSampler::Nearest)
            .is_none()
    );
    assert!(bindings.is_empty());
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
        ..
    } = initialize_render_harness_resources();
    let pipeline = ImguiRenderPipeline::default();
    let mut extracted = ImguiExtractedBevyTextures::default();
    let mut gpu_images = RenderAssets::<GpuImage>::default();
    let mut texture_bind_groups = ImguiTextureBindGroups::default();
    let texture_id = imgui::TextureId::new(42);
    let image_id = AssetId::<Image>::default();
    let binding = TextureBinding::Legacy(texture_id);

    extracted.replace_for_test(vec![(texture_id, image_id)]);
    let mut current_image = gpu_image(&render_device, TextureUsages::TEXTURE_BINDING);
    gpu_images.insert(image_id, current_image.clone());

    prepare_bevy_image_texture_bind_groups(
        Some(&gpu_images),
        &extracted,
        &render_device,
        &pipeline_cache,
        &pipeline,
        &mut texture_bind_groups,
    );

    assert_eq!(texture_bind_groups.len(), 1);
    let first_bind_group = texture_bind_groups
        .get(&binding, ImguiSampler::Linear)
        .expect("registered Bevy image handles should resolve to a real bind group")
        .clone();

    prepare_bevy_image_texture_bind_groups(
        Some(&gpu_images),
        &extracted,
        &render_device,
        &pipeline_cache,
        &pipeline,
        &mut texture_bind_groups,
    );
    let reused_bind_group = texture_bind_groups
        .get(&binding, ImguiSampler::Linear)
        .expect("an unchanged image should retain its bind group");
    assert_eq!(
        reused_bind_group, &first_bind_group,
        "an unchanged texture view and sampler must reuse the existing bind group"
    );

    let replacement_view = gpu_image(&render_device, TextureUsages::TEXTURE_BINDING);
    current_image.texture = replacement_view.texture;
    current_image.texture_view = replacement_view.texture_view;
    current_image.texture_descriptor = replacement_view.texture_descriptor;
    current_image.texture_view_descriptor = replacement_view.texture_view_descriptor;
    gpu_images.insert(image_id, current_image.clone());
    prepare_bevy_image_texture_bind_groups(
        Some(&gpu_images),
        &extracted,
        &render_device,
        &pipeline_cache,
        &pipeline,
        &mut texture_bind_groups,
    );
    let view_bind_group = texture_bind_groups
        .get(&binding, ImguiSampler::Linear)
        .expect("a replacement texture view should produce a bind group")
        .clone();
    assert_ne!(
        view_bind_group, first_bind_group,
        "changing the texture view must rebuild the bind group"
    );

    current_image.sampler = render_device.create_sampler(&SamplerDescriptor {
        label: Some("dear_imgui_bevy_replacement_sampler"),
        ..Default::default()
    });
    gpu_images.insert(image_id, current_image.clone());
    prepare_bevy_image_texture_bind_groups(
        Some(&gpu_images),
        &extracted,
        &render_device,
        &pipeline_cache,
        &pipeline,
        &mut texture_bind_groups,
    );
    assert_ne!(
        texture_bind_groups
            .get(&binding, ImguiSampler::Linear)
            .expect("a replacement sampler should produce a bind group"),
        &view_bind_group,
        "changing only the sampler must rebuild the bind group"
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
    extracted.replace_for_test(vec![(texture_id, image_id)]);
    prepare_bevy_image_texture_bind_groups(
        Some(&gpu_images),
        &extracted,
        &render_device,
        &pipeline_cache,
        &pipeline,
        &mut texture_bind_groups,
    );
    assert_eq!(texture_bind_groups.len(), 1);

    extracted.replace_for_test(Vec::new());
    prepare_bevy_image_texture_bind_groups(
        None,
        &extracted,
        &render_device,
        &pipeline_cache,
        &pipeline,
        &mut texture_bind_groups,
    );
    assert!(
        texture_bind_groups.is_empty(),
        "unregistered Bevy images must be retired even without RenderAssets<GpuImage>"
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
        ..
    } = initialize_render_harness_resources();
    let pipeline = ImguiRenderPipeline::default();
    let mut extracted = ImguiExtractedBevyTextures::default();
    let mut gpu_images = RenderAssets::<GpuImage>::default();
    let mut texture_bind_groups = ImguiTextureBindGroups::default();
    let texture_id = imgui::TextureId::new(99);
    let image_id = AssetId::<Image>::default();
    let binding = TextureBinding::Legacy(texture_id);

    extracted.replace_for_test(vec![(texture_id, image_id)]);
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

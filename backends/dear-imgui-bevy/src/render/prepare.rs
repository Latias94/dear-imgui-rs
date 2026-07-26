//! CPU draw preparation, GPU uploads, and texture reconciliation.

use super::resources::{
    ImguiRenderTexture, ImguiTextureUpload, ImguiTextureViewCompatibility, PreparedFrameData,
};
use super::*;

pub(super) fn prepare_imgui_render_frame(
    extracted: Res<ImguiExtractedRenderFrame>,
    mut prepared: ResMut<ImguiPreparedRenderFrame>,
) {
    let Some(snapshot) = extracted.snapshot() else {
        prepared.clear(extracted.frame_index());
        return;
    };

    let Some(frame_index) = extracted.frame_index() else {
        prepared.clear(None);
        return;
    };

    let draw_data = snapshot.draw_data();
    let primary_uniforms = valid_display_rect(draw_data)
        .map(|_| ImguiUniforms::from_display_rect(draw_data.display_pos, draw_data.display_size));
    let (vertices, indices, draws, uniforms_by_view) =
        prepare_snapshot_draw_data(snapshot, extracted.camera_targets());
    prepared.replace(PreparedFrameData {
        frame_index,
        uniforms: primary_uniforms,
        uniforms_by_view,
        vertices,
        indices,
        draws,
        texture_request_count: snapshot.texture_requests().len(),
    });
}

pub(super) fn upload_imgui_buffers(
    prepared: Res<ImguiPreparedRenderFrame>,
    mut gpu_buffers: ResMut<ImguiGpuBuffers>,
    render_device: Option<Res<RenderDevice>>,
    render_queue: Option<Res<RenderQueue>>,
) {
    if let (Some(render_device), Some(render_queue)) = (render_device, render_queue) {
        gpu_buffers.upload(&prepared, &render_device, &render_queue);
    }
}

pub(super) fn prepare_imgui_uniform_bind_groups(
    prepared: Res<ImguiPreparedRenderFrame>,
    render_device: Option<Res<RenderDevice>>,
    render_queue: Option<Res<RenderQueue>>,
    pipeline_cache: Option<Res<PipelineCache>>,
    pipeline: Res<ImguiRenderPipeline>,
    mut gpu_resources: Option<ResMut<ImguiPipelineGpuResources>>,
) {
    let (Some(render_device), Some(render_queue), Some(pipeline_cache), Some(gpu_resources)) = (
        render_device,
        render_queue,
        pipeline_cache,
        gpu_resources.as_deref_mut(),
    ) else {
        return;
    };

    gpu_resources.prepare_camera_uniforms(
        &prepared,
        &render_device,
        &render_queue,
        &pipeline_cache,
        &pipeline,
    );
}

#[derive(SystemParam)]
pub(super) struct ImguiTextureBindGroupParams<'w> {
    extracted: ResMut<'w, ImguiExtractedRenderFrame>,
    extracted_bevy_textures: Res<'w, ImguiExtractedBevyTextures>,
    gpu_images: Option<Res<'w, RenderAssets<GpuImage>>>,
    render_device: Option<Res<'w, RenderDevice>>,
    render_queue: Option<Res<'w, RenderQueue>>,
    pipeline_cache: Option<Res<'w, PipelineCache>>,
    pipeline: Res<'w, ImguiRenderPipeline>,
    renderer_release: Res<'w, ImguiRendererRelease>,
}

pub(super) fn prepare_imgui_texture_bind_groups(
    mut params: ImguiTextureBindGroupParams,
    mut texture_bind_groups: ResMut<ImguiTextureBindGroups>,
) {
    if params.renderer_release.release_requested() {
        return;
    }
    let (Some(render_device), Some(render_queue), Some(pipeline_cache)) = (
        params.render_device,
        params.render_queue,
        params.pipeline_cache,
    ) else {
        params
            .renderer_release
            .update_resources_live(texture_bind_groups.has_managed_resources());
        return;
    };

    let Some(snapshot) = params.extracted.snapshot() else {
        prepare_bevy_image_texture_bind_groups(
            params.gpu_images.as_deref(),
            &params.extracted_bevy_textures,
            &render_device,
            &pipeline_cache,
            &params.pipeline,
            &mut texture_bind_groups,
        );
        params
            .renderer_release
            .update_resources_live(texture_bind_groups.has_managed_resources());
        return;
    };

    let mut texture_feedback = Vec::new();
    for request in snapshot.texture_requests() {
        let snapshot_texture = request.texture();
        if !matches!(request.operation(), imgui::render::TextureOp::Destroy)
            && !texture_bind_groups.accepts_managed_texture_upload(snapshot_texture)
        {
            continue;
        }
        match request.operation() {
            imgui::render::TextureOp::Create {
                format,
                width,
                height,
                row_pitch,
                pixels,
            } => {
                if !validate_managed_texture_extent(&render_device, *width, *height) {
                    continue;
                }
                if let Some(render_texture) = create_imgui_render_texture(
                    &render_device,
                    &render_queue,
                    &pipeline_cache,
                    &params.pipeline,
                    ImguiTextureUpload {
                        format: *format,
                        width: *width,
                        height: *height,
                        row_pitch: *row_pitch,
                        pixels,
                    },
                ) {
                    let tex_id = texture_bind_groups.managed_texture_id(snapshot_texture);
                    texture_bind_groups.insert_render_texture(
                        TextureBinding::Managed(snapshot_texture),
                        render_texture,
                    );
                    params.renderer_release.update_resources_live(true);
                    if let Ok(feedback) = request.uploaded(tex_id) {
                        texture_feedback.push(feedback);
                    }
                }
            }
            imgui::render::TextureOp::Update {
                format,
                width,
                height,
                rects,
            } => {
                if !validate_managed_texture_extent(&render_device, *width, *height) {
                    continue;
                }
                if let Some(render_texture) = texture_bind_groups
                    .textures
                    .get(&TextureBinding::Managed(snapshot_texture))
                {
                    let Some(texture_extent) = render_texture.extent else {
                        continue;
                    };
                    if texture_extent != [*width, *height] {
                        continue;
                    }
                    let Some(texture) = render_texture.texture.as_ref() else {
                        continue;
                    };
                    let Some(updates) =
                        convert_imgui_texture_update_rects(*format, *width, *height, rects)
                    else {
                        continue;
                    };
                    for update in updates {
                        write_texture_rows(
                            &render_queue,
                            texture,
                            update.origin,
                            update.width,
                            update.height,
                            update.row_pitch,
                            &update.pixels,
                        );
                    }
                    if let Some(texture_id) = texture_bind_groups
                        .managed_texture_ids
                        .get(&snapshot_texture)
                        .copied()
                        && let Ok(feedback) = request.uploaded(texture_id)
                    {
                        texture_feedback.push(feedback);
                    }
                }
            }
            imgui::render::TextureOp::Destroy => {
                texture_bind_groups
                    .destroy_managed_texture(snapshot_texture, snapshot.epoch().sequence());
                if let Ok(feedback) = request.destroyed() {
                    texture_feedback.push(feedback);
                }
            }
        }
    }

    prepare_bevy_image_texture_bind_groups(
        params.gpu_images.as_deref(),
        &params.extracted_bevy_textures,
        &render_device,
        &pipeline_cache,
        &params.pipeline,
        &mut texture_bind_groups,
    );
    params.extracted.extend_texture_feedback(texture_feedback);
    params
        .renderer_release
        .update_resources_live(texture_bind_groups.has_managed_resources());
}

pub(super) fn release_imgui_renderer_resources(
    renderer_release: Res<ImguiRendererRelease>,
    snapshot_mailbox: Res<crate::context::ImguiFrameMailbox>,
    mut extracted: ResMut<ImguiExtractedRenderFrame>,
    mut prepared: ResMut<ImguiPreparedRenderFrame>,
    mut texture_bind_groups: ResMut<ImguiTextureBindGroups>,
) {
    let Some(generation) = renderer_release.requested_generation() else {
        return;
    };

    snapshot_mailbox.clear();
    let route_epoch = extracted.route_epoch();
    extracted.clear(0, route_epoch);
    prepared.clear(None);
    let released_gpu_resources = texture_bind_groups.take_managed_renderer_state();
    drop(released_gpu_resources);
    assert!(
        renderer_release.acknowledge_release(generation),
        "Bevy renderer release acknowledgement generation changed during cleanup"
    );
}

pub(super) fn commit_imgui_render_frame(
    mut extracted: ResMut<ImguiExtractedRenderFrame>,
    mut texture_bind_groups: ResMut<ImguiTextureBindGroups>,
) {
    extracted.commit();
    texture_bind_groups.prune_destroyed_managed_textures(extracted.completion_watermark());
}

fn validate_managed_texture_extent(render_device: &RenderDevice, width: u32, height: u32) -> bool {
    managed_texture_extent_supported(
        width,
        height,
        render_device.limits().max_texture_dimension_2d,
    )
}

pub(super) fn managed_texture_extent_supported(
    width: u32,
    height: u32,
    max_dimension_2d: u32,
) -> bool {
    width > 0 && height > 0 && width <= max_dimension_2d && height <= max_dimension_2d
}

pub(super) fn validate_texture_update_rect(
    texture_width: u32,
    texture_height: u32,
    rect: imgui::TextureRect,
) -> bool {
    let x = u32::from(rect.x);
    let y = u32::from(rect.y);
    let w = u32::from(rect.w);
    let h = u32::from(rect.h);
    w > 0
        && h > 0
        && x.checked_add(w).is_some_and(|right| right <= texture_width)
        && y.checked_add(h)
            .is_some_and(|bottom| bottom <= texture_height)
}

fn create_imgui_render_texture(
    render_device: &RenderDevice,
    render_queue: &RenderQueue,
    pipeline_cache: &PipelineCache,
    pipeline: &ImguiRenderPipeline,
    upload: ImguiTextureUpload<'_>,
) -> Option<ImguiRenderTexture> {
    let (pixels, row_pitch) = convert_imgui_texture_pixels(
        upload.format,
        upload.width,
        upload.height,
        upload.row_pitch,
        upload.pixels,
    )?;
    let texture = render_device.create_texture(&TextureDescriptor {
        label: Some("dear_imgui_bevy_texture"),
        size: Extent3d {
            width: upload.width,
            height: upload.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    write_texture_rows(
        render_queue,
        &texture,
        Origin3d::ZERO,
        upload.width,
        upload.height,
        row_pitch,
        &pixels,
    );
    let view = texture.create_view(&TextureViewDescriptor::default());
    let layout = pipeline_cache.get_bind_group_layout(pipeline.texture_layout());
    let linear_sampler = create_standard_imgui_sampler(render_device, ImguiSampler::Linear);
    let nearest_sampler = create_standard_imgui_sampler(render_device, ImguiSampler::Nearest);
    let linear_bind_group = create_texture_sampler_bind_group(
        render_device,
        &layout,
        Some("dear_imgui_bevy_texture_bind_group"),
        &view,
        &linear_sampler,
    );
    let nearest_bind_group = create_texture_sampler_bind_group(
        render_device,
        &layout,
        Some("dear_imgui_bevy_texture_bind_group_nearest"),
        &view,
        &nearest_sampler,
    );
    Some(ImguiRenderTexture {
        texture: Some(texture),
        _view: Some(view),
        extent: Some([upload.width, upload.height]),
        linear_bind_group,
        nearest_bind_group,
    })
}

pub(super) fn create_standard_imgui_sampler(
    render_device: &RenderDevice,
    sampler: ImguiSampler,
) -> Sampler {
    match sampler {
        ImguiSampler::Linear => render_device.create_sampler(&SamplerDescriptor {
            label: Some("dear_imgui_bevy_sampler"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: MipmapFilterMode::Linear,
            ..Default::default()
        }),
        ImguiSampler::Nearest => render_device.create_sampler(&SamplerDescriptor {
            label: Some("dear_imgui_bevy_nearest_sampler"),
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: MipmapFilterMode::Nearest,
            ..Default::default()
        }),
    }
}

pub(super) fn create_texture_sampler_bind_group(
    render_device: &RenderDevice,
    layout: &BindGroupLayout,
    label: Option<&'static str>,
    view: &TextureView,
    sampler: &Sampler,
) -> BindGroup {
    render_device.create_bind_group(
        label,
        layout,
        &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(view),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Sampler(sampler),
            },
        ],
    )
}

pub(super) struct ConvertedTextureUpdateRect {
    pub(super) origin: Origin3d,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) row_pitch: u32,
    pub(super) pixels: Vec<u8>,
}

pub(super) fn convert_imgui_texture_update_rects(
    format: imgui::texture::TextureFormat,
    texture_width: u32,
    texture_height: u32,
    rects: &[imgui::render::snapshot::TextureUploadRect],
) -> Option<Vec<ConvertedTextureUpdateRect>> {
    if rects.is_empty() {
        return None;
    }

    let mut updates = Vec::with_capacity(rects.len());
    for rect in rects {
        if !validate_texture_update_rect(texture_width, texture_height, rect.rect) {
            return None;
        }
        let width = u32::from(rect.rect.w);
        let height = u32::from(rect.rect.h);
        let (pixels, row_pitch) =
            convert_imgui_texture_pixels(format, width, height, rect.row_pitch, &rect.data)?;
        updates.push(ConvertedTextureUpdateRect {
            origin: Origin3d {
                x: u32::from(rect.rect.x),
                y: u32::from(rect.rect.y),
                z: 0,
            },
            width,
            height,
            row_pitch,
            pixels,
        });
    }
    Some(updates)
}

pub(super) fn prepare_bevy_image_texture_bind_groups(
    gpu_images: Option<&RenderAssets<GpuImage>>,
    extracted_bevy_textures: &ImguiExtractedBevyTextures,
    render_device: &RenderDevice,
    pipeline_cache: &PipelineCache,
    pipeline: &ImguiRenderPipeline,
    texture_bind_groups: &mut ImguiTextureBindGroups,
) {
    retain_extracted_bevy_image_bindings(extracted_bevy_textures, texture_bind_groups);

    let Some(gpu_images) = gpu_images else {
        return;
    };

    for (texture_id, asset_id) in extracted_bevy_textures.textures() {
        let binding = TextureBinding::Legacy(*texture_id);
        let Some(gpu_image) = gpu_images.get(*asset_id) else {
            texture_bind_groups.remove_binding(&binding);
            continue;
        };
        let Some(bind_group) = create_bevy_image_texture_bind_group(
            render_device,
            pipeline_cache,
            pipeline,
            gpu_image,
        ) else {
            texture_bind_groups.remove_binding(&binding);
            continue;
        };
        texture_bind_groups.insert_bevy_image(binding, bind_group);
    }
}

pub(super) fn retain_extracted_bevy_image_bindings(
    extracted_bevy_textures: &ImguiExtractedBevyTextures,
    texture_bind_groups: &mut ImguiTextureBindGroups,
) {
    let active_bindings = extracted_bevy_textures
        .textures()
        .iter()
        .map(|(texture_id, _)| TextureBinding::Legacy(*texture_id))
        .collect::<HashSet<_>>();
    texture_bind_groups.retain_bevy_image_bindings(&active_bindings);
}

fn create_bevy_image_texture_bind_group(
    render_device: &RenderDevice,
    pipeline_cache: &PipelineCache,
    pipeline: &ImguiRenderPipeline,
    gpu_image: &GpuImage,
) -> Option<BindGroup> {
    if !ImguiTextureViewCompatibility::from_gpu_image(gpu_image)
        .supports_imgui_sampling(render_device.features())
    {
        return None;
    }

    let layout = pipeline_cache.get_bind_group_layout(pipeline.texture_layout());
    Some(create_texture_sampler_bind_group(
        render_device,
        &layout,
        Some("dear_imgui_bevy_image_texture_bind_group"),
        &gpu_image.texture_view,
        &gpu_image.sampler,
    ))
}

pub(super) fn convert_imgui_texture_pixels(
    format: imgui::texture::TextureFormat,
    width: u32,
    height: u32,
    row_pitch: usize,
    pixels: &[u8],
) -> Option<(Vec<u8>, u32)> {
    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    if width == 0 || height == 0 {
        return None;
    }

    match format {
        imgui::texture::TextureFormat::RGBA32 => {
            let dst_row_pitch = width.checked_mul(4)?;
            copy_or_repack_rows(pixels, row_pitch, dst_row_pitch, height)
                .map(|pixels| (pixels, dst_row_pitch as u32))
        }
        imgui::texture::TextureFormat::Alpha8 => {
            let mut rgba = vec![0; width.checked_mul(height)?.checked_mul(4)?];
            for row in 0..height {
                let src_start = row.checked_mul(row_pitch)?;
                let src_end = src_start.checked_add(width)?;
                if src_end > pixels.len() {
                    return None;
                }
                for (col, alpha) in pixels[src_start..src_end].iter().copied().enumerate() {
                    let dst = row.checked_mul(width)?.checked_add(col)?.checked_mul(4)?;
                    rgba[dst..dst + 4].copy_from_slice(&[255, 255, 255, alpha]);
                }
            }
            Some((rgba, width.checked_mul(4)? as u32))
        }
    }
}

fn copy_or_repack_rows(
    pixels: &[u8],
    src_row_pitch: usize,
    dst_row_pitch: usize,
    rows: usize,
) -> Option<Vec<u8>> {
    if src_row_pitch < dst_row_pitch {
        return None;
    }
    let required_src = src_row_pitch.checked_mul(rows)?;
    if pixels.len() < required_src {
        return None;
    }
    if src_row_pitch == dst_row_pitch {
        return Some(pixels[..required_src].to_vec());
    }

    let mut out = vec![0; dst_row_pitch.checked_mul(rows)?];
    for row in 0..rows {
        let src = row.checked_mul(src_row_pitch)?;
        let dst = row.checked_mul(dst_row_pitch)?;
        out[dst..dst + dst_row_pitch].copy_from_slice(&pixels[src..src + dst_row_pitch]);
    }
    Some(out)
}

pub(super) fn write_texture_rows(
    render_queue: &RenderQueue,
    texture: &Texture,
    origin: Origin3d,
    width: u32,
    height: u32,
    row_pitch: u32,
    pixels: &[u8],
) {
    if width == 0 || height == 0 || row_pitch == 0 {
        return;
    }

    let alignment = COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_row_pitch = row_pitch.div_ceil(alignment) * alignment;
    if padded_row_pitch == row_pitch {
        render_queue.write_texture(
            TexelCopyTextureInfo {
                texture: &**texture,
                mip_level: 0,
                origin,
                aspect: TextureAspect::All,
            },
            pixels,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_pitch),
                rows_per_image: Some(height),
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        return;
    }

    let row_pitch = usize::try_from(row_pitch).ok();
    let padded_row_pitch = usize::try_from(padded_row_pitch).ok();
    let height_usize = usize::try_from(height).ok();
    let (Some(row_pitch), Some(padded_row_pitch), Some(height_usize)) =
        (row_pitch, padded_row_pitch, height_usize)
    else {
        return;
    };
    let Some(required) = row_pitch.checked_mul(height_usize) else {
        return;
    };
    if pixels.len() < required {
        return;
    }
    let Some(padded_len) = padded_row_pitch.checked_mul(height_usize) else {
        return;
    };
    let mut padded = vec![0; padded_len];
    for row in 0..height_usize {
        let src = row * row_pitch;
        let dst = row * padded_row_pitch;
        padded[dst..dst + row_pitch].copy_from_slice(&pixels[src..src + row_pitch]);
    }
    render_queue.write_texture(
        TexelCopyTextureInfo {
            texture: &**texture,
            mip_level: 0,
            origin,
            aspect: TextureAspect::All,
        },
        &padded,
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(padded_row_pitch as u32),
            rows_per_image: Some(height),
        },
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
}

fn prepare_snapshot_draw_data(
    snapshot: &imgui::render::FrameSnapshot,
    camera_targets: &[ImguiCameraTarget],
) -> (
    Vec<ImguiGpuVertex>,
    Vec<DrawIdx>,
    Vec<ImguiPreparedDraw>,
    HashMap<RetainedViewEntity, ImguiUniforms>,
) {
    prepare_draw_data(snapshot.draw_data(), snapshot.viewports(), camera_targets)
}

pub(super) fn prepare_draw_data(
    main_draw: &imgui::render::DrawDataSnapshot,
    viewports: &[imgui::render::ViewportDrawDataSnapshot],
    camera_targets: &[ImguiCameraTarget],
) -> (
    Vec<ImguiGpuVertex>,
    Vec<DrawIdx>,
    Vec<ImguiPreparedDraw>,
    HashMap<RetainedViewEntity, ImguiUniforms>,
) {
    let viewport_draws = snapshot_viewport_draws(main_draw, viewports);
    let vertex_count = viewport_draws
        .iter()
        .flat_map(|(_, draw)| &draw.draw_lists)
        .map(|list| list.vtx.len())
        .sum();
    let index_count = viewport_draws
        .iter()
        .flat_map(|(_, draw)| &draw.draw_lists)
        .map(|list| list.idx.len())
        .sum();
    let mut vertices = Vec::with_capacity(vertex_count);
    let mut indices = Vec::with_capacity(index_count);
    let mut draws = Vec::new();
    let mut uniforms_by_view = HashMap::new();

    let mut list_vertex_base = 0usize;
    let mut list_index_base = 0usize;

    for (viewport_id, draw) in viewport_draws {
        let target_cameras = camera_targets
            .iter()
            .filter(|target| target.viewport_id == viewport_id)
            .collect::<Vec<_>>();
        if target_cameras.is_empty() {
            continue;
        }
        if valid_display_rect(draw).is_none() {
            continue;
        }

        let target_cameras = target_cameras
            .into_iter()
            .filter_map(|target| {
                let uniforms = uniforms_for_target_draw(draw, target)?;
                Some((target, uniforms))
            })
            .collect::<Vec<_>>();
        if target_cameras.is_empty() {
            continue;
        }
        for (target, uniforms) in &target_cameras {
            uniforms_by_view.insert(target.view, *uniforms);
        }

        let mut active_sampler = ImguiSampler::Linear;
        for list in &draw.draw_lists {
            vertices.extend(list.vtx.iter().copied().map(ImguiGpuVertex::from));
            indices.extend(list.idx.iter().copied());

            for command in &list.commands {
                let (count, clip_rect, texture, vtx_offset, idx_offset) = match command {
                    DrawCmdSnapshot::Elements {
                        count,
                        clip_rect,
                        texture,
                        vtx_offset,
                        idx_offset,
                    } => (*count, *clip_rect, *texture, *vtx_offset, *idx_offset),
                    DrawCmdSnapshot::ResetRenderState | DrawCmdSnapshot::SetSamplerLinear => {
                        active_sampler = ImguiSampler::Linear;
                        continue;
                    }
                    DrawCmdSnapshot::SetSamplerNearest => {
                        active_sampler = ImguiSampler::Nearest;
                        continue;
                    }
                };

                let Some(scissor) = scissor_from_clip_rect(draw, clip_rect) else {
                    continue;
                };
                let Some(framebuffer_size) = framebuffer_size_for_draw(draw) else {
                    continue;
                };
                let Some(index_start) = list_index_base.checked_add(idx_offset) else {
                    continue;
                };
                let Some(index_end) = index_start.checked_add(count) else {
                    continue;
                };
                let Some(vertex_offset) = list_vertex_base.checked_add(vtx_offset) else {
                    continue;
                };
                if index_end > list_index_base + list.idx.len()
                    || vertex_offset > list_vertex_base + list.vtx.len()
                {
                    continue;
                }
                let local_index_end = index_end - list_index_base;
                if draw_indices_reference_out_of_bounds(
                    &list.idx[idx_offset..local_index_end],
                    vertex_offset,
                    vertices.len(),
                ) {
                    continue;
                }
                let Ok(index_start) = u32::try_from(index_start) else {
                    continue;
                };
                let Ok(index_end) = u32::try_from(index_end) else {
                    continue;
                };
                let Ok(vertex_offset) = i32::try_from(vertex_offset) else {
                    continue;
                };

                for (target, _) in &target_cameras {
                    draws.push(ImguiPreparedDraw {
                        context_id: target.context_id,
                        route_epoch: target.route_epoch,
                        camera: target.camera,
                        view: target.view,
                        order: target.order,
                        camera_order: target.camera_order,
                        camera_schedule: target.camera_schedule,
                        target: target.target.clone(),
                        target_format: target.target_format,
                        texture_usages: target.texture_usages,
                        msaa: target.msaa,
                        physical_target_size: target.physical_target_size,
                        viewport_id,
                        texture,
                        sampler: active_sampler,
                        scissor,
                        framebuffer_size,
                        camera_viewport: target.camera_viewport,
                        index_range: index_start..index_end,
                        vertex_offset,
                    });
                }
            }

            list_vertex_base += list.vtx.len();
            list_index_base += list.idx.len();
        }
    }

    (vertices, indices, draws, uniforms_by_view)
}

fn snapshot_viewport_draws<'draw>(
    main_draw: &'draw imgui::render::DrawDataSnapshot,
    viewports: &'draw [imgui::render::ViewportDrawDataSnapshot],
) -> Vec<(Option<imgui::Id>, &'draw imgui::render::DrawDataSnapshot)> {
    if viewports.is_empty() {
        return vec![(None, main_draw)];
    }

    let mut draws = viewports
        .iter()
        .map(|viewport| (Some(viewport.viewport_id), &viewport.draw))
        .collect::<Vec<_>>();
    if !draws.iter().any(|(viewport_id, _)| viewport_id.is_none()) {
        draws.insert(0, (None, main_draw));
    }
    draws
}

pub(super) fn scissor_from_clip_rect(
    draw: &imgui::render::DrawDataSnapshot,
    clip_rect: [f32; 4],
) -> Option<ImguiScissorRect> {
    let valid_rect = valid_display_rect(draw)?;
    if !clip_rect.iter().all(|value| value.is_finite()) {
        return None;
    }
    if clip_rect[2] <= clip_rect[0] || clip_rect[3] <= clip_rect[1] {
        return None;
    }

    let scale = draw.framebuffer_scale;
    let min_x = ((clip_rect[0] - draw.display_pos[0]) * scale[0]).floor();
    let min_y = ((clip_rect[1] - draw.display_pos[1]) * scale[1]).floor();
    let max_x = ((clip_rect[2] - draw.display_pos[0]) * scale[0]).ceil();
    let max_y = ((clip_rect[3] - draw.display_pos[1]) * scale[1]).ceil();

    let framebuffer_width = valid_rect.framebuffer_width;
    let framebuffer_height = valid_rect.framebuffer_height;

    let min_x = min_x.clamp(0.0, framebuffer_width);
    let min_y = min_y.clamp(0.0, framebuffer_height);
    let max_x = max_x.clamp(min_x, framebuffer_width);
    let max_y = max_y.clamp(min_y, framebuffer_height);

    let width = max_x - min_x;
    let height = max_y - min_y;
    if width <= 0.0 || height <= 0.0 {
        return None;
    }

    Some(ImguiScissorRect {
        x: min_x as u32,
        y: min_y as u32,
        width: width as u32,
        height: height as u32,
    })
}

fn framebuffer_size_for_draw(draw: &imgui::render::DrawDataSnapshot) -> Option<[u32; 2]> {
    let valid_rect = valid_display_rect(draw)?;
    Some([
        valid_rect.framebuffer_width as u32,
        valid_rect.framebuffer_height as u32,
    ])
}

fn uniforms_for_target_draw(
    draw: &imgui::render::DrawDataSnapshot,
    target: &ImguiCameraTarget,
) -> Option<ImguiUniforms> {
    if let Some(viewport) = target.camera_viewport {
        let [viewport_width, viewport_height] = viewport.physical_size;
        if viewport_width == 0 || viewport_height == 0 {
            return None;
        }

        let [scale_x, scale_y] = draw.framebuffer_scale;
        if scale_x <= 0.0 || scale_y <= 0.0 || !scale_x.is_finite() || !scale_y.is_finite() {
            return None;
        }

        let display_pos = [
            draw.display_pos[0] + viewport.physical_position[0] as f32 / scale_x,
            draw.display_pos[1] + viewport.physical_position[1] as f32 / scale_y,
        ];
        let display_size = [
            viewport.physical_size[0] as f32 / scale_x,
            viewport.physical_size[1] as f32 / scale_y,
        ];
        if ![
            display_pos[0],
            display_pos[1],
            display_size[0],
            display_size[1],
        ]
        .iter()
        .all(|value| value.is_finite())
        {
            return None;
        }
        return Some(ImguiUniforms::from_display_rect(display_pos, display_size));
    }

    Some(ImguiUniforms::from_display_rect(
        draw.display_pos,
        draw.display_size,
    ))
}

fn render_viewport_for_draw(draw: &ImguiPreparedDraw) -> Option<ImguiCameraViewport> {
    let camera_viewport = draw.camera_viewport?;
    let [width, height] = camera_viewport.physical_size;
    if width == 0 || height == 0 {
        return None;
    }
    Some(camera_viewport)
}

pub(super) fn render_viewport_for_pass(
    drawable: &[&ImguiPreparedDraw],
    render_target_size: Option<[u32; 2]>,
) -> Option<ImguiCameraViewport> {
    let viewport = drawable
        .iter()
        .find_map(|draw| render_viewport_for_draw(draw))?;
    let Some(render_target_size) = render_target_size else {
        return Some(viewport);
    };
    let viewport_rect = intersect_scissor_with_rect(
        ImguiScissorRect {
            x: viewport.physical_position[0],
            y: viewport.physical_position[1],
            width: viewport.physical_size[0],
            height: viewport.physical_size[1],
        },
        [0, 0],
        render_target_size,
    )?;
    Some(ImguiCameraViewport {
        physical_position: [viewport_rect.x, viewport_rect.y],
        physical_size: [viewport_rect.width, viewport_rect.height],
    })
}

pub(super) fn scissor_for_render_pass(
    draw: &ImguiPreparedDraw,
    render_target_size: Option<[u32; 2]>,
) -> Option<ImguiScissorRect> {
    let viewport = render_viewport_for_draw(draw);
    let scissor = match viewport {
        Some(viewport) => intersect_scissor_with_camera_viewport(draw.scissor, viewport)?,
        None => draw.scissor,
    };
    match render_target_size {
        Some(size) => intersect_scissor_with_rect(scissor, [0, 0], size),
        None => Some(scissor),
    }
}

pub(super) fn intersect_scissor_with_camera_viewport(
    scissor: ImguiScissorRect,
    viewport: ImguiCameraViewport,
) -> Option<ImguiScissorRect> {
    intersect_scissor_with_rect(scissor, viewport.physical_position, viewport.physical_size)
}

fn intersect_scissor_with_rect(
    scissor: ImguiScissorRect,
    rect_position: [u32; 2],
    rect_size: [u32; 2],
) -> Option<ImguiScissorRect> {
    let [rect_width, rect_height] = rect_size;
    if rect_width == 0 || rect_height == 0 {
        return None;
    }
    let scissor_min_x = u64::from(scissor.x);
    let scissor_min_y = u64::from(scissor.y);
    let scissor_max_x = scissor_min_x.checked_add(u64::from(scissor.width))?;
    let scissor_max_y = scissor_min_y.checked_add(u64::from(scissor.height))?;
    let viewport_min_x = u64::from(rect_position[0]);
    let viewport_min_y = u64::from(rect_position[1]);
    let viewport_max_x = viewport_min_x.checked_add(u64::from(rect_width))?;
    let viewport_max_y = viewport_min_y.checked_add(u64::from(rect_height))?;

    let min_x = scissor_min_x.max(viewport_min_x);
    let min_y = scissor_min_y.max(viewport_min_y);
    let max_x = scissor_max_x.min(viewport_max_x);
    let max_y = scissor_max_y.min(viewport_max_y);
    if max_x <= min_x || max_y <= min_y {
        return None;
    }

    Some(ImguiScissorRect {
        x: u32::try_from(min_x).ok()?,
        y: u32::try_from(min_y).ok()?,
        width: u32::try_from(max_x - min_x).ok()?,
        height: u32::try_from(max_y - min_y).ok()?,
    })
}

pub(super) fn draw_indices_reference_out_of_bounds(
    indices: &[DrawIdx],
    vertex_offset: usize,
    vertex_count: usize,
) -> bool {
    indices.iter().copied().max().is_some_and(|max_index| {
        usize::from(max_index)
            .checked_add(vertex_offset)
            .is_none_or(|absolute_index| absolute_index >= vertex_count)
    })
}

#[derive(Clone, Copy)]
struct ValidDisplayRect {
    framebuffer_width: f32,
    framebuffer_height: f32,
}

fn valid_display_rect(draw: &imgui::render::DrawDataSnapshot) -> Option<ValidDisplayRect> {
    let [display_x, display_y] = draw.display_pos;
    let [display_width, display_height] = draw.display_size;
    let [scale_x, scale_y] = draw.framebuffer_scale;
    if ![
        display_x,
        display_y,
        display_width,
        display_height,
        scale_x,
        scale_y,
    ]
    .iter()
    .all(|value| value.is_finite())
    {
        return None;
    }
    if display_width <= 0.0 || display_height <= 0.0 || scale_x <= 0.0 || scale_y <= 0.0 {
        return None;
    }

    let framebuffer_width = (display_width * scale_x).ceil();
    let framebuffer_height = (display_height * scale_y).ceil();
    if !framebuffer_width.is_finite()
        || !framebuffer_height.is_finite()
        || framebuffer_width <= 0.0
        || framebuffer_height <= 0.0
        || framebuffer_width > u32::MAX as f32
        || framebuffer_height > u32::MAX as f32
    {
        return None;
    }

    Some(ValidDisplayRect {
        framebuffer_width,
        framebuffer_height,
    })
}

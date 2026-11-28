//! CPU draw preparation and render buffer uploads.

use dear_imgui_backend_support::compute_scissor_rect;

#[cfg(test)]
use super::resources::PreparedContextMetadata;
use super::*;
use crate::route::{ImguiDiagnostic, ImguiDiagnosticKind, ImguiDiagnosticOrigin, ImguiDiagnostics};

pub(super) fn prepare_imgui_render_frame(
    mut extracted: ResMut<ImguiExtractedRenderFrame>,
    mut prepared: ResMut<ImguiPreparedRenderFrame>,
    diagnostics: Res<ImguiDiagnostics>,
) {
    let frame = prepared.begin_prepare();
    let mut preparation_diagnostics = Vec::new();

    for context_id in extracted.sorted_context_ids() {
        let Some(snapshot) = extracted.snapshot(context_id) else {
            continue;
        };
        #[cfg(test)]
        let metadata = PreparedContextMetadata {
            frame_index: extracted
                .frame_index(context_id)
                .expect("an extracted snapshot must retain its frame index"),
            texture_request_count: snapshot.texture_requests().len(),
        };
        let result = prepare_draw_data(
            snapshot.draw_data(),
            snapshot.viewports(),
            extracted.camera_targets(context_id),
        );
        let PreparedImguiDrawData {
            vertices: context_vertices,
            indices: context_indices,
            draws: mut context_draws,
            uniforms_by_view,
        } = match result {
            Ok(prepared) => prepared,
            Err(kind) => {
                preparation_diagnostics.push(ImguiDiagnostic::new(kind).with_context(context_id));
                // Abandon the whole Context epoch before texture upload or completion commit.
                extracted.remove_context(context_id);
                continue;
            }
        };
        #[cfg(test)]
        frame.contexts.insert(context_id, metadata);

        let Ok(index_base) = u32::try_from(frame.indices.len()) else {
            continue;
        };
        let Ok(vertex_base) = i32::try_from(frame.vertices.len()) else {
            continue;
        };
        context_draws.retain_mut(|draw| {
            let Some(index_start) = draw.index_range.start.checked_add(index_base) else {
                return false;
            };
            let Some(index_end) = draw.index_range.end.checked_add(index_base) else {
                return false;
            };
            let Some(vertex_offset) = draw.vertex_offset.checked_add(vertex_base) else {
                return false;
            };
            draw.index_range = index_start..index_end;
            draw.vertex_offset = vertex_offset;
            true
        });
        frame.uniforms_by_context_view.extend(
            uniforms_by_view
                .into_iter()
                .map(|(view, uniforms)| ((context_id, view), uniforms)),
        );
        frame.vertices.extend(context_vertices);
        frame.indices.extend(context_indices);
        frame.draws.extend(context_draws);
    }
    diagnostics.replace(
        ImguiDiagnosticOrigin::RenderPreparation,
        extracted.route_epoch(),
        preparation_diagnostics,
    );
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

pub(super) struct PreparedImguiDrawData {
    pub(super) vertices: Vec<ImguiGpuVertex>,
    pub(super) indices: Vec<DrawIdx>,
    pub(super) draws: Vec<ImguiPreparedDraw>,
    pub(super) uniforms_by_view: HashMap<RetainedViewEntity, ImguiUniforms>,
}

pub(super) fn prepare_draw_data(
    main_draw: &imgui::render::DrawDataSnapshot,
    viewports: &[imgui::render::ViewportDrawDataSnapshot],
    camera_targets: &[ImguiCameraTarget],
) -> Result<PreparedImguiDrawData, ImguiDiagnosticKind> {
    let viewport_draws = std::iter::once((None, main_draw)).chain(
        viewports
            .iter()
            .map(|viewport| (Some(viewport.viewport_id), &viewport.draw)),
    );
    // Validate sampler callbacks even in viewports with no eligible camera.
    let (vertex_count, index_count) = viewport_draws
        .clone()
        .flat_map(|(_, draw)| &draw.draw_lists)
        .try_fold((0, 0), |(vertices, indices), list| {
            if list
                .commands
                .iter()
                .any(|command| matches!(command, DrawCmdSnapshot::SetSamplerFromTexture))
            {
                return Err(ImguiDiagnosticKind::TextureSamplerUnsupported);
            }
            Ok((vertices + list.vtx.len(), indices + list.idx.len()))
        })?;
    let mut vertices = Vec::with_capacity(vertex_count);
    let mut indices = Vec::with_capacity(index_count);
    let mut draws = Vec::new();
    let mut uniforms_by_view = HashMap::new();

    let mut list_vertex_base = 0usize;
    let mut list_index_base = 0usize;

    for (viewport_id, draw) in viewport_draws {
        let Some(framebuffer_size) = framebuffer_size_for_draw(draw) else {
            continue;
        };

        let mut target_cameras = Vec::new();
        for target in camera_targets
            .iter()
            .filter(|target| target.viewport_id == viewport_id)
        {
            if let Some(uniforms) = uniforms_for_target_draw(draw, target) {
                uniforms_by_view.insert(target.view, uniforms);
                target_cameras.push(target);
            }
        }
        if target_cameras.is_empty() {
            continue;
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
                    DrawCmdSnapshot::SetSamplerFromTexture => {
                        return Err(ImguiDiagnosticKind::TextureSamplerUnsupported);
                    }
                };

                let Some([x, y, width, height]) = compute_scissor_rect(
                    clip_rect,
                    draw.display_pos,
                    draw.framebuffer_scale,
                    framebuffer_size,
                ) else {
                    continue;
                };
                let scissor = ImguiScissorRect {
                    x,
                    y,
                    width,
                    height,
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

                for target in &target_cameras {
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

    Ok(PreparedImguiDrawData {
        vertices,
        indices,
        draws,
        uniforms_by_view,
    })
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

fn framebuffer_size_for_draw(draw: &imgui::render::DrawDataSnapshot) -> Option<[u32; 2]> {
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

    Some([framebuffer_width as u32, framebuffer_height as u32])
}

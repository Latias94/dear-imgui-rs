//! Single-sample Dear ImGui overlay and final window-output passes.

use std::collections::{HashSet, hash_map::Entry};

use bevy_render::{render_resource::DownlevelFlags, renderer::RenderAdapter};

use super::*;

#[derive(SystemParam)]
pub(super) struct ImguiRenderPassParams<'w> {
    pipeline_cache: Option<Res<'w, PipelineCache>>,
    render_adapter: Option<Res<'w, RenderAdapter>>,
    render_queue: Option<Res<'w, RenderQueue>>,
    queued: Res<'w, ImguiQueuedPipelines>,
    prepared: Res<'w, ImguiPreparedRenderFrame>,
    gpu_buffers: Res<'w, ImguiGpuBuffers>,
    gpu_resources: Option<Res<'w, ImguiPipelineGpuResources>>,
    texture_bind_groups: Res<'w, ImguiTextureBindGroups>,
    renderer_releases: Res<'w, ImguiRendererReleases>,
}

type ImguiOverlayView<'a> = (
    &'a ViewTarget,
    &'a ExtractedView,
    &'a ExtractedCamera,
    &'a CameraMainTextureUsages,
    &'a Msaa,
    Option<&'a ResolvedCompositingSpace>,
);

pub(super) fn render_imgui_overlay(
    view: ViewQuery<ImguiOverlayView>,
    params: ImguiRenderPassParams,
    mut render_context: RenderContext,
) {
    let Some(pipeline_cache) = params.pipeline_cache else {
        return;
    };
    let Some(gpu_resources) = params.gpu_resources else {
        return;
    };
    let Some(render_queue) = params.render_queue else {
        return;
    };
    if !params.gpu_buffers.has_uploaded_buffers() {
        return;
    }

    let (view_target, view, camera, texture_usages, msaa, compositing_space) = view.into_inner();
    let view_id = view.retained_view_entity;
    let Some(pipeline_id) = params.queued.get(view_id) else {
        return;
    };
    let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline_id) else {
        return;
    };

    let released_contexts = params.renderer_releases.release_requested_contexts();
    let mut drawable = params
        .prepared
        .draws()
        .iter()
        .filter(|draw| {
            !released_contexts.contains(&draw.context_id)
                && prepared_draw_matches_view(draw, view, camera, texture_usages, msaa)
        })
        .collect::<Vec<_>>();
    if drawable.is_empty() {
        return;
    }
    drawable.sort_by_key(|draw| (draw.order, draw.context_id.get().get()));

    let compositing_space = ResolvedCompositingSpace::space(compositing_space);
    let mut common_bind_groups = HashMap::new();
    for context_id in drawable.iter().map(|draw| draw.context_id) {
        let Entry::Vacant(entry) = common_bind_groups.entry(context_id) else {
            continue;
        };
        let Some(uniforms) = params
            .prepared
            .uniforms_for_view(context_id, view_id)
            .map(|uniforms| uniforms.with_compositing_space(compositing_space))
        else {
            continue;
        };
        let Some(bind_group) =
            gpu_resources.update_camera_uniforms(context_id, view_id, &render_queue, uniforms)
        else {
            continue;
        };
        entry.insert(bind_group);
    }
    drawable.retain(|draw| common_bind_groups.contains_key(&draw.context_id));
    if drawable.is_empty() {
        return;
    }
    let render_target_size = camera.physical_target_size.map(|size| [size.x, size.y]);

    let color_attachment = view_target.get_unsampled_color_attachment();
    let mut render_pass =
        render_context
            .command_encoder()
            .begin_render_pass(&RenderPassDescriptor {
                label: Some("dear_imgui_bevy_overlay_pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: color_attachment.view,
                    depth_slice: color_attachment.depth_slice,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

    render_pass.set_pipeline(pipeline);
    if let Some(viewport) = render_viewport_for_pass(&drawable, render_target_size) {
        render_pass.set_viewport(
            viewport.physical_position[0] as f32,
            viewport.physical_position[1] as f32,
            viewport.physical_size[0] as f32,
            viewport.physical_size[1] as f32,
            0.0,
            1.0,
        );
    }
    let Some(vertex_buffer) = params.gpu_buffers.vertex_buffer() else {
        return;
    };
    render_pass.set_vertex_buffer(0, *vertex_buffer.slice(..));
    if let Some(index_buffer) = params.gpu_buffers.index_buffer() {
        render_pass.set_index_buffer(*index_buffer.slice(..), IndexFormat::Uint16);
    } else {
        return;
    }

    let supports_base_vertex = params.render_adapter.as_ref().is_some_and(|adapter| {
        adapter
            .get_downlevel_capabilities()
            .flags
            .contains(DownlevelFlags::BASE_VERTEX)
    });
    let mut bound_context = None;
    let mut bound_vertex_offset = 0;
    for draw in drawable {
        if draw.index_range.is_empty() {
            continue;
        }
        let common_bind_group = common_bind_groups
            .get(&draw.context_id)
            .expect("drawable Context must retain its prepared uniform bind group");
        let texture_bind_group = params
            .texture_bind_groups
            .get(&draw.texture, draw.sampler)
            .unwrap_or_else(|| gpu_resources.fallback_bind_group());
        let Some(scissor) = scissor_for_render_pass(draw, render_target_size) else {
            continue;
        };
        if bound_context != Some(draw.context_id) {
            render_pass.set_bind_group(0, *common_bind_group, &[]);
            bound_context = Some(draw.context_id);
        }
        render_pass.set_bind_group(1, texture_bind_group, &[]);
        render_pass.set_scissor_rect(scissor.x, scissor.y, scissor.width, scissor.height);

        // WebGL2 and some downlevel adapters need the command's vertex range bound at slot zero.
        let base_vertex = if supports_base_vertex {
            draw.vertex_offset
        } else {
            if draw.vertex_offset != bound_vertex_offset {
                let offset = draw.vertex_offset as BufferAddress
                    * size_of::<ImguiGpuVertex>() as BufferAddress;
                render_pass.set_vertex_buffer(0, *vertex_buffer.slice(offset..));
                bound_vertex_offset = draw.vertex_offset;
            }
            0
        };
        render_pass.draw_indexed(draw.index_range.clone(), base_vertex, 0..1);
    }
}

fn prepared_draw_matches_view(
    draw: &ImguiPreparedDraw,
    view: &ExtractedView,
    camera: &ExtractedCamera,
    texture_usages: &CameraMainTextureUsages,
    msaa: &Msaa,
) -> bool {
    draw.view == view.retained_view_entity
        && draw.target_format == view.target_format
        && matches!(camera.output_mode, CameraOutputMode::Write { .. })
        && camera.schedule == draw.camera_schedule
        && camera.target.as_ref() == Some(&draw.target)
        && draw.camera_order == camera.order
        && draw.texture_usages == texture_usages.0
        && draw.msaa == *msaa
        && ImguiCameraViewport::matches_extracted_view(
            draw.camera_viewport,
            draw.physical_target_size,
            view,
            camera,
        )
}

/// Touches swapchain outputs immediately before Bevy presents them.
///
/// Vulkan validation requires a swapchain image to leave `UNDEFINED` before present. Bevy can still
/// do an initial present or mark a view as needing present before a real output pass reaches the
/// swapchain, so this final pass is intentionally attached to the root render graph `Finish` set.
pub(super) fn ensure_presentable_window_outputs(
    windows: Query<(&MainEntity, &ExtractedWindow)>,
    extracted: Res<ImguiExtractedRenderFrame>,
    views: Query<(&ViewTarget, &ExtractedCamera)>,
    clear_color: Res<ClearColor>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    let mut encoder = None;
    let imgui_window_outputs = imgui_target_windows(
        extracted
            .context_ids()
            .flat_map(|context_id| extracted.camera_targets(context_id)),
    );

    for (main_entity, window) in windows
        .iter()
        .filter(|(main_entity, _)| imgui_window_outputs.contains(&main_entity.id()))
    {
        let mut view_needs_present = false;
        let mut output_color = None;

        for (view_target, camera) in &views {
            if !camera_targets_window(camera, main_entity.id()) {
                continue;
            }
            view_needs_present |= view_target.needs_present();
            output_color.get_or_insert_with(|| output_clear_color(camera, &clear_color));
        }

        if !window.needs_initial_present && !view_needs_present {
            continue;
        }
        let Some(swapchain_view) = window.swap_chain_texture_view.as_ref() else {
            continue;
        };

        let encoder = encoder.get_or_insert_with(|| {
            render_device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("dear_imgui_bevy_presentable_output_encoder"),
            })
        });
        let ops = if view_needs_present {
            Operations {
                load: LoadOp::Load,
                store: StoreOp::Store,
            }
        } else {
            let clear_color = output_color.flatten().unwrap_or(clear_color.0);
            let clear_color: bevy_color::LinearRgba = clear_color.into();
            Operations {
                load: LoadOp::Clear(clear_color.into()),
                store: StoreOp::Store,
            }
        };

        let render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("dear_imgui_bevy_clear_unwritten_output"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: swapchain_view,
                depth_slice: None,
                resolve_target: None,
                ops,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        drop(render_pass);
    }

    if let Some(encoder) = encoder {
        render_queue.submit([encoder.finish()]);
    }
}

pub(super) fn imgui_target_windows<'a>(
    targets: impl IntoIterator<Item = &'a ImguiCameraTarget>,
) -> HashSet<Entity> {
    targets
        .into_iter()
        .filter_map(|target| match &target.target {
            NormalizedRenderTarget::Window(window) => Some(window.entity()),
            _ => None,
        })
        .collect()
}

fn camera_targets_window(camera: &ExtractedCamera, window: Entity) -> bool {
    matches!(
        camera.target,
        Some(NormalizedRenderTarget::Window(target)) if target.entity() == window
    )
}

fn output_clear_color(
    camera: &ExtractedCamera,
    clear_color: &ClearColor,
) -> Option<bevy_color::Color> {
    match camera.output_mode {
        CameraOutputMode::Skip => None,
        CameraOutputMode::Write {
            clear_color: ClearColorConfig::Custom(color),
            ..
        } => Some(color),
        CameraOutputMode::Write {
            clear_color: ClearColorConfig::Default | ClearColorConfig::None,
            ..
        } => Some(clear_color.0),
    }
}

fn render_viewport_for_draw(draw: &ImguiPreparedDraw) -> Option<ImguiCameraViewport> {
    let camera_viewport = draw.camera_viewport?;
    let [width, height] = camera_viewport.physical_size;
    if width == 0 || height == 0 {
        return None;
    }
    Some(camera_viewport)
}

fn render_viewport_for_pass(
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
    let scissor_max_x = scissor_min_x + u64::from(scissor.width);
    let scissor_max_y = scissor_min_y + u64::from(scissor.height);
    let viewport_min_x = u64::from(rect_position[0]);
    let viewport_min_y = u64::from(rect_position[1]);
    let viewport_max_x = viewport_min_x + u64::from(rect_width);
    let viewport_max_y = viewport_min_y + u64::from(rect_height);

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

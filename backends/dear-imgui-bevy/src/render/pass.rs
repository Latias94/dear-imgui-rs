//! Single-sample Dear ImGui overlay and final window-output passes.

use super::prepare::{render_viewport_for_pass, scissor_for_render_pass};
use super::*;

#[derive(SystemParam)]
pub(super) struct ImguiRenderPassParams<'w> {
    pipeline_cache: Option<Res<'w, PipelineCache>>,
    render_queue: Option<Res<'w, RenderQueue>>,
    queued: Res<'w, ImguiQueuedPipelines>,
    prepared: Res<'w, ImguiPreparedRenderFrame>,
    gpu_buffers: Res<'w, ImguiGpuBuffers>,
    gpu_resources: Option<Res<'w, ImguiPipelineGpuResources>>,
    texture_bind_groups: Res<'w, ImguiTextureBindGroups>,
}

pub(super) fn render_imgui_overlay(
    view: ViewQuery<(&ViewTarget, &ExtractedView, Option<&ExtractedCamera>)>,
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

    let (view_target, view, camera_metadata) = view.into_inner();
    let camera = view.retained_view_entity.main_entity.id();
    let Some(pipeline_id) = params.queued.get(camera) else {
        return;
    };
    let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline_id) else {
        return;
    };

    let drawable = params
        .prepared
        .draws()
        .iter()
        .filter(|draw| draw.camera == camera)
        .collect::<Vec<_>>();
    if drawable.is_empty() {
        return;
    }

    let Some(uniforms) = params.prepared.uniforms_for_camera(camera) else {
        return;
    };
    let uniforms = uniforms.with_gamma(ImguiUniforms::gamma_for_target(
        view.target_format,
        camera_metadata.and_then(|camera| camera.compositing_space),
    ));
    let render_target_size = camera_metadata
        .and_then(|camera| camera.physical_target_size)
        .map(|size| [size.x, size.y]);
    let Some(common_bind_group) =
        gpu_resources.update_camera_uniforms(camera, &render_queue, uniforms)
    else {
        return;
    };

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
    render_pass.set_bind_group(0, common_bind_group, &[]);
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
    if let Some(vertex_buffer) = params.gpu_buffers.vertex_buffer() {
        render_pass.set_vertex_buffer(0, *vertex_buffer.slice(..));
    } else {
        return;
    }
    if let Some(index_buffer) = params.gpu_buffers.index_buffer() {
        render_pass.set_index_buffer(*index_buffer.slice(..), IndexFormat::Uint16);
    } else {
        return;
    }

    for draw in drawable {
        let texture_bind_group = params
            .texture_bind_groups
            .get(&draw.texture, draw.sampler)
            .unwrap_or_else(|| gpu_resources.fallback_bind_group());
        let Some(scissor) = scissor_for_render_pass(draw, render_target_size) else {
            continue;
        };
        render_pass.set_bind_group(1, texture_bind_group, &[]);
        render_pass.set_scissor_rect(scissor.x, scissor.y, scissor.width, scissor.height);
        render_pass.draw_indexed(draw.index_range.clone(), draw.vertex_offset, 0..1);
    }
}

/// Touches swapchain outputs immediately before Bevy presents them.
///
/// Vulkan validation requires a swapchain image to leave `UNDEFINED` before present. Bevy can still
/// do an initial present or mark a view as needing present before a real output pass reaches the
/// swapchain, so this final pass is intentionally attached to the root render graph `Finish` set.
pub(super) fn ensure_presentable_window_outputs(
    windows: Res<ExtractedWindows>,
    views: Query<(&ViewTarget, &ExtractedCamera)>,
    clear_color: Res<ClearColor>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    let mut encoder = None;

    for window in windows.values() {
        let mut view_needs_present = false;
        let mut output_color = None;

        for (view_target, camera) in &views {
            if !camera_targets_window(camera, window.entity) {
                continue;
            }
            view_needs_present |= view_target.needs_present();
            output_color.get_or_insert_with(|| output_clear_color(Some(camera), &clear_color));
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

fn camera_targets_window(camera: &ExtractedCamera, window: Entity) -> bool {
    matches!(
        camera.target,
        Some(NormalizedRenderTarget::Window(target)) if target.entity() == window
    )
}

fn output_clear_color(
    camera: Option<&ExtractedCamera>,
    clear_color: &ClearColor,
) -> Option<bevy_color::Color> {
    match camera.map(|camera| camera.output_mode) {
        Some(CameraOutputMode::Skip) => None,
        Some(CameraOutputMode::Write {
            clear_color: ClearColorConfig::Custom(color),
            ..
        }) => Some(color),
        Some(CameraOutputMode::Write {
            clear_color: ClearColorConfig::Default | ClearColorConfig::None,
            ..
        })
        | None => Some(clear_color.0),
    }
}

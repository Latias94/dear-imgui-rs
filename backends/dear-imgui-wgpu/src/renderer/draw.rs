// Renderer draw helpers: preflight, resource preparation, and one command executor.

use std::ops::Range;

use super::*;
use crate::wgpu;
use dear_imgui_rs::{
    TextureId,
    render::{DrawData, DrawIdx, RawCallbackCommand},
};

// ImGui index type is currently u16 in dear-imgui-rs, but keep this derived so
// future upgrades to u32 require fewer backend changes.
const IMGUI_INDEX_FORMAT: wgpu::IndexFormat = if std::mem::size_of::<DrawIdx>() == 2 {
    wgpu::IndexFormat::Uint16
} else {
    wgpu::IndexFormat::Uint32
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) struct FramebufferExtent {
    width: u32,
    height: u32,
}

impl FramebufferExtent {
    pub(super) fn from_draw_data(draw_data: &DrawData) -> RendererResult<Option<Self>> {
        let size = draw_data.display_size();
        let scale = draw_data.framebuffer_scale();
        let width = size[0] * scale[0];
        let height = size[1] * scale[1];
        if !width.is_finite() || !height.is_finite() {
            return Err(RendererError::InvalidRenderState(
                "draw data produced a non-finite framebuffer extent".to_owned(),
            ));
        }
        if width <= 0.0 || height <= 0.0 {
            return Ok(None);
        }
        if width > u32::MAX as f32 || height > u32::MAX as f32 {
            return Err(RendererError::InvalidRenderState(
                "draw data framebuffer extent exceeds WGPU limits".to_owned(),
            ));
        }
        Ok(Some(Self {
            width: width as u32,
            height: height as u32,
        }))
    }

    pub(super) const fn explicit(width: u32, height: u32) -> Option<Self> {
        if width == 0 || height == 0 {
            None
        } else {
            Some(Self { width, height })
        }
    }

    fn width_f32(self) -> f32 {
        self.width as f32
    }

    fn height_f32(self) -> f32 {
        self.height as f32
    }
}

fn project_scissor_rect(
    clip_rect: [f32; 4],
    clip_off: [f32; 2],
    clip_scale: [f32; 2],
    extent: FramebufferExtent,
) -> RendererResult<Option<[u32; 4]>> {
    let transformed = [
        (clip_rect[0] - clip_off[0]) * clip_scale[0],
        (clip_rect[1] - clip_off[1]) * clip_scale[1],
        (clip_rect[2] - clip_off[0]) * clip_scale[0],
        (clip_rect[3] - clip_off[1]) * clip_scale[1],
    ];
    if transformed.iter().any(|value| !value.is_finite()) {
        return Err(RendererError::InvalidRenderState(
            "draw command contains a non-finite clip rectangle".to_owned(),
        ));
    }

    let clip_min_x = transformed[0].max(0.0);
    let clip_min_y = transformed[1].max(0.0);
    let clip_max_x = transformed[2].min(extent.width_f32());
    let clip_max_y = transformed[3].min(extent.height_f32());
    if clip_max_x <= clip_min_x || clip_max_y <= clip_min_y {
        return Ok(None);
    }
    let scissor = [
        clip_min_x as u32,
        clip_min_y as u32,
        (clip_max_x - clip_min_x) as u32,
        (clip_max_y - clip_min_y) as u32,
    ];
    if scissor[2] == 0 || scissor[3] == 0 {
        Ok(None)
    } else {
        Ok(Some(scissor))
    }
}

pub(super) enum PreparedDrawCommand<'draw> {
    Elements {
        image_bind_group: wgpu::BindGroup,
        scissor: [u32; 4],
        indices: Range<u32>,
        base_vertex: i32,
    },
    ResetRenderState,
    SetSampler(wgpu::BindGroup),
    RawCallback(RawCallbackCommand<'draw>),
}

pub(super) struct PreparedDrawData<'draw> {
    commands: Vec<PreparedDrawCommand<'draw>>,
    linear_common_bind_group: wgpu::BindGroup,
    has_elements: bool,
}

impl PreparedDrawData<'_> {
    pub(super) fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    pub(super) fn has_elements(&self) -> bool {
        self.has_elements
    }
}

pub(super) struct PreparedRenderState {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
}

impl WgpuRenderer {
    pub(super) fn preflight_draw_callback_support(draw_data: &DrawData) -> RendererResult<()> {
        #[cfg(target_arch = "wasm32")]
        for draw_list in draw_data.draw_lists() {
            for command in draw_list.commands() {
                if matches!(command, dear_imgui_rs::render::DrawCmd::RawCallback(_)) {
                    return Err(RendererError::RawDrawCallbackUnsupported);
                }
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        let _ = draw_data;

        Ok(())
    }

    /// Uploads the frame's vertex and index buffers after command preflight succeeds.
    pub(super) fn prepare_frame_resources_static(
        draw_data: &DrawData,
        backend_data: &mut WgpuBackendData,
    ) -> RendererResult<()> {
        let mut total_vtx_count = 0usize;
        let mut total_idx_count = 0usize;
        for draw_list in draw_data.draw_lists() {
            total_vtx_count = total_vtx_count
                .checked_add(draw_list.vtx_buffer().len())
                .ok_or(RendererError::DrawBufferOffsetOverflow { buffer: "vertex" })?;
            total_idx_count = total_idx_count
                .checked_add(draw_list.idx_buffer().len())
                .ok_or(RendererError::DrawBufferOffsetOverflow { buffer: "index" })?;
        }

        if total_vtx_count == 0 && total_idx_count == 0 {
            return Ok(());
        }
        let mut vertices = Vec::with_capacity(total_vtx_count);
        let mut indices = Vec::with_capacity(total_idx_count);
        for draw_list in draw_data.draw_lists() {
            vertices.extend_from_slice(draw_list.vtx_buffer());
            indices.extend_from_slice(draw_list.idx_buffer());
        }

        let frame_index = backend_data.frame_index % backend_data.num_frames_in_flight.get();
        let frame_resources = backend_data
            .frame_resources
            .get_mut(frame_index as usize)
            .ok_or_else(|| {
                RendererError::InvalidRenderState(
                    "WGPU frame resource index is out of bounds".to_owned(),
                )
            })?;

        if total_vtx_count != 0 {
            frame_resources.ensure_vertex_buffer_capacity(&backend_data.device, total_vtx_count)?;
            frame_resources.upload_vertex_data(&backend_data.queue, &vertices)?;
        }
        if total_idx_count != 0 {
            frame_resources.ensure_index_buffer_capacity(&backend_data.device, total_idx_count)?;
            frame_resources.upload_index_data(&backend_data.queue, &indices)?;
        }
        Ok(())
    }

    pub(super) fn prepare_draw_data<'draw>(
        texture_manager: &WgpuTextureManager,
        default_texture: &Option<wgpu::TextureView>,
        draw_data: &'draw DrawData,
        extent: FramebufferExtent,
        backend_data: &mut WgpuBackendData,
    ) -> RendererResult<PreparedDrawData<'draw>> {
        Self::preflight_draw_callback_support(draw_data)?;

        let (linear_common_bind_group, nearest_common_bind_group) = {
            let uniform = backend_data
                .render_resources
                .uniform_buffer()
                .ok_or_else(|| {
                    RendererError::InvalidRenderState("Uniform buffer not initialized".to_owned())
                })?;
            let nearest = backend_data
                .render_resources
                .nearest_common_bind_group()
                .ok_or_else(|| {
                    RendererError::InvalidRenderState(
                        "Nearest sampler bind group not initialized".to_owned(),
                    )
                })?;
            (uniform.bind_group().clone(), nearest.clone())
        };

        let mut commands = Vec::new();
        let mut global_idx_offset = 0u32;
        let mut global_vtx_offset = 0i32;
        let clip_off = draw_data.display_pos();
        let clip_scale = draw_data.framebuffer_scale();
        let mut has_elements = false;

        for draw_list in draw_data.draw_lists() {
            let vertices = draw_list.vtx_buffer();
            let indices = draw_list.idx_buffer();
            for command in draw_list.commands() {
                match command {
                    dear_imgui_rs::render::DrawCmd::Elements { count, cmd_params } => {
                        if count == 0 {
                            continue;
                        }

                        let local_end = cmd_params.idx_offset.checked_add(count).ok_or(
                            RendererError::DrawBufferOffsetOverflow {
                                buffer: "command index",
                            },
                        )?;
                        if local_end > indices.len() {
                            return Err(RendererError::DrawCommandIndexRangeOutOfBounds {
                                start: cmd_params.idx_offset,
                                end: local_end,
                                len: indices.len(),
                            });
                        }
                        let max_index = indices[cmd_params.idx_offset..local_end]
                            .iter()
                            .map(|index| *index as usize)
                            .max()
                            .unwrap_or(0);
                        let referenced_vertex = cmd_params
                            .vtx_offset
                            .checked_add(max_index)
                            .ok_or(RendererError::DrawBufferOffsetOverflow {
                                buffer: "command vertex",
                            })?;
                        if referenced_vertex >= vertices.len() {
                            return Err(RendererError::DrawCommandVertexOutOfBounds {
                                index: referenced_vertex,
                                len: vertices.len(),
                            });
                        }

                        let count = u32::try_from(count).map_err(|_| {
                            RendererError::DrawBufferTooLarge {
                                buffer: "command index",
                            }
                        })?;
                        let local_index = u32::try_from(cmd_params.idx_offset).map_err(|_| {
                            RendererError::DrawBufferTooLarge {
                                buffer: "command index",
                            }
                        })?;
                        let start = global_idx_offset.checked_add(local_index).ok_or(
                            RendererError::DrawBufferOffsetOverflow {
                                buffer: "command index",
                            },
                        )?;
                        let end = start.checked_add(count).ok_or(
                            RendererError::DrawBufferOffsetOverflow {
                                buffer: "command index",
                            },
                        )?;
                        let local_vertex = i32::try_from(cmd_params.vtx_offset).map_err(|_| {
                            RendererError::DrawBufferTooLarge {
                                buffer: "command vertex",
                            }
                        })?;
                        let base_vertex = global_vtx_offset.checked_add(local_vertex).ok_or(
                            RendererError::DrawBufferOffsetOverflow {
                                buffer: "command vertex",
                            },
                        )?;

                        let Some(scissor) = project_scissor_rect(
                            cmd_params.clip_rect,
                            clip_off,
                            clip_scale,
                            extent,
                        )?
                        else {
                            continue;
                        };

                        let texture_id = cmd_params.texture_id;
                        let (cache_id, texture_view) = if texture_id.is_null() {
                            (
                                TextureId::null(),
                                default_texture.as_ref().ok_or_else(|| {
                                    RendererError::InvalidRenderState(
                                        "default WGPU texture is not available".to_owned(),
                                    )
                                })?,
                            )
                        } else {
                            (
                                texture_id,
                                texture_manager
                                    .texture_view(texture_id)
                                    .ok_or(RendererError::InvalidTextureId(texture_id))?,
                            )
                        };
                        let image_bind_group = backend_data
                            .render_resources
                            .get_or_create_image_bind_group(
                                &backend_data.device,
                                cache_id,
                                texture_view,
                            )?
                            .clone();

                        commands.push(PreparedDrawCommand::Elements {
                            image_bind_group,
                            scissor,
                            indices: start..end,
                            base_vertex,
                        });
                        has_elements = true;
                    }
                    dear_imgui_rs::render::DrawCmd::ResetRenderState => {
                        commands.push(PreparedDrawCommand::ResetRenderState);
                    }
                    dear_imgui_rs::render::DrawCmd::SetSamplerLinear => {
                        commands.push(PreparedDrawCommand::SetSampler(
                            linear_common_bind_group.clone(),
                        ));
                    }
                    dear_imgui_rs::render::DrawCmd::SetSamplerNearest => {
                        commands.push(PreparedDrawCommand::SetSampler(
                            nearest_common_bind_group.clone(),
                        ));
                    }
                    dear_imgui_rs::render::DrawCmd::RawCallback(callback) => {
                        commands.push(PreparedDrawCommand::RawCallback(callback));
                    }
                }
            }

            let index_count = u32::try_from(indices.len())
                .map_err(|_| RendererError::DrawBufferTooLarge { buffer: "index" })?;
            global_idx_offset = global_idx_offset
                .checked_add(index_count)
                .ok_or(RendererError::DrawBufferOffsetOverflow { buffer: "index" })?;
            let vertex_count = i32::try_from(vertices.len())
                .map_err(|_| RendererError::DrawBufferTooLarge { buffer: "vertex" })?;
            global_vtx_offset = global_vtx_offset
                .checked_add(vertex_count)
                .ok_or(RendererError::DrawBufferOffsetOverflow { buffer: "vertex" })?;
        }

        Ok(PreparedDrawData {
            commands,
            linear_common_bind_group,
            has_elements,
        })
    }

    pub(super) fn prepare_render_state_static(
        draw_data: &DrawData,
        backend_data: &WgpuBackendData,
        gamma: f32,
        has_elements: bool,
    ) -> RendererResult<PreparedRenderState> {
        let pipeline = backend_data
            .pipeline_state
            .as_ref()
            .ok_or_else(|| RendererError::InvalidRenderState("Pipeline not created".to_owned()))?
            .clone();
        let uniform = backend_data
            .render_resources
            .uniform_buffer()
            .ok_or_else(|| {
                RendererError::InvalidRenderState("Uniform buffer not initialized".to_owned())
            })?;

        let frame_resources = backend_data
            .frame_resources
            .get((backend_data.frame_index % backend_data.num_frames_in_flight.get()) as usize)
            .ok_or_else(|| {
                RendererError::InvalidRenderState(
                    "WGPU frame resource index is out of bounds".to_owned(),
                )
            })?;
        let vertex_buffer = frame_resources.vertex_buffer().cloned();
        let index_buffer = frame_resources.index_buffer().cloned();
        if has_elements && (vertex_buffer.is_none() || index_buffer.is_none()) {
            return Err(RendererError::InvalidRenderState(
                "draw elements require initialized vertex and index buffers".to_owned(),
            ));
        }

        let matrix =
            Uniforms::create_orthographic_matrix(draw_data.display_pos(), draw_data.display_size());
        let mut uniforms = Uniforms::new();
        uniforms.update(matrix, gamma);
        uniform.update(&backend_data.queue, &uniforms);

        Ok(PreparedRenderState {
            pipeline,
            vertex_buffer,
            index_buffer,
        })
    }

    fn setup_prepared_render_state(
        render_pass: &mut wgpu::RenderPass<'_>,
        extent: FramebufferExtent,
        state: &PreparedRenderState,
        linear_common_bind_group: &wgpu::BindGroup,
    ) {
        render_pass.set_viewport(0.0, 0.0, extent.width_f32(), extent.height_f32(), 0.0, 1.0);
        render_pass.set_pipeline(&state.pipeline);
        render_pass.set_bind_group(0, linear_common_bind_group, &[]);
        if let (Some(vertex_buffer), Some(index_buffer)) =
            (&state.vertex_buffer, &state.index_buffer)
        {
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), IMGUI_INDEX_FORMAT);
        }
    }

    pub(super) fn execute_prepared_draw_data(
        prepared: PreparedDrawData<'_>,
        state: &PreparedRenderState,
        extent: FramebufferExtent,
        render_pass: &mut wgpu::RenderPass<'_>,
        platform_io: *mut dear_imgui_rs::sys::ImGuiPlatformIO,
        device: &wgpu::Device,
    ) -> RendererResult<()> {
        unsafe {
            RendererRenderStateGuard::<crate::WgpuRenderStateStorage>::preflight(platform_io)
        }
        .map_err(super::map_renderer_render_state_error)?;
        Self::setup_prepared_render_state(
            render_pass,
            extent,
            state,
            &prepared.linear_common_bind_group,
        );

        let mut callback_state = crate::WgpuRenderStateStorage::new(device, render_pass);
        let guard = unsafe { RendererRenderStateGuard::install(platform_io, &mut callback_state) }
            .map_err(super::map_renderer_render_state_error)?;

        for command in prepared.commands {
            match command {
                PreparedDrawCommand::Elements {
                    image_bind_group,
                    scissor,
                    indices,
                    base_vertex,
                } => {
                    render_pass.set_bind_group(1, &image_bind_group, &[]);
                    render_pass.set_scissor_rect(scissor[0], scissor[1], scissor[2], scissor[3]);
                    render_pass.draw_indexed(indices, base_vertex, 0..1);
                }
                PreparedDrawCommand::ResetRenderState => Self::setup_prepared_render_state(
                    render_pass,
                    extent,
                    state,
                    &prepared.linear_common_bind_group,
                ),
                PreparedDrawCommand::SetSampler(bind_group) => {
                    render_pass.set_bind_group(0, &bind_group, &[]);
                }
                PreparedDrawCommand::RawCallback(callback) => {
                    unsafe { callback.invoke() };
                    guard
                        .validate()
                        .map_err(super::map_renderer_render_state_error)?;
                }
            }
        }

        guard
            .finish()
            .map_err(super::map_renderer_render_state_error)
    }
}

#[cfg(test)]
mod tests {
    use super::{FramebufferExtent, project_scissor_rect};

    #[test]
    fn scissor_projection_rejects_non_finite_values_before_clamping() {
        let extent = FramebufferExtent {
            width: 64,
            height: 64,
        };
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let error =
                project_scissor_rect([invalid, 0.0, 32.0, 32.0], [0.0, 0.0], [1.0, 1.0], extent)
                    .unwrap_err();
            assert!(matches!(error, crate::RendererError::InvalidRenderState(_)));
        }
    }

    #[test]
    fn scissor_projection_clamps_only_finite_rectangles() {
        let extent = FramebufferExtent {
            width: 64,
            height: 64,
        };
        assert_eq!(
            project_scissor_rect([-8.0, -4.0, 72.0, 68.0], [0.0, 0.0], [1.0, 1.0], extent,)
                .unwrap(),
            Some([0, 0, 64, 64])
        );
    }
}

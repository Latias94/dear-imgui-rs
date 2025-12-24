// Renderer draw helpers: frame resources, setup state, draw lists traversal

use super::*;
use dear_imgui_rs::render::{DrawData, DrawIdx};

// ImGui index type is currently u16 in dear-imgui-rs, but keep this derived so
// future upgrades to u32 require fewer backend changes.
const IMGUI_INDEX_FORMAT: wgpu::IndexFormat = if std::mem::size_of::<DrawIdx>() == 2 {
    wgpu::IndexFormat::Uint16
} else {
    wgpu::IndexFormat::Uint32
};

impl WgpuRenderer {
    /// Prepare frame resources (buffers)
    pub(super) fn prepare_frame_resources_static(
        draw_data: &DrawData,
        backend_data: &mut WgpuBackendData,
    ) -> RendererResult<()> {
        // Calculate total vertex and index counts
        let mut total_vtx_count = 0;
        let mut total_idx_count = 0;
        for draw_list in draw_data.draw_lists() {
            total_vtx_count += draw_list.vtx_buffer().len();
            total_idx_count += draw_list.idx_buffer().len();
        }

        if total_vtx_count == 0 || total_idx_count == 0 {
            return Ok(());
        }

        // Collect all vertices and indices first
        let mut vertices = Vec::with_capacity(total_vtx_count);
        let mut indices = Vec::with_capacity(total_idx_count);

        for draw_list in draw_data.draw_lists() {
            vertices.extend_from_slice(draw_list.vtx_buffer());
            indices.extend_from_slice(draw_list.idx_buffer());
        }

        // Get current frame resources and update buffers
        let frame_index = backend_data.frame_index % backend_data.num_frames_in_flight;
        let frame_resources = &mut backend_data.frame_resources[frame_index as usize];

        // Ensure buffer capacity and upload data
        frame_resources.ensure_vertex_buffer_capacity(&backend_data.device, total_vtx_count)?;
        frame_resources.ensure_index_buffer_capacity(&backend_data.device, total_idx_count)?;

        frame_resources.upload_vertex_data(&backend_data.queue, &vertices)?;
        frame_resources.upload_index_data(&backend_data.queue, &indices)?;

        Ok(())
    }

    /// Setup render state
    ///
    /// This corresponds to ImGui_ImplWGPU_SetupRenderState in the C++ implementation
    pub(super) fn setup_render_state_static(
        draw_data: &DrawData,
        render_pass: &mut wgpu::RenderPass,
        backend_data: &WgpuBackendData,
        gamma: f32,
    ) -> RendererResult<()> {
        let pipeline = backend_data
            .pipeline_state
            .as_ref()
            .ok_or_else(|| RendererError::InvalidRenderState("Pipeline not created".to_string()))?;

        // Setup viewport
        let fb_width = draw_data.display_size[0] * draw_data.framebuffer_scale[0];
        let fb_height = draw_data.display_size[1] * draw_data.framebuffer_scale[1];
        render_pass.set_viewport(0.0, 0.0, fb_width, fb_height, 0.0, 1.0);

        // Set pipeline
        render_pass.set_pipeline(pipeline);

        // Update uniforms
        let mvp =
            Uniforms::create_orthographic_matrix(draw_data.display_pos, draw_data.display_size);
        let mut uniforms = Uniforms::new();
        uniforms.update(mvp, gamma);

        // Update uniform buffer
        if let Some(uniform_buffer) = backend_data.render_resources.uniform_buffer() {
            uniform_buffer.update(&backend_data.queue, &uniforms);
            render_pass.set_bind_group(0, uniform_buffer.bind_group(), &[]);
        }

        // Set vertex and index buffers
        let frame_resources = &backend_data.frame_resources
            [(backend_data.frame_index % backend_data.num_frames_in_flight) as usize];
        if let (Some(vertex_buffer), Some(index_buffer)) = (
            frame_resources.vertex_buffer(),
            frame_resources.index_buffer(),
        ) {
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), IMGUI_INDEX_FORMAT);
        }

        Ok(())
    }

    /// Render all draw lists
    pub(super) fn render_draw_lists_static(
        texture_manager: &mut WgpuTextureManager,
        default_texture: &Option<wgpu::TextureView>,
        draw_data: &DrawData,
        render_pass: &mut wgpu::RenderPass,
        backend_data: &mut WgpuBackendData,
        gamma: f32,
    ) -> RendererResult<()> {
        let mut global_vtx_offset = 0i32;
        let mut global_idx_offset = 0u32;
        let clip_scale = draw_data.framebuffer_scale;
        let clip_off = draw_data.display_pos;
        let fb_width = draw_data.display_size[0] * draw_data.framebuffer_scale[0];
        let fb_height = draw_data.display_size[1] * draw_data.framebuffer_scale[1];

        // Extract common bind group handles up front to avoid borrowing conflicts with render_resources.
        let device = backend_data.device.clone();
        let (common_layout, uniform_buffer, default_common_bg) = {
            let ub = backend_data
                .render_resources
                .uniform_buffer()
                .ok_or_else(|| {
                    RendererError::InvalidRenderState("Uniform buffer not initialized".to_string())
                })?;
            (
                ub.bind_group_layout().clone(),
                ub.buffer().clone(),
                ub.bind_group().clone(),
            )
        };
        let mut current_sampler_id: Option<u64> = None; // None = default sampler

        for draw_list in draw_data.draw_lists() {
            for cmd in draw_list.commands() {
                match cmd {
                    dear_imgui_rs::render::DrawCmd::Elements {
                        count,
                        cmd_params,
                        raw_cmd,
                    } => {
                        // Resolve effective ImTextureID now (after texture updates)
                        let tex_id = unsafe {
                            let mut cmd_copy = *raw_cmd;
                            dear_imgui_rs::sys::ImDrawCmd_GetTexID(&mut cmd_copy)
                        } as u64;

                        // Switch common bind group (sampler) if this texture uses a custom sampler.
                        let desired_sampler_id = if tex_id == 0 {
                            None
                        } else {
                            texture_manager.custom_sampler_id_for_texture(tex_id)
                        };
                        if desired_sampler_id != current_sampler_id {
                            if let Some(sampler_id) = desired_sampler_id {
                                if let Some(bg0) = texture_manager
                                    .get_or_create_common_bind_group_for_sampler(
                                        &device,
                                        &common_layout,
                                        &uniform_buffer,
                                        sampler_id,
                                    )
                                {
                                    render_pass.set_bind_group(0, &bg0, &[]);
                                } else {
                                    render_pass.set_bind_group(0, &default_common_bg, &[]);
                                }
                            } else {
                                render_pass.set_bind_group(0, &default_common_bg, &[]);
                            }
                            current_sampler_id = desired_sampler_id;
                        }

                        let texture_bind_group = if tex_id == 0 {
                            if let Some(default_tex) = default_texture {
                                backend_data
                                    .render_resources
                                    .get_or_create_image_bind_group(
                                        &backend_data.device,
                                        0,
                                        default_tex,
                                    )?
                                    .clone()
                            } else {
                                return Err(RendererError::InvalidRenderState(
                                    "Default texture not available".to_string(),
                                ));
                            }
                        } else if let Some(wgpu_texture) = texture_manager.get_texture(tex_id) {
                            backend_data
                                .render_resources
                                .get_or_create_image_bind_group(
                                    &backend_data.device,
                                    tex_id,
                                    wgpu_texture.view(),
                                )?
                                .clone()
                        } else if let Some(default_tex) = default_texture {
                            backend_data
                                .render_resources
                                .get_or_create_image_bind_group(
                                    &backend_data.device,
                                    0,
                                    default_tex,
                                )?
                                .clone()
                        } else {
                            return Err(RendererError::InvalidRenderState(
                                "Texture not found and no default texture".to_string(),
                            ));
                        };

                        render_pass.set_bind_group(1, &texture_bind_group, &[]);

                        // Project scissor/clipping rectangles
                        let clip_min_x = (cmd_params.clip_rect[0] - clip_off[0]) * clip_scale[0];
                        let clip_min_y = (cmd_params.clip_rect[1] - clip_off[1]) * clip_scale[1];
                        let clip_max_x = (cmd_params.clip_rect[2] - clip_off[0]) * clip_scale[0];
                        let clip_max_y = (cmd_params.clip_rect[3] - clip_off[1]) * clip_scale[1];

                        // Clamp to viewport
                        let clip_min_x = clip_min_x.max(0.0);
                        let clip_min_y = clip_min_y.max(0.0);
                        let clip_max_x = clip_max_x.min(fb_width);
                        let clip_max_y = clip_max_y.min(fb_height);

                        if clip_max_x <= clip_min_x || clip_max_y <= clip_min_y {
                            continue;
                        }

                        render_pass.set_scissor_rect(
                            clip_min_x as u32,
                            clip_min_y as u32,
                            (clip_max_x - clip_min_x) as u32,
                            (clip_max_y - clip_min_y) as u32,
                        );

                        // Draw
                        let Ok(count_u32) = u32::try_from(count) else {
                            continue;
                        };
                        let Ok(idx_offset_u32) = u32::try_from(cmd_params.idx_offset) else {
                            continue;
                        };
                        let Some(start_index) = idx_offset_u32.checked_add(global_idx_offset)
                        else {
                            continue;
                        };
                        let Some(end_index) = start_index.checked_add(count_u32) else {
                            continue;
                        };
                        let Ok(vtx_offset_i32) = i32::try_from(cmd_params.vtx_offset) else {
                            continue;
                        };
                        let Some(vertex_offset) = vtx_offset_i32.checked_add(global_vtx_offset)
                        else {
                            continue;
                        };
                        render_pass.draw_indexed(start_index..end_index, vertex_offset, 0..1);
                    }
                    dear_imgui_rs::render::DrawCmd::ResetRenderState => {
                        Self::setup_render_state_static(
                            draw_data,
                            render_pass,
                            backend_data,
                            gamma,
                        )?;
                        current_sampler_id = None;
                    }
                    dear_imgui_rs::render::DrawCmd::RawCallback { .. } => {
                        tracing::warn!(
                            target: "dear-imgui-wgpu",
                            "Warning: Raw callbacks are not supported in WGPU renderer",
                        );
                    }
                }
            }

            let idx_len_u32 = u32::try_from(draw_list.idx_buffer().len())
                .map_err(|_| RendererError::Generic("index buffer too large".to_string()))?;
            global_idx_offset = global_idx_offset.checked_add(idx_len_u32).ok_or_else(|| {
                RendererError::Generic("index buffer offset overflow".to_string())
            })?;

            let vtx_len_i32 = i32::try_from(draw_list.vtx_buffer().len())
                .map_err(|_| RendererError::Generic("vertex buffer too large".to_string()))?;
            global_vtx_offset = global_vtx_offset.checked_add(vtx_len_i32).ok_or_else(|| {
                RendererError::Generic("vertex buffer offset overflow".to_string())
            })?;
        }

        Ok(())
    }
}

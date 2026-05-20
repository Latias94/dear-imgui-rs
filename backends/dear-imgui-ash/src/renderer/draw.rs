use super::*;

impl AshRenderer {
    pub fn cmd_draw(
        &mut self,
        command_buffer: vk::CommandBuffer,
        draw_data: &mut dear_imgui_rs::render::DrawData,
    ) -> RendererResult<()> {
        let gamma = self.gamma();
        if !draw_data.valid() || draw_data.total_vtx_count() == 0 {
            return Ok(());
        }

        self.reap_completed_uploads()?;
        self.process_texture_requests(draw_data)?;

        let Some(mesh) = self.frames.next() else {
            return Err(RendererError::FrameResourcesUnavailable);
        };
        record_draw_commands(
            &self.device,
            &mut self.allocator,
            &self.textures,
            self.default_texture_id,
            self.pipeline_layout,
            command_buffer,
            draw_data,
            self.pipeline,
            gamma,
            mesh,
        )
    }

    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) fn cmd_draw_with_mesh(
        &mut self,
        command_buffer: vk::CommandBuffer,
        draw_data: &mut dear_imgui_rs::render::DrawData,
        pipeline: vk::Pipeline,
        gamma: f32,
        mesh: &mut Mesh,
    ) -> RendererResult<()> {
        if !draw_data.valid() || draw_data.total_vtx_count() == 0 {
            return Ok(());
        }

        self.reap_completed_uploads()?;
        self.process_texture_requests(draw_data)?;
        record_draw_commands(
            &self.device,
            &mut self.allocator,
            &self.textures,
            self.default_texture_id,
            self.pipeline_layout,
            command_buffer,
            draw_data,
            pipeline,
            gamma,
            mesh,
        )
    }
}

pub(super) struct Frames {
    pub(super) meshes: Vec<Mesh>,
    index: usize,
}

impl Frames {
    pub(super) fn new(count: usize) -> Self {
        Self {
            meshes: (0..count).map(|_| Mesh::default()).collect(),
            index: 0,
        }
    }

    pub(super) fn next(&mut self) -> Option<&mut Mesh> {
        if self.meshes.is_empty() {
            return None;
        }
        let i = self.index;
        self.index = (self.index + 1) % self.meshes.len();
        Some(&mut self.meshes[i])
    }

    pub(super) fn destroy(self, device: &Device, allocator: &mut Allocator) -> RendererResult<()> {
        for mesh in self.meshes {
            mesh.destroy(device, allocator)?;
        }
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct Mesh {
    pub(super) vertices: vk::Buffer,
    pub(super) vertices_mem: Option<Memory>,
    pub(super) vertex_capacity: usize,
    pub(super) indices: vk::Buffer,
    pub(super) indices_mem: Option<Memory>,
    pub(super) index_capacity: usize,
}

impl Mesh {
    fn update(
        &mut self,
        device: &Device,
        allocator: &mut Allocator,
        draw_data: &dear_imgui_rs::render::DrawData,
    ) -> RendererResult<()> {
        let vertices = create_vertices(draw_data);
        Self::update_gpu_buffer(
            device,
            allocator,
            &mut self.vertices,
            &mut self.vertices_mem,
            &mut self.vertex_capacity,
            &vertices,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            "vertex buffer size overflow",
        )?;

        let indices = create_indices(draw_data);
        Self::update_gpu_buffer(
            device,
            allocator,
            &mut self.indices,
            &mut self.indices_mem,
            &mut self.index_capacity,
            &indices,
            vk::BufferUsageFlags::INDEX_BUFFER,
            "index buffer size overflow",
        )?;

        Ok(())
    }

    fn update_gpu_buffer<T: Copy>(
        device: &Device,
        allocator: &mut Allocator,
        buffer: &mut vk::Buffer,
        memory: &mut Option<Memory>,
        capacity: &mut usize,
        data: &[T],
        usage: vk::BufferUsageFlags,
        overflow_message: &'static str,
    ) -> RendererResult<()> {
        if data.len() > *capacity {
            let size = data
                .len()
                .checked_mul(std::mem::size_of::<T>())
                .ok_or_else(|| RendererError::Allocator(overflow_message.into()))?;
            let (new_buffer, mut new_mem) = allocator.create_buffer(device, size, usage)?;
            if let Err(err) = allocator.update_buffer(device, &mut new_mem, data) {
                let _ = allocator.destroy_buffer(device, new_buffer, new_mem);
                return Err(err);
            }

            let old_buffer = std::mem::replace(buffer, new_buffer);
            let old_mem = memory.replace(new_mem);
            *capacity = data.len();

            if old_buffer != vk::Buffer::null()
                && let Some(old_mem) = old_mem
            {
                allocator.destroy_buffer(device, old_buffer, old_mem)?;
            }
            return Ok(());
        }

        if let Some(mem) = memory.as_mut() {
            allocator.update_buffer(device, mem, data)?;
        }
        Ok(())
    }

    pub(super) fn destroy(self, device: &Device, allocator: &mut Allocator) -> RendererResult<()> {
        if self.vertices != vk::Buffer::null() {
            if let Some(mem) = self.vertices_mem {
                allocator.destroy_buffer(device, self.vertices, mem)?;
            }
        }
        if self.indices != vk::Buffer::null() {
            if let Some(mem) = self.indices_mem {
                allocator.destroy_buffer(device, self.indices, mem)?;
            }
        }
        Ok(())
    }
}

fn create_vertices(
    draw_data: &dear_imgui_rs::render::DrawData,
) -> Vec<dear_imgui_rs::render::DrawVert> {
    let vertex_count = draw_data.total_vtx_count();
    let mut vertices = Vec::with_capacity(vertex_count);
    for draw_list in draw_data.draw_lists() {
        vertices.extend_from_slice(draw_list.vtx_buffer());
    }
    vertices
}

fn create_indices(
    draw_data: &dear_imgui_rs::render::DrawData,
) -> Vec<dear_imgui_rs::render::DrawIdx> {
    let index_count = draw_data.total_idx_count();
    let mut indices = Vec::with_capacity(index_count);
    for draw_list in draw_data.draw_lists() {
        indices.extend_from_slice(draw_list.idx_buffer());
    }
    indices
}

pub(super) fn record_draw_commands(
    device: &Device,
    allocator: &mut Allocator,
    textures: &TextureManager,
    default_texture_id: u64,
    pipeline_layout: vk::PipelineLayout,
    command_buffer: vk::CommandBuffer,
    draw_data: &dear_imgui_rs::render::DrawData,
    pipeline: vk::Pipeline,
    gamma: f32,
    mesh: &mut Mesh,
) -> RendererResult<()> {
    let fb_width = (draw_data.display_size[0] * draw_data.framebuffer_scale[0]).round();
    let fb_height = (draw_data.display_size[1] * draw_data.framebuffer_scale[1]).round();
    if fb_width <= 0.0 || fb_height <= 0.0 {
        return Ok(());
    }
    let fb_width_u32 = fb_width as u32;
    let fb_height_u32 = fb_height as u32;

    mesh.update(device, allocator, draw_data)?;

    let viewport = vk::Viewport {
        x: 0.0,
        y: 0.0,
        width: fb_width,
        height: fb_height,
        min_depth: 0.0,
        max_depth: 1.0,
    };

    let ortho = ortho_matrix_vk(draw_data.display_pos, draw_data.display_size);
    let push_constants = PushConstants {
        ortho,
        gamma_pad: [gamma, 0.0, 0.0, 0.0],
    };

    unsafe {
        device.cmd_bind_pipeline(command_buffer, vk::PipelineBindPoint::GRAPHICS, pipeline);
        device.cmd_set_viewport(command_buffer, 0, &[viewport]);
        device.cmd_push_constants(
            command_buffer,
            pipeline_layout,
            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            0,
            any_as_u8_slice(&push_constants),
        );
        device.cmd_bind_vertex_buffers(command_buffer, 0, &[mesh.vertices], &[0]);
        device.cmd_bind_index_buffer(command_buffer, mesh.indices, 0, vk::IndexType::UINT16);
    }

    let clip_off = draw_data.display_pos;
    let clip_scale = draw_data.framebuffer_scale;

    let mut global_vtx_offset: i32 = 0;
    let mut global_idx_offset: u32 = 0;

    for draw_list in draw_data.draw_lists() {
        for cmd in draw_list.commands() {
            match cmd {
                dear_imgui_rs::render::DrawCmd::Elements {
                    count,
                    cmd_params,
                    raw_cmd,
                } => {
                    let tex_id = resolve_effective_texture_id(cmd_params.texture_id, raw_cmd);
                    let ds = textures
                        .get_descriptor_set(tex_id.id())
                        .or_else(|| {
                            if tex_id.is_null() {
                                textures.get_descriptor_set(default_texture_id)
                            } else {
                                None
                            }
                        })
                        .ok_or_else(|| RendererError::BadTextureId(tex_id.id()))?;

                    let scissor = clip_rect_to_scissor(
                        cmd_params.clip_rect,
                        clip_off,
                        clip_scale,
                        fb_width_u32,
                        fb_height_u32,
                    );
                    let Some(scissor) = scissor else {
                        continue;
                    };

                    unsafe {
                        device.cmd_set_scissor(command_buffer, 0, &[scissor]);
                        device.cmd_bind_descriptor_sets(
                            command_buffer,
                            vk::PipelineBindPoint::GRAPHICS,
                            pipeline_layout,
                            0,
                            &[ds],
                            &[],
                        );
                    }

                    let Some(count_u32) = u32::try_from(count).ok() else {
                        continue;
                    };
                    let Some(idx_offset_u32) = u32::try_from(cmd_params.idx_offset).ok() else {
                        continue;
                    };
                    let Some(first_index) = idx_offset_u32.checked_add(global_idx_offset) else {
                        continue;
                    };
                    let Ok(vtx_offset_i32) = i32::try_from(cmd_params.vtx_offset) else {
                        continue;
                    };
                    let Some(vertex_offset) = vtx_offset_i32.checked_add(global_vtx_offset) else {
                        continue;
                    };

                    unsafe {
                        device.cmd_draw_indexed(
                            command_buffer,
                            count_u32,
                            1,
                            first_index,
                            vertex_offset,
                            0,
                        );
                    }
                }
                dear_imgui_rs::render::DrawCmd::ResetRenderState => unsafe {
                    device.cmd_bind_pipeline(
                        command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        pipeline,
                    );
                    device.cmd_set_viewport(command_buffer, 0, &[viewport]);
                    device.cmd_push_constants(
                        command_buffer,
                        pipeline_layout,
                        vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        0,
                        any_as_u8_slice(&push_constants),
                    );
                    device.cmd_bind_vertex_buffers(command_buffer, 0, &[mesh.vertices], &[0]);
                    device.cmd_bind_index_buffer(
                        command_buffer,
                        mesh.indices,
                        0,
                        vk::IndexType::UINT16,
                    );
                },
                dear_imgui_rs::render::DrawCmd::SetSamplerLinear
                | dear_imgui_rs::render::DrawCmd::SetSamplerNearest => {
                    // Standard sampler callbacks are only installed by backends that can
                    // switch sampler state without rebuilding Vulkan descriptor bindings.
                }
                dear_imgui_rs::render::DrawCmd::RawCallback { .. } => {
                    // Skip raw callbacks.
                }
            }
        }

        global_idx_offset = global_idx_offset.saturating_add(draw_list.idx_buffer().len() as u32);
        global_vtx_offset = global_vtx_offset.saturating_add(draw_list.vtx_buffer().len() as i32);
    }

    Ok(())
}

fn resolve_effective_texture_id(
    legacy: TextureId,
    raw_cmd: *const dear_imgui_rs::sys::ImDrawCmd,
) -> TextureId {
    if raw_cmd.is_null() {
        return legacy;
    }
    unsafe {
        let mut copy = *raw_cmd;
        TextureId::from(dear_imgui_rs::sys::ImDrawCmd_GetTexID(&mut copy))
    }
}

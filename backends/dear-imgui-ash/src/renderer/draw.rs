use super::*;

impl AshRenderer {
    /// Reconcile managed textures before recording any viewport from this frame.
    ///
    /// Multi-viewport integrations must consume the pending frame here before renderer callbacks
    /// draw secondary viewports. The returned [`ReconciledFrame`] is the only capability that can
    /// expose draw data. The retirement batch must remain associated with the submission/fence
    /// that covers every draw which may still reference the retired resources.
    pub fn prepare_frame<'ctx>(
        &mut self,
        frame: PendingFrame<'ctx>,
    ) -> RendererResult<(ReconciledFrame<'ctx>, Option<TextureRetirementBatch>)> {
        self.ensure_pending_frame_matches(&frame)?;
        let binding = self.context_state.binding();
        binding
            .try_with_bound_context(|| self.prepare_frame_bound(frame))
            .map_err(|error| RendererError::InvalidRenderState(error.to_string()))?
    }

    fn prepare_frame_bound<'ctx>(
        &mut self,
        frame: PendingFrame<'ctx>,
    ) -> RendererResult<(ReconciledFrame<'ctx>, Option<TextureRetirementBatch>)> {
        self.reap_completed_uploads()?;
        let request_epoch = frame.epoch().sequence();
        let feedback = self.process_texture_requests(frame.texture_requests(), request_epoch)?;
        let frame = frame.reconcile_texture_feedback(feedback)?;
        self.textures
            .prune_destroyed_managed_textures(frame.completion_progress().watermark());
        let pending_retirement = self.pending_texture_retirement()?;
        Ok((frame, pending_retirement))
    }

    /// Record a frame previously returned by [`Self::prepare_frame`].
    ///
    /// # Safety
    ///
    /// `command_buffer` must be a live, recording primary command buffer from this renderer's
    /// device, externally synchronized, and inside a compatible render pass or dynamic-rendering
    /// scope. It must not be reset or submitted concurrently. The caller must ensure that no
    /// recorded command buffer is submitted after renderer resources it references are updated,
    /// unregistered, retired, or destroyed. Submission must use the renderer's configured queue,
    /// or provide synchronization equivalent to that queue's upload ordering. Before each call,
    /// the GPU must have completed every earlier draw which can reference the internal mesh slot
    /// selected by this call; normally the application waits the corresponding in-flight fence
    /// before reusing a frame slot.
    pub unsafe fn cmd_draw(
        &mut self,
        command_buffer: vk::CommandBuffer,
        frame: ReconciledFrame<'_>,
    ) -> RendererResult<()> {
        self.ensure_reconciled_frame_matches(&frame)?;
        let binding = self.context_state.binding();
        binding
            .try_with_bound_context(|| {
                let platform_io = platform_io_for_current_context()?;
                unsafe {
                    RendererRenderStateGuard::<AshRenderStateStorage>::preflight(platform_io)
                }
                .map_err(map_renderer_render_state_error)?;
                self.cmd_draw_reconciled_bound(command_buffer, frame, platform_io)
            })
            .map_err(|error| RendererError::InvalidRenderState(error.to_string()))?
    }

    fn cmd_draw_reconciled_bound(
        &mut self,
        command_buffer: vk::CommandBuffer,
        frame: ReconciledFrame<'_>,
        platform_io: *mut dear_imgui_rs::sys::ImGuiPlatformIO,
    ) -> RendererResult<()> {
        let draw_data = frame.draw_data();
        if !draw_data.valid() {
            return Ok(());
        }

        let gamma = self.gamma();
        let Some(mesh) = self.frames.next() else {
            return Err(RendererError::FrameResourcesUnavailable);
        };
        record_draw_commands(
            DrawCommandContext {
                device: &self.device,
                allocator: &mut self.allocator,
                textures: &self.textures,
                default_texture_id: self.default_texture_id,
                pipeline_layout: self.resources.pipeline_layout,
                linear_sampler_set: self.resources.linear_sampler_set,
                nearest_sampler_set: self.resources.nearest_sampler_set,
            },
            DrawCommandInput {
                command_buffer,
                draw_data,
                pipeline: self.resources.pipeline,
                gamma,
                mesh,
                platform_io,
            },
        )?;
        Ok(())
    }

    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) fn cmd_draw_with_mesh(
        &mut self,
        command_buffer: vk::CommandBuffer,
        draw_data: &dear_imgui_rs::render::DrawData,
        pipeline: vk::Pipeline,
        gamma: f32,
        mesh: &mut Mesh,
    ) -> RendererResult<()> {
        self.ensure_operational()?;
        if !draw_data.valid() {
            return Ok(());
        }
        let binding = self.context_state.binding();
        binding
            .try_with_bound_context(|| {
                let platform_io = platform_io_for_current_context()?;
                record_draw_commands(
                    DrawCommandContext {
                        device: &self.device,
                        allocator: &mut self.allocator,
                        textures: &self.textures,
                        default_texture_id: self.default_texture_id,
                        pipeline_layout: self.resources.pipeline_layout,
                        linear_sampler_set: self.resources.linear_sampler_set,
                        nearest_sampler_set: self.resources.nearest_sampler_set,
                    },
                    DrawCommandInput {
                        command_buffer,
                        draw_data,
                        pipeline,
                        gamma,
                        mesh,
                        platform_io,
                    },
                )
            })
            .map_err(|error| RendererError::InvalidRenderState(error.to_string()))?
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
    vertices: GpuBuffer,
    indices: GpuBuffer,
}

impl Mesh {
    fn update(
        &mut self,
        device: &Device,
        allocator: &mut Allocator,
        draw_data: &dear_imgui_rs::render::DrawData,
    ) -> RendererResult<()> {
        let vertices = create_vertices(draw_data);
        self.vertices.update(
            device,
            allocator,
            &vertices,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            "vertex buffer size overflow",
        )?;

        let indices = create_indices(draw_data);
        self.indices.update(
            device,
            allocator,
            &indices,
            vk::BufferUsageFlags::INDEX_BUFFER,
            "index buffer size overflow",
        )?;

        Ok(())
    }
    pub(super) fn destroy(self, device: &Device, allocator: &mut Allocator) -> RendererResult<()> {
        self.vertices.destroy(device, allocator)?;
        self.indices.destroy(device, allocator)
    }
}

#[derive(Default)]
struct GpuBuffer {
    buffer: vk::Buffer,
    memory: Option<Memory>,
    capacity: usize,
}

impl GpuBuffer {
    fn update<T: Copy>(
        &mut self,
        device: &Device,
        allocator: &mut Allocator,
        data: &[T],
        usage: vk::BufferUsageFlags,
        overflow_message: &'static str,
    ) -> RendererResult<()> {
        if data.is_empty() {
            return Ok(());
        }

        if data.len() > self.capacity {
            let size = data
                .len()
                .checked_mul(std::mem::size_of::<T>())
                .ok_or_else(|| RendererError::Allocator(overflow_message.into()))?;
            let (new_buffer, mut new_mem) = allocator.create_buffer(device, size, usage)?;
            if let Err(err) = allocator.update_buffer(device, &mut new_mem, data) {
                let _ = allocator.destroy_buffer(device, new_buffer, new_mem);
                return Err(err);
            }

            let old_buffer = std::mem::replace(&mut self.buffer, new_buffer);
            let old_mem = self.memory.replace(new_mem);
            self.capacity = data.len();

            if old_buffer != vk::Buffer::null()
                && let Some(old_mem) = old_mem
            {
                allocator.destroy_buffer(device, old_buffer, old_mem)?;
            }
            return Ok(());
        }

        if let Some(mem) = self.memory.as_mut() {
            allocator.update_buffer(device, mem, data)?;
        }
        Ok(())
    }

    fn destroy(self, device: &Device, allocator: &mut Allocator) -> RendererResult<()> {
        if self.buffer != vk::Buffer::null()
            && let Some(memory) = self.memory
        {
            allocator.destroy_buffer(device, self.buffer, memory)?;
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

pub(super) struct DrawCommandContext<'renderer> {
    pub(super) device: &'renderer Device,
    pub(super) allocator: &'renderer mut Allocator,
    pub(super) textures: &'renderer TextureManager,
    pub(super) default_texture_id: u64,
    pub(super) pipeline_layout: vk::PipelineLayout,
    pub(super) linear_sampler_set: vk::DescriptorSet,
    pub(super) nearest_sampler_set: vk::DescriptorSet,
}

pub(super) struct DrawCommandInput<'draw> {
    pub(super) command_buffer: vk::CommandBuffer,
    pub(super) draw_data: &'draw dear_imgui_rs::render::DrawData,
    pub(super) pipeline: vk::Pipeline,
    pub(super) gamma: f32,
    pub(super) mesh: &'draw mut Mesh,
    pub(super) platform_io: *mut dear_imgui_rs::sys::ImGuiPlatformIO,
}

struct RenderState<'draw> {
    command_buffer: vk::CommandBuffer,
    pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
    linear_sampler_set: vk::DescriptorSet,
    viewport: vk::Viewport,
    push_constants: PushConstants,
    mesh: &'draw Mesh,
    has_geometry: bool,
}

pub(super) fn record_draw_commands(
    context: DrawCommandContext<'_>,
    input: DrawCommandInput<'_>,
) -> RendererResult<()> {
    let DrawCommandContext {
        device,
        allocator,
        textures,
        default_texture_id,
        pipeline_layout,
        linear_sampler_set,
        nearest_sampler_set,
    } = context;
    let DrawCommandInput {
        command_buffer,
        draw_data,
        pipeline,
        gamma,
        mesh,
        platform_io,
    } = input;
    let display_pos = draw_data.display_pos();
    let display_size = draw_data.display_size();
    let framebuffer_scale = draw_data.framebuffer_scale();
    let fb_width = (display_size[0] * framebuffer_scale[0]).round();
    let fb_height = (display_size[1] * framebuffer_scale[1]).round();
    if fb_width <= 0.0 || fb_height <= 0.0 {
        return Ok(());
    }
    let fb_width_u32 = fb_width as u32;
    let fb_height_u32 = fb_height as u32;

    unsafe { RendererRenderStateGuard::<AshRenderStateStorage>::preflight(platform_io) }
        .map_err(map_renderer_render_state_error)?;
    let prepared = prepare_draw_commands(
        textures,
        default_texture_id,
        linear_sampler_set,
        nearest_sampler_set,
        draw_data,
        fb_width_u32,
        fb_height_u32,
    )?;
    mesh.update(device, allocator, draw_data)?;
    let has_geometry = draw_data.total_vtx_count() > 0 && draw_data.total_idx_count() > 0;

    let viewport = vk::Viewport {
        x: 0.0,
        y: 0.0,
        width: fb_width,
        height: fb_height,
        min_depth: 0.0,
        max_depth: 1.0,
    };

    let ortho = ortho_matrix_vk(display_pos, display_size);
    let render_state = RenderState {
        command_buffer,
        pipeline,
        pipeline_layout,
        linear_sampler_set,
        viewport,
        push_constants: PushConstants {
            ortho,
            gamma_pad: [gamma, 0.0, 0.0, 0.0],
        },
        mesh,
        has_geometry,
    };

    setup_render_state(device, &render_state);

    let mut callback_state = AshRenderStateStorage::new(
        device,
        command_buffer,
        pipeline,
        pipeline_layout,
        linear_sampler_set,
    );
    let mut guard = unsafe { RendererRenderStateGuard::install(platform_io, &mut callback_state) }
        .map_err(map_renderer_render_state_error)?;

    let recording_result: RendererResult<()> = (|| {
        for command in prepared {
            match command {
                PreparedDrawCommand::Elements {
                    descriptor_set,
                    scissor,
                    count,
                    first_index,
                    vertex_offset,
                } => unsafe {
                    device.cmd_set_scissor(command_buffer, 0, &[scissor]);
                    device.cmd_bind_descriptor_sets(
                        command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        pipeline_layout,
                        0,
                        &[descriptor_set],
                        &[],
                    );
                    device.cmd_draw_indexed(
                        command_buffer,
                        count,
                        1,
                        first_index,
                        vertex_offset,
                        0,
                    );
                    guard.state_mut().record_draw_command();
                },
                PreparedDrawCommand::ResetRenderState => {
                    setup_render_state(device, &render_state);
                    guard.state_mut().record_reset(linear_sampler_set);
                }
                PreparedDrawCommand::SetSampler(descriptor_set) => unsafe {
                    device.cmd_bind_descriptor_sets(
                        command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        pipeline_layout,
                        1,
                        &[descriptor_set],
                        &[],
                    );
                    guard.state_mut().set_sampler_descriptor_set(descriptor_set);
                },
                PreparedDrawCommand::RawCallback(callback) => {
                    unsafe { callback.invoke() };
                    guard.validate().map_err(map_renderer_render_state_error)?;
                }
            }
        }
        Ok(())
    })();

    let guard_result = guard.finish().map_err(map_renderer_render_state_error);
    let full_scissor = vk::Rect2D {
        offset: vk::Offset2D { x: 0, y: 0 },
        extent: vk::Extent2D {
            width: fb_width_u32,
            height: fb_height_u32,
        },
    };
    unsafe { device.cmd_set_scissor(command_buffer, 0, &[full_scissor]) };

    recording_result?;
    guard_result
}

enum PreparedDrawCommand<'draw> {
    Elements {
        descriptor_set: vk::DescriptorSet,
        scissor: vk::Rect2D,
        count: u32,
        first_index: u32,
        vertex_offset: i32,
    },
    ResetRenderState,
    SetSampler(vk::DescriptorSet),
    RawCallback(dear_imgui_rs::render::RawCallbackCommand<'draw>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ValidatedElementRange {
    count: u32,
    first_index: u32,
    vertex_offset: i32,
}

fn validate_element_range(
    indices: &[dear_imgui_rs::render::DrawIdx],
    vertex_count: usize,
    count: usize,
    index_offset: usize,
    vertex_offset: usize,
    global_index_offset: u32,
    global_vertex_offset: i32,
) -> RendererResult<ValidatedElementRange> {
    let local_end =
        index_offset
            .checked_add(count)
            .ok_or(RendererError::DrawBufferOffsetOverflow {
                buffer: "command index",
            })?;
    if local_end > indices.len() {
        return Err(RendererError::DrawCommandIndexRangeOutOfBounds {
            start: index_offset,
            end: local_end,
            len: indices.len(),
        });
    }
    let max_index = indices[index_offset..local_end]
        .iter()
        .map(|index| *index as usize)
        .max()
        .unwrap_or(0);
    let referenced_vertex =
        vertex_offset
            .checked_add(max_index)
            .ok_or(RendererError::DrawBufferOffsetOverflow {
                buffer: "command vertex",
            })?;
    if referenced_vertex >= vertex_count {
        return Err(RendererError::DrawCommandVertexOutOfBounds {
            index: referenced_vertex,
            len: vertex_count,
        });
    }

    let count = u32::try_from(count).map_err(|_| RendererError::DrawBufferTooLarge {
        buffer: "command index",
    })?;
    let local_index =
        u32::try_from(index_offset).map_err(|_| RendererError::DrawBufferTooLarge {
            buffer: "command index",
        })?;
    let first_index = global_index_offset.checked_add(local_index).ok_or(
        RendererError::DrawBufferOffsetOverflow {
            buffer: "command index",
        },
    )?;
    let local_vertex =
        i32::try_from(vertex_offset).map_err(|_| RendererError::DrawBufferTooLarge {
            buffer: "command vertex",
        })?;
    let vertex_offset = global_vertex_offset.checked_add(local_vertex).ok_or(
        RendererError::DrawBufferOffsetOverflow {
            buffer: "command vertex",
        },
    )?;
    Ok(ValidatedElementRange {
        count,
        first_index,
        vertex_offset,
    })
}

fn texture_descriptor_set(
    textures: &TextureManager,
    default_texture_id: u64,
    texture_id: TextureId,
) -> RendererResult<vk::DescriptorSet> {
    textures
        .get_descriptor_set(texture_id.id())
        .or_else(|| {
            texture_id
                .is_null()
                .then(|| textures.get_descriptor_set(default_texture_id))
                .flatten()
        })
        .ok_or_else(|| RendererError::BadTextureId(texture_id.id()))
}

fn prepare_draw_commands<'draw>(
    textures: &TextureManager,
    default_texture_id: u64,
    linear_sampler_set: vk::DescriptorSet,
    nearest_sampler_set: vk::DescriptorSet,
    draw_data: &'draw dear_imgui_rs::render::DrawData,
    fb_width: u32,
    fb_height: u32,
) -> RendererResult<Vec<PreparedDrawCommand<'draw>>> {
    let mut commands = Vec::new();
    let mut global_idx_offset = 0_u32;
    let mut global_vtx_offset = 0_i32;
    let clip_off = draw_data.display_pos();
    let clip_scale = draw_data.framebuffer_scale();

    for draw_list in draw_data.draw_lists() {
        let vertices = draw_list.vtx_buffer();
        let indices = draw_list.idx_buffer();
        for command in draw_list.commands() {
            match command {
                dear_imgui_rs::render::DrawCmd::Elements { count, cmd_params } => {
                    if count == 0 {
                        continue;
                    }
                    let range = validate_element_range(
                        indices,
                        vertices.len(),
                        count,
                        cmd_params.idx_offset,
                        cmd_params.vtx_offset,
                        global_idx_offset,
                        global_vtx_offset,
                    )?;
                    let Some(scissor) = clip_rect_to_scissor(
                        cmd_params.clip_rect,
                        clip_off,
                        clip_scale,
                        fb_width,
                        fb_height,
                    ) else {
                        continue;
                    };
                    let texture_id = cmd_params.texture_id;
                    let descriptor_set =
                        texture_descriptor_set(textures, default_texture_id, texture_id)?;
                    commands.push(PreparedDrawCommand::Elements {
                        descriptor_set,
                        scissor,
                        count: range.count,
                        first_index: range.first_index,
                        vertex_offset: range.vertex_offset,
                    });
                }
                dear_imgui_rs::render::DrawCmd::ResetRenderState => {
                    commands.push(PreparedDrawCommand::ResetRenderState);
                }
                dear_imgui_rs::render::DrawCmd::SetSamplerLinear => {
                    commands.push(PreparedDrawCommand::SetSampler(linear_sampler_set));
                }
                dear_imgui_rs::render::DrawCmd::SetSamplerNearest => {
                    commands.push(PreparedDrawCommand::SetSampler(nearest_sampler_set));
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

    Ok(commands)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(any(feature = "gpu-allocator", feature = "vk-mem")))]
    fn complete_initial_texture_requests(frame: PendingFrame<'_>) -> ReconciledFrame<'_> {
        let feedback = frame
            .texture_requests()
            .iter()
            .enumerate()
            .map(|(index, request)| match request.kind() {
                dear_imgui_rs::render::TextureRequestKind::Destroy => request.destroyed().unwrap(),
                dear_imgui_rs::render::TextureRequestKind::Create
                | dear_imgui_rs::render::TextureRequestKind::Update => request
                    .uploaded(TextureId::from(u64::try_from(index + 1).unwrap()))
                    .unwrap(),
            })
            .collect::<Vec<_>>();
        assert!(!feedback.is_empty());
        frame
            .reconcile_texture_feedback(feedback)
            .expect("synthetic renderer feedback should initialize Context texture bindings")
    }

    #[cfg(not(any(feature = "gpu-allocator", feature = "vk-mem")))]
    #[test]
    fn split_prepare_and_record_reconciles_once_and_preserves_retirement_batch() {
        use crate::renderer::lifecycle::renderer_for_test;
        use crate::renderer::texture::{
            ManagedTextureRetirementKey, RetiredManagedVulkanTexture, VulkanTexture,
        };
        use dear_imgui_rs::FramePrepareOptions;

        let mut context = Context::create();
        let mut renderer = renderer_for_test(&mut context);
        renderer.frames = Frames::new(1);

        context.prepare_frame(
            FramePrepareOptions::new([0.0, 0.0], 1.0 / 60.0).renderer_has_textures(),
        );
        let initial = context
            .begin_frame()
            .render(renderer.renderer_consumer().unwrap());
        drop(complete_initial_texture_requests(initial));

        let reservation = renderer
            .textures
            .retiring_textures
            .reserve()
            .expect("synthetic retirement queue should have capacity");
        let expected_batch = reservation.batch();
        let retirement_key = ManagedTextureRetirementKey::Destroyed(SnapshotTextureId::FontAtlas {
            context: context.id(),
            stamp: 7,
            generation: 11,
        });
        let retired = RetiredManagedVulkanTexture {
            texture_id: TextureId::from(99_u64),
            texture: VulkanTexture {
                image: vk::Image::null(),
                image_mem: vk::DeviceMemory::null(),
                image_view: vk::ImageView::null(),
                descriptor_set: vk::DescriptorSet::null(),
                width: 1,
                height: 1,
            },
        };
        assert_eq!(
            renderer
                .textures
                .retiring_textures
                .commit(reservation, retirement_key, retired),
            expected_batch
        );

        context.prepare_frame(
            FramePrepareOptions::new([0.0, 0.0], 1.0 / 60.0).renderer_has_textures(),
        );
        let pending = context
            .begin_frame()
            .render(renderer.renderer_consumer().unwrap());
        assert!(pending.texture_requests().is_empty());
        let (reconciled, prepared_batch) = renderer
            .prepare_frame(pending)
            .expect("an empty request phase should reconcile without Vulkan work");
        assert_eq!(prepared_batch, Some(expected_batch));

        unsafe { renderer.cmd_draw(vk::CommandBuffer::null(), reconciled) }
            .expect("a zero-sized framebuffer should not issue Vulkan commands");
        assert_eq!(
            renderer.pending_texture_retirement().unwrap(),
            Some(expected_batch)
        );

        renderer.destroyed = true;
    }

    #[cfg(not(any(feature = "gpu-allocator", feature = "vk-mem")))]
    #[test]
    fn rejected_prepare_abandons_the_epoch_and_reissues_requests() {
        use crate::renderer::lifecycle::renderer_for_test;
        use dear_imgui_rs::FramePrepareOptions;

        let mut owner = Context::create();
        let mut renderer = renderer_for_test(&mut owner);
        let owner = owner.suspend_or_panic();

        let mut foreign = Context::create();
        foreign.prepare_frame(
            FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0).renderer_has_textures(),
        );
        let consumer = foreign.create_synchronous_renderer_consumer().unwrap();

        let pending = foreign.begin_frame().render(&consumer);
        let first_requests = pending
            .texture_requests()
            .iter()
            .map(|request| (request.texture(), request.kind(), request.upload_identity()))
            .collect::<Vec<_>>();
        assert!(!first_requests.is_empty());
        assert!(matches!(
            renderer.prepare_frame(pending),
            Err(RendererError::ContextMismatch { .. })
        ));

        let retry = foreign.begin_frame().render(&consumer);
        let retried_requests = retry
            .texture_requests()
            .iter()
            .map(|request| (request.texture(), request.kind(), request.upload_identity()))
            .collect::<Vec<_>>();
        assert_eq!(retried_requests, first_requests);
        drop(retry);

        drop(consumer);
        drop(foreign);
        let owner = owner.activate().expect("owner Context should reactivate");
        renderer.destroyed = true;
        drop(renderer);
        drop(owner);
    }

    #[test]
    fn element_range_rejects_index_and_vertex_violations() {
        let index_error = validate_element_range(&[0, 1], 2, 2, 1, 0, 0, 0).unwrap_err();
        assert!(matches!(
            index_error,
            RendererError::DrawCommandIndexRangeOutOfBounds {
                start: 1,
                end: 3,
                len: 2,
            }
        ));

        let vertex_error = validate_element_range(&[0, 2], 2, 2, 0, 0, 0, 0).unwrap_err();
        assert!(matches!(
            vertex_error,
            RendererError::DrawCommandVertexOutOfBounds { index: 2, len: 2 }
        ));
    }

    #[test]
    fn element_range_rejects_arithmetic_and_vulkan_offset_overflow() {
        let command_vertex =
            validate_element_range(&[1], usize::MAX, 1, 0, usize::MAX, 0, 0).unwrap_err();
        assert!(matches!(
            command_vertex,
            RendererError::DrawBufferOffsetOverflow {
                buffer: "command vertex"
            }
        ));

        let first_index = validate_element_range(&[0, 0], 1, 1, 1, 0, u32::MAX, 0).unwrap_err();
        assert!(matches!(
            first_index,
            RendererError::DrawBufferOffsetOverflow {
                buffer: "command index"
            }
        ));

        let vertex_offset =
            validate_element_range(&[0], usize::MAX, 1, 0, 1, 0, i32::MAX).unwrap_err();
        assert!(matches!(
            vertex_offset,
            RendererError::DrawBufferOffsetOverflow {
                buffer: "command vertex"
            }
        ));

        let oversized_vertex =
            validate_element_range(&[0], usize::MAX, 1, 0, i32::MAX as usize + 1, 0, 0)
                .unwrap_err();
        assert!(matches!(
            oversized_vertex,
            RendererError::DrawBufferTooLarge {
                buffer: "command vertex"
            }
        ));
    }

    #[test]
    fn unknown_texture_id_is_rejected_before_recording() {
        let textures = TextureManager::new();
        assert!(matches!(
            texture_descriptor_set(&textures, 0, TextureId::from(41_u64)),
            Err(RendererError::BadTextureId(41))
        ));
    }
}

fn setup_render_state(device: &Device, state: &RenderState<'_>) {
    unsafe {
        device.cmd_bind_pipeline(
            state.command_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            state.pipeline,
        );
        device.cmd_set_viewport(state.command_buffer, 0, &[state.viewport]);
        device.cmd_push_constants(
            state.command_buffer,
            state.pipeline_layout,
            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            0,
            any_as_u8_slice(&state.push_constants),
        );
        if state.has_geometry {
            device.cmd_bind_vertex_buffers(
                state.command_buffer,
                0,
                &[state.mesh.vertices.buffer],
                &[0],
            );
            device.cmd_bind_index_buffer(
                state.command_buffer,
                state.mesh.indices.buffer,
                0,
                vk::IndexType::UINT16,
            );
        }
        device.cmd_bind_descriptor_sets(
            state.command_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            state.pipeline_layout,
            1,
            &[state.linear_sampler_set],
            &[],
        );
    }
}

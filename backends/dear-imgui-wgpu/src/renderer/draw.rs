// Renderer draw helpers: preflight, resource preparation, and one command executor.

use std::{collections::HashSet, ops::Range, sync::Arc};

use super::draw_source::{DrawCommand, DrawSource};
use super::*;
use crate::{FrameResources, WgpuDrawDependencies, wgpu};
use dear_imgui_backend_support::try_compute_scissor_rect;
use dear_imgui_rs::{
    TextureId,
    render::{DrawData, DrawIdx, DrawRequirements, RawCallbackCommand, TextureBinding},
};

const IMGUI_INDEX_FORMAT: wgpu::IndexFormat = match std::mem::size_of::<DrawIdx>() {
    2 => wgpu::IndexFormat::Uint16,
    4 => wgpu::IndexFormat::Uint32,
    _ => panic!("Dear ImGui draw indices must be 16 or 32 bits"),
};

/// Physical dimensions of the WGPU render target receiving Dear ImGui commands.
///
/// WGPU render passes do not expose attachment dimensions. Applications must therefore pass the
/// extent of the texture view used to create the render pass instead of asking the renderer to
/// infer it from Dear ImGui's logical display metrics.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct FramebufferExtent {
    width: u32,
    height: u32,
}

impl FramebufferExtent {
    /// Creates an extent. A zero width or height represents a target that cannot be drawn yet.
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Returns the extent of a WGPU texture.
    pub fn from_texture(texture: &wgpu::Texture) -> Self {
        let size = texture.size();
        Self::new(size.width, size.height)
    }

    /// Returns the physical width in pixels.
    pub const fn width(self) -> u32 {
        self.width
    }

    /// Returns the physical height in pixels.
    pub const fn height(self) -> u32 {
        self.height
    }

    /// Returns whether the target has no drawable area.
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
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
    try_compute_scissor_rect(
        clip_rect,
        clip_off,
        clip_scale,
        [extent.width, extent.height],
    )
    .map_err(|error| RendererError::InvalidRenderState(error.to_string()))
}

pub(super) enum PreparedDrawCommand<'draw> {
    Elements {
        image_bind_group: wgpu::BindGroup,
        scissor: [u32; 4],
        indices: Range<u32>,
        base_vertex: i32,
    },
    ResetRenderState,
    SetSampler(PreparedSampler),
    RawCallback(RawCallbackCommand<'draw>),
}

#[derive(Copy, Clone)]
pub(super) enum PreparedSampler {
    Linear,
    Nearest,
}

pub(super) struct PreparedDrawData<'draw> {
    commands: Vec<PreparedDrawCommand<'draw>>,
    has_elements: bool,
    dependencies: WgpuDrawDependencies,
    managed_texture_reads: Vec<Arc<()>>,
}

impl PreparedDrawData<'_> {
    pub(super) fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    pub(super) fn has_elements(&self) -> bool {
        self.has_elements
    }

    pub(crate) fn dependencies(&self) -> &WgpuDrawDependencies {
        &self.dependencies
    }
}

pub(super) struct PreparedRenderState {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    supports_base_vertex: bool,
    linear_common_bind_group: wgpu::BindGroup,
    nearest_common_bind_group: wgpu::BindGroup,
    recording_lease: Arc<()>,
}

impl WgpuRenderer {
    pub(super) fn preflight_draw_callback_support(
        requirements: DrawRequirements,
    ) -> RendererResult<()> {
        #[cfg(target_arch = "wasm32")]
        if requirements.requires_raw_callback_support() {
            return Err(RendererError::RawDrawCallbackUnsupported);
        }

        #[cfg(not(target_arch = "wasm32"))]
        let _ = requirements;

        Ok(())
    }

    /// Uploads the frame's vertex and index buffers after command preflight succeeds.
    pub(super) fn prepare_frame_resources_static(
        draw_data: DrawSource<'_>,
        frame_resources: &mut FrameResources,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> RendererResult<()> {
        let mut total_vtx_count = 0usize;
        let mut total_idx_count = 0usize;
        for draw_list in draw_data.lists() {
            total_vtx_count = total_vtx_count
                .checked_add(draw_list.vertices().len())
                .ok_or(RendererError::DrawBufferOffsetOverflow { buffer: "vertex" })?;
            total_idx_count = total_idx_count
                .checked_add(draw_list.indices().len())
                .ok_or(RendererError::DrawBufferOffsetOverflow { buffer: "index" })?;
        }

        frame_resources.upload_draw_lists(
            device,
            queue,
            [total_vtx_count, total_idx_count],
            draw_data
                .lists()
                .map(|list| (list.vertices(), list.indices())),
        )
    }

    pub(super) fn prepare_draw_data<'draw>(
        texture_manager: &WgpuTextureManager,
        default_texture: &Option<wgpu::TextureView>,
        draw_data: &'draw DrawData,
        extent: FramebufferExtent,
        backend_data: &mut WgpuBackendData,
    ) -> RendererResult<PreparedDrawData<'draw>> {
        Self::preflight_draw_callback_support(draw_data.requirements())?;
        Self::prepare_draw_source(
            texture_manager,
            default_texture,
            DrawSource::Live(draw_data),
            extent,
            backend_data,
        )
    }

    pub(super) fn prepare_draw_source<'draw>(
        texture_manager: &WgpuTextureManager,
        default_texture: &Option<wgpu::TextureView>,
        draw_data: DrawSource<'draw>,
        extent: FramebufferExtent,
        backend_data: &mut WgpuBackendData,
    ) -> RendererResult<PreparedDrawData<'draw>> {
        let mut commands = Vec::new();
        let mut global_idx_offset = 0u32;
        let mut global_vtx_offset = 0i32;
        let clip_off = draw_data.display_pos();
        let clip_scale = draw_data.framebuffer_scale();
        let mut has_elements = false;
        let mut dependencies = WgpuDrawDependencies::default();
        let mut textures_seen = HashSet::new();
        let mut managed_texture_reads = Vec::new();

        for draw_list in draw_data.lists() {
            let vertices = draw_list.vertices();
            let indices = draw_list.indices();
            for command in draw_list.commands() {
                match command {
                    DrawCommand::Elements {
                        count,
                        clip_rect,
                        texture,
                        vtx_offset,
                        idx_offset,
                    } => {
                        if count == 0 {
                            continue;
                        }

                        let local_end = idx_offset.checked_add(count).ok_or(
                            RendererError::DrawBufferOffsetOverflow {
                                buffer: "command index",
                            },
                        )?;
                        if local_end > indices.len() {
                            return Err(RendererError::DrawCommandIndexRangeOutOfBounds {
                                start: idx_offset,
                                end: local_end,
                                len: indices.len(),
                            });
                        }
                        let max_index = indices[idx_offset..local_end]
                            .iter()
                            .map(|index| *index as usize)
                            .max()
                            .unwrap_or(0);
                        let referenced_vertex = vtx_offset.checked_add(max_index).ok_or(
                            RendererError::DrawBufferOffsetOverflow {
                                buffer: "command vertex",
                            },
                        )?;
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
                        let local_index = u32::try_from(idx_offset).map_err(|_| {
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
                        let local_vertex = i32::try_from(vtx_offset).map_err(|_| {
                            RendererError::DrawBufferTooLarge {
                                buffer: "command vertex",
                            }
                        })?;
                        let base_vertex = global_vtx_offset.checked_add(local_vertex).ok_or(
                            RendererError::DrawBufferOffsetOverflow {
                                buffer: "command vertex",
                            },
                        )?;

                        let Some(scissor) =
                            project_scissor_rect(clip_rect, clip_off, clip_scale, extent)?
                        else {
                            continue;
                        };

                        let texture_id =
                            match texture {
                                TextureBinding::Legacy(id) => id,
                                TextureBinding::Managed(id) => texture_manager
                                    .managed_texture_id(id)
                                    .ok_or(RendererError::ManagedTextureMissing(id))?,
                            };
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

                        if !texture_id.is_null() && textures_seen.insert(texture_id) {
                            if let Some(binding) = texture_manager.external_binding(texture_id) {
                                dependencies.external_bindings.push(binding);
                            } else if let Some(lease) =
                                texture_manager.managed_recording_lease(texture_id)
                            {
                                managed_texture_reads.push(lease);
                            }
                        }

                        commands.push(PreparedDrawCommand::Elements {
                            image_bind_group,
                            scissor,
                            indices: start..end,
                            base_vertex,
                        });
                        has_elements = true;
                    }
                    DrawCommand::ResetRenderState => {
                        commands.push(PreparedDrawCommand::ResetRenderState);
                    }
                    DrawCommand::SetSamplerLinear => {
                        commands.push(PreparedDrawCommand::SetSampler(PreparedSampler::Linear));
                    }
                    DrawCommand::SetSamplerNearest => {
                        commands.push(PreparedDrawCommand::SetSampler(PreparedSampler::Nearest));
                    }
                    DrawCommand::SetSamplerFromTexture => {
                        return Err(RendererError::TextureSamplerUnsupported);
                    }
                    DrawCommand::RawCallback(callback) => {
                        dependencies.contains_unknown_callback_dependencies = true;
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
            has_elements,
            dependencies,
            managed_texture_reads,
        })
    }

    pub(super) fn prepare_render_state_static(
        draw_data: &DrawData,
        backend_data: &mut WgpuBackendData,
        gamma: f32,
        has_elements: bool,
    ) -> RendererResult<PreparedRenderState> {
        Self::prepare_render_state_source(
            DrawSource::Live(draw_data),
            backend_data,
            gamma,
            has_elements,
        )
    }

    pub(super) fn prepare_render_state_source(
        draw_data: DrawSource<'_>,
        backend_data: &mut WgpuBackendData,
        gamma: f32,
        has_elements: bool,
    ) -> RendererResult<PreparedRenderState> {
        let pipeline = backend_data
            .pipeline_state
            .as_ref()
            .ok_or_else(|| RendererError::InvalidRenderState("Pipeline not created".to_owned()))?
            .clone();
        let device = backend_data.device.clone();
        let queue = backend_data.queue.clone();
        let supports_base_vertex = backend_data.supports_base_vertex;
        let frame_resources = backend_data.acquire_frame_resources()?;
        Self::prepare_frame_resources_static(draw_data, frame_resources, &device, &queue)?;
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
        let uniform = frame_resources.uniform_buffer()?;
        uniform.update(&queue, &uniforms);

        Ok(PreparedRenderState {
            pipeline,
            vertex_buffer,
            index_buffer,
            supports_base_vertex,
            linear_common_bind_group: uniform.bind_group().clone(),
            nearest_common_bind_group: frame_resources.nearest_common_bind_group()?.clone(),
            recording_lease: frame_resources.recording_lease(),
        })
    }

    fn setup_prepared_render_state(
        render_pass: &mut wgpu::RenderPass<'_>,
        extent: FramebufferExtent,
        state: &PreparedRenderState,
    ) {
        render_pass.set_viewport(0.0, 0.0, extent.width_f32(), extent.height_f32(), 0.0, 1.0);
        render_pass.set_pipeline(&state.pipeline);
        render_pass.set_bind_group(0, &state.linear_common_bind_group, &[]);
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
            RendererRenderStateGuard::<crate::WgpuRenderStateStorage<'_, '_>>::preflight(
                platform_io,
            )
        }
        .map_err(super::map_renderer_render_state_error)?;

        Self::execute_prepared_commands(
            prepared,
            state,
            extent,
            render_pass,
            |callback, render_pass| {
                let mut callback_state = crate::WgpuRenderStateStorage::new(device, render_pass);
                let guard =
                    unsafe { RendererRenderStateGuard::install(platform_io, &mut callback_state) }
                        .map_err(super::map_renderer_render_state_error)?;
                unsafe { callback.invoke() };
                guard
                    .finish()
                    .map_err(super::map_renderer_render_state_error)
            },
        )
    }

    pub(super) fn execute_prepared_commands<'draw, 'encoder>(
        prepared: PreparedDrawData<'draw>,
        state: &PreparedRenderState,
        extent: FramebufferExtent,
        render_pass: &mut wgpu::RenderPass<'encoder>,
        mut raw_callback: impl FnMut(
            RawCallbackCommand<'draw>,
            &mut wgpu::RenderPass<'encoder>,
        ) -> RendererResult<()>,
    ) -> RendererResult<()> {
        // Deferred actions stay with this encoder through finish and are dropped with discarded
        // commands. Register before recording so even partially recorded passes keep their leases.
        let frame_lease = Arc::clone(&state.recording_lease);
        let texture_leases = prepared.managed_texture_reads;
        render_pass.on_submitted_work_done(move || drop((frame_lease, texture_leases)));
        Self::setup_prepared_render_state(render_pass, extent, state);
        let mut bound_vertex_offset = Some(0);
        for command in prepared.commands {
            match command {
                PreparedDrawCommand::Elements {
                    image_bind_group,
                    scissor,
                    indices,
                    base_vertex,
                } => {
                    let (vertex_offset, draw_base_vertex) = if state.supports_base_vertex {
                        (0, base_vertex)
                    } else {
                        (base_vertex, 0)
                    };
                    if bound_vertex_offset != Some(vertex_offset) {
                        let buffer = state.vertex_buffer.as_ref().ok_or_else(|| {
                            RendererError::InvalidRenderState("Missing vertex buffer".into())
                        })?;
                        let offset = vertex_offset as u64
                            * size_of::<dear_imgui_rs::render::DrawVert>() as u64;
                        render_pass.set_vertex_buffer(0, buffer.slice(offset..));
                        bound_vertex_offset = Some(vertex_offset);
                    }
                    render_pass.set_bind_group(1, &image_bind_group, &[]);
                    render_pass.set_scissor_rect(scissor[0], scissor[1], scissor[2], scissor[3]);
                    render_pass.draw_indexed(indices, draw_base_vertex, 0..1);
                }
                PreparedDrawCommand::ResetRenderState => {
                    Self::setup_prepared_render_state(render_pass, extent, state);
                    bound_vertex_offset = Some(0);
                }
                PreparedDrawCommand::SetSampler(sampler) => {
                    let bind_group = match sampler {
                        PreparedSampler::Linear => &state.linear_common_bind_group,
                        PreparedSampler::Nearest => &state.nearest_common_bind_group,
                    };
                    render_pass.set_bind_group(0, bind_group, &[]);
                }
                PreparedDrawCommand::RawCallback(callback) => {
                    raw_callback(callback, render_pass)?;
                    bound_vertex_offset = None;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        WgpuInitInfo,
        texture::{external_test_texture, request_test_device},
    };
    use dear_imgui_rs::{Context, internal::RawCast, sys};

    fn element_command(texture: TextureId, count: u32, clip: [f32; 4]) -> sys::ImDrawCmd {
        let mut command = sys::ImDrawCmd::default();
        command.TexRef._TexID = texture.id();
        command.ElemCount = count;
        command.ClipRect = sys::ImVec4 {
            x: clip[0],
            y: clip[1],
            z: clip[2],
            w: clip[3],
        };
        command
    }

    fn with_draw_commands<R>(
        commands: &mut [sys::ImDrawCmd],
        callback: impl FnOnce(&DrawData) -> R,
    ) -> R {
        let mut indices: [DrawIdx; 3] = [0, 1, 2];
        let mut vertices = [sys::ImDrawVert::default(); 3];
        let mut list = sys::ImDrawList::default();
        list.CmdBuffer.Size = i32::try_from(commands.len()).unwrap();
        list.CmdBuffer.Capacity = list.CmdBuffer.Size;
        list.CmdBuffer.Data = commands.as_mut_ptr();
        list.IdxBuffer.Size = 3;
        list.IdxBuffer.Capacity = 3;
        list.IdxBuffer.Data = indices.as_mut_ptr();
        list.VtxBuffer.Size = 3;
        list.VtxBuffer.Capacity = 3;
        list.VtxBuffer.Data = vertices.as_mut_ptr();
        let mut lists = [&mut list as *mut sys::ImDrawList];
        let raw = sys::ImDrawData {
            Valid: true,
            TotalIdxCount: 3,
            TotalVtxCount: 3,
            CmdLists: sys::ImVector_ImDrawListPtr {
                Size: 1,
                Capacity: 1,
                Data: lists.as_mut_ptr(),
            },
            DisplaySize: sys::ImVec2 { x: 64.0, y: 64.0 },
            FramebufferScale: sys::ImVec2 { x: 1.0, y: 1.0 },
            ..Default::default()
        };
        // SAFETY: every native vector borrows live, initialized arrays for the closure duration.
        // DrawData has the same layout as ImDrawData, and no native destructor owns these arrays.
        callback(unsafe { DrawData::from_raw(&raw) })
    }

    #[test]
    fn managed_uploads_reuse_completed_versions_with_older_commands_still_pending()
    -> RendererResult<()> {
        use dear_imgui_rs::{
            Color as ImGuiColor,
            texture::{
                OwnedTextureData, TextureFormat as ImGuiTextureFormat, TextureRegion,
                TextureSubresource,
            },
        };
        use wgpu::*;

        let (device, queue) =
            request_test_device().expect("managed upload test requires an adapter");
        let mut context = Context::create();
        let mut renderer = WgpuRenderer::new(
            WgpuInitInfo::new(device.clone(), queue.clone(), TextureFormat::Rgba8Unorm),
            &mut context,
        )?;
        renderer.set_gamma_mode(crate::GammaMode::Linear);
        context.io_mut().set_display_size([16.0, 16.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);

        let red = [255, 0, 0, 255];
        let green = [0, 255, 0, 255];
        let blue = [0, 0, 255, 255];
        let texture = context.register_texture(
            OwnedTextureData::from_pixels(ImGuiTextureFormat::RGBA32, 2, 2, &red.repeat(4))
                .unwrap(),
        );
        let target_size = Extent3d {
            width: 16,
            height: 16,
            depth_or_array_layers: 1,
        };
        let target = device.create_texture(&TextureDescriptor {
            label: None,
            size: target_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let target_view = target.create_view(&TextureViewDescriptor::default());
        let readback = device.create_buffer(&BufferDescriptor {
            label: None,
            size: u64::from(COPY_BYTES_PER_ROW_ALIGNMENT) * u64::from(target_size.height),
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let record = |context: &mut Context, renderer: &mut WgpuRenderer| {
            let draw = context.frame().get_foreground_draw_list();
            for y in 0..2 {
                for x in 0..2 {
                    let pos = [x as f32 * 8.0, y as f32 * 8.0];
                    let uv = [x as f32 * 0.5 + 0.25, y as f32 * 0.5 + 0.25];
                    draw.add_image(
                        texture,
                        pos,
                        [pos[0] + 8.0, pos[1] + 8.0],
                        uv,
                        uv,
                        ImGuiColor::WHITE,
                    );
                }
            }
            drop(draw);
            let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor::default());
            {
                let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &target_view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: Operations {
                            load: LoadOp::Clear(Color::TRANSPARENT),
                            store: StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });
                let pending = context.try_render(renderer.renderer_consumer()?)?;
                renderer.render(pending, &mut pass, FramebufferExtent::new(16, 16))?;
            }
            encoder.copy_texture_to_buffer(
                TexelCopyTextureInfo {
                    texture: &target,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(COPY_BYTES_PER_ROW_ALIGNMENT),
                        rows_per_image: None,
                    },
                },
                target_size,
            );
            Ok::<_, RendererError>(encoder.finish())
        };
        let patch = |context: &mut Context, top: [u8; 4], bottom: [u8; 4]| {
            let rows = [top, bottom];
            context
                .try_with_texture_mut(texture, |mut texture| {
                    texture.update_subresource(TextureSubresource::new(
                        TextureRegion::new(1, 0, 1, 2).unwrap(),
                        4,
                        rows.as_flattened(),
                    ))
                })
                .unwrap();
        };
        let wait = || {
            device
                .poll(PollType::Wait {
                    submission_index: None,
                    timeout: Some(std::time::Duration::from_secs(10)),
                })
                .unwrap();
        };
        let read_pixels = || {
            let (sender, receiver) = std::sync::mpsc::sync_channel(1);
            readback
                .slice(..)
                .map_async(MapMode::Read, move |result| sender.send(result).unwrap());
            wait();
            receiver
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap()
                .unwrap();
            let bytes = readback.slice(..).get_mapped_range().unwrap();
            let pixels = [(4, 4), (12, 4), (4, 12), (12, 12)].map(|(x, y)| {
                let start = y * COPY_BYTES_PER_ROW_ALIGNMENT as usize + x * 4;
                <[u8; 4]>::try_from(&bytes[start..start + 4]).unwrap()
            });
            drop(bytes);
            readback.unmap();
            pixels
        };

        let older = record(&mut context, &mut renderer)?;
        let texture_id = context
            .with_texture(texture, |texture| texture.texture_id())
            .unwrap();
        let original_view = renderer
            .gpu
            .texture_manager
            .texture_view(texture_id)
            .unwrap()
            .clone();
        patch(&mut context, blue, green);
        queue.submit([record(&mut context, &mut renderer)?]);
        wait();
        let current_view = renderer
            .gpu
            .texture_manager
            .texture_view(texture_id)
            .unwrap()
            .clone();
        assert_ne!(current_view, original_view);
        let binding = renderer
            .gpu
            .backend_data
            .as_ref()
            .unwrap()
            .render_resources
            .image_bind_groups[&texture_id]
            .clone();

        drop(record(&mut context, &mut renderer)?);
        patch(&mut context, green, blue);
        queue.submit([record(&mut context, &mut renderer)?]);
        assert_eq!(
            renderer.gpu.texture_manager.texture_view(texture_id),
            Some(&current_view)
        );
        assert_eq!(
            renderer
                .gpu
                .backend_data
                .as_ref()
                .unwrap()
                .render_resources
                .image_bind_groups
                .get(&texture_id),
            Some(&binding)
        );
        assert_eq!(read_pixels(), [red, green, red, blue]);

        queue.submit([older]);
        assert_eq!(read_pixels(), [red; 4]);
        renderer.shutdown(&mut context)
    }

    #[test]
    fn prepared_dependencies_include_only_unique_drawable_external_bindings() -> RendererResult<()>
    {
        let Some((device, queue)) = request_test_device() else {
            return Ok(());
        };
        let (_texture, view) = external_test_texture(&device);
        let mut manager = WgpuTextureManager::new();
        let first = manager.register_external_view(&view)?;
        let clipped = manager.register_external_view(&view)?;
        let zero_count = manager.register_external_view(&view)?;
        let second = manager.register_external_view(&view)?;
        let mut backend = WgpuBackendData::new(WgpuInitInfo::new(
            device.clone(),
            queue,
            wgpu::TextureFormat::Rgba8Unorm,
        ));
        backend.render_resources.initialize(&device)?;
        let visible = [0.0, 0.0, 32.0, 32.0];
        let mut commands = [
            element_command(second.texture_id(), 3, visible),
            element_command(clipped.texture_id(), 3, [80.0, 80.0, 96.0, 96.0]),
            element_command(zero_count.texture_id(), 0, visible),
            element_command(first.texture_id(), 3, visible),
            element_command(first.texture_id(), 3, visible),
        ];
        let captured = with_draw_commands(&mut commands, |draw_data| {
            let prepared = WgpuRenderer::prepare_draw_data(
                &manager,
                &None,
                draw_data,
                FramebufferExtent::new(64, 64),
                &mut backend,
            )?;
            assert_eq!(prepared.commands.len(), 3);
            Ok::<_, RendererError>(prepared.dependencies().clone())
        })?;
        assert_eq!(
            captured.external_bindings,
            [
                manager.external_binding(second.texture_id()).unwrap(),
                manager.external_binding(first.texture_id()).unwrap(),
            ]
        );
        assert!(!captured.contains_unknown_callback_dependencies);
        assert!(
            !backend
                .render_resources
                .image_bind_groups
                .contains_key(&clipped.texture_id())
        );
        assert!(
            !backend
                .render_resources
                .image_bind_groups
                .contains_key(&zero_count.texture_id())
        );

        manager.update_external_view(first, &view)?;
        assert_eq!(captured.external_bindings[1].revision.get(), 1);
        assert_eq!(
            manager
                .external_binding(first.texture_id())
                .unwrap()
                .revision
                .get(),
            2
        );
        Ok(())
    }

    #[test]
    fn visible_unregistered_texture_rejects_draw_preparation() -> RendererResult<()> {
        let Some((device, queue)) = request_test_device() else {
            return Ok(());
        };
        let manager = WgpuTextureManager::new();
        let invalid = TextureId::new(u64::MAX);
        let mut backend = WgpuBackendData::new(WgpuInitInfo::new(
            device,
            queue,
            wgpu::TextureFormat::Rgba8Unorm,
        ));
        let mut commands = [element_command(invalid, 3, [0.0, 0.0, 32.0, 32.0])];
        with_draw_commands(&mut commands, |draw_data| {
            assert!(matches!(
                WgpuRenderer::prepare_draw_data(
                    &manager, &None, draw_data, FramebufferExtent::new(64, 64), &mut backend,
                ),
                Err(RendererError::InvalidTextureId(id)) if id == invalid
            ));
        });
        assert!(backend.render_resources.image_bind_groups.is_empty());
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn render_callback_frame<'ctx>(
        context: &'ctx mut Context,
        consumer: &dear_imgui_rs::render::SynchronousRendererConsumer,
        draw: impl FnOnce(&dear_imgui_rs::DrawListMut<'_>),
    ) -> dear_imgui_rs::render::ReconciledFrame<'ctx> {
        context.prepare_frame(
            dear_imgui_rs::FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0)
                .renderer_has_textures(),
        );
        let frame = context.begin_frame();
        draw(&frame.ui().get_background_draw_list());
        let pending = frame.render(consumer);
        let feedback: Vec<_> = pending
            .texture_requests()
            .iter()
            .map(|r| r.retry())
            .collect();
        pending.reconcile_texture_feedback(feedback).unwrap()
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn only_non_standard_callbacks_make_draw_dependencies_incomplete() -> RendererResult<()> {
        let Some((device, queue)) = request_test_device() else {
            return Ok(());
        };
        let mut context = Context::create();
        let platform_io = context.platform_io_mut();
        let reset = super::super::callbacks::draw_callback_reset_render_state;
        let linear = super::super::callbacks::draw_callback_set_sampler_linear;
        let nearest = super::super::callbacks::draw_callback_set_sampler_nearest;
        // SAFETY: the callbacks have the backend ABI and remain live for both preparations.
        unsafe {
            platform_io.set_draw_callback_reset_render_state_raw(Some(reset));
            platform_io.set_draw_callback_set_sampler_linear_raw(Some(linear));
            platform_io.set_draw_callback_set_sampler_nearest_raw(Some(nearest));
        }
        unsafe extern "C" fn unknown(
            _list: *const sys::ImDrawList,
            _command: *const sys::ImDrawCmd,
        ) {
        }
        let consumer = context.create_synchronous_renderer_consumer().unwrap();
        let manager = WgpuTextureManager::new();
        let mut backend = WgpuBackendData::new(WgpuInitInfo::new(
            device,
            queue,
            wgpu::TextureFormat::Rgba8Unorm,
        ));
        for (count, expected_unknown) in [(3, false), (4, true)] {
            let frame = render_callback_frame(&mut context, &consumer, |list| {
                // Both native callbacks are static and do not read user data.
                unsafe { list.add_callback(reset, std::ptr::null_mut(), 0) };
                list.set_sampler_linear();
                list.set_sampler_nearest();
                if expected_unknown {
                    unsafe { list.add_callback(unknown, std::ptr::null_mut(), 0) };
                }
            });
            let prepared = WgpuRenderer::prepare_draw_data(
                &manager,
                &None,
                frame.draw_data(),
                FramebufferExtent::new(64, 64),
                &mut backend,
            )?;
            assert!(prepared.dependencies().external_bindings.is_empty());
            assert_eq!(prepared.commands.len(), count);
            assert_eq!(
                prepared
                    .dependencies()
                    .contains_unknown_callback_dependencies,
                expected_unknown
            );
        }
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn texture_owned_sampler_is_rejected_during_preparation() -> RendererResult<()> {
        let Some((device, queue)) = request_test_device() else {
            return Ok(());
        };
        let mut context = Context::create();
        unsafe extern "C" fn texture_sampler(
            _list: *const sys::ImDrawList,
            _command: *const sys::ImDrawCmd,
        ) {
        }
        // SAFETY: the callback has the native ABI and remains live throughout preparation.
        unsafe {
            context
                .platform_io_mut()
                .set_draw_callback_set_sampler_from_texture_raw(Some(texture_sampler));
        }
        let consumer = context.create_synchronous_renderer_consumer().unwrap();
        let frame = render_callback_frame(&mut context, &consumer, |list| {
            list.set_sampler_from_texture()
        });
        let manager = WgpuTextureManager::new();
        let mut backend = WgpuBackendData::new(WgpuInitInfo::new(
            device,
            queue,
            wgpu::TextureFormat::Rgba8Unorm,
        ));
        assert!(matches!(
            WgpuRenderer::prepare_draw_data(
                &manager,
                &None,
                frame.draw_data(),
                FramebufferExtent::new(64, 64),
                &mut backend,
            ),
            Err(RendererError::TextureSamplerUnsupported)
        ));
        Ok(())
    }

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
    fn scissor_projection_rounds_outward_before_clamping() {
        let extent = FramebufferExtent {
            width: 64,
            height: 64,
        };
        assert_eq!(
            project_scissor_rect([-8.0, -4.0, 72.0, 68.0], [0.0, 0.0], [1.0, 1.0], extent,)
                .unwrap(),
            Some([0, 0, 64, 64])
        );
        assert_eq!(
            project_scissor_rect([10.25, 20.5, 10.75, 20.75], [10.0, 20.0], [2.0; 2], extent)
                .unwrap(),
            Some([0, 1, 2, 1]),
        );
        assert_eq!(
            project_scissor_rect([1.0, 2.0, 1.0, 3.0], [0.0; 2], [1.0; 2], extent).unwrap(),
            None,
        );
    }
}

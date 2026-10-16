// Renderer draw helpers: preflight, resource preparation, and one command executor.

use std::{collections::HashSet, ops::Range};

use super::*;
use crate::{FrameResources, WgpuDrawDependencies, wgpu};
use dear_imgui_rs::{
    TextureId,
    render::{DrawData, DrawIdx, DrawRequirements, RawCallbackCommand},
};

// ImGui index type is currently u16 in dear-imgui-rs, but keep this derived so
// future upgrades to u32 require fewer backend changes.
const IMGUI_INDEX_FORMAT: wgpu::IndexFormat = if std::mem::size_of::<DrawIdx>() == 2 {
    wgpu::IndexFormat::Uint16
} else {
    wgpu::IndexFormat::Uint32
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
    linear_common_bind_group: wgpu::BindGroup,
    nearest_common_bind_group: wgpu::BindGroup,
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
        draw_data: &DrawData,
        frame_resources: &mut FrameResources,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
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

        if total_vtx_count != 0 {
            frame_resources.ensure_vertex_buffer_capacity(device, total_vtx_count)?;
            frame_resources.upload_vertex_data(queue, &vertices)?;
        }
        if total_idx_count != 0 {
            frame_resources.ensure_index_buffer_capacity(device, total_idx_count)?;
            frame_resources.upload_index_data(queue, &indices)?;
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
        Self::preflight_draw_callback_support(draw_data.requirements())?;

        let mut commands = Vec::new();
        let mut global_idx_offset = 0u32;
        let mut global_vtx_offset = 0i32;
        let clip_off = draw_data.display_pos();
        let clip_scale = draw_data.framebuffer_scale();
        let mut has_elements = false;
        let mut dependencies = WgpuDrawDependencies::default();
        let mut external_bindings_seen = HashSet::new();

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

                        if let Some(binding) = texture_manager.external_binding(texture_id) {
                            if external_bindings_seen.insert(binding) {
                                dependencies.external_bindings.push(binding);
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
                    dear_imgui_rs::render::DrawCmd::ResetRenderState => {
                        commands.push(PreparedDrawCommand::ResetRenderState);
                    }
                    dear_imgui_rs::render::DrawCmd::SetSamplerLinear => {
                        commands.push(PreparedDrawCommand::SetSampler(PreparedSampler::Linear));
                    }
                    dear_imgui_rs::render::DrawCmd::SetSamplerNearest => {
                        commands.push(PreparedDrawCommand::SetSampler(PreparedSampler::Nearest));
                    }
                    dear_imgui_rs::render::DrawCmd::RawCallback(callback) => {
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
        })
    }

    pub(super) fn prepare_render_state_static(
        draw_data: &DrawData,
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
            linear_common_bind_group: uniform.bind_group().clone(),
            nearest_common_bind_group: frame_resources.nearest_common_bind_group()?.clone(),
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
            RendererRenderStateGuard::<crate::WgpuRenderStateStorage>::preflight(platform_io)
        }
        .map_err(super::map_renderer_render_state_error)?;
        Self::setup_prepared_render_state(render_pass, extent, state);

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
                PreparedDrawCommand::ResetRenderState => {
                    Self::setup_prepared_render_state(render_pass, extent, state)
                }
                PreparedDrawCommand::SetSampler(sampler) => {
                    let bind_group = match sampler {
                        PreparedSampler::Linear => &state.linear_common_bind_group,
                        PreparedSampler::Nearest => &state.nearest_common_bind_group,
                    };
                    render_pass.set_bind_group(0, bind_group, &[]);
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
        let mut raw = sys::ImDrawData::default();
        raw.Valid = true;
        raw.TotalIdxCount = 3;
        raw.TotalVtxCount = 3;
        raw.CmdLists.Size = 1;
        raw.CmdLists.Capacity = 1;
        raw.CmdLists.Data = lists.as_mut_ptr();
        raw.DisplaySize = sys::ImVec2 { x: 64.0, y: 64.0 };
        raw.FramebufferScale = sys::ImVec2 { x: 1.0, y: 1.0 };
        // SAFETY: every native vector borrows live, initialized arrays for the closure duration.
        // DrawData has the same layout as ImDrawData, and no native destructor owns these arrays.
        callback(unsafe { DrawData::from_raw(&raw) })
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
        let mut commands = [
            sys::ImDrawCmd {
                UserCallback: Some(reset),
                ..Default::default()
            },
            sys::ImDrawCmd {
                UserCallback: Some(linear),
                ..Default::default()
            },
            sys::ImDrawCmd {
                UserCallback: Some(nearest),
                ..Default::default()
            },
            sys::ImDrawCmd {
                UserCallback: Some(unknown),
                ..Default::default()
            },
        ];
        let manager = WgpuTextureManager::new();
        let mut backend = WgpuBackendData::new(WgpuInitInfo::new(
            device,
            queue,
            wgpu::TextureFormat::Rgba8Unorm,
        ));
        for (count, expected_unknown) in [(3, false), (4, true)] {
            with_draw_commands(&mut commands[..count], |draw_data| {
                let prepared = WgpuRenderer::prepare_draw_data(
                    &manager,
                    &None,
                    draw_data,
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
                Ok::<_, RendererError>(())
            })?;
        }
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

use dear_imgui_rs::render::{
    DrawCmd, DrawCmdParams, DrawData, DrawIdx, DrawVert, PendingFrame, ReconciledFrame,
    RendererRenderStateGuard,
};
use dear_imgui_rs::sys;
use glow::{Context, HasContext};
use std::mem::size_of;

use super::GlowRenderer;
use super::sampler::{SamplerFilter, TextureFilterGuard};
use crate::{
    draw_indices_as_bytes, draw_verts_as_bytes,
    error::{RenderError, RenderResult},
    state::{
        FramebufferSrgbScope, GlStateGuard, GlowRenderStateStorage, GlowSamplerStrategy,
        map_renderer_render_state_error,
    },
    texture::TextureMap,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SamplerBinding {
    TextureOwned,
    Linear,
    Nearest,
    CallbackOwned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FramebufferExtent {
    width: i32,
    height: i32,
}

struct DrawListRenderScope<'draw> {
    gl: &'draw Context,
    texture_map: &'draw dyn TextureMap,
    display_pos: [f32; 2],
    framebuffer_scale: [f32; 2],
    extent: FramebufferExtent,
    vertices: &'draw [DrawVert],
    indices: &'draw [DrawIdx],
}

impl FramebufferExtent {
    fn from_draw_data(draw_data: &DrawData) -> RenderResult<Option<Self>> {
        let display_pos = draw_data.display_pos();
        let display_size = draw_data.display_size();
        let framebuffer_scale = draw_data.framebuffer_scale();
        for (field, value) in [
            ("DisplayPos.x", display_pos[0]),
            ("DisplayPos.y", display_pos[1]),
            ("DisplaySize.x", display_size[0]),
            ("DisplaySize.y", display_size[1]),
            ("FramebufferScale.x", framebuffer_scale[0]),
            ("FramebufferScale.y", framebuffer_scale[1]),
        ] {
            if !value.is_finite() {
                return Err(RenderError::NonFiniteDrawValue { field, value });
            }
        }

        let width = f64::from(display_size[0]) * f64::from(framebuffer_scale[0]);
        let height = f64::from(display_size[1]) * f64::from(framebuffer_scale[1]);
        if width <= 0.0 || height <= 0.0 {
            return Ok(None);
        }
        for (dimension, value) in [("width", width), ("height", height)] {
            if value > f64::from(i32::MAX) {
                return Err(RenderError::FramebufferDimensionOutOfRange { dimension, value });
            }
        }

        let extent = Self {
            width: width as i32,
            height: height as i32,
        };
        if extent.width == 0 || extent.height == 0 {
            return Ok(None);
        }

        Ok(Some(extent))
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
    display_pos: [f32; 2],
    framebuffer_scale: [f32; 2],
    extent: FramebufferExtent,
) -> RenderResult<Option<[i32; 4]>> {
    let transformed = [
        (clip_rect[0] - display_pos[0]) * framebuffer_scale[0],
        (clip_rect[1] - display_pos[1]) * framebuffer_scale[1],
        (clip_rect[2] - display_pos[0]) * framebuffer_scale[0],
        (clip_rect[3] - display_pos[1]) * framebuffer_scale[1],
    ];
    if transformed.iter().any(|value| !value.is_finite()) {
        return Err(RenderError::NonFiniteClipRect(clip_rect));
    }

    let clip_min_x = transformed[0].max(0.0);
    let clip_min_y = transformed[1].max(0.0);
    let clip_max_x = transformed[2].min(extent.width_f32());
    let clip_max_y = transformed[3].min(extent.height_f32());
    if clip_max_x <= clip_min_x || clip_max_y <= clip_min_y {
        return Ok(None);
    }

    let width = (clip_max_x - clip_min_x) as i32;
    let height = (clip_max_y - clip_min_y) as i32;
    if width == 0 || height == 0 {
        return Ok(None);
    }
    Ok(Some([
        clip_min_x as i32,
        extent.height - clip_max_y as i32,
        width,
        height,
    ]))
}

impl SamplerBinding {
    fn reset(has_sampler_objects: bool) -> Self {
        if has_sampler_objects {
            Self::Linear
        } else {
            Self::TextureOwned
        }
    }

    fn fallback_filter(self) -> Option<SamplerFilter> {
        match self {
            Self::Linear => Some(SamplerFilter::Linear),
            Self::Nearest => Some(SamplerFilter::Nearest),
            Self::TextureOwned | Self::CallbackOwned => None,
        }
    }

    fn explicit(filter: SamplerFilter) -> Self {
        match filter {
            SamplerFilter::Linear => Self::Linear,
            SamplerFilter::Nearest => Self::Nearest,
        }
    }

    fn after_raw_callback(self, has_sampler_objects: bool) -> Self {
        if has_sampler_objects {
            Self::CallbackOwned
        } else {
            self
        }
    }
}

fn platform_io_for_current_context() -> RenderResult<*mut sys::ImGuiPlatformIO> {
    let context = unsafe { sys::igGetCurrentContext() };
    let platform_io = unsafe { sys::igGetPlatformIO_ContextPtr(context) };
    if platform_io.is_null() {
        Err(RenderError::MissingPlatformIo)
    } else {
        Ok(platform_io)
    }
}

struct VertexArrayGuard<'a> {
    gl: &'a Context,
    vertex_array: crate::GlVertexArray,
}

impl<'a> VertexArrayGuard<'a> {
    fn create_and_bind(gl: &'a Context) -> RenderResult<Self> {
        let vertex_array =
            unsafe { gl.create_vertex_array() }.map_err(|error| RenderError::CreateResource {
                resource: "vertex array object",
                error,
            })?;
        unsafe { gl.bind_vertex_array(Some(vertex_array)) };
        Ok(Self { gl, vertex_array })
    }

    fn bind(&self) {
        unsafe { self.gl.bind_vertex_array(Some(self.vertex_array)) };
    }
}

impl Drop for VertexArrayGuard<'_> {
    fn drop(&mut self) {
        unsafe { self.gl.delete_vertex_array(self.vertex_array) };
    }
}

fn clear_viewport_framebuffer(gl: &Context, color: [f32; 4]) {
    unsafe {
        gl.disable(glow::SCISSOR_TEST);
        gl.color_mask(true, true, true, true);
        gl.clear_color(color[0], color[1], color[2], color[3]);
        gl.clear(glow::COLOR_BUFFER_BIT);
    }
}

impl GlowRenderer {
    /// Finalizes and renders a frame for this renderer's bound Dear ImGui Context.
    pub fn render_context(&mut self, context: &mut dear_imgui_rs::Context) -> RenderResult<()> {
        let frame = self.reconcile_context_frame(context)?;
        self.render_reconciled(frame).map(drop)
    }

    pub(super) fn reconcile_context_frame<'context>(
        &mut self,
        context: &'context mut dear_imgui_rs::Context,
    ) -> RenderResult<ReconciledFrame<'context>> {
        self.ensure_context_matches(context)?;
        let frame = context.try_render(self.renderer_consumer()?)?;
        self.reconcile_frame(frame)
    }

    /// Consume and render one Context-borrowed Dear ImGui frame.
    pub fn render(&mut self, frame: PendingFrame<'_>) -> RenderResult<()> {
        let frame = self.reconcile_frame(frame)?;
        self.render_reconciled(frame).map(drop)
    }

    /// Reconciles managed textures without reading or drawing the frame's commands.
    pub fn reconcile_frame<'frame>(
        &mut self,
        frame: PendingFrame<'frame>,
    ) -> RenderResult<ReconciledFrame<'frame>> {
        self.ensure_operational()?;
        self.validate_pending_frame(&frame)?;
        if self.is_destroyed {
            return Err(RenderError::RendererDestroyed);
        }
        let gl = self
            .gl_context
            .clone()
            .ok_or(RenderError::MissingGlContext)?;
        self.reconcile_pending_frame(&gl, frame)
    }

    /// Draws one already-reconciled frame and returns its linear presentation capability.
    pub fn render_reconciled<'frame>(
        &mut self,
        frame: ReconciledFrame<'frame>,
    ) -> RenderResult<ReconciledFrame<'frame>> {
        self.ensure_operational()?;
        self.validate_reconciled_frame(&frame)?;
        if self.is_destroyed {
            return Err(RenderError::RendererDestroyed);
        }
        let gl = self
            .gl_context
            .clone()
            .ok_or(RenderError::MissingGlContext)?;
        self.render_reconciled_frame(&gl, frame)
    }

    /// Consume and render a frame using an externally managed OpenGL context.
    pub fn render_with_context(
        &mut self,
        gl: &Context,
        frame: PendingFrame<'_>,
    ) -> RenderResult<()> {
        let frame = self.reconcile_frame_with_context(gl, frame)?;
        self.render_with_context_reconciled(gl, frame).map(drop)
    }

    /// Reconciles managed textures with an externally managed OpenGL context.
    pub fn reconcile_frame_with_context<'frame>(
        &mut self,
        gl: &Context,
        frame: PendingFrame<'frame>,
    ) -> RenderResult<ReconciledFrame<'frame>> {
        self.ensure_operational()?;
        self.validate_pending_frame(&frame)?;
        if self.is_destroyed {
            return Err(RenderError::RendererDestroyed);
        }
        self.reconcile_pending_frame(gl, frame)
    }

    /// Draws one reconciled frame with an externally managed OpenGL context.
    pub fn render_with_context_reconciled<'frame>(
        &mut self,
        gl: &Context,
        frame: ReconciledFrame<'frame>,
    ) -> RenderResult<ReconciledFrame<'frame>> {
        self.ensure_operational()?;
        self.validate_reconciled_frame(&frame)?;
        if self.is_destroyed {
            return Err(RenderError::RendererDestroyed);
        }
        self.render_reconciled_frame(gl, frame)
    }

    fn reconcile_pending_frame<'frame>(
        &mut self,
        gl: &Context,
        frame: PendingFrame<'frame>,
    ) -> RenderResult<ReconciledFrame<'frame>> {
        let request_epoch = frame.epoch().sequence();
        let feedback =
            self.process_texture_requests(gl, frame.texture_requests(), request_epoch)?;
        let reconciled = frame.reconcile_texture_feedback(feedback)?;
        self.prune_destroyed_managed_textures(reconciled.completion_progress().watermark());
        Ok(reconciled)
    }

    fn render_reconciled_frame<'frame>(
        &mut self,
        gl: &Context,
        reconciled: ReconciledFrame<'frame>,
    ) -> RenderResult<ReconciledFrame<'frame>> {
        self.render_draw_data(gl, reconciled.draw_data())?;
        Ok(reconciled)
    }

    fn validate_pending_frame(&self, frame: &PendingFrame<'_>) -> RenderResult<()> {
        let consumer = self.renderer_consumer()?;
        if frame.context_id() != consumer.context_id() {
            return Err(RenderError::ContextMismatch {
                expected: consumer.context_id(),
                actual: frame.context_id(),
            });
        }
        let epoch = frame.epoch();
        if epoch.consumer_generation() != consumer.generation() {
            return Err(RenderError::ConsumerGenerationMismatch {
                expected: consumer.generation(),
                actual: epoch.consumer_generation(),
            });
        }
        Ok(())
    }

    fn validate_reconciled_frame(&self, frame: &ReconciledFrame<'_>) -> RenderResult<()> {
        let consumer = self.renderer_consumer()?;
        if frame.context_id() != consumer.context_id() {
            return Err(RenderError::ContextMismatch {
                expected: consumer.context_id(),
                actual: frame.context_id(),
            });
        }
        let epoch = frame.epoch().ok_or(RenderError::ManagedFrameRequired)?;
        if epoch.consumer_generation() != consumer.generation() {
            return Err(RenderError::ConsumerGenerationMismatch {
                expected: consumer.generation(),
                actual: epoch.consumer_generation(),
            });
        }
        Ok(())
    }

    /// Draw already-reconciled data. Multi-viewport callbacks use this for secondary viewports.
    pub(super) fn render_draw_data(
        &mut self,
        gl: &Context,
        draw_data: &DrawData,
    ) -> RenderResult<()> {
        let binding = self
            .context_binding
            .clone()
            .ok_or(RenderError::RendererNotAttached)?;
        binding
            .try_with_bound_context(|| {
                self.render_draw_data_transaction(gl, Some(draw_data), false)
            })
            .map_err(RenderError::ContextBinding)?
    }

    #[cfg(feature = "multi-viewport")]
    pub(super) fn render_viewport_draw_data(
        &mut self,
        gl: &Context,
        draw_data: Option<&DrawData>,
        clear: bool,
    ) -> RenderResult<()> {
        self.ensure_operational()?;
        let binding = self
            .context_binding
            .clone()
            .ok_or(RenderError::RendererNotAttached)?;
        binding
            .try_with_bound_context(|| self.render_draw_data_transaction(gl, draw_data, clear))
            .map_err(RenderError::ContextBinding)?
    }

    fn render_draw_data_transaction(
        &mut self,
        gl: &Context,
        draw_data: Option<&DrawData>,
        clear: bool,
    ) -> RenderResult<()> {
        if self.is_destroyed {
            return Err(RenderError::RendererDestroyed);
        }

        let framebuffer_extent = draw_data
            .map(FramebufferExtent::from_draw_data)
            .transpose()?
            .flatten();
        let drawable = framebuffer_extent.is_some();
        if !clear && !drawable {
            return Ok(());
        }

        let platform_io = drawable.then(platform_io_for_current_context).transpose()?;
        if let Some(platform_io) = platform_io {
            unsafe {
                RendererRenderStateGuard::<GlowRenderStateStorage<'_>>::preflight(platform_io)
            }
            .map_err(map_renderer_render_state_error)?;
        }

        let _gl_state = GlStateGuard::capture(
            gl,
            self.gl_version,
            self.has_separate_polygon_modes,
            self.has_sampler_object_support,
        );
        let framebuffer_srgb = self
            .supports_framebuffer_srgb_control()
            .then(|| FramebufferSrgbScope::enter(gl, self.framebuffer_srgb));

        if clear {
            clear_viewport_framebuffer(gl, self.viewport_clear_color);
        }

        let Some(draw_data) = draw_data.filter(|_| drawable) else {
            return Ok(());
        };
        let extent = framebuffer_extent.expect("drawable data has a framebuffer extent");

        let vertex_array = VertexArrayGuard::create_and_bind(gl)?;

        let sampler_strategy = if self.samplers.is_some() {
            GlowSamplerStrategy::SamplerObjects
        } else {
            GlowSamplerStrategy::TextureParameters
        };
        let mut callback_state = GlowRenderStateStorage::new(gl, sampler_strategy);
        let render_state = unsafe {
            RendererRenderStateGuard::install(
                platform_io.expect("drawable data has PlatformIO"),
                &mut callback_state,
            )
        }
        .map_err(map_renderer_render_state_error)?;
        self.set_up_render_state(
            gl,
            draw_data,
            extent,
            &vertex_array,
            framebuffer_srgb.as_ref(),
        )?;
        let texture_map = self.texture_map_for_draw();
        self.render_draw_lists(
            gl,
            texture_map,
            draw_data,
            extent,
            &vertex_array,
            framebuffer_srgb.as_ref(),
            &render_state,
        )?;
        render_state
            .finish()
            .map_err(map_renderer_render_state_error)?;
        Ok(())
    }

    fn texture_map_for_draw(&self) -> &dyn TextureMap {
        self.texture_map
            .as_deref()
            .expect("GlowRenderer texture_map missing (internal invariant)")
    }

    /// Set up OpenGL render state for ImGui rendering
    fn set_up_render_state(
        &self,
        gl: &Context,
        draw_data: &DrawData,
        extent: FramebufferExtent,
        vertex_array: &VertexArrayGuard<'_>,
        framebuffer_srgb: Option<&FramebufferSrgbScope<'_>>,
    ) -> RenderResult<()> {
        vertex_array.bind();
        if let Some(framebuffer_srgb) = framebuffer_srgb {
            framebuffer_srgb.reapply();
        }
        unsafe {
            // Ensure sampler uses texture unit 0 (shader binds sampler to 0)
            gl.active_texture(glow::TEXTURE0);
            // Setup render state: alpha-blending enabled, no face culling, no depth testing, scissor enabled, polygon fill
            gl.enable(glow::BLEND);
            gl.blend_equation(glow::FUNC_ADD);
            gl.blend_func_separate(
                glow::SRC_ALPHA,
                glow::ONE_MINUS_SRC_ALPHA,
                glow::ONE,
                glow::ONE_MINUS_SRC_ALPHA,
            );
            gl.disable(glow::CULL_FACE);
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::STENCIL_TEST);
            gl.enable(glow::SCISSOR_TEST);

            if self.gl_version.supports_polygon_mode() {
                gl.polygon_mode(glow::FRONT_AND_BACK, glow::FILL);
            }

            if self.gl_version.supports_primitive_restart() {
                gl.disable(glow::PRIMITIVE_RESTART);
            }

            // Setup viewport, orthographic projection matrix
            gl.viewport(0, 0, extent.width, extent.height);

            // Calculate projection matrix like the original implementation
            let display_pos = draw_data.display_pos();
            let display_size = draw_data.display_size();
            let l = display_pos[0];
            let r = display_pos[0] + display_size[0];
            let t = display_pos[1];
            let b = display_pos[1] + display_size[1];

            // Support for GL 4.5 rarely used glClipControl(GL_UPPER_LEFT)
            let (t, b) = if self.has_clip_origin_support {
                // Check current clip origin
                let clip_origin = gl.get_parameter_i32(glow::CLIP_ORIGIN);
                if clip_origin == glow::UPPER_LEFT as i32 {
                    (b, t) // Swap top and bottom if origin is upper left
                } else {
                    (t, b)
                }
            } else {
                (t, b)
            };

            let ortho_projection = [
                [2.0 / (r - l), 0.0, 0.0, 0.0],
                [0.0, 2.0 / (t - b), 0.0, 0.0],
                [0.0, 0.0, -1.0, 0.0],
                [(r + l) / (l - r), (t + b) / (b - t), 0.0, 1.0],
            ];

            gl.use_program(self.shaders.program);
            if let Some(location) = self.shaders.attrib_location_tex.as_ref() {
                gl.uniform_1_i32(Some(location), 0);
            }
            if let Some(location) = self.shaders.attrib_location_proj_mtx.as_ref() {
                gl.uniform_matrix_4_f32_slice(Some(location), false, &ortho_projection.concat());
            }
            if let Some(location) = self.shaders.attrib_location_color_gamma.as_ref() {
                // Decode vertex color from sRGB when writing to sRGB framebuffer,
                // otherwise pass-through (1.0). Allow override if set.
                let gamma = self
                    .color_gamma_override
                    .unwrap_or(if self.framebuffer_srgb {
                        2.2_f32
                    } else {
                        1.0_f32
                    });
                gl.uniform_1_f32(Some(location), gamma);
            }

            if let Some(samplers) = &self.samplers {
                samplers.bind(gl, SamplerFilter::Linear);
            }

            // Bind vertex/index buffers and setup attributes for ImDrawVert
            gl.bind_buffer(glow::ARRAY_BUFFER, self.vbo_handle);
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, self.ebo_handle);
            gl.enable_vertex_attrib_array(self.shaders.attrib_location_vtx_pos);
            gl.enable_vertex_attrib_array(self.shaders.attrib_location_vtx_uv);
            gl.enable_vertex_attrib_array(self.shaders.attrib_location_vtx_color);

            let pos_offset = std::mem::offset_of!(DrawVert, pos) as i32;
            let uv_offset = std::mem::offset_of!(DrawVert, uv) as i32;
            let color_offset = std::mem::offset_of!(DrawVert, col) as i32;

            gl.vertex_attrib_pointer_f32(
                self.shaders.attrib_location_vtx_pos,
                2,
                glow::FLOAT,
                false,
                size_of::<DrawVert>() as i32,
                pos_offset,
            );
            gl.vertex_attrib_pointer_f32(
                self.shaders.attrib_location_vtx_uv,
                2,
                glow::FLOAT,
                false,
                size_of::<DrawVert>() as i32,
                uv_offset,
            );
            // Color attribute - our DrawVert uses u32 packed color, so we need to handle it as 4 bytes
            // The u32 is stored as RGBA in little-endian format, so we can treat it as 4 unsigned bytes
            gl.vertex_attrib_pointer_f32(
                self.shaders.attrib_location_vtx_color,
                4,
                glow::UNSIGNED_BYTE,
                true, // normalized = true, converts [0,255] to [0.0,1.0]
                size_of::<DrawVert>() as i32,
                color_offset,
            );
        }

        Ok(())
    }

    /// Render all draw lists
    fn render_draw_lists(
        &self,
        gl: &Context,
        texture_map: &dyn TextureMap,
        draw_data: &DrawData,
        extent: FramebufferExtent,
        vertex_array: &VertexArrayGuard<'_>,
        framebuffer_srgb: Option<&FramebufferSrgbScope<'_>>,
        render_state: &RendererRenderStateGuard<'_, GlowRenderStateStorage<'_>>,
    ) -> RenderResult<()> {
        let mut sampler_binding = SamplerBinding::reset(self.samplers.is_some());

        for draw_list in draw_data.draw_lists() {
            // Upload vertex/index buffers
            self.upload_vertex_buffer(gl, draw_list.vtx_buffer())?;
            self.upload_index_buffer(gl, draw_list.idx_buffer())?;

            let scope = DrawListRenderScope {
                gl,
                texture_map,
                display_pos: draw_data.display_pos(),
                framebuffer_scale: draw_data.framebuffer_scale(),
                extent,
                vertices: draw_list.vtx_buffer(),
                indices: draw_list.idx_buffer(),
            };

            for command in draw_list.commands() {
                match command {
                    DrawCmd::Elements { count, cmd_params } => {
                        self.render_elements(&scope, count, &cmd_params, sampler_binding)?;
                    }
                    DrawCmd::ResetRenderState => {
                        self.set_up_render_state(
                            gl,
                            draw_data,
                            extent,
                            vertex_array,
                            framebuffer_srgb,
                        )?;
                        sampler_binding = SamplerBinding::reset(self.samplers.is_some());
                    }
                    DrawCmd::SetSamplerLinear => {
                        if let Some(samplers) = &self.samplers {
                            samplers.bind(gl, SamplerFilter::Linear);
                        }
                        sampler_binding = SamplerBinding::explicit(SamplerFilter::Linear);
                    }
                    DrawCmd::SetSamplerNearest => {
                        if let Some(samplers) = &self.samplers {
                            samplers.bind(gl, SamplerFilter::Nearest);
                        }
                        sampler_binding = SamplerBinding::explicit(SamplerFilter::Nearest);
                    }
                    DrawCmd::RawCallback(callback) => {
                        unsafe { callback.invoke() };
                        render_state
                            .validate()
                            .map_err(map_renderer_render_state_error)?;
                        sampler_binding =
                            sampler_binding.after_raw_callback(self.samplers.is_some());
                    }
                }
            }
        }

        Ok(())
    }

    /// Upload vertex buffer data
    ///
    /// Following the original Dear ImGui OpenGL3 implementation, we always use glBufferData()
    /// instead of glBufferSubData() to avoid issues with Intel GPU drivers.
    /// See: https://github.com/ocornut/imgui/issues/4468
    fn upload_vertex_buffer(&self, gl: &Context, vertices: &[DrawVert]) -> RenderResult<()> {
        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, self.vbo_handle);

            // Always use glBufferData() following the original implementation
            // This avoids corruption issues reported with Intel GPU drivers when using glBufferSubData()
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                draw_verts_as_bytes(vertices),
                glow::STREAM_DRAW,
            );
        }

        Ok(())
    }

    /// Upload index buffer data
    ///
    /// Following the original Dear ImGui OpenGL3 implementation, we always use glBufferData()
    /// instead of glBufferSubData() to avoid issues with Intel GPU drivers.
    /// See: https://github.com/ocornut/imgui/issues/4468
    fn upload_index_buffer(
        &self,
        gl: &Context,
        indices: &[dear_imgui_rs::render::DrawIdx],
    ) -> RenderResult<()> {
        unsafe {
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, self.ebo_handle);

            // Always use glBufferData() following the original implementation
            // This avoids corruption issues reported with Intel GPU drivers when using glBufferSubData()
            gl.buffer_data_u8_slice(
                glow::ELEMENT_ARRAY_BUFFER,
                draw_indices_as_bytes(indices),
                glow::STREAM_DRAW,
            );
        }

        Ok(())
    }

    /// Render elements with the given parameters
    fn render_elements(
        &self,
        scope: &DrawListRenderScope<'_>,
        count: usize,
        cmd_params: &DrawCmdParams,
        sampler_binding: SamplerBinding,
    ) -> RenderResult<()> {
        if count == 0 {
            return Ok(());
        }
        let index_end = cmd_params.idx_offset.checked_add(count).ok_or(
            RenderError::DrawCommandIndexRangeOutOfBounds {
                start: cmd_params.idx_offset,
                end: usize::MAX,
                len: scope.indices.len(),
            },
        )?;
        if index_end > scope.indices.len() {
            return Err(RenderError::DrawCommandIndexRangeOutOfBounds {
                start: cmd_params.idx_offset,
                end: index_end,
                len: scope.indices.len(),
            });
        }
        let max_index = scope.indices[cmd_params.idx_offset..index_end]
            .iter()
            .map(|index| *index as usize)
            .max()
            .expect("a non-empty draw command has at least one index");
        let referenced_vertex = cmd_params.vtx_offset.checked_add(max_index).ok_or(
            RenderError::DrawCommandVertexOutOfBounds {
                index: usize::MAX,
                len: scope.vertices.len(),
            },
        )?;
        if referenced_vertex >= scope.vertices.len() {
            return Err(RenderError::DrawCommandVertexOutOfBounds {
                index: referenced_vertex,
                len: scope.vertices.len(),
            });
        }

        let draw_count =
            i32::try_from(count).map_err(|_| RenderError::DrawParameterOutOfRange {
                field: "element count",
                value: count,
            })?;
        let index_byte_offset = cmd_params
            .idx_offset
            .checked_mul(size_of::<dear_imgui_rs::render::DrawIdx>())
            .ok_or(RenderError::DrawParameterOutOfRange {
                field: "index byte offset",
                value: cmd_params.idx_offset,
            })?;
        let index_byte_offset =
            i32::try_from(index_byte_offset).map_err(|_| RenderError::DrawParameterOutOfRange {
                field: "index byte offset",
                value: index_byte_offset,
            })?;
        let vertex_offset = i32::try_from(cmd_params.vtx_offset).map_err(|_| {
            RenderError::DrawParameterOutOfRange {
                field: "vertex offset",
                value: cmd_params.vtx_offset,
            }
        })?;
        if !self.gl_version.supports_vertex_offset() && vertex_offset != 0 {
            return Err(RenderError::VertexOffsetUnsupported {
                offset: cmd_params.vtx_offset,
            });
        }

        let Some(scissor) = project_scissor_rect(
            cmd_params.clip_rect,
            scope.display_pos,
            scope.framebuffer_scale,
            scope.extent,
        )?
        else {
            return Ok(());
        };
        let texture = scope
            .texture_map
            .get(cmd_params.texture_id)
            .ok_or(RenderError::UnknownTextureId(cmd_params.texture_id))?;

        unsafe {
            // Bind texture
            scope.gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            let _filter_override = if self.samplers.is_none() {
                sampler_binding
                    .fallback_filter()
                    .map(|filter| TextureFilterGuard::override_bound_texture(scope.gl, filter))
            } else {
                None
            };

            scope
                .gl
                .scissor(scissor[0], scissor[1], scissor[2], scissor[3]);

            // Draw - dynamically detect index type like the original implementation
            let index_type = if size_of::<dear_imgui_rs::render::DrawIdx>() == 2 {
                glow::UNSIGNED_SHORT
            } else {
                glow::UNSIGNED_INT
            };

            if self.gl_version.supports_vertex_offset() {
                scope.gl.draw_elements_base_vertex(
                    glow::TRIANGLES,
                    draw_count,
                    index_type,
                    index_byte_offset,
                    vertex_offset,
                );
            } else {
                scope
                    .gl
                    .draw_elements(glow::TRIANGLES, draw_count, index_type, index_byte_offset);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU32, Ordering};

    use dear_imgui_rs::{TextureFormat, TextureId};
    use glow::HasContext;

    use super::{
        FramebufferExtent, GlowRenderer, clear_viewport_framebuffer, project_scissor_rect,
    };
    use super::{SamplerBinding, VertexArrayGuard};
    use crate::renderer::sampler::SamplerFilter;
    use crate::{GlTexture, GlVersion, InitResult, shaders::Shaders, texture::TextureMap};

    static CLEAR_EVENTS: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    static BOUND_VERTEX_ARRAY: AtomicU32 = AtomicU32::new(0);
    static DELETED_VERTEX_ARRAYS: AtomicU32 = AtomicU32::new(0);
    static VERTEX_ARRAY_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn fallback_sampler_state_changes_only_after_explicit_commands() {
        assert_eq!(SamplerBinding::reset(false), SamplerBinding::TextureOwned);
        assert_eq!(
            SamplerBinding::explicit(SamplerFilter::Nearest).fallback_filter(),
            Some(SamplerFilter::Nearest)
        );
        assert_eq!(
            SamplerBinding::explicit(SamplerFilter::Linear).fallback_filter(),
            Some(SamplerFilter::Linear)
        );
        assert_eq!(
            SamplerBinding::Nearest
                .after_raw_callback(false)
                .fallback_filter(),
            Some(SamplerFilter::Nearest)
        );
        assert_eq!(SamplerBinding::reset(false), SamplerBinding::TextureOwned);
    }

    #[test]
    fn sampler_object_reset_rebinds_the_linear_sampler() {
        assert_eq!(
            SamplerBinding::Nearest.after_raw_callback(true),
            SamplerBinding::CallbackOwned
        );
        assert_eq!(SamplerBinding::reset(true), SamplerBinding::Linear);
    }

    #[test]
    fn framebuffer_extent_rejects_non_finite_and_out_of_range_values() {
        let raw = dear_imgui_rs::sys::ImDrawData {
            DisplaySize: dear_imgui_rs::sys::ImVec2 {
                x: f32::INFINITY,
                y: 64.0,
            },
            FramebufferScale: dear_imgui_rs::sys::ImVec2 { x: 1.0, y: 1.0 },
            ..Default::default()
        };
        // SAFETY: DrawData is a repr(transparent) read-only wrapper around ImDrawData.
        let draw_data =
            unsafe { &*std::ptr::from_ref(&raw).cast::<dear_imgui_rs::render::DrawData>() };
        assert!(matches!(
            FramebufferExtent::from_draw_data(draw_data),
            Err(crate::RenderError::NonFiniteDrawValue { .. })
        ));

        let raw = dear_imgui_rs::sys::ImDrawData {
            DisplaySize: dear_imgui_rs::sys::ImVec2 { x: 0.5, y: 64.0 },
            FramebufferScale: dear_imgui_rs::sys::ImVec2 { x: 1.0, y: 1.0 },
            ..Default::default()
        };
        // SAFETY: DrawData is a repr(transparent) read-only wrapper around ImDrawData.
        let draw_data =
            unsafe { &*std::ptr::from_ref(&raw).cast::<dear_imgui_rs::render::DrawData>() };
        assert!(matches!(
            FramebufferExtent::from_draw_data(draw_data),
            Ok(None)
        ));

        let raw = dear_imgui_rs::sys::ImDrawData {
            DisplaySize: dear_imgui_rs::sys::ImVec2 {
                x: f32::MAX,
                y: 64.0,
            },
            FramebufferScale: dear_imgui_rs::sys::ImVec2 { x: 1.0, y: 1.0 },
            ..Default::default()
        };
        // SAFETY: DrawData is a repr(transparent) read-only wrapper around ImDrawData.
        let draw_data =
            unsafe { &*std::ptr::from_ref(&raw).cast::<dear_imgui_rs::render::DrawData>() };
        assert!(matches!(
            FramebufferExtent::from_draw_data(draw_data),
            Err(crate::RenderError::FramebufferDimensionOutOfRange {
                dimension: "width",
                ..
            })
        ));
    }

    #[test]
    fn scissor_projection_rejects_non_finite_values_and_clamps_to_framebuffer() {
        let extent = FramebufferExtent {
            width: 64,
            height: 64,
        };
        assert!(matches!(
            project_scissor_rect([f32::NAN, 0.0, 32.0, 32.0], [0.0, 0.0], [1.0, 1.0], extent,),
            Err(crate::RenderError::NonFiniteClipRect(_))
        ));
        assert_eq!(
            project_scissor_rect([-8.0, -4.0, 72.0, 68.0], [0.0, 0.0], [1.0, 1.0], extent,)
                .unwrap(),
            Some([0, 0, 64, 64])
        );
    }

    unsafe extern "system" fn get_string(name: u32) -> *const u8 {
        if name == glow::VERSION {
            c"4.6".as_ptr().cast()
        } else {
            c"".as_ptr().cast()
        }
    }

    unsafe extern "system" fn get_string_i(_name: u32, _index: u32) -> *const u8 {
        c"".as_ptr().cast()
    }

    unsafe extern "system" fn get_integer(_name: u32, value: *mut i32) {
        if !value.is_null() {
            unsafe { *value = 0 };
        }
    }

    unsafe extern "system" fn disable(capability: u32) {
        assert_eq!(capability, glow::SCISSOR_TEST);
        CLEAR_EVENTS.lock().unwrap().push("disable-scissor");
    }

    unsafe extern "system" fn color_mask(r: u8, g: u8, b: u8, a: u8) {
        assert_eq!([r, g, b, a], [1, 1, 1, 1]);
        CLEAR_EVENTS.lock().unwrap().push("color-mask");
    }

    unsafe extern "system" fn clear_color(r: f32, g: f32, b: f32, a: f32) {
        assert_eq!([r, g, b, a], [0.1, 0.2, 0.3, 0.4]);
        CLEAR_EVENTS.lock().unwrap().push("clear-color");
    }

    unsafe extern "system" fn clear(mask: u32) {
        assert_eq!(mask, glow::COLOR_BUFFER_BIT);
        CLEAR_EVENTS.lock().unwrap().push("clear");
    }

    unsafe extern "system" fn gen_vertex_arrays(count: i32, arrays: *mut u32) {
        assert_eq!(count, 1);
        unsafe { *arrays = 44 };
    }

    unsafe extern "system" fn bind_vertex_array(array: u32) {
        BOUND_VERTEX_ARRAY.store(array, Ordering::SeqCst);
    }

    unsafe extern "system" fn delete_vertex_arrays(count: i32, _arrays: *const u32) {
        DELETED_VERTEX_ARRAYS.fetch_add(count.max(0) as u32, Ordering::SeqCst);
    }

    fn fake_gl() -> glow::Context {
        unsafe {
            glow::Context::from_loader_function(|name| {
                match name {
                    "glGetString" => get_string as *const (),
                    "glGetStringi" => get_string_i as *const (),
                    "glGetIntegerv" => get_integer as *const (),
                    "glDisable" => disable as *const (),
                    "glColorMask" => color_mask as *const (),
                    "glClearColor" => clear_color as *const (),
                    "glClear" => clear as *const (),
                    "glGenVertexArrays" => gen_vertex_arrays as *const (),
                    "glBindVertexArray" => bind_vertex_array as *const (),
                    "glDeleteVertexArrays" => delete_vertex_arrays as *const (),
                    _ => std::ptr::null(),
                }
                .cast()
            })
        }
    }

    struct PanicTextureMap;

    impl TextureMap for PanicTextureMap {
        fn get(&self, _texture_id: TextureId) -> Option<GlTexture> {
            panic!("injected texture map panic")
        }

        fn set(&mut self, _texture_id: TextureId, _gl_texture: GlTexture) {}
        fn remove(&mut self, _texture_id: TextureId) -> Option<GlTexture> {
            None
        }
        fn clear(&mut self) {}
        fn register_texture(
            &mut self,
            _gl_texture: GlTexture,
            _width: u32,
            _height: u32,
            _format: TextureFormat,
        ) -> InitResult<TextureId> {
            unreachable!()
        }
        fn update_texture(
            &mut self,
            _texture_id: TextureId,
            _gl_texture: GlTexture,
            _width: u32,
            _height: u32,
        ) {
        }
        fn texture_format(&self, _texture_id: TextureId) -> Option<TextureFormat> {
            None
        }
    }

    fn test_renderer(texture_map: Box<dyn TextureMap>) -> GlowRenderer {
        GlowRenderer {
            shaders: Shaders {
                program: None,
                attrib_location_tex: None,
                attrib_location_proj_mtx: None,
                attrib_location_color_gamma: None,
                attrib_location_vtx_pos: 0,
                attrib_location_vtx_uv: 0,
                attrib_location_vtx_color: 0,
            },
            vbo_handle: None,
            ebo_handle: None,
            owned_textures: Vec::new(),
            samplers: None,
            gl_version: GlVersion {
                major: 3,
                minor: 3,
                is_es: false,
            },
            has_clip_origin_support: false,
            has_separate_polygon_modes: false,
            has_sampler_object_support: true,
            is_destroyed: false,
            gl_context: None,
            context_binding: None,
            backend_user_data: Box::default(),
            renderer_name_ptr: std::ptr::null(),
            renderer_texture_max: [0, 0],
            renderer_state_fault: None,
            synthetic_test_renderer: true,
            texture_map: Some(texture_map),
            managed_textures: std::collections::HashMap::new(),
            destroyed_managed_textures: std::collections::HashMap::new(),
            renderer_consumer: None,
            framebuffer_srgb: false,
            color_gamma_override: None,
            viewport_clear_color: [0.0, 0.0, 0.0, 1.0],
        }
    }

    #[test]
    fn viewport_clear_is_unclipped_and_writes_every_color_channel() {
        CLEAR_EVENTS.lock().unwrap().clear();
        clear_viewport_framebuffer(&fake_gl(), [0.1, 0.2, 0.3, 0.4]);
        assert_eq!(
            *CLEAR_EVENTS.lock().unwrap(),
            ["disable-scissor", "color-mask", "clear-color", "clear"]
        );
    }

    #[test]
    fn texture_map_remains_owned_when_lookup_panics() {
        let renderer = test_renderer(Box::new(PanicTextureMap));
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            renderer.texture_map_for_draw().get(TextureId::new(1));
        }));
        assert!(panic.is_err());
        assert!(renderer.texture_map.is_some());
    }

    #[test]
    fn temporary_vertex_array_is_deleted_when_rendering_panics() {
        let _guard = VERTEX_ARRAY_TEST_LOCK.lock().unwrap();
        DELETED_VERTEX_ARRAYS.store(0, Ordering::SeqCst);
        let gl = fake_gl();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _vertex_array = VertexArrayGuard::create_and_bind(&gl).unwrap();
            panic!("injected render panic");
        }));
        assert!(panic.is_err());
        assert_eq!(DELETED_VERTEX_ARRAYS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn renderer_vertex_array_can_be_rebound_after_a_raw_callback() {
        let _guard = VERTEX_ARRAY_TEST_LOCK.lock().unwrap();
        BOUND_VERTEX_ARRAY.store(0, Ordering::SeqCst);
        let gl = fake_gl();
        let vertex_array = VertexArrayGuard::create_and_bind(&gl).unwrap();
        assert_eq!(BOUND_VERTEX_ARRAY.load(Ordering::SeqCst), 44);

        unsafe { gl.bind_vertex_array(None) };
        assert_eq!(BOUND_VERTEX_ARRAY.load(Ordering::SeqCst), 0);

        vertex_array.bind();
        assert_eq!(BOUND_VERTEX_ARRAY.load(Ordering::SeqCst), 44);
    }
}

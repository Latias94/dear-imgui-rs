use dear_imgui_rs::{
    internal::RawWrapper,
    render::{DrawCmd, DrawCmdParams, DrawData, DrawVert, ReconciledFrame, RenderedFrame},
};
use glow::{Context, HasContext};
use std::mem::size_of;

use super::GlowRenderer;
use crate::{
    draw_indices_as_bytes, draw_verts_as_bytes,
    error::{RenderError, RenderResult},
    gl_debug_message,
    state::{FramebufferSrgbScope, GlStateGuard},
    texture::TextureMap,
};

#[cfg(feature = "bind_vertex_array_support")]
struct VertexArrayGuard<'a> {
    gl: &'a Context,
    vertex_array: crate::GlVertexArray,
}

#[cfg(feature = "bind_vertex_array_support")]
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
}

#[cfg(feature = "bind_vertex_array_support")]
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
    /// Consume and render one Context-borrowed Dear ImGui frame.
    pub fn render(&mut self, frame: RenderedFrame<'_>) -> RenderResult<()> {
        self.render_reconciled(frame).map(drop)
    }

    /// Renders one frame and returns its texture-reconciliation proof to a presentation owner.
    pub fn render_reconciled<'frame>(
        &mut self,
        mut frame: RenderedFrame<'frame>,
    ) -> RenderResult<ReconciledFrame<'frame>> {
        self.render_borrowed(&mut frame)?;
        frame.into_reconciled().map_err(Into::into)
    }

    pub(super) fn render_borrowed(&mut self, frame: &mut RenderedFrame<'_>) -> RenderResult<()> {
        self.ensure_operational()?;
        self.validate_rendered_frame(frame)?;
        if self.is_destroyed {
            return Err(RenderError::RendererDestroyed);
        }
        let gl = self
            .gl_context
            .clone()
            .ok_or(RenderError::MissingGlContext)?;
        self.prepare_rendered_frame(&gl, frame)?;
        self.render_draw_data(&gl, frame.draw_data())
    }

    /// Consume and render a frame using an externally managed OpenGL context.
    pub fn render_with_context(
        &mut self,
        gl: &Context,
        frame: RenderedFrame<'_>,
    ) -> RenderResult<()> {
        self.render_with_context_reconciled(gl, frame).map(drop)
    }

    /// Renders with an external OpenGL context and returns texture-reconciliation proof.
    pub fn render_with_context_reconciled<'frame>(
        &mut self,
        gl: &Context,
        mut frame: RenderedFrame<'frame>,
    ) -> RenderResult<ReconciledFrame<'frame>> {
        self.ensure_operational()?;
        self.validate_rendered_frame(&frame)?;
        if self.is_destroyed {
            return Err(RenderError::RendererDestroyed);
        }
        self.prepare_rendered_frame(gl, &mut frame)?;
        self.render_draw_data(gl, frame.draw_data())?;
        frame.into_reconciled().map_err(Into::into)
    }

    fn prepare_rendered_frame(
        &mut self,
        gl: &Context,
        frame: &mut RenderedFrame<'_>,
    ) -> RenderResult<()> {
        if frame.is_texture_feedback_reconciled() {
            return Ok(());
        }
        let request_epoch = frame.epoch().map_or(0, |epoch| epoch.sequence());
        let feedback =
            self.process_texture_requests(gl, frame.texture_requests(), request_epoch)?;
        let progress = frame.reconcile_texture_feedback(feedback)?;
        self.prune_destroyed_managed_textures(progress.watermark());
        Ok(())
    }

    fn validate_rendered_frame(&self, frame: &RenderedFrame<'_>) -> RenderResult<()> {
        let consumer = self
            .renderer_consumer
            .as_ref()
            .ok_or(RenderError::RendererNotAttached)?;
        if frame.context_id() != consumer.context_id() {
            return Err(RenderError::ContextMismatch {
                expected: consumer.context_id(),
                actual: frame.context_id(),
            });
        }
        let epoch = frame.epoch().ok_or(RenderError::MissingRendererEpoch)?;
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
        self.render_draw_data_transaction(gl, Some(draw_data), false)
    }

    #[cfg(feature = "multi-viewport")]
    pub(super) fn render_viewport_draw_data(
        &mut self,
        gl: &Context,
        draw_data: Option<&DrawData>,
        clear: bool,
    ) -> RenderResult<()> {
        self.ensure_operational()?;
        self.render_draw_data_transaction(gl, draw_data, clear)
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

        let framebuffer_size = draw_data.map(|draw_data| {
            let display_size = draw_data.display_size();
            let framebuffer_scale = draw_data.framebuffer_scale();
            (
                display_size[0] * framebuffer_scale[0],
                display_size[1] * framebuffer_scale[1],
            )
        });
        let drawable = framebuffer_size.is_some_and(|(width, height)| width > 0.0 && height > 0.0);
        if !clear && !drawable {
            return Ok(());
        }

        gl_debug_message(gl, "dear-imgui-glow: start render");

        let _gl_state = GlStateGuard::capture(gl, self.gl_version);
        let _framebuffer_srgb = FramebufferSrgbScope::enter(gl, self.framebuffer_srgb);

        if clear {
            clear_viewport_framebuffer(gl, self.viewport_clear_color);
        }

        let Some(draw_data) = draw_data.filter(|_| drawable) else {
            return Ok(());
        };
        let (fb_width, fb_height) = framebuffer_size.expect("drawable data has a framebuffer size");

        #[cfg(feature = "bind_vertex_array_support")]
        let _vertex_array = self
            .gl_version
            .bind_vertex_array_support()
            .then(|| VertexArrayGuard::create_and_bind(gl))
            .transpose()?;

        self.set_up_render_state(gl, draw_data, fb_width, fb_height)?;
        let texture_map = self.texture_map_for_draw();
        self.render_draw_lists(gl, texture_map, draw_data)?;
        gl_debug_message(gl, "dear-imgui-glow: end render");

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
        fb_width: f32,
        fb_height: f32,
    ) -> RenderResult<()> {
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

            #[cfg(feature = "polygon_mode_support")]
            if self.gl_version.polygon_mode_support() {
                gl.polygon_mode(glow::FRONT_AND_BACK, glow::FILL);
            }

            #[cfg(feature = "primitive_restart_support")]
            if self.gl_version.primitive_restart_support() {
                gl.disable(glow::PRIMITIVE_RESTART);
            }

            // Setup viewport, orthographic projection matrix
            gl.viewport(0, 0, fb_width as i32, fb_height as i32);

            // Calculate projection matrix like the original implementation
            let display_pos = draw_data.display_pos();
            let display_size = draw_data.display_size();
            let l = display_pos[0];
            let r = display_pos[0] + display_size[0];
            let t = display_pos[1];
            let b = display_pos[1] + display_size[1];

            // Support for GL 4.5 rarely used glClipControl(GL_UPPER_LEFT)
            #[cfg(feature = "clip_origin_support")]
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
            if let Some(location) = self.shaders.attrib_location_tex {
                gl.uniform_1_i32(Some(&location), 0);
            }
            if let Some(location) = self.shaders.attrib_location_proj_mtx {
                gl.uniform_matrix_4_f32_slice(Some(&location), false, &ortho_projection.concat());
            }
            if let Some(location) = self.shaders.attrib_location_color_gamma {
                // Decode vertex color from sRGB when writing to sRGB framebuffer,
                // otherwise pass-through (1.0). Allow override if set.
                let gamma = self
                    .color_gamma_override
                    .unwrap_or(if self.framebuffer_srgb {
                        2.2_f32
                    } else {
                        1.0_f32
                    });
                gl.uniform_1_f32(Some(&location), gamma);
            }

            #[cfg(feature = "bind_sampler_support")]
            if self.gl_version.bind_sampler_support() {
                gl.bind_sampler(0, None);
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
    ) -> RenderResult<()> {
        gl_debug_message(gl, "start loop over draw lists");

        let mut sampler_filter = glow::LINEAR;

        for draw_list in draw_data.draw_lists() {
            // Upload vertex/index buffers
            self.upload_vertex_buffer(gl, draw_list.vtx_buffer())?;
            self.upload_index_buffer(gl, draw_list.idx_buffer())?;

            gl_debug_message(gl, "start loop over commands");
            for command in draw_list.commands() {
                match command {
                    DrawCmd::Elements {
                        count,
                        cmd_params,
                        raw_cmd: _,
                    } => {
                        self.render_elements(
                            gl,
                            texture_map,
                            count,
                            &cmd_params,
                            draw_data,
                            sampler_filter,
                        )?;
                    }
                    DrawCmd::ResetRenderState => {
                        let display_size = draw_data.display_size();
                        let framebuffer_scale = draw_data.framebuffer_scale();
                        self.set_up_render_state(
                            gl,
                            draw_data,
                            display_size[0] * framebuffer_scale[0],
                            display_size[1] * framebuffer_scale[1],
                        )?;
                        sampler_filter = glow::LINEAR;
                    }
                    DrawCmd::SetSamplerLinear => {
                        sampler_filter = glow::LINEAR;
                    }
                    DrawCmd::SetSamplerNearest => {
                        sampler_filter = glow::NEAREST;
                    }
                    DrawCmd::RawCallback { callback, raw_cmd } => {
                        let res =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                                callback(draw_list.raw(), raw_cmd)
                            }));
                        if res.is_err() {
                            eprintln!("dear-imgui-glow: panic in DrawCmd raw callback");
                            std::process::abort();
                        }
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
        gl: &Context,
        texture_map: &dyn TextureMap,
        count: usize,
        cmd_params: &DrawCmdParams,
        draw_data: &DrawData,
        sampler_filter: u32,
    ) -> RenderResult<()> {
        // Get texture
        let texture = texture_map.get(cmd_params.texture_id).ok_or_else(|| {
            RenderError::InvalidTexture(format!("Texture ID {:?} not found", cmd_params.texture_id))
        })?;

        unsafe {
            // Bind texture
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                sampler_filter as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                sampler_filter as i32,
            );

            // Set scissor rectangle
            let clip_rect = cmd_params.clip_rect;
            let display_pos = draw_data.display_pos();
            let display_size = draw_data.display_size();
            let framebuffer_scale = draw_data.framebuffer_scale();
            let clip_min_x = (clip_rect[0] - display_pos[0]) * framebuffer_scale[0];
            let clip_min_y = (clip_rect[1] - display_pos[1]) * framebuffer_scale[1];
            let clip_max_x = (clip_rect[2] - display_pos[0]) * framebuffer_scale[0];
            let clip_max_y = (clip_rect[3] - display_pos[1]) * framebuffer_scale[1];

            if clip_max_x <= clip_min_x || clip_max_y <= clip_min_y {
                return Ok(());
            }

            // Apply scissor/clipping rectangle (Y is inverted in OpenGL)
            let fb_height = display_size[1] * framebuffer_scale[1];
            gl.scissor(
                clip_min_x as i32,
                (fb_height - clip_max_y) as i32,
                (clip_max_x - clip_min_x) as i32,
                (clip_max_y - clip_min_y) as i32,
            );

            // Draw - dynamically detect index type like the original implementation
            let idx_offset = cmd_params.idx_offset * size_of::<dear_imgui_rs::render::DrawIdx>();
            let index_type = if size_of::<dear_imgui_rs::render::DrawIdx>() == 2 {
                glow::UNSIGNED_SHORT
            } else {
                glow::UNSIGNED_INT
            };

            #[cfg(feature = "vertex_offset_support")]
            if self.gl_version.vertex_offset_support() {
                gl.draw_elements_base_vertex(
                    glow::TRIANGLES,
                    count as i32,
                    index_type,
                    idx_offset as i32,
                    cmd_params.vtx_offset as i32,
                );
            } else {
                gl.draw_elements(glow::TRIANGLES, count as i32, index_type, idx_offset as i32);
            }

            #[cfg(not(feature = "vertex_offset_support"))]
            gl.draw_elements(glow::TRIANGLES, count as i32, index_type, idx_offset as i32);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU32, Ordering};

    use dear_imgui_rs::{TextureFormat, TextureId};

    #[cfg(feature = "bind_vertex_array_support")]
    use super::VertexArrayGuard;
    use super::{GlowRenderer, clear_viewport_framebuffer};
    use crate::{GlTexture, GlVersion, InitResult, shaders::Shaders, texture::TextureMap};

    static CLEAR_EVENTS: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    static DELETED_VERTEX_ARRAYS: AtomicU32 = AtomicU32::new(0);

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

    unsafe extern "system" fn bind_vertex_array(_array: u32) {}

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
            #[cfg(feature = "bind_vertex_array_support")]
            vertex_array_object: None,
            gl_version: GlVersion {
                major: 3,
                minor: 3,
                is_es: false,
            },
            has_clip_origin_support: false,
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

    #[cfg(feature = "bind_vertex_array_support")]
    #[test]
    fn temporary_vertex_array_is_deleted_when_rendering_panics() {
        DELETED_VERTEX_ARRAYS.store(0, Ordering::SeqCst);
        let gl = fake_gl();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _vertex_array = VertexArrayGuard::create_and_bind(&gl).unwrap();
            panic!("injected render panic");
        }));
        assert!(panic.is_err());
        assert_eq!(DELETED_VERTEX_ARRAYS.load(Ordering::SeqCst), 1);
    }
}

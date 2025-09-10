//! Main renderer implementation

use dear_imgui::{
    internal::RawWrapper,
    render::{DrawCmd, DrawCmdParams, DrawData, DrawVert},
    Context as ImGuiContext, TextureData, TextureFormat, TextureId,
};
use glow::{Context, HasContext};
use std::mem::size_of;

use crate::{
    error::{InitError, InitResult, RenderError, RenderResult},
    gl_debug_message,
    shaders::Shaders,
    state::GlStateBackup,
    texture::{SimpleTextureMap, TextureMap},
    to_byte_slice,
    versions::{GlVersion, GlslVersion},
    GlBuffer, GlTexture, GlVertexArray,
};

/// Main renderer for Dear ImGui using Glow
pub struct Renderer {
    shaders: Shaders,
    state_backup: GlStateBackup,
    pub vbo_handle: Option<GlBuffer>,
    pub ebo_handle: Option<GlBuffer>,
    pub font_atlas_texture: Option<GlTexture>,
    #[cfg(feature = "bind_vertex_array_support")]
    pub vertex_array_object: Option<GlVertexArray>,
    pub gl_version: GlVersion,
    pub has_clip_origin_support: bool,
    pub is_destroyed: bool,
}

impl Renderer {
    /// Create a new renderer
    ///
    /// Following the official OpenGL3 backend approach: relies on OpenGL's GL_FRAMEBUFFER_SRGB
    /// for automatic sRGB conversion rather than manual shader-based conversion.
    pub fn new<T: TextureMap>(
        gl: &Context,
        imgui_context: &mut ImGuiContext,
        texture_map: &mut T,
    ) -> InitResult<Self> {
        let gl_version = GlVersion::read(gl);

        #[cfg(feature = "clip_origin_support")]
        let has_clip_origin_support = {
            let support = gl_version.clip_origin_support();

            #[cfg(feature = "gl_extensions_support")]
            if support {
                support
            } else {
                let extensions_count = unsafe { gl.get_parameter_i32(glow::NUM_EXTENSIONS) } as u32;
                (0..extensions_count).any(|index| {
                    let extension_name =
                        unsafe { gl.get_parameter_indexed_string(glow::EXTENSIONS, index) };
                    extension_name == "GL_ARB_clip_control"
                })
            }
            #[cfg(not(feature = "gl_extensions_support"))]
            support
        };
        #[cfg(not(feature = "clip_origin_support"))]
        let has_clip_origin_support = false;

        let mut state_backup = GlStateBackup::default();
        state_backup.backup(gl, gl_version);

        // Configure ImGui context BEFORE building font atlas
        // This sets RENDERER_HAS_TEXTURES flag which is required for ImGui 1.92+
        Self::configure_imgui_context_static(imgui_context);

        let font_atlas_texture = Self::prepare_font_atlas(gl, imgui_context, texture_map)?;

        let shaders = Shaders::new(gl, gl_version)?;
        let vbo_handle = unsafe { gl.create_buffer() }.map_err(InitError::CreateBufferObject)?;
        let ebo_handle = unsafe { gl.create_buffer() }.map_err(InitError::CreateBufferObject)?;

        state_backup.restore(gl, gl_version);

        let renderer = Self {
            shaders,
            state_backup,
            vbo_handle: Some(vbo_handle),
            ebo_handle: Some(ebo_handle),
            font_atlas_texture: Some(font_atlas_texture),
            #[cfg(feature = "bind_vertex_array_support")]
            vertex_array_object: None,
            gl_version,
            has_clip_origin_support,
            is_destroyed: false,
        };

        Ok(renderer)
    }

    /// Prepare the font atlas texture
    ///
    /// With the new texture management system (ImGuiBackendFlags_RendererHasTextures),
    /// we don't need to manually create font textures. The textures will be created
    /// automatically when needed through the ImTextureData system.
    fn prepare_font_atlas<T: TextureMap>(
        _gl: &Context,
        imgui_context: &mut ImGuiContext,
        _texture_map: &mut T,
    ) -> InitResult<GlTexture> {
        let mut fonts = imgui_context.fonts();

        // Build the font atlas - this will trigger texture creation through the new system
        fonts.build();

        // With ImGuiBackendFlags_RendererHasTextures, we don't need to manually create textures.
        // The texture will be created automatically when the first frame is rendered.
        // We return a dummy texture here since the actual texture creation happens in render().

        // Create a dummy 1x1 white texture as a placeholder
        let dummy_texture = unsafe {
            let gl_texture = _gl
                .create_texture()
                .map_err(|e| InitError::CreateTexture(e))?;

            _gl.bind_texture(glow::TEXTURE_2D, Some(gl_texture));

            // Set texture parameters
            _gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            _gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );
            _gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            _gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );

            // Upload 1x1 white pixel
            let white_pixel = [255u8, 255u8, 255u8, 255u8];
            _gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                1,
                1,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(&white_pixel)),
            );

            _gl.bind_texture(glow::TEXTURE_2D, None);

            gl_texture
        };

        Ok(dummy_texture)
    }

    /// Configure the ImGui context for this renderer (static version)
    fn configure_imgui_context_static(imgui_context: &mut ImGuiContext) {
        let io = imgui_context.io_mut();

        // Set backend capabilities
        let mut flags = io.backend_flags();
        flags.insert(dear_imgui::BackendFlags::RENDERER_HAS_VTX_OFFSET);
        flags.insert(dear_imgui::BackendFlags::RENDERER_HAS_TEXTURES);

        #[cfg(feature = "multi-viewport")]
        {
            flags.insert(dear_imgui::BackendFlags::RENDERER_HAS_VIEWPORTS);
        }

        io.set_backend_flags(flags);
    }

    /// Configure the ImGui context for this renderer
    fn configure_imgui_context(&self, imgui_context: &mut ImGuiContext) {
        Self::configure_imgui_context_static(imgui_context);
    }

    /// Destroy the renderer and free OpenGL resources
    pub fn destroy(&mut self, gl: &Context) {
        if self.is_destroyed {
            return;
        }

        if let Some(h) = self.vbo_handle {
            unsafe { gl.delete_buffer(h) };
            self.vbo_handle = None;
        }
        if let Some(h) = self.ebo_handle {
            unsafe { gl.delete_buffer(h) };
            self.ebo_handle = None;
        }
        if let Some(p) = self.shaders.program {
            unsafe { gl.delete_program(p) };
            self.shaders.program = None;
        }
        if let Some(h) = self.font_atlas_texture {
            unsafe { gl.delete_texture(h) };
            self.font_atlas_texture = None;
        }

        #[cfg(feature = "bind_vertex_array_support")]
        if let Some(vao) = self.vertex_array_object {
            unsafe { gl.delete_vertex_array(vao) };
            self.vertex_array_object = None;
        }

        self.is_destroyed = true;
    }

    /// Render Dear ImGui draw data
    pub fn render<T: TextureMap>(
        &mut self,
        gl: &Context,
        texture_map: &T,
        draw_data: &DrawData,
    ) -> RenderResult<()> {
        if self.is_destroyed {
            return Err(RenderError::RendererDestroyed);
        }

        let fb_width = draw_data.display_size[0] * draw_data.framebuffer_scale[0];
        let fb_height = draw_data.display_size[1] * draw_data.framebuffer_scale[1];
        if !(fb_width > 0.0 && fb_height > 0.0) {
            return Ok(());
        }

        gl_debug_message(gl, "dear-imgui-glow: start render");

        // Catch up with texture updates. Most of the times, the list will have 1 element with an OK status, aka nothing to do.
        // (This almost always points to ImGui::GetPlatformIO().Textures[] but is part of ImDrawData to allow overriding or disabling texture updates).
        // Following the original Dear ImGui OpenGL3 implementation
        // Note: This is commented out for now as it requires mutable access to texture_map
        // TODO: Implement proper texture update handling
        /*
        if let Some(textures) = draw_data.textures() {
            for texture_data in textures {
                if texture_data.status() != dear_imgui::TextureStatus::OK {
                    self.update_texture_from_data(gl, texture_map, texture_data)?;
                }
            }
        }
        */

        self.state_backup.backup(gl, self.gl_version);

        #[cfg(feature = "bind_vertex_array_support")]
        if self.gl_version.bind_vertex_array_support() {
            unsafe {
                self.vertex_array_object = Some(gl.create_vertex_array().map_err(|err| {
                    RenderError::Generic(format!("Error creating vertex array object: {}", err))
                })?);
                gl.bind_vertex_array(self.vertex_array_object);
            }
        }

        self.set_up_render_state(gl, draw_data, fb_width, fb_height)?;

        // Render draw lists
        self.render_draw_lists(gl, texture_map, draw_data)?;

        // Cleanup
        #[cfg(feature = "bind_vertex_array_support")]
        if self.gl_version.bind_vertex_array_support() {
            if let Some(vao) = self.vertex_array_object {
                unsafe { gl.delete_vertex_array(vao) };
                self.vertex_array_object = None;
            }
        }

        self.state_backup.restore(gl, self.gl_version);
        gl_debug_message(gl, "dear-imgui-glow: end render");

        Ok(())
    }

    /// Set up OpenGL render state for ImGui rendering
    fn set_up_render_state(
        &mut self,
        gl: &Context,
        draw_data: &DrawData,
        fb_width: f32,
        fb_height: f32,
    ) -> RenderResult<()> {
        unsafe {
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

            // Note: We don't enable GL_FRAMEBUFFER_SRGB here because:
            // 1. Modern applications typically create sRGB surfaces directly (e.g., glutin's .with_srgb(true))
            // 2. The official OpenGL3 backend also doesn't explicitly enable GL_FRAMEBUFFER_SRGB
            // 3. Enabling it when the surface is already sRGB would cause incorrect double conversion
            // The sRGB conversion is handled by the surface/framebuffer configuration

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
            let l = draw_data.display_pos[0];
            let r = draw_data.display_pos[0] + draw_data.display_size[0];
            let t = draw_data.display_pos[1];
            let b = draw_data.display_pos[1] + draw_data.display_size[1];

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

            // Use memoffset to calculate correct field offsets, following the original implementation
            let pos_offset = memoffset::offset_of!(DrawVert, pos) as i32;
            let uv_offset = memoffset::offset_of!(DrawVert, uv) as i32;
            let color_offset = memoffset::offset_of!(DrawVert, col) as i32;

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

    /// Called every frame to prepare for rendering
    pub fn new_frame(&mut self, gl: &Context) -> RenderResult<()> {
        // Recreate device objects if they were destroyed
        if self.is_destroyed || self.shaders.program.is_none() {
            self.create_device_objects(gl)?;
        }
        Ok(())
    }

    /// Create OpenGL device objects (buffers, shaders, etc.)
    pub fn create_device_objects(&mut self, gl: &Context) -> RenderResult<()> {
        if self.shaders.program.is_none() {
            self.shaders = Shaders::new(gl, self.gl_version)
                .map_err(|e| RenderError::Generic(format!("Failed to create shaders: {:?}", e)))?;
        }

        if self.vbo_handle.is_none() {
            self.vbo_handle = Some(
                unsafe { gl.create_buffer() }
                    .map_err(|e| RenderError::Generic(format!("Failed to create VBO: {}", e)))?,
            );
        }

        if self.ebo_handle.is_none() {
            self.ebo_handle = Some(
                unsafe { gl.create_buffer() }
                    .map_err(|e| RenderError::Generic(format!("Failed to create EBO: {}", e)))?,
            );
        }

        self.is_destroyed = false;
        Ok(())
    }

    /// Destroy OpenGL device objects
    pub fn destroy_device_objects(&mut self, gl: &Context) {
        if let Some(vbo) = self.vbo_handle.take() {
            unsafe { gl.delete_buffer(vbo) };
        }
        if let Some(ebo) = self.ebo_handle.take() {
            unsafe { gl.delete_buffer(ebo) };
        }
        if let Some(program) = self.shaders.program.take() {
            unsafe { gl.delete_program(program) };
        }
        if let Some(texture) = self.font_atlas_texture.take() {
            unsafe { gl.delete_texture(texture) };
        }
        self.is_destroyed = true;
    }

    /// Render all draw lists
    fn render_draw_lists<T: TextureMap>(
        &mut self,
        gl: &Context,
        texture_map: &T,
        draw_data: &DrawData,
    ) -> RenderResult<()> {
        gl_debug_message(gl, "start loop over draw lists");

        for draw_list in draw_data.draw_lists() {
            // Upload vertex/index buffers
            self.upload_vertex_buffer(gl, draw_list.vtx_buffer())?;
            self.upload_index_buffer(gl, draw_list.idx_buffer())?;

            gl_debug_message(gl, "start loop over commands");
            for command in draw_list.commands() {
                match command {
                    DrawCmd::Elements { count, cmd_params } => {
                        self.render_elements(gl, texture_map, count, &cmd_params, draw_data)?;
                    }
                    DrawCmd::ResetRenderState => {
                        self.set_up_render_state(
                            gl,
                            draw_data,
                            draw_data.display_size[0] * draw_data.framebuffer_scale[0],
                            draw_data.display_size[1] * draw_data.framebuffer_scale[1],
                        )?;
                    }
                    DrawCmd::RawCallback { callback, raw_cmd } => {
                        unsafe { callback(draw_list.raw(), raw_cmd) };
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
    fn upload_vertex_buffer(&mut self, gl: &Context, vertices: &[DrawVert]) -> RenderResult<()> {
        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, self.vbo_handle);

            // Always use glBufferData() following the original implementation
            // This avoids corruption issues reported with Intel GPU drivers when using glBufferSubData()
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                to_byte_slice(vertices),
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
        &mut self,
        gl: &Context,
        indices: &[dear_imgui::render::DrawIdx],
    ) -> RenderResult<()> {
        unsafe {
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, self.ebo_handle);

            // Always use glBufferData() following the original implementation
            // This avoids corruption issues reported with Intel GPU drivers when using glBufferSubData()
            gl.buffer_data_u8_slice(
                glow::ELEMENT_ARRAY_BUFFER,
                to_byte_slice(indices),
                glow::STREAM_DRAW,
            );
        }

        Ok(())
    }

    /// Render elements with the given parameters
    fn render_elements<T: TextureMap>(
        &self,
        gl: &Context,
        texture_map: &T,
        count: usize,
        cmd_params: &DrawCmdParams,
        draw_data: &DrawData,
    ) -> RenderResult<()> {
        // Get texture
        let texture = if let Some(tex) = texture_map.get(cmd_params.texture_id) {
            tex
        } else {
            // Use font atlas texture as fallback
            self.font_atlas_texture.ok_or_else(|| {
                RenderError::InvalidTexture(format!(
                    "Texture ID {:?} not found",
                    cmd_params.texture_id
                ))
            })?
        };

        unsafe {
            // Bind texture
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));

            // Set scissor rectangle
            let clip_rect = cmd_params.clip_rect;
            let clip_min_x =
                (clip_rect[0] - draw_data.display_pos[0]) * draw_data.framebuffer_scale[0];
            let clip_min_y =
                (clip_rect[1] - draw_data.display_pos[1]) * draw_data.framebuffer_scale[1];
            let clip_max_x =
                (clip_rect[2] - draw_data.display_pos[0]) * draw_data.framebuffer_scale[0];
            let clip_max_y =
                (clip_rect[3] - draw_data.display_pos[1]) * draw_data.framebuffer_scale[1];

            if clip_max_x <= clip_min_x || clip_max_y <= clip_min_y {
                return Ok(());
            }

            // Apply scissor/clipping rectangle (Y is inverted in OpenGL)
            let fb_height = draw_data.display_size[1] * draw_data.framebuffer_scale[1];
            gl.scissor(
                clip_min_x as i32,
                (fb_height - clip_max_y) as i32,
                (clip_max_x - clip_min_x) as i32,
                (clip_max_y - clip_min_y) as i32,
            );

            // Draw - dynamically detect index type like the original implementation
            let idx_offset = cmd_params.idx_offset * size_of::<dear_imgui::render::DrawIdx>();
            let index_type = if size_of::<dear_imgui::render::DrawIdx>() == 2 {
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

    fn renderer_destroyed() -> RenderError {
        RenderError::RendererDestroyed
    }
}

/// Multi-viewport support functions
#[cfg(feature = "multi-viewport")]
pub mod multi_viewport {
    use dear_imgui::{sys, ViewportFlags};
    use std::ffi::c_void;

    /// Render a viewport (called by ImGui for multi-viewport support)
    pub unsafe extern "C" fn render_window(
        viewport: *mut sys::ImGuiViewport,
        _render_arg: *mut c_void,
    ) {
        if viewport.is_null() {
            return;
        }

        let viewport = &*viewport;

        // Clear the viewport if needed using Dear ImGui's ViewportFlags
        let flags = ViewportFlags::from_bits_truncate(viewport.Flags);
        if !flags.contains(ViewportFlags::NO_RENDERER_CLEAR) {
            // Note: In a real implementation, you would get the GL context from somewhere
            // This is a simplified example
            // gl.clear_color(0.0, 0.0, 0.0, 1.0);
            // gl.clear(glow::COLOR_BUFFER_BIT);
        }

        // Render the draw data
        if !viewport.DrawData.is_null() {
            // Note: In a real implementation, you would need to:
            // 1. Get the GL context for this viewport
            // 2. Get the renderer instance
            // 3. Call renderer.render() with the draw data
            // This requires more complex state management
        }
    }

    /// Initialize multi-viewport support
    pub fn init_multi_viewport_support(imgui_context: &mut dear_imgui::Context) {
        let platform_io = imgui_context.platform_io_mut();

        // Set the renderer callback
        unsafe {
            (*platform_io.as_raw_mut()).Renderer_RenderWindow = Some(render_window);
        }
    }

    /// Shutdown multi-viewport support
    pub fn shutdown_multi_viewport_support(context: &mut dear_imgui::Context) {
        // Destroy platform windows using the high-level interface
        context.destroy_platform_windows();
    }
}

/// Auto renderer that owns the OpenGL context and handles textures itself
pub struct AutoRenderer {
    gl: std::rc::Rc<glow::Context>,
    texture_map: SimpleTextureMap,
    renderer: Renderer,
}

impl AutoRenderer {
    /// Create a new AutoRenderer for simple rendering
    ///
    /// Following the official OpenGL3 backend approach: relies on OpenGL's GL_FRAMEBUFFER_SRGB
    /// for automatic sRGB conversion rather than manual shader-based conversion.
    pub fn new(gl: glow::Context, imgui_context: &mut ImGuiContext) -> InitResult<Self> {
        let mut texture_map = SimpleTextureMap::default();
        let renderer = Renderer::new(&gl, imgui_context, &mut texture_map)?;
        Ok(Self {
            gl: std::rc::Rc::new(gl),
            texture_map,
            renderer,
        })
    }

    /// Get a reference to the OpenGL context
    #[inline]
    pub fn gl_context(&self) -> &std::rc::Rc<glow::Context> {
        &self.gl
    }

    /// Get a reference to the texture map
    #[inline]
    pub fn texture_map(&self) -> &SimpleTextureMap {
        &self.texture_map
    }

    /// Get a mutable reference to the texture map
    #[inline]
    pub fn texture_map_mut(&mut self) -> &mut SimpleTextureMap {
        &mut self.texture_map
    }

    /// Get a reference to the renderer
    #[inline]
    pub fn renderer(&self) -> &Renderer {
        &self.renderer
    }

    /// Called every frame to prepare for rendering
    pub fn new_frame(&mut self) -> RenderResult<()> {
        self.renderer.new_frame(&self.gl)
    }

    /// Render Dear ImGui draw data
    #[inline]
    pub fn render(&mut self, draw_data: &DrawData) -> RenderResult<()> {
        // Handle texture updates first, following the original Dear ImGui OpenGL3 implementation
        for texture_data in draw_data.textures() {
            if texture_data.status() != dear_imgui::TextureStatus::OK {
                self.update_texture_from_data(texture_data)?;
            }
        }

        self.renderer.render(&self.gl, &self.texture_map, draw_data)
    }

    /// Update texture from Dear ImGui texture data
    /// Following the original Dear ImGui OpenGL3 implementation
    fn update_texture_from_data(
        &mut self,
        texture_data: &dear_imgui::TextureData,
    ) -> RenderResult<()> {
        use dear_imgui::TextureStatus;

        match texture_data.status() {
            TextureStatus::WantCreate => {
                // Create new texture
                self.create_texture_from_data(texture_data)?;
            }
            TextureStatus::WantUpdates => {
                // Update existing texture
                self.update_existing_texture_from_data(texture_data)?;
            }
            TextureStatus::WantDestroy => {
                // Destroy texture
                self.destroy_texture_from_data(texture_data)?;
            }
            TextureStatus::OK | TextureStatus::Destroyed => {
                // Nothing to do
            }
        }

        Ok(())
    }

    /// Create a new texture from ImTextureData
    fn create_texture_from_data(
        &mut self,
        texture_data: &dear_imgui::TextureData,
    ) -> RenderResult<()> {
        let width = texture_data.width() as u32;
        let height = texture_data.height() as u32;
        let format = texture_data.format();

        if let Some(pixels) = texture_data.pixels() {
            let gl_texture = unsafe {
                let gl_texture = self.gl.create_texture().map_err(|e| {
                    RenderError::Generic(format!("Failed to create texture: {}", e))
                })?;

                self.gl.bind_texture(glow::TEXTURE_2D, Some(gl_texture));

                // Set texture parameters
                self.gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_MIN_FILTER,
                    glow::LINEAR as i32,
                );
                self.gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_MAG_FILTER,
                    glow::LINEAR as i32,
                );
                self.gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_WRAP_S,
                    glow::CLAMP_TO_EDGE as i32,
                );
                self.gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_WRAP_T,
                    glow::CLAMP_TO_EDGE as i32,
                );

                // Upload texture data based on format
                match format {
                    dear_imgui::TextureFormat::RGBA32 => {
                        self.gl.tex_image_2d(
                            glow::TEXTURE_2D,
                            0,
                            glow::RGBA as i32,
                            width as i32,
                            height as i32,
                            0,
                            glow::RGBA,
                            glow::UNSIGNED_BYTE,
                            glow::PixelUnpackData::Slice(Some(pixels)),
                        );
                    }
                    dear_imgui::TextureFormat::Alpha8 => {
                        // Convert Alpha8 to RGBA32 for OpenGL
                        let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);
                        for &alpha in pixels {
                            rgba_data.push(255); // R
                            rgba_data.push(255); // G
                            rgba_data.push(255); // B
                            rgba_data.push(alpha); // A
                        }

                        self.gl.tex_image_2d(
                            glow::TEXTURE_2D,
                            0,
                            glow::RGBA as i32,
                            width as i32,
                            height as i32,
                            0,
                            glow::RGBA,
                            glow::UNSIGNED_BYTE,
                            glow::PixelUnpackData::Slice(Some(&rgba_data)),
                        );
                    }
                }

                self.gl.bind_texture(glow::TEXTURE_2D, None);
                gl_texture
            };

            // Store the texture in our map
            let texture_id = texture_data.tex_id();
            self.texture_map.set(texture_id, gl_texture);

            // TODO: Set the texture ID back to ImGui
            // This would require calling texture_data.set_tex_id() but that needs mutable access
        }

        Ok(())
    }

    /// Update an existing texture from ImTextureData
    fn update_existing_texture_from_data(
        &mut self,
        texture_data: &dear_imgui::TextureData,
    ) -> RenderResult<()> {
        // For now, we recreate the texture. A more efficient implementation would
        // use glTexSubImage2D for partial updates.
        self.create_texture_from_data(texture_data)
    }

    /// Destroy a texture from ImTextureData
    fn destroy_texture_from_data(
        &mut self,
        texture_data: &dear_imgui::TextureData,
    ) -> RenderResult<()> {
        let texture_id = texture_data.tex_id();

        if let Some(gl_texture) = self.texture_map.get(texture_id) {
            unsafe {
                self.gl.delete_texture(gl_texture);
            }
            self.texture_map.remove(texture_id);
        }

        Ok(())
    }

    /// Update a texture from ImGui texture data
    pub fn update_texture(
        &mut self,
        texture_id: TextureId,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> InitResult<()> {
        use crate::texture::update_imgui_texture;
        let gl_texture = update_imgui_texture(&self.gl, texture_id, width, height, data)?;

        // Update the texture mapping with modern texture management
        self.texture_map
            .update_texture(texture_id, gl_texture, width as i32, height as i32);

        Ok(())
    }

    /// Register a new texture with the modern texture management system
    pub fn register_texture(
        &mut self,
        width: u32,
        height: u32,
        format: TextureFormat,
        data: &[u8],
    ) -> InitResult<TextureId> {
        use crate::texture::create_texture_from_rgba;

        let gl_texture = create_texture_from_rgba(&self.gl, width, height, data)?;
        let texture_id =
            self.texture_map
                .register_texture(gl_texture, width as i32, height as i32, format);

        Ok(texture_id)
    }

    /// Get texture data for a given texture ID
    pub fn get_texture_data(&self, texture_id: TextureId) -> Option<&TextureData> {
        self.texture_map.get_texture_data(texture_id)
    }

    /// Get mutable texture data for a given texture ID
    pub fn get_texture_data_mut(&mut self, texture_id: TextureId) -> Option<&mut TextureData> {
        self.texture_map.get_texture_data_mut(texture_id)
    }
}

impl Drop for AutoRenderer {
    fn drop(&mut self) {
        self.renderer.destroy_device_objects(&self.gl);
    }
}

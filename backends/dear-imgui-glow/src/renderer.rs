//! Main renderer implementation

use dear_imgui_rs::{
    Context as ImGuiContext, TextureData, TextureFormat, TextureId,
    internal::RawWrapper,
    render::{DrawCmd, DrawCmdParams, DrawData, DrawVert},
};
use glow::{Context, HasContext};
use std::mem::size_of;

use crate::{
    GlBuffer, GlTexture, GlVertexArray,
    error::{InitError, InitResult, RenderError, RenderResult},
    gl_debug_message,
    shaders::Shaders,
    state::GlStateBackup,
    texture::{SimpleTextureMap, TextureMap},
    to_byte_slice,
    versions::GlVersion,
};

/// Main renderer for Dear ImGui using Glow (OpenGL)
///
/// This renderer provides a unified API similar to the WGPU backend while maintaining
/// flexibility for advanced use cases. It can either own the OpenGL context and texture
/// management (simple usage) or work with externally managed resources (advanced usage).
pub struct GlowRenderer {
    // Core rendering state
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

    // Resource management
    gl_context: Option<std::rc::Rc<glow::Context>>, // None = externally managed
    texture_map: Box<dyn TextureMap>,
    // Optional: enable GL_FRAMEBUFFER_SRGB during ImGui rendering
    framebuffer_srgb: bool,
    // Optional: override color gamma applied to vertex colors (None = auto)
    color_gamma_override: Option<f32>,
    // Clear color used for secondary viewports (multi-viewport). Main framebuffer
    // clear remains responsibility of the application.
    viewport_clear_color: [f32; 4],
}

impl GlowRenderer {
    /// Create a new Glow renderer with owned OpenGL context (recommended)
    ///
    /// This is the preferred way to create a Glow renderer as it handles all resource
    /// management automatically and provides a simple API similar to the WGPU backend.
    ///
    /// # Arguments
    /// * `gl` - OpenGL context (will be owned by the renderer)
    /// * `imgui_context` - Dear ImGui context to configure
    ///
    /// # Example
    /// ```rust,no_run
    /// use dear_imgui_glow::GlowRenderer;
    /// # use dear_imgui_glow::glow;
    /// # use dear_imgui_rs::Context as ImGuiContext;
    ///
    /// # let gl_context = unsafe { glow::Context::from_loader_function(|_| std::ptr::null()) };
    /// # let mut imgui_context = ImGuiContext::create();
    /// let mut renderer = GlowRenderer::new(gl_context, &mut imgui_context).unwrap();
    /// ```
    pub fn new(gl: glow::Context, imgui_context: &mut ImGuiContext) -> InitResult<Self> {
        let texture_map = Box::new(SimpleTextureMap::default());
        Self::with_texture_map(Some(gl), imgui_context, texture_map)
    }

    /// Create a new Glow renderer with custom texture management (advanced)
    ///
    /// This method allows you to provide your own texture management implementation
    /// and optionally manage the OpenGL context externally.
    ///
    /// # Arguments
    /// * `gl` - OpenGL context (Some = owned, None = externally managed)
    /// * `imgui_context` - Dear ImGui context to configure
    /// * `texture_map` - Custom texture map implementation
    ///
    /// # Example
    /// ```rust,no_run
    /// use dear_imgui_glow::{GlowRenderer, SimpleTextureMap};
    /// # use dear_imgui_glow::glow;
    /// # use dear_imgui_rs::Context as ImGuiContext;
    ///
    /// let texture_map = Box::new(SimpleTextureMap::default());
    /// # let gl_context = unsafe { glow::Context::from_loader_function(|_| std::ptr::null()) };
    /// # let mut imgui_context = ImGuiContext::create();
    /// let mut renderer = GlowRenderer::with_texture_map(
    ///     Some(gl_context),
    ///     &mut imgui_context,
    ///     texture_map
    /// ).unwrap();
    /// ```
    pub fn with_texture_map(
        gl: Option<glow::Context>,
        imgui_context: &mut ImGuiContext,
        texture_map: Box<dyn TextureMap>,
    ) -> InitResult<Self> {
        match gl {
            Some(context) => {
                let gl_rc = std::rc::Rc::new(context);
                Self::init_internal(Some(gl_rc.clone()), &gl_rc, imgui_context, texture_map)
            }
            None => Err(InitError::Generic(
                "OpenGL context is required for initialization".to_string(),
            )),
        }
    }

    /// Create a new Glow renderer with external OpenGL context (advanced)
    ///
    /// This method is for advanced users who want to manage the OpenGL context
    /// externally while still using custom texture management.
    ///
    /// # Arguments
    /// * `gl` - Reference to externally managed OpenGL context
    /// * `imgui_context` - Dear ImGui context to configure
    /// * `texture_map` - Custom texture map implementation
    ///
    /// # Example
    /// ```rust,no_run
    /// use dear_imgui_glow::{GlowRenderer, SimpleTextureMap};
    /// # use dear_imgui_glow::glow;
    /// # use dear_imgui_rs::Context as ImGuiContext;
    ///
    /// let texture_map = Box::new(SimpleTextureMap::default());
    /// # let gl_context = unsafe { glow::Context::from_loader_function(|_| std::ptr::null()) };
    /// # let mut imgui_context = ImGuiContext::create();
    /// let mut renderer = GlowRenderer::with_external_context(
    ///     &gl_context,
    ///     &mut imgui_context,
    ///     texture_map
    /// ).unwrap();
    /// ```
    pub fn with_external_context(
        gl: &Context,
        imgui_context: &mut ImGuiContext,
        texture_map: Box<dyn TextureMap>,
    ) -> InitResult<Self> {
        Self::init_internal(None, gl, imgui_context, texture_map)
    }

    /// Internal initialization method
    fn init_internal(
        owned_gl: Option<std::rc::Rc<glow::Context>>,
        gl: &Context,
        imgui_context: &mut ImGuiContext,
        mut texture_map: Box<dyn TextureMap>,
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

        let font_atlas_texture = Self::prepare_font_atlas(gl, imgui_context, &mut *texture_map)?;

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
            gl_context: owned_gl,
            texture_map,
            framebuffer_srgb: false,
            color_gamma_override: None,
            viewport_clear_color: [0.0, 0.0, 0.0, 1.0],
        };

        Ok(renderer)
    }

    /// Prepare the font atlas texture
    ///
    /// With the new texture management system (ImGuiBackendFlags_RendererHasTextures),
    /// we don't need to manually create font textures. The textures will be created
    /// automatically when needed through the ImTextureData system.
    fn prepare_font_atlas(
        gl: &Context,
        imgui_context: &mut ImGuiContext,
        texture_map: &mut dyn TextureMap,
    ) -> InitResult<GlTexture> {
        let mut fonts = imgui_context.fonts();

        // Build the font atlas CPU data
        fonts.build();

        // Try to upload the font atlas immediately (font-atlas fallback, legacy-style),
        // mirroring dear imgui's OpenGL3 backend and our WGPU backend behavior.
        // This only applies to the font atlas. User textures use the modern
        // ImTextureData flow handled during DrawData::textures() processing.
        // Doing this ensures the font texture is available even if draw_data-based
        // texture updates are not triggered on the first frame.
        let mut created_font_tex: Option<GlTexture> = None;
        unsafe {
            let tex = fonts.get_tex_data();
            if !tex.is_null() {
                let width = (*tex).Width as u32;
                let height = (*tex).Height as u32;
                let bpp = (*tex).BytesPerPixel;
                let px_ptr = (*tex).Pixels as *const u8;

                if !px_ptr.is_null() && width > 0 && height > 0 {
                    // Prepare pixel buffer as RGBA8
                    let rgba_pixels: Option<Vec<u8>> = match bpp {
                        4 => (width as usize)
                            .checked_mul(height as usize)
                            .and_then(|v| v.checked_mul(4))
                            .map(|size| std::slice::from_raw_parts(px_ptr, size).to_vec()),
                        1 => {
                            // NOTE(opt): For Alpha8 fonts/textures we currently expand to RGBA8 (white RGB + alpha)
                            // for maximum compatibility across GL/ES/WebGL.
                            // This can be optimized using single-channel textures + texture swizzle when available:
                            // - Desktop GL 3.3+ (or ARB_texture_swizzle), GLES 3.0+ support TEXTURE_SWIZZLE_RGBA.
                            // - Upload as RED/ALPHA/LUMINANCE depending on platform, then set swizzle to (1,1,1,R)
                            //   so sampling returns vec4(1,1,1,alpha) without duplicating data to 4 channels.
                            // - Requires feature/extension gating and fallback to RGBA path for older GL/ES/WebGL.
                            (width as usize)
                                .checked_mul(height as usize)
                                .and_then(|size| {
                                    let cap = size.checked_mul(4)?;
                                    let src = std::slice::from_raw_parts(px_ptr, size);
                                    let mut out = Vec::with_capacity(cap);
                                    for &a in src.iter() {
                                        out.extend_from_slice(&[255, 255, 255, a]);
                                    }
                                    Some(out)
                                })
                        }
                        _ => None,
                    };

                    if let Some(pixels) = rgba_pixels {
                        // Create GL texture and upload
                        let gl_texture = gl.create_texture().map_err(InitError::CreateTexture)?;

                        gl.bind_texture(glow::TEXTURE_2D, Some(gl_texture));
                        // Pixel store alignment for tightly packed RGBA8
                        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
                        gl.tex_image_2d(
                            glow::TEXTURE_2D,
                            0,
                            glow::RGBA as i32,
                            width as i32,
                            height as i32,
                            0,
                            glow::RGBA,
                            glow::UNSIGNED_BYTE,
                            glow::PixelUnpackData::Slice(Some(&pixels)),
                        );
                        // Set texture params
                        gl.tex_parameter_i32(
                            glow::TEXTURE_2D,
                            glow::TEXTURE_MIN_FILTER,
                            glow::LINEAR as i32,
                        );
                        gl.tex_parameter_i32(
                            glow::TEXTURE_2D,
                            glow::TEXTURE_MAG_FILTER,
                            glow::LINEAR as i32,
                        );
                        gl.tex_parameter_i32(
                            glow::TEXTURE_2D,
                            glow::TEXTURE_WRAP_S,
                            glow::CLAMP_TO_EDGE as i32,
                        );
                        gl.tex_parameter_i32(
                            glow::TEXTURE_2D,
                            glow::TEXTURE_WRAP_T,
                            glow::CLAMP_TO_EDGE as i32,
                        );
                        gl.bind_texture(glow::TEXTURE_2D, None);

                        // Register in our texture map and push TexID back to Dear ImGui
                        let tex_id = texture_map.register_texture(
                            gl_texture,
                            width as i32,
                            height as i32,
                            TextureFormat::RGBA32,
                        );
                        fonts.set_texture_id(tex_id);

                        created_font_tex = Some(gl_texture);
                    }
                }
            }
        }

        if let Some(tex) = created_font_tex {
            return Ok(tex);
        }

        // Fallback: create a 1x1 white texture as a last resort
        let dummy_texture = unsafe {
            let gl_texture = gl.create_texture().map_err(InitError::CreateTexture)?;
            gl.bind_texture(glow::TEXTURE_2D, Some(gl_texture));
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );
            let white_pixel = [255u8, 255u8, 255u8, 255u8];
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            gl.tex_image_2d(
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
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl_texture
        };

        Ok(dummy_texture)
    }

    /// Configure the ImGui context for this renderer (static version)
    fn configure_imgui_context_static(imgui_context: &mut ImGuiContext) {
        let should_set_name = imgui_context.io().backend_renderer_name().is_none();
        if should_set_name {
            let _ = imgui_context.set_renderer_name(Some(format!(
                "dear-imgui-glow {}",
                env!("CARGO_PKG_VERSION")
            )));
        }

        let io = imgui_context.io_mut();

        // Set backend capabilities
        let mut flags = io.backend_flags();
        flags.insert(dear_imgui_rs::BackendFlags::RENDERER_HAS_VTX_OFFSET);
        flags.insert(dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES);

        #[cfg(feature = "multi-viewport")]
        {
            flags.insert(dear_imgui_rs::BackendFlags::RENDERER_HAS_VIEWPORTS);
        }

        io.set_backend_flags(flags);
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

    /// Get a reference to the OpenGL context (if owned by the renderer)
    pub fn gl_context(&self) -> Option<&std::rc::Rc<glow::Context>> {
        self.gl_context.as_ref()
    }

    /// Get a reference to the texture map
    pub fn texture_map(&self) -> &dyn TextureMap {
        &*self.texture_map
    }

    /// Get a mutable reference to the texture map
    pub fn texture_map_mut(&mut self) -> &mut dyn TextureMap {
        &mut *self.texture_map
    }

    /// Called every frame to prepare for rendering
    pub fn new_frame(&mut self) -> RenderResult<()> {
        // Check if we need to recreate device objects
        let needs_recreation = self.is_destroyed || self.shaders.program.is_none();

        if needs_recreation {
            if let Some(gl) = self.gl_context.clone() {
                self.create_device_objects(&gl)?;
            } else {
                return Err(RenderError::Generic(
                    "No OpenGL context available".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Enable/disable GL_FRAMEBUFFER_SRGB around ImGui rendering
    /// Default is disabled; prefer application-level control of sRGB.
    pub fn set_framebuffer_srgb_enabled(&mut self, enabled: bool) {
        self.framebuffer_srgb = enabled;
    }

    /// Override the color gamma applied to ImGui vertex colors.
    /// Pass `Some(gamma)` to force a value (e.g., 2.2 or 1.0), or `None` to use auto:
    /// auto = 2.2 when sRGB is enabled, otherwise 1.0.
    pub fn set_color_gamma_override(&mut self, gamma: Option<f32>) {
        self.color_gamma_override = gamma;
    }

    /// Set clear color for secondary viewports when multi-viewport is enabled.
    ///
    /// This only affects the per-viewport renderer callback installed via
    /// `multi_viewport::enable`. Clearing of the main framebuffer remains
    /// responsibility of the application.
    pub fn set_viewport_clear_color(&mut self, color: [f32; 4]) {
        self.viewport_clear_color = color;
    }

    /// Render Dear ImGui draw data
    pub fn render(&mut self, draw_data: &DrawData) -> RenderResult<()> {
        // Handle texture updates first, following the original Dear ImGui OpenGL3 implementation
        for texture_data in draw_data.textures() {
            if texture_data.status() != dear_imgui_rs::TextureStatus::OK {
                self.update_texture_from_data(texture_data)?;
            }
        }

        if let Some(gl) = self.gl_context.clone() {
            self.render_internal(&gl, draw_data)
        } else {
            Err(RenderError::Generic("No OpenGL context available. Use render_with_context() for externally managed contexts.".to_string()))
        }
    }

    /// Advanced render method with external OpenGL context
    pub fn render_with_context(&mut self, gl: &Context, draw_data: &DrawData) -> RenderResult<()> {
        // Handle texture updates first
        for texture_data in draw_data.textures() {
            if texture_data.status() != dear_imgui_rs::TextureStatus::OK {
                self.update_texture_from_data(texture_data)?;
            }
        }

        self.render_internal(gl, draw_data)
    }

    /// Get OpenGL context reference (owned or external)
    fn get_gl_context(&self) -> RenderResult<&Context> {
        match &self.gl_context {
            Some(gl) => Ok(gl),
            None => Err(RenderError::Generic("No OpenGL context available. Use render_with_context() for externally managed contexts.".to_string())),
        }
    }

    /// Internal render implementation
    fn render_internal(&mut self, gl: &Context, draw_data: &DrawData) -> RenderResult<()> {
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
                if texture_data.status() != dear_imgui_rs::TextureStatus::OK {
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

        // Render draw lists - we need to avoid borrowing self and self.texture_map at the same time
        // Create a raw pointer to avoid borrow checker issues
        let texture_map_ptr = &*self.texture_map as *const dyn TextureMap;
        unsafe {
            self.render_draw_lists(gl, &*texture_map_ptr, draw_data)?;
        }

        // Cleanup
        #[cfg(feature = "bind_vertex_array_support")]
        if self.gl_version.bind_vertex_array_support()
            && let Some(vao) = self.vertex_array_object
        {
            unsafe { gl.delete_vertex_array(vao) };
            self.vertex_array_object = None;
        }

        // Optionally disable FRAMEBUFFER_SRGB before restoring state (we didn't back it up)
        if self.framebuffer_srgb {
            unsafe { gl.disable(glow::FRAMEBUFFER_SRGB) };
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

            // Optionally enable sRGB frame-buffer writes for sRGB-capable surfaces.
            // Note: This is typically controlled by the application. We expose a toggle
            // for convenience; it will be disabled after rendering to avoid leaking state.
            if self.framebuffer_srgb {
                gl.enable(glow::FRAMEBUFFER_SRGB);
            }

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
    fn render_draw_lists(
        &mut self,
        gl: &Context,
        texture_map: &dyn TextureMap,
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
                    DrawCmd::Elements {
                        count,
                        cmd_params,
                        raw_cmd,
                    } => {
                        let tex_id_u64 = unsafe {
                            dear_imgui_rs::sys::ImDrawCmd_GetTexID(
                                raw_cmd as *mut dear_imgui_rs::sys::ImDrawCmd,
                            )
                        } as u64;
                        let tex_id = dear_imgui_rs::TextureId::from(tex_id_u64);
                        self.render_elements(
                            gl,
                            texture_map,
                            count,
                            tex_id,
                            &cmd_params,
                            draw_data,
                        )?;
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
        indices: &[dear_imgui_rs::render::DrawIdx],
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
    fn render_elements(
        &self,
        gl: &Context,
        texture_map: &dyn TextureMap,
        count: usize,
        effective_tex_id: dear_imgui_rs::TextureId,
        cmd_params: &DrawCmdParams,
        draw_data: &DrawData,
    ) -> RenderResult<()> {
        // Get texture
        let texture = if let Some(tex) = texture_map.get(effective_tex_id) {
            tex
        } else {
            // Use font atlas texture as fallback
            self.font_atlas_texture.ok_or_else(|| {
                RenderError::InvalidTexture(format!("Texture ID {:?} not found", effective_tex_id))
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

impl GlowRenderer {
    /// Update texture from Dear ImGui texture data
    /// Following the original Dear ImGui OpenGL3 implementation
    fn update_texture_from_data(
        &mut self,
        texture_data: &mut dear_imgui_rs::TextureData,
    ) -> RenderResult<()> {
        use dear_imgui_rs::TextureStatus;

        match texture_data.status() {
            TextureStatus::WantCreate => {
                // Create new texture and assign ID back to Dear ImGui
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
        texture_data: &mut dear_imgui_rs::TextureData,
    ) -> RenderResult<()> {
        let gl = self.get_gl_context()?;
        let width = texture_data.width() as u32;
        let height = texture_data.height() as u32;
        let format = texture_data.format();

        if let Some(pixels) = texture_data.pixels() {
            let gl_texture = unsafe {
                // Backup texture binding / active texture / unpack alignment
                let last_active = gl.get_parameter_i32(glow::ACTIVE_TEXTURE) as u32;
                gl.active_texture(glow::TEXTURE0);
                let last_texture = gl.get_parameter_i32(glow::TEXTURE_BINDING_2D) as u32;
                let last_unpack = gl.get_parameter_i32(glow::UNPACK_ALIGNMENT);

                let gl_texture = gl.create_texture().map_err(|e| {
                    RenderError::Generic(format!("Failed to create texture: {}", e))
                })?;

                gl.bind_texture(glow::TEXTURE_2D, Some(gl_texture));
                gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);

                // Set texture parameters
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_MIN_FILTER,
                    glow::LINEAR as i32,
                );
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_MAG_FILTER,
                    glow::LINEAR as i32,
                );
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_WRAP_S,
                    glow::CLAMP_TO_EDGE as i32,
                );
                gl.tex_parameter_i32(
                    glow::TEXTURE_2D,
                    glow::TEXTURE_WRAP_T,
                    glow::CLAMP_TO_EDGE as i32,
                );

                // Upload texture data based on format
                match format {
                    dear_imgui_rs::TextureFormat::RGBA32 => {
                        gl.tex_image_2d(
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
                    dear_imgui_rs::TextureFormat::Alpha8 => {
                        // NOTE(opt): Could use GL RED + TEXTURE_SWIZZLE to avoid 4x expansion when
                        // GL 3.3+/GLES3.0+/ARB_texture_swizzle is available. See note in prepare_font_atlas().
                        // Convert Alpha8 to RGBA32 for OpenGL
                        let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);
                        for &alpha in pixels {
                            rgba_data.push(255); // R
                            rgba_data.push(255); // G
                            rgba_data.push(255); // B
                            rgba_data.push(alpha); // A
                        }

                        gl.tex_image_2d(
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

                // Restore pixel store and previous binding
                gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, last_unpack);
                if last_texture != 0 {
                    let restore =
                        glow::NativeTexture(std::num::NonZeroU32::new(last_texture).unwrap());
                    gl.bind_texture(glow::TEXTURE_2D, Some(restore));
                } else {
                    gl.bind_texture(glow::TEXTURE_2D, None);
                }
                gl.active_texture(last_active);
                gl_texture
            };
            // Register texture and set ID back to Dear ImGui
            let tex_id =
                self.texture_map
                    .register_texture(gl_texture, width as i32, height as i32, format);
            texture_data.set_tex_id(tex_id);
            texture_data.set_status(dear_imgui_rs::TextureStatus::OK);
        }

        Ok(())
    }

    /// Update an existing texture from ImTextureData
    fn update_existing_texture_from_data(
        &mut self,
        texture_data: &mut dear_imgui_rs::TextureData,
    ) -> RenderResult<()> {
        let gl = self.get_gl_context()?;
        let tex_id = texture_data.tex_id();
        let gl_texture = match self.texture_map.get(tex_id) {
            Some(t) => t,
            None => {
                // If texture doesn't exist, create it fully
                return self.create_texture_from_data(texture_data);
            }
        };

        let width = texture_data.width() as u32;
        let height = texture_data.height() as u32;
        let bpp = texture_data.bytes_per_pixel() as usize;

        let pixels = match texture_data.pixels() {
            Some(p) => p,
            None => {
                // No CPU pixels available this frame; nothing to upload.
                // Mark as OK to avoid retry storm and proceed with existing GPU texture.
                texture_data.set_status(dear_imgui_rs::TextureStatus::OK);
                return Ok(());
            }
        };

        // Backup texture binding / active texture / unpack alignment
        let last_active = unsafe { gl.get_parameter_i32(glow::ACTIVE_TEXTURE) as u32 };
        let last_texture = unsafe { gl.get_parameter_i32(glow::TEXTURE_BINDING_2D) as u32 };
        let last_unpack = unsafe { gl.get_parameter_i32(glow::UNPACK_ALIGNMENT) };
        unsafe {
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(gl_texture));
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        }

        // Collect update rects; prefer explicit Updates[] then fallback to single UpdateRect
        let mut rects: Vec<dear_imgui_rs::texture::TextureRect> = texture_data.updates().collect();
        if rects.is_empty() {
            let r = texture_data.update_rect();
            if r.w > 0 && r.h > 0 {
                rects.push(r);
            }
        }

        if rects.is_empty() {
            // Nothing to update; mark OK and return
            texture_data.set_status(dear_imgui_rs::TextureStatus::OK);
            // Restore previous binding and pixel store
            unsafe {
                gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, last_unpack);
                if last_texture != 0 {
                    let restore =
                        glow::NativeTexture(std::num::NonZeroU32::new(last_texture).unwrap());
                    gl.bind_texture(glow::TEXTURE_2D, Some(restore));
                } else {
                    gl.bind_texture(glow::TEXTURE_2D, None);
                }
                gl.active_texture(last_active);
            }
            return Ok(());
        }

        // Iterate update rects and upload each sub-region
        // Reuse a single staging buffer to avoid repeated allocations
        let mut sub_rgba: Vec<u8> = Vec::new();
        for rect in rects.into_iter() {
            let rx = rect.x as u32;
            let ry = rect.y as u32;
            let rw = rect.w as u32;
            let rh = rect.h as u32;

            if rw == 0 || rh == 0 {
                continue;
            }
            if rx >= width || ry >= height {
                continue;
            }

            let rw = rw.min(width - rx);
            let rh = rh.min(height - ry);
            if rw == 0 || rh == 0 {
                continue;
            }

            let row_stride = (width as usize).checked_mul(bpp).unwrap_or(0);
            if row_stride == 0 {
                continue;
            }

            let needed = (rw as usize)
                .checked_mul(rh as usize)
                .and_then(|v| v.checked_mul(4))
                .unwrap_or(0);
            if needed == 0 {
                continue;
            }
            if sub_rgba.capacity() < needed {
                sub_rgba.reserve(needed - sub_rgba.capacity());
            }
            sub_rgba.clear();

            for row in 0..(rh as usize) {
                let src_row_start = match (ry as usize + row)
                    .checked_mul(row_stride)
                    .and_then(|v| v.checked_add((rx as usize).saturating_mul(bpp)))
                {
                    Some(v) => v,
                    None => break,
                };
                let src_row_len = match (rw as usize).checked_mul(bpp) {
                    Some(v) => v,
                    None => break,
                };
                let Some(src_end) = src_row_start.checked_add(src_row_len) else {
                    break;
                };
                if src_end > pixels.len() {
                    break;
                }
                let src = &pixels[src_row_start..src_end];

                match texture_data.format() {
                    dear_imgui_rs::TextureFormat::RGBA32 => {
                        // Already RGBA, just append
                        sub_rgba.extend_from_slice(src);
                    }
                    dear_imgui_rs::TextureFormat::Alpha8 => {
                        // NOTE(opt): Could use GL RED + TEXTURE_SWIZZLE (if available) to avoid expansion.
                        // Convert A8 -> RGBA8 with white RGB
                        for &a in src.iter() {
                            sub_rgba.extend_from_slice(&[255, 255, 255, a]);
                        }
                    }
                }
            }

            unsafe {
                gl.tex_sub_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    rx as i32,
                    ry as i32,
                    rw as i32,
                    rh as i32,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(Some(&sub_rgba)),
                );
            }
        }

        // Restore previous binding and pixel store
        unsafe {
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, last_unpack);
            if last_texture != 0 {
                let restore = glow::NativeTexture(std::num::NonZeroU32::new(last_texture).unwrap());
                gl.bind_texture(glow::TEXTURE_2D, Some(restore));
            } else {
                gl.bind_texture(glow::TEXTURE_2D, None);
            }
            gl.active_texture(last_active);
        }

        // Mark status OK after updates
        texture_data.set_status(dear_imgui_rs::TextureStatus::OK);
        Ok(())
    }

    /// Destroy a texture from ImTextureData
    fn destroy_texture_from_data(
        &mut self,
        texture_data: &mut dear_imgui_rs::TextureData,
    ) -> RenderResult<()> {
        let gl = self.get_gl_context()?;
        let texture_id = texture_data.tex_id();

        if let Some(gl_texture) = self.texture_map.get(texture_id) {
            unsafe {
                gl.delete_texture(gl_texture);
            }
            self.texture_map.remove(texture_id);
        }

        texture_data.set_status(dear_imgui_rs::TextureStatus::Destroyed);

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
        let gl = self
            .get_gl_context()
            .map_err(|e| InitError::Generic(e.to_string()))?;
        let gl_texture = update_imgui_texture(gl, texture_id, width, height, data)?;

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

        let gl = self
            .get_gl_context()
            .map_err(|e| InitError::Generic(e.to_string()))?;
        let gl_texture = create_texture_from_rgba(gl, width, height, data)?;
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

/// Multi-viewport support for the Glow renderer.
///
/// This follows the Dear ImGui pattern where:
/// - the *platform* backend (e.g. SDL3, winit+glutin) owns OS windows and GL contexts;
/// - the *renderer* backend only renders `ImDrawData` into the currently bound context.
#[cfg(feature = "multi-viewport")]
pub mod multi_viewport {
    use super::GlowRenderer;
    use dear_imgui_rs::{Context, ViewportFlags, internal::RawCast, render::DrawData, sys};
    use glow::HasContext;
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Global pointer to the active GlowRenderer used for multi-viewport rendering.
    //
    // This mirrors the pattern used by the WGPU backend and the official C++ backends,
    // where the renderer backend installs callbacks in ImGuiPlatformIO.
    static RENDERER_PTR: AtomicUsize = AtomicUsize::new(0);

    #[inline]
    fn get_renderer<'a>() -> Option<&'a mut GlowRenderer> {
        let ptr = RENDERER_PTR.load(Ordering::SeqCst);
        if ptr == 0 {
            None
        } else {
            // Safety: we only ever store valid `GlowRenderer` pointers in RENDERER_PTR,
            // and the caller must ensure the referenced renderer outlives callbacks.
            Some(unsafe { &mut *(ptr as *mut GlowRenderer) })
        }
    }

    /// Enable Glow multi-viewport rendering for the given ImGui context and renderer.
    ///
    /// This registers a `Renderer_RenderWindow` callback in `ImGuiPlatformIO`, which
    /// Dear ImGui will call from `Context::render_platform_windows_default()` for each
    /// secondary viewport.
    ///
    /// The platform backend (e.g. SDL3) is expected to:
    /// - create/destroy platform windows;
    /// - set the appropriate OpenGL context current in `Platform_RenderWindow`;
    /// - swap buffers in `Platform_SwapBuffers`.
    ///
    /// This function assumes that `renderer` owns a `glow::Context` (the common case);
    /// if `GlowRenderer` was created with an external context (`gl_context()` returns
    /// `None`), the multi-viewport callback will early-return and do nothing.
    pub fn enable(renderer: &mut GlowRenderer, imgui_context: &mut Context) {
        // Store renderer pointer for callbacks
        RENDERER_PTR.store(renderer as *mut _ as usize, Ordering::SeqCst);

        // Install raw Renderer_RenderWindow callback. We don't need the typed
        // trampolines here, as we never expose Viewport typed wrappers.
        let platform_io = imgui_context.platform_io_mut();
        platform_io.set_renderer_render_window_raw(Some(renderer_render_window_sys));
    }

    /// Disable Glow multi-viewport rendering and clear the renderer callback.
    pub fn disable(imgui_context: &mut Context) {
        let platform_io = imgui_context.platform_io_mut();
        platform_io.set_renderer_render_window_raw(None);
        RENDERER_PTR.store(0, Ordering::SeqCst);
    }

    /// Backwards-compatible helper mirroring older naming.
    ///
    /// Prefer using [`enable`] directly so the renderer instance is clearly threaded
    /// through your setup code.
    #[deprecated(
        since = "0.6.0",
        note = "use multi_viewport::enable(renderer, imgui_context) instead"
    )]
    pub fn init_multi_viewport_support(_imgui_context: &mut Context) {
        // Kept only to avoid breaking existing code that might call this.
        // Without a renderer reference there is nothing useful to do here.
    }

    /// Shutdown helper that clears callbacks and destroys platform windows.
    pub fn shutdown_multi_viewport_support(context: &mut Context) {
        disable(context);
        context.destroy_platform_windows();
    }

    /// Renderer callback used by Dear ImGui for each secondary viewport.
    ///
    /// This corresponds to `ImGuiPlatformIO::Renderer_RenderWindow`.
    ///
    /// Safety: called from C with a valid `ImGuiViewport*` while the ImGui
    /// context and registered renderer are still alive.
    pub unsafe extern "C" fn renderer_render_window_sys(
        viewport: *mut sys::ImGuiViewport,
        _render_arg: *mut c_void,
    ) {
        if viewport.is_null() {
            return;
        }

        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let renderer = match get_renderer() {
                Some(r) => r,
                None => return,
            };

            // We currently only support the common case where GlowRenderer owns the
            // GL context. If the context is externally managed, the application
            // should render viewports by calling `render_with_context` manually.
            let gl_rc = match renderer.gl_context() {
                Some(rc) => rc.clone(),
                None => return,
            };
            let gl = &*gl_rc;

            // Safety: viewport was checked for null above.
            let vp_ref = unsafe { &*viewport };

            // Clear the viewport if needed using Dear ImGui's ViewportFlags.
            let flags = ViewportFlags::from_bits_truncate(vp_ref.Flags);
            if !flags.contains(ViewportFlags::NO_RENDERER_CLEAR) {
                let c = renderer.viewport_clear_color;
                unsafe {
                    gl.clear_color(c[0], c[1], c[2], c[3]);
                    gl.clear(glow::COLOR_BUFFER_BIT);
                }
            }

            // Render the draw data for this viewport, if present.
            if !vp_ref.DrawData.is_null() {
                // Safety: DrawData pointer is owned by Dear ImGui for the duration
                // of this callback.
                let raw_dd: &sys::ImDrawData = unsafe { &*vp_ref.DrawData };
                let draw_data: &DrawData = unsafe { DrawData::from_raw(raw_dd) };

                if let Err(err) = renderer.render_with_context(gl, draw_data) {
                    eprintln!("dear-imgui-glow: error rendering viewport: {:?}", err);
                }
            }
        }));

        if res.is_err() {
            eprintln!("dear-imgui-glow: panic in Renderer_RenderWindow callback");
            std::process::abort();
        }
    }
}

impl Drop for GlowRenderer {
    fn drop(&mut self) {
        if let Some(gl) = self.gl_context.take() {
            self.destroy_device_objects(&gl);
        }
    }
}

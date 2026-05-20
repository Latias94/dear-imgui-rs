use dear_imgui_rs::{Context as ImGuiContext, TextureFormat};
use glow::{Context, HasContext};

use super::{
    GlowRenderer,
    callbacks::{
        draw_callback_reset_render_state, draw_callback_set_sampler_linear,
        draw_callback_set_sampler_nearest,
    },
};
use crate::{
    GlTexture,
    error::{InitError, InitResult},
    shaders::Shaders,
    state::GlStateBackup,
    texture::{SimpleTextureMap, TextureMap, checked_gl_texture_size},
    versions::GlVersion,
};

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
            None => Err(InitError::MissingGlContext),
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
        let font_atlas_texture_data = imgui_context.fonts().get_tex_data();

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
            font_atlas_texture_data,
            #[cfg(feature = "bind_vertex_array_support")]
            vertex_array_object: None,
            gl_version,
            has_clip_origin_support,
            is_destroyed: false,
            gl_context: owned_gl,
            texture_map: Some(texture_map),
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

        // Build the font atlas CPU data (legacy/fallback path only).
        // With ImGui 1.92+ and BackendFlags::RENDERER_HAS_TEXTURES, the renderer will normally
        // receive font texture requests via DrawData::textures().
        if !fonts.is_built() {
            fonts.build();
        }

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
                    let (width_i32, height_i32) = checked_gl_texture_size(width, height)?;
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
                            width_i32,
                            height_i32,
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
                            width,
                            height,
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

        let platform_io = imgui_context.platform_io_mut();
        platform_io
            .set_draw_callback_reset_render_state_raw(Some(draw_callback_reset_render_state));
        platform_io
            .set_draw_callback_set_sampler_linear_raw(Some(draw_callback_set_sampler_linear));
        platform_io
            .set_draw_callback_set_sampler_nearest_raw(Some(draw_callback_set_sampler_nearest));
    }
}

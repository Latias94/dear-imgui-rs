use dear_imgui_rs::Context as ImGuiContext;
use glow::{Context, HasContext};

use super::{
    GlowRenderer,
    callbacks::{
        draw_callback_reset_render_state, draw_callback_set_sampler_linear,
        draw_callback_set_sampler_nearest,
    },
};
use crate::{
    error::{InitError, InitResult},
    shaders::Shaders,
    state::GlStateBackup,
    texture::{SimpleTextureMap, TextureMap},
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

    /// Create a renderer that shares ownership of its Glow function table.
    ///
    /// Unlike [`Self::with_external_context`], the renderer retains the exact `Rc` used to create
    /// its GL objects. This is the supported construction path when the renderer will later be
    /// consumed by `GlowViewportRuntime`.
    ///
    /// ```rust,no_run
    /// use std::rc::Rc;
    /// use dear_imgui_glow::{GlowRenderer, SimpleTextureMap, glow};
    /// use dear_imgui_rs::Context as ImGuiContext;
    ///
    /// let gl = Rc::new(unsafe {
    ///     glow::Context::from_loader_function(|_| std::ptr::null())
    /// });
    /// let mut imgui = ImGuiContext::create();
    /// let renderer = GlowRenderer::with_shared_context(
    ///     Rc::clone(&gl),
    ///     &mut imgui,
    ///     Box::new(SimpleTextureMap::default()),
    /// )?;
    /// # Ok::<(), dear_imgui_glow::InitError>(())
    /// ```
    pub fn with_shared_context(
        gl: std::rc::Rc<Context>,
        imgui_context: &mut ImGuiContext,
        texture_map: Box<dyn TextureMap>,
    ) -> InitResult<Self> {
        Self::init_internal(
            Some(std::rc::Rc::clone(&gl)),
            &gl,
            imgui_context,
            texture_map,
        )
    }

    /// Internal initialization method
    fn init_internal(
        owned_gl: Option<std::rc::Rc<glow::Context>>,
        gl: &Context,
        imgui_context: &mut ImGuiContext,
        texture_map: Box<dyn TextureMap>,
    ) -> InitResult<Self> {
        let renderer_consumer = imgui_context.create_renderer_consumer()?;
        imgui_context.reset_renderer_texture_bindings(&renderer_consumer)?;
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

        let shaders = Shaders::new(gl, gl_version)?;
        let vbo_handle = unsafe { gl.create_buffer() }.map_err(InitError::CreateBufferObject)?;
        let ebo_handle = unsafe { gl.create_buffer() }.map_err(InitError::CreateBufferObject)?;

        state_backup.restore(gl, gl_version);
        Self::configure_imgui_context_static(imgui_context);

        let renderer = Self {
            shaders,
            state_backup,
            vbo_handle: Some(vbo_handle),
            ebo_handle: Some(ebo_handle),
            owned_textures: Vec::new(),
            #[cfg(feature = "bind_vertex_array_support")]
            vertex_array_object: None,
            gl_version,
            has_clip_origin_support,
            is_destroyed: false,
            gl_context: owned_gl,
            texture_map: Some(texture_map),
            managed_textures: std::collections::HashMap::new(),
            renderer_consumer: Some(renderer_consumer),
            framebuffer_srgb: false,
            color_gamma_override: None,
            viewport_clear_color: [0.0, 0.0, 0.0, 1.0],
        };

        Ok(renderer)
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

        io.set_backend_flags(flags);

        let platform_io = imgui_context.platform_io_mut();
        platform_io
            .set_draw_callback_reset_render_state_raw(Some(draw_callback_reset_render_state));
        platform_io
            .set_draw_callback_set_sampler_linear_raw(Some(draw_callback_set_sampler_linear));
        platform_io
            .set_draw_callback_set_sampler_nearest_raw(Some(draw_callback_set_sampler_nearest));
    }

    pub(super) fn unconfigure_imgui_context_static(imgui_context: &mut ImGuiContext) {
        let expected_name = format!("dear-imgui-glow {}", env!("CARGO_PKG_VERSION"));
        if imgui_context
            .io()
            .backend_renderer_name()
            .is_some_and(|name| name.to_bytes() == expected_name.as_bytes())
        {
            let _ = imgui_context.set_renderer_name(None::<String>);
        }

        let io = imgui_context.io_mut();
        let mut flags = io.backend_flags();
        flags.remove(dear_imgui_rs::BackendFlags::RENDERER_HAS_VTX_OFFSET);
        flags.remove(dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES);
        io.set_backend_flags(flags);

        let platform_io = imgui_context.platform_io_mut();
        if platform_io
            .draw_callback_reset_render_state_raw()
            .is_some_and(|callback| {
                std::ptr::eq(
                    callback as *const (),
                    draw_callback_reset_render_state as *const (),
                )
            })
        {
            platform_io.set_draw_callback_reset_render_state_raw(None);
        }
        if platform_io
            .draw_callback_set_sampler_linear_raw()
            .is_some_and(|callback| {
                std::ptr::eq(
                    callback as *const (),
                    draw_callback_set_sampler_linear as *const (),
                )
            })
        {
            platform_io.set_draw_callback_set_sampler_linear_raw(None);
        }
        if platform_io
            .draw_callback_set_sampler_nearest_raw()
            .is_some_and(|callback| {
                std::ptr::eq(
                    callback as *const (),
                    draw_callback_set_sampler_nearest as *const (),
                )
            })
        {
            platform_io.set_draw_callback_set_sampler_nearest_raw(None);
        }
    }
}

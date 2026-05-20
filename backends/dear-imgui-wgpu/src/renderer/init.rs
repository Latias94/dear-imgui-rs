use super::{
    WgpuRenderer,
    callbacks::{
        draw_callback_reset_render_state, draw_callback_set_sampler_linear,
        draw_callback_set_sampler_nearest,
    },
};
use crate::wgpu;
use crate::{
    GammaMode, RendererError, RendererResult, ShaderManager, WgpuBackendData, WgpuInitInfo,
    WgpuTextureManager,
};
use dear_imgui_rs::{BackendFlags, Context, TextureId, sys};
use wgpu::*;

impl WgpuRenderer {
    /// Create a new WGPU renderer with full initialization (recommended)
    ///
    /// This is the preferred way to create a WGPU renderer as it ensures proper
    /// initialization order and is consistent with other backends.
    ///
    /// # Arguments
    /// * `init_info` - WGPU initialization information (device, queue, format)
    /// * `imgui_ctx` - Dear ImGui context to configure
    ///
    /// # Example
    /// ```rust,no_run
    /// use dear_imgui_rs::Context;
    /// use dear_imgui_wgpu::{WgpuRenderer, WgpuInitInfo};
    ///
    /// # fn main() -> Result<(), dear_imgui_wgpu::RendererError> {
    /// # let (device, queue) = todo!("initialize a WGPU Device/Queue");
    /// # let surface_format = wgpu::TextureFormat::Bgra8UnormSrgb;
    /// # let mut imgui_context = Context::create();
    /// let init_info = WgpuInitInfo::new(device, queue, surface_format);
    /// let mut renderer = WgpuRenderer::new(init_info, &mut imgui_context)?;
    /// # Ok(()) }
    /// ```
    pub fn new(init_info: WgpuInitInfo, imgui_ctx: &mut Context) -> RendererResult<Self> {
        // Native and wasm experimental path: fully configure context, including font atlas.
        #[cfg(any(
            not(target_arch = "wasm32"),
            all(target_arch = "wasm32", feature = "wasm-font-atlas-experimental")
        ))]
        {
            let mut renderer = Self::empty();
            renderer.init_with_context(init_info, imgui_ctx)?;
            Ok(renderer)
        }

        // Default wasm path: skip font atlas manipulation for safety.
        #[cfg(all(target_arch = "wasm32", not(feature = "wasm-font-atlas-experimental")))]
        {
            Self::new_without_font_atlas(init_info, imgui_ctx)
        }
    }

    /// Create an empty WGPU renderer for advanced usage
    ///
    /// This creates an uninitialized renderer that must be initialized later
    /// using `init_with_context()`. Most users should use `new()` instead.
    ///
    /// # Example
    /// ```rust,no_run
    /// use dear_imgui_rs::Context;
    /// use dear_imgui_wgpu::{WgpuRenderer, WgpuInitInfo};
    ///
    /// # fn main() -> Result<(), dear_imgui_wgpu::RendererError> {
    /// # let (device, queue) = todo!("initialize a WGPU Device/Queue");
    /// # let surface_format = wgpu::TextureFormat::Bgra8UnormSrgb;
    /// # let mut imgui_context = Context::create();
    /// let mut renderer = WgpuRenderer::empty();
    /// let init_info = WgpuInitInfo::new(device, queue, surface_format);
    /// renderer.init_with_context(init_info, &mut imgui_context)?;
    /// # Ok(()) }
    /// ```
    pub fn empty() -> Self {
        Self {
            backend_data: None,
            shader_manager: ShaderManager::new(),
            texture_manager: WgpuTextureManager::new(),
            default_texture: None,
            gamma_mode: GammaMode::Auto,
            #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
            viewport_clear_color: Color::BLACK,
        }
    }

    /// Initialize the renderer
    ///
    /// This corresponds to ImGui_ImplWGPU_Init in the C++ implementation
    pub fn init(&mut self, init_info: WgpuInitInfo) -> RendererResult<()> {
        // Create backend data
        let mut backend_data = WgpuBackendData::new(init_info);

        // Preflight: ensure the render target format is render-attachable and blendable.
        // The ImGui pipeline always uses alpha blending; non-blendable formats will
        // fail validation later with less actionable errors.
        let fmt = backend_data.render_target_format;
        if let Some(adapter) = backend_data.adapter.as_ref() {
            let fmt_features = adapter.get_texture_format_features(fmt);
            if !fmt_features
                .allowed_usages
                .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
                || !fmt_features
                    .flags
                    .contains(wgpu::TextureFormatFeatureFlags::BLENDABLE)
            {
                return Err(RendererError::InvalidRenderState(format!(
                    "Render target format {:?} is not suitable for ImGui WGPU renderer (requires RENDER_ATTACHMENT + BLENDABLE). allowed_usages={:?} flags={:?}",
                    fmt, fmt_features.allowed_usages, fmt_features.flags
                )));
            }
        }

        // Initialize render resources
        backend_data
            .render_resources
            .initialize(&backend_data.device)?;

        // Initialize shaders
        self.shader_manager.initialize(&backend_data.device)?;

        // Create default texture (1x1 white pixel)
        let default_texture =
            self.create_default_texture(&backend_data.device, &backend_data.queue)?;
        self.default_texture = Some(default_texture);

        // Create device objects (pipeline, etc.)
        self.create_device_objects(&mut backend_data)?;

        self.backend_data = Some(backend_data);
        Ok(())
    }

    /// Initialize the renderer with ImGui context configuration (without font atlas for WASM)
    ///
    /// This is a variant of init_with_context that skips font atlas preparation,
    /// useful for WASM builds where font atlas memory sharing is problematic.
    pub fn new_without_font_atlas(
        init_info: WgpuInitInfo,
        imgui_ctx: &mut Context,
    ) -> RendererResult<Self> {
        let mut renderer = Self::empty();

        // First initialize the renderer
        renderer.init(init_info)?;

        // Then configure the ImGui context with backend capabilities
        renderer.configure_imgui_context(imgui_ctx);

        // Skip font atlas preparation for WASM
        // The default font will be used automatically by Dear ImGui

        Ok(renderer)
    }

    /// Initialize the renderer with ImGui context configuration
    ///
    /// This is a convenience method that combines init() and configure_imgui_context()
    /// to ensure proper initialization order, similar to the glow backend approach.
    pub fn init_with_context(
        &mut self,
        init_info: WgpuInitInfo,
        imgui_ctx: &mut Context,
    ) -> RendererResult<()> {
        // First initialize the renderer
        self.init(init_info)?;

        // Then configure the ImGui context with backend capabilities
        // This must be done BEFORE preparing the font atlas
        self.configure_imgui_context(imgui_ctx);

        // Finally prepare the font atlas
        self.prepare_font_atlas(imgui_ctx)?;

        Ok(())
    }

    /// Set gamma mode
    pub fn set_gamma_mode(&mut self, mode: GammaMode) {
        self.gamma_mode = mode;
    }

    /// Set clear color for secondary viewports (multi-viewport mode).
    ///
    /// This color is used as the load/clear color when rendering ImGui-created
    /// platform windows via `RenderPlatformWindowsDefault`. It is independent
    /// from whatever clear color your main swapchain uses.
    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub fn set_viewport_clear_color(&mut self, color: Color) {
        self.viewport_clear_color = color;
    }

    /// Get current clear color for secondary viewports.
    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub fn viewport_clear_color(&self) -> Color {
        self.viewport_clear_color
    }

    /// Configure Dear ImGui context with WGPU backend capabilities
    pub fn configure_imgui_context(&self, imgui_context: &mut Context) {
        let should_set_name = imgui_context.io().backend_renderer_name().is_none();
        if should_set_name {
            let _ = imgui_context.set_renderer_name(Some(format!(
                "dear-imgui-wgpu {}",
                env!("CARGO_PKG_VERSION")
            )));
        }

        let io = imgui_context.io_mut();
        let mut flags = io.backend_flags();

        // Set WGPU renderer capabilities
        // We can honor the ImDrawCmd::VtxOffset field, allowing for large meshes.
        flags.insert(BackendFlags::RENDERER_HAS_VTX_OFFSET);
        // We can honor ImGuiPlatformIO::Textures[] requests during render.
        flags.insert(BackendFlags::RENDERER_HAS_TEXTURES);

        #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
        {
            // We can render additional platform windows
            flags.insert(BackendFlags::RENDERER_HAS_VIEWPORTS);
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

    /// Prepare font atlas for rendering
    pub fn prepare_font_atlas(&mut self, imgui_ctx: &mut Context) -> RendererResult<()> {
        if let Some(backend_data) = &self.backend_data {
            let device = backend_data.device.clone();
            let queue = backend_data.queue.clone();
            self.reload_font_texture(imgui_ctx, &device, &queue)?;
            if imgui_ctx
                .io()
                .backend_flags()
                .contains(BackendFlags::RENDERER_HAS_TEXTURES)
            {
                // New backend texture system: font textures are produced via DrawData::textures()
                // requests; do not assign a legacy TexID.
                return Ok(());
            }

            // Legacy fallback: only upload when the atlas does not already resolve to a live
            // WGPU texture. This keeps the backend idempotent without carrying a separate
            // renderer-side font texture cache now that the managed ImTextureData path is the
            // primary mode.
            let mut tex_ref = imgui_ctx.font_atlas().get_tex_ref();
            let existing_tex_id = unsafe { sys::ImTextureRef_GetTexID(&mut tex_ref) };
            let existing_tex_id = TextureId::from(existing_tex_id);
            let has_live_font_texture = !existing_tex_id.is_null()
                && self.texture_manager.contains_texture(existing_tex_id);

            if !has_live_font_texture
                && let Some(tex_id) =
                    self.try_upload_font_atlas_legacy(imgui_ctx, &device, &queue)?
                && cfg!(debug_assertions)
            {
                tracing::debug!(
                    target: "dear-imgui-wgpu",
                    "[dear-imgui-wgpu][debug] Font atlas uploaded via legacy fallback path. tex_id={}",
                    tex_id.id()
                );
            }
        }
        Ok(())
    }

    // create_device_objects moved to renderer/pipeline.rs

    /// Create a default 1x1 white texture
    fn create_default_texture(
        &self,
        device: &Device,
        queue: &Queue,
    ) -> RendererResult<TextureView> {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Dear ImGui Default Texture"),
            size: Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload white pixel
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8, 255u8, 255u8, 255u8], // RGBA white
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        Ok(texture.create_view(&TextureViewDescriptor::default()))
    }
}

impl Default for WgpuRenderer {
    fn default() -> Self {
        Self::empty()
    }
}

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
use dear_imgui_rs::{BackendFlags, Context};
use wgpu::*;

impl WgpuRenderer {
    /// Create a WGPU renderer bound to one Dear ImGui context (recommended)
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
    /// use dear_imgui_wgpu::{WgpuRenderer, WgpuInitInfo, wgpu};
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

    /// Create an empty, unbound WGPU renderer for advanced usage
    ///
    /// This creates an uninitialized renderer that must be initialized later
    /// using `init_with_context()`. Most users should use `new()` instead.
    ///
    /// # Example
    /// ```rust,no_run
    /// use dear_imgui_rs::Context;
    /// use dear_imgui_wgpu::{WgpuRenderer, WgpuInitInfo, wgpu};
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
            context_binding: None,
            backend_data: None,
            shader_manager: ShaderManager::new(),
            texture_manager: WgpuTextureManager::new(),
            default_texture: None,
            gamma_mode: GammaMode::Auto,
            #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
            viewport_clear_color: Color::BLACK,
            #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
            multi_viewport_active: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Initialize renderer-owned GPU state.
    ///
    /// Public initialization always goes through [`Self::init_with_context`] so renderer resources
    /// and Dear ImGui texture bindings cannot be replaced independently.
    fn initialize_device(&mut self, init_info: WgpuInitInfo) -> RendererResult<()> {
        #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
        self.ensure_multi_viewport_inactive()?;

        self.ensure_uninitialized()?;

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

        if let Err(error) = self.create_device_objects(&mut backend_data) {
            self.shader_manager = ShaderManager::new();
            self.default_texture = None;
            return Err(error);
        }

        self.backend_data = Some(backend_data);
        Ok(())
    }

    fn ensure_uninitialized(&self) -> RendererResult<()> {
        if self.backend_data.is_some() || self.context_binding.is_some() {
            return Err(RendererError::InvalidRenderState(
                "renderer is already initialized; call shutdown() with its ImGui context before reinitializing"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    fn initialize_for_context(
        &mut self,
        init_info: WgpuInitInfo,
        imgui_ctx: &mut Context,
        prepare_font_atlas: bool,
    ) -> RendererResult<()> {
        #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
        self.ensure_multi_viewport_inactive()?;

        self.ensure_uninitialized()?;
        Self::ensure_context_available(imgui_ctx)?;
        self.initialize_device(init_info)?;

        if let Err(error) = self.attach_context(imgui_ctx, prepare_font_atlas) {
            self.invalidate_device_objects_only();
            self.backend_data = None;
            return Err(error);
        }

        Ok(())
    }

    fn attach_context(
        &mut self,
        imgui_ctx: &mut Context,
        prepare_font_atlas: bool,
    ) -> RendererResult<()> {
        let renderer_flags_added = Self::configure_imgui_context(imgui_ctx)?;
        if let Err(error) = self.bind_context(imgui_ctx, renderer_flags_added) {
            Self::unconfigure_imgui_context(imgui_ctx, renderer_flags_added);
            return Err(error);
        }

        // A newly attached renderer cannot inherit GPU bindings from a previous renderer/device.
        imgui_ctx
            .platform_io_mut()
            .invalidate_renderer_texture_bindings();

        if prepare_font_atlas && let Err(error) = self.prepare_font_atlas(imgui_ctx) {
            imgui_ctx
                .platform_io_mut()
                .invalidate_renderer_texture_bindings();
            Self::unconfigure_imgui_context(imgui_ctx, renderer_flags_added);
            self.clear_context_binding();
            return Err(error);
        }

        Ok(())
    }

    fn ensure_context_available(imgui_context: &Context) -> RendererResult<()> {
        let platform_io = imgui_context.platform_io();
        let draw_callbacks_occupied = platform_io.draw_callback_reset_render_state_raw().is_some()
            || platform_io.draw_callback_set_sampler_linear_raw().is_some()
            || platform_io
                .draw_callback_set_sampler_nearest_raw()
                .is_some();

        if imgui_context.io().backend_renderer_name().is_some() || draw_callbacks_occupied {
            Err(RendererError::ContextAlreadyHasRenderer)
        } else {
            Ok(())
        }
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
        renderer.initialize_for_context(init_info, imgui_ctx, false)?;
        Ok(renderer)
    }

    /// Initialize the renderer and bind it to one ImGui context
    ///
    /// This is a convenience method that combines init() and configure_imgui_context()
    /// to ensure proper initialization order, similar to the glow backend approach.
    pub fn init_with_context(
        &mut self,
        init_info: WgpuInitInfo,
        imgui_ctx: &mut Context,
    ) -> RendererResult<()> {
        self.initialize_for_context(init_info, imgui_ctx, true)
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

    fn configure_imgui_context(imgui_context: &mut Context) -> RendererResult<BackendFlags> {
        imgui_context
            .set_renderer_name(Some(format!(
                "dear-imgui-wgpu {}",
                env!("CARGO_PKG_VERSION")
            )))
            .map_err(|error| {
                RendererError::InvalidRenderState(format!(
                    "failed to configure Dear ImGui renderer name: {error}"
                ))
            })?;

        let io = imgui_context.io_mut();
        let previous_flags = io.backend_flags();
        let renderer_flags =
            BackendFlags::RENDERER_HAS_VTX_OFFSET | BackendFlags::RENDERER_HAS_TEXTURES;

        // Set WGPU renderer capabilities
        // We can honor the ImDrawCmd::VtxOffset field, allowing for large meshes.
        // We can also honor ImGuiPlatformIO::Textures[] requests during render.
        io.set_backend_flags(previous_flags | renderer_flags);

        let platform_io = imgui_context.platform_io_mut();
        platform_io
            .set_draw_callback_reset_render_state_raw(Some(draw_callback_reset_render_state));
        platform_io
            .set_draw_callback_set_sampler_linear_raw(Some(draw_callback_set_sampler_linear));
        platform_io
            .set_draw_callback_set_sampler_nearest_raw(Some(draw_callback_set_sampler_nearest));

        Ok(renderer_flags & !previous_flags)
    }

    /// Prepare the bound context's font atlas for rendering.
    pub(super) fn prepare_font_atlas(&mut self, imgui_ctx: &mut Context) -> RendererResult<()> {
        self.ensure_context_matches(imgui_ctx)?;
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
            let existing_tex_id = imgui_ctx.font_atlas().texture_id();
            let has_live_font_texture = !existing_tex_id.is_null()
                && self.texture_manager.contains_texture(existing_tex_id);

            if !has_live_font_texture
                && let Some(_tex_id) =
                    self.try_upload_font_atlas_legacy(imgui_ctx, &device, &queue)?
                && cfg!(debug_assertions)
            {
                backend_debug!(
                    target: "dear_imgui_wgpu",
                    "[dear-imgui-wgpu][debug] Font atlas uploaded via legacy fallback path. tex_id={}",
                    _tex_id.id()
                );
            }
        }
        Ok(())
    }

    /// Recreate every resource discarded by `invalidate_device_objects()`.
    pub(super) fn create_device_objects(
        &mut self,
        backend_data: &mut WgpuBackendData,
    ) -> RendererResult<()> {
        backend_data
            .render_resources
            .initialize(&backend_data.device)?;
        self.shader_manager.initialize(&backend_data.device)?;
        self.default_texture =
            Some(self.create_default_texture(&backend_data.device, &backend_data.queue)?);
        self.create_render_pipeline(backend_data)
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_preflight_rejects_an_existing_renderer_name() {
        let mut context = Context::create();
        context
            .set_renderer_name(Some("foreign-renderer"))
            .expect("test renderer name should be valid");

        assert!(matches!(
            WgpuRenderer::ensure_context_available(&context),
            Err(RendererError::ContextAlreadyHasRenderer)
        ));
    }

    #[test]
    fn unconfigure_removes_only_flags_added_by_wgpu() {
        let mut context = Context::create();
        context
            .io_mut()
            .set_backend_flags(BackendFlags::RENDERER_HAS_TEXTURES);

        let added = WgpuRenderer::configure_imgui_context(&mut context)
            .expect("fresh context should accept WGPU renderer state");
        assert_eq!(added, BackendFlags::RENDERER_HAS_VTX_OFFSET);

        WgpuRenderer::unconfigure_imgui_context(&mut context, added);
        assert_eq!(
            context.io().backend_flags(),
            BackendFlags::RENDERER_HAS_TEXTURES
        );
        assert!(context.io().backend_renderer_name().is_none());
        assert!(
            context
                .platform_io()
                .draw_callback_reset_render_state_raw()
                .is_none()
        );
    }
}

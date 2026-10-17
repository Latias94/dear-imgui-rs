use super::{
    WgpuRenderer,
    callbacks::{
        draw_callback_reset_render_state, draw_callback_set_sampler_linear,
        draw_callback_set_sampler_nearest,
    },
};
use crate::{GammaMode, RendererError, RendererResult, WgpuBackendData, WgpuInitInfo};
use dear_imgui_rs::{BackendFlags, Context};
#[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
use wgpu::Color;

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
        let mut renderer = Self::empty();
        renderer.initialize_for_context(init_info, imgui_ctx)?;
        Ok(renderer)
    }

    pub(super) fn empty() -> Self {
        Self {
            context_state: None,
            gpu: super::gpu::GpuState::empty(),
            #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
            viewport_clear_color: Color::BLACK,
            renderer_consumer: None,
            drop_deferral: None,
        }
    }

    /// Initialize renderer-owned GPU state.
    ///
    /// Initialization is private so renderer resources and Dear ImGui bindings cannot be replaced
    /// independently.
    fn initialize_device(&mut self, init_info: WgpuInitInfo) -> RendererResult<()> {
        self.ensure_uninitialized()?;

        self.gpu.initialize(init_info)
    }

    fn ensure_uninitialized(&self) -> RendererResult<()> {
        if self.gpu.backend_data.is_some()
            || self.context_state.is_some()
            || self.renderer_consumer.is_some()
        {
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
    ) -> RendererResult<()> {
        self.ensure_uninitialized()?;
        Self::ensure_context_available(imgui_ctx)?;
        self.initialize_device(init_info)?;

        if let Err(error) = self.attach_context(imgui_ctx) {
            self.invalidate_device_objects_only();
            self.gpu.backend_data = None;
            return Err(error);
        }

        Ok(())
    }

    fn attach_context(&mut self, imgui_ctx: &mut Context) -> RendererResult<()> {
        let (renderer_flags_added, renderer_name_ptr) = Self::configure_imgui_context(imgui_ctx)?;
        if let Err(error) = self.bind_context(imgui_ctx, renderer_flags_added) {
            Self::clear_unbound_imgui_context(imgui_ctx, renderer_flags_added, renderer_name_ptr);
            return Err(error);
        }
        let consumer = match imgui_ctx.create_synchronous_renderer_consumer() {
            Ok(consumer) => consumer,
            Err(error) => {
                self.clear_bound_imgui_context(imgui_ctx);
                return Err(error.into());
            }
        };
        self.renderer_consumer = Some(consumer);

        // A newly attached renderer cannot inherit GPU bindings from a previous renderer/device.
        // There is no local map yet, but use the same explicit permit/commit transaction as every
        // destructive path so the Context validates the consumer generation before publication
        // changes.
        let consumer = self
            .renderer_consumer
            .take()
            .ok_or(RendererError::ContextNotBound)?;
        let reset = match imgui_ctx.prepare_renderer_texture_reset(&consumer) {
            Ok(reset) => reset,
            Err(error) => {
                drop(consumer);
                self.clear_bound_imgui_context(imgui_ctx);
                return Err(error.into());
            }
        };
        reset.commit();
        self.gpu.texture_manager.clear_destroyed_managed_textures();
        self.renderer_consumer = Some(consumer);

        Ok(())
    }

    pub(super) fn ensure_context_available(imgui_context: &Context) -> RendererResult<()> {
        if !imgui_context.io().backend_renderer_user_data().is_null() {
            return Err(RendererError::ContextAlreadyHasRenderer);
        }
        if imgui_context.io().backend_renderer_name().is_some() {
            return Err(RendererError::ContextAlreadyHasRenderer);
        }
        let reserved_flags = BackendFlags::RENDERER_HAS_VTX_OFFSET
            | BackendFlags::RENDERER_HAS_TEXTURES
            | BackendFlags::from_bits_retain(
                dear_imgui_rs::sys::ImGuiBackendFlags_RendererHasViewports,
            );
        if !(imgui_context.io().backend_flags() & reserved_flags).is_empty() {
            return Err(RendererError::ContextAlreadyHasRenderer);
        }

        let platform_io = imgui_context.platform_io();
        // SAFETY: PlatformIO belongs to this Context and is immutably borrowed for the check.
        let raw = unsafe { &*platform_io.as_raw() };
        if unsafe { !platform_io.renderer_render_state().is_null() }
            || raw.Renderer_TextureMaxWidth != 0
            || raw.Renderer_TextureMaxHeight != 0
            || raw.Renderer_CreateWindow.is_some()
            || raw.Renderer_DestroyWindow.is_some()
            || raw.Renderer_SetWindowSize.is_some()
            || raw.Renderer_RenderWindow.is_some()
            || raw.Renderer_SwapBuffers.is_some()
        {
            return Err(RendererError::ContextAlreadyHasRenderer);
        }
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

    /// Set gamma mode
    pub fn set_gamma_mode(&mut self, mode: GammaMode) {
        self.gpu.gamma_mode = mode;
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

    pub(super) fn configure_imgui_context(
        imgui_context: &mut Context,
    ) -> RendererResult<(BackendFlags, *const std::ffi::c_char)> {
        // Keep this function transactional even when called directly by an internal recovery
        // path: no renderer field may be overwritten after the preflight has passed.
        Self::ensure_context_available(imgui_context)?;
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
        let renderer_name_ptr = imgui_context
            .io()
            .backend_renderer_name()
            .expect("WGPU just published BackendRendererName")
            .as_ptr();

        let io = imgui_context.io_mut();
        let previous_flags = io.backend_flags();
        let renderer_flags =
            BackendFlags::RENDERER_HAS_VTX_OFFSET | BackendFlags::RENDERER_HAS_TEXTURES;

        // Set WGPU renderer capabilities
        // We can honor the ImDrawCmd::VtxOffset field, allowing for large meshes.
        // We can also honor ImGuiPlatformIO::Textures[] requests during render.
        io.set_backend_flags(previous_flags | renderer_flags);

        let platform_io = imgui_context.platform_io_mut();
        // SAFETY: these static callbacks use Dear ImGui's exact draw-callback ABI and stay valid
        // for the renderer lifetime.
        unsafe {
            platform_io
                .set_draw_callback_reset_render_state_raw(Some(draw_callback_reset_render_state));
            platform_io
                .set_draw_callback_set_sampler_linear_raw(Some(draw_callback_set_sampler_linear));
            platform_io
                .set_draw_callback_set_sampler_nearest_raw(Some(draw_callback_set_sampler_nearest));
        }

        Ok((renderer_flags & !previous_flags, renderer_name_ptr))
    }

    pub(super) fn clear_unbound_imgui_context(
        imgui_context: &mut Context,
        renderer_flags_added: BackendFlags,
        renderer_name_ptr: *const std::ffi::c_char,
    ) {
        let owned_name = imgui_context
            .io()
            .backend_renderer_name()
            .is_some_and(|name| name.as_ptr() == renderer_name_ptr);
        if owned_name {
            imgui_context
                .set_renderer_name::<String>(None)
                .expect("clearing WGPU BackendRendererName must not fail");
        }
        let io = imgui_context.io_mut();
        io.set_backend_flags(io.backend_flags() & !renderer_flags_added);
        Self::clear_owned_draw_callbacks(imgui_context.platform_io_mut());
    }

    pub(super) fn clear_bound_imgui_context(&mut self, imgui_context: &mut Context) {
        if let Some(state) = self.context_state.as_ref() {
            state.clear_with_context(imgui_context);
        }
        self.clear_context_state();
    }

    pub(super) fn create_device_objects(
        &mut self,
        backend: &mut WgpuBackendData,
    ) -> RendererResult<()> {
        self.gpu.create_device_objects(backend)
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
    fn context_preflight_rejects_reserved_renderer_flags() {
        let mut context = Context::create();
        context
            .io_mut()
            .set_backend_flags(BackendFlags::RENDERER_HAS_TEXTURES);

        assert!(matches!(
            WgpuRenderer::ensure_context_available(&context),
            Err(RendererError::ContextAlreadyHasRenderer)
        ));
        context.io_mut().set_backend_flags(BackendFlags::empty());
    }

    #[test]
    fn context_preflight_always_rejects_the_viewport_renderer_capability() {
        let context = Context::create();
        let viewport_flag = BackendFlags::from_bits_retain(
            dear_imgui_rs::sys::ImGuiBackendFlags_RendererHasViewports,
        );
        let raw_io = unsafe { dear_imgui_rs::sys::igGetIO_ContextPtr(context.as_raw()) };
        unsafe { (*raw_io).BackendFlags = viewport_flag.bits() };
        let flags_before = context.io().backend_flags();

        assert!(matches!(
            WgpuRenderer::ensure_context_available(&context),
            Err(RendererError::ContextAlreadyHasRenderer)
        ));
        assert_eq!(context.io().backend_flags(), flags_before);
        assert!(context.io().backend_renderer_user_data().is_null());
        assert!(context.io().backend_renderer_name().is_none());

        unsafe { (*raw_io).BackendFlags = 0 };
    }

    #[test]
    fn unconfigure_removes_the_exclusive_wgpu_claim() {
        let mut context = Context::create();
        let (added, _) = WgpuRenderer::configure_imgui_context(&mut context)
            .expect("fresh context should accept WGPU renderer state");
        assert_eq!(
            added,
            BackendFlags::RENDERER_HAS_VTX_OFFSET | BackendFlags::RENDERER_HAS_TEXTURES
        );
        let mut renderer = WgpuRenderer::empty();
        renderer
            .bind_context(&mut context, added)
            .expect("configured Context should bind once");

        renderer.clear_bound_imgui_context(&mut context);
        assert_eq!(context.io().backend_flags(), BackendFlags::empty());
        assert!(context.io().backend_renderer_user_data().is_null());
        assert!(context.io().backend_renderer_name().is_none());
        assert!(
            context
                .platform_io()
                .draw_callback_reset_render_state_raw()
                .is_none()
        );
    }
}

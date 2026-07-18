use super::{
    WgpuRenderer,
    callbacks::{
        draw_callback_reset_render_state, draw_callback_set_sampler_linear,
        draw_callback_set_sampler_nearest,
    },
};
use crate::{FrameResources, RenderResources, RendererError, RendererResult, ShaderManager};
use dear_imgui_rs::{BackendFlags, Context};

impl WgpuRenderer {
    /// Called every frame to prepare for rendering
    ///
    /// This corresponds to ImGui_ImplWGPU_NewFrame in the C++ implementation
    pub fn new_frame(&mut self) -> RendererResult<()> {
        let needs_recreation = match &self.backend_data {
            Some(backend_data) => {
                self.ensure_context_alive()?;
                !backend_data.is_initialized()
            }
            None => {
                return Err(RendererError::InvalidRenderState(
                    "renderer is not initialized".to_owned(),
                ));
            }
        };

        if needs_recreation {
            let mut backend_data = self
                .backend_data
                .take()
                .expect("new_frame() already verified backend data");
            let result = self.create_device_objects(&mut backend_data);
            self.backend_data = Some(backend_data);
            result?;
        }
        Ok(())
    }

    /// Invalidate device objects and reset their managed texture bindings.
    ///
    /// This corresponds to `ImGui_ImplWGPU_InvalidateDeviceObjects`. Passing the context makes
    /// destroying GPU textures and requeueing Context-owned uploads one operation.
    pub fn invalidate_device_objects(&mut self, imgui_context: &mut Context) -> RendererResult<()> {
        self.ensure_context_matches(imgui_context)?;

        self.invalidate_device_objects_only();
        imgui_context.reset_renderer_texture_bindings(self.renderer_consumer()?)?;

        Ok(())
    }

    pub(super) fn invalidate_device_objects_only(&mut self) {
        if let Some(ref mut backend_data) = self.backend_data {
            backend_data.pipeline_state = None;
            backend_data.render_resources = RenderResources::new();

            // Clear frame resources
            for frame_resources in &mut backend_data.frame_resources {
                *frame_resources = FrameResources::new();
            }
        }

        // Clear texture manager
        self.texture_manager.clear();
        self.default_texture = None;
        self.shader_manager = ShaderManager::new();
    }

    /// Shutdown the renderer and detach its Dear ImGui state.
    ///
    /// This corresponds to ImGui_ImplWGPU_Shutdown in the C++ implementation.
    ///
    /// The matching context is required so managed texture IDs, backend flags, the renderer name,
    /// and standard draw callbacks cannot outlive the GPU resources they describe. An initialized
    /// renderer consumed by a multi-viewport runtime is intentionally unavailable through this
    /// method until that owning runtime completes teardown.
    pub fn shutdown(&mut self, imgui_context: &mut Context) -> RendererResult<()> {
        self.ensure_context_matches(imgui_context)?;
        let renderer_flags_added = self.renderer_flags_added()?;

        self.invalidate_device_objects_only();
        imgui_context.reset_renderer_texture_bindings(self.renderer_consumer()?)?;
        self.backend_data = None;
        self.renderer_consumer = None;
        Self::unconfigure_imgui_context(imgui_context, renderer_flags_added);
        self.clear_context_binding();
        Ok(())
    }

    pub(super) fn unconfigure_imgui_context(
        imgui_context: &mut Context,
        renderer_flags_added: BackendFlags,
    ) {
        let renderer_name_is_ours =
            Self::renderer_name_is_ours(imgui_context.io().backend_renderer_name());
        let draw_callbacks_are_ours = Self::owned_draw_callbacks_match(imgui_context.platform_io());
        if renderer_name_is_ours {
            let _ = imgui_context.set_renderer_name(None::<String>);
        }

        if renderer_name_is_ours && draw_callbacks_are_ours {
            let io = imgui_context.io_mut();
            let mut flags = io.backend_flags();
            flags.remove(renderer_flags_added);
            io.set_backend_flags(flags);
        }

        Self::clear_owned_draw_callbacks(imgui_context.platform_io_mut());
    }

    pub(super) fn renderer_name_is_ours(name: Option<&std::ffi::CStr>) -> bool {
        let expected_name = format!("dear-imgui-wgpu {}", env!("CARGO_PKG_VERSION"));
        name.is_some_and(|name| name.to_bytes() == expected_name.as_bytes())
    }

    pub(super) fn owned_draw_callbacks_match(
        platform_io: &dear_imgui_rs::platform_io::PlatformIo,
    ) -> bool {
        platform_io
            .draw_callback_reset_render_state_raw()
            .map(|callback| callback as usize)
            == Some(draw_callback_reset_render_state as *const () as usize)
            && platform_io
                .draw_callback_set_sampler_linear_raw()
                .map(|callback| callback as usize)
                == Some(draw_callback_set_sampler_linear as *const () as usize)
            && platform_io
                .draw_callback_set_sampler_nearest_raw()
                .map(|callback| callback as usize)
                == Some(draw_callback_set_sampler_nearest as *const () as usize)
    }

    pub(super) fn clear_owned_draw_callbacks(
        platform_io: &mut dear_imgui_rs::platform_io::PlatformIo,
    ) {
        if platform_io
            .draw_callback_reset_render_state_raw()
            .map(|callback| callback as usize)
            == Some(draw_callback_reset_render_state as *const () as usize)
        {
            platform_io.set_draw_callback_reset_render_state_raw(None);
        }
        if platform_io
            .draw_callback_set_sampler_linear_raw()
            .map(|callback| callback as usize)
            == Some(draw_callback_set_sampler_linear as *const () as usize)
        {
            platform_io.set_draw_callback_set_sampler_linear_raw(None);
        }
        if platform_io
            .draw_callback_set_sampler_nearest_raw()
            .map(|callback| callback as usize)
            == Some(draw_callback_set_sampler_nearest as *const () as usize)
        {
            platform_io.set_draw_callback_set_sampler_nearest_raw(None);
        }
        unsafe {
            platform_io.set_renderer_render_state(std::ptr::null_mut());
        }
    }

    #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
    pub(super) fn shutdown_without_context_reset(&mut self) {
        self.invalidate_device_objects_only();
        self.backend_data = None;
        self.renderer_consumer = None;
        self.clear_context_binding();
    }
}

#[cfg(test)]
mod tests {
    use dear_imgui_rs::{
        BackendFlags, Context,
        texture::{OwnedTextureData, TextureFormat, TextureStatus},
    };

    use super::WgpuRenderer;
    use crate::RendererError;

    #[test]
    fn unbound_device_invalidation_preserves_managed_texture_bindings() {
        let mut context = Context::create();
        let mut texture = OwnedTextureData::new();
        texture.create(TextureFormat::RGBA32, 1, 1);
        let texture = context.register_texture(texture);

        let mut renderer = WgpuRenderer::empty();
        let result = renderer.invalidate_device_objects(&mut context);

        assert!(matches!(result, Err(RendererError::ContextNotBound)));
        context
            .with_texture(texture, |texture| {
                assert_eq!(texture.status(), TextureStatus::WantCreate);
                assert!(texture.texture_id().is_null());
            })
            .expect("registered texture should remain active");
    }

    #[test]
    fn foreign_context_lifecycle_calls_are_transactional() {
        let mut owner = Context::create();
        let mut renderer = WgpuRenderer::empty();
        renderer.renderer_consumer = Some(
            owner
                .create_renderer_consumer()
                .expect("test context should create a renderer consumer"),
        );
        renderer
            .bind_context(&owner, BackendFlags::empty())
            .expect("test renderer should bind once");

        let suspended_owner = owner.suspend();
        let mut foreign = Context::create();
        let foreign_flags = foreign.io().backend_flags();

        assert!(matches!(
            renderer.invalidate_device_objects(&mut foreign),
            Err(RendererError::ContextMismatch)
        ));
        assert!(matches!(
            renderer.shutdown(&mut foreign),
            Err(RendererError::ContextMismatch)
        ));
        assert_eq!(foreign.io().backend_flags(), foreign_flags);
        assert!(renderer.context_binding.is_some());

        let suspended_foreign = foreign.suspend();
        let mut owner = suspended_owner
            .activate()
            .expect("owner context should reactivate");
        renderer
            .shutdown(&mut owner)
            .expect("matching context should shut down the test renderer");
        assert!(renderer.context_binding.is_none());
        drop(suspended_foreign);
    }
}

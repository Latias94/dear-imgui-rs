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

    /// Invalidate device objects and detach their managed texture bindings.
    ///
    /// This corresponds to `ImGui_ImplWGPU_InvalidateDeviceObjects`. Passing the context makes
    /// invalidating GPU textures and requeueing their `ImTextureData` uploads one operation.
    pub fn invalidate_device_objects(&mut self, imgui_context: &mut Context) -> RendererResult<()> {
        self.ensure_context_matches(imgui_context)?;

        #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
        self.ensure_multi_viewport_inactive()?;

        self.invalidate_device_objects_only();
        imgui_context
            .platform_io_mut()
            .invalidate_renderer_texture_bindings();

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
    /// Returns [`RendererError::MultiViewportActive`](crate::RendererError::MultiViewportActive)
    /// while multi-viewport callbacks are registered. Call the matching
    /// `shutdown_multi_viewport_support` helper first so platform windows and callback-owned
    /// surfaces are destroyed before renderer GPU state is invalidated. The matching context is
    /// required so managed texture IDs, backend flags, the renderer name, and standard draw
    /// callbacks cannot outlive the GPU resources they describe.
    pub fn shutdown(&mut self, imgui_context: &mut Context) -> RendererResult<()> {
        self.ensure_context_matches(imgui_context)?;
        let renderer_flags_added = self.renderer_flags_added()?;

        #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
        self.ensure_multi_viewport_inactive()?;

        #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
        self.clear_multi_viewport_renderer_state();
        self.invalidate_device_objects_only();
        imgui_context
            .platform_io_mut()
            .invalidate_renderer_texture_bindings();
        self.backend_data = None;
        Self::unconfigure_imgui_context(imgui_context, renderer_flags_added);
        self.clear_context_binding();
        Ok(())
    }

    pub(super) fn unconfigure_imgui_context(
        imgui_context: &mut Context,
        renderer_flags_added: BackendFlags,
    ) {
        let expected_name = format!("dear-imgui-wgpu {}", env!("CARGO_PKG_VERSION"));
        let renderer_name_is_ours = imgui_context
            .io()
            .backend_renderer_name()
            .is_some_and(|name| name.to_bytes() == expected_name.as_bytes());
        if renderer_name_is_ours {
            let _ = imgui_context.set_renderer_name(None::<String>);
        }

        let io = imgui_context.io_mut();
        let mut flags = io.backend_flags();
        flags.remove(renderer_flags_added);
        io.set_backend_flags(flags);

        let platform_io = imgui_context.platform_io_mut();
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
    pub(super) fn clear_multi_viewport_renderer_state(&mut self) {
        // Make any installed multi-viewport callbacks become a no-op if the renderer is
        // shut down or dropped without an explicit disable/shutdown call.
        #[cfg(feature = "multi-viewport-winit")]
        {
            super::multi_viewport::clear_for_drop(self as *mut WgpuRenderer);
        }
        #[cfg(feature = "multi-viewport-sdl3")]
        {
            super::multi_viewport_sdl3::clear_for_drop(self as *mut WgpuRenderer);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::c_void;

    use dear_imgui_rs::{
        BackendFlags, Context, FramePrepareOptions, TextureData, TextureFormat, TextureId,
        TextureStatus,
    };

    use super::WgpuRenderer;
    use crate::RendererError;

    #[test]
    fn unbound_device_invalidation_preserves_managed_texture_bindings() {
        let mut context = Context::create();
        let mut texture = TextureData::new();
        texture.create(TextureFormat::RGBA32, 1, 1);
        texture.set_tex_id(TextureId::new(77));
        texture.set_backend_user_data(std::ptr::dangling_mut::<c_void>());
        texture.set_status(TextureStatus::OK);
        let texture = context.register_texture(texture);
        context.prepare_frame(
            FramePrepareOptions::new([640.0, 480.0], 1.0 / 60.0).renderer_has_textures(),
        );
        let _ = context.font_atlas().build();
        let _ = context.begin_frame().render();

        let mut renderer = WgpuRenderer::empty();
        let result = renderer.invalidate_device_objects(&mut context);

        assert!(matches!(result, Err(RendererError::ContextNotBound)));
        context
            .with_texture(texture, |texture| {
                assert_eq!(texture.status(), TextureStatus::OK);
                assert_eq!(texture.tex_id(), TextureId::new(77));
                assert!(!texture.backend_user_data().is_null());
            })
            .expect("registered texture should remain active");
    }

    #[test]
    fn foreign_context_lifecycle_calls_are_transactional() {
        let owner = Context::create();
        let mut renderer = WgpuRenderer::empty();
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

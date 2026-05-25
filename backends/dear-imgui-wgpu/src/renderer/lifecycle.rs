use super::WgpuRenderer;
use crate::{FrameResources, RenderResources, RendererResult};

impl WgpuRenderer {
    /// Called every frame to prepare for rendering
    ///
    /// This corresponds to ImGui_ImplWGPU_NewFrame in the C++ implementation
    pub fn new_frame(&mut self) -> RendererResult<()> {
        let needs_recreation = if let Some(backend_data) = &self.backend_data {
            backend_data.pipeline_state.is_none()
        } else {
            false
        };

        if needs_recreation {
            // Extract the backend data temporarily to avoid borrow checker issues
            let mut backend_data = self.backend_data.take().unwrap();
            self.create_device_objects(&mut backend_data)?;
            self.backend_data = Some(backend_data);
        }
        Ok(())
    }

    /// Invalidate device objects
    ///
    /// This corresponds to ImGui_ImplWGPU_InvalidateDeviceObjects in the C++ implementation
    pub fn invalidate_device_objects(&mut self) -> RendererResult<()> {
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

        Ok(())
    }

    /// Shutdown the renderer
    ///
    /// This corresponds to ImGui_ImplWGPU_Shutdown in the C++ implementation.
    ///
    /// If multi-viewport support was enabled, this also makes renderer callbacks no-op for this
    /// renderer. Call the matching multi-viewport shutdown helper when you also need to uninstall
    /// callbacks from the ImGui context and destroy platform windows.
    pub fn shutdown(&mut self) {
        #[cfg(any(feature = "multi-viewport-winit", feature = "multi-viewport-sdl3"))]
        self.clear_multi_viewport_renderer_state();
        self.invalidate_device_objects().ok();
        self.backend_data = None;
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

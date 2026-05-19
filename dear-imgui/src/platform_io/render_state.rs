use crate::sys;

use super::PlatformIo;

impl PlatformIo {
    /// Set the renderer render state
    ///
    /// This is used by renderer backends to expose their current render state
    /// to draw callbacks during rendering. The pointer should remain valid
    /// during the entire render_draw_data() call.
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - The pointer is valid for the duration of the render call
    /// - The pointed-to data matches the expected render state structure for the backend
    /// - The pointer is set to null after rendering is complete
    pub unsafe fn set_renderer_render_state(&mut self, render_state: *mut std::ffi::c_void) {
        self.inner_mut().Renderer_RenderState = render_state;
    }

    /// Get the current renderer render state
    ///
    /// Returns the render state pointer that was set by the renderer backend.
    /// This is typically used by draw callbacks to access the current render state.
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - The returned pointer is cast to the correct render state type for the backend
    /// - The pointer is only used during the render_draw_data() call
    pub unsafe fn renderer_render_state(&self) -> *mut std::ffi::c_void {
        self.inner().Renderer_RenderState
    }

    /// Set the standard draw callback used to request renderer-state reset.
    ///
    /// Renderer backends may install a backend-specific function pointer here. Higher-level
    /// draw iteration recognizes this callback as [`crate::render::DrawCmd::ResetRenderState`]
    /// instead of treating it as an arbitrary raw callback.
    #[doc(alias = "DrawCallback_ResetRenderState")]
    pub fn set_draw_callback_reset_render_state_raw(&mut self, callback: sys::ImDrawCallback) {
        self.inner_mut().DrawCallback_ResetRenderState = callback;
    }

    /// Get the standard draw callback used to request renderer-state reset.
    #[doc(alias = "DrawCallback_ResetRenderState")]
    pub fn draw_callback_reset_render_state_raw(&self) -> sys::ImDrawCallback {
        self.inner().DrawCallback_ResetRenderState
    }

    /// Set the standard draw callback used to request linear texture sampling.
    #[doc(alias = "DrawCallback_SetSamplerLinear")]
    pub fn set_draw_callback_set_sampler_linear_raw(&mut self, callback: sys::ImDrawCallback) {
        self.inner_mut().DrawCallback_SetSamplerLinear = callback;
    }

    /// Get the standard draw callback used to request linear texture sampling.
    #[doc(alias = "DrawCallback_SetSamplerLinear")]
    pub fn draw_callback_set_sampler_linear_raw(&self) -> sys::ImDrawCallback {
        self.inner().DrawCallback_SetSamplerLinear
    }

    /// Set the standard draw callback used to request nearest/point texture sampling.
    #[doc(alias = "DrawCallback_SetSamplerNearest")]
    pub fn set_draw_callback_set_sampler_nearest_raw(&mut self, callback: sys::ImDrawCallback) {
        self.inner_mut().DrawCallback_SetSamplerNearest = callback;
    }

    /// Get the standard draw callback used to request nearest/point texture sampling.
    #[doc(alias = "DrawCallback_SetSamplerNearest")]
    pub fn draw_callback_set_sampler_nearest_raw(&self) -> sys::ImDrawCallback {
        self.inner().DrawCallback_SetSamplerNearest
    }
}

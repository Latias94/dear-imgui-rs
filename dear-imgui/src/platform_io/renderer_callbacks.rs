#[cfg(feature = "multi-viewport")]
use std::ffi::c_void;

use crate::sys;

use super::PlatformIo;
#[cfg(feature = "multi-viewport")]
use super::{Viewport, callback_abi};

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
    ///
    /// # Safety
    ///
    /// When present, the callback must use the exact [`sys::ImDrawCallback`] ABI, remain callable
    /// while installed, accept the draw-list and draw-command pointers supplied by Dear ImGui,
    /// and never unwind across the C ABI. Rust records the standard callback identities at
    /// `Render()`, so the registration can be cleared after that frame is rendered. Keep the
    /// function and any state it requires alive while a raw native consumer could still invoke it.
    #[doc(alias = "DrawCallback_ResetRenderState")]
    pub unsafe fn set_draw_callback_reset_render_state_raw(
        &mut self,
        callback: sys::ImDrawCallback,
    ) {
        self.inner_mut().DrawCallback_ResetRenderState = callback;
    }

    /// Get the standard draw callback used to request renderer-state reset.
    #[doc(alias = "DrawCallback_ResetRenderState")]
    pub fn draw_callback_reset_render_state_raw(&self) -> sys::ImDrawCallback {
        self.inner().DrawCallback_ResetRenderState
    }

    /// Set the standard draw callback used to request linear texture sampling.
    ///
    /// # Safety
    ///
    /// See [`Self::set_draw_callback_reset_render_state_raw`].
    #[doc(alias = "DrawCallback_SetSamplerLinear")]
    pub unsafe fn set_draw_callback_set_sampler_linear_raw(
        &mut self,
        callback: sys::ImDrawCallback,
    ) {
        self.inner_mut().DrawCallback_SetSamplerLinear = callback;
    }

    /// Get the standard draw callback used to request linear texture sampling.
    #[doc(alias = "DrawCallback_SetSamplerLinear")]
    pub fn draw_callback_set_sampler_linear_raw(&self) -> sys::ImDrawCallback {
        self.inner().DrawCallback_SetSamplerLinear
    }

    /// Set the standard draw callback used to request nearest/point texture sampling.
    ///
    /// # Safety
    ///
    /// See [`Self::set_draw_callback_reset_render_state_raw`].
    #[doc(alias = "DrawCallback_SetSamplerNearest")]
    pub unsafe fn set_draw_callback_set_sampler_nearest_raw(
        &mut self,
        callback: sys::ImDrawCallback,
    ) {
        self.inner_mut().DrawCallback_SetSamplerNearest = callback;
    }

    /// Get the standard draw callback used to request nearest/point texture sampling.
    #[doc(alias = "DrawCallback_SetSamplerNearest")]
    pub fn draw_callback_set_sampler_nearest_raw(&self) -> sys::ImDrawCallback {
        self.inner().DrawCallback_SetSamplerNearest
    }

    /// Sets the standard callback requesting a texture's own sampler settings.
    ///
    /// # Safety
    ///
    /// See [`Self::set_draw_callback_reset_render_state_raw`].
    #[doc(alias = "DrawCallback_SetSamplerFromTex")]
    pub unsafe fn set_draw_callback_set_sampler_from_texture_raw(
        &mut self,
        callback: sys::ImDrawCallback,
    ) {
        self.inner_mut().DrawCallback_SetSamplerFromTex = callback;
    }

    /// Gets the standard callback requesting a texture's own sampler settings.
    #[doc(alias = "DrawCallback_SetSamplerFromTex")]
    pub fn draw_callback_set_sampler_from_texture_raw(&self) -> sys::ImDrawCallback {
        self.inner().DrawCallback_SetSamplerFromTex
    }
}

#[cfg(feature = "multi-viewport")]
pub(super) unsafe fn clear_renderer_aggregate_callbacks_for_platform_io(
    pio: *mut sys::ImGuiPlatformIO,
) {
    unsafe { sys::ImGuiPlatformIO_Set_Renderer_SetWindowSize_PointerParam(pio, None) }
}

#[cfg(feature = "multi-viewport")]
impl PlatformIo {
    /// Clear all renderer backend handlers.
    ///
    /// This resets the `Renderer_*` callback table stored in `ImGuiPlatformIO`.
    /// This also releases the aggregate callback storage belonging to this `PlatformIo`.
    ///
    /// # Safety
    ///
    /// Dear ImGui must no longer be able to invoke any renderer callback, and all renderer-owned
    /// viewport state must already have been released.
    pub unsafe fn clear_renderer_handlers(&mut self) {
        unsafe {
            sys::ImGuiPlatformIO_ClearRendererHandlers(self.as_raw_mut());
            clear_renderer_aggregate_callbacks_for_platform_io(self.as_raw_mut());
        }
    }

    /// Set renderer create window callback (raw).
    ///
    /// # Safety
    ///
    /// When present, the callback must remain callable until it is replaced or cleared, must not
    /// unwind across the C ABI, and must uphold Dear ImGui's renderer callback contract for every
    /// viewport passed to it. Replacing or clearing a live callback is valid only after all state
    /// interpreted by that callback has been released.
    pub unsafe fn set_renderer_create_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.inner_mut().Renderer_CreateWindow = callback;
    }

    /// Return the raw `Renderer_CreateWindow` callback.
    #[doc(hidden)]
    pub fn renderer_create_window_raw(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)> {
        self.inner().Renderer_CreateWindow
    }

    /// Set renderer create window callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// Same requirements as [`Self::set_platform_create_window`], but for Dear ImGui's
    /// `Renderer_CreateWindow` callback.
    pub unsafe fn set_renderer_create_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        unsafe { self.set_renderer_create_window_raw(callback_abi::window(callback)) }
    }

    /// Set renderer destroy window callback (raw).
    ///
    /// # Safety
    ///
    /// See [`Self::set_renderer_create_window_raw`].
    pub unsafe fn set_renderer_destroy_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.inner_mut().Renderer_DestroyWindow = callback;
    }

    /// Return the raw `Renderer_DestroyWindow` callback.
    #[doc(hidden)]
    pub fn renderer_destroy_window_raw(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)> {
        self.inner().Renderer_DestroyWindow
    }

    /// Set renderer destroy window callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_renderer_create_window`].
    pub unsafe fn set_renderer_destroy_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport)>,
    ) {
        unsafe { self.set_renderer_destroy_window_raw(callback_abi::window(callback)) }
    }

    /// Set renderer set window size callback through the aggregate ABI shim.
    ///
    /// The callback receives a pointer because the C++ slot accepts `ImVec2` by value. The
    /// repository-owned C++ thunk performs that C++ call and forwards a pointer into Rust.
    ///
    /// # Safety
    ///
    /// See [`Self::set_renderer_create_window_raw`]. The pointed-to `ImVec2` is valid only for the
    /// duration of the callback.
    pub unsafe fn set_renderer_set_window_size_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *const sys::ImVec2)>,
    ) {
        unsafe {
            sys::ImGuiPlatformIO_Set_Renderer_SetWindowSize_PointerParam(
                self.as_raw_mut(),
                callback,
            );
        }
    }

    /// Set the renderer window size callback. The vector is borrowed for the call.
    ///
    /// # Safety
    ///
    /// See [`Self::set_renderer_create_window`].
    pub unsafe fn set_renderer_set_window_size(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, &sys::ImVec2)>,
    ) {
        unsafe { self.set_renderer_set_window_size_raw(callback_abi::vec2_input(callback)) }
    }

    /// Set renderer render window callback (raw).
    ///
    /// # Safety
    ///
    /// See [`Self::set_renderer_create_window_raw`]. `render_arg` must satisfy the backend's
    /// contract whenever Dear ImGui supplies it.
    pub unsafe fn set_renderer_render_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)>,
    ) {
        self.inner_mut().Renderer_RenderWindow = callback;
    }

    /// Return the raw `Renderer_RenderWindow` callback.
    #[doc(hidden)]
    pub fn renderer_render_window_raw(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)> {
        self.inner().Renderer_RenderWindow
    }

    /// Set renderer render window callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_renderer_create_window`].
    pub unsafe fn set_renderer_render_window(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
    ) {
        unsafe { self.set_renderer_render_window_raw(callback_abi::render(callback)) }
    }

    /// Set renderer swap buffers callback (raw).
    ///
    /// # Safety
    ///
    /// See [`Self::set_renderer_create_window_raw`]. `render_arg` must satisfy the backend's
    /// contract whenever Dear ImGui supplies it.
    pub unsafe fn set_renderer_swap_buffers_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)>,
    ) {
        self.inner_mut().Renderer_SwapBuffers = callback;
    }

    /// Return the raw `Renderer_SwapBuffers` callback.
    #[doc(hidden)]
    pub fn renderer_swap_buffers_raw(
        &self,
    ) -> Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)> {
        self.inner().Renderer_SwapBuffers
    }

    /// Return whether the five renderer callback slots are empty.
    ///
    /// Renderer backends use this before installation so they never silently replace another
    /// backend's callback table.
    #[doc(hidden)]
    pub fn renderer_callbacks_are_empty(&self) -> bool {
        let raw = self.inner();
        raw.Renderer_CreateWindow.is_none()
            && raw.Renderer_DestroyWindow.is_none()
            && raw.Renderer_SetWindowSize.is_none()
            && raw.Renderer_RenderWindow.is_none()
            && raw.Renderer_SwapBuffers.is_none()
    }

    /// Return whether `Renderer_SetWindowSize` is still owned by an aggregate pointer callback.
    #[doc(hidden)]
    pub fn renderer_set_window_size_matches_pointer_callback(
        &self,
        callback: unsafe extern "C" fn(*mut sys::ImGuiViewport, *const sys::ImVec2),
    ) -> bool {
        unsafe {
            sys::ImGuiPlatformIO_RendererSetWindowSizeMatchesPointerParam(self.raw.get(), callback)
        }
    }

    /// Clear `Renderer_SetWindowSize` only when it is still owned by an aggregate pointer callback.
    ///
    /// Returns `true` when the live slot was cleared. The shim also releases saved storage for
    /// the departing callback if a native backend has already replaced the slot.
    ///
    /// # Safety
    ///
    /// Dear ImGui must no longer be able to invoke this callback, and all backend state it
    /// interprets must already have been released.
    #[doc(hidden)]
    pub unsafe fn clear_renderer_set_window_size_if_pointer_callback(
        &mut self,
        callback: unsafe extern "C" fn(*mut sys::ImGuiViewport, *const sys::ImVec2),
    ) -> bool {
        unsafe {
            sys::ImGuiPlatformIO_ClearRendererSetWindowSizeIfPointerParam(
                self.as_raw_mut(),
                callback,
            )
        }
    }

    /// Set renderer swap buffers callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_renderer_create_window`].
    pub unsafe fn set_renderer_swap_buffers(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
    ) {
        unsafe { self.set_renderer_swap_buffers_raw(callback_abi::render(callback)) }
    }
}

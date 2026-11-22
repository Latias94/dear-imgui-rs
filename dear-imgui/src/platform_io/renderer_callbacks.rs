use std::ffi::c_void;

use crate::sys;

use super::core::clear_renderer_aggregate_callbacks_for_platform_io;
use super::{PlatformIo, Viewport, callback_abi};

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
    #[cfg(feature = "multi-viewport")]
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
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_renderer_create_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.inner_mut().Renderer_CreateWindow = callback;
    }

    /// Return the raw `Renderer_CreateWindow` callback.
    #[cfg(feature = "multi-viewport")]
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
    #[cfg(feature = "multi-viewport")]
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
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_renderer_destroy_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.inner_mut().Renderer_DestroyWindow = callback;
    }

    /// Return the raw `Renderer_DestroyWindow` callback.
    #[cfg(feature = "multi-viewport")]
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
    #[cfg(feature = "multi-viewport")]
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
    #[cfg(feature = "multi-viewport")]
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
    #[cfg(feature = "multi-viewport")]
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
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_renderer_render_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)>,
    ) {
        self.inner_mut().Renderer_RenderWindow = callback;
    }

    /// Return the raw `Renderer_RenderWindow` callback.
    #[cfg(feature = "multi-viewport")]
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
    #[cfg(feature = "multi-viewport")]
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
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_renderer_swap_buffers_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)>,
    ) {
        self.inner_mut().Renderer_SwapBuffers = callback;
    }

    /// Return the raw `Renderer_SwapBuffers` callback.
    #[cfg(feature = "multi-viewport")]
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
    #[cfg(feature = "multi-viewport")]
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
    #[cfg(feature = "multi-viewport")]
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
    #[cfg(feature = "multi-viewport")]
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
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_renderer_swap_buffers(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, *mut c_void)>,
    ) {
        unsafe { self.set_renderer_swap_buffers_raw(callback_abi::render(callback)) }
    }
}

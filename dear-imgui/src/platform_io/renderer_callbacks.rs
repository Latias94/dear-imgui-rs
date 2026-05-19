use std::ffi::c_void;

use crate::sys;

use super::{PlatformIo, Viewport, trampolines};

impl PlatformIo {
    /// Clear all renderer backend handlers.
    ///
    /// This resets the `Renderer_*` callback table stored in `ImGuiPlatformIO`.
    /// This also clears Rust typed renderer callback storage for this `PlatformIo`'s context.
    #[cfg(feature = "multi-viewport")]
    pub fn clear_renderer_handlers(&mut self) {
        unsafe { sys::ImGuiPlatformIO_ClearRendererHandlers(self.as_raw_mut()) }

        trampolines::clear_renderer_callbacks_for_platform_io(self.as_raw());
    }

    /// Set renderer create window callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_renderer_create_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.inner_mut().Renderer_CreateWindow = callback;
        self.clear_platform_io_cb(&trampolines::RENDERER_CREATE_WINDOW_CB);
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
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_renderer_create_window_raw(callback.map(|_| {
            trampolines::renderer_create_window as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
        self.store_current_context_cb(&RENDERER_CREATE_WINDOW_CB, callback);
    }

    /// Set renderer destroy window callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_renderer_destroy_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport)>,
    ) {
        self.inner_mut().Renderer_DestroyWindow = callback;
        self.clear_platform_io_cb(&trampolines::RENDERER_DESTROY_WINDOW_CB);
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
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_renderer_destroy_window_raw(callback.map(|_| {
            trampolines::renderer_destroy_window as unsafe extern "C" fn(*mut sys::ImGuiViewport)
        }));
        self.store_current_context_cb(&RENDERER_DESTROY_WINDOW_CB, callback);
    }

    /// Set renderer set window size callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_renderer_set_window_size_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2)>,
    ) {
        self.inner_mut().Renderer_SetWindowSize = callback;
        self.clear_platform_io_cb(&trampolines::RENDERER_SET_WINDOW_SIZE_CB);
    }

    /// Set renderer set window size callback (typed Viewport).
    ///
    /// # Safety
    ///
    /// See [`Self::set_renderer_create_window`].
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_renderer_set_window_size(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut Viewport, sys::ImVec2)>,
    ) {
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_renderer_set_window_size_raw(callback.map(|_| {
            trampolines::renderer_set_window_size
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2)
        }));
        self.store_current_context_cb(&RENDERER_SET_WINDOW_SIZE_CB, callback);
    }

    /// Set renderer render window callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_renderer_render_window_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)>,
    ) {
        self.inner_mut().Renderer_RenderWindow = callback;
        self.clear_platform_io_cb(&trampolines::RENDERER_RENDER_WINDOW_CB);
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
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_renderer_render_window_raw(callback.map(|_| {
            trampolines::renderer_render_window
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)
        }));
        self.store_current_context_cb(&RENDERER_RENDER_WINDOW_CB, callback);
    }

    /// Set renderer swap buffers callback (raw)
    #[cfg(feature = "multi-viewport")]
    pub fn set_renderer_swap_buffers_raw(
        &mut self,
        callback: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)>,
    ) {
        self.inner_mut().Renderer_SwapBuffers = callback;
        self.clear_platform_io_cb(&trampolines::RENDERER_SWAP_BUFFERS_CB);
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
        self.assert_current_context_platform_io_for_callbacks();
        use trampolines::*;
        self.set_renderer_swap_buffers_raw(callback.map(|_| {
            trampolines::renderer_swap_buffers
                as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void)
        }));
        self.store_current_context_cb(&RENDERER_SWAP_BUFFERS_CB, callback);
    }
}

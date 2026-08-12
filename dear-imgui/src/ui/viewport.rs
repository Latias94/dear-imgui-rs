use super::*;

impl Ui {
    /// Returns a reference to the main Dear ImGui viewport (safe wrapper)
    ///
    /// Same viewport used by [`Ui::dockspace`](crate::Ui::dockspace)'s default host.
    ///
    /// The returned reference is owned by the currently active ImGui context and
    /// must not be used after the context is destroyed.
    #[doc(alias = "GetMainViewport")]
    pub fn main_viewport(&self) -> &crate::platform_io::Viewport {
        self.run_with_bound_context(|| unsafe {
            let ptr = sys::igGetMainViewport();
            if ptr.is_null() {
                panic!("Ui::main_viewport() requires an active ImGui context");
            }
            crate::platform_io::Viewport::from_raw(ptr as *const sys::ImGuiViewport)
        })
    }

    /// Set the viewport for the next window.
    ///
    /// This is a convenience wrapper over `ImGui::SetNextWindowViewport`.
    /// Useful when hosting a fullscreen DockSpace window inside the main viewport.
    #[doc(alias = "SetNextWindowViewport")]
    pub fn set_next_window_viewport(&self, viewport_id: Id) {
        self.run_with_bound_context(|| unsafe { sys::igSetNextWindowViewport(viewport_id.into()) });
    }

    /// Returns the viewport of the current window.
    ///
    /// Dear ImGui keeps an implicit fallback window current for the duration of an open frame, so
    /// this may be called before, after, or outside an explicit user window's `Begin`/`End` scope.
    /// Inside an explicit window it returns that window's viewport; after the window ends it
    /// returns the fallback window's viewport again.
    ///
    /// # Panics
    ///
    /// Panics if raw/native API use has cleared the current viewport while this [`Ui`] still
    /// represents an active frame.
    #[doc(alias = "GetWindowViewport")]
    pub fn window_viewport(&self) -> &crate::platform_io::Viewport {
        self.run_with_bound_context(|| unsafe {
            let ptr = sys::igGetWindowViewport();
            if ptr.is_null() {
                panic!("Ui::window_viewport() requires a current window");
            }
            crate::platform_io::Viewport::from_raw(ptr as *const sys::ImGuiViewport)
        })
    }

    /// Find a viewport by ID.
    #[doc(alias = "FindViewportByID")]
    pub fn find_viewport_by_id(&self, viewport_id: Id) -> Option<&crate::platform_io::Viewport> {
        self.run_with_bound_context(|| unsafe {
            let ptr = sys::igFindViewportByID(viewport_id.raw());
            if ptr.is_null() {
                None
            } else {
                Some(crate::platform_io::Viewport::from_raw(
                    ptr as *const sys::ImGuiViewport,
                ))
            }
        })
    }

    /// Find a viewport by its platform handle.
    ///
    /// The platform handle type depends on the backend (e.g. `HWND` on Windows).
    #[doc(alias = "FindViewportByPlatformHandle")]
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn find_viewport_by_platform_handle(
        &self,
        platform_handle: *mut std::ffi::c_void,
    ) -> Option<&crate::platform_io::Viewport> {
        self.run_with_bound_context(|| unsafe {
            let ptr = sys::igFindViewportByPlatformHandle(platform_handle);
            if ptr.is_null() {
                None
            } else {
                Some(crate::platform_io::Viewport::from_raw(
                    ptr as *const sys::ImGuiViewport,
                ))
            }
        })
    }
}

use crate::sys;

use super::Context;
use super::binding::{CTX_MUTEX, with_bound_context};

impl Context {
    /// Get shared access to the platform IO.
    ///
    /// Note: `ImGuiPlatformIO` exists even when multi-viewport is disabled. We expose it
    /// unconditionally so callers can use ImGui 1.92+ texture management via `PlatformIO.Textures[]`.
    pub fn platform_io(&self) -> &crate::platform_io::PlatformIo {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            let pio = self.platform_io_ptr("Context::platform_io()");
            crate::platform_io::PlatformIo::from_raw(pio)
        }
    }

    /// Get mutable access to the platform IO.
    ///
    /// Note: `ImGuiPlatformIO` exists even when multi-viewport is disabled. We expose it
    /// unconditionally so callers can use ImGui 1.92+ texture management via `PlatformIO.Textures[]`.
    pub fn platform_io_mut(&mut self) -> &mut crate::platform_io::PlatformIo {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            let pio = self.platform_io_ptr("Context::platform_io_mut()");
            crate::platform_io::PlatformIo::from_raw_mut(pio)
        }
    }

    /// Returns a reference to the main Dear ImGui viewport.
    ///
    /// The returned reference is owned by this ImGui context and
    /// must not be used after the context is destroyed.
    #[doc(alias = "GetMainViewport")]
    pub fn main_viewport(&mut self) -> &crate::platform_io::Viewport {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            with_bound_context(self.raw, || {
                let ptr = sys::igGetMainViewport();
                if ptr.is_null() {
                    panic!("Context::main_viewport() requires a valid ImGui context");
                }
                crate::platform_io::Viewport::from_raw(ptr as *const sys::ImGuiViewport)
            })
        }
    }

    /// Enable multi-viewport support flags
    #[cfg(feature = "multi-viewport")]
    pub fn enable_multi_viewport(&mut self) {
        // Enable viewport flags
        crate::viewport_backend::utils::enable_viewport_flags(self.io_mut());
    }

    /// Update platform windows
    ///
    /// This function should be called every frame when multi-viewport is enabled.
    /// It updates all platform windows and handles viewport management.
    #[cfg(feature = "multi-viewport")]
    pub fn update_platform_windows(&mut self) {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            with_bound_context(self.raw, || {
                // Ensure main viewport is properly set up before updating platform windows
                let main_viewport = sys::igGetMainViewport();
                if !main_viewport.is_null() && (*main_viewport).PlatformHandle.is_null() {
                    eprintln!(
                        "update_platform_windows: main viewport not set up, setting it up now"
                    );
                    // The main viewport needs to be set up - this should be done by the backend
                    // For now, we'll just log this and continue
                }

                sys::igUpdatePlatformWindows();
            });
        }
    }

    /// Render platform windows with default implementation
    ///
    /// This function renders all platform windows using the default implementation.
    /// It calls the platform and renderer backends to render each viewport.
    #[cfg(feature = "multi-viewport")]
    pub fn render_platform_windows_default(&mut self) {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            with_bound_context(self.raw, || {
                sys::igRenderPlatformWindowsDefault(std::ptr::null_mut(), std::ptr::null_mut());
            });
        }
    }

    /// Destroy all platform windows
    ///
    /// This function should be called during shutdown to properly clean up
    /// all platform windows and their associated resources.
    #[cfg(feature = "multi-viewport")]
    pub fn destroy_platform_windows(&mut self) {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            with_bound_context(self.raw, || {
                sys::igDestroyPlatformWindows();
            });
        }
    }
}

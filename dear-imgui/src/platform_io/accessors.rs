use crate::sys;

use super::{PlatformIo, Viewport};

impl PlatformIo {
    /// Get access to the monitors vector
    #[cfg(feature = "multi-viewport")]
    pub fn monitors(&self) -> &crate::internal::ImVector<sys::ImGuiPlatformMonitor> {
        unsafe {
            crate::internal::imvector_cast_ref::<
                sys::ImGuiPlatformMonitor,
                sys::ImVector_ImGuiPlatformMonitor,
            >(&self.inner().Monitors)
        }
    }

    /// Get mutable access to the monitors vector
    #[cfg(feature = "multi-viewport")]
    pub fn monitors_mut(&mut self) -> &mut crate::internal::ImVector<sys::ImGuiPlatformMonitor> {
        unsafe {
            crate::internal::imvector_cast_mut::<
                sys::ImGuiPlatformMonitor,
                sys::ImVector_ImGuiPlatformMonitor,
            >(&mut self.inner_mut().Monitors)
        }
    }

    /// Get access to the viewports vector
    #[cfg(feature = "multi-viewport")]
    pub(crate) fn viewports(&self) -> &crate::internal::ImVector<*mut sys::ImGuiViewport> {
        unsafe {
            crate::internal::imvector_cast_ref::<
                *mut sys::ImGuiViewport,
                sys::ImVector_ImGuiViewportPtr,
            >(&self.inner().Viewports)
        }
    }

    /// Get mutable access to the viewports vector
    #[cfg(feature = "multi-viewport")]
    pub(crate) fn viewports_mut(
        &mut self,
    ) -> &mut crate::internal::ImVector<*mut sys::ImGuiViewport> {
        unsafe {
            crate::internal::imvector_cast_mut::<
                *mut sys::ImGuiViewport,
                sys::ImVector_ImGuiViewportPtr,
            >(&mut self.inner_mut().Viewports)
        }
    }

    /// Get an iterator over all viewports
    #[cfg(feature = "multi-viewport")]
    pub fn viewports_iter(&self) -> impl Iterator<Item = &Viewport> {
        self.viewports()
            .iter()
            .map(|&ptr| unsafe { Viewport::from_raw(ptr) })
    }

    /// Get a mutable iterator over all viewports
    #[cfg(feature = "multi-viewport")]
    pub fn viewports_iter_mut(&mut self) -> impl Iterator<Item = &mut Viewport> {
        self.viewports_mut()
            .iter_mut()
            .map(|&mut ptr| unsafe { Viewport::from_raw_mut(ptr) })
    }
}

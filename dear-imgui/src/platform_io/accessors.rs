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

    /// Replace Dear ImGui's platform monitor list.
    ///
    /// Multi-viewport backends must keep at least one monitor in `PlatformIO.Monitors` before
    /// enabling `ConfigFlags::VIEWPORTS_ENABLE`. The vector storage is allocated with Dear ImGui's
    /// allocator so the context can safely own and release it.
    #[cfg(feature = "multi-viewport")]
    pub fn set_monitors(&mut self, monitors: &[sys::ImGuiPlatformMonitor]) {
        let raw = &mut self.inner_mut().Monitors;
        if !raw.Data.is_null() {
            unsafe { sys::igMemFree(raw.Data.cast()) };
            raw.Data = std::ptr::null_mut();
        }
        raw.Size = 0;
        raw.Capacity = 0;

        if monitors.is_empty() {
            return;
        }

        let byte_len = std::mem::size_of_val(monitors);
        let data = unsafe { sys::igMemAlloc(byte_len) }.cast::<sys::ImGuiPlatformMonitor>();
        assert!(
            !data.is_null(),
            "PlatformIo::set_monitors() failed to allocate monitor storage"
        );
        unsafe {
            data.copy_from_nonoverlapping(monitors.as_ptr(), monitors.len());
        }
        raw.Data = data;
        raw.Size = monitors
            .len()
            .try_into()
            .expect("PlatformIo::set_monitors() supports at most i32::MAX monitors");
        raw.Capacity = raw.Size;
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

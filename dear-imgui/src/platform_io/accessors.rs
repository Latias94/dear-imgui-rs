use crate::sys;

use super::{PlatformIo, Viewport};

impl PlatformIo {
    /// Replace Dear ImGui's platform monitor list.
    ///
    /// Multi-viewport backends must keep at least one monitor in `PlatformIO.Monitors` before
    /// enabling `ConfigFlags::VIEWPORTS_ENABLE`. The vector storage is allocated with Dear ImGui's
    /// allocator so the context can safely own and release it.
    ///
    /// # Safety
    ///
    /// Every non-null `PlatformHandle` must use the representation expected by the installed
    /// platform callbacks and remain valid for as long as the monitor entry can be observed.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn set_monitors(&mut self, monitors: &[sys::ImGuiPlatformMonitor]) {
        assert_monitor_contract(monitors, "PlatformIo::set_monitors()");
        let count = i32::try_from(monitors.len())
            .expect("PlatformIo::set_monitors() supports at most i32::MAX monitors");

        // Allocate the replacement before releasing the old vector so a failed allocation leaves
        // the previously valid monitor contract intact.
        let data = if monitors.is_empty() {
            std::ptr::null_mut()
        } else {
            let byte_len = std::mem::size_of_val(monitors);
            let data = unsafe { sys::igMemAlloc(byte_len) }.cast::<sys::ImGuiPlatformMonitor>();
            assert!(
                !data.is_null(),
                "PlatformIo::set_monitors() failed to allocate monitor storage"
            );
            unsafe {
                data.copy_from_nonoverlapping(monitors.as_ptr(), monitors.len());
            }
            data
        };

        let raw = &mut self.inner_mut().Monitors;
        if !raw.Data.is_null() {
            unsafe { sys::igMemFree(raw.Data.cast()) };
        }
        raw.Data = data;
        raw.Size = count;
        raw.Capacity = count;
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

pub(crate) fn assert_monitor_contract(monitors: &[sys::ImGuiPlatformMonitor], caller: &str) {
    for (index, monitor) in monitors.iter().enumerate() {
        let values = [
            monitor.MainPos.x,
            monitor.MainPos.y,
            monitor.MainSize.x,
            monitor.MainSize.y,
            monitor.WorkPos.x,
            monitor.WorkPos.y,
            monitor.WorkSize.x,
            monitor.WorkSize.y,
            monitor.DpiScale,
        ];
        assert!(
            values.iter().all(|value| value.is_finite()),
            "{caller} rejected monitor {index}: geometry and DPI values must be finite"
        );
        assert!(
            monitor.MainSize.x > 0.0 && monitor.MainSize.y > 0.0,
            "{caller} rejected monitor {index}: MainSize must be positive"
        );
        assert!(
            monitor.WorkSize.x >= 0.0 && monitor.WorkSize.y >= 0.0,
            "{caller} rejected monitor {index}: WorkSize must not be negative"
        );

        let main_max = [
            monitor.MainPos.x + monitor.MainSize.x,
            monitor.MainPos.y + monitor.MainSize.y,
        ];
        let work_max = [
            monitor.WorkPos.x + monitor.WorkSize.x,
            monitor.WorkPos.y + monitor.WorkSize.y,
        ];
        assert!(
            main_max
                .iter()
                .chain(work_max.iter())
                .all(|value| value.is_finite()),
            "{caller} rejected monitor {index}: geometry bounds must not overflow"
        );
        assert!(
            monitor.WorkPos.x >= monitor.MainPos.x
                && monitor.WorkPos.y >= monitor.MainPos.y
                && work_max[0] <= main_max[0]
                && work_max[1] <= main_max[1],
            "{caller} rejected monitor {index}: WorkPos/WorkSize must be contained within MainPos/MainSize"
        );
        assert!(
            monitor.DpiScale > 0.0 && monitor.DpiScale < 99.0,
            "{caller} rejected monitor {index}: DpiScale must be greater than 0 and less than 99"
        );
    }
}

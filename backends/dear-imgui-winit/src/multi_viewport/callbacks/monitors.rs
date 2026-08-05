use super::*;

#[derive(Clone, Copy)]
struct MonitorVectorState {
    data: *mut dear_imgui_rs::sys::ImGuiPlatformMonitor,
    size: i32,
    capacity: i32,
}

impl MonitorVectorState {
    unsafe fn from_platform_io(raw: *mut dear_imgui_rs::sys::ImGuiPlatformIO) -> Self {
        let monitors = unsafe { &(*raw).Monitors };
        Self {
            data: monitors.Data,
            size: monitors.Size,
            capacity: monitors.Capacity,
        }
    }

    unsafe fn install_into(self, raw: *mut dear_imgui_rs::sys::ImGuiPlatformIO) {
        let monitors = unsafe { &mut (*raw).Monitors };
        monitors.Data = self.data;
        monitors.Size = self.size;
        monitors.Capacity = self.capacity;
    }

    unsafe fn matches(self, raw: *mut dear_imgui_rs::sys::ImGuiPlatformIO) -> bool {
        let monitors = unsafe { &(*raw).Monitors };
        monitors.Data == self.data
            && monitors.Size == self.size
            && monitors.Capacity == self.capacity
    }

    unsafe fn free(self) {
        if !self.data.is_null() {
            unsafe { dear_imgui_rs::sys::igMemFree(self.data.cast()) };
        }
    }
}

pub(in super::super) struct PreparedMonitors {
    storage: Option<MonitorVectorState>,
}

impl PreparedMonitors {
    fn allocate(
        context: &Context,
        monitors: &[dear_imgui_rs::sys::ImGuiPlatformMonitor],
    ) -> Result<Self, WinitPlatformError> {
        validate_monitors(monitors)?;
        let count =
            i32::try_from(monitors.len()).map_err(|_| WinitPlatformError::MonitorCountOverflow)?;
        let byte_len = std::mem::size_of_val(monitors);
        let data = context.binding().with_bound_context(|| unsafe {
            dear_imgui_rs::sys::igMemAlloc(byte_len)
                .cast::<dear_imgui_rs::sys::ImGuiPlatformMonitor>()
        });
        if data.is_null() {
            return Err(WinitPlatformError::MonitorStorageAllocationFailed);
        }
        unsafe { data.copy_from_nonoverlapping(monitors.as_ptr(), monitors.len()) };
        Ok(Self {
            storage: Some(MonitorVectorState {
                data,
                size: count,
                capacity: count,
            }),
        })
    }

    fn take_storage(&mut self) -> MonitorVectorState {
        self.storage
            .take()
            .expect("prepared monitor storage can only be published once")
    }
}

impl Drop for PreparedMonitors {
    fn drop(&mut self) {
        if let Some(storage) = self.storage.take() {
            unsafe { storage.free() };
        }
    }
}

pub(in super::super) struct MonitorOwnership {
    prior: MonitorVectorState,
    installed: MonitorVectorState,
}

impl MonitorOwnership {
    pub(in super::super) unsafe fn installed_matches(
        &self,
        raw: *mut dear_imgui_rs::sys::ImGuiPlatformIO,
    ) -> bool {
        unsafe { self.installed.matches(raw) }
    }

    unsafe fn installed_equals(
        &self,
        raw: *mut dear_imgui_rs::sys::ImGuiPlatformIO,
        monitors: &[dear_imgui_rs::sys::ImGuiPlatformMonitor],
    ) -> Result<bool, WinitPlatformError> {
        if !unsafe { self.installed.matches(raw) } {
            return Err(WinitPlatformError::PlatformStateReplaced {
                field: "PlatformIO.Monitors",
            });
        }
        let count = usize::try_from(self.installed.size).map_err(|_| {
            WinitPlatformError::PlatformStateReplaced {
                field: "PlatformIO.Monitors",
            }
        })?;
        if count != monitors.len() {
            return Ok(false);
        }
        if count == 0 {
            return Ok(true);
        }
        if self.installed.data.is_null() {
            return Err(WinitPlatformError::PlatformStateReplaced {
                field: "PlatformIO.Monitors",
            });
        }
        let installed = unsafe { std::slice::from_raw_parts(self.installed.data, count) };
        Ok(installed == monitors)
    }

    unsafe fn replace_installed(
        &mut self,
        raw: *mut dear_imgui_rs::sys::ImGuiPlatformIO,
        mut prepared: PreparedMonitors,
    ) -> Result<(), WinitPlatformError> {
        if !unsafe { self.installed.matches(raw) } {
            return Err(WinitPlatformError::PlatformStateReplaced {
                field: "PlatformIO.Monitors",
            });
        }
        let replacement = prepared.take_storage();
        unsafe { replacement.install_into(raw) };
        let previous = std::mem::replace(&mut self.installed, replacement);
        unsafe { previous.free() };
        Ok(())
    }

    pub(in super::super) unsafe fn restore_if_owned(
        self,
        raw: *mut dear_imgui_rs::sys::ImGuiPlatformIO,
    ) {
        if unsafe { self.installed.matches(raw) } {
            unsafe { self.prior.install_into(raw) };
            unsafe { self.installed.free() };
        } else if unsafe { self.prior.matches(raw) } {
            // An allocator-aware foreign replacement may have freed Winit's allocation before
            // reproducing the prior state (most commonly the empty vector). It is therefore not
            // safe to free the detached pointer again. A direct raw replacement can leak Winit's
            // allocation, but never turns uncertain ownership into a double free.
        } else {
            // A foreign owner replaced the vector through the allocator-aware API. That operation
            // released our installed allocation, so only the detached prior allocation remains.
            unsafe { self.prior.free() };
        }
    }

    pub(in super::super) unsafe fn context_destroyed(self) {
        // Dear ImGui released whichever vector remained installed. The prior allocation was
        // detached from native ownership when Winit published its monitor list.
        unsafe { self.prior.free() };
    }
}

pub(in super::super) fn prepare_monitors(
    context: &Context,
    window: &winit::window::Window,
) -> Result<PreparedMonitors, WinitPlatformError> {
    let monitors = collect_monitors(window);
    PreparedMonitors::allocate(context, &monitors)
}

fn move_primary_to_front<T: Eq>(monitors: &mut Vec<T>, primary: Option<T>) {
    let Some(primary) = primary else {
        return;
    };
    if let Some(index) = monitors.iter().position(|monitor| *monitor == primary) {
        let primary = monitors.remove(index);
        monitors.insert(0, primary);
    } else {
        monitors.insert(0, primary);
    }
}

pub(in super::super) fn collect_monitors(
    window: &winit::window::Window,
) -> Vec<dear_imgui_rs::sys::ImGuiPlatformMonitor> {
    let mut monitor_handles = window.available_monitors().collect::<Vec<_>>();
    monitor_handles.sort_by_key(|monitor| {
        let position = monitor.position();
        let size = monitor.size();
        (
            position.x,
            position.y,
            size.width,
            size.height,
            monitor.name(),
        )
    });
    move_primary_to_front(&mut monitor_handles, window.primary_monitor());
    let mut monitors = monitor_handles
        .into_iter()
        .map(|monitor| {
            monitor_from_physical(monitor.position(), monitor.size(), monitor.scale_factor())
        })
        .collect::<Vec<_>>();
    if monitors.is_empty() {
        monitors.push(monitor_from_physical(
            PhysicalPosition::new(0, 0),
            window.inner_size(),
            window.scale_factor(),
        ));
    }
    monitors
}

pub(in super::super) fn refresh_monitors(
    context: &Context,
    window: &winit::window::Window,
    ownership: &mut MonitorOwnership,
) -> Result<bool, WinitPlatformError> {
    let monitors = collect_monitors(window);
    refresh_published_monitors(context, &monitors, ownership)
}

fn refresh_published_monitors(
    context: &Context,
    monitors: &[dear_imgui_rs::sys::ImGuiPlatformMonitor],
    ownership: &mut MonitorOwnership,
) -> Result<bool, WinitPlatformError> {
    validate_monitors(monitors)?;
    let raw = unsafe { dear_imgui_rs::sys::igGetPlatformIO_Nil() };
    if raw.is_null() {
        return Err(WinitPlatformError::ContextMismatch);
    }
    if unsafe { ownership.installed_equals(raw, monitors)? } {
        return Ok(false);
    }
    let prepared = PreparedMonitors::allocate(context, monitors)?;
    unsafe { ownership.replace_installed(raw, prepared)? };
    Ok(true)
}

#[cfg(test)]
pub(in super::super) fn refresh_monitors_for_test(
    context: &Context,
    monitors: &[dear_imgui_rs::sys::ImGuiPlatformMonitor],
    ownership: &mut MonitorOwnership,
) -> Result<bool, WinitPlatformError> {
    refresh_published_monitors(context, monitors, ownership)
}

#[cfg(test)]
pub(in super::super) fn prepare_monitors_for_test(
    context: &Context,
    monitors: Vec<dear_imgui_rs::sys::ImGuiPlatformMonitor>,
) -> Result<PreparedMonitors, WinitPlatformError> {
    PreparedMonitors::allocate(context, &monitors)
}

pub(in super::super) fn publish_monitors(
    context: &mut Context,
    mut prepared: PreparedMonitors,
) -> MonitorOwnership {
    context.binding().with_bound_context(|| unsafe {
        let raw = context.platform_io_mut().as_raw_mut();
        let prior = MonitorVectorState::from_platform_io(raw);
        let installed = prepared.take_storage();
        installed.install_into(raw);
        MonitorOwnership { prior, installed }
    })
}

fn validate_monitors(
    monitors: &[dear_imgui_rs::sys::ImGuiPlatformMonitor],
) -> Result<(), WinitPlatformError> {
    if monitors.is_empty() {
        return Err(WinitPlatformError::NoMonitors);
    }
    for (monitor, value) in monitors.iter().enumerate() {
        let values = [
            value.MainPos.x,
            value.MainPos.y,
            value.MainSize.x,
            value.MainSize.y,
            value.WorkPos.x,
            value.WorkPos.y,
            value.WorkSize.x,
            value.WorkSize.y,
            value.DpiScale,
        ];
        if !values.iter().all(|value| value.is_finite()) {
            return Err(WinitPlatformError::InvalidMonitorGeometry {
                monitor,
                reason: "geometry and DPI values must be finite",
            });
        }
        if value.MainSize.x <= 0.0 || value.MainSize.y <= 0.0 {
            return Err(WinitPlatformError::InvalidMonitorGeometry {
                monitor,
                reason: "MainSize must be positive",
            });
        }
        if value.WorkSize.x < 0.0 || value.WorkSize.y < 0.0 {
            return Err(WinitPlatformError::InvalidMonitorGeometry {
                monitor,
                reason: "WorkSize must not be negative",
            });
        }

        let main_max = [
            value.MainPos.x + value.MainSize.x,
            value.MainPos.y + value.MainSize.y,
        ];
        let work_max = [
            value.WorkPos.x + value.WorkSize.x,
            value.WorkPos.y + value.WorkSize.y,
        ];
        if !main_max
            .iter()
            .chain(work_max.iter())
            .all(|value| value.is_finite())
        {
            return Err(WinitPlatformError::InvalidMonitorGeometry {
                monitor,
                reason: "geometry bounds must not overflow",
            });
        }
        if value.WorkPos.x < value.MainPos.x
            || value.WorkPos.y < value.MainPos.y
            || work_max[0] > main_max[0]
            || work_max[1] > main_max[1]
        {
            return Err(WinitPlatformError::InvalidMonitorGeometry {
                monitor,
                reason: "work area must be contained within the main area",
            });
        }
        if value.DpiScale <= 0.0 || value.DpiScale >= 99.0 {
            return Err(WinitPlatformError::InvalidMonitorGeometry {
                monitor,
                reason: "DpiScale must be greater than 0 and less than 99",
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::move_primary_to_front;

    #[test]
    fn primary_monitor_is_first_without_duplication() {
        let mut monitors = vec![2, 1, 3];
        move_primary_to_front(&mut monitors, Some(1));
        assert_eq!(monitors, vec![1, 2, 3]);

        move_primary_to_front(&mut monitors, Some(4));
        assert_eq!(monitors, vec![4, 1, 2, 3]);

        move_primary_to_front(&mut monitors, Some(3));
        assert_eq!(monitors, vec![3, 4, 1, 2]);
    }
}

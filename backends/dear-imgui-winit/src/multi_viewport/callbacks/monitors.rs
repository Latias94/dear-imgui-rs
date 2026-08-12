use super::super::coordinates::monitor_from_snapshot;
use super::*;
use crate::native_support::{MonitorSnapshot, collect_monitor_snapshot_set};
use std::cmp::Ordering;

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
    facts: Option<Vec<MonitorSnapshot>>,
    values: Vec<dear_imgui_rs::sys::ImGuiPlatformMonitor>,
}

impl PreparedMonitors {
    fn allocate(
        context: &Context,
        facts: Option<Vec<MonitorSnapshot>>,
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
            facts,
            values: monitors.to_vec(),
        })
    }

    fn take_storage(&mut self) -> MonitorVectorState {
        self.storage
            .take()
            .expect("prepared monitor storage can only be published once")
    }

    fn take_publication(
        &mut self,
    ) -> (
        MonitorVectorState,
        Option<Vec<MonitorSnapshot>>,
        Vec<dear_imgui_rs::sys::ImGuiPlatformMonitor>,
    ) {
        (
            self.take_storage(),
            self.facts.take(),
            std::mem::take(&mut self.values),
        )
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
    facts: Option<Vec<MonitorSnapshot>>,
    values: Vec<dear_imgui_rs::sys::ImGuiPlatformMonitor>,
}

impl MonitorOwnership {
    pub(in super::super) unsafe fn installed_matches(
        &self,
        raw: *mut dear_imgui_rs::sys::ImGuiPlatformIO,
    ) -> bool {
        unsafe { self.installed.matches(raw) }
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
        let (replacement, facts, values) = prepared.take_publication();
        unsafe { replacement.install_into(raw) };
        let previous = std::mem::replace(&mut self.installed, replacement);
        self.facts = facts;
        self.values = values;
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
    let (facts, monitors) = match collect_monitor_publication(window) {
        MonitorCollection::Available(publication) => (publication.facts, publication.values),
        MonitorCollection::Unavailable => (None, fallback_monitors(window)),
    };
    PreparedMonitors::allocate(context, facts, &monitors)
}

fn fallback_monitors(
    window: &winit::window::Window,
) -> Vec<dear_imgui_rs::sys::ImGuiPlatformMonitor> {
    vec![monitor_from_physical(
        PhysicalPosition::new(0, 0),
        window.inner_size(),
        window.scale_factor(),
    )]
}

#[derive(Clone, Debug, PartialEq)]
struct MonitorPublication {
    facts: Option<Vec<MonitorSnapshot>>,
    values: Vec<dear_imgui_rs::sys::ImGuiPlatformMonitor>,
}

enum MonitorCollection {
    Available(MonitorPublication),
    Unavailable,
}

fn snapshot_order(left: &MonitorSnapshot, right: &MonitorSnapshot) -> Ordering {
    left.identity()
        .cmp(right.identity())
        .then_with(|| compare_f64_pair(left.main().position(), right.main().position()))
        .then_with(|| compare_f64_pair(left.main().size(), right.main().size()))
        .then_with(|| left.scale_factor().total_cmp(&right.scale_factor()))
}

fn compare_f64_pair(left: [f64; 2], right: [f64; 2]) -> Ordering {
    left[0]
        .total_cmp(&right[0])
        .then_with(|| left[1].total_cmp(&right[1]))
}

fn normalize_snapshots(
    mut snapshots: Vec<MonitorSnapshot>,
    primary: Option<&crate::native_support::MonitorIdentity>,
) -> Vec<MonitorSnapshot> {
    snapshots.sort_by(snapshot_order);
    // Only remove exact duplicate facts. Detached fallback identities can collide for identical
    // displays; dropping a distinct work rectangle would silently lose native evidence.
    snapshots.dedup_by(|left, right| left == right);
    if let Some(primary) = primary
        && let Some(index) = snapshots
            .iter()
            .position(|snapshot| snapshot.identity() == primary)
    {
        let primary = snapshots.remove(index);
        snapshots.insert(0, primary);
    }
    snapshots
}

fn collect_monitor_publication(window: &winit::window::Window) -> MonitorCollection {
    let Ok(publication) = collect_monitor_snapshot_set(window) else {
        return MonitorCollection::Unavailable;
    };
    let (snapshots, primary) = publication.into_parts();
    monitor_collection_from_snapshots(snapshots, primary.as_ref())
}

fn monitor_collection_from_snapshots(
    snapshots: Vec<MonitorSnapshot>,
    primary: Option<&crate::native_support::MonitorIdentity>,
) -> MonitorCollection {
    if snapshots.is_empty() {
        return MonitorCollection::Unavailable;
    }
    let snapshots = normalize_snapshots(snapshots, primary);
    if snapshots.is_empty() {
        return MonitorCollection::Unavailable;
    }
    let Some(values) = snapshots
        .iter()
        .map(monitor_from_snapshot)
        .collect::<Option<Vec<_>>>()
    else {
        return MonitorCollection::Unavailable;
    };
    if validate_monitors(&values).is_err() {
        return MonitorCollection::Unavailable;
    }
    MonitorCollection::Available(MonitorPublication {
        facts: Some(snapshots),
        values,
    })
}

pub(in super::super) fn refresh_monitors(
    context: &Context,
    window: &winit::window::Window,
    ownership: &mut MonitorOwnership,
) -> Result<bool, WinitPlatformError> {
    refresh_monitor_collection(context, collect_monitor_publication(window), ownership)
}

fn refresh_monitor_collection(
    context: &Context,
    collection: MonitorCollection,
    ownership: &mut MonitorOwnership,
) -> Result<bool, WinitPlatformError> {
    let MonitorCollection::Available(publication) = collection else {
        return Ok(false);
    };
    refresh_published_monitors(context, publication, ownership)
}

fn refresh_published_monitors(
    context: &Context,
    publication: MonitorPublication,
    ownership: &mut MonitorOwnership,
) -> Result<bool, WinitPlatformError> {
    validate_monitors(&publication.values)?;
    let raw = unsafe { dear_imgui_rs::sys::igGetPlatformIO_Nil() };
    if raw.is_null() {
        return Err(WinitPlatformError::ContextMismatch);
    }
    if !unsafe { ownership.installed.matches(raw) } {
        return Err(WinitPlatformError::PlatformStateReplaced {
            field: "PlatformIO.Monitors",
        });
    }
    if ownership.facts == publication.facts && ownership.values == publication.values {
        return Ok(false);
    }
    let prepared = PreparedMonitors::allocate(context, publication.facts, &publication.values)?;
    unsafe { ownership.replace_installed(raw, prepared)? };
    Ok(true)
}

#[cfg(test)]
pub(in super::super) fn refresh_monitors_for_test(
    context: &Context,
    monitors: &[dear_imgui_rs::sys::ImGuiPlatformMonitor],
    ownership: &mut MonitorOwnership,
) -> Result<bool, WinitPlatformError> {
    refresh_published_monitors(
        context,
        MonitorPublication {
            facts: None,
            values: monitors.to_vec(),
        },
        ownership,
    )
}

#[cfg(test)]
pub(in super::super) fn refresh_monitor_snapshots_for_test(
    context: &Context,
    snapshots: Option<Vec<MonitorSnapshot>>,
    ownership: &mut MonitorOwnership,
) -> Result<bool, WinitPlatformError> {
    let collection = snapshots
        .map(|snapshots| monitor_collection_from_snapshots(snapshots, None))
        .unwrap_or(MonitorCollection::Unavailable);
    refresh_monitor_collection(context, collection, ownership)
}

#[cfg(test)]
pub(in super::super) fn prepare_monitors_for_test(
    context: &Context,
    monitors: Vec<dear_imgui_rs::sys::ImGuiPlatformMonitor>,
) -> Result<PreparedMonitors, WinitPlatformError> {
    PreparedMonitors::allocate(context, None, &monitors)
}

pub(in super::super) fn publish_monitors(
    context: &mut Context,
    mut prepared: PreparedMonitors,
) -> MonitorOwnership {
    context.binding().with_bound_context(|| unsafe {
        let raw = context.platform_io_mut().as_raw_mut();
        let prior = MonitorVectorState::from_platform_io(raw);
        let (installed, facts, values) = prepared.take_publication();
        installed.install_into(raw);
        MonitorOwnership {
            prior,
            installed,
            facts,
            values,
        }
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
    use super::{MonitorSnapshot, normalize_snapshots};
    use crate::native_support::{
        MonitorIdentity, PhysicalMonitorRect, WorkAreaFallback, WorkAreaProvenance,
    };

    #[test]
    fn detached_primary_identity_is_promoted_without_fabrication() {
        let main = PhysicalMonitorRect::new([0.0, 0.0], [1920.0, 1080.0]).unwrap();
        let primary = MonitorSnapshot::from_test(
            MonitorIdentity::from_test_key("primary"),
            main,
            main,
            1.0,
            WorkAreaProvenance::FullMain(WorkAreaFallback::SourceUnavailable),
        );
        let secondary_main = PhysicalMonitorRect::new([1920.0, 0.0], [1920.0, 1080.0]).unwrap();
        let secondary = MonitorSnapshot::from_test(
            MonitorIdentity::from_test_key("secondary"),
            secondary_main,
            secondary_main,
            1.0,
            WorkAreaProvenance::FullMain(WorkAreaFallback::SourceUnavailable),
        );

        let primary_identity = MonitorIdentity::from_test_key("primary");
        let snapshots = normalize_snapshots(vec![secondary, primary], Some(&primary_identity));
        assert_eq!(snapshots[0].identity(), &primary_identity);
        assert_eq!(snapshots.len(), 2);
    }
}

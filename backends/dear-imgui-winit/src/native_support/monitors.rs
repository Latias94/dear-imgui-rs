use std::panic::{AssertUnwindSafe, catch_unwind};

use thiserror::Error;
use winit::monitor::MonitorHandle;
use winit::window::Window;

#[cfg(target_os = "linux")]
use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};

#[cfg(target_os = "macos")]
use winit::platform::macos::MonitorHandleExtMacOS;
#[cfg(target_os = "windows")]
use winit::platform::windows::MonitorHandleExtWindows;

use super::geometry::{PhysicalMonitorRect, RectValidationError};

/// A detached monitor identity suitable for comparing adjacent snapshots.
///
/// The identity never retains a live Winit handle. Native identifiers are used when the host
/// exposes one; otherwise the value is derived from the monitor's current geometry and name.
/// It is a refresh-local identity and must not be treated as a persistent device identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MonitorIdentity {
    key: String,
}

impl MonitorIdentity {
    fn from_monitor(
        monitor: &MonitorHandle,
        main: PhysicalMonitorRect,
        backend: MonitorBackend,
    ) -> Self {
        let fallback = || {
            let name = catch_unwind(AssertUnwindSafe(|| monitor.name()))
                .ok()
                .flatten()
                .unwrap_or_default();
            format!(
                "fallback:{:?}:{:?}:{:?}:{name}",
                main.position(),
                main.size(),
                backend,
            )
        };

        let key = match backend {
            #[cfg(target_os = "windows")]
            MonitorBackend::Windows => catch_unwind(AssertUnwindSafe(|| monitor.native_id()))
                .ok()
                .map(|id| format!("windows:{id}"))
                .unwrap_or_else(fallback),
            #[cfg(target_os = "macos")]
            MonitorBackend::MacOs => catch_unwind(AssertUnwindSafe(|| monitor.native_id()))
                .ok()
                .map(|id| format!("macos:{id}"))
                .unwrap_or_else(fallback),
            #[cfg(target_os = "linux")]
            MonitorBackend::X11 | MonitorBackend::Wayland | MonitorBackend::Other => fallback(),
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
            MonitorBackend::Other => fallback(),
        };
        Self { key }
    }

    #[cfg(test)]
    pub(crate) fn from_test_key(key: &str) -> Self {
        Self {
            key: key.to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MonitorBackend {
    #[cfg(target_os = "windows")]
    Windows,
    #[cfg(target_os = "macos")]
    MacOs,
    #[cfg(target_os = "linux")]
    X11,
    #[cfg(target_os = "linux")]
    Wayland,
    #[cfg(any(
        target_os = "linux",
        not(any(target_os = "windows", target_os = "macos", target_os = "linux"))
    ))]
    Other,
}

/// Describes where the work rectangle came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkAreaProvenance {
    WindowsRcWork,
    MacOsVisibleFrame,
    FullMain(WorkAreaFallback),
}

/// Conservative reasons for using the full monitor rectangle as work area.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkAreaFallback {
    Wayland,
    UnsupportedWindowSystem,
    SourceUnavailable,
    InvalidNativeData,
    MainThreadUnavailable,
    AmbiguousDesktopScope,
}

/// Owned native facts for one monitor.
#[derive(Clone, Debug, PartialEq)]
pub struct MonitorSnapshot {
    identity: MonitorIdentity,
    main: PhysicalMonitorRect,
    work: PhysicalMonitorRect,
    scale_factor: f64,
    provenance: WorkAreaProvenance,
}

/// A complete detached monitor batch and the primary identity proven during that same refresh.
///
/// `primary_identity` is omitted when the host cannot provide an unambiguous detached match. In
/// particular, geometry/name fallback identities can collide on identical displays; callers must
/// not promote an arbitrary candidate to primary in that case.
#[derive(Clone, Debug, PartialEq)]
pub struct MonitorSnapshotSet {
    snapshots: Vec<MonitorSnapshot>,
    primary_identity: Option<MonitorIdentity>,
}

impl MonitorSnapshotSet {
    /// Returns the owned monitor snapshots in collection order.
    pub fn snapshots(&self) -> &[MonitorSnapshot] {
        &self.snapshots
    }

    /// Returns the detached primary identity when it was uniquely proven in this batch.
    pub fn primary_identity(&self) -> Option<&MonitorIdentity> {
        self.primary_identity.as_ref()
    }

    /// Splits the batch into its owned snapshots and detached primary identity.
    pub fn into_parts(self) -> (Vec<MonitorSnapshot>, Option<MonitorIdentity>) {
        (self.snapshots, self.primary_identity)
    }
}

/// A failure forming a complete set of trusted monitor facts.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum MonitorCollectionError {
    #[error("monitor {monitor} main facts are unavailable")]
    MainFactsUnavailable { monitor: usize },
    #[error("monitor {monitor} main geometry is invalid: {reason}")]
    InvalidMainGeometry {
        monitor: usize,
        reason: RectValidationError,
    },
    #[error("monitor {monitor} scale factor must be finite and positive")]
    InvalidScaleFactor { monitor: usize },
    #[error("monitor {monitor} work geometry is inconsistent with its main geometry")]
    InvalidWorkGeometry { monitor: usize },
}

impl MonitorSnapshot {
    fn new(
        index: usize,
        monitor: MonitorHandle,
        main: PhysicalMonitorRect,
        work: PhysicalMonitorRect,
        scale_factor: f64,
        provenance: WorkAreaProvenance,
        backend: MonitorBackend,
    ) -> Result<Self, MonitorCollectionError> {
        if !main.has_positive_area() {
            return Err(MonitorCollectionError::InvalidMainGeometry {
                monitor: index,
                reason: RectValidationError::ZeroArea,
            });
        }
        if !scale_factor.is_finite() || scale_factor <= 0.0 {
            return Err(MonitorCollectionError::InvalidScaleFactor { monitor: index });
        }
        if !valid_work_geometry(main, work, provenance) {
            return Err(MonitorCollectionError::InvalidWorkGeometry { monitor: index });
        }
        Ok(Self {
            identity: MonitorIdentity::from_monitor(&monitor, main, backend),
            main,
            work,
            scale_factor,
            provenance,
        })
    }

    /// Returns the detached monitor identity.
    pub fn identity(&self) -> &MonitorIdentity {
        &self.identity
    }

    /// Returns the full physical monitor rectangle.
    pub fn main(&self) -> PhysicalMonitorRect {
        self.main
    }

    /// Returns the physical work rectangle.
    pub fn work(&self) -> PhysicalMonitorRect {
        self.work
    }

    /// Returns the Winit monitor scale factor.
    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    /// Returns the source or fallback provenance for the work rectangle.
    pub fn work_area_provenance(&self) -> WorkAreaProvenance {
        self.provenance
    }

    #[cfg(test)]
    pub(crate) fn from_test(
        identity: MonitorIdentity,
        main: PhysicalMonitorRect,
        work: PhysicalMonitorRect,
        scale_factor: f64,
        provenance: WorkAreaProvenance,
    ) -> Self {
        Self {
            identity,
            main,
            work,
            scale_factor,
            provenance,
        }
    }
}

fn valid_work_geometry(
    main: PhysicalMonitorRect,
    work: PhysicalMonitorRect,
    provenance: WorkAreaProvenance,
) -> bool {
    main.contains(work) && (!matches!(provenance, WorkAreaProvenance::FullMain(_)) || work == main)
}

/// Collects owned monitor snapshots from the exact host window's monitor enumeration.
///
/// Enumerating through `Window::available_monitors` is intentional: callers cannot accidentally
/// pair a display handle from one Winit host with monitor handles obtained from another event-loop
/// or backend generation.
pub fn collect_monitor_snapshots(
    host: &Window,
) -> Result<Vec<MonitorSnapshot>, MonitorCollectionError> {
    Ok(collect_monitor_snapshot_set(host)?.into_parts().0)
}

/// Collects a complete detached monitor batch and the primary identity proven in that batch.
///
/// The primary handle is never retained. Its identity is derived before the platform-specific
/// collectors consume their monitor handles, then accepted only when exactly one collected
/// snapshot carries the same detached identity.
pub fn collect_monitor_snapshot_set(
    host: &Window,
) -> Result<MonitorSnapshotSet, MonitorCollectionError> {
    let monitors: Vec<_> = host.available_monitors().collect();
    let backend = monitor_backend(host);
    let primary = catch_unwind(AssertUnwindSafe(|| host.primary_monitor()))
        .ok()
        .flatten();
    let primary_identity_hint = primary.as_ref().and_then(|monitor| {
        winit_main_facts(monitor, 0)
            .ok()
            .map(|(main, _)| MonitorIdentity::from_monitor(monitor, main, backend))
    });
    let snapshots = collect_monitor_snapshots_from_handles(monitors, backend)?;
    let primary_identity = unique_primary_identity(primary_identity_hint, &snapshots);

    Ok(MonitorSnapshotSet {
        snapshots,
        primary_identity,
    })
}

fn collect_monitor_snapshots_from_handles(
    monitors: Vec<MonitorHandle>,
    backend: MonitorBackend,
) -> Result<Vec<MonitorSnapshot>, MonitorCollectionError> {
    #[cfg(target_os = "windows")]
    {
        windows::collect(monitors, backend)
    }

    #[cfg(target_os = "macos")]
    {
        macos::collect(monitors, backend)
    }

    #[cfg(target_os = "linux")]
    {
        linux::collect(monitors, backend)
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    collect_full_main(monitors, WorkAreaFallback::UnsupportedWindowSystem, backend)
}

fn unique_primary_identity(
    candidate: Option<MonitorIdentity>,
    snapshots: &[MonitorSnapshot],
) -> Option<MonitorIdentity> {
    let candidate = candidate?;
    (snapshots
        .iter()
        .filter(|snapshot| snapshot.identity() == &candidate)
        .count()
        == 1)
        .then_some(candidate)
}

fn monitor_backend(host: &Window) -> MonitorBackend {
    #[cfg(target_os = "windows")]
    {
        let _ = host;
        MonitorBackend::Windows
    }
    #[cfg(target_os = "macos")]
    {
        let _ = host;
        MonitorBackend::MacOs
    }
    #[cfg(target_os = "linux")]
    {
        match host.display_handle().ok().map(|handle| handle.as_raw()) {
            Some(RawDisplayHandle::Xlib(_)) => MonitorBackend::X11,
            Some(RawDisplayHandle::Wayland(_)) => MonitorBackend::Wayland,
            _ => MonitorBackend::Other,
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = host;
        MonitorBackend::Other
    }
}

#[cfg(not(target_os = "windows"))]
fn collect_full_main(
    monitors: Vec<MonitorHandle>,
    fallback: WorkAreaFallback,
    backend: MonitorBackend,
) -> Result<Vec<MonitorSnapshot>, MonitorCollectionError> {
    monitors
        .into_iter()
        .enumerate()
        .map(|(index, monitor)| {
            let (main, scale) = winit_main_facts(&monitor, index)?;
            MonitorSnapshot::new(
                index,
                monitor,
                main,
                main,
                scale,
                WorkAreaProvenance::FullMain(fallback),
                backend,
            )
        })
        .collect()
}

fn winit_main_facts(
    monitor: &MonitorHandle,
    index: usize,
) -> Result<(PhysicalMonitorRect, f64), MonitorCollectionError> {
    let facts = catch_unwind(AssertUnwindSafe(|| {
        let position = monitor.position();
        let size = monitor.size();
        let scale = monitor.scale_factor();
        (position, size, scale)
    }))
    .map_err(|_| MonitorCollectionError::MainFactsUnavailable { monitor: index })?;
    let main =
        PhysicalMonitorRect::from_i32_u32([facts.0.x, facts.0.y], [facts.1.width, facts.1.height])
            .map_err(|reason| MonitorCollectionError::InvalidMainGeometry {
                monitor: index,
                reason,
            })?;
    if !main.has_positive_area() {
        return Err(MonitorCollectionError::InvalidMainGeometry {
            monitor: index,
            reason: RectValidationError::ZeroArea,
        });
    }
    if !facts.2.is_finite() || facts.2 <= 0.0 {
        return Err(MonitorCollectionError::InvalidScaleFactor { monitor: index });
    }
    Ok((main, facts.2))
}

#[cfg(any(target_os = "macos", test))]
fn work_rect_from_local_insets(
    main: PhysicalMonitorRect,
    frame_size: [f64; 2],
    insets: [f64; 4],
) -> Option<PhysicalMonitorRect> {
    let [left, top, right, bottom] = insets;
    if !frame_size.into_iter().chain(insets).all(f64::is_finite)
        || frame_size[0] <= 0.0
        || frame_size[1] <= 0.0
        || insets.into_iter().any(|inset| inset < 0.0)
        || left + right > frame_size[0]
        || top + bottom > frame_size[1]
    {
        return None;
    }
    let scale = [
        main.size()[0] / frame_size[0],
        main.size()[1] / frame_size[1],
    ];
    PhysicalMonitorRect::new(
        [
            main.position()[0] + left * scale[0],
            main.position()[1] + top * scale[1],
        ],
        [
            (frame_size[0] - left - right) * scale[0],
            (frame_size[1] - top - bottom) * scale[1],
        ],
    )
    .ok()
    .filter(|work| main.contains(*work))
}

#[cfg(target_os = "windows")]
mod windows {
    use std::mem::size_of;

    use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITORINFO};

    use super::*;

    pub(super) fn collect(
        monitors: Vec<MonitorHandle>,
        backend: MonitorBackend,
    ) -> Result<Vec<MonitorSnapshot>, MonitorCollectionError> {
        monitors
            .into_iter()
            .enumerate()
            .map(|(index, monitor)| {
                let native = monitor.hmonitor();
                let hmonitor = native as windows_sys::Win32::Graphics::Gdi::HMONITOR;
                let mut info = MONITORINFO {
                    cbSize: size_of::<MONITORINFO>() as u32,
                    ..MONITORINFO::default()
                };
                if unsafe { GetMonitorInfoW(hmonitor, &mut info) } == 0 {
                    let (main, scale) = winit_main_facts(&monitor, index)?;
                    return MonitorSnapshot::new(
                        index,
                        monitor,
                        main,
                        main,
                        scale,
                        WorkAreaProvenance::FullMain(WorkAreaFallback::SourceUnavailable),
                        backend,
                    );
                }
                let main = rect_from_native(info.rcMonitor, index)?;
                let scale = winit_scale(&monitor, index)?;
                let (work, provenance) = match native_rect(info.rcWork)
                    .ok()
                    .filter(|work| main.contains(*work))
                {
                    Some(work) => (work, WorkAreaProvenance::WindowsRcWork),
                    None => (
                        main,
                        WorkAreaProvenance::FullMain(WorkAreaFallback::InvalidNativeData),
                    ),
                };
                MonitorSnapshot::new(index, monitor, main, work, scale, provenance, backend)
            })
            .collect()
    }

    fn rect_from_native(
        rect: windows_sys::Win32::Foundation::RECT,
        index: usize,
    ) -> Result<PhysicalMonitorRect, MonitorCollectionError> {
        let main =
            native_rect(rect).map_err(|reason| MonitorCollectionError::InvalidMainGeometry {
                monitor: index,
                reason,
            })?;
        if !main.has_positive_area() {
            return Err(MonitorCollectionError::InvalidMainGeometry {
                monitor: index,
                reason: RectValidationError::ZeroArea,
            });
        }
        Ok(main)
    }

    fn native_rect(
        rect: windows_sys::Win32::Foundation::RECT,
    ) -> Result<PhysicalMonitorRect, RectValidationError> {
        let width = i64::from(rect.right) - i64::from(rect.left);
        let height = i64::from(rect.bottom) - i64::from(rect.top);
        if width < 0 || height < 0 || width > u32::MAX as i64 || height > u32::MAX as i64 {
            return Err(RectValidationError::NegativeSize);
        }
        PhysicalMonitorRect::from_i32_u32([rect.left, rect.top], [width as u32, height as u32])
    }

    fn winit_scale(monitor: &MonitorHandle, index: usize) -> Result<f64, MonitorCollectionError> {
        let scale = catch_unwind(AssertUnwindSafe(|| monitor.scale_factor()))
            .map_err(|_| MonitorCollectionError::MainFactsUnavailable { monitor: index })?;
        if !scale.is_finite() || scale <= 0.0 {
            return Err(MonitorCollectionError::InvalidScaleFactor { monitor: index });
        }
        Ok(scale)
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2::MainThreadMarker;
    use objc2::rc::Retained;
    use objc2_app_kit::NSScreen;

    use super::*;

    pub(super) fn collect(
        monitors: Vec<MonitorHandle>,
        backend: MonitorBackend,
    ) -> Result<Vec<MonitorSnapshot>, MonitorCollectionError> {
        let Some(mtm) = MainThreadMarker::new() else {
            return collect_full_main(monitors, WorkAreaFallback::MainThreadUnavailable, backend);
        };
        monitors
            .into_iter()
            .enumerate()
            .map(|(index, monitor)| collect_one(monitor, index, mtm, backend))
            .collect()
    }

    fn collect_one(
        monitor: MonitorHandle,
        index: usize,
        _mtm: MainThreadMarker,
        backend: MonitorBackend,
    ) -> Result<MonitorSnapshot, MonitorCollectionError> {
        let (main, scale) = winit_main_facts(&monitor, index)?;
        let Some(raw_screen) = monitor.ns_screen() else {
            return MonitorSnapshot::new(
                index,
                monitor,
                main,
                main,
                scale,
                WorkAreaProvenance::FullMain(WorkAreaFallback::SourceUnavailable),
                backend,
            );
        };
        let Some(screen) = (unsafe { Retained::<NSScreen>::retain(raw_screen.cast()) }) else {
            return MonitorSnapshot::new(
                index,
                monitor,
                main,
                main,
                scale,
                WorkAreaProvenance::FullMain(WorkAreaFallback::SourceUnavailable),
                backend,
            );
        };
        let frame = screen.frame();
        let visible = screen.visibleFrame();
        let values = [
            frame.origin.x,
            frame.origin.y,
            frame.size.width,
            frame.size.height,
            visible.origin.x,
            visible.origin.y,
            visible.size.width,
            visible.size.height,
        ];
        if !values.into_iter().all(|value| (value as f64).is_finite())
            || frame.size.width <= 0.0
            || frame.size.height <= 0.0
        {
            return MonitorSnapshot::new(
                index,
                monitor,
                main,
                main,
                scale,
                WorkAreaProvenance::FullMain(WorkAreaFallback::InvalidNativeData),
                backend,
            );
        }
        let left = visible.origin.x - frame.origin.x;
        let bottom = visible.origin.y - frame.origin.y;
        let right = frame.origin.x + frame.size.width - (visible.origin.x + visible.size.width);
        let top = frame.origin.y + frame.size.height - (visible.origin.y + visible.size.height);
        if [left, right, top, bottom]
            .into_iter()
            .any(|value| !(value as f64).is_finite() || value < 0.0)
            || left + right > frame.size.width
            || top + bottom > frame.size.height
        {
            return MonitorSnapshot::new(
                index,
                monitor,
                main,
                main,
                scale,
                WorkAreaProvenance::FullMain(WorkAreaFallback::InvalidNativeData),
                backend,
            );
        }
        let native_work = work_rect_from_local_insets(
            main,
            [frame.size.width as f64, frame.size.height as f64],
            [left as f64, top as f64, right as f64, bottom as f64],
        );
        let (work, provenance) = match native_work {
            Some(work) => (work, WorkAreaProvenance::MacOsVisibleFrame),
            None => (
                main,
                WorkAreaProvenance::FullMain(WorkAreaFallback::InvalidNativeData),
            ),
        };
        MonitorSnapshot::new(index, monitor, main, work, scale, provenance, backend)
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;

    pub(super) fn collect(
        monitors: Vec<MonitorHandle>,
        backend: MonitorBackend,
    ) -> Result<Vec<MonitorSnapshot>, MonitorCollectionError> {
        let fallback = match backend {
            MonitorBackend::X11 => WorkAreaFallback::AmbiguousDesktopScope,
            MonitorBackend::Wayland => WorkAreaFallback::Wayland,
            _ => WorkAreaFallback::SourceUnavailable,
        };
        collect_full_main(monitors, fallback, backend)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_snapshot(identity: &str, x: f64) -> MonitorSnapshot {
        let main = PhysicalMonitorRect::new([x, 0.0], [1920.0, 1080.0]).unwrap();
        MonitorSnapshot::from_test(
            MonitorIdentity::from_test_key(identity),
            main,
            main,
            1.0,
            WorkAreaProvenance::FullMain(WorkAreaFallback::SourceUnavailable),
        )
    }

    #[test]
    fn primary_identity_requires_one_detached_match() {
        let primary = MonitorIdentity::from_test_key("primary");
        let snapshots = vec![
            test_snapshot("primary", 0.0),
            test_snapshot("secondary", 1920.0),
        ];
        assert_eq!(
            unique_primary_identity(Some(primary.clone()), &snapshots),
            Some(primary.clone())
        );

        let ambiguous = vec![
            test_snapshot("primary", 0.0),
            test_snapshot("primary", 1920.0),
        ];
        assert_eq!(unique_primary_identity(Some(primary), &ambiguous), None);
    }

    #[test]
    fn fallback_provenance_requires_exact_positive_main_geometry() {
        let main = PhysicalMonitorRect::new([-10.0, 2.0], [100.0, 80.0]).unwrap();
        let reduced = PhysicalMonitorRect::new([-10.0, 12.0], [100.0, 70.0]).unwrap();
        let zero = PhysicalMonitorRect::new([-10.0, 2.0], [0.0, 80.0]).unwrap();

        assert!(valid_work_geometry(
            main,
            main,
            WorkAreaProvenance::FullMain(WorkAreaFallback::SourceUnavailable)
        ));
        assert!(!valid_work_geometry(
            main,
            reduced,
            WorkAreaProvenance::FullMain(WorkAreaFallback::InvalidNativeData)
        ));
        assert!(valid_work_geometry(
            main,
            reduced,
            WorkAreaProvenance::WindowsRcWork
        ));
        assert!(valid_work_geometry(
            main,
            zero,
            WorkAreaProvenance::WindowsRcWork
        ));
    }

    #[test]
    fn local_insets_map_cocoa_top_and_bottom_into_winit_space() {
        let main = PhysicalMonitorRect::new([1920.0, -200.0], [2560.0, 1440.0]).unwrap();
        let work =
            work_rect_from_local_insets(main, [1280.0, 720.0], [10.0, 20.0, 30.0, 40.0]).unwrap();

        assert_eq!(work.position(), [1940.0, -160.0]);
        assert_eq!(work.size(), [2480.0, 1320.0]);
    }

    #[test]
    fn invalid_local_insets_fall_back_instead_of_escaping_main() {
        let main = PhysicalMonitorRect::new([0.0, 0.0], [100.0, 100.0]).unwrap();

        assert!(
            work_rect_from_local_insets(main, [100.0, 100.0], [60.0, 0.0, 60.0, 0.0]).is_none()
        );
        assert!(
            work_rect_from_local_insets(main, [100.0, 100.0], [f64::NAN, 0.0, 0.0, 0.0]).is_none()
        );
    }
}

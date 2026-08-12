//! Narrow, owned access to native desktop facts used by first-party hosts.
//!
//! This module deliberately does not own a Winit event loop, a Dear ImGui
//! context, or a viewport lifecycle. It only converts a live Winit host into
//! owned monitor facts and, on Windows, exposes an exact-window policy lease.
//!
//! [`collect_monitor_snapshot_set`] enumerates monitors from the supplied
//! [`winit::window::Window`] so the display and monitor handles belong to one
//! Winit host. The returned [`MonitorSnapshot`] values own only detached data;
//! they do not retain live Winit monitor handles. The batch carries a detached
//! primary identity only when it is unambiguous in that refresh. X11 work-area
//! data is kept as desktop-scoped evidence and therefore currently falls back
//! to the full monitor rectangle.

mod geometry;
mod monitors;

#[cfg(target_os = "windows")]
mod window_policy;

pub use geometry::{PhysicalMonitorRect, RectValidationError};
pub use monitors::{
    MonitorCollectionError, MonitorIdentity, MonitorSnapshot, MonitorSnapshotSet, WorkAreaFallback,
    WorkAreaProvenance, collect_monitor_snapshot_set, collect_monitor_snapshots,
};

#[cfg(target_os = "windows")]
pub use window_policy::{NativeWindowPolicy, WindowPolicyError, WindowPolicyLease};

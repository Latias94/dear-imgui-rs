#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use super::protocol::ImguiViewportFeedback;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_ecs::entity::Entity;
use bevy_math::IVec2;
use bevy_window::Window;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_window::WindowPosition;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_winit::WINIT_WINDOWS;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use dear_imgui_rs::sys;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use dear_imgui_winit::native_support::{MonitorSnapshot, MonitorSnapshotSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)] // Exactly one variant is constructed on each native target.
pub(super) enum DesktopCoordinateSpace {
    Physical,
    Logical,
}

pub(super) const fn native_desktop_coordinate_space() -> DesktopCoordinateSpace {
    #[cfg(target_os = "macos")]
    {
        DesktopCoordinateSpace::Logical
    }
    #[cfg(not(target_os = "macos"))]
    {
        DesktopCoordinateSpace::Physical
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn desktop_position_from_physical(position: IVec2, scale_factor: f32) -> [f32; 2] {
    let position = [position.x as f32, position.y as f32];
    match native_desktop_coordinate_space() {
        DesktopCoordinateSpace::Physical => position,
        DesktopCoordinateSpace::Logical => {
            let scale_factor = positive_finite_or(scale_factor, 1.0);
            [position[0] / scale_factor, position[1] / scale_factor]
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn desktop_size_from_physical(size: [u32; 2], scale_factor: f32) -> [f32; 2] {
    let size = [size[0] as f32, size[1] as f32];
    match native_desktop_coordinate_space() {
        DesktopCoordinateSpace::Physical => size,
        DesktopCoordinateSpace::Logical => {
            let scale_factor = positive_finite_or(scale_factor, 1.0);
            [size[0] / scale_factor, size[1] / scale_factor]
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) fn desktop_framebuffer_scale(scale_factor: f32) -> [f32; 2] {
    match native_desktop_coordinate_space() {
        DesktopCoordinateSpace::Physical => [1.0, 1.0],
        DesktopCoordinateSpace::Logical => {
            let scale_factor = positive_finite_or(scale_factor, 1.0);
            [scale_factor, scale_factor]
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn desktop_metrics_for_window(window: &Window) -> ([f32; 2], [f32; 2]) {
    let scale_factor = window.scale_factor();
    (
        desktop_size_from_physical(
            [window.physical_width(), window.physical_height()],
            scale_factor,
        ),
        desktop_framebuffer_scale(scale_factor),
    )
}

#[cfg(test)]
pub(super) fn monitor_from_window(window: &Window) -> sys::ImGuiPlatformMonitor {
    let mut monitor = sys::ImGuiPlatformMonitor::default();
    let pos = match window.position {
        WindowPosition::At(pos) => desktop_position_from_physical(pos, window.scale_factor()),
        WindowPosition::Automatic | WindowPosition::Centered(_) => [0.0, 0.0],
    };
    let size = desktop_size_from_physical(
        [window.physical_width(), window.physical_height()],
        window.scale_factor(),
    );
    monitor.MainPos = sys::ImVec2 {
        x: pos[0],
        y: pos[1],
    };
    monitor.MainSize = sys::ImVec2 {
        x: size[0],
        y: size[1],
    };
    monitor.WorkPos = monitor.MainPos;
    monitor.WorkSize = monitor.MainSize;
    monitor.DpiScale = positive_finite_or(window.scale_factor(), 1.0);
    monitor
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ImguiMonitorPublication {
    pub(crate) facts: Option<Vec<MonitorSnapshot>>,
    pub(crate) values: Vec<sys::ImGuiPlatformMonitor>,
}

impl ImguiMonitorPublication {
    pub(crate) fn equivalent_to(&self, other: &Self) -> bool {
        self.values == other.values
            && match (&self.facts, &other.facts) {
                (Some(left), Some(right)) => left == right,
                (None, None) => true,
                // A detached facts batch is stronger evidence than a legacy numeric-only seam;
                // publish it even when the projected ImGui values happen to be unchanged.
                _ => false,
            }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn monitor_publication_from_snapshot_set(
    snapshot_set: MonitorSnapshotSet,
) -> Option<ImguiMonitorPublication> {
    let (mut snapshots, primary_identity) = snapshot_set.into_parts();
    snapshots.sort_by(|left, right| {
        let left_primary = primary_identity
            .as_ref()
            .is_some_and(|identity| identity == left.identity());
        let right_primary = primary_identity
            .as_ref()
            .is_some_and(|identity| identity == right.identity());
        right_primary
            .cmp(&left_primary)
            .then_with(|| left.identity().cmp(right.identity()))
            .then_with(|| left.main().position()[0].total_cmp(&right.main().position()[0]))
            .then_with(|| left.main().position()[1].total_cmp(&right.main().position()[1]))
            .then_with(|| left.main().size()[0].total_cmp(&right.main().size()[0]))
            .then_with(|| left.main().size()[1].total_cmp(&right.main().size()[1]))
            .then_with(|| left.scale_factor().total_cmp(&right.scale_factor()))
            .then_with(|| left.work().position()[0].total_cmp(&right.work().position()[0]))
            .then_with(|| left.work().position()[1].total_cmp(&right.work().position()[1]))
            .then_with(|| left.work().size()[0].total_cmp(&right.work().size()[0]))
            .then_with(|| left.work().size()[1].total_cmp(&right.work().size()[1]))
    });
    // Detached fallback identities can collide for identical displays. Only exact duplicate
    // facts are redundant; identity equality alone is not sufficient evidence to drop a monitor.
    snapshots.dedup();
    let values = snapshots
        .iter()
        .map(platform_monitor_from_snapshot)
        .collect::<Option<Vec<_>>>()?;
    (!values.is_empty()).then_some(ImguiMonitorPublication {
        facts: Some(snapshots),
        values,
    })
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn platform_monitor_from_snapshot(snapshot: &MonitorSnapshot) -> Option<sys::ImGuiPlatformMonitor> {
    let scale = f32_from_f64(snapshot.scale_factor())?;
    let main_pos = desktop_position_from_physical_f64(snapshot.main().position(), scale)?;
    let main_size = desktop_size_from_physical_f64(snapshot.main().size(), scale)?;
    let work_pos = desktop_position_from_physical_f64(snapshot.work().position(), scale)?;
    let work_size = desktop_size_from_physical_f64(snapshot.work().size(), scale)?;
    Some(sys::ImGuiPlatformMonitor {
        MainPos: sys::ImVec2 {
            x: main_pos[0],
            y: main_pos[1],
        },
        MainSize: sys::ImVec2 {
            x: main_size[0],
            y: main_size[1],
        },
        WorkPos: sys::ImVec2 {
            x: work_pos[0],
            y: work_pos[1],
        },
        WorkSize: sys::ImVec2 {
            x: work_size[0],
            y: work_size[1],
        },
        DpiScale: scale,
        ..sys::ImGuiPlatformMonitor::default()
    })
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn f32_from_f64(value: f64) -> Option<f32> {
    let value = value as f32;
    value.is_finite().then_some(value)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn desktop_position_from_physical_f64(position: [f64; 2], scale_factor: f32) -> Option<[f32; 2]> {
    let position = [f32_from_f64(position[0])?, f32_from_f64(position[1])?];
    match native_desktop_coordinate_space() {
        DesktopCoordinateSpace::Physical => Some(position),
        DesktopCoordinateSpace::Logical => {
            let scale_factor = positive_finite_or(scale_factor, 1.0);
            let position = [position[0] / scale_factor, position[1] / scale_factor];
            position.into_iter().all(f32::is_finite).then_some(position)
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn desktop_size_from_physical_f64(size: [f64; 2], scale_factor: f32) -> Option<[f32; 2]> {
    let size = [f32_from_f64(size[0])?, f32_from_f64(size[1])?];
    if size.iter().any(|value| *value < 0.0) {
        return None;
    }
    match native_desktop_coordinate_space() {
        DesktopCoordinateSpace::Physical => Some(size),
        DesktopCoordinateSpace::Logical => {
            let scale_factor = positive_finite_or(scale_factor, 1.0);
            let size = [size[0] / scale_factor, size[1] / scale_factor];
            size.into_iter().all(f32::is_finite).then_some(size)
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn viewport_feedback_from_window(
    entity: Entity,
    window: &Window,
    previous: Option<ImguiViewportFeedback>,
) -> ImguiViewportFeedback {
    feedback_from_window_for_entity(entity, window, previous, None)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) fn feedback_from_window_for_entity(
    entity: Entity,
    window: &Window,
    previous: Option<ImguiViewportFeedback>,
    minimized: Option<bool>,
) -> ImguiViewportFeedback {
    let pos = winit_window_client_origin_desktop(entity)
        .or_else(|| previous.map(|feedback| feedback.pos))
        .or_else(|| window_position_desktop(&window.position, window.scale_factor()))
        .unwrap_or([0.0, 0.0]);
    let scale_factor = window_client_scale_factor(entity, window);
    let size = winit_window_client_size_desktop(entity).unwrap_or_else(|| {
        desktop_size_from_physical(
            [window.physical_width(), window.physical_height()],
            scale_factor,
        )
    });
    ImguiViewportFeedback {
        pos,
        size,
        framebuffer_scale: desktop_framebuffer_scale(scale_factor),
        dpi_scale: scale_factor,
        focused: window.focused,
        minimized: minimized
            .or_else(|| previous.map(|feedback| feedback.minimized))
            .unwrap_or(false),
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn window_client_origin_desktop(
    entity: Entity,
    position: &WindowPosition,
    scale_factor: f32,
) -> Option<[f32; 2]> {
    if let Some(pos) = winit_window_client_origin_desktop(entity) {
        return Some(pos);
    }
    window_position_desktop(position, scale_factor)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) fn window_position_desktop(
    position: &WindowPosition,
    scale_factor: f32,
) -> Option<[f32; 2]> {
    match *position {
        WindowPosition::At(pos) => Some(desktop_position_from_physical(pos, scale_factor)),
        WindowPosition::Automatic | WindowPosition::Centered(_) => None,
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn window_client_logical_to_desktop(
    entity: Entity,
    scale_factor: f32,
    cached_client_origin: Option<[f32; 2]>,
    client_position: [f32; 2],
) -> Option<[f32; 2]> {
    if !client_position.into_iter().all(f32::is_finite) {
        return None;
    }
    let origin = winit_window_client_origin_desktop(entity).or(cached_client_origin)?;
    let scale_factor =
        winit_window_scale_factor(entity).unwrap_or_else(|| positive_finite_or(scale_factor, 1.0));
    let client_position = match native_desktop_coordinate_space() {
        DesktopCoordinateSpace::Physical => [
            client_position[0] * scale_factor,
            client_position[1] * scale_factor,
        ],
        DesktopCoordinateSpace::Logical => client_position,
    };
    let position = [
        origin[0] + client_position[0],
        origin[1] + client_position[1],
    ];
    position.into_iter().all(f32::is_finite).then_some(position)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn desktop_to_window_client_logical(
    entity: Entity,
    position: &WindowPosition,
    scale_factor: f32,
    desktop_position: [f32; 2],
) -> Option<[f32; 2]> {
    if !desktop_position.into_iter().all(f32::is_finite) {
        return None;
    }
    let origin = window_client_origin_desktop(entity, position, scale_factor)?;
    let mut client_position = [
        desktop_position[0] - origin[0],
        desktop_position[1] - origin[1],
    ];
    if native_desktop_coordinate_space() == DesktopCoordinateSpace::Physical {
        let scale_factor = winit_window_scale_factor(entity)
            .unwrap_or_else(|| positive_finite_or(scale_factor, 1.0));
        client_position[0] /= scale_factor;
        client_position[1] /= scale_factor;
    }
    client_position
        .into_iter()
        .all(f32::is_finite)
        .then_some(client_position)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn window_client_scale_factor(entity: Entity, window: &Window) -> f32 {
    winit_window_scale_factor(entity)
        .unwrap_or_else(|| positive_finite_or(window.scale_factor(), 1.0))
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn winit_window_client_origin_desktop(entity: Entity) -> Option<[f32; 2]> {
    WINIT_WINDOWS.with_borrow(|windows| {
        let window = windows.get_window(entity)?;
        let scale = positive_finite_or(window.scale_factor() as f32, 1.0);
        let pos_phys = window.inner_position().ok()?;
        Some(desktop_position_from_physical(
            IVec2::new(pos_phys.x, pos_phys.y),
            scale,
        ))
    })
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn winit_window_client_size_desktop(entity: Entity) -> Option<[f32; 2]> {
    WINIT_WINDOWS.with_borrow(|windows| {
        let window = windows.get_window(entity)?;
        let size = window.inner_size();
        Some(desktop_size_from_physical(
            [size.width, size.height],
            positive_finite_or(window.scale_factor() as f32, 1.0),
        ))
    })
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) fn winit_window_decoration_offset_desktop(entity: Entity) -> Option<[f32; 2]> {
    WINIT_WINDOWS.with_borrow(|windows| {
        let window = windows.get_window(entity)?;
        let scale = positive_finite_or(window.scale_factor() as f32, 1.0);
        let inner = window.inner_position().ok()?;
        let outer = window.outer_position().ok()?;
        let inner = desktop_position_from_physical(IVec2::new(inner.x, inner.y), scale);
        let outer = desktop_position_from_physical(IVec2::new(outer.x, outer.y), scale);
        Some([inner[0] - outer[0], inner[1] - outer[1]])
    })
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn winit_window_scale_factor(entity: Entity) -> Option<f32> {
    WINIT_WINDOWS.with_borrow(|windows| {
        windows
            .get_window(entity)
            .map(|window| positive_finite_or(window.scale_factor() as f32, 1.0))
    })
}

pub(super) fn positive_finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) fn physical_outer_pos_for_client_pos(
    entity: Entity,
    pos: [f32; 2],
    dpi_scale: f32,
) -> IVec2 {
    let pos = if let Some(offset) = winit_window_decoration_offset_desktop(entity) {
        [pos[0] - offset[0], pos[1] - offset[1]]
    } else {
        pos
    };
    physical_pos_from_desktop(pos, dpi_scale)
}

pub(super) fn physical_pos_from_desktop(pos: [f32; 2], scale_factor: f32) -> IVec2 {
    let pos = finite_desktop_pos(pos);
    let pos = match native_desktop_coordinate_space() {
        DesktopCoordinateSpace::Physical => pos,
        DesktopCoordinateSpace::Logical => {
            let scale_factor = positive_finite_or(scale_factor, 1.0);
            [pos[0] * scale_factor, pos[1] * scale_factor]
        }
    };
    IVec2::new(pos[0].round() as i32, pos[1].round() as i32)
}

fn physical_extent(value: f32) -> u32 {
    value.round().max(1.0) as u32
}

pub(super) fn finite_desktop_pos(pos: [f32; 2]) -> [f32; 2] {
    [finite_or(pos[0], 0.0), finite_or(pos[1], 0.0)]
}

pub(super) fn finite_desktop_size(size: [f32; 2]) -> [f32; 2] {
    [
        positive_finite_or(size[0], 1.0),
        positive_finite_or(size[1], 1.0),
    ]
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

pub(super) fn set_window_desktop_size(window: &mut Window, size: [f32; 2], scale_factor: f32) {
    let size = finite_desktop_size(size);
    let size = match native_desktop_coordinate_space() {
        DesktopCoordinateSpace::Physical => size,
        DesktopCoordinateSpace::Logical => {
            let scale_factor = positive_finite_or(scale_factor, 1.0);
            [size[0] * scale_factor, size[1] * scale_factor]
        }
    };
    window
        .resolution
        .set_physical_resolution(physical_extent(size[0]), physical_extent(size[1]));
}

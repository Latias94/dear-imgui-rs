use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize, Position, Size};
use winit::window::Window;

use crate::native_support::MonitorSnapshot;
use crate::sanitize;

/// Dear ImGui requires every monitor rectangle, platform window rectangle, and mouse position to
/// use one native desktop coordinate space. Winit exposes physical values everywhere, but its
/// macOS implementation encodes Cocoa desktop points using each monitor/window backing scale.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
enum DesktopCoordinateSpace {
    Physical,
    Logical,
}

const fn native_desktop_coordinate_space() -> DesktopCoordinateSpace {
    #[cfg(target_os = "macos")]
    {
        DesktopCoordinateSpace::Logical
    }
    #[cfg(not(target_os = "macos"))]
    {
        DesktopCoordinateSpace::Physical
    }
}

fn sanitized_scale(scale: f64) -> f64 {
    sanitize::positive_finite_or(scale, 1.0)
}

pub(super) fn validate_viewport_position(position: [f32; 2]) -> Option<[f32; 2]> {
    let position = sanitize::finite_vec2_f32(position)?;
    let minimum = f64::from(i32::MIN);
    let maximum = f64::from(i32::MAX);
    position
        .into_iter()
        .map(f64::from)
        .all(|value| value >= minimum && value <= maximum)
        .then_some(position)
}

pub(super) fn validate_viewport_size(size: [f32; 2]) -> Option<[f32; 2]> {
    let size = sanitize::finite_vec2_f32(size)?;
    let maximum = f64::from(i32::MAX);
    size.into_iter()
        .map(f64::from)
        .all(|value| value > 0.0 && value <= maximum)
        .then_some(size)
}

fn desktop_position_from_physical_values(
    space: DesktopCoordinateSpace,
    position: [f64; 2],
    scale: f64,
) -> Option<[f64; 2]> {
    if !position.into_iter().all(f64::is_finite) {
        return None;
    }

    let position = match space {
        DesktopCoordinateSpace::Physical => position,
        DesktopCoordinateSpace::Logical => {
            let scale = sanitized_scale(scale);
            [position[0] / scale, position[1] / scale]
        }
    };
    position.into_iter().all(f64::is_finite).then_some(position)
}

fn desktop_size_from_physical_values(
    space: DesktopCoordinateSpace,
    size: [f64; 2],
    scale: f64,
) -> [f32; 2] {
    let size = match space {
        DesktopCoordinateSpace::Physical => size,
        DesktopCoordinateSpace::Logical => {
            let scale = sanitized_scale(scale);
            [size[0] / scale, size[1] / scale]
        }
    };
    [
        sanitize::finite_non_negative_f64_to_f32(size[0]).unwrap_or(0.0),
        sanitize::finite_non_negative_f64_to_f32(size[1]).unwrap_or(0.0),
    ]
}

fn checked_desktop_size_from_physical_values(
    space: DesktopCoordinateSpace,
    size: [f64; 2],
    scale: f64,
) -> Option<[f32; 2]> {
    let size = match space {
        DesktopCoordinateSpace::Physical => size,
        DesktopCoordinateSpace::Logical => {
            let scale = sanitized_scale(scale);
            [size[0] / scale, size[1] / scale]
        }
    };
    if !size
        .into_iter()
        .all(|value| value.is_finite() && value >= 0.0)
    {
        return None;
    }
    Some([
        sanitize::finite_non_negative_f64_to_f32(size[0])?,
        sanitize::finite_non_negative_f64_to_f32(size[1])?,
    ])
}

pub(super) fn desktop_position_from_physical(
    position: PhysicalPosition<i32>,
    scale: f64,
) -> Option<[f32; 2]> {
    desktop_position_from_physical_values(
        native_desktop_coordinate_space(),
        [f64::from(position.x), f64::from(position.y)],
        scale,
    )
    .and_then(sanitize::finite_vec2_f64_to_f32)
}

/// Converts detached native monitor facts into the coordinate model used by Winit viewports.
///
/// Main and work rectangles are converted independently. The conversion is fallible so native
/// values that cannot be represented as finite ImGui `f32` values are rejected instead of being
/// silently clamped into a different geometry.
pub(super) fn monitor_from_snapshot(
    snapshot: &MonitorSnapshot,
) -> Option<dear_imgui_rs::sys::ImGuiPlatformMonitor> {
    let scale = sanitized_scale(snapshot.scale_factor());
    let main = snapshot.main();
    let work = snapshot.work();
    let main_position = desktop_position_from_physical_values(
        native_desktop_coordinate_space(),
        main.position(),
        scale,
    )
    .and_then(sanitize::finite_vec2_f64_to_f32)?;
    let work_position = desktop_position_from_physical_values(
        native_desktop_coordinate_space(),
        work.position(),
        scale,
    )
    .and_then(sanitize::finite_vec2_f64_to_f32)?;
    let main_size = checked_desktop_size_from_physical_values(
        native_desktop_coordinate_space(),
        main.size(),
        scale,
    )?;
    let work_size = checked_desktop_size_from_physical_values(
        native_desktop_coordinate_space(),
        work.size(),
        scale,
    )?;

    Some(dear_imgui_rs::sys::ImGuiPlatformMonitor {
        MainPos: dear_imgui_rs::sys::ImVec2 {
            x: main_position[0],
            y: main_position[1],
        },
        MainSize: dear_imgui_rs::sys::ImVec2 {
            x: main_size[0],
            y: main_size[1],
        },
        WorkPos: dear_imgui_rs::sys::ImVec2 {
            x: work_position[0],
            y: work_position[1],
        },
        WorkSize: dear_imgui_rs::sys::ImVec2 {
            x: work_size[0],
            y: work_size[1],
        },
        DpiScale: sanitize::positive_finite_f32_or(scale as f32, 1.0),
        PlatformHandle: std::ptr::null_mut(),
    })
}

pub(super) unsafe fn viewport_target_dpi_scale(
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> f32 {
    let Some(viewport_ref) = (unsafe { viewport.as_ref() }) else {
        return 1.0;
    };
    let fallback = sanitize::positive_finite_f32_or(viewport_ref.DpiScale, 1.0);
    let monitor = unsafe { dear_imgui_rs::sys::igGetViewportPlatformMonitor(viewport) };
    unsafe { monitor.as_ref() }
        .map(|monitor| sanitize::positive_finite_f32_or(monitor.DpiScale, fallback))
        .unwrap_or(fallback)
}

pub(crate) fn desktop_size_for_window(window: &Window) -> [f32; 2] {
    desktop_size_from_physical(window.inner_size(), window.scale_factor())
}

pub(crate) fn single_window_display_metrics(window: &Window) -> ([f32; 2], [f32; 2]) {
    single_window_display_metrics_from_physical(window.inner_size(), window.scale_factor())
}

fn single_window_display_metrics_from_physical(
    size: PhysicalSize<u32>,
    scale: f64,
) -> ([f32; 2], [f32; 2]) {
    let scale = sanitized_scale(scale);
    let logical_size: LogicalSize<f64> = size.to_logical(scale);
    (
        sanitize::finite_non_negative_size(logical_size),
        sanitize::framebuffer_scale(scale, 1.0),
    )
}

pub(super) fn desktop_size_from_physical(size: PhysicalSize<u32>, scale: f64) -> [f32; 2] {
    desktop_size_from_physical_values(
        native_desktop_coordinate_space(),
        [f64::from(size.width), f64::from(size.height)],
        scale,
    )
}

pub(crate) fn framebuffer_scale_for_window(window: &Window) -> [f32; 2] {
    framebuffer_scale_for_space(native_desktop_coordinate_space(), window.scale_factor())
}

pub(crate) fn scale_factor_inner_size_override(
    scale_viewports: bool,
    current_size: PhysicalSize<u32>,
) -> Option<PhysicalSize<u32>> {
    scale_factor_inner_size_override_for_space(
        native_desktop_coordinate_space(),
        scale_viewports,
        current_size,
    )
}

fn scale_factor_inner_size_override_for_space(
    space: DesktopCoordinateSpace,
    scale_viewports: bool,
    current_size: PhysicalSize<u32>,
) -> Option<PhysicalSize<u32>> {
    (matches!(space, DesktopCoordinateSpace::Physical) && !scale_viewports).then_some(current_size)
}

fn framebuffer_scale_for_space(space: DesktopCoordinateSpace, scale: f64) -> [f32; 2] {
    match space {
        // The ImGui coordinate unit already is a framebuffer pixel on Windows and X11.
        DesktopCoordinateSpace::Physical => [1.0, 1.0],
        DesktopCoordinateSpace::Logical => {
            let scale = sanitize::positive_finite_f32_or(sanitized_scale(scale) as f32, 1.0);
            [scale, scale]
        }
    }
}

pub(super) fn framebuffer_scale_for_dpi_scale(scale: f64) -> [f32; 2] {
    framebuffer_scale_for_space(native_desktop_coordinate_space(), scale)
}

pub(crate) fn window_position_from_desktop(position: [f32; 2]) -> Position {
    match native_desktop_coordinate_space() {
        DesktopCoordinateSpace::Physical => Position::Physical(
            PhysicalPosition::new(f64::from(position[0]), f64::from(position[1])).cast(),
        ),
        DesktopCoordinateSpace::Logical => Position::Logical(LogicalPosition::new(
            f64::from(position[0]),
            f64::from(position[1]),
        )),
    }
}

pub(crate) fn window_size_from_desktop(size: [f32; 2]) -> Size {
    match native_desktop_coordinate_space() {
        DesktopCoordinateSpace::Physical => {
            Size::Physical(PhysicalSize::new(f64::from(size[0]), f64::from(size[1])).cast())
        }
        DesktopCoordinateSpace::Logical => {
            Size::Logical(LogicalSize::new(f64::from(size[0]), f64::from(size[1])))
        }
    }
}

pub(super) fn request_client_geometry(
    window: &Window,
    position: [f32; 2],
    size: [f32; 2],
    dpi_scale: f32,
) {
    let _ = window.request_inner_size(window_size_from_desktop(size));
    let outer_position = outer_position_from_client(window, position, dpi_scale);
    window.set_outer_position(window_position_from_desktop(outer_position));
}

pub(crate) fn client_physical_to_screen_pos(
    window: &Window,
    client_position: [f64; 2],
) -> Option<[f32; 2]> {
    let scale = window.scale_factor();
    let client = desktop_position_from_physical_values(
        native_desktop_coordinate_space(),
        client_position,
        scale,
    )?;
    let origin = window.inner_position().ok()?;
    let origin = desktop_position_from_physical_values(
        native_desktop_coordinate_space(),
        [f64::from(origin.x), f64::from(origin.y)],
        scale,
    )?;
    sanitize::finite_vec2_f64_to_f32([origin[0] + client[0], origin[1] + client[1]])
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ObservedClientGeometry {
    pub(super) position: [f32; 2],
    pub(super) size: [f32; 2],
    pub(super) decoration_offset: Option<[f32; 2]>,
}

pub(super) fn observe_client_geometry(window: &Window) -> Option<ObservedClientGeometry> {
    let scale = window.scale_factor();
    let inner = window.inner_position().ok()?;
    let position = desktop_position_from_physical(inner, scale)?;
    let size = desktop_size_from_physical(window.inner_size(), scale);
    let decoration_offset = window
        .outer_position()
        .ok()
        .and_then(|outer| decoration_offset_from_positions(inner, outer, scale));
    Some(ObservedClientGeometry {
        position,
        size,
        decoration_offset,
    })
}

pub(super) fn decoration_offset(window: &Window) -> Option<[f32; 2]> {
    let scale = window.scale_factor();
    let inner = window.inner_position().ok()?;
    let outer = window.outer_position().ok()?;
    decoration_offset_from_positions(inner, outer, scale)
}

fn decoration_offset_from_positions(
    inner: PhysicalPosition<i32>,
    outer: PhysicalPosition<i32>,
    scale: f64,
) -> Option<[f32; 2]> {
    let inner = desktop_position_from_physical_values(
        native_desktop_coordinate_space(),
        [f64::from(inner.x), f64::from(inner.y)],
        scale,
    )?;
    let outer = desktop_position_from_physical_values(
        native_desktop_coordinate_space(),
        [f64::from(outer.x), f64::from(outer.y)],
        scale,
    )?;
    sanitize::finite_vec2_f64_to_f32([inner[0] - outer[0], inner[1] - outer[1]])
}

pub(super) fn outer_position_from_client(
    window: &Window,
    client_position: [f32; 2],
    dpi_scale: f32,
) -> [f32; 2] {
    #[cfg(target_os = "windows")]
    if !window.is_decorated() {
        // Undecorated Winit windows retain resize-capable Win32 style bits while WM_NCCALCSIZE
        // makes the client area cover the window. Inferring decorations from those styles would
        // invent a title-bar offset and desynchronize ImGui hit testing from the native client
        // origin.
        return client_position;
    }

    // Decorated Windows windows need the destination monitor DPI, which is not necessarily the
    // current live decoration offset. Other platforms use Winit's observed client/outer geometry.
    #[cfg(target_os = "windows")]
    if let Some(position) =
        windows::outer_position_from_client_style(window, client_position, dpi_scale)
    {
        return position;
    }

    let _ = dpi_scale;
    decoration_offset(window)
        .and_then(|offset| outer_position_from_decoration_offset(client_position, offset))
        .unwrap_or(client_position)
}

fn outer_position_from_decoration_offset(
    client_position: [f32; 2],
    decoration_offset: [f32; 2],
) -> Option<[f32; 2]> {
    sanitize::finite_vec2_f32([
        client_position[0] - decoration_offset[0],
        client_position[1] - decoration_offset[1],
    ])
}

pub(crate) fn ime_cursor_area_for_viewport(
    window: &Window,
    input_position: [f32; 2],
    viewport_position: [f32; 2],
    line_height: f32,
) -> Option<(LogicalPosition<f64>, LogicalSize<f64>)> {
    let input_position = sanitize::finite_vec2_f32(input_position)?;
    let viewport_position = sanitize::finite_vec2_f32(viewport_position)?;
    let line_height = if line_height.is_finite() && line_height > 0.0 {
        line_height
    } else {
        16.0
    };
    let mut client_position = [
        f64::from(input_position[0] - viewport_position[0]),
        f64::from(input_position[1] - viewport_position[1]),
    ];
    let mut line_height = f64::from(line_height);
    if native_desktop_coordinate_space() == DesktopCoordinateSpace::Physical {
        let scale = sanitized_scale(window.scale_factor());
        client_position[0] /= scale;
        client_position[1] /= scale;
        line_height /= scale;
    }
    if !client_position.into_iter().all(f64::is_finite)
        || !line_height.is_finite()
        || line_height <= 0.0
    {
        return None;
    }
    Some((
        LogicalPosition::new(client_position[0], client_position[1]),
        LogicalSize::new(line_height, line_height),
    ))
}

#[cfg(target_os = "windows")]
mod windows {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::HiDpi::AdjustWindowRectExForDpi;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GWL_EXSTYLE, GWL_STYLE, GetWindowLongW};
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use winit::window::Window;

    use crate::sanitize;

    pub(super) fn outer_position_from_client_style(
        window: &Window,
        client_position: [f32; 2],
        dpi_scale: f32,
    ) -> Option<[f32; 2]> {
        let handle = window.window_handle().ok()?.as_raw();
        let RawWindowHandle::Win32(handle) = handle else {
            return None;
        };
        let hwnd = handle.hwnd.get() as windows_sys::Win32::Foundation::HWND;
        let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
        let ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
        let dpi_scale = sanitize::positive_finite_f32_or(
            dpi_scale,
            sanitize::positive_finite_f32_or(window.scale_factor() as f32, 1.0),
        );
        let dpi = (dpi_scale * 96.0).round().clamp(1.0, u32::MAX as f32) as u32;
        let mut rect = RECT {
            left: client_position[0].round() as i32,
            top: client_position[1].round() as i32,
            right: client_position[0].round() as i32,
            bottom: client_position[1].round() as i32,
        };
        let adjusted = unsafe { AdjustWindowRectExForDpi(&mut rect, style, 0, ex_style, dpi) } != 0;
        adjusted.then_some([rect.left as f32, rect.top as f32])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor_from_values(
        space: DesktopCoordinateSpace,
        position: [f64; 2],
        size: [f64; 2],
        scale: f64,
    ) -> dear_imgui_rs::sys::ImGuiPlatformMonitor {
        let position =
            desktop_position_from_physical_values(space, position, scale).unwrap_or([0.0, 0.0]);
        let size = desktop_size_from_physical_values(space, size, scale);
        dear_imgui_rs::sys::ImGuiPlatformMonitor {
            MainPos: dear_imgui_rs::sys::ImVec2 {
                x: position[0] as f32,
                y: position[1] as f32,
            },
            MainSize: dear_imgui_rs::sys::ImVec2 {
                x: size[0],
                y: size[1],
            },
            WorkPos: dear_imgui_rs::sys::ImVec2 {
                x: position[0] as f32,
                y: position[1] as f32,
            },
            WorkSize: dear_imgui_rs::sys::ImVec2 {
                x: size[0],
                y: size[1],
            },
            DpiScale: sanitize::positive_finite_f32_or(sanitized_scale(scale) as f32, 1.0),
            PlatformHandle: std::ptr::null_mut(),
        }
    }

    #[test]
    fn physical_desktop_monitors_keep_a_mixed_dpi_layout_contiguous() {
        let primary = monitor_from_values(
            DesktopCoordinateSpace::Physical,
            [0.0, 0.0],
            [1920.0, 1080.0],
            1.0,
        );
        let secondary = monitor_from_values(
            DesktopCoordinateSpace::Physical,
            [1920.0, 0.0],
            [2560.0, 1440.0],
            1.5,
        );

        assert_eq!(primary.MainPos.x, 0.0);
        assert_eq!(primary.MainSize.x, 1920.0);
        assert_eq!(secondary.MainPos.x, 1920.0);
        assert_eq!(secondary.MainSize.x, 2560.0);
        assert_eq!(secondary.DpiScale, 1.5);
    }

    #[test]
    fn monitor_conversion_preserves_negative_origins_and_sanitizes_scale() {
        let monitor = monitor_from_values(
            DesktopCoordinateSpace::Physical,
            [-1600.0, -200.0],
            [1600.0, 900.0],
            f64::NAN,
        );

        assert_eq!(monitor.MainPos.x, -1600.0);
        assert_eq!(monitor.MainPos.y, -200.0);
        assert_eq!(monitor.WorkPos, monitor.MainPos);
        assert_eq!(monitor.WorkSize, monitor.MainSize);
        assert_eq!(monitor.DpiScale, 1.0);
    }

    #[test]
    fn snapshot_conversion_preserves_distinct_main_and_work_rectangles() {
        use crate::native_support::{
            MonitorIdentity, MonitorSnapshot, PhysicalMonitorRect, WorkAreaProvenance,
        };

        let main = PhysicalMonitorRect::new([-1920.0, 0.0], [1920.0, 1080.0]).unwrap();
        let work = PhysicalMonitorRect::new([-1920.0, 45.0], [1920.0, 1035.0]).unwrap();
        let snapshot = MonitorSnapshot::from_test(
            MonitorIdentity::from_test_key("secondary"),
            main,
            work,
            1.5,
            WorkAreaProvenance::WindowsRcWork,
        );

        let monitor = monitor_from_snapshot(&snapshot).unwrap();
        let divisor = match native_desktop_coordinate_space() {
            DesktopCoordinateSpace::Physical => 1.0,
            DesktopCoordinateSpace::Logical => 1.5,
        };
        assert_eq!(monitor.MainPos.x, (-1920.0 / divisor) as f32);
        assert_eq!(monitor.MainPos.y, 0.0);
        assert_eq!(monitor.MainSize.x, (1920.0 / divisor) as f32);
        assert_eq!(monitor.MainSize.y, (1080.0 / divisor) as f32);
        assert_eq!(monitor.WorkPos.x, (-1920.0 / divisor) as f32);
        assert_eq!(monitor.WorkPos.y, (45.0 / divisor) as f32);
        assert_eq!(monitor.WorkSize.x, (1920.0 / divisor) as f32);
        assert_eq!(monitor.WorkSize.y, (1035.0 / divisor) as f32);
        assert_eq!(monitor.DpiScale, 1.5);
    }

    #[test]
    fn physical_client_input_uses_the_same_desktop_units_as_window_origins() {
        let position = client_to_screen_for_space(
            DesktopCoordinateSpace::Physical,
            [320.5, 72.0],
            [1920.0, -140.0],
            1.5,
        );
        assert_eq!(position, Some([2240.5, -68.0]));
    }

    #[test]
    fn logical_client_input_converts_both_origin_and_local_position_once() {
        let position = client_to_screen_for_space(
            DesktopCoordinateSpace::Logical,
            [300.0, 150.0],
            [2880.0, 600.0],
            1.5,
        );
        assert_eq!(position, Some([2120.0, 500.0]));
    }

    #[test]
    fn framebuffer_scale_is_independent_from_monitor_dpi_in_physical_space() {
        assert_eq!(
            framebuffer_scale_for_space(DesktopCoordinateSpace::Physical, 1.5),
            [1.0, 1.0]
        );
        assert_eq!(
            framebuffer_scale_for_space(DesktopCoordinateSpace::Logical, 2.0),
            [2.0, 2.0]
        );
    }

    #[test]
    fn single_window_metrics_restore_logical_units_after_native_desktop_units() {
        assert_eq!(
            single_window_display_metrics_from_physical(PhysicalSize::new(1200_u32, 900_u32), 1.5,),
            ([800.0, 600.0], [1.5, 1.5])
        );
    }

    #[test]
    fn physical_desktop_ime_area_converts_back_to_winit_client_logical_units() {
        let (position, size) = ime_cursor_area_for_space(
            DesktopCoordinateSpace::Physical,
            [2220.0, 360.0],
            [1920.0, 0.0],
            30.0,
            1.5,
        )
        .unwrap();

        assert_eq!(position, LogicalPosition::new(200.0, 240.0));
        assert_eq!(size, LogicalSize::new(20.0, 20.0));
    }

    #[test]
    fn undecorated_client_origin_needs_no_outer_position_offset() {
        assert_eq!(
            outer_position_from_decoration_offset([2447.0, 802.0], [0.0, 0.0]),
            Some([2447.0, 802.0])
        );
    }

    #[test]
    fn observed_decoration_offset_positions_the_client_origin() {
        assert_eq!(
            outer_position_from_decoration_offset([2447.0, 802.0], [11.0, 45.0]),
            Some([2436.0, 757.0])
        );
    }

    #[test]
    fn scale_factor_change_preserves_the_backends_desktop_size_unit() {
        let current = PhysicalSize::new(800, 600);
        assert_eq!(
            scale_factor_inner_size_override_for_space(
                DesktopCoordinateSpace::Physical,
                false,
                current,
            ),
            Some(current)
        );
        assert_eq!(
            scale_factor_inner_size_override_for_space(
                DesktopCoordinateSpace::Physical,
                true,
                current,
            ),
            None
        );
        assert_eq!(
            scale_factor_inner_size_override_for_space(
                DesktopCoordinateSpace::Logical,
                false,
                current,
            ),
            None
        );
    }

    fn client_to_screen_for_space(
        space: DesktopCoordinateSpace,
        client: [f64; 2],
        origin: [f64; 2],
        scale: f64,
    ) -> Option<[f32; 2]> {
        let client = desktop_position_from_physical_values(space, client, scale)?;
        let origin = desktop_position_from_physical_values(space, origin, scale)?;
        sanitize::finite_vec2_f64_to_f32([origin[0] + client[0], origin[1] + client[1]])
    }

    fn ime_cursor_area_for_space(
        space: DesktopCoordinateSpace,
        input_position: [f32; 2],
        viewport_position: [f32; 2],
        line_height: f32,
        scale: f64,
    ) -> Option<(LogicalPosition<f64>, LogicalSize<f64>)> {
        let input_position = sanitize::finite_vec2_f32(input_position)?;
        let viewport_position = sanitize::finite_vec2_f32(viewport_position)?;
        let mut client_position = [
            f64::from(input_position[0] - viewport_position[0]),
            f64::from(input_position[1] - viewport_position[1]),
        ];
        let mut line_height = f64::from(line_height);
        if space == DesktopCoordinateSpace::Physical {
            let scale = sanitized_scale(scale);
            client_position[0] /= scale;
            client_position[1] /= scale;
            line_height /= scale;
        }
        Some((
            LogicalPosition::new(client_position[0], client_position[1]),
            LogicalSize::new(line_height, line_height),
        ))
    }
}

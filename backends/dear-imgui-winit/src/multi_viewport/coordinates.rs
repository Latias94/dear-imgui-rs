use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize, Position, Size};
use winit::window::Window;

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

fn desktop_input_position_from_physical(position: [f64; 2], scale: f64) -> Option<[f64; 2]> {
    desktop_position_from_physical_values(native_desktop_coordinate_space(), position, scale)
}

pub(super) fn monitor_from_physical(
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    scale: f64,
) -> dear_imgui_rs::sys::ImGuiPlatformMonitor {
    let scale = sanitized_scale(scale);
    let position = desktop_position_from_physical(position, scale).unwrap_or([0.0, 0.0]);
    let size = desktop_size_from_physical_values(
        native_desktop_coordinate_space(),
        [f64::from(size.width), f64::from(size.height)],
        scale,
    );

    dear_imgui_rs::sys::ImGuiPlatformMonitor {
        MainPos: dear_imgui_rs::sys::ImVec2 {
            x: position[0],
            y: position[1],
        },
        MainSize: dear_imgui_rs::sys::ImVec2 {
            x: size[0],
            y: size[1],
        },
        // Winit does not expose platform work areas, so the whole monitor remains the portable
        // fallback until a backend-specific work-area callback is available.
        WorkPos: dear_imgui_rs::sys::ImVec2 {
            x: position[0],
            y: position[1],
        },
        WorkSize: dear_imgui_rs::sys::ImVec2 {
            x: size[0],
            y: size[1],
        },
        DpiScale: sanitize::positive_finite_f32_or(scale as f32, 1.0),
        PlatformHandle: std::ptr::null_mut(),
    }
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

pub(crate) fn client_physical_to_screen_pos(
    window: &Window,
    client_position: [f64; 2],
) -> Option<[f32; 2]> {
    let scale = window.scale_factor();
    let client = desktop_input_position_from_physical(client_position, scale)?;
    let origin = window.inner_position().ok()?;
    let origin = desktop_position_from_physical_values(
        native_desktop_coordinate_space(),
        [f64::from(origin.x), f64::from(origin.y)],
        scale,
    )?;
    sanitize::finite_vec2_f64_to_f32([origin[0] + client[0], origin[1] + client[1]])
}

pub(super) fn decoration_offset(window: &Window) -> Option<(f64, f64)> {
    let scale = window.scale_factor();
    let inner = window.inner_position().ok()?;
    let outer = window.outer_position().ok()?;
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
    let offset = [inner[0] - outer[0], inner[1] - outer[1]];
    sanitize::finite_vec2_f64_to_f32(offset)?;
    Some((offset[0], offset[1]))
}

pub(super) fn outer_position_from_client(
    window: &Window,
    client_position: [f32; 2],
    dpi_scale: f32,
) -> [f32; 2] {
    #[cfg(target_os = "windows")]
    if let Some(position) = windows::outer_position_from_client(window, client_position, dpi_scale)
    {
        return position;
    }

    let _ = dpi_scale;
    decoration_offset(window)
        .and_then(|(dx, dy)| {
            sanitize::finite_vec2_f32([
                client_position[0] - dx as f32,
                client_position[1] - dy as f32,
            ])
        })
        .unwrap_or(client_position)
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

    pub(super) fn outer_position_from_client(
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

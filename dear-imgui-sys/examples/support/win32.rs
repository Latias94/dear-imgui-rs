//! Shared HWND ownership and message forwarding for the raw Win32 examples.

use dear_imgui_sys::{self as sys, backend_shim::win32};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::{
            GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{HiDpi::AdjustWindowRectExForDpi, WindowsAndMessaging::*},
    },
    core::PCWSTR,
};

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: Win32 supplies the message; the official handler accepts an absent ImGui context.
    unsafe {
        if win32::dear_imgui_backend_win32_wnd_proc_handler(hwnd.0, message, wparam.0, lparam.0)
            != 0
        {
            return LRESULT(1);
        }
        if message == WM_CLOSE {
            // The application drops its backend guards before destroying HWND.
            PostQuitMessage(0);
            return LRESULT(0);
        }
        DefWindowProcW(hwnd, message, wparam, lparam)
    }
}

pub struct Window(pub HWND);

impl Window {
    pub fn new(class_name: PCWSTR, title: PCWSTR) -> windows::core::Result<Self> {
        // SAFETY: examples pass static wide strings; this thread owns window creation and messages.
        unsafe {
            let instance = HINSTANCE(GetModuleHandleW(None)?.0);
            let class = WNDCLASSW {
                style: CS_OWNDC,
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                lpszClassName: class_name,
                ..Default::default()
            };
            if RegisterClassW(&class) == 0 {
                return Err(windows::core::Error::from_thread());
            }
            let window = Self(CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class_name,
                title,
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                960,
                640,
                None,
                None,
                Some(instance),
                None,
            )?);

            // Treat the requested client size as 96-DPI logical pixels. Query the actual HWND
            // after creation so secondary monitors use their own DPI rather than the primary's.
            let scale = win32::dear_imgui_backend_win32_get_dpi_scale_for_hwnd(window.0.0);
            let mut client = RECT {
                right: (960.0 * scale).round() as i32,
                bottom: (640.0 * scale).round() as i32,
                ..Default::default()
            };
            AdjustWindowRectExForDpi(
                &mut client,
                WS_OVERLAPPEDWINDOW,
                false,
                WINDOW_EX_STYLE(0),
                (96.0 * scale).round() as u32,
            )?;
            let mut monitor = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if !GetMonitorInfoW(
                MonitorFromWindow(window.0, MONITOR_DEFAULTTONEAREST),
                &mut monitor,
            )
            .as_bool()
            {
                return Err(windows::core::Error::from_thread());
            }
            let work = monitor.rcWork;
            let width = (client.right - client.left)
                .min(work.right - work.left)
                .max(1);
            let height = (client.bottom - client.top)
                .min(work.bottom - work.top)
                .max(1);
            let mut position = RECT::default();
            GetWindowRect(window.0, &mut position)?;
            SetWindowPos(
                window.0,
                None,
                position.left.clamp(work.left, work.right - width),
                position.top.clamp(work.top, work.bottom - height),
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )?;
            Ok(window)
        }
    }

    pub fn show(&self) {
        // SAFETY: HWND stays live until this guard drops.
        unsafe {
            let _ = ShowWindow(self.0, SW_SHOW);
        }
    }

    pub fn pump_events(&self) -> bool {
        // SAFETY: messages are consumed and dispatched on the creating thread.
        unsafe {
            let mut message = MSG::default();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                if message.message == WM_QUIT {
                    return false;
                }
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        true
    }

    pub fn client_size(&self) -> windows::core::Result<(u32, u32)> {
        // SAFETY: HWND stays live until this guard drops.
        unsafe {
            let mut rect = RECT::default();
            GetClientRect(self.0, &mut rect)?;
            Ok((
                (rect.right - rect.left).max(0) as u32,
                (rect.bottom - rect.top).max(0) as u32,
            ))
        }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        // SAFETY: GPU/backend guards drop before this owning window guard.
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}

/// Absolute per-monitor scaling for the raw examples' ImGui style.
pub struct DpiStyle {
    context: *mut sys::ImGuiContext,
    baseline: sys::ImGuiStyle,
    scale: f32,
}

impl DpiStyle {
    /// # Safety
    /// A live ImGui context and the Win32 backend for `window` must be current on this thread.
    /// Call outside a frame, without borrowed style references. Keep the context alive and current
    /// for every update. Set the application's unscaled style before constructing this helper.
    pub unsafe fn new(window: &Window) -> Self {
        unsafe {
            let context = sys::igGetCurrentContext();
            assert!(
                !context.is_null(),
                "DPI style requires a current ImGui context"
            );
            let io = sys::igGetIO_Nil();
            // This helper owns font and widget scaling together, using the attached HWND's DPI.
            // Keep core font scaling disabled so there is one source for FontScaleDpi.
            (*io).ConfigDpiScaleFonts = false;
            // The official WndProc consumes WM_DPICHANGED's suggested rectangle with this enabled.
            (*io).ConfigDpiScaleViewports = true;
            let mut dpi = Self {
                context,
                baseline: *sys::igGetStyle(),
                scale: 0.0,
            };
            dpi.update(window);
            dpi
        }
    }

    /// # Safety
    /// The context captured by `new` must be current and outside a frame, with no borrowed style
    /// references. `window` must be the live window attached to its Win32 backend on this thread.
    pub unsafe fn update(&mut self, window: &Window) -> f32 {
        unsafe {
            assert_eq!(
                sys::igGetCurrentContext(),
                self.context,
                "DPI style context changed"
            );
            let scale = win32::dear_imgui_backend_win32_get_dpi_scale_for_hwnd(window.0.0);
            if scale != self.scale {
                // Reapply from the original values so monitor changes never accumulate rounding.
                let style = sys::igGetStyle();
                *style = self.baseline;
                sys::ImGuiStyle_ScaleAllSizes(style, scale);
                (*style).FontScaleDpi = scale;
                self.scale = scale;
            }
            scale
        }
    }
}

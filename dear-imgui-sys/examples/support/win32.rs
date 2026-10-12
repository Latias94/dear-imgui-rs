//! Shared HWND ownership and message forwarding for the raw Win32 examples.

use dear_imgui_sys::backend_shim::win32;
use windows::{
    Win32::{Foundation::*, System::LibraryLoader::GetModuleHandleW, UI::WindowsAndMessaging::*},
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
            Ok(Self(CreateWindowExW(
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
            )?))
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

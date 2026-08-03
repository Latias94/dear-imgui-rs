use winit::window::Window;

use super::WinitPlatformError;

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct NativeMouseState {
    pub(super) position: [i32; 2],
    pub(super) hovered_window: Option<usize>,
    pub(super) focused_window: Option<usize>,
}

#[cfg(target_os = "windows")]
struct WindowsCursorHitTestState {
    input_enabled: std::sync::atomic::AtomicBool,
    no_focus_on_click: std::sync::atomic::AtomicBool,
}

pub(super) struct NativeCursorHitTest {
    #[cfg(target_os = "windows")]
    windows: windows::WindowsCursorHitTest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MouseCaptureTransfer {
    NotOwned,
    Transferred,
}

impl NativeCursorHitTest {
    pub(super) fn install(window: &Window) -> Result<Self, WinitPlatformError> {
        #[cfg(target_os = "windows")]
        {
            Ok(Self {
                windows: windows::WindowsCursorHitTest::install(window)?,
            })
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = window;
            Ok(Self {})
        }
    }

    pub(super) fn set_enabled(
        &self,
        window: &Window,
        enabled: bool,
    ) -> Result<(), WinitPlatformError> {
        #[cfg(target_os = "windows")]
        {
            let _ = window;
            self.windows.set_enabled(enabled);
            Ok(())
        }

        #[cfg(not(target_os = "windows"))]
        {
            window.set_cursor_hittest(enabled).map_err(|error| {
                WinitPlatformError::WindowOperation {
                    operation: "set_cursor_hittest",
                    message: error.to_string(),
                }
            })
        }
    }

    pub(super) fn set_no_focus_on_click(
        &self,
        window: &Window,
        enabled: bool,
    ) -> Result<(), WinitPlatformError> {
        #[cfg(target_os = "windows")]
        {
            let _ = window;
            self.windows.set_no_focus_on_click(enabled);
            Ok(())
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = (window, enabled);
            Ok(())
        }
    }

    #[cfg(target_os = "windows")]
    pub(super) fn native_window_id(&self) -> usize {
        self.windows.native_window_id()
    }
}

#[cfg(target_os = "windows")]
pub(super) fn query_native_mouse_state() -> Option<NativeMouseState> {
    windows::query_native_mouse_state()
}

pub(super) fn transfer_mouse_capture(
    source: &Window,
    target: &Window,
) -> Result<MouseCaptureTransfer, WinitPlatformError> {
    #[cfg(target_os = "windows")]
    {
        windows::transfer_mouse_capture(source, target)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (source, target);
        Ok(MouseCaptureTransfer::NotOwned)
    }
}

pub(super) fn raise_window_without_activation(window: &Window) -> Result<(), WinitPlatformError> {
    #[cfg(target_os = "windows")]
    {
        windows::raise_window_without_activation(window)
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = window;
        Ok(())
    }
}

pub(super) fn focus_and_raise_window(window: &Window) -> Result<(), WinitPlatformError> {
    #[cfg(target_os = "windows")]
    {
        windows::focus_and_raise_window(window)
    }

    #[cfg(not(target_os = "windows"))]
    {
        window.focus_window();
        Ok(())
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use std::io;
    use std::sync::atomic::{AtomicBool, Ordering};

    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetCapture, ReleaseCapture, SetCapture, SetFocus,
    };
    use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetCursorPos, GetForegroundWindow, HTTRANSPARENT, HWND_TOP,
        MA_NOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetForegroundWindow, SetWindowPos,
        WM_MOUSEACTIVATE, WM_NCDESTROY, WM_NCHITTEST, WindowFromPoint,
    };
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use winit::window::Window;

    use super::{MouseCaptureTransfer, NativeMouseState, WinitPlatformError};

    pub(super) struct WindowsCursorHitTest {
        hwnd: HWND,
        state: Box<super::WindowsCursorHitTestState>,
        subclass_id: usize,
    }

    impl WindowsCursorHitTest {
        pub(super) fn install(window: &Window) -> Result<Self, WinitPlatformError> {
            let hwnd = window_handle(window)?;
            let state = Box::new(super::WindowsCursorHitTestState {
                input_enabled: AtomicBool::new(true),
                no_focus_on_click: AtomicBool::new(false),
            });
            let subclass_id = state.as_ref() as *const super::WindowsCursorHitTestState as usize;
            let installed = unsafe {
                SetWindowSubclass(
                    hwnd,
                    Some(cursor_hittest_subclass),
                    subclass_id,
                    subclass_id,
                )
            };
            if installed == 0 {
                return Err(WinitPlatformError::WindowOperation {
                    operation: "install Win32 viewport hit-test hook",
                    message: format!(
                        "SetWindowSubclass returned FALSE ({})",
                        io::Error::last_os_error()
                    ),
                });
            }

            Ok(Self {
                hwnd,
                state,
                subclass_id,
            })
        }

        pub(super) fn native_window_id(&self) -> usize {
            self.hwnd as usize
        }

        pub(super) fn set_enabled(&self, enabled: bool) {
            self.state.input_enabled.store(enabled, Ordering::Release);
        }

        pub(super) fn set_no_focus_on_click(&self, enabled: bool) {
            self.state
                .no_focus_on_click
                .store(enabled, Ordering::Release);
        }
    }

    impl Drop for WindowsCursorHitTest {
        fn drop(&mut self) {
            unsafe {
                RemoveWindowSubclass(self.hwnd, Some(cursor_hittest_subclass), self.subclass_id);
            }
        }
    }

    unsafe extern "system" fn cursor_hittest_subclass(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        subclass_id: usize,
        reference_data: usize,
    ) -> LRESULT {
        let state = unsafe { (reference_data as *const super::WindowsCursorHitTestState).as_ref() };
        let input_enabled = state.is_none_or(|state| state.input_enabled.load(Ordering::Acquire));
        if message == WM_NCHITTEST && !input_enabled {
            return HTTRANSPARENT as LRESULT;
        }
        if message == WM_MOUSEACTIVATE
            && state.is_some_and(|state| state.no_focus_on_click.load(Ordering::Acquire))
        {
            return MA_NOACTIVATE as LRESULT;
        }
        if message == WM_NCDESTROY {
            unsafe {
                RemoveWindowSubclass(hwnd, Some(cursor_hittest_subclass), subclass_id);
            }
        }
        unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
    }

    fn window_handle(window: &Window) -> Result<HWND, WinitPlatformError> {
        let handle = window
            .window_handle()
            .map_err(|error| WinitPlatformError::WindowOperation {
                operation: "query Win32 window handle",
                message: error.to_string(),
            })?
            .as_raw();
        let RawWindowHandle::Win32(handle) = handle else {
            return Err(WinitPlatformError::WindowOperation {
                operation: "query Win32 window handle",
                message: "Winit returned a non-Win32 window handle on Windows".to_owned(),
            });
        };
        Ok(handle.hwnd.get() as HWND)
    }

    pub(super) fn query_native_mouse_state() -> Option<NativeMouseState> {
        let mut position = POINT { x: 0, y: 0 };
        if unsafe { GetCursorPos(&mut position) } == 0 {
            return None;
        }

        let hovered_window = unsafe { WindowFromPoint(position) };
        let focused_window = unsafe { GetForegroundWindow() };
        Some(NativeMouseState {
            position: [position.x, position.y],
            hovered_window: (!hovered_window.is_null()).then_some(hovered_window as usize),
            focused_window: (!focused_window.is_null()).then_some(focused_window as usize),
        })
    }

    pub(super) fn transfer_mouse_capture(
        source: &Window,
        target: &Window,
    ) -> Result<MouseCaptureTransfer, WinitPlatformError> {
        let source = window_handle(source)?;
        let target = window_handle(target)?;
        if unsafe { GetCapture() } != source {
            return Ok(MouseCaptureTransfer::NotOwned);
        }

        if unsafe { ReleaseCapture() } == 0 {
            return Err(WinitPlatformError::WindowOperation {
                operation: "release Win32 viewport mouse capture",
                message: io::Error::last_os_error().to_string(),
            });
        }
        unsafe {
            SetCapture(target);
        }
        if unsafe { GetCapture() } != target {
            return Err(WinitPlatformError::WindowOperation {
                operation: "transfer Win32 viewport mouse capture",
                message: io::Error::last_os_error().to_string(),
            });
        }
        Ok(MouseCaptureTransfer::Transferred)
    }

    pub(super) fn raise_window_without_activation(
        window: &Window,
    ) -> Result<(), WinitPlatformError> {
        let hwnd = window_handle(window)?;
        let raised = unsafe {
            SetWindowPos(
                hwnd,
                HWND_TOP,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        };
        if raised == 0 {
            return Err(WinitPlatformError::WindowOperation {
                operation: "raise Win32 viewport without activation",
                message: io::Error::last_os_error().to_string(),
            });
        }
        Ok(())
    }

    pub(super) fn focus_and_raise_window(window: &Window) -> Result<(), WinitPlatformError> {
        let hwnd = window_handle(window)?;
        unsafe {
            // Match the official Win32 backend. SetForegroundWindow may be denied by the OS
            // foreground-lock policy, so these focus requests are intentionally best effort.
            BringWindowToTop(hwnd);
            SetForegroundWindow(hwnd);
            SetFocus(hwnd);
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn no_inputs_maps_only_hit_testing_to_transparent() {
            let state = super::super::WindowsCursorHitTestState {
                input_enabled: AtomicBool::new(false),
                no_focus_on_click: AtomicBool::new(false),
            };
            let reference_data = &state as *const super::super::WindowsCursorHitTestState as usize;

            assert_eq!(
                unsafe {
                    cursor_hittest_subclass(
                        std::ptr::null_mut(),
                        WM_NCHITTEST,
                        0,
                        0,
                        1,
                        reference_data,
                    )
                },
                HTTRANSPARENT as LRESULT
            );
        }

        #[test]
        fn no_focus_on_click_returns_no_activate() {
            let state = super::super::WindowsCursorHitTestState {
                input_enabled: AtomicBool::new(true),
                no_focus_on_click: AtomicBool::new(true),
            };
            let reference_data = &state as *const super::super::WindowsCursorHitTestState as usize;

            assert_eq!(
                unsafe {
                    cursor_hittest_subclass(
                        std::ptr::null_mut(),
                        WM_MOUSEACTIVATE,
                        0,
                        0,
                        1,
                        reference_data,
                    )
                },
                MA_NOACTIVATE as LRESULT
            );
        }
    }
}

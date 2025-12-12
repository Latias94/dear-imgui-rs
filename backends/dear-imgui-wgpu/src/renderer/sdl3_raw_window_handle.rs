//! SDL3 raw-window-handle adapters for WGPU multi-viewport.
//!
//! The SDL3 platform backend stores `ImGuiViewport::PlatformHandle` as an SDL_WindowID.
//! This module converts that ID back into an `SDL_Window*` and exposes window/display
//! handles compatible with `wgpu::Instance::create_surface`.

use core::ptr::NonNull;
#[cfg(all(
    unix,
    not(target_os = "macos"),
    not(target_os = "ios"),
    not(target_os = "android")
))]
use std::ffi::CStr;

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WindowHandle,
};

/// A surface target backed by an SDL3 window.
///
/// This implements `HasWindowHandle`/`HasDisplayHandle` so it can be passed to
/// `wgpu::Instance::create_surface`.
pub struct Sdl3SurfaceTarget {
    window: *mut sdl3_sys::video::SDL_Window,
}

// SAFETY: This type only contains a raw SDL_Window pointer and is used
// synchronously to create a WGPU surface on the main thread. The caller must
// ensure the underlying SDL window remains valid for the lifetime of the surface.
unsafe impl Send for Sdl3SurfaceTarget {}
unsafe impl Sync for Sdl3SurfaceTarget {}

impl Sdl3SurfaceTarget {
    /// Create a target from a raw SDL3 window pointer.
    ///
    /// # Safety
    ///
    /// `window` must be a valid `SDL_Window*` and remain alive while surfaces
    /// created from this target are in use.
    pub const unsafe fn new(window: *mut sdl3_sys::video::SDL_Window) -> Self {
        Self { window }
    }

    /// Create a target from an SDL3 window id.
    ///
    /// # Safety
    ///
    /// `id` must refer to a live SDL3 window.
    pub unsafe fn from_window_id(id: sdl3_sys::video::SDL_WindowID) -> Option<Self> {
        let window = unsafe { sdl3_sys::video::SDL_GetWindowFromID(id) };
        if window.is_null() {
            None
        } else {
            Some(unsafe { Self::new(window) })
        }
    }

    pub fn raw_window(&self) -> *mut sdl3_sys::video::SDL_Window {
        self.window
    }
}

impl HasWindowHandle for Sdl3SurfaceTarget {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        // Windows
        #[cfg(target_os = "windows")]
        unsafe {
            use raw_window_handle::Win32WindowHandle;
            use std::num::NonZeroIsize;

            let window_properties = sdl3_sys::video::SDL_GetWindowProperties(self.window);

            let hwnd = sdl3_sys::properties::SDL_GetPointerProperty(
                window_properties,
                sdl3_sys::video::SDL_PROP_WINDOW_WIN32_HWND_POINTER,
                std::ptr::null_mut(),
            );
            if hwnd.is_null() {
                return Err(HandleError::Unavailable);
            }

            let hinstance = sdl3_sys::properties::SDL_GetPointerProperty(
                window_properties,
                sdl3_sys::video::SDL_PROP_WINDOW_WIN32_INSTANCE_POINTER,
                std::ptr::null_mut(),
            );

            let mut handle = Win32WindowHandle::new(
                NonZeroIsize::new(hwnd as isize).ok_or(HandleError::Unavailable)?,
            );
            handle.hinstance = NonZeroIsize::new(hinstance as isize);
            Ok(WindowHandle::borrow_raw(RawWindowHandle::Win32(handle)))
        }

        // macOS
        #[cfg(target_os = "macos")]
        unsafe {
            use objc2::{msg_send, runtime::NSObject};
            use raw_window_handle::AppKitWindowHandle;

            let window_properties = sdl3_sys::video::SDL_GetWindowProperties(self.window);

            let ns_window = sdl3_sys::properties::SDL_GetPointerProperty(
                window_properties,
                sdl3_sys::video::SDL_PROP_WINDOW_COCOA_WINDOW_POINTER,
                std::ptr::null_mut(),
            );
            if ns_window.is_null() {
                return Err(HandleError::Unavailable);
            }

            let ns_view: *mut NSObject = msg_send![ns_window as *mut NSObject, contentView];
            if ns_view.is_null() {
                return Err(HandleError::Unavailable);
            }

            let handle = AppKitWindowHandle::new(NonNull::new_unchecked(ns_view.cast()));
            Ok(WindowHandle::borrow_raw(RawWindowHandle::AppKit(handle)))
        }

        // iOS
        #[cfg(target_os = "ios")]
        unsafe {
            use raw_window_handle::UiKitWindowHandle;

            let window_properties = sdl3_sys::video::SDL_GetWindowProperties(self.window);

            let ui_view = sdl3_sys::properties::SDL_GetPointerProperty(
                window_properties,
                sdl3_sys::video::SDL_PROP_WINDOW_UIKIT_WINDOW_POINTER,
                std::ptr::null_mut(),
            );
            let ui_view = NonNull::new(ui_view).ok_or(HandleError::Unavailable)?;
            let handle = UiKitWindowHandle::new(ui_view.cast());
            Ok(WindowHandle::borrow_raw(RawWindowHandle::UiKit(handle)))
        }

        // Android
        #[cfg(target_os = "android")]
        unsafe {
            use raw_window_handle::AndroidNdkWindowHandle;

            let window_properties = sdl3_sys::video::SDL_GetWindowProperties(self.window);

            let native_window = sdl3_sys::properties::SDL_GetPointerProperty(
                window_properties,
                sdl3_sys::video::SDL_PROP_WINDOW_ANDROID_WINDOW_POINTER,
                std::ptr::null_mut(),
            );
            let native_window = NonNull::new(native_window).ok_or(HandleError::Unavailable)?;
            let handle = AndroidNdkWindowHandle::new(native_window.cast());
            Ok(WindowHandle::borrow_raw(RawWindowHandle::AndroidNdk(
                handle,
            )))
        }

        // Linux (X11 or Wayland)
        #[cfg(all(
            unix,
            not(target_os = "macos"),
            not(target_os = "ios"),
            not(target_os = "android")
        ))]
        unsafe {
            let video_driver_ptr = sdl3_sys::video::SDL_GetCurrentVideoDriver();
            if video_driver_ptr.is_null() {
                return Err(HandleError::Unavailable);
            }
            let video_driver = CStr::from_ptr(video_driver_ptr);

            match video_driver.to_bytes() {
                b"x11" => {
                    use raw_window_handle::XlibWindowHandle;

                    let window_properties = sdl3_sys::video::SDL_GetWindowProperties(self.window);

                    let window_num = sdl3_sys::properties::SDL_GetNumberProperty(
                        window_properties,
                        sdl3_sys::video::SDL_PROP_WINDOW_X11_WINDOW_NUMBER,
                        0,
                    );
                    let handle = XlibWindowHandle::new(window_num as u64);
                    Ok(WindowHandle::borrow_raw(RawWindowHandle::Xlib(handle)))
                }
                b"wayland" => {
                    use raw_window_handle::WaylandWindowHandle;

                    let window_properties = sdl3_sys::video::SDL_GetWindowProperties(self.window);

                    let surface = sdl3_sys::properties::SDL_GetPointerProperty(
                        window_properties,
                        sdl3_sys::video::SDL_PROP_WINDOW_WAYLAND_SURFACE_POINTER,
                        std::ptr::null_mut(),
                    );
                    let surface = NonNull::new(surface).ok_or(HandleError::Unavailable)?;
                    let handle = WaylandWindowHandle::new(surface.cast());
                    Ok(WindowHandle::borrow_raw(RawWindowHandle::Wayland(handle)))
                }
                _ => Err(HandleError::Unavailable),
            }
        }
    }
}

impl HasDisplayHandle for Sdl3SurfaceTarget {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        // Windows
        #[cfg(target_os = "windows")]
        unsafe {
            use raw_window_handle::WindowsDisplayHandle;
            let handle = WindowsDisplayHandle::new();
            Ok(DisplayHandle::borrow_raw(RawDisplayHandle::Windows(handle)))
        }

        // macOS
        #[cfg(target_os = "macos")]
        unsafe {
            use raw_window_handle::AppKitDisplayHandle;
            let handle = AppKitDisplayHandle::new();
            Ok(DisplayHandle::borrow_raw(RawDisplayHandle::AppKit(handle)))
        }

        // iOS
        #[cfg(target_os = "ios")]
        unsafe {
            use raw_window_handle::UiKitDisplayHandle;
            let handle = UiKitDisplayHandle::new();
            Ok(DisplayHandle::borrow_raw(RawDisplayHandle::UiKit(handle)))
        }

        // Android
        #[cfg(target_os = "android")]
        unsafe {
            use raw_window_handle::AndroidDisplayHandle;
            let handle = AndroidDisplayHandle::new();
            Ok(DisplayHandle::borrow_raw(RawDisplayHandle::Android(handle)))
        }

        // Linux (X11 or Wayland)
        #[cfg(all(
            unix,
            not(target_os = "macos"),
            not(target_os = "ios"),
            not(target_os = "android")
        ))]
        unsafe {
            let video_driver_ptr = sdl3_sys::video::SDL_GetCurrentVideoDriver();
            if video_driver_ptr.is_null() {
                return Err(HandleError::Unavailable);
            }
            let video_driver = CStr::from_ptr(video_driver_ptr);

            let window_properties = sdl3_sys::video::SDL_GetWindowProperties(self.window);
            match video_driver.to_bytes() {
                b"x11" => {
                    use raw_window_handle::XlibDisplayHandle;

                    let display = sdl3_sys::properties::SDL_GetPointerProperty(
                        window_properties,
                        sdl3_sys::video::SDL_PROP_WINDOW_X11_DISPLAY_POINTER,
                        std::ptr::null_mut(),
                    );
                    let display = NonNull::new(display).ok_or(HandleError::Unavailable)?;

                    let screen = sdl3_sys::properties::SDL_GetNumberProperty(
                        window_properties,
                        sdl3_sys::video::SDL_PROP_WINDOW_X11_SCREEN_NUMBER,
                        0,
                    );

                    let handle = XlibDisplayHandle::new(Some(display.cast()), screen as i32);
                    Ok(DisplayHandle::borrow_raw(RawDisplayHandle::Xlib(handle)))
                }
                b"wayland" => {
                    use raw_window_handle::WaylandDisplayHandle;

                    let display = sdl3_sys::properties::SDL_GetPointerProperty(
                        window_properties,
                        sdl3_sys::video::SDL_PROP_WINDOW_WAYLAND_DISPLAY_POINTER,
                        std::ptr::null_mut(),
                    );
                    let display = NonNull::new(display).ok_or(HandleError::Unavailable)?;

                    let handle = WaylandDisplayHandle::new(display.cast());
                    Ok(DisplayHandle::borrow_raw(RawDisplayHandle::Wayland(handle)))
                }
                _ => Err(HandleError::Unavailable),
            }
        }
    }
}

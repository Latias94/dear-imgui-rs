use std::sync::Arc;

use dear_imgui_rs::NativeWindowTarget;
use winit::window::{Window, WindowAttributes};

use super::WinitPlatformError;
#[cfg(target_os = "windows")]
use super::registry::registered_window_for_viewport_id;
use super::runtime::RuntimeControl;
use super::viewport_data::ViewportData;

pub(super) fn resolve_owner(
    control: &RuntimeControl,
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> Result<Option<(NativeWindowTarget, Arc<Window>)>, WinitPlatformError> {
    let parent_id = unsafe { (*viewport).ParentViewportId };
    if parent_id == 0 {
        return Ok(None);
    }
    if parent_id == unsafe { (*viewport).ID } {
        return Err(WinitPlatformError::InvalidWindowConfiguration {
            field: "ParentViewportId",
            reason: "a native window cannot own itself",
        });
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = control;
        // Dear ImGui publishes this default hint even when HAS_PARENT_VIEWPORT is absent.
        // Unsupported hosts leave that default unapplied, while explicit secondary-owner
        // requests fail instead of silently modifying a different native relationship.
        let main = unsafe { dear_imgui_rs::sys::igGetMainViewport() };
        if !main.is_null() && parent_id == unsafe { (*main).ID } {
            return Ok(None);
        }
        return Err(WinitPlatformError::WindowPolicyUnsupported {
            policy: "ParentViewportId",
            operation: "native owner relationship",
        });
    }
    #[cfg(target_os = "windows")]
    registered_window_for_viewport_id(control, parent_id)
        .map(Some)
        .ok_or(WinitPlatformError::NativeOwnerUnavailable {
            viewport_id: parent_id,
        })
}

pub(super) fn creation_attributes(
    attributes: WindowAttributes,
    owner: Option<&(NativeWindowTarget, Arc<Window>)>,
) -> Result<WindowAttributes, WinitPlatformError> {
    #[cfg(target_os = "windows")]
    if let Some((_, window)) = owner {
        use winit::platform::windows::WindowAttributesExtWindows;
        return Ok(attributes.with_owner_window(native_handle(window)? as isize));
    }
    #[cfg(not(target_os = "windows"))]
    let _ = owner;
    Ok(attributes)
}

pub(super) fn sync_owner(
    control: &RuntimeControl,
    data: &ViewportData,
    viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
) -> Result<(), WinitPlatformError> {
    if data.is_main() {
        return Ok(());
    }
    let owner = resolve_owner(control, viewport)?;
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::{GetLastError, SetLastError};
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GWLP_HWNDPARENT, GetWindowLongPtrW, SetWindowLongPtrW,
        };
        let requested = owner.as_ref().map(|(target, _)| target.instance_id);
        control.validate_native_owner_chain(data.instance_id(), requested)?;
        let hwnd = native_handle(data.window())?;
        let parent = owner
            .as_ref()
            .map(|(_, window)| native_handle(window))
            .transpose()?
            .unwrap_or(std::ptr::null_mut());
        if unsafe { GetWindowLongPtrW(hwnd, GWLP_HWNDPARENT) } != parent as isize {
            if data.render_state().is_pinned() {
                return Err(WinitPlatformError::WindowOperation {
                    operation: "set_native_owner",
                    message:
                        "native owner changes are deferred while recorded work retains this window"
                            .to_owned(),
                });
            }
            unsafe { SetLastError(0) };
            let previous = unsafe { SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, parent as isize) };
            if previous == 0 && unsafe { GetLastError() } != 0 {
                return Err(WinitPlatformError::WindowOperation {
                    operation: "set_native_owner",
                    message: std::io::Error::last_os_error().to_string(),
                });
            }
        }
    }
    data.set_native_owner(owner);
    Ok(())
}

#[cfg(target_os = "windows")]
fn native_handle(
    window: &Window,
) -> Result<windows_sys::Win32::Foundation::HWND, WinitPlatformError> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = window
        .window_handle()
        .map_err(|error| WinitPlatformError::WindowOperation {
            operation: "native_owner_handle",
            message: error.to_string(),
        })?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err(WinitPlatformError::UnsupportedWindowSystem {
            target: "non-Win32 owner",
        });
    };
    Ok(handle.hwnd.get() as windows_sys::Win32::Foundation::HWND)
}

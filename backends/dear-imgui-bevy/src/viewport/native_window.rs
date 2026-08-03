#[cfg(target_os = "linux")]
use std::cell::Cell;

use bevy_ecs::prelude::Entity;
#[cfg(test)]
use bevy_ecs::prelude::Resource;
use bevy_window::WindowWrapper;
use bevy_winit::WINIT_WINDOWS;
use raw_window_handle::{HasDisplayHandle, RawDisplayHandle};

/// Native windows removed from Bevy's thread-local registry during terminal shutdown.
///
/// Keeping the wrappers alive until the render world has observed `WindowClosing` guarantees
/// that their final drop happens on the caller thread instead of a render worker. This mirrors
/// Bevy's own deferred native-window destruction contract, including its macOS requirement.
pub(crate) struct NativeWindowRetirements {
    _windows: Vec<WindowWrapper<winit::window::Window>>,
    mappings_removed: usize,
}

impl NativeWindowRetirements {
    pub(crate) fn requires_render_drain(&self) -> bool {
        self.mappings_removed != 0
    }
}

pub(crate) fn retire_windows(
    entities: impl IntoIterator<Item = Entity>,
) -> NativeWindowRetirements {
    let mut windows = Vec::new();
    let mut mappings_removed = 0;
    WINIT_WINDOWS.with_borrow_mut(|winit_windows| {
        for entity in entities {
            if winit_windows.entity_to_winit.contains_key(&entity) {
                mappings_removed += 1;
            }
            if let Some(window) = winit_windows.remove_window(entity) {
                windows.push(window);
            }
        }
    });
    NativeWindowRetirements {
        _windows: windows,
        mappings_removed,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DesktopPositionSupport {
    Available,
    Unavailable,
    PendingWindow,
}

#[cfg(test)]
#[derive(Resource, Clone, Copy)]
pub(crate) struct DesktopPositionSupportOverride(pub(crate) DesktopPositionSupport);

impl DesktopPositionSupport {
    pub(super) const fn allows_native_viewports(self) -> bool {
        matches!(self, Self::Available)
    }

    pub(super) const fn can_report_hovered_viewport(self) -> bool {
        self.allows_native_viewports() && cfg!(target_os = "windows")
    }
}

impl From<DesktopPositionSupport> for super::ImguiNativeViewportStatus {
    fn from(value: DesktopPositionSupport) -> Self {
        match value {
            DesktopPositionSupport::Available => Self::Available,
            DesktopPositionSupport::PendingWindow => Self::PendingNativeWindow,
            DesktopPositionSupport::Unavailable => Self::GlobalDesktopCoordinatesUnavailable,
        }
    }
}

pub(super) const fn supports_pointer_passthrough() -> bool {
    cfg!(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "linux"
    ))
}

pub(crate) fn desktop_position_support(entity: Entity) -> DesktopPositionSupport {
    WINIT_WINDOWS.with_borrow(|windows| {
        let Some(window) = windows.get_window(entity) else {
            return DesktopPositionSupport::PendingWindow;
        };
        let Ok(display) = window.display_handle() else {
            return DesktopPositionSupport::Unavailable;
        };
        support_for_display(display.as_raw())
    })
}

const fn support_for_display(display: RawDisplayHandle) -> DesktopPositionSupport {
    match display {
        RawDisplayHandle::Windows(_)
        | RawDisplayHandle::AppKit(_)
        | RawDisplayHandle::Xlib(_)
        | RawDisplayHandle::Xcb(_) => DesktopPositionSupport::Available,
        // Wayland deliberately withholds global window positions from clients. Dear ImGui's
        // classic multi-viewport contract requires both querying and setting those positions.
        RawDisplayHandle::Wayland(_) => DesktopPositionSupport::Unavailable,
        _ => DesktopPositionSupport::Unavailable,
    }
}

#[cfg(target_os = "linux")]
thread_local! {
    static CAPTURED_X11_WINDOW: Cell<Option<Entity>> = const { Cell::new(None) };
}

/// Capture X11 pointer motion while a Dear ImGui drag is held outside its source window.
///
/// X11 does not retain pointer delivery across top-level windows by default. This mirrors the
/// capture step used by the official SDL backend and deliberately leaves the pointer unconstrained.
pub(crate) fn capture_pointer(entity: Entity) {
    #[cfg(target_os = "linux")]
    CAPTURED_X11_WINDOW.with(|current| {
        transition_x11_pointer_capture(
            current,
            entity,
            release_x11_pointer_capture,
            grab_x11_pointer,
        );
    });

    #[cfg(not(target_os = "linux"))]
    let _ = entity;
}

#[cfg(target_os = "linux")]
fn transition_x11_pointer_capture(
    current: &Cell<Option<Entity>>,
    entity: Entity,
    mut release: impl FnMut(Option<Entity>),
    mut grab: impl FnMut(Entity) -> bool,
) {
    if current.get() == Some(entity) {
        return;
    }
    release(current.replace(None));
    if grab(entity) {
        current.set(Some(entity));
    }
}

pub(crate) fn release_pointer_capture() {
    #[cfg(target_os = "linux")]
    CAPTURED_X11_WINDOW.with(|current| {
        release_x11_pointer_capture(current.get());
        current.set(None);
    });
}

pub(crate) fn release_pointer_capture_for(entity: Entity) {
    #[cfg(target_os = "linux")]
    CAPTURED_X11_WINDOW.with(|current| {
        if current.get() != Some(entity) {
            return;
        }
        release_x11_pointer_capture(Some(entity));
        current.set(None);
    });

    #[cfg(not(target_os = "linux"))]
    let _ = entity;
}

#[cfg(target_os = "linux")]
fn grab_x11_pointer(entity: Entity) -> bool {
    with_xlib_window(entity, |xlib, display, window| {
        let event_mask = (x11_dl::xlib::ButtonPressMask
            | x11_dl::xlib::ButtonReleaseMask
            | x11_dl::xlib::PointerMotionMask
            | x11_dl::xlib::EnterWindowMask
            | x11_dl::xlib::LeaveWindowMask) as u32;
        // SAFETY: the raw display and window belong to the live winit window retained by
        // WINIT_WINDOWS for this call. XGrabPointer does not retain Rust references.
        let result = unsafe {
            (xlib.XGrabPointer)(
                display,
                window,
                x11_dl::xlib::True,
                event_mask,
                x11_dl::xlib::GrabModeAsync,
                x11_dl::xlib::GrabModeAsync,
                0,
                0,
                x11_dl::xlib::CurrentTime,
            )
        };
        // SAFETY: `display` remains valid for the duration of the winit window borrow.
        unsafe { (xlib.XFlush)(display) };
        result == x11_dl::xlib::GrabSuccess
    })
    .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn release_x11_pointer_capture(entity: Option<Entity>) {
    let Some(entity) = entity else {
        return;
    };
    let _ = with_xlib_window(entity, |xlib, display, _| {
        // SAFETY: the display belongs to the live winit window retained for this call.
        unsafe {
            (xlib.XUngrabPointer)(display, x11_dl::xlib::CurrentTime);
            (xlib.XFlush)(display);
        }
    });
}

#[cfg(target_os = "linux")]
fn with_xlib_window<T>(
    entity: Entity,
    operation: impl FnOnce(&x11_dl::xlib::Xlib, *mut x11_dl::xlib::Display, x11_dl::xlib::Window) -> T,
) -> Option<T> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    WINIT_WINDOWS.with_borrow(|windows| {
        let window = windows.get_window(entity)?;
        let RawDisplayHandle::Xlib(display) = window.display_handle().ok()?.as_raw() else {
            return None;
        };
        let RawWindowHandle::Xlib(window_handle) = window.window_handle().ok()?.as_raw() else {
            return None;
        };
        let display = display.display?.as_ptr().cast::<x11_dl::xlib::Display>();
        let xlib = x11_dl::xlib::Xlib::open().ok()?;
        Some(operation(&xlib, display, window_handle.window))
    })
}

#[cfg(test)]
mod tests {
    use std::ptr::NonNull;

    use raw_window_handle::{RawDisplayHandle, WaylandDisplayHandle, XlibDisplayHandle};

    use super::{DesktopPositionSupport, support_for_display};

    #[cfg(target_os = "linux")]
    use super::transition_x11_pointer_capture;

    #[test]
    fn wayland_does_not_claim_global_desktop_positions() {
        let display = NonNull::dangling();
        assert_eq!(
            support_for_display(RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
                display
            ))),
            DesktopPositionSupport::Unavailable
        );
    }

    #[test]
    fn x11_claims_global_desktop_positions() {
        assert_eq!(
            support_for_display(RawDisplayHandle::Xlib(XlibDisplayHandle::new(
                Some(NonNull::dangling()),
                0,
            ))),
            DesktopPositionSupport::Available
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn x11_capture_releases_the_previous_owner_and_clears_failed_regrabs() {
        use std::cell::{Cell, RefCell};

        use bevy_ecs::prelude::Entity;

        let first = Entity::from_raw_u32(1).expect("test entity index should be valid");
        let second = Entity::from_raw_u32(2).expect("test entity index should be valid");
        let current = Cell::new(None);
        let released = RefCell::new(Vec::new());

        transition_x11_pointer_capture(
            &current,
            first,
            |entity| released.borrow_mut().push(entity),
            |_| true,
        );
        assert_eq!(current.get(), Some(first));

        transition_x11_pointer_capture(
            &current,
            first,
            |entity| released.borrow_mut().push(entity),
            |_| panic!("the current owner must not be grabbed twice"),
        );
        assert_eq!(current.get(), Some(first));

        transition_x11_pointer_capture(
            &current,
            second,
            |entity| released.borrow_mut().push(entity),
            |_| false,
        );
        assert_eq!(current.get(), None);
        assert_eq!(*released.borrow(), vec![None, Some(first)]);
    }
}

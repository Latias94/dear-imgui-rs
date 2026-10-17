use dear_imgui_rs::NativeWindowTarget;
use winit::window::{Theme, Window};

use super::WinitWindowSnapshot;
use super::ownership::{WinitPlatform, WinitPlatformControl};

/// Native policy availability, independent of whether a window manager honors every request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WinitWindowPolicySupport {
    Dynamic,
    CreationOnly,
    Unsupported,
}

/// Backend support for editor window policies on the observed native window system.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WinitWindowCapabilities {
    pub secondary_creation: bool,
    pub icon: WinitWindowPolicySupport,
    pub client_size_limits: WinitWindowPolicySupport,
    pub theme: WinitWindowPolicySupport,
    pub resizable: WinitWindowPolicySupport,
    pub transparency: WinitWindowPolicySupport,
    pub parent_owner: WinitWindowPolicySupport,
    pub desktop_position: WinitWindowPolicySupport,
}

impl WinitWindowCapabilities {
    pub(crate) fn capture(window: &Window) -> Self {
        #[cfg(target_os = "linux")]
        use WinitWindowPolicySupport::CreationOnly;
        use WinitWindowPolicySupport::{Dynamic, Unsupported};
        let desktop = cfg!(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        ));
        #[cfg(target_os = "linux")]
        let x11 = {
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            window.window_handle().is_ok_and(|handle| {
                matches!(
                    handle.as_raw(),
                    RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_)
                )
            })
        };
        #[cfg(not(target_os = "linux"))]
        let _ = window;
        #[cfg(target_os = "linux")]
        let positioning = x11;
        #[cfg(not(target_os = "linux"))]
        let positioning = cfg!(any(target_os = "windows", target_os = "macos"));
        Self {
            secondary_creation: cfg!(feature = "multi-viewport") && positioning,
            icon: if cfg!(target_os = "windows") {
                Dynamic
            } else {
                #[cfg(target_os = "linux")]
                if x11 {
                    Dynamic
                } else {
                    Unsupported
                }
                #[cfg(not(target_os = "linux"))]
                Unsupported
            },
            client_size_limits: if desktop { Dynamic } else { Unsupported },
            theme: if desktop { Dynamic } else { Unsupported },
            resizable: if desktop { Dynamic } else { Unsupported },
            transparency: {
                #[cfg(target_os = "linux")]
                {
                    if x11 { CreationOnly } else { Dynamic }
                }
                #[cfg(not(target_os = "linux"))]
                {
                    if desktop { Dynamic } else { Unsupported }
                }
            },
            parent_owner: if cfg!(all(feature = "multi-viewport", target_os = "windows")) {
                Dynamic
            } else {
                Unsupported
            },
            desktop_position: if positioning { Dynamic } else { Unsupported },
        }
    }
}

/// Native observations. `None` means Winit cannot establish that fact on this platform.
///
/// These fields are observations rather than requested creation settings. In particular, native
/// transparency and icon contents cannot be queried through Winit and are never inferred here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WinitWindowNativeState {
    pub visible: Option<bool>,
    pub minimized: Option<bool>,
    pub maximized: Option<bool>,
    pub focused: Option<bool>,
    pub resizable: Option<bool>,
    pub decorations: Option<bool>,
    pub theme: Option<Theme>,
    pub transparent: Option<bool>,
}

impl WinitWindowNativeState {
    pub(crate) fn capture(window: &Window) -> Self {
        let desktop = cfg!(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        ));
        Self {
            visible: window.is_visible(),
            minimized: window.is_minimized(),
            maximized: desktop.then(|| window.is_maximized()),
            focused: desktop.then(|| window.has_focus()),
            resizable: cfg!(any(target_os = "windows", target_os = "macos"))
                .then(|| window.is_resizable()),
            decorations: desktop.then(|| window.is_decorated()),
            theme: window.theme(),
            transparent: None,
        }
    }
}

/// Exactly-once registration observations owned by a platform owner.
///
/// `Created` follows successful native publication. `Retired` ends registry access; renderer work
/// may still retain a native allocation lease. Neither variant retains a window or a Context.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WinitWindowLifecycleEvent {
    Created(WinitWindowSnapshot),
    Retired(NativeWindowTarget),
}

impl WinitPlatformControl {
    pub(crate) fn record_window_created(&self, snapshot: WinitWindowSnapshot) {
        self.ime_enabled_windows
            .borrow_mut()
            .remove(&snapshot.target.instance_id);
        self.binding
            .clear_ime_preedit_for_viewport(snapshot.target.viewport_id);
        self.window_lifecycle_events
            .borrow_mut()
            .push_back(WinitWindowLifecycleEvent::Created(snapshot));
    }

    pub(crate) fn record_window_retired(&self, target: NativeWindowTarget) {
        self.ime_enabled_windows
            .borrow_mut()
            .remove(&target.instance_id);
        self.binding
            .clear_ime_preedit_for_viewport(target.viewport_id);
        self.release_pointer_control(
            super::WinitPointerReleaseReason::Retired,
            Some(target.instance_id),
        );
        #[cfg(feature = "multi-viewport")]
        if target.is_main
            && let Some(state) = self.attached_window_render_state.borrow_mut().take()
        {
            state.retire();
        }
        self.window_lifecycle_events
            .borrow_mut()
            .push_back(WinitWindowLifecycleEvent::Retired(target));
    }

    pub(super) fn retire_main_window_registration(&self) {
        if let Some(mut target) = self.attached_window_target.take() {
            if self.binding.lifecycle() == dear_imgui_rs::ContextLifecycle::Alive {
                let viewport = unsafe { dear_imgui_rs::sys::igGetMainViewport() };
                if !viewport.is_null() {
                    target.viewport_id = dear_imgui_rs::Id::from(unsafe { (*viewport).ID });
                }
            }
            self.record_window_retired(target);
        }
    }
}

impl WinitPlatform {
    /// Drains owned lifecycle observations in order, including after explicit shutdown.
    ///
    /// This queue is independent of callback faults. Main-window viewport toggles and logical
    /// routing remaps do not create new registrations and therefore emit no additional events.
    pub fn drain_window_lifecycle_events(&self) -> Vec<WinitWindowLifecycleEvent> {
        self.control
            .window_lifecycle_events
            .borrow_mut()
            .drain(..)
            .collect()
    }
}

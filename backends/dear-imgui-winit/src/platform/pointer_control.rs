use std::sync::Arc;

use dear_imgui_rs::{NativeWindowInstanceId, NativeWindowTarget};
use winit::event::WindowEvent;
use winit::window::{CursorGrabMode, Window, WindowId};

use super::WinitPlatformError;
use super::ownership::{WinitPlatform, WinitPlatformControl};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WinitPointerGrabMode {
    Locked,
    Confined,
}

/// Native grab order. A fallback is attempted only when the preferred mode fails.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WinitPointerGrabPreference {
    Locked,
    Confined,
    #[default]
    LockedOrConfined,
    ConfinedOrLocked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WinitPointerRequest {
    pub grab: WinitPointerGrabPreference,
    pub hide_cursor: bool,
    /// Visibility restored when Dear ImGui cursor management was disabled at release time.
    /// Winit cannot query a host-managed cursor's previous native visibility.
    pub fallback_restore_visible: bool,
}

impl Default for WinitPointerRequest {
    fn default() -> Self {
        Self {
            grab: WinitPointerGrabPreference::LockedOrConfined,
            hide_cursor: true,
            fallback_restore_visible: true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WinitPointerGrabFailure {
    pub mode: WinitPointerGrabMode,
    pub message: String,
}

/// Complete native grab attempts, including failures preceding a successful fallback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WinitPointerCaptureReport {
    pub target: NativeWindowTarget,
    pub requested: WinitPointerGrabPreference,
    pub granted: Option<WinitPointerGrabMode>,
    pub failures: Vec<WinitPointerGrabFailure>,
    /// Best-effort reset failure after every requested grab mode failed.
    pub cleanup_error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WinitPointerReleaseReason {
    ExplicitCancel,
    Escape,
    FocusLost,
    CloseRequested,
    NativeDestroyed,
    Retired,
    PlatformShutdown,
}

/// Pointer ownership ended. An error means native ungrab could not be confirmed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WinitPointerReleaseReport {
    pub target: NativeWindowTarget,
    pub reason: WinitPointerReleaseReason,
    pub native_error: Option<String>,
}

pub(super) struct ActivePointerControl {
    target: NativeWindowTarget,
    window: Arc<Window>,
    fallback_restore_visible: bool,
}

fn attempt_grab(
    target: NativeWindowTarget,
    preference: WinitPointerGrabPreference,
    mut attempt: impl FnMut(WinitPointerGrabMode) -> Result<(), String>,
) -> WinitPointerCaptureReport {
    use WinitPointerGrabMode::{Confined, Locked};
    let (first, fallback) = match preference {
        WinitPointerGrabPreference::Locked => (Locked, None),
        WinitPointerGrabPreference::Confined => (Confined, None),
        WinitPointerGrabPreference::LockedOrConfined => (Locked, Some(Confined)),
        WinitPointerGrabPreference::ConfinedOrLocked => (Confined, Some(Locked)),
    };
    let mut report = WinitPointerCaptureReport {
        target,
        requested: preference,
        granted: None,
        failures: Vec::new(),
        cleanup_error: None,
    };
    for mode in std::iter::once(first).chain(fallback) {
        match attempt(mode) {
            Ok(()) => {
                report.granted = Some(mode);
                break;
            }
            Err(message) => report
                .failures
                .push(WinitPointerGrabFailure { mode, message }),
        }
    }
    report
}

pub(crate) fn release_reason(event: &WindowEvent) -> Option<WinitPointerReleaseReason> {
    match event {
        WindowEvent::Focused(false) => Some(WinitPointerReleaseReason::FocusLost),
        WindowEvent::CloseRequested => Some(WinitPointerReleaseReason::CloseRequested),
        WindowEvent::Destroyed => Some(WinitPointerReleaseReason::NativeDestroyed),
        WindowEvent::KeyboardInput { event, .. }
            if escape_pressed(&event.logical_key, event.state) =>
        {
            Some(WinitPointerReleaseReason::Escape)
        }
        _ => None,
    }
}

fn escape_pressed(key: &winit::keyboard::Key, state: winit::event::ElementState) -> bool {
    state.is_pressed() && key == &winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape)
}

impl WinitPlatformControl {
    pub(crate) fn pointer_controls_window(&self, window_id: WindowId) -> bool {
        self.pointer_control
            .borrow()
            .as_ref()
            .is_some_and(|active| active.window.id() == window_id)
    }

    pub(crate) fn release_pointer_control(
        &self,
        reason: WinitPointerReleaseReason,
        expected: Option<NativeWindowInstanceId>,
    ) -> Option<WinitPointerReleaseReport> {
        if expected.is_some_and(|expected| {
            self.pointer_control
                .borrow()
                .as_ref()
                .is_none_or(|active| active.target.instance_id != expected)
        }) {
            return None;
        }
        // End the Rust ownership scope before native callouts; restoration never borrows the
        // state slot while calling the host window system.
        let active = self.pointer_control.borrow_mut().take()?;
        let native_error = active
            .window
            .set_cursor_grab(CursorGrabMode::None)
            .err()
            .map(|error| error.to_string());
        if let Some(settings) = self.cursor_settings.get() {
            settings.apply(&active.window);
        } else {
            active
                .window
                .set_cursor_visible(active.fallback_restore_visible);
        }
        let report = WinitPointerReleaseReport {
            target: active.target,
            reason,
            native_error,
        };
        self.pointer_release_events
            .borrow_mut()
            .push_back(report.clone());
        Some(report)
    }

    fn pointer_window_lease(
        &self,
        instance_id: NativeWindowInstanceId,
    ) -> Result<Option<Arc<Window>>, WinitPlatformError> {
        self.binding.try_with_bound_context(|| {
            self.validate_complete_contract_in_current_context()?;
            #[cfg(feature = "multi-viewport")]
            {
                let runtime = self.runtime.borrow().clone();
                if let Some(runtime) = runtime {
                    return runtime.window_lease_in_current_context(instance_id);
                }
            }
            if self
                .attached_window_target
                .get()
                .map(|target| target.instance_id)
                != Some(instance_id)
            {
                return Ok(None);
            }
            Ok(self.attached_window.borrow().clone())
        })?
    }
}

impl WinitPlatform {
    /// Gives the engine control of one exact native window's pointer and cursor visibility.
    ///
    /// Raw `DeviceEvent` motion remains the host's responsibility. This operation does not change
    /// `NO_MOUSE_CURSOR_CHANGE`; backend cursor changes are suspended only for this registration.
    pub fn request_pointer_control(
        &mut self,
        instance_id: NativeWindowInstanceId,
        request: WinitPointerRequest,
    ) -> Result<WinitPointerCaptureReport, WinitPlatformError> {
        self.control.validate_operational_contract()?;
        if self.control.pointer_control.borrow().is_some() {
            return Err(WinitPlatformError::PointerControlAlreadyOwned);
        }
        let snapshot = self
            .window_snapshot_for_instance(instance_id)?
            .ok_or(WinitPlatformError::WindowRegistrationUnavailable { instance_id })?;
        let window = self
            .control
            .pointer_window_lease(instance_id)?
            .ok_or(WinitPlatformError::WindowRegistrationUnavailable { instance_id })?;
        let mut report = attempt_grab(snapshot.target, request.grab, |mode| {
            window
                .set_cursor_grab(match mode {
                    WinitPointerGrabMode::Locked => CursorGrabMode::Locked,
                    WinitPointerGrabMode::Confined => CursorGrabMode::Confined,
                })
                .map_err(|error| error.to_string())
        });
        if report.granted.is_some() {
            window.set_cursor_visible(!request.hide_cursor);
            self.control
                .pointer_control
                .replace(Some(ActivePointerControl {
                    target: snapshot.target,
                    window,
                    fallback_restore_visible: request.fallback_restore_visible,
                }));
        } else {
            report.cleanup_error = window
                .set_cursor_grab(CursorGrabMode::None)
                .err()
                .map(|error| error.to_string());
        }
        Ok(report)
    }

    /// Ends engine pointer control and restores the most recent Dear ImGui cursor policy.
    pub fn cancel_pointer_control(&mut self) -> Option<WinitPointerReleaseReport> {
        self.control
            .release_pointer_control(WinitPointerReleaseReason::ExplicitCancel, None)
    }

    /// Returns the registration and routing association captured when pointer control began.
    pub fn pointer_control_target(&self) -> Option<NativeWindowTarget> {
        self.control
            .pointer_control
            .borrow()
            .as_ref()
            .map(|active| active.target)
    }

    /// Drains automatic and explicit release reports independently of platform faults.
    pub fn drain_pointer_release_events(&self) -> Vec<WinitPointerReleaseReport> {
        self.control
            .pointer_release_events
            .borrow_mut()
            .drain(..)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::test_sync::lock_context;
    use dear_imgui_rs::{Context, Id};

    #[test]
    fn fallback_preserves_the_failed_preferred_grab() {
        let _guard = lock_context();
        let context = Context::create();
        let target = NativeWindowTarget {
            instance_id: NativeWindowInstanceId::allocate(context.id()).unwrap(),
            viewport_id: Id::from(7),
            is_main: true,
        };
        let report = attempt_grab(
            target,
            WinitPointerGrabPreference::LockedOrConfined,
            |mode| match mode {
                WinitPointerGrabMode::Locked => Err("lock unavailable".to_owned()),
                WinitPointerGrabMode::Confined => Ok(()),
            },
        );
        assert_eq!(report.granted, Some(WinitPointerGrabMode::Confined));
        assert_eq!(
            report.failures,
            vec![WinitPointerGrabFailure {
                mode: WinitPointerGrabMode::Locked,
                message: "lock unavailable".to_owned()
            }]
        );
        let failure = attempt_grab(target, WinitPointerGrabPreference::Locked, |_| {
            Err("denied".to_owned())
        });
        assert!(failure.granted.is_none());
        assert_eq!(failure.failures.len(), 1);
    }

    #[test]
    fn focus_close_and_native_destruction_end_pointer_ownership() {
        let escape = winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape);
        assert!(escape_pressed(&escape, winit::event::ElementState::Pressed));
        assert!(!escape_pressed(
            &escape,
            winit::event::ElementState::Released
        ));
        assert_eq!(
            release_reason(&WindowEvent::Focused(false)),
            Some(WinitPointerReleaseReason::FocusLost)
        );
        assert_eq!(
            release_reason(&WindowEvent::CloseRequested),
            Some(WinitPointerReleaseReason::CloseRequested)
        );
        assert_eq!(
            release_reason(&WindowEvent::Destroyed),
            Some(WinitPointerReleaseReason::NativeDestroyed)
        );
        assert_eq!(release_reason(&WindowEvent::Focused(true)), None);
    }
}

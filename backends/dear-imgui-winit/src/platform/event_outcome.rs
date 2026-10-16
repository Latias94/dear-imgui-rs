use dear_imgui_rs::{Context, NativeWindowTarget};
use winit::window::WindowId;

/// Input streams actually updated while handling one native event.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WinitEventForwarding {
    pub keyboard: bool,
    pub text: bool,
    pub pointer: bool,
    pub focus: bool,
}

/// Dear ImGui's capture intent at event-dispatch time, from the most recent UI frame.
///
/// These are routing hints, not a consumption verdict for this event. Engines should preserve the
/// recipient selected for a press until its matching release, even if capture intent changes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WinitCaptureIntent {
    pub keyboard: bool,
    pub pointer: bool,
    pub text_input: bool,
}

impl WinitCaptureIntent {
    pub(crate) fn snapshot(context: &Context) -> Self {
        let io = context.io();
        Self {
            keyboard: io.want_capture_keyboard(),
            pointer: io.want_capture_mouse(),
            text_input: io.want_text_input(),
        }
    }
}

/// Exact native registration selected by WindowId-first event routing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WinitEventTarget {
    pub window_id: WindowId,
    pub native: NativeWindowTarget,
}

/// Native platform action distinguished from Dear ImGui input forwarding.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WinitPlatformAction {
    #[default]
    None,
    /// The host should decide whether to close its main window/application.
    MainCloseRequested,
    /// The backend set this secondary viewport's PlatformRequestClose flag.
    ViewportCloseRequested,
    GeometryChanged,
    NativeDestroyed,
}

/// Facts observed while routing one event, without conflating ownership and capture intent.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WinitEventOutcome {
    /// `None` for a foreign WindowId or a non-window event.
    pub target: Option<WinitEventTarget>,
    pub forwarded: WinitEventForwarding,
    /// `None` when the event did not belong to this platform.
    pub capture: Option<WinitCaptureIntent>,
    /// Native input/state changed and a new UI frame was requested.
    pub needs_frame: bool,
    pub platform_action: WinitPlatformAction,
    pub pointer_release: Option<super::WinitPointerReleaseReport>,
}

impl WinitEventOutcome {
    /// Returns whether this event was associated with a currently owned native registration.
    pub fn is_owned(&self) -> bool {
        self.target.is_some()
    }
}

use bevy_ecs::prelude::{Entity, Res, Resource};
use dear_imgui_rs as imgui;

use crate::ContextId;

use super::capture::CaptureScopes;

/// Dear ImGui capture intent for one scope.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ImguiInputCaptureState {
    /// Dear ImGui wants mouse input.
    pub want_capture_mouse: bool,
    /// Dear ImGui wants mouse input, except when a popup close should be allowed through.
    pub want_capture_mouse_unless_popup_close: bool,
    /// Dear ImGui wants keyboard input.
    pub want_capture_keyboard: bool,
    /// Dear ImGui wants text input / IME.
    pub want_text_input: bool,
}

impl ImguiInputCaptureState {
    /// Whether Dear ImGui wants pointer input.
    #[must_use]
    pub const fn wants_pointer_input(self) -> bool {
        self.want_capture_mouse
    }

    /// Whether Dear ImGui wants pointer input after allowing popup-close clicks through.
    #[must_use]
    pub const fn wants_pointer_input_unless_popup_close(self) -> bool {
        self.want_capture_mouse_unless_popup_close
    }

    /// Whether Dear ImGui wants keyboard input.
    #[must_use]
    pub const fn wants_keyboard_input(self) -> bool {
        self.want_capture_keyboard
    }

    /// Whether Dear ImGui wants text input / IME.
    #[must_use]
    pub const fn wants_text_input(self) -> bool {
        self.want_text_input
    }

    /// Whether Dear ImGui wants any pointer, keyboard, or text input.
    #[must_use]
    pub const fn wants_any_input(self) -> bool {
        self.wants_pointer_input() || self.wants_keyboard_input() || self.wants_text_input()
    }

    pub(super) fn from_io(io: &imgui::Io) -> Self {
        Self {
            want_capture_mouse: io.want_capture_mouse(),
            want_capture_mouse_unless_popup_close: io.want_capture_mouse_unless_popup_close(),
            want_capture_keyboard: io.want_capture_keyboard(),
            want_text_input: io.want_text_input(),
        }
    }

    #[cfg(feature = "render")]
    pub(super) fn merge(self, other: Self) -> Self {
        Self {
            want_capture_mouse: self.want_capture_mouse || other.want_capture_mouse,
            want_capture_mouse_unless_popup_close: self.want_capture_mouse_unless_popup_close
                || other.want_capture_mouse_unless_popup_close,
            want_capture_keyboard: self.want_capture_keyboard || other.want_capture_keyboard,
            want_text_input: self.want_text_input || other.want_text_input,
        }
    }
}

/// Last-known Dear ImGui capture intent exposed as a Bevy resource.
///
/// The backend samples these flags after mapping one input batch and before calling `NewFrame`, as
/// Dear ImGui recommends for event dispatch. Game/editor systems in `Update` therefore receive one
/// stable decision for that batch; the backend itself does not remove or stop Bevy messages.
///
/// The unscoped queries are the aggregate across routed Contexts. Use the scoped helpers when a
/// game system belongs to one Context or host window. The backend owns this resource's state;
/// consumers receive a copy through [`Self::aggregate`] when they need a stable snapshot.
#[derive(Resource, Debug, Clone, Default, Eq, PartialEq)]
pub struct ImguiInputCapture {
    aggregate: ImguiInputCaptureState,
    scopes: CaptureScopes,
}

impl ImguiInputCapture {
    /// Whether Dear ImGui wants pointer input.
    #[must_use]
    pub fn wants_pointer_input(&self) -> bool {
        self.aggregate.want_capture_mouse
    }

    /// Whether Dear ImGui wants pointer input after allowing popup-close clicks through.
    #[must_use]
    pub fn wants_pointer_input_unless_popup_close(&self) -> bool {
        self.aggregate.want_capture_mouse_unless_popup_close
    }

    /// Whether Dear ImGui wants keyboard input.
    #[must_use]
    pub fn wants_keyboard_input(&self) -> bool {
        self.aggregate.want_capture_keyboard
    }

    /// Whether Dear ImGui wants text input / IME.
    #[must_use]
    pub fn wants_text_input(&self) -> bool {
        self.aggregate.want_text_input
    }

    /// Whether Dear ImGui wants any pointer, keyboard, or text input.
    #[must_use]
    pub fn wants_any_input(&self) -> bool {
        self.wants_pointer_input() || self.wants_keyboard_input() || self.wants_text_input()
    }

    /// Return capture state for `context_id`, or an empty state when it has no active input route.
    #[must_use]
    pub fn context(&self, context_id: ContextId) -> ImguiInputCaptureState {
        self.scopes.context(context_id)
    }

    /// Return capture state for `context_id` when it has an active input route.
    #[must_use]
    pub fn for_context(&self, context_id: ContextId) -> Option<ImguiInputCaptureState> {
        self.scopes.for_context(context_id)
    }

    /// Return capture state aggregated for `window`, or an empty state when no Context is routed.
    #[must_use]
    pub fn window(&self, window: Entity) -> ImguiInputCaptureState {
        self.scopes.window(window)
    }

    /// Return capture state for `window` when one or more Contexts are routed to it.
    #[must_use]
    pub fn for_window(&self, window: Entity) -> Option<ImguiInputCaptureState> {
        self.scopes.for_window(window)
    }

    /// Return capture state for the primary Dear ImGui Context.
    #[must_use]
    pub fn primary(&self) -> ImguiInputCaptureState {
        self.scopes
            .primary_context()
            .map_or_else(ImguiInputCaptureState::default, |context_id| {
                self.context(context_id)
            })
    }

    /// Return the aggregate capture state across all routed Contexts.
    #[must_use]
    pub const fn aggregate(&self) -> ImguiInputCaptureState {
        self.aggregate
    }

    /// Whether one Context wants pointer input.
    #[must_use]
    pub fn wants_pointer_input_for_context(&self, context_id: ContextId) -> bool {
        self.context(context_id).wants_pointer_input()
    }

    /// Whether one window's routed Contexts want pointer input.
    #[must_use]
    pub fn wants_pointer_input_for_window(&self, window: Entity) -> bool {
        self.window(window).wants_pointer_input()
    }

    /// Whether the primary Context wants pointer input.
    #[must_use]
    pub fn primary_wants_pointer_input(&self) -> bool {
        self.primary().wants_pointer_input()
    }

    /// Whether one Context wants keyboard input.
    #[must_use]
    pub fn wants_keyboard_input_for_context(&self, context_id: ContextId) -> bool {
        self.context(context_id).wants_keyboard_input()
    }

    /// Whether one window's routed Contexts want keyboard input.
    #[must_use]
    pub fn wants_keyboard_input_for_window(&self, window: Entity) -> bool {
        self.window(window).wants_keyboard_input()
    }

    /// Whether the primary Context wants keyboard input.
    #[must_use]
    pub fn primary_wants_keyboard_input(&self) -> bool {
        self.primary().wants_keyboard_input()
    }

    /// Whether one Context wants text input / IME.
    #[must_use]
    pub fn wants_text_input_for_context(&self, context_id: ContextId) -> bool {
        self.context(context_id).wants_text_input()
    }

    /// Whether one window's routed Contexts want text input / IME.
    #[must_use]
    pub fn wants_text_input_for_window(&self, window: Entity) -> bool {
        self.window(window).wants_text_input()
    }

    /// Whether the primary Context wants text input / IME.
    #[must_use]
    pub fn primary_wants_text_input(&self) -> bool {
        self.primary().wants_text_input()
    }

    #[cfg(not(feature = "render"))]
    pub(super) fn update_from_io(&mut self, context_id: ContextId, window: Entity, io: &imgui::Io) {
        let state = ImguiInputCaptureState::from_io(io);
        self.scopes.update_primary(context_id, window, state);
        self.set_aggregate(state);
    }

    pub(super) fn set_aggregate(&mut self, state: ImguiInputCaptureState) {
        self.aggregate = state;
    }

    #[cfg(feature = "render")]
    pub(super) fn begin_routes(
        &mut self,
        primary_context: Option<ContextId>,
        routes: &[(ContextId, Entity)],
    ) {
        self.scopes.begin_routes(primary_context, routes);
    }

    #[cfg(feature = "render")]
    pub(super) fn update_context(&mut self, context_id: ContextId, io: &imgui::Io) {
        self.scopes.update_context(context_id, io);
    }

    #[cfg(feature = "render")]
    pub(super) fn remove_context(&mut self, context_id: ContextId) {
        self.scopes.remove_context(context_id);
    }

    #[cfg(feature = "render")]
    pub(super) fn finish_routes(&mut self) {
        let aggregate = self.scopes.finish_routes();
        self.set_aggregate(aggregate);
    }
}

/// Run condition that returns true while Dear ImGui wants pointer input.
pub fn imgui_wants_pointer_input(capture: Res<ImguiInputCapture>) -> bool {
    capture.wants_pointer_input()
}

/// Run condition that returns true while Dear ImGui wants pointer input, excluding popup-close clicks.
pub fn imgui_wants_pointer_input_unless_popup_close(capture: Res<ImguiInputCapture>) -> bool {
    capture.wants_pointer_input_unless_popup_close()
}

/// Run condition that returns true while Dear ImGui wants keyboard input.
pub fn imgui_wants_keyboard_input(capture: Res<ImguiInputCapture>) -> bool {
    capture.wants_keyboard_input()
}

/// Run condition that returns true while Dear ImGui wants text input or IME.
pub fn imgui_wants_text_input(capture: Res<ImguiInputCapture>) -> bool {
    capture.wants_text_input()
}

/// Run condition that returns true while Dear ImGui wants any pointer, keyboard, or text input.
pub fn imgui_wants_any_input(capture: Res<ImguiInputCapture>) -> bool {
    capture.wants_any_input()
}

/// Build a run condition for pointer capture by one Context.
pub fn imgui_context_wants_pointer_input(
    context_id: ContextId,
) -> impl FnMut(Res<ImguiInputCapture>) -> bool + Clone {
    move |capture| capture.wants_pointer_input_for_context(context_id)
}

/// Build a run condition for pointer capture in one host window.
pub fn imgui_window_wants_pointer_input(
    window: Entity,
) -> impl FnMut(Res<ImguiInputCapture>) -> bool + Clone {
    move |capture| capture.wants_pointer_input_for_window(window)
}

/// Build a run condition for keyboard capture by one Context.
pub fn imgui_context_wants_keyboard_input(
    context_id: ContextId,
) -> impl FnMut(Res<ImguiInputCapture>) -> bool + Clone {
    move |capture| capture.wants_keyboard_input_for_context(context_id)
}

/// Build a run condition for keyboard capture in one host window.
pub fn imgui_window_wants_keyboard_input(
    window: Entity,
) -> impl FnMut(Res<ImguiInputCapture>) -> bool + Clone {
    move |capture| capture.wants_keyboard_input_for_window(window)
}

/// Build a run condition for text/IME capture by one Context.
pub fn imgui_context_wants_text_input(
    context_id: ContextId,
) -> impl FnMut(Res<ImguiInputCapture>) -> bool + Clone {
    move |capture| capture.wants_text_input_for_context(context_id)
}

/// Build a run condition for text/IME capture in one host window.
pub fn imgui_window_wants_text_input(
    window: Entity,
) -> impl FnMut(Res<ImguiInputCapture>) -> bool + Clone {
    move |capture| capture.wants_text_input_for_window(window)
}

/// Run condition that returns true while the primary Context wants keyboard input.
pub fn imgui_primary_wants_keyboard_input(capture: Res<ImguiInputCapture>) -> bool {
    capture.primary_wants_keyboard_input()
}

/// Run condition that returns true while the primary Context wants pointer input.
pub fn imgui_primary_wants_pointer_input(capture: Res<ImguiInputCapture>) -> bool {
    capture.primary_wants_pointer_input()
}

/// Run condition that returns true while the primary Context wants text input / IME.
pub fn imgui_primary_wants_text_input(capture: Res<ImguiInputCapture>) -> bool {
    capture.primary_wants_text_input()
}

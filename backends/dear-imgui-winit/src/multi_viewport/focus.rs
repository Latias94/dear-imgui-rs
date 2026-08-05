use std::collections::HashSet;
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

use winit::window::WindowId;

/// Bound the optimistic state independently of rendering rate. A native focus event should arrive
/// on the next event-loop turn; this leaves room for a mapped-window round trip under load.
const PLATFORM_FOCUS_CONFIRMATION_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingPlatformFocus {
    window_id: WindowId,
    deadline: Instant,
    retry_pending: bool,
}

/// Tracks the gap between a Winit focus request and its later `WindowEvent::Focused` confirmation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct PlatformFocusState {
    pending: Option<PendingPlatformFocus>,
}

impl PlatformFocusState {
    pub(super) fn request(&mut self, window_id: WindowId, now: Instant) {
        self.pending = Some(PendingPlatformFocus {
            window_id,
            deadline: now + PLATFORM_FOCUS_CONFIRMATION_TIMEOUT,
            retry_pending: true,
        });
    }

    pub(super) fn cancel(&mut self, window_id: WindowId) {
        if self
            .pending
            .is_some_and(|pending| pending.window_id == window_id)
        {
            self.pending = None;
        }
    }

    /// A positive native event either confirms the requested target or proves that another window
    /// won. Loss events may precede the matching gain during a transfer, so expiry handles them.
    pub(super) fn note_native_event(&mut self, focused: bool) {
        if focused {
            self.pending = None;
        }
    }

    /// Retain a request until its deadline and return the one-shot retry target.
    pub(super) fn advance(
        &mut self,
        now: Instant,
        owned_windows: &HashSet<WindowId>,
    ) -> Option<WindowId> {
        let mut pending = self.pending?;
        if now >= pending.deadline || !owned_windows.contains(&pending.window_id) {
            self.pending = None;
            return None;
        }

        let retry = pending.retry_pending.then_some(pending.window_id);
        pending.retry_pending = false;
        self.pending = Some(pending);
        retry
    }

    pub(super) fn has_pending_for_owned_window(
        &self,
        now: Instant,
        owned_windows: &HashSet<WindowId>,
    ) -> bool {
        self.pending.is_some_and(|pending| {
            now < pending.deadline && owned_windows.contains(&pending.window_id)
        })
    }

    /// Return the pending target immediately while the native focus cache is still stale.
    pub(super) fn effective_focus(
        self,
        now: Instant,
        window_id: WindowId,
        native_focused: bool,
    ) -> bool {
        self.pending
            .filter(|pending| now < pending.deadline)
            .map_or(native_focused, |pending| pending.window_id == window_id)
    }
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct ContextFocusState {
    focused_windows: HashSet<WindowId>,
    context_focused: bool,
    focus_loss_pending: bool,
}

impl ContextFocusState {
    pub(super) fn with_focused_window(window_id: Option<WindowId>) -> Self {
        // Dear ImGui treats a newly attached platform as focused until it receives an explicit
        // loss event. Start from that reported state even when Winit says the main window is
        // already unfocused, then reconcile the empty set at the next platform-frame boundary.
        let mut state = Self {
            context_focused: true,
            ..Self::default()
        };
        if let Some(window_id) = window_id {
            state.focused_windows.insert(window_id);
        }
        state
    }

    /// Records a native focus event and returns whether Dear ImGui needs a focus-gained event.
    pub(super) fn note_window_focus(&mut self, window_id: WindowId, focused: bool) -> bool {
        if focused {
            self.focused_windows.insert(window_id);
            self.focus_loss_pending = false;
            if !self.context_focused {
                self.context_focused = true;
                return true;
            }
        } else if self.focused_windows.remove(&window_id)
            && self.focused_windows.is_empty()
            && self.context_focused
        {
            // Focus transfers between native viewports commonly report the old window losing
            // focus before the new one gains it. Defer the Context-level loss until the next
            // platform-frame boundary so that transfer can cancel it.
            self.focus_loss_pending = true;
        }
        false
    }

    /// Reconciles destroyed windows and returns whether the Context has now lost focus.
    pub(super) fn reconcile_owned_windows(
        &mut self,
        owned_windows: &HashSet<WindowId>,
        platform_focus_pending: bool,
    ) -> bool {
        self.focused_windows
            .retain(|window_id| owned_windows.contains(window_id));
        if self.context_focused && self.focused_windows.is_empty() && !platform_focus_pending {
            self.focus_loss_pending = true;
        }
        if self.focus_loss_pending && self.focused_windows.is_empty() && !platform_focus_pending {
            self.focus_loss_pending = false;
            self.context_focused = false;
            true
        } else {
            false
        }
    }
}

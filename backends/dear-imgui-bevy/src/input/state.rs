#[cfg(not(feature = "render"))]
use std::collections::HashSet;

use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
#[cfg(not(feature = "render"))]
use bevy_window::WindowPosition;
use dear_imgui_rs as imgui;

use crate::ContextId;

#[cfg(feature = "render")]
use super::route::RoutedInputState;
#[cfg(all(test, feature = "render"))]
use super::route::{ImguiInputSlot, ImguiRoutedWindowState};

/// Runtime state needed to map Bevy input streams into Dear ImGui events.
#[derive(Resource, Debug, Default)]
pub(crate) struct ImguiInputState {
    #[cfg(not(feature = "render"))]
    pub(super) active_touch_id: Option<u64>,
    #[cfg(not(feature = "render"))]
    pub(super) active_touch_window: Option<Entity>,
    #[cfg(not(feature = "render"))]
    pub(super) ime_enabled: bool,
    #[cfg(not(feature = "render"))]
    pub(super) primary_window_focused: Option<bool>,
    #[cfg(not(feature = "render"))]
    pub(super) focused_window: Option<Entity>,
    #[cfg(not(feature = "render"))]
    pub(super) mouse_hovered_window: Option<Entity>,
    #[cfg(not(feature = "render"))]
    pub(super) pressed_keys: HashSet<imgui::Key>,
    #[cfg(not(feature = "render"))]
    pub(super) pressed_mouse_buttons: HashSet<imgui::MouseButton>,
    #[cfg(feature = "render")]
    pub(super) routed: RoutedInputState,
}

impl ImguiInputState {
    /// Currently selected touch id for touch-to-mouse translation.
    #[cfg(test)]
    #[must_use]
    pub fn active_touch_id(&self) -> Option<u64> {
        #[cfg(feature = "render")]
        {
            let primary_context = self.routed.primary_context?;
            self.routed.windows.iter().find_map(|(slot, state)| {
                (slot.context_id == primary_context)
                    .then_some(state.active_touch_id)
                    .flatten()
            })
        }
        #[cfg(not(feature = "render"))]
        {
            self.active_touch_id
        }
    }

    /// Whether the last mapped-window IME message left IME enabled.
    #[cfg(test)]
    #[must_use]
    pub fn ime_enabled(&self) -> bool {
        #[cfg(feature = "render")]
        {
            self.routed.primary_context.is_some_and(|primary_context| {
                self.routed
                    .windows
                    .iter()
                    .any(|(slot, state)| slot.context_id == primary_context && state.ime_enabled)
            })
        }
        #[cfg(not(feature = "render"))]
        {
            self.ime_enabled
        }
    }

    /// Last focus state observed for the primary window.
    #[cfg(test)]
    #[must_use]
    pub fn primary_window_focused(&self) -> Option<bool> {
        #[cfg(feature = "render")]
        {
            let primary_context = self.routed.primary_context?;
            Some(self.routed.primary_window.is_some_and(|window| {
                self.routed
                    .windows
                    .get(&ImguiInputSlot {
                        context_id: primary_context,
                        window,
                    })
                    .is_some_and(|state| state.focused)
            }))
        }
        #[cfg(not(feature = "render"))]
        {
            self.primary_window_focused
        }
    }

    /// Last Bevy window entity reported as focused by the backend.
    #[cfg(test)]
    #[must_use]
    pub fn focused_window(&self) -> Option<Entity> {
        #[cfg(feature = "render")]
        {
            self.routed
                .primary_context
                .and_then(|context_id| self.routed.last_focused.get(&context_id).copied())
        }
        #[cfg(not(feature = "render"))]
        {
            self.focused_window
        }
    }

    /// Last Bevy window entity reported as hovered by the OS mouse.
    #[cfg(any(test, not(feature = "render")))]
    #[must_use]
    pub(crate) fn mouse_hovered_window(&self) -> Option<Entity> {
        #[cfg(feature = "render")]
        {
            self.routed
                .primary_context
                .and_then(|context_id| self.routed.last_hovered.get(&context_id).copied())
        }
        #[cfg(not(feature = "render"))]
        {
            self.mouse_hovered_window
        }
    }

    /// Return the state owned by one Context and host window.
    ///
    /// The aggregate test accessors above project the primary Context from the same routed state.
    /// Multi-Context assertions should use this scoped query instead.
    #[cfg(all(test, feature = "render"))]
    #[must_use]
    pub fn for_context_window(
        &self,
        context_id: ContextId,
        window: Entity,
    ) -> Option<ImguiInputWindowState> {
        self.routed
            .windows
            .get(&ImguiInputSlot { context_id, window })
            .map(ImguiRoutedWindowState::snapshot)
    }

    #[cfg(feature = "render")]
    pub(crate) fn context_window_focus_states(&self, context_id: ContextId) -> Vec<(Entity, bool)> {
        self.routed
            .windows
            .iter()
            .filter_map(|(slot, state)| {
                (slot.context_id == context_id).then_some((slot.window, state.focused))
            })
            .collect()
    }

    #[cfg(feature = "render")]
    pub(crate) fn platform_cursor_window_for_context(
        &self,
        context_id: ContextId,
        default_window: Option<Entity>,
    ) -> Option<Entity> {
        if let Some(window) = self.routed.last_hovered.get(&context_id).copied() {
            return self
                .routed
                .pointer_targets
                .get(&window)
                .and_then(|contexts| contexts.first())
                .is_some_and(|owner| *owner == context_id)
                .then_some(window);
        }

        let window = default_window?;
        match self.routed.pointer_targets.get(&window) {
            None => Some(window),
            Some(contexts) => contexts
                .first()
                .is_some_and(|owner| *owner == context_id)
                .then_some(window),
        }
    }
}

/// Read-only scoped input state for one Dear ImGui Context and host window.
#[cfg(all(test, feature = "render"))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ImguiInputWindowState {
    /// Currently selected touch id for touch-to-mouse translation.
    pub active_touch_id: Option<u64>,
    /// Whether the platform IME is enabled for this Context/window pair.
    pub ime_enabled: bool,
    /// Whether this Context owns keyboard focus in the host window.
    pub focused: bool,
    /// Whether the pointer currently hovers this Context's input region.
    pub mouse_hovered: bool,
}

#[derive(Clone, Copy)]
pub(super) struct ImguiInputWindow {
    #[cfg(any(
        not(feature = "render"),
        all(feature = "multi-viewport", not(target_arch = "wasm32"))
    ))]
    pub(super) entity: Entity,
    #[cfg(not(feature = "render"))]
    pub(super) position: WindowPosition,
    #[cfg(any(
        not(feature = "render"),
        all(feature = "multi-viewport", not(target_arch = "wasm32"))
    ))]
    pub(super) scale_factor: f32,
    pub(super) viewport_id: imgui::Id,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(super) desktop_origin: Option<[f32; 2]>,
    #[cfg(not(feature = "render"))]
    pub(super) context_id: ContextId,
    #[cfg(not(feature = "render"))]
    pub(super) is_primary: bool,
}

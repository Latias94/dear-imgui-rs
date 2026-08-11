//! Route-local state, target mapping, and frame metrics for Context input.

use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::{Entity, Resource};
use bevy_math::{Rect, Vec2};
use dear_imgui_rs::{self as imgui, ContextId};

use crate::route::{ImguiInputPolicy, ImguiRenderRouteEpoch};

#[cfg(test)]
use super::ImguiInputWindowState;
use super::{INVALID_MOUSE_POS, ImguiInputWindow};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ImguiInputFrameMetrics {
    pub(crate) host_window: Entity,
    pub(crate) display_size: [f32; 2],
    pub(crate) framebuffer_scale: [f32; 2],
}

/// A complete input transaction prepared for one Dear ImGui driver run.
///
/// The transaction intentionally has no `Default` or `Clone` implementation.  A producer must
/// publish the route snapshot, Context metrics, and platform-feedback authority together, and the
/// driver consumes that publication exactly once. This prevents a route epoch from being paired
/// with metrics or cursor/IME permissions from a different schedule run and prevents a missed
/// input schedule from replaying the previous frame.
#[derive(Debug)]
pub(crate) struct ImguiFrameInput {
    routes: ImguiRenderRouteEpoch,
    contexts: HashMap<ContextId, ImguiInputFrameMetrics>,
    platform_feedback_hosts: HashMap<ContextId, Entity>,
}

impl ImguiFrameInput {
    pub(crate) fn new(
        routes: ImguiRenderRouteEpoch,
        contexts: HashMap<ContextId, ImguiInputFrameMetrics>,
        platform_feedback_hosts: HashMap<ContextId, Entity>,
    ) -> Self {
        Self {
            routes,
            contexts,
            platform_feedback_hosts,
        }
    }

    pub(crate) const fn render_routes(&self) -> &ImguiRenderRouteEpoch {
        &self.routes
    }

    pub(crate) fn get(&self, context_id: ContextId) -> Option<ImguiInputFrameMetrics> {
        self.contexts.get(&context_id).copied()
    }

    pub(crate) fn platform_feedback_host(&self, context_id: ContextId) -> Option<Entity> {
        self.platform_feedback_hosts.get(&context_id).copied()
    }
}

#[derive(Debug, Default, Resource)]
pub(crate) struct ImguiFrameInputSlot {
    ready: Option<ImguiFrameInput>,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    latest_hosts: HashMap<ContextId, Entity>,
}

impl ImguiFrameInputSlot {
    pub(crate) fn publish(&mut self, input: ImguiFrameInput) {
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        {
            self.latest_hosts.clear();
            self.latest_hosts.extend(
                input
                    .contexts
                    .iter()
                    .map(|(context_id, metrics)| (*context_id, metrics.host_window)),
            );
        }
        self.ready = Some(input);
    }

    pub(crate) fn take(&mut self) -> Option<ImguiFrameInput> {
        self.ready.take()
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(crate) fn latest_host(&self, context_id: ContextId) -> Option<Entity> {
        self.latest_hosts.get(&context_id).copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct ImguiInputSlot {
    pub(super) context_id: ContextId,
    pub(super) window: Entity,
}

#[derive(Debug, Default)]
pub(super) struct ImguiRoutedWindowState {
    pub(super) active_touch_id: Option<u64>,
    pub(super) ime_enabled: bool,
    pub(super) focused: bool,
    pub(super) mouse_hovered: bool,
    pub(super) pressed_keys: HashSet<imgui::Key>,
    pub(super) pressed_mouse_buttons: HashSet<imgui::MouseButton>,
}

impl ImguiRoutedWindowState {
    #[cfg(test)]
    pub(super) fn snapshot(&self) -> ImguiInputWindowState {
        ImguiInputWindowState {
            active_touch_id: self.active_touch_id,
            ime_enabled: self.ime_enabled,
            focused: self.focused,
            mouse_hovered: self.mouse_hovered,
        }
    }

    pub(super) fn modifiers(&self) -> (bool, bool, bool, bool) {
        super::modifier_state(&self.pressed_keys)
    }
}

#[derive(Debug, Default)]
pub(super) struct RoutedInputState {
    pub(super) windows: HashMap<ImguiInputSlot, ImguiRoutedWindowState>,
    pub(super) pointer_targets: HashMap<Entity, Vec<ContextId>>,
    pub(super) pointer_positions: HashMap<Entity, Vec2>,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(super) raw_window_scale_factors: HashMap<Entity, f32>,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    pub(super) raw_native_pointer_positions: HashMap<Entity, Vec2>,
    pub(super) pointer_outside_windows: HashSet<Entity>,
    pub(super) focused_targets: HashMap<Entity, Vec<ContextId>>,
    pub(super) primary_context: Option<ContextId>,
    pub(super) primary_window: Option<Entity>,
    pub(super) last_focused: HashMap<ContextId, Entity>,
    pub(super) last_hovered: HashMap<ContextId, Entity>,
}

#[derive(Clone, Copy)]
pub(super) struct RoutedInputTarget {
    pub(super) context_id: ContextId,
    pub(super) host_window: Entity,
    pub(super) logical_region: Rect,
    pub(super) policy: ImguiInputPolicy,
    pub(super) display_size: [f32; 2],
    pub(super) framebuffer_scale: [f32; 2],
    pub(super) tracks_host_metrics: bool,
    pub(super) native_viewport: Option<ImguiInputWindow>,
}

impl RoutedInputTarget {
    pub(super) const fn slot(self) -> ImguiInputSlot {
        ImguiInputSlot {
            context_id: self.context_id,
            window: self.host_window,
        }
    }

    pub(super) const fn is_native_viewport(self) -> bool {
        self.native_viewport.is_some()
    }

    pub(super) fn contains(self, position: Vec2) -> bool {
        self.native_viewport.is_some()
            || (position.x >= self.logical_region.min.x
                && position.x < self.logical_region.max.x
                && position.y >= self.logical_region.min.y
                && position.y < self.logical_region.max.y)
    }

    pub(super) fn map_position(self, context: &imgui::Context, position: Vec2) -> [f32; 2] {
        if let Some(window) = self.native_viewport {
            return super::mouse_pos_for_window(context, window, position);
        }

        let size = self.logical_region.max - self.logical_region.min;
        if !size.x.is_finite() || !size.y.is_finite() || size.x <= 0.0 || size.y <= 0.0 {
            return INVALID_MOUSE_POS;
        }
        let normalized = (position - self.logical_region.min) / size;
        [
            normalized.x * self.display_size[0],
            normalized.y * self.display_size[1],
        ]
    }

    pub(super) fn viewport_id(self, context: &mut imgui::Context) -> imgui::Id {
        self.native_viewport
            .map_or_else(|| context.main_viewport().id(), |window| window.viewport_id)
    }
}

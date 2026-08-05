//! Routed window input mapping for the Bevy backend.
//!
//! This module maps Bevy windows and explicit logical input routes into their owning Dear ImGui
//! Contexts. It translates Bevy's window/input messages into Dear ImGui IO events without consuming
//! or rewriting Bevy's messages. Gameplay systems should use Dear ImGui's capture flags as policy
//! hints instead of expecting this backend to stop Bevy input propagation.

mod capture;
mod events;
mod feedback;
#[cfg(feature = "render")]
mod route;

use capture::CaptureScopes;
pub(crate) use events::ImguiInputMessageReaders;
use events::discard_all_unread_messages;
pub(crate) use feedback::map_imgui_mouse_cursor;
#[cfg(feature = "render")]
pub(crate) use route::{ImguiContextInputMetrics, ImguiInputFrameMetrics};
#[cfg(feature = "render")]
use route::{ImguiInputSlot, ImguiRoutedWindowState, RoutedInputState, RoutedInputTarget};

#[cfg(feature = "render")]
use crate::route::{ImguiInputPolicy, ImguiResolvedInputRoute, ImguiResolvedRoutes};
use crate::viewport::ImguiViewportOwner;
use crate::{ContextId, ImguiContextError, ImguiContexts, ImguiViewportWindow};
use bevy_app::{App, PreUpdate};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy_input::ButtonState;
use bevy_input::keyboard::{KeyCode, KeyboardFocusLost, KeyboardInput};
use bevy_input::mouse::{
    MouseButton as BevyMouseButton, MouseButtonInput, MouseScrollUnit, MouseWheel,
};
use bevy_input::touch::{TouchInput, TouchPhase};
#[cfg(feature = "render")]
use bevy_math::Rect;
use bevy_math::Vec2;
#[cfg(not(feature = "render"))]
use bevy_window::WindowPosition;
use bevy_window::{
    CursorEntered, CursorLeft, CursorMoved, Ime, PrimaryWindow, Window,
    WindowBackendScaleFactorChanged, WindowFocused, WindowResized, WindowScaleFactorChanged,
};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_winit::{RawWinitWindowEvent, WINIT_WINDOWS};
use dear_imgui_rs as imgui;
#[cfg(feature = "render")]
use std::collections::HashMap;
use std::collections::HashSet;
#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
use std::collections::VecDeque;

const INVALID_MOUSE_POS: [f32; 2] = [-f32::MAX, -f32::MAX];

#[cfg(feature = "render")]
type RoutedInputWindowComponents = (
    Entity,
    &'static Window,
    Option<&'static PrimaryWindow>,
    Option<&'static ImguiViewportWindow>,
    Option<&'static ImguiViewportOwner>,
);

/// System set that injects Bevy window input into Dear ImGui IO.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImguiInputSystems;

/// Runtime state needed to map Bevy input streams into Dear ImGui events.
#[derive(Resource, Debug, Default)]
pub(crate) struct ImguiInputState {
    #[cfg(not(feature = "render"))]
    active_touch_id: Option<u64>,
    #[cfg(not(feature = "render"))]
    active_touch_window: Option<Entity>,
    #[cfg(not(feature = "render"))]
    ime_enabled: bool,
    #[cfg(not(feature = "render"))]
    primary_window_focused: Option<bool>,
    #[cfg(not(feature = "render"))]
    focused_window: Option<Entity>,
    #[cfg(not(feature = "render"))]
    mouse_hovered_window: Option<Entity>,
    #[cfg(not(feature = "render"))]
    pressed_keys: HashSet<imgui::Key>,
    #[cfg(not(feature = "render"))]
    pressed_mouse_buttons: HashSet<imgui::MouseButton>,
    #[cfg(feature = "render")]
    routed: RoutedInputState,
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
    fn update_from_io(&mut self, context_id: ContextId, window: Entity, io: &imgui::Io) {
        let state = ImguiInputCaptureState::from_io(io);
        self.scopes.update_primary(context_id, window, state);
        self.set_aggregate(state);
    }

    fn set_aggregate(&mut self, state: ImguiInputCaptureState) {
        self.aggregate = state;
    }

    #[cfg(feature = "render")]
    fn begin_routes(&mut self, primary_context: Option<ContextId>, routes: &[(ContextId, Entity)]) {
        self.scopes.begin_routes(primary_context, routes);
    }

    #[cfg(feature = "render")]
    fn update_context(&mut self, context_id: ContextId, io: &imgui::Io) {
        self.scopes.update_context(context_id, io);
    }

    #[cfg(feature = "render")]
    fn remove_context(&mut self, context_id: ContextId) {
        self.scopes.remove_context(context_id);
    }

    #[cfg(feature = "render")]
    fn finish_routes(&mut self) {
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

pub(crate) fn install_input_mapping(app: &mut App) {
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    app.add_message::<RawWinitWindowEvent>();
    app.add_message::<WindowResized>()
        .add_message::<WindowScaleFactorChanged>()
        .add_message::<WindowBackendScaleFactorChanged>()
        .add_message::<WindowFocused>()
        .add_message::<CursorEntered>()
        .add_message::<CursorMoved>()
        .add_message::<CursorLeft>()
        .add_message::<Ime>()
        .add_message::<MouseButtonInput>()
        .add_message::<MouseWheel>()
        .add_message::<KeyboardInput>()
        .add_message::<KeyboardFocusLost>()
        .add_message::<TouchInput>()
        .init_resource::<ImguiInputState>()
        .init_resource::<ImguiInputCapture>();
    #[cfg(feature = "render")]
    app.init_resource::<ImguiContextInputMetrics>().add_systems(
        PreUpdate,
        routed_window_input_system.in_set(ImguiInputSystems),
    );
    #[cfg(not(feature = "render"))]
    app.add_systems(
        PreUpdate,
        primary_window_input_system.in_set(ImguiInputSystems),
    );
}

#[cfg(feature = "render")]
#[allow(clippy::too_many_arguments)]
fn routed_window_input_system(
    windows: Query<RoutedInputWindowComponents>,
    resolved_routes: Res<ImguiResolvedRoutes>,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))] viewport_bridge: NonSend<
        crate::ImguiViewportBridge,
    >,
    contexts: Option<NonSendMut<ImguiContexts>>,
    mut input_state: ResMut<ImguiInputState>,
    mut capture: ResMut<ImguiInputCapture>,
    mut input_metrics: ResMut<ImguiContextInputMetrics>,
    mut messages: ImguiInputMessageReaders,
) {
    let Some(mut contexts) = contexts else {
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        crate::viewport::native_window::release_pointer_capture();
        *input_state = ImguiInputState::default();
        *capture = ImguiInputCapture::default();
        *input_metrics = ImguiContextInputMetrics::default();
        discard_all_unread_messages(&mut messages);
        return;
    };
    let primary_context = contexts.primary_id();
    input_state.routed.primary_context = primary_context;
    input_state.routed.primary_window = windows
        .iter()
        .find_map(|(entity, _, primary_window, _, _)| primary_window.is_some().then_some(entity));

    let mut targets = Vec::new();
    for route in resolved_routes.input_routes().iter().copied() {
        let Ok((_, window, _, _, _)) = windows.get(route.host_window()) else {
            continue;
        };
        let target = routed_target_for_route(route, &resolved_routes, window);
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        {
            let mut target = target;
            if route_covers_window(route, window)
                && let Some(viewport_id) =
                    viewport_bridge.viewport_for_window(route.context_id(), route.host_window())
            {
                target.native_viewport = Some(ImguiInputWindow {
                    entity: route.host_window(),
                    scale_factor: window.scale_factor(),
                    viewport_id,
                    desktop_origin: viewport_bridge.viewport_desktop_origin_for_window(
                        route.context_id(),
                        route.host_window(),
                    ),
                });
            }
            targets.push(target);
        }
        #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
        targets.push(target);
    }

    let declared_slots = targets
        .iter()
        .map(|target| target.slot())
        .collect::<HashSet<_>>();
    let input_enabled_contexts = targets
        .iter()
        .filter(|target| !matches!(target.policy, ImguiInputPolicy::Disabled))
        .map(|target| target.context_id)
        .collect::<HashSet<_>>();
    for (entity, window, primary_window, viewport_window, viewport_owner) in &windows {
        let (Some(viewport_window), Some(viewport_owner)) = (viewport_window, viewport_owner)
        else {
            continue;
        };
        if !viewport_owner.matches_window(viewport_window) {
            continue;
        }
        let context_id = viewport_window.context_id();
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        if viewport_bridge.viewport_window(context_id, viewport_window.viewport_id())
            != Some(entity)
        {
            continue;
        }
        if primary_window.is_some()
            || !contexts.contains(context_id)
            || !input_enabled_contexts.contains(&context_id)
            || declared_slots.contains(&ImguiInputSlot {
                context_id,
                window: entity,
            })
        {
            continue;
        }
        targets.push(RoutedInputTarget {
            context_id,
            host_window: entity,
            logical_region: Rect::from_corners(
                Vec2::ZERO,
                Vec2::new(
                    finite_non_negative_size([window.width(), window.height()])[0],
                    finite_non_negative_size([window.width(), window.height()])[1],
                ),
            ),
            policy: ImguiInputPolicy::Exclusive { priority: i32::MAX },
            display_size: sanitized_window_display_size(window),
            framebuffer_scale: sanitized_window_framebuffer_scale(window),
            tracks_host_metrics: false,
            native_viewport: Some(ImguiInputWindow {
                #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                entity,
                #[cfg(any(
                    not(feature = "render"),
                    all(feature = "multi-viewport", not(target_arch = "wasm32"))
                ))]
                scale_factor: window.scale_factor(),
                viewport_id: viewport_window.viewport_id(),
                #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                desktop_origin: viewport_bridge
                    .viewport_desktop_origin_for_window(context_id, entity),
            }),
        });
    }

    let capture_routes = targets
        .iter()
        .map(|target| (target.context_id, target.host_window))
        .collect::<Vec<_>>();
    capture.begin_routes(primary_context, &capture_routes);

    let mut unavailable_contexts = HashSet::new();
    release_stale_routed_input(
        &mut contexts,
        &mut input_state,
        &targets,
        &mut unavailable_contexts,
    );

    let mut context_metrics = HashMap::new();
    for target in targets
        .iter()
        .copied()
        .filter(|target| target.tracks_host_metrics || !target.is_native_viewport())
    {
        context_metrics.insert(
            target.context_id,
            ImguiInputFrameMetrics {
                host_window: target.host_window,
                display_size: target.display_size,
                framebuffer_scale: target.framebuffer_scale,
            },
        );
        configure_routed_context(
            &mut contexts,
            target.context_id,
            &mut unavailable_contexts,
            |context| sync_routed_metrics(context, target),
        );
    }

    let mut focus_updates = messages
        .window_focused
        .read()
        .map(|event| {
            let targets_for_window = if event.focused {
                focus_targets_for_window(&input_state, &targets, event.window)
            } else {
                Vec::new()
            };
            (event.window, targets_for_window)
        })
        .collect::<Vec<_>>();
    if !messages.keyboard_focus_lost.is_empty() {
        messages.keyboard_focus_lost.clear();
        focus_updates.extend(
            input_state
                .routed
                .focused_targets
                .keys()
                .copied()
                .map(|window| (window, Vec::new())),
        );
    }
    let focus_message_windows = focus_updates
        .iter()
        .map(|(window, _)| *window)
        .collect::<HashSet<_>>();
    for (window, window_state, primary_window, _, _) in &windows {
        if primary_window.is_none() || focus_message_windows.contains(&window) {
            continue;
        }
        if window_state.focused {
            if input_state.routed.focused_targets.contains_key(&window) {
                continue;
            }
            let targets_for_window = default_focus_targets(&targets, window);
            if !targets_for_window.is_empty() {
                focus_updates.push((window, targets_for_window));
            }
        } else if input_state.routed.focused_targets.contains_key(&window) {
            focus_updates.push((window, Vec::new()));
        }
    }
    replace_routed_focus_targets(
        &mut contexts,
        &mut input_state,
        &focus_updates,
        &mut unavailable_contexts,
    );

    for event in messages.window_resized.read() {
        apply_routed_resize(
            &mut contexts,
            &targets,
            event.window,
            [event.width, event.height],
            &mut context_metrics,
            &mut unavailable_contexts,
        );
    }
    for event in messages.window_scale_factor_changed.read() {
        apply_routed_scale_factor(
            &mut contexts,
            &targets,
            event.window,
            positive_finite_or(event.scale_factor as f32, 1.0),
            &mut context_metrics,
            &mut unavailable_contexts,
        );
    }
    for event in messages.window_backend_scale_factor_changed.read() {
        apply_routed_scale_factor(
            &mut contexts,
            &targets,
            event.window,
            positive_finite_or(event.scale_factor as f32, 1.0),
            &mut context_metrics,
            &mut unavailable_contexts,
        );
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    // Bevy captures each logical position while handling the Winit event, before later events in
    // the same batch can change the Window's effective scale factor.
    let typed_cursor_moved = messages.cursor_moved.read().cloned().collect::<Vec<_>>();

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    let (mut ordered_pointer_events, mut raw_pointer_duplicates) = collect_raw_winit_pointer_events(
        &mut messages.raw_winit_window,
        &windows,
        &mut input_state.routed.raw_window_scale_factors,
        &typed_cursor_moved,
    );
    #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
    let (mut ordered_pointer_events, mut raw_pointer_duplicates) = (Vec::new(), HashMap::new());

    for event in messages.cursor_entered.read() {
        append_typed_pointer_event(
            &mut ordered_pointer_events,
            &mut raw_pointer_duplicates,
            OrderedPointerEvent::Entered {
                window: event.window,
            },
        );
    }
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    let cursor_moved = typed_cursor_moved.iter();
    #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
    let cursor_moved = messages.cursor_moved.read();
    for event in cursor_moved {
        append_typed_pointer_event(
            &mut ordered_pointer_events,
            &mut raw_pointer_duplicates,
            OrderedPointerEvent::Moved {
                window: event.window,
                position: event.position,
                #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                native_position: None,
            },
        );
    }
    for event in messages.cursor_left.read() {
        append_typed_pointer_event(
            &mut ordered_pointer_events,
            &mut raw_pointer_duplicates,
            OrderedPointerEvent::Left {
                window: event.window,
            },
        );
    }
    for event in messages.mouse_button_input.read() {
        append_typed_pointer_event(
            &mut ordered_pointer_events,
            &mut raw_pointer_duplicates,
            OrderedPointerEvent::Button {
                window: event.window,
                button: event.button,
                state: event.state,
            },
        );
    }

    for event in ordered_pointer_events {
        apply_routed_pointer_event(
            &mut contexts,
            &mut input_state,
            &targets,
            event,
            &mut unavailable_contexts,
        );
    }

    for event in messages.mouse_wheel.read() {
        for target in refresh_routed_pointer_from_cached_position(
            &mut contexts,
            &mut input_state,
            &targets,
            event.window,
            &mut unavailable_contexts,
        ) {
            configure_routed_context(
                &mut contexts,
                target.context_id,
                &mut unavailable_contexts,
                |context| {
                    let viewport_id = target.viewport_id(context);
                    let io = context.io_mut();
                    io.add_mouse_source_event(imgui::MouseSource::Mouse);
                    add_mouse_viewport_event(io, Some(viewport_id));
                    io.add_mouse_wheel_event(normalize_wheel(event.unit, event.x, event.y));
                },
            );
        }
    }

    for event in messages.touch_input.read() {
        apply_routed_touch_input(
            &mut contexts,
            &mut input_state,
            &targets,
            event,
            &mut unavailable_contexts,
        );
    }

    for event in messages.keyboard_input.read() {
        for target in focused_targets_for_window(&input_state, &targets, event.window) {
            apply_routed_keyboard_input(
                &mut contexts,
                &mut input_state,
                target,
                event,
                &mut unavailable_contexts,
            );
        }
    }

    for event in messages.ime.read() {
        let window = ime_window(event);
        for target in focused_targets_for_window(&input_state, &targets, window) {
            apply_routed_ime_event(
                &mut contexts,
                &mut input_state,
                target,
                event,
                &mut unavailable_contexts,
            );
        }
    }

    let capture_contexts = capture_routes
        .iter()
        .map(|(context_id, _)| *context_id)
        .collect::<HashSet<_>>();
    for context_id in capture_contexts {
        configure_routed_context(
            &mut contexts,
            context_id,
            &mut unavailable_contexts,
            |context| capture.update_context(context_id, context.io()),
        );
    }
    prune_unavailable_routed_contexts(&mut input_state, &mut capture, &unavailable_contexts);
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    if !routed_has_pressed_mouse_buttons(&input_state) {
        crate::viewport::native_window::release_pointer_capture();
    }
    capture.finish_routes();
    input_metrics.replace(resolved_routes.epoch(), context_metrics);
}

#[cfg(feature = "render")]
fn routed_target_for_route(
    route: ImguiResolvedInputRoute,
    resolved_routes: &ImguiResolvedRoutes,
    host_window: &Window,
) -> RoutedInputTarget {
    let fallback_scale = sanitized_window_framebuffer_scale(host_window);
    let render_route = resolved_routes.render_route(route.context_id());
    let (display_size, framebuffer_scale) = render_route
        .map(|render_route| {
            let physical = render_route.physical_output_size();
            let scale_factor = positive_finite_or(render_route.target_info().scale_factor, 1.0);
            (
                [
                    physical.x as f32 / scale_factor,
                    physical.y as f32 / scale_factor,
                ],
                [scale_factor, scale_factor],
            )
        })
        .unwrap_or((sanitized_window_display_size(host_window), fallback_scale));
    RoutedInputTarget {
        context_id: route.context_id(),
        host_window: route.host_window(),
        logical_region: route.logical_region(),
        policy: route.policy(),
        display_size: finite_non_negative_size(display_size),
        framebuffer_scale,
        tracks_host_metrics: route.source().as_camera().is_some() || render_route.is_none(),
        native_viewport: None,
    }
}

#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
fn route_covers_window(route: ImguiResolvedInputRoute, window: &Window) -> bool {
    let region = route.logical_region();
    let size = sanitized_window_display_size(window);
    region.min == Vec2::ZERO
        && (region.max.x - size[0]).abs() <= 0.5
        && (region.max.y - size[1]).abs() <= 0.5
}

#[cfg(feature = "render")]
#[derive(Clone, Copy, Debug, PartialEq)]
enum OrderedPointerEvent {
    Entered {
        window: Entity,
    },
    Moved {
        window: Entity,
        position: Vec2,
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        native_position: Option<Vec2>,
    },
    Left {
        window: Entity,
    },
    Button {
        window: Entity,
        button: BevyMouseButton,
        state: ButtonState,
    },
}

#[cfg(feature = "render")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum PointerEventIdentity {
    Entered {
        window: Entity,
    },
    Moved {
        window: Entity,
    },
    Left {
        window: Entity,
    },
    Button {
        window: Entity,
        button: BevyMouseButton,
        state: ButtonState,
    },
}

#[cfg(feature = "render")]
impl OrderedPointerEvent {
    // Raw and typed messages are two views of the same Winit occurrence. Coordinates cannot be
    // part of this identity because a later same-batch DPI change mutates the Window first.
    const fn identity(self) -> PointerEventIdentity {
        match self {
            Self::Entered { window } => PointerEventIdentity::Entered { window },
            Self::Moved { window, .. } => PointerEventIdentity::Moved { window },
            Self::Left { window } => PointerEventIdentity::Left { window },
            Self::Button {
                window,
                button,
                state,
            } => PointerEventIdentity::Button {
                window,
                button,
                state,
            },
        }
    }
}

#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
#[derive(Clone, Copy, Debug, PartialEq)]
enum RawWindowPointerEvent {
    ScaleFactorChanged {
        window: Entity,
        scale_factor: f32,
    },
    Entered {
        window: Entity,
    },
    Moved {
        window: Entity,
        physical_position: Vec2,
        current_native_scale_factor: f32,
        typed_logical_position: Option<Vec2>,
    },
    Left {
        window: Entity,
    },
    Button {
        window: Entity,
        button: BevyMouseButton,
        state: ButtonState,
    },
}

#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
fn collect_raw_winit_pointer_events(
    messages: &mut MessageReader<RawWinitWindowEvent>,
    windows: &Query<RoutedInputWindowComponents>,
    scale_factors: &mut HashMap<Entity, f32>,
    typed_cursor_moved: &[CursorMoved],
) -> (
    Vec<OrderedPointerEvent>,
    HashMap<PointerEventIdentity, usize>,
) {
    let live_windows = windows
        .iter()
        .map(|(entity, _, _, _, _)| entity)
        .collect::<HashSet<_>>();
    scale_factors.retain(|window, _| live_windows.contains(window));
    // Existing entries are the effective scales from the end of the previous raw batch. The
    // current Window value is only a first-batch fallback because Bevy has already applied every
    // raw event before this system runs.
    for (entity, window, _, _, _) in windows.iter() {
        scale_factors
            .entry(entity)
            .or_insert_with(|| positive_finite_or(window.scale_factor(), 1.0));
    }

    let mut typed_cursor_positions = HashMap::<Entity, VecDeque<Vec2>>::new();
    for event in typed_cursor_moved {
        typed_cursor_positions
            .entry(event.window)
            .or_default()
            .push_back(event.position);
    }

    let mut raw_events = Vec::new();
    for message in messages.read() {
        let Some(entity) = WINIT_WINDOWS
            .with_borrow(|winit_windows| winit_windows.get_window_entity(message.window_id))
        else {
            continue;
        };
        let Ok((_, window, _, _, _)) = windows.get(entity) else {
            continue;
        };
        let event = match &message.event {
            winit::event::WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                Some(RawWindowPointerEvent::ScaleFactorChanged {
                    window: entity,
                    scale_factor: window
                        .resolution
                        .scale_factor_override()
                        .unwrap_or_else(|| positive_finite_or(*scale_factor as f32, 1.0)),
                })
            }
            winit::event::WindowEvent::CursorEntered { .. } => {
                Some(RawWindowPointerEvent::Entered { window: entity })
            }
            winit::event::WindowEvent::CursorMoved { position, .. } => {
                Some(RawWindowPointerEvent::Moved {
                    window: entity,
                    physical_position: Vec2::new(position.x as f32, position.y as f32),
                    current_native_scale_factor: current_native_scale_factor(entity, window),
                    typed_logical_position: typed_cursor_positions
                        .get_mut(&entity)
                        .and_then(VecDeque::pop_front),
                })
            }
            winit::event::WindowEvent::CursorLeft { .. } => {
                Some(RawWindowPointerEvent::Left { window: entity })
            }
            winit::event::WindowEvent::MouseInput { state, button, .. } => {
                Some(RawWindowPointerEvent::Button {
                    window: entity,
                    button: map_winit_mouse_button(*button),
                    state: match *state {
                        winit::event::ElementState::Pressed => ButtonState::Pressed,
                        winit::event::ElementState::Released => ButtonState::Released,
                    },
                })
            }
            _ => None,
        };
        if let Some(event) = event {
            raw_events.push(event);
        }
    }

    let result = order_raw_pointer_events(raw_events, scale_factors);
    for (entity, window, _, _, _) in windows.iter() {
        scale_factors.insert(entity, positive_finite_or(window.scale_factor(), 1.0));
    }
    result
}

#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
fn order_raw_pointer_events(
    events: impl IntoIterator<Item = RawWindowPointerEvent>,
    scale_factors: &mut HashMap<Entity, f32>,
) -> (
    Vec<OrderedPointerEvent>,
    HashMap<PointerEventIdentity, usize>,
) {
    let mut ordered = Vec::new();
    let mut duplicates = HashMap::new();
    for event in events {
        let event = match event {
            RawWindowPointerEvent::ScaleFactorChanged {
                window,
                scale_factor,
            } => {
                scale_factors.insert(window, positive_finite_or(scale_factor, 1.0));
                continue;
            }
            RawWindowPointerEvent::Entered { window } => OrderedPointerEvent::Entered { window },
            RawWindowPointerEvent::Moved {
                window,
                physical_position,
                current_native_scale_factor,
                typed_logical_position,
            } => {
                let event_scale_factor = scale_factors
                    .get(&window)
                    .copied()
                    .map_or(1.0, |scale_factor| positive_finite_or(scale_factor, 1.0));
                let position = typed_logical_position
                    .unwrap_or_else(|| physical_position / event_scale_factor);
                OrderedPointerEvent::Moved {
                    window,
                    position,
                    native_position: Some(raw_native_pointer_position(
                        physical_position,
                        position,
                        current_native_scale_factor,
                    )),
                }
            }
            RawWindowPointerEvent::Left { window } => OrderedPointerEvent::Left { window },
            RawWindowPointerEvent::Button {
                window,
                button,
                state,
            } => OrderedPointerEvent::Button {
                window,
                button,
                state,
            },
        };
        ordered.push(event);
        *duplicates.entry(event.identity()).or_insert(0) += 1;
    }
    (ordered, duplicates)
}

#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
fn current_native_scale_factor(entity: Entity, window: &Window) -> f32 {
    WINIT_WINDOWS
        .with_borrow(|windows| {
            windows
                .get_window(entity)
                .map(|window| window.scale_factor() as f32)
        })
        .map_or_else(
            || positive_finite_or(window.scale_factor(), 1.0),
            |scale_factor| positive_finite_or(scale_factor, 1.0),
        )
}

#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
fn raw_native_pointer_position(
    physical_position: Vec2,
    logical_position: Vec2,
    current_native_scale_factor: f32,
) -> Vec2 {
    #[cfg(target_os = "macos")]
    {
        let _ = (physical_position, current_native_scale_factor);
        logical_position
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = logical_position;
        physical_position / positive_finite_or(current_native_scale_factor, 1.0)
    }
}

#[cfg(feature = "render")]
fn append_typed_pointer_event(
    ordered: &mut Vec<OrderedPointerEvent>,
    raw_duplicates: &mut HashMap<PointerEventIdentity, usize>,
    event: OrderedPointerEvent,
) {
    match raw_duplicates.entry(event.identity()) {
        std::collections::hash_map::Entry::Occupied(mut entry) => {
            if *entry.get() > 1 {
                *entry.get_mut() -= 1;
            } else {
                entry.remove();
            }
        }
        std::collections::hash_map::Entry::Vacant(_) => ordered.push(event),
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn map_winit_mouse_button(button: winit::event::MouseButton) -> BevyMouseButton {
    match button {
        winit::event::MouseButton::Left => BevyMouseButton::Left,
        winit::event::MouseButton::Right => BevyMouseButton::Right,
        winit::event::MouseButton::Middle => BevyMouseButton::Middle,
        winit::event::MouseButton::Back => BevyMouseButton::Back,
        winit::event::MouseButton::Forward => BevyMouseButton::Forward,
        winit::event::MouseButton::Other(button) => BevyMouseButton::Other(button),
    }
}

#[cfg(feature = "render")]
fn configure_routed_context(
    contexts: &mut ImguiContexts,
    context_id: ContextId,
    unavailable_contexts: &mut HashSet<ContextId>,
    operation: impl FnOnce(&mut imgui::Context),
) -> bool {
    if unavailable_contexts.contains(&context_id) {
        return false;
    }
    match contexts.configure(context_id, operation) {
        Ok(()) => true,
        Err(
            ImguiContextError::TeardownInProgress { .. } | ImguiContextError::UnknownContext { .. },
        ) => {
            unavailable_contexts.insert(context_id);
            false
        }
        Err(error) => panic!("dear-imgui-bevy could not prepare routed Context input: {error}"),
    }
}

#[cfg(feature = "render")]
fn sync_routed_metrics(context: &mut imgui::Context, target: RoutedInputTarget) {
    let io = context.io_mut();
    io.set_display_size(target.display_size);
    io.set_display_framebuffer_scale(target.framebuffer_scale);
}

#[cfg(feature = "render")]
fn pointer_targets_for_position(
    targets: &[RoutedInputTarget],
    host_window: Entity,
    position: Vec2,
) -> Vec<RoutedInputTarget> {
    let highest_exclusive = targets
        .iter()
        .filter(|target| target.host_window == host_window && target.contains(position))
        .filter_map(|target| target.policy.priority())
        .max();
    targets
        .iter()
        .copied()
        .filter(|target| target.host_window == host_window && target.contains(position))
        .filter(|target| match target.policy {
            ImguiInputPolicy::Exclusive { priority } => highest_exclusive == Some(priority),
            ImguiInputPolicy::Shared => true,
            ImguiInputPolicy::Disabled => false,
        })
        .collect()
}

#[cfg(feature = "render")]
fn default_pointer_targets(
    targets: &[RoutedInputTarget],
    host_window: Entity,
) -> Vec<RoutedInputTarget> {
    let exclusive_count = targets
        .iter()
        .filter(|target| {
            target.host_window == host_window
                && matches!(target.policy, ImguiInputPolicy::Exclusive { .. })
        })
        .count();
    targets
        .iter()
        .copied()
        .filter(|target| target.host_window == host_window)
        .filter(|target| {
            matches!(target.policy, ImguiInputPolicy::Shared)
                || (exclusive_count == 1
                    && matches!(target.policy, ImguiInputPolicy::Exclusive { .. }))
        })
        .collect()
}

#[cfg(feature = "render")]
fn targets_for_contexts(
    targets: &[RoutedInputTarget],
    host_window: Entity,
    context_ids: &[ContextId],
) -> Vec<RoutedInputTarget> {
    targets
        .iter()
        .copied()
        .filter(|target| {
            target.host_window == host_window && context_ids.contains(&target.context_id)
        })
        .collect()
}

#[cfg(feature = "render")]
fn pointer_targets_for_window_without_position(
    state: &ImguiInputState,
    targets: &[RoutedInputTarget],
    host_window: Entity,
) -> Vec<RoutedInputTarget> {
    if let Some(position) = state.routed.pointer_positions.get(&host_window) {
        return pointer_targets_for_position(targets, host_window, *position);
    }
    let pointer_contexts = state
        .routed
        .pointer_targets
        .get(&host_window)
        .filter(|contexts| !contexts.is_empty());
    pointer_contexts.map_or_else(
        || default_pointer_targets(targets, host_window),
        |contexts| targets_for_contexts(targets, host_window, contexts),
    )
}

#[cfg(feature = "render")]
fn refresh_routed_pointer_from_cached_position(
    contexts: &mut ImguiContexts,
    state: &mut ImguiInputState,
    targets: &[RoutedInputTarget],
    host_window: Entity,
    unavailable_contexts: &mut HashSet<ContextId>,
) -> Vec<RoutedInputTarget> {
    let Some(position) = state.routed.pointer_positions.get(&host_window).copied() else {
        return pointer_targets_for_window_without_position(state, targets, host_window);
    };
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    let native_position = state
        .routed
        .raw_native_pointer_positions
        .get(&host_window)
        .copied();
    let selected = pointer_targets_for_position(targets, host_window, position);
    replace_routed_pointer_targets(
        contexts,
        state,
        host_window,
        selected.iter().map(|target| target.context_id).collect(),
        unavailable_contexts,
    );
    for target in selected.iter().copied() {
        mark_routed_hovered(state, target, true);
        configure_routed_context(
            contexts,
            target.context_id,
            unavailable_contexts,
            |context| {
                #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                let position = if target.is_native_viewport() {
                    native_position.unwrap_or(position)
                } else {
                    position
                };
                add_routed_pointer_position(context, target, position, imgui::MouseSource::Mouse)
            },
        );
    }
    selected
}

#[cfg(feature = "render")]
fn apply_routed_pointer_event(
    contexts: &mut ImguiContexts,
    state: &mut ImguiInputState,
    targets: &[RoutedInputTarget],
    event: OrderedPointerEvent,
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    match event {
        OrderedPointerEvent::Entered { window } => {
            state.routed.pointer_outside_windows.remove(&window);
            clear_other_outside_window_pointers(contexts, state, window, unavailable_contexts);
            let selected = pointer_targets_for_window_without_position(state, targets, window);
            replace_routed_pointer_targets(
                contexts,
                state,
                window,
                selected.iter().map(|target| target.context_id).collect(),
                unavailable_contexts,
            );
            for target in selected {
                mark_routed_hovered(state, target, true);
                configure_routed_context(
                    contexts,
                    target.context_id,
                    unavailable_contexts,
                    |context| {
                        let viewport_id = target.viewport_id(context);
                        let io = context.io_mut();
                        io.add_mouse_source_event(imgui::MouseSource::Mouse);
                        add_mouse_viewport_event(io, Some(viewport_id));
                    },
                );
            }
        }
        OrderedPointerEvent::Moved {
            window,
            position,
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            native_position,
        } => {
            if !state.routed.pointer_outside_windows.contains(&window) {
                clear_other_outside_window_pointers(contexts, state, window, unavailable_contexts);
            }
            state.routed.pointer_positions.insert(window, position);
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            if let Some(native_position) = native_position {
                state
                    .routed
                    .raw_native_pointer_positions
                    .insert(window, native_position);
            } else {
                state.routed.raw_native_pointer_positions.remove(&window);
            }
            let pointer_targets = refresh_routed_pointer_from_cached_position(
                contexts,
                state,
                targets,
                window,
                unavailable_contexts,
            );
            #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
            let _ = pointer_targets;
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            if routed_window_has_pressed_mouse_buttons(state, window)
                && pointer_targets
                    .iter()
                    .any(|target| target.is_native_viewport())
            {
                crate::viewport::native_window::capture_pointer(window);
            }
        }
        OrderedPointerEvent::Left { window } => {
            state.routed.pointer_outside_windows.insert(window);
            if !routed_window_has_pressed_mouse_buttons(state, window) {
                clear_routed_window_pointer(contexts, state, window, unavailable_contexts);
            }
        }
        OrderedPointerEvent::Button {
            window,
            button,
            state: button_state,
        } => {
            let pointer_targets = refresh_routed_pointer_from_cached_position(
                contexts,
                state,
                targets,
                window,
                unavailable_contexts,
            );
            let button_targets = if button_state.is_pressed() {
                pointer_targets
            } else {
                pointer_or_sticky_button_targets(state, targets, window, button)
            };
            if button_state.is_pressed() {
                replace_routed_focus_targets(
                    contexts,
                    state,
                    &[(
                        window,
                        button_targets
                            .iter()
                            .map(|target| target.context_id)
                            .collect(),
                    )],
                    unavailable_contexts,
                );
            }
            if let Some(button) = map_bevy_mouse_button(button) {
                for target in button_targets.iter().copied() {
                    apply_routed_mouse_button(
                        contexts,
                        state,
                        target,
                        button,
                        button_state.is_pressed(),
                        unavailable_contexts,
                    );
                }
            }

            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            {
                if !routed_has_pressed_mouse_buttons(state) {
                    crate::viewport::native_window::release_pointer_capture();
                    clear_all_outside_window_pointers(contexts, state, unavailable_contexts);
                }
            }
        }
    }
}

#[cfg(feature = "render")]
fn routed_window_has_pressed_mouse_buttons(state: &ImguiInputState, window: Entity) -> bool {
    state.routed.windows.iter().any(|(slot, window_state)| {
        slot.window == window && !window_state.pressed_mouse_buttons.is_empty()
    })
}

#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
fn routed_has_pressed_mouse_buttons(state: &ImguiInputState) -> bool {
    state
        .routed
        .windows
        .values()
        .any(|window_state| !window_state.pressed_mouse_buttons.is_empty())
}

#[cfg(feature = "render")]
fn clear_routed_window_pointer(
    contexts: &mut ImguiContexts,
    state: &mut ImguiInputState,
    window: Entity,
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    state.routed.pointer_positions.remove(&window);
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    state.routed.raw_native_pointer_positions.remove(&window);
    replace_routed_pointer_targets(contexts, state, window, Vec::new(), unavailable_contexts);
}

#[cfg(feature = "render")]
fn clear_other_outside_window_pointers(
    contexts: &mut ImguiContexts,
    state: &mut ImguiInputState,
    active_window: Entity,
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    let stale_windows = state
        .routed
        .pointer_outside_windows
        .iter()
        .copied()
        .filter(|window| *window != active_window)
        .collect::<Vec<_>>();
    for window in stale_windows {
        state.routed.pointer_outside_windows.remove(&window);
        clear_routed_window_pointer(contexts, state, window, unavailable_contexts);
    }
}

#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
fn clear_all_outside_window_pointers(
    contexts: &mut ImguiContexts,
    state: &mut ImguiInputState,
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    let stale_windows = state
        .routed
        .pointer_outside_windows
        .drain()
        .collect::<Vec<_>>();
    for window in stale_windows {
        clear_routed_window_pointer(contexts, state, window, unavailable_contexts);
    }
}

#[cfg(feature = "render")]
fn default_focus_targets(targets: &[RoutedInputTarget], host_window: Entity) -> Vec<ContextId> {
    default_pointer_targets(targets, host_window)
        .into_iter()
        .map(|target| target.context_id)
        .collect()
}

#[cfg(feature = "render")]
fn focus_targets_for_window(
    state: &ImguiInputState,
    targets: &[RoutedInputTarget],
    host_window: Entity,
) -> Vec<ContextId> {
    state
        .routed
        .pointer_targets
        .get(&host_window)
        .filter(|contexts| !contexts.is_empty())
        .cloned()
        .unwrap_or_else(|| default_focus_targets(targets, host_window))
}

#[cfg(feature = "render")]
fn focused_targets_for_window(
    state: &ImguiInputState,
    targets: &[RoutedInputTarget],
    host_window: Entity,
) -> Vec<RoutedInputTarget> {
    state
        .routed
        .focused_targets
        .get(&host_window)
        .map_or_else(Vec::new, |contexts| {
            targets_for_contexts(targets, host_window, contexts)
        })
}

#[cfg(feature = "render")]
fn unique_contexts(contexts: Vec<ContextId>) -> Vec<ContextId> {
    let mut seen = HashSet::new();
    contexts
        .into_iter()
        .filter(|context_id| seen.insert(*context_id))
        .collect()
}

#[cfg(feature = "render")]
fn replace_routed_pointer_targets(
    contexts: &mut ImguiContexts,
    state: &mut ImguiInputState,
    host_window: Entity,
    next: Vec<ContextId>,
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    let next = unique_contexts(next);
    let previous = if next.is_empty() {
        state
            .routed
            .pointer_targets
            .remove(&host_window)
            .unwrap_or_default()
    } else {
        state
            .routed
            .pointer_targets
            .insert(host_window, next.clone())
            .unwrap_or_default()
    };
    for context_id in previous {
        if next.contains(&context_id) {
            continue;
        }
        let slot = ImguiInputSlot {
            context_id,
            window: host_window,
        };
        let hovered = state
            .routed
            .windows
            .get_mut(&slot)
            .is_some_and(|window_state| {
                let hovered = window_state.mouse_hovered;
                window_state.mouse_hovered = false;
                hovered
            });
        if state.routed.last_hovered.get(&context_id) == Some(&host_window) {
            state.routed.last_hovered.remove(&context_id);
        }
        let another_window_is_hovered =
            state
                .routed
                .windows
                .iter()
                .any(|(other_slot, window_state)| {
                    other_slot.context_id == context_id && window_state.mouse_hovered
                });
        if hovered && !another_window_is_hovered {
            configure_routed_context(
                contexts,
                context_id,
                unavailable_contexts,
                clear_routed_pointer,
            );
        }
    }
}

#[cfg(feature = "render")]
fn mark_routed_hovered(state: &mut ImguiInputState, target: RoutedInputTarget, hovered: bool) {
    let slot = target.slot();
    state.routed.windows.entry(slot).or_default().mouse_hovered = hovered;
    if hovered {
        state
            .routed
            .last_hovered
            .insert(target.context_id, target.host_window);
    } else if state.routed.last_hovered.get(&target.context_id) == Some(&target.host_window) {
        state.routed.last_hovered.remove(&target.context_id);
    }
}

#[cfg(feature = "render")]
fn add_routed_pointer_position(
    context: &mut imgui::Context,
    target: RoutedInputTarget,
    position: Vec2,
    source: imgui::MouseSource,
) {
    let mouse_position = target.map_position(context, position);
    let viewport_id = target.viewport_id(context);
    let io = context.io_mut();
    io.add_mouse_source_event(source);
    add_mouse_viewport_event(io, Some(viewport_id));
    io.add_mouse_pos_event(mouse_position);
}

#[cfg(feature = "render")]
fn clear_routed_pointer(context: &mut imgui::Context) {
    let io = context.io_mut();
    io.add_mouse_source_event(imgui::MouseSource::Mouse);
    add_mouse_viewport_event(io, None);
    io.add_mouse_pos_event(INVALID_MOUSE_POS);
    clear_mouse_hovered_viewport(io);
}

#[cfg(feature = "render")]
fn focused_context_ids(state: &RoutedInputState) -> HashSet<ContextId> {
    state.focused_targets.values().flatten().copied().collect()
}

#[cfg(feature = "render")]
fn replace_routed_focus_targets(
    contexts: &mut ImguiContexts,
    state: &mut ImguiInputState,
    updates: &[(Entity, Vec<ContextId>)],
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    if updates.is_empty() {
        return;
    }

    let before = focused_context_ids(&state.routed);
    for &(window, ref next) in updates {
        let next = unique_contexts(next.clone());
        if next.is_empty() {
            state.routed.focused_targets.remove(&window);
        } else {
            state.routed.focused_targets.insert(window, next);
        }
    }

    for window_state in state.routed.windows.values_mut() {
        window_state.focused = false;
    }
    state.routed.last_focused.clear();
    for (&window, context_ids) in &state.routed.focused_targets {
        for &context_id in context_ids {
            state
                .routed
                .windows
                .entry(ImguiInputSlot { context_id, window })
                .or_default()
                .focused = true;
            state.routed.last_focused.insert(context_id, window);
        }
    }

    let after = focused_context_ids(&state.routed);
    for context_id in before.difference(&after).copied() {
        configure_routed_context(contexts, context_id, unavailable_contexts, |context| {
            release_routed_context_input(context, state, context_id)
        });
    }
    for context_id in after.difference(&before).copied() {
        configure_routed_context(contexts, context_id, unavailable_contexts, |context| {
            context.io_mut().add_focus_event(true)
        });
    }
}

#[cfg(feature = "render")]
fn release_routed_context_input(
    context: &mut imgui::Context,
    state: &mut ImguiInputState,
    context_id: ContextId,
) {
    for (slot, window_state) in &mut state.routed.windows {
        if slot.context_id != context_id {
            continue;
        }
        release_routed_sticky_input(context, window_state);
        if window_state.mouse_hovered {
            window_state.mouse_hovered = false;
            clear_routed_pointer(context);
        }
        window_state.focused = false;
    }
    state.routed.last_focused.remove(&context_id);
    state.routed.last_hovered.remove(&context_id);
    context.io_mut().add_focus_event(false);
}

#[cfg(feature = "render")]
fn release_routed_sticky_input(context: &mut imgui::Context, state: &mut ImguiRoutedWindowState) {
    let io = context.io_mut();
    release_sticky_keys_and_buttons(
        io,
        &mut state.pressed_keys,
        &mut state.pressed_mouse_buttons,
    );
    if state.active_touch_id.take().is_some() {
        io.add_mouse_source_event(imgui::MouseSource::TouchScreen);
        io.add_mouse_button_event(imgui::MouseButton::Left, false);
    }
}

#[cfg(feature = "render")]
fn release_stale_routed_input(
    contexts: &mut ImguiContexts,
    state: &mut ImguiInputState,
    targets: &[RoutedInputTarget],
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    let valid_slots = targets
        .iter()
        .map(|target| target.slot())
        .collect::<HashSet<_>>();
    let before = focused_context_ids(&state.routed);
    let stale_slots = state
        .routed
        .windows
        .keys()
        .filter(|slot| !valid_slots.contains(slot))
        .copied()
        .collect::<Vec<_>>();
    for slot in stale_slots {
        let Some(mut window_state) = state.routed.windows.remove(&slot) else {
            continue;
        };
        state
            .routed
            .pointer_targets
            .entry(slot.window)
            .and_modify(|contexts| contexts.retain(|context_id| *context_id != slot.context_id));
        state
            .routed
            .focused_targets
            .entry(slot.window)
            .and_modify(|contexts| contexts.retain(|context_id| *context_id != slot.context_id));
        if state.routed.last_hovered.get(&slot.context_id) == Some(&slot.window) {
            state.routed.last_hovered.remove(&slot.context_id);
        }
        if state.routed.last_focused.get(&slot.context_id) == Some(&slot.window) {
            state.routed.last_focused.remove(&slot.context_id);
        }
        let had_pointer_input = window_state.mouse_hovered
            || !window_state.pressed_mouse_buttons.is_empty()
            || window_state.active_touch_id.is_some();
        configure_routed_context(contexts, slot.context_id, unavailable_contexts, |context| {
            release_routed_sticky_input(context, &mut window_state);
            if had_pointer_input {
                clear_routed_pointer(context);
            }
        });
    }
    state
        .routed
        .pointer_targets
        .retain(|_, contexts| !contexts.is_empty());
    state
        .routed
        .focused_targets
        .retain(|_, contexts| !contexts.is_empty());
    state
        .routed
        .pointer_outside_windows
        .retain(|window| targets.iter().any(|target| target.host_window == *window));

    let after = focused_context_ids(&state.routed);
    for context_id in before.difference(&after).copied() {
        configure_routed_context(contexts, context_id, unavailable_contexts, |context| {
            release_routed_context_input(context, state, context_id)
        });
    }
}

#[cfg(feature = "render")]
fn apply_routed_resize(
    contexts: &mut ImguiContexts,
    targets: &[RoutedInputTarget],
    host_window: Entity,
    size: [f32; 2],
    context_metrics: &mut HashMap<ContextId, ImguiInputFrameMetrics>,
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    for target in targets.iter().copied().filter(|target| {
        target.host_window == host_window
            && target.tracks_host_metrics
            && target.logical_region.min == Vec2::ZERO
    }) {
        let display_size = finite_non_negative_size(size);
        let metrics = ImguiInputFrameMetrics {
            host_window,
            display_size,
            framebuffer_scale: target.framebuffer_scale,
        };
        context_metrics.insert(target.context_id, metrics);
        configure_routed_context(
            contexts,
            target.context_id,
            unavailable_contexts,
            |context| {
                let io = context.io_mut();
                io.set_display_size(metrics.display_size);
                io.set_display_framebuffer_scale(metrics.framebuffer_scale);
            },
        );
    }
}

#[cfg(feature = "render")]
fn apply_routed_scale_factor(
    contexts: &mut ImguiContexts,
    targets: &[RoutedInputTarget],
    host_window: Entity,
    scale_factor: f32,
    context_metrics: &mut HashMap<ContextId, ImguiInputFrameMetrics>,
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    for target in targets
        .iter()
        .copied()
        .filter(|target| target.host_window == host_window && target.tracks_host_metrics)
    {
        let metrics = ImguiInputFrameMetrics {
            host_window,
            display_size: context_metrics
                .get(&target.context_id)
                .map_or(target.display_size, |metrics| metrics.display_size),
            framebuffer_scale: [scale_factor, scale_factor],
        };
        context_metrics.insert(target.context_id, metrics);
        configure_routed_context(
            contexts,
            target.context_id,
            unavailable_contexts,
            |context| {
                let io = context.io_mut();
                io.set_display_size(metrics.display_size);
                io.set_display_framebuffer_scale(metrics.framebuffer_scale);
            },
        );
    }
}

#[cfg(feature = "render")]
fn pointer_or_sticky_button_targets(
    state: &ImguiInputState,
    targets: &[RoutedInputTarget],
    host_window: Entity,
    button: BevyMouseButton,
) -> Vec<RoutedInputTarget> {
    let Some(button) = map_bevy_mouse_button(button) else {
        return Vec::new();
    };
    let mut context_ids = state
        .routed
        .pointer_targets
        .get(&host_window)
        .cloned()
        .unwrap_or_default();
    context_ids.extend(
        state
            .routed
            .windows
            .iter()
            .filter(|(_, window_state)| window_state.pressed_mouse_buttons.contains(&button))
            .map(|(slot, _)| slot.context_id),
    );
    unique_contexts(context_ids)
        .into_iter()
        .filter_map(|context_id| {
            targets
                .iter()
                .find(|target| target.context_id == context_id && target.host_window == host_window)
                .or_else(|| {
                    targets
                        .iter()
                        .find(|target| target.context_id == context_id)
                })
                .copied()
        })
        .collect()
}

#[cfg(feature = "render")]
fn apply_routed_mouse_button(
    contexts: &mut ImguiContexts,
    state: &mut ImguiInputState,
    target: RoutedInputTarget,
    button: imgui::MouseButton,
    pressed: bool,
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    if pressed {
        state
            .routed
            .windows
            .entry(target.slot())
            .or_default()
            .pressed_mouse_buttons
            .insert(button);
    } else {
        for (slot, window_state) in &mut state.routed.windows {
            if slot.context_id == target.context_id {
                window_state.pressed_mouse_buttons.remove(&button);
            }
        }
    }
    configure_routed_context(
        contexts,
        target.context_id,
        unavailable_contexts,
        |context| {
            let viewport_id = target.viewport_id(context);
            let io = context.io_mut();
            io.add_mouse_source_event(imgui::MouseSource::Mouse);
            add_mouse_viewport_event(io, Some(viewport_id));
            io.add_mouse_button_event(button, pressed);
        },
    );
}

#[cfg(feature = "render")]
fn active_touch_targets(
    state: &ImguiInputState,
    targets: &[RoutedInputTarget],
    window: Entity,
    touch_id: u64,
) -> Vec<RoutedInputTarget> {
    let context_ids = state
        .routed
        .windows
        .iter()
        .filter(|(slot, window_state)| {
            slot.window == window && window_state.active_touch_id == Some(touch_id)
        })
        .map(|(slot, _)| slot.context_id)
        .collect::<Vec<_>>();
    targets_for_contexts(targets, window, &context_ids)
}

#[cfg(feature = "render")]
fn apply_routed_touch_input(
    contexts: &mut ImguiContexts,
    state: &mut ImguiInputState,
    targets: &[RoutedInputTarget],
    event: &TouchInput,
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    let selected = match event.phase {
        TouchPhase::Started => pointer_targets_for_position(targets, event.window, event.position),
        TouchPhase::Moved | TouchPhase::Ended | TouchPhase::Canceled => {
            active_touch_targets(state, targets, event.window, event.id)
        }
    };
    for target in selected {
        let window_state = state.routed.windows.entry(target.slot()).or_default();
        match event.phase {
            TouchPhase::Started if window_state.active_touch_id.is_none() => {
                window_state.active_touch_id = Some(event.id);
                configure_routed_context(
                    contexts,
                    target.context_id,
                    unavailable_contexts,
                    |context| {
                        add_routed_pointer_position(
                            context,
                            target,
                            event.position,
                            imgui::MouseSource::TouchScreen,
                        );
                        context
                            .io_mut()
                            .add_mouse_button_event(imgui::MouseButton::Left, true);
                    },
                );
            }
            TouchPhase::Moved if window_state.active_touch_id == Some(event.id) => {
                configure_routed_context(
                    contexts,
                    target.context_id,
                    unavailable_contexts,
                    |context| {
                        add_routed_pointer_position(
                            context,
                            target,
                            event.position,
                            imgui::MouseSource::TouchScreen,
                        );
                    },
                );
            }
            TouchPhase::Ended | TouchPhase::Canceled
                if window_state.active_touch_id == Some(event.id) =>
            {
                window_state.active_touch_id = None;
                configure_routed_context(
                    contexts,
                    target.context_id,
                    unavailable_contexts,
                    |context| {
                        add_routed_pointer_position(
                            context,
                            target,
                            event.position,
                            imgui::MouseSource::TouchScreen,
                        );
                        context
                            .io_mut()
                            .add_mouse_button_event(imgui::MouseButton::Left, false);
                    },
                );
            }
            _ => {}
        }
    }
}

#[cfg(feature = "render")]
fn apply_routed_keyboard_input(
    contexts: &mut ImguiContexts,
    state: &mut ImguiInputState,
    target: RoutedInputTarget,
    event: &KeyboardInput,
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    let pressed = event.state.is_pressed();
    let key = map_bevy_key_code(event.key_code);
    if let Some(key) = key {
        if pressed {
            state
                .routed
                .windows
                .entry(target.slot())
                .or_default()
                .pressed_keys
                .insert(key);
        } else {
            for (slot, window_state) in &mut state.routed.windows {
                if slot.context_id == target.context_id {
                    window_state.pressed_keys.remove(&key);
                }
            }
        }
    }
    let modifiers = routed_context_modifiers(&state.routed, target.context_id);
    configure_routed_context(
        contexts,
        target.context_id,
        unavailable_contexts,
        |context| {
            let io = context.io_mut();
            if pressed && let Some(text) = &event.text {
                add_keyboard_text(io, text);
            }
            apply_modifier_events(io, modifiers);
            if let Some(key) = key {
                io.add_key_event(key, pressed);
            }
        },
    );
}

#[cfg(feature = "render")]
fn routed_context_modifiers(
    state: &RoutedInputState,
    context_id: ContextId,
) -> (bool, bool, bool, bool) {
    state
        .windows
        .iter()
        .filter(|(slot, _)| slot.context_id == context_id)
        .fold((false, false, false, false), |aggregate, (_, window)| {
            let modifiers = window.modifiers();
            (
                aggregate.0 || modifiers.0,
                aggregate.1 || modifiers.1,
                aggregate.2 || modifiers.2,
                aggregate.3 || modifiers.3,
            )
        })
}

#[cfg(feature = "render")]
fn ime_window(event: &Ime) -> Entity {
    match event {
        Ime::Preedit { window, .. }
        | Ime::Commit { window, .. }
        | Ime::Enabled { window }
        | Ime::Disabled { window } => *window,
    }
}

#[cfg(feature = "render")]
fn apply_routed_ime_event(
    contexts: &mut ImguiContexts,
    state: &mut ImguiInputState,
    target: RoutedInputTarget,
    event: &Ime,
    unavailable_contexts: &mut HashSet<ContextId>,
) {
    match event {
        Ime::Enabled { .. } => {
            state
                .routed
                .windows
                .entry(target.slot())
                .or_default()
                .ime_enabled = true;
        }
        Ime::Disabled { .. } => {
            state
                .routed
                .windows
                .entry(target.slot())
                .or_default()
                .ime_enabled = false;
        }
        Ime::Preedit { .. } | Ime::Commit { .. } => {}
    }
    configure_routed_context(
        contexts,
        target.context_id,
        unavailable_contexts,
        |context| {
            if let Ime::Commit { value, .. } = event {
                add_ime_text(context.io_mut(), value);
            }
        },
    );
}

#[cfg(feature = "render")]
fn prune_unavailable_routed_contexts(
    state: &mut ImguiInputState,
    capture: &mut ImguiInputCapture,
    unavailable_contexts: &HashSet<ContextId>,
) {
    if unavailable_contexts.is_empty() {
        return;
    }
    state
        .routed
        .windows
        .retain(|slot, _| !unavailable_contexts.contains(&slot.context_id));
    state.routed.pointer_targets.retain(|_, contexts| {
        contexts.retain(|context_id| !unavailable_contexts.contains(context_id));
        !contexts.is_empty()
    });
    state.routed.focused_targets.retain(|_, contexts| {
        contexts.retain(|context_id| !unavailable_contexts.contains(context_id));
        !contexts.is_empty()
    });
    state
        .routed
        .last_focused
        .retain(|context_id, _| !unavailable_contexts.contains(context_id));
    state
        .routed
        .last_hovered
        .retain(|context_id, _| !unavailable_contexts.contains(context_id));
    for &context_id in unavailable_contexts {
        capture.remove_context(context_id);
    }
}

/// Translate primary-window Bevy messages into Dear ImGui IO events.
#[allow(clippy::too_many_arguments)]
#[cfg(not(feature = "render"))]
pub(crate) fn primary_window_input_system(
    primary_window: Query<(Entity, &Window), With<PrimaryWindow>>,
    viewport_windows: Query<
        (Entity, &Window, &ImguiViewportWindow, &ImguiViewportOwner),
        Without<PrimaryWindow>,
    >,
    contexts: Option<NonSendMut<ImguiContexts>>,
    mut input_state: ResMut<ImguiInputState>,
    mut capture: ResMut<ImguiInputCapture>,
    mut messages: ImguiInputMessageReaders,
) {
    let Some(mut contexts) = contexts else {
        *input_state = ImguiInputState::default();
        *capture = ImguiInputCapture::default();
        discard_all_unread_messages(&mut messages);
        return;
    };
    let Some(primary_id) = contexts.primary_id() else {
        *capture = ImguiInputCapture::default();
        discard_all_unread_messages(&mut messages);
        return;
    };

    let result = contexts.configure(primary_id, |context| {
        translate_primary_window_input(
            &primary_window,
            &viewport_windows,
            primary_id,
            context,
            &mut input_state,
            &mut capture,
            &mut messages,
        );
    });
    match result {
        Ok(()) => {}
        Err(
            ImguiContextError::TeardownInProgress { .. } | ImguiContextError::UnknownContext { .. },
        ) => {
            *capture = ImguiInputCapture::default();
            discard_all_unread_messages(&mut messages);
        }
        Err(error) => {
            panic!("dear-imgui-bevy could not prepare primary Context input: {error}")
        }
    }
}

#[cfg(not(feature = "render"))]
fn translate_primary_window_input(
    primary_window: &Query<(Entity, &Window), With<PrimaryWindow>>,
    viewport_windows: &Query<
        (Entity, &Window, &ImguiViewportWindow, &ImguiViewportOwner),
        Without<PrimaryWindow>,
    >,
    primary_context: ContextId,
    context: &mut imgui::Context,
    input_state: &mut ImguiInputState,
    capture: &mut ImguiInputCapture,
    messages: &mut ImguiInputMessageReaders,
) {
    let Ok((primary_window_entity, window)) = primary_window.single() else {
        release_input_for_missing_primary_window(context, input_state);
        *capture = ImguiInputCapture::default();
        discard_all_unread_messages(messages);
        return;
    };

    sync_window_metrics(context, window);
    let primary_viewport_id = context.main_viewport().id();
    let primary_window = ImguiInputWindow {
        entity: primary_window_entity,
        position: window.position,
        scale_factor: window.scale_factor(),
        viewport_id: primary_viewport_id,
        context_id: primary_context,
        is_primary: true,
    };
    prune_stale_window_state(
        context,
        input_state,
        primary_window,
        viewport_windows,
        window.focused,
    );

    for event in messages
        .window_resized
        .read()
        .filter(|event| event.window == primary_window_entity)
    {
        context
            .io_mut()
            .set_display_size(finite_non_negative_size([event.width, event.height]));
    }

    for event in messages
        .window_scale_factor_changed
        .read()
        .filter(|event| event.window == primary_window_entity)
    {
        set_framebuffer_scale(context, positive_finite_or(event.scale_factor as f32, 1.0));
    }

    for event in messages
        .window_backend_scale_factor_changed
        .read()
        .filter(|event| event.window == primary_window_entity)
    {
        set_framebuffer_scale(context, positive_finite_or(event.scale_factor as f32, 1.0));
    }

    let focus_events = messages
        .window_focused
        .read()
        .filter_map(|event| {
            imgui_window_for_event(event.window, primary_window, viewport_windows)
                .map(|window| (window, event.focused))
        })
        .collect::<Vec<_>>();
    let keyboard_focus_lost = !messages.keyboard_focus_lost.is_empty();
    if keyboard_focus_lost {
        messages.keyboard_focus_lost.clear();
    }
    if focus_events.is_empty() && !keyboard_focus_lost {
        sync_initial_focus(context, input_state, primary_window, window.focused);
    } else if input_state.primary_window_focused.is_none() {
        input_state.primary_window_focused = Some(window.focused);
    }
    apply_focus_events(context, input_state, &focus_events);

    if keyboard_focus_lost {
        apply_focus_event(context, input_state, primary_window, false);
    }

    for (_event, window) in messages.cursor_entered.read().filter_map(|event| {
        imgui_window_for_event(event.window, primary_window, viewport_windows)
            .map(|window| (event, window))
    }) {
        input_state.mouse_hovered_window = Some(window.entity);
        let io = context.io_mut();
        io.add_mouse_source_event(imgui::MouseSource::Mouse);
        add_mouse_viewport_event(io, Some(window.viewport_id));
    }

    for event in messages.cursor_moved.read().filter_map(|event| {
        imgui_window_for_event(event.window, primary_window, viewport_windows)
            .map(|window| (event, window))
    }) {
        let (event, window) = event;
        input_state.mouse_hovered_window = Some(window.entity);
        let mouse_pos = mouse_pos_for_window(context, window, event.position);
        let io = context.io_mut();
        io.add_mouse_source_event(imgui::MouseSource::Mouse);
        add_mouse_viewport_event(io, Some(window.viewport_id));
        io.add_mouse_pos_event(mouse_pos);
    }

    for window in messages
        .cursor_left
        .read()
        .filter_map(|event| imgui_window_for_event(event.window, primary_window, viewport_windows))
    {
        if input_state
            .mouse_hovered_window
            .is_some_and(|entity| entity != window.entity)
        {
            continue;
        }
        input_state.mouse_hovered_window = None;
        let io = context.io_mut();
        io.add_mouse_source_event(imgui::MouseSource::Mouse);
        add_mouse_viewport_event(io, None);
        io.add_mouse_pos_event(INVALID_MOUSE_POS);
    }

    for event in messages.mouse_button_input.read().filter(|event| {
        imgui_window_for_event(event.window, primary_window, viewport_windows).is_some()
    }) {
        if let Some(button) = map_bevy_mouse_button(event.button) {
            let pressed = event.state.is_pressed();
            if pressed {
                input_state.pressed_mouse_buttons.insert(button);
            } else {
                input_state.pressed_mouse_buttons.remove(&button);
            }
            let io = context.io_mut();
            io.add_mouse_source_event(imgui::MouseSource::Mouse);
            if let Some(window) =
                imgui_window_for_event(event.window, primary_window, viewport_windows)
            {
                add_mouse_viewport_event(io, Some(window.viewport_id));
            }
            io.add_mouse_button_event(button, pressed);
        }
    }

    for event in messages.mouse_wheel.read().filter_map(|event| {
        imgui_window_for_event(event.window, primary_window, viewport_windows)
            .map(|window| (event, window))
    }) {
        let (event, window) = event;
        let io = context.io_mut();
        io.add_mouse_source_event(imgui::MouseSource::Mouse);
        add_mouse_viewport_event(io, Some(window.viewport_id));
        io.add_mouse_wheel_event(normalize_wheel(event.unit, event.x, event.y));
    }

    for event in messages.keyboard_input.read().filter(|event| {
        imgui_window_for_event(event.window, primary_window, viewport_windows).is_some()
    }) {
        apply_keyboard_input(context, input_state, event);
    }

    for event in messages.touch_input.read().filter_map(|event| {
        imgui_window_for_event(event.window, primary_window, viewport_windows)
            .map(|window| (event, window))
    }) {
        let (event, window) = event;
        apply_touch_input(context, input_state, event, window);
    }

    for event in messages.ime.read() {
        let window = match event {
            Ime::Preedit { window, .. }
            | Ime::Commit { window, .. }
            | Ime::Enabled { window }
            | Ime::Disabled { window } => *window,
        };
        if imgui_window_for_event(window, primary_window, viewport_windows).is_some() {
            apply_ime_event(context, input_state, event);
        }
    }

    capture.update_from_io(primary_context, primary_window_entity, context.io());
}

#[derive(Clone, Copy)]
struct ImguiInputWindow {
    #[cfg(any(
        not(feature = "render"),
        all(feature = "multi-viewport", not(target_arch = "wasm32"))
    ))]
    entity: Entity,
    #[cfg(not(feature = "render"))]
    position: WindowPosition,
    #[cfg(any(
        not(feature = "render"),
        all(feature = "multi-viewport", not(target_arch = "wasm32"))
    ))]
    scale_factor: f32,
    viewport_id: imgui::Id,
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    desktop_origin: Option<[f32; 2]>,
    #[cfg(not(feature = "render"))]
    context_id: ContextId,
    #[cfg(not(feature = "render"))]
    is_primary: bool,
}

#[cfg(not(feature = "render"))]
fn imgui_window_for_event(
    entity: Entity,
    primary_window: ImguiInputWindow,
    viewport_windows: &Query<
        (Entity, &Window, &ImguiViewportWindow, &ImguiViewportOwner),
        Without<PrimaryWindow>,
    >,
) -> Option<ImguiInputWindow> {
    if entity == primary_window.entity {
        return Some(primary_window);
    }

    let Ok((entity, window, viewport_window, viewport_owner)) = viewport_windows.get(entity) else {
        return None;
    };
    if !viewport_owner.matches_window(viewport_window)
        || viewport_window.context_id() != primary_window.context_id
    {
        return None;
    }
    Some(ImguiInputWindow {
        entity,
        position: window.position,
        scale_factor: window.scale_factor(),
        viewport_id: viewport_window.viewport_id(),
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        desktop_origin: None,
        context_id: viewport_window.context_id(),
        is_primary: false,
    })
}

#[cfg(not(feature = "render"))]
fn is_mapped_imgui_window(
    entity: Entity,
    primary_window: ImguiInputWindow,
    viewport_windows: &Query<
        (Entity, &Window, &ImguiViewportWindow, &ImguiViewportOwner),
        Without<PrimaryWindow>,
    >,
) -> bool {
    entity == primary_window.entity
        || viewport_windows
            .get(entity)
            .is_ok_and(|(_, _, marker, owner)| {
                owner.matches_window(marker) && marker.context_id() == primary_window.context_id
            })
}

fn add_mouse_viewport_event(io: &mut imgui::Io, viewport_id: Option<imgui::Id>) {
    if !io
        .config_flags()
        .contains(imgui::ConfigFlags::VIEWPORTS_ENABLE)
    {
        return;
    }
    if io
        .backend_flags()
        .contains(imgui::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT)
    {
        io.add_mouse_viewport_event(viewport_id.unwrap_or_default());
    }
}

fn mouse_pos_for_window(
    context: &imgui::Context,
    _window: ImguiInputWindow,
    local_pos: Vec2,
) -> [f32; 2] {
    let pos = [local_pos.x, local_pos.y];
    if !context
        .io()
        .config_flags()
        .contains(imgui::ConfigFlags::VIEWPORTS_ENABLE)
    {
        return pos;
    }

    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    {
        crate::viewport::window_client_logical_to_desktop(
            _window.entity,
            _window.scale_factor,
            _window.desktop_origin,
            pos,
        )
        .unwrap_or(pos)
    }

    #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
    {
        #[cfg(not(feature = "render"))]
        {
            let WindowPosition::At(window_pos) = _window.position else {
                return pos;
            };
            let scale_factor = positive_finite_or(_window.scale_factor, 1.0);
            return [
                pos[0] + window_pos.x as f32 / scale_factor,
                pos[1] + window_pos.y as f32 / scale_factor,
            ];
        }
        #[cfg(feature = "render")]
        pos
    }
}

fn positive_finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

/// Convert a Bevy mouse button into Dear ImGui's button space.
#[must_use]
pub(crate) fn map_bevy_mouse_button(button: BevyMouseButton) -> Option<imgui::MouseButton> {
    match button {
        BevyMouseButton::Left => Some(imgui::MouseButton::Left),
        BevyMouseButton::Right => Some(imgui::MouseButton::Right),
        BevyMouseButton::Middle => Some(imgui::MouseButton::Middle),
        BevyMouseButton::Back => Some(imgui::MouseButton::Extra1),
        BevyMouseButton::Forward => Some(imgui::MouseButton::Extra2),
        BevyMouseButton::Other(_) => None,
    }
}

/// Convert a Bevy physical key code into Dear ImGui's key space.
#[must_use]
pub(crate) fn map_bevy_key_code(key_code: KeyCode) -> Option<imgui::Key> {
    use KeyCode as B;
    use imgui::Key as I;

    match key_code {
        B::Backquote => Some(I::GraveAccent),
        B::Backslash => Some(I::Backslash),
        B::BracketLeft => Some(I::LeftBracket),
        B::BracketRight => Some(I::RightBracket),
        B::Comma => Some(I::Comma),
        B::Digit0 => Some(I::Key0),
        B::Digit1 => Some(I::Key1),
        B::Digit2 => Some(I::Key2),
        B::Digit3 => Some(I::Key3),
        B::Digit4 => Some(I::Key4),
        B::Digit5 => Some(I::Key5),
        B::Digit6 => Some(I::Key6),
        B::Digit7 => Some(I::Key7),
        B::Digit8 => Some(I::Key8),
        B::Digit9 => Some(I::Key9),
        B::Equal => Some(I::Equal),
        B::IntlBackslash | B::IntlRo | B::IntlYen => Some(I::Oem102),
        B::KeyA => Some(I::A),
        B::KeyB => Some(I::B),
        B::KeyC => Some(I::C),
        B::KeyD => Some(I::D),
        B::KeyE => Some(I::E),
        B::KeyF => Some(I::F),
        B::KeyG => Some(I::G),
        B::KeyH => Some(I::H),
        B::KeyI => Some(I::I),
        B::KeyJ => Some(I::J),
        B::KeyK => Some(I::K),
        B::KeyL => Some(I::L),
        B::KeyM => Some(I::M),
        B::KeyN => Some(I::N),
        B::KeyO => Some(I::O),
        B::KeyP => Some(I::P),
        B::KeyQ => Some(I::Q),
        B::KeyR => Some(I::R),
        B::KeyS => Some(I::S),
        B::KeyT => Some(I::T),
        B::KeyU => Some(I::U),
        B::KeyV => Some(I::V),
        B::KeyW => Some(I::W),
        B::KeyX => Some(I::X),
        B::KeyY => Some(I::Y),
        B::KeyZ => Some(I::Z),
        B::Minus => Some(I::Minus),
        B::Period => Some(I::Period),
        B::Quote => Some(I::Apostrophe),
        B::Semicolon => Some(I::Semicolon),
        B::Slash => Some(I::Slash),
        B::AltLeft => Some(I::LeftAlt),
        B::AltRight => Some(I::RightAlt),
        B::Backspace | B::NumpadBackspace => Some(I::Backspace),
        B::CapsLock => Some(I::CapsLock),
        B::ContextMenu => Some(I::Menu),
        B::ControlLeft => Some(I::LeftCtrl),
        B::ControlRight => Some(I::RightCtrl),
        B::Enter => Some(I::Enter),
        B::SuperLeft | B::Meta => Some(I::LeftSuper),
        B::SuperRight => Some(I::RightSuper),
        B::ShiftLeft => Some(I::LeftShift),
        B::ShiftRight => Some(I::RightShift),
        B::Space => Some(I::Space),
        B::Tab => Some(I::Tab),
        B::Delete => Some(I::Delete),
        B::End => Some(I::End),
        B::Home => Some(I::Home),
        B::Insert => Some(I::Insert),
        B::PageDown => Some(I::PageDown),
        B::PageUp => Some(I::PageUp),
        B::ArrowDown => Some(I::DownArrow),
        B::ArrowLeft => Some(I::LeftArrow),
        B::ArrowRight => Some(I::RightArrow),
        B::ArrowUp => Some(I::UpArrow),
        B::NumLock => Some(I::NumLock),
        B::Numpad0 => Some(I::Keypad0),
        B::Numpad1 => Some(I::Keypad1),
        B::Numpad2 => Some(I::Keypad2),
        B::Numpad3 => Some(I::Keypad3),
        B::Numpad4 => Some(I::Keypad4),
        B::Numpad5 => Some(I::Keypad5),
        B::Numpad6 => Some(I::Keypad6),
        B::Numpad7 => Some(I::Keypad7),
        B::Numpad8 => Some(I::Keypad8),
        B::Numpad9 => Some(I::Keypad9),
        B::NumpadAdd => Some(I::KeypadAdd),
        B::NumpadDecimal | B::NumpadComma => Some(I::KeypadDecimal),
        B::NumpadDivide => Some(I::KeypadDivide),
        B::NumpadEnter => Some(I::KeypadEnter),
        B::NumpadEqual => Some(I::KeypadEqual),
        B::NumpadMultiply | B::NumpadStar => Some(I::KeypadMultiply),
        B::NumpadSubtract => Some(I::KeypadSubtract),
        B::Escape => Some(I::Escape),
        B::PrintScreen => Some(I::PrintScreen),
        B::ScrollLock => Some(I::ScrollLock),
        B::Pause => Some(I::Pause),
        B::F1 => Some(I::F1),
        B::F2 => Some(I::F2),
        B::F3 => Some(I::F3),
        B::F4 => Some(I::F4),
        B::F5 => Some(I::F5),
        B::F6 => Some(I::F6),
        B::F7 => Some(I::F7),
        B::F8 => Some(I::F8),
        B::F9 => Some(I::F9),
        B::F10 => Some(I::F10),
        B::F11 => Some(I::F11),
        B::F12 => Some(I::F12),
        _ => None,
    }
}

#[cfg(not(feature = "render"))]
fn sync_window_metrics(context: &mut imgui::Context, window: &Window) {
    let io = context.io_mut();
    io.set_display_size(sanitized_window_display_size(window));
    io.set_display_framebuffer_scale(sanitized_window_framebuffer_scale(window));
}

#[cfg(not(feature = "render"))]
fn set_framebuffer_scale(context: &mut imgui::Context, scale_factor: f32) {
    context
        .io_mut()
        .set_display_framebuffer_scale([scale_factor, scale_factor]);
}

pub(crate) fn sanitized_window_display_size(window: &Window) -> [f32; 2] {
    finite_non_negative_size([window.width(), window.height()])
}

pub(crate) fn sanitized_window_framebuffer_scale(window: &Window) -> [f32; 2] {
    let scale_factor = positive_finite_or(window.scale_factor(), 1.0);
    [scale_factor, scale_factor]
}

fn finite_non_negative_size(size: [f32; 2]) -> [f32; 2] {
    [
        if size[0].is_finite() && size[0] >= 0.0 {
            size[0]
        } else {
            0.0
        },
        if size[1].is_finite() && size[1] >= 0.0 {
            size[1]
        } else {
            0.0
        },
    ]
}

#[cfg(not(feature = "render"))]
fn sync_initial_focus(
    context: &mut imgui::Context,
    state: &mut ImguiInputState,
    window: ImguiInputWindow,
    focused: bool,
) {
    if state.primary_window_focused == Some(focused) {
        return;
    }
    if !focused
        && state
            .focused_window
            .is_some_and(|entity| entity != window.entity)
    {
        if window.is_primary {
            state.primary_window_focused = Some(false);
        }
        return;
    }
    apply_focus_event(context, state, window, focused);
}

#[cfg(not(feature = "render"))]
fn prune_stale_window_state(
    context: &mut imgui::Context,
    state: &mut ImguiInputState,
    primary_window: ImguiInputWindow,
    viewport_windows: &Query<
        (Entity, &Window, &ImguiViewportWindow, &ImguiViewportOwner),
        Without<PrimaryWindow>,
    >,
    primary_focused: bool,
) {
    let focused_was_stale = state
        .focused_window
        .is_some_and(|entity| !is_mapped_imgui_window(entity, primary_window, viewport_windows));
    if focused_was_stale {
        state.focused_window = primary_focused.then_some(primary_window.entity);
        if !primary_focused {
            state.primary_window_focused = Some(false);
            context.io_mut().add_focus_event(false);
            release_sticky_input(context, state);
        }
    }

    if state
        .mouse_hovered_window
        .is_some_and(|entity| !is_mapped_imgui_window(entity, primary_window, viewport_windows))
    {
        state.mouse_hovered_window = None;
        let io = context.io_mut();
        io.add_mouse_source_event(imgui::MouseSource::Mouse);
        add_mouse_viewport_event(io, None);
        io.add_mouse_pos_event(INVALID_MOUSE_POS);
        clear_mouse_hovered_viewport(io);
    }

    if state
        .active_touch_window
        .is_some_and(|entity| !is_mapped_imgui_window(entity, primary_window, viewport_windows))
    {
        state.active_touch_id = None;
        state.active_touch_window = None;
        let io = context.io_mut();
        io.add_mouse_source_event(imgui::MouseSource::TouchScreen);
        io.add_mouse_pos_event(INVALID_MOUSE_POS);
        io.add_mouse_button_event(imgui::MouseButton::Left, false);
        add_mouse_viewport_event(io, None);
        clear_mouse_hovered_viewport(io);
    }
}

#[cfg(not(feature = "render"))]
fn release_input_for_missing_primary_window(
    context: &mut imgui::Context,
    state: &mut ImguiInputState,
) {
    let had_focus = state.primary_window_focused != Some(false) || state.focused_window.is_some();
    let had_mouse_window = state.mouse_hovered_window.is_some();
    let had_pointer_input = had_mouse_window
        || !state.pressed_mouse_buttons.is_empty()
        || state.active_touch_id.is_some();

    if had_focus {
        context.io_mut().add_focus_event(false);
    }
    state.primary_window_focused = Some(false);
    state.focused_window = None;
    state.ime_enabled = false;

    if state.has_sticky_input() {
        release_sticky_input(context, state);
    }

    if had_pointer_input {
        state.mouse_hovered_window = None;
        let io = context.io_mut();
        io.add_mouse_source_event(imgui::MouseSource::Mouse);
        add_mouse_viewport_event(io, None);
        io.add_mouse_pos_event(INVALID_MOUSE_POS);
        clear_mouse_hovered_viewport(io);
    }
}

fn clear_mouse_hovered_viewport(io: &mut imgui::Io) {
    io.set_mouse_hovered_viewport(imgui::Id::from(0));
}

#[cfg(not(feature = "render"))]
fn apply_focus_event(
    context: &mut imgui::Context,
    state: &mut ImguiInputState,
    window: ImguiInputWindow,
    focused: bool,
) {
    context.io_mut().add_focus_event(focused);
    if window.is_primary {
        state.primary_window_focused = Some(focused);
    }
    state.focused_window = focused.then_some(window.entity);
    if !focused {
        release_sticky_input(context, state);
    }
}

#[cfg(not(feature = "render"))]
fn apply_focus_events(
    context: &mut imgui::Context,
    state: &mut ImguiInputState,
    events: &[(ImguiInputWindow, bool)],
) {
    if events.is_empty() {
        return;
    }

    let was_focused = state.focused_window.is_some();
    let mut focused_window = state.focused_window;
    for &(window, focused) in events {
        if window.is_primary {
            state.primary_window_focused = Some(focused);
        }
        if focused {
            focused_window = Some(window.entity);
        } else if focused_window == Some(window.entity) {
            focused_window = None;
        }
    }

    // Dear ImGui focus is context-wide: moving focus between mapped OS windows must not look like
    // an application blur, otherwise held keys/buttons are released during intra-app focus changes.
    match (was_focused, focused_window.is_some()) {
        (false, true) => context.io_mut().add_focus_event(true),
        (true, false) => {
            context.io_mut().add_focus_event(false);
            release_sticky_input(context, state);
        }
        _ => {}
    }
    state.focused_window = focused_window;
}

#[cfg(not(feature = "render"))]
fn apply_keyboard_input(
    context: &mut imgui::Context,
    state: &mut ImguiInputState,
    event: &KeyboardInput,
) {
    let pressed = event.state == ButtonState::Pressed;

    if pressed && let Some(text) = &event.text {
        add_keyboard_text(context.io_mut(), text);
    }

    if let Some(key) = map_bevy_key_code(event.key_code) {
        if pressed {
            state.pressed_keys.insert(key);
        } else {
            state.pressed_keys.remove(&key);
        }
        let io = context.io_mut();
        sync_modifier_events(io, state);
        io.add_key_event(key, pressed);
    }
}

#[cfg(not(feature = "render"))]
fn sync_modifier_events(io: &mut imgui::Io, state: &ImguiInputState) {
    apply_modifier_events(io, modifier_state(&state.pressed_keys));
}

#[cfg(not(feature = "render"))]
impl ImguiInputState {
    fn has_sticky_input(&self) -> bool {
        !self.pressed_keys.is_empty()
            || !self.pressed_mouse_buttons.is_empty()
            || self.active_touch_id.is_some()
    }
}

#[cfg(not(feature = "render"))]
fn apply_touch_input(
    context: &mut imgui::Context,
    state: &mut ImguiInputState,
    event: &TouchInput,
    window: ImguiInputWindow,
) {
    match event.phase {
        TouchPhase::Started => {
            if state.active_touch_id.is_none() {
                state.active_touch_id = Some(event.id);
                state.active_touch_window = Some(window.entity);
                let mouse_pos = mouse_pos_for_window(context, window, event.position);
                let io = context.io_mut();
                io.add_mouse_source_event(imgui::MouseSource::TouchScreen);
                add_mouse_viewport_event(io, Some(window.viewport_id));
                io.add_mouse_pos_event(mouse_pos);
                io.add_mouse_button_event(imgui::MouseButton::Left, true);
            }
        }
        TouchPhase::Moved => {
            if state.active_touch_id == Some(event.id) {
                state.active_touch_window = Some(window.entity);
                let mouse_pos = mouse_pos_for_window(context, window, event.position);
                let io = context.io_mut();
                io.add_mouse_source_event(imgui::MouseSource::TouchScreen);
                add_mouse_viewport_event(io, Some(window.viewport_id));
                io.add_mouse_pos_event(mouse_pos);
            }
        }
        TouchPhase::Ended | TouchPhase::Canceled => {
            if state.active_touch_id == Some(event.id) {
                state.active_touch_id = None;
                state.active_touch_window = None;
                let mouse_pos = mouse_pos_for_window(context, window, event.position);
                let io = context.io_mut();
                io.add_mouse_source_event(imgui::MouseSource::TouchScreen);
                add_mouse_viewport_event(io, Some(window.viewport_id));
                io.add_mouse_pos_event(mouse_pos);
                io.add_mouse_button_event(imgui::MouseButton::Left, false);
            }
        }
    }
}

#[cfg(not(feature = "render"))]
fn apply_ime_event(context: &mut imgui::Context, state: &mut ImguiInputState, event: &Ime) {
    match event {
        Ime::Commit { value, .. } => {
            add_ime_text(context.io_mut(), value);
        }
        Ime::Enabled { .. } => {
            state.ime_enabled = true;
        }
        Ime::Disabled { .. } => {
            state.ime_enabled = false;
        }
        Ime::Preedit { .. } => {}
    }
}

fn normalize_wheel(unit: MouseScrollUnit, x: f32, y: f32) -> [f32; 2] {
    match unit {
        MouseScrollUnit::Line => [x, y],
        MouseScrollUnit::Pixel => [pixel_wheel_step(x), pixel_wheel_step(y)],
    }
}

fn pixel_wheel_step(value: f32) -> f32 {
    match value.partial_cmp(&0.0) {
        Some(std::cmp::Ordering::Greater) => 1.0,
        Some(std::cmp::Ordering::Less) => -1.0,
        _ => 0.0,
    }
}

fn add_keyboard_text(io: &mut imgui::Io, text: &str) {
    for character in text.chars().filter(|character| *character != '\u{7f}') {
        io.add_input_character(character);
    }
}

fn add_ime_text(io: &mut imgui::Io, text: &str) {
    for character in text.chars().filter(|character| !character.is_control()) {
        io.add_input_character(character);
    }
}

fn modifier_state(keys: &HashSet<imgui::Key>) -> (bool, bool, bool, bool) {
    (
        keys.contains(&imgui::Key::LeftCtrl) || keys.contains(&imgui::Key::RightCtrl),
        keys.contains(&imgui::Key::LeftShift) || keys.contains(&imgui::Key::RightShift),
        keys.contains(&imgui::Key::LeftAlt) || keys.contains(&imgui::Key::RightAlt),
        keys.contains(&imgui::Key::LeftSuper) || keys.contains(&imgui::Key::RightSuper),
    )
}

fn apply_modifier_events(io: &mut imgui::Io, modifiers: (bool, bool, bool, bool)) {
    io.add_key_event(imgui::Key::ModCtrl, modifiers.0);
    io.add_key_event(imgui::Key::ModShift, modifiers.1);
    io.add_key_event(imgui::Key::ModAlt, modifiers.2);
    io.add_key_event(imgui::Key::ModSuper, modifiers.3);
}

fn release_sticky_keys_and_buttons(
    io: &mut imgui::Io,
    keys: &mut HashSet<imgui::Key>,
    mouse_buttons: &mut HashSet<imgui::MouseButton>,
) {
    for key in keys.drain() {
        io.add_key_event(key, false);
    }
    apply_modifier_events(io, (false, false, false, false));
    for button in mouse_buttons.drain() {
        io.add_mouse_button_event(button, false);
    }
}

#[cfg(not(feature = "render"))]
fn release_sticky_input(context: &mut imgui::Context, state: &mut ImguiInputState) {
    let io = context.io_mut();
    release_sticky_keys_and_buttons(
        io,
        &mut state.pressed_keys,
        &mut state.pressed_mouse_buttons,
    );

    if state.active_touch_id.take().is_some() {
        state.active_touch_window = None;
        io.add_mouse_source_event(imgui::MouseSource::TouchScreen);
        io.add_mouse_button_event(imgui::MouseButton::Left, false);
    }
}

#[cfg(test)]
#[path = "tests/input.rs"]
mod input_tests;

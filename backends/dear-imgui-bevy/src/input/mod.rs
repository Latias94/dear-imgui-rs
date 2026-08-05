//! Routed window input mapping for the Bevy backend.
//!
//! This module maps Bevy windows and explicit logical input routes into their owning Dear ImGui
//! Contexts. It translates Bevy's window/input messages into Dear ImGui IO events without consuming
//! or rewriting Bevy's messages. Gameplay systems should use Dear ImGui's capture flags as policy
//! hints instead of expecting this backend to stop Bevy input propagation.

mod capture;
mod capture_api;
mod common;
#[cfg(feature = "render")]
mod event_ingest;
mod events;
mod feedback;
#[cfg(not(feature = "render"))]
mod primary;
#[cfg(feature = "render")]
mod route;
#[cfg(feature = "render")]
mod routing;
mod state;

pub use capture_api::{
    ImguiInputCapture, ImguiInputCaptureState, imgui_context_wants_keyboard_input,
    imgui_context_wants_pointer_input, imgui_context_wants_text_input,
    imgui_primary_wants_keyboard_input, imgui_primary_wants_pointer_input,
    imgui_primary_wants_text_input, imgui_wants_any_input, imgui_wants_keyboard_input,
    imgui_wants_pointer_input, imgui_wants_pointer_input_unless_popup_close,
    imgui_wants_text_input, imgui_window_wants_keyboard_input, imgui_window_wants_pointer_input,
    imgui_window_wants_text_input,
};
#[cfg(test)]
pub(crate) use common::map_bevy_key_code;
pub(crate) use common::sanitized_window_framebuffer_scale;
#[cfg(feature = "render")]
use common::{INVALID_MOUSE_POS, modifier_state, mouse_pos_for_window};
#[cfg(all(test, feature = "render"))]
use event_ingest::{OrderedPointerEvent, append_typed_pointer_event};
#[cfg(all(
    test,
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
use event_ingest::{RawWindowPointerEvent, order_raw_pointer_events};
pub(crate) use feedback::map_imgui_mouse_cursor;
#[cfg(not(feature = "render"))]
pub(crate) use primary::primary_window_input_system;
#[cfg(feature = "render")]
pub(crate) use route::{ImguiContextInputMetrics, ImguiInputFrameMetrics};
#[cfg(feature = "render")]
use routing::routed_window_input_system;
pub(crate) use state::ImguiInputState;
#[cfg(feature = "render")]
use state::ImguiInputWindow;
#[cfg(all(test, feature = "render"))]
pub(crate) use state::ImguiInputWindowState;

use bevy_app::{App, PreUpdate};
#[cfg(feature = "render")]
use bevy_ecs::entity::Entity;
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy_input::keyboard::{KeyboardFocusLost, KeyboardInput};
use bevy_input::mouse::{MouseButtonInput, MouseWheel};
use bevy_input::touch::TouchInput;
use bevy_window::{
    CursorEntered, CursorLeft, CursorMoved, Ime, WindowBackendScaleFactorChanged, WindowFocused,
    WindowResized, WindowScaleFactorChanged,
};
#[cfg(feature = "render")]
use bevy_window::{PrimaryWindow, Window};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_winit::RawWinitWindowEvent;

#[cfg(feature = "render")]
use crate::ImguiViewportWindow;
#[cfg(feature = "render")]
use crate::viewport::ImguiViewportOwner;

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

#[cfg(test)]
#[path = "tests/input.rs"]
mod input_tests;

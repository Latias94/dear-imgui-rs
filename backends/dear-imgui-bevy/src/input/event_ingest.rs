#[cfg(feature = "render")]
use std::collections::HashMap;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use std::collections::{HashSet, VecDeque};

use bevy_ecs::entity::Entity;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_ecs::prelude::{MessageReader, Query};
use bevy_input::ButtonState;
use bevy_input::mouse::MouseButton as BevyMouseButton;
use bevy_math::Vec2;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_window::{CursorMoved, Window};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_winit::{RawWinitWindowEvent, WINIT_WINDOWS};

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use super::RoutedInputWindowComponents;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use super::common::positive_finite_or;

#[cfg(feature = "render")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum OrderedPointerEvent {
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
pub(super) enum PointerEventIdentity {
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
    pub(super) const fn identity(self) -> PointerEventIdentity {
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
pub(super) enum RawWindowPointerEvent {
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
pub(super) fn collect_raw_winit_pointer_events(
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
pub(super) fn order_raw_pointer_events(
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
pub(super) fn append_typed_pointer_event(
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

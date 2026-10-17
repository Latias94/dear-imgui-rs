//! Owned event handoff for SDL3 callback-mode applications.
//!
//! SDL may invoke `SDL_AppEvent` away from the main thread while Dear ImGui and SDL video work
//! remain main-thread-bound. This module copies the exact SDL payloads consumed by the official
//! backend before the callback returns and defers processing to the main thread.

use std::collections::{HashMap, VecDeque};
use std::ffi::{CStr, CString};
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, MutexGuard};

use dear_imgui_rs::NativeWindowTarget;

use sdl3::event::{DisplayEvent, Event, WindowEvent};
use sdl3_sys::events::{
    SDL_DisplayEvent, SDL_EVENT_DISPLAY_ADDED, SDL_EVENT_DISPLAY_CONTENT_SCALE_CHANGED,
    SDL_EVENT_DISPLAY_MOVED, SDL_EVENT_DISPLAY_ORIENTATION, SDL_EVENT_DISPLAY_REMOVED,
    SDL_EVENT_DISPLAY_USABLE_BOUNDS_CHANGED, SDL_EVENT_DROP_BEGIN, SDL_EVENT_DROP_COMPLETE,
    SDL_EVENT_DROP_FILE, SDL_EVENT_DROP_POSITION, SDL_EVENT_DROP_TEXT, SDL_EVENT_GAMEPAD_ADDED,
    SDL_EVENT_GAMEPAD_REMOVED, SDL_EVENT_KEY_DOWN, SDL_EVENT_KEY_UP, SDL_EVENT_MOUSE_BUTTON_DOWN,
    SDL_EVENT_MOUSE_BUTTON_UP, SDL_EVENT_MOUSE_MOTION, SDL_EVENT_MOUSE_WHEEL, SDL_EVENT_QUIT,
    SDL_EVENT_TEXT_INPUT, SDL_EVENT_WINDOW_CLOSE_REQUESTED, SDL_EVENT_WINDOW_FOCUS_GAINED,
    SDL_EVENT_WINDOW_FOCUS_LOST, SDL_EVENT_WINDOW_MOUSE_ENTER, SDL_EVENT_WINDOW_MOUSE_LEAVE,
    SDL_EVENT_WINDOW_MOVED, SDL_EVENT_WINDOW_PIXEL_SIZE_CHANGED, SDL_EVENT_WINDOW_RESIZED,
    SDL_Event, SDL_GamepadDeviceEvent, SDL_KeyboardEvent, SDL_MouseButtonEvent,
    SDL_MouseMotionEvent, SDL_MouseWheelEvent, SDL_TextInputEvent, SDL_WindowEvent,
};
use sdl3_sys::keycode::SDLK_ESCAPE;
#[cfg(test)]
use sdl3_sys::video::SDL_DisplayID;
use sdl3_sys::video::SDL_WindowID;

const DEFAULT_CALLBACK_EVENT_CAPACITY: usize = 1024;

/// Owned application payload from SDL's desktop drag-and-drop callbacks.
#[derive(Clone, Debug, PartialEq)]
pub struct Sdl3DropEvent {
    pub timestamp: u64,
    pub window_id: u32,
    /// SDL client coordinates when supplied; begin events and unavailable values remain `None`.
    pub position: Option<[f32; 2]>,
    pub source: Option<String>,
    pub payload: Sdl3DropPayload,
    /// Registration observed by an owner-bound handoff when the callback copied the payload.
    /// This is historical metadata, not proof that this registration generated the OS event.
    pub observed_target: Option<NativeWindowTarget>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Sdl3DropPayload {
    Begin,
    File(String),
    Text(String),
    Position,
    /// SDL ends a drop sequence, including a hover that supplied no file or text payload.
    Complete,
}

/// Current association of a previously observed drop registration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Sdl3DropTarget {
    Current(NativeWindowTarget),
    Retired(NativeWindowTarget),
    /// No registration was safely observed; numeric SDL IDs alone do not establish ownership.
    Unowned,
}

#[derive(Clone, Copy)]
struct PublishedWindowTarget {
    target: NativeWindowTarget,
    published_at: u64,
}

/// Thread-safe metadata publication only. The runtime's native registry remains authoritative.
#[derive(Default)]
pub(crate) struct CallbackWindowTargets(Mutex<HashMap<u32, PublishedWindowTarget>>);

impl CallbackWindowTargets {
    pub(crate) fn publish(&self, window_id: u32, target: NativeWindowTarget) {
        let mut targets = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let published_at = targets
            .get(&window_id)
            .filter(|entry| entry.target.instance_id == target.instance_id)
            .map(|entry| entry.published_at)
            .unwrap_or_else(|| unsafe { sdl3_sys::timer::SDL_GetTicksNS() });
        targets.insert(
            window_id,
            PublishedWindowTarget {
                target,
                published_at,
            },
        );
    }

    pub(crate) fn retire(&self, window_id: u32, target: NativeWindowTarget) {
        let mut targets = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if targets
            .get(&window_id)
            .is_some_and(|entry| entry.target.instance_id == target.instance_id)
        {
            targets.remove(&window_id);
        }
    }

    fn observe(&self, window_id: u32, timestamp: u64) -> Option<NativeWindowTarget> {
        let targets = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let entry = targets.get(&window_id)?;
        (timestamp != 0 && timestamp >= entry.published_at).then_some(entry.target)
    }
}

fn dispose_panic_payload_without_unwinding(mut payload: Box<dyn std::any::Any + Send>) {
    const MAX_DROP_ATTEMPTS: usize = 8;

    for _ in 0..MAX_DROP_ATTEMPTS {
        match catch_unwind(AssertUnwindSafe(|| drop(payload))) {
            Ok(()) => return,
            Err(next_payload) => payload = next_payload,
        }
    }

    // A hostile Drop implementation can create an unbounded chain of new panic payloads. Leaking
    // only the final payload is the bounded fallback that keeps unwinding out of the C callback.
    std::mem::forget(payload);
}

/// A deferred failure observed by [`Sdl3CallbackEventHandoff`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum Sdl3CallbackEventHandoffError {
    /// Event ownership code panicked inside the callback boundary.
    #[error("SDL3 callback event ownership panicked; the event was not queued")]
    CallbackPanicked,
    /// A previous panic poisoned the event queue.
    #[error("SDL3 callback event queue was poisoned; queued events were recovered")]
    QueuePoisoned,
    /// The bounded callback queue could not retain every event.
    #[error("SDL3 callback event queue overflowed; {dropped_events} event(s) were dropped")]
    QueueOverflow {
        /// Number of events dropped since the previous overflow report.
        dropped_events: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum CoalescingKey {
    MouseMotion(u32),
    WindowMoved(u32),
    WindowResized(u32),
    WindowPixelSizeChanged(u32),
    DisplayOrientation(u32),
    DisplayMoved(u32),
    DisplayContentScaleChanged(u32),
    DisplayUsableBoundsChanged(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ApplicationEvent {
    Quit,
    Escape,
    WindowCloseRequested { window_id: u32 },
    WindowPixelSizeChanged { window_id: u32 },
    Other,
}

impl ApplicationEvent {
    unsafe fn from_callback_raw(raw: &SDL_Event) -> Self {
        let event_type = raw.event_type();
        if event_type == SDL_EVENT_QUIT {
            Self::Quit
        } else if event_type == SDL_EVENT_KEY_DOWN && unsafe { raw.key.key } == SDLK_ESCAPE {
            Self::Escape
        } else if event_type == SDL_EVENT_WINDOW_CLOSE_REQUESTED {
            Self::WindowCloseRequested {
                window_id: unsafe { raw.window.windowID.0 },
            }
        } else if event_type == SDL_EVENT_WINDOW_PIXEL_SIZE_CHANGED {
            Self::WindowPixelSizeChanged {
                window_id: unsafe { raw.window.windowID.0 },
            }
        } else {
            Self::Other
        }
    }
}

enum ImGuiEvent {
    MouseMotion(SDL_MouseMotionEvent),
    MouseWheel(SDL_MouseWheelEvent),
    MouseButton(SDL_MouseButtonEvent),
    TextInput {
        timestamp: u64,
        window_id: SDL_WindowID,
        text: CString,
    },
    Keyboard(SDL_KeyboardEvent),
    Display(SDL_DisplayEvent),
    Window(SDL_WindowEvent),
    GamepadDevice(SDL_GamepadDeviceEvent),
}

/// An SDL callback event whose Dear ImGui payload is fully owned.
///
/// Desktop drops retain owned application payloads without replaying them through Dear ImGui.
/// Other pointer-bearing variants the official backend does not consume remain inert summaries.
pub struct Sdl3CallbackEvent {
    application: ApplicationEvent,
    imgui: Option<ImGuiEvent>,
    drop: Option<Sdl3DropEvent>,
}

impl fmt::Debug for Sdl3CallbackEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Sdl3CallbackEvent")
            .field("application", &self.application)
            .field("has_imgui_payload", &self.imgui.is_some())
            .field("drop", &self.drop)
            .finish()
    }
}

impl Sdl3CallbackEvent {
    pub(crate) fn from_owned_event(event: &Event) -> Result<Option<Self>, crate::Sdl3BackendError> {
        match event {
            Event::TextInput {
                timestamp,
                window_id,
                text,
            } => {
                let text = CString::new(text.as_bytes())
                    .map_err(|_| crate::Sdl3BackendError::TextInputContainsNul)?;
                Ok(Some(Self {
                    application: ApplicationEvent::Other,
                    imgui: Some(ImGuiEvent::TextInput {
                        timestamp: *timestamp,
                        window_id: SDL_WindowID(*window_id),
                        text,
                    }),
                    drop: None,
                }))
            }
            Event::Window {
                win_event: WindowEvent::None,
                ..
            }
            | Event::Display {
                display_event: DisplayEvent::None,
                ..
            } => Ok(None),
            Event::DropFile {
                timestamp,
                window_id,
                filename,
            }
            | Event::DropText {
                timestamp,
                window_id,
                filename,
            } => Ok(Some(Self {
                application: ApplicationEvent::Other,
                imgui: None,
                drop: Some(Sdl3DropEvent {
                    timestamp: *timestamp,
                    window_id: *window_id,
                    position: None,
                    source: None,
                    observed_target: None,
                    payload: if matches!(event, Event::DropFile { .. }) {
                        Sdl3DropPayload::File(filename.clone())
                    } else {
                        Sdl3DropPayload::Text(filename.clone())
                    },
                }),
            })),
            Event::DropBegin {
                timestamp,
                window_id,
            }
            | Event::DropComplete {
                timestamp,
                window_id,
            } => Ok(Some(Self {
                application: ApplicationEvent::Other,
                imgui: None,
                drop: Some(Sdl3DropEvent {
                    timestamp: *timestamp,
                    window_id: *window_id,
                    position: None,
                    source: None,
                    observed_target: None,
                    payload: if matches!(event, Event::DropBegin { .. }) {
                        Sdl3DropPayload::Begin
                    } else {
                        Sdl3DropPayload::Complete
                    },
                }),
            })),
            Event::Unknown { timestamp, type_ }
                if *type_ == SDL_EVENT_DISPLAY_USABLE_BOUNDS_CHANGED.0 =>
            {
                Ok(Some(Self {
                    application: ApplicationEvent::Other,
                    imgui: Some(ImGuiEvent::Display(SDL_DisplayEvent {
                        r#type: SDL_EVENT_DISPLAY_USABLE_BOUNDS_CHANGED,
                        timestamp: *timestamp,
                        ..Default::default()
                    })),
                    drop: None,
                }))
            }
            Event::Window { .. }
            | Event::KeyDown { .. }
            | Event::KeyUp { .. }
            | Event::MouseMotion { .. }
            | Event::MouseButtonDown { .. }
            | Event::MouseButtonUp { .. }
            | Event::MouseWheel { .. }
            | Event::Display { .. }
            | Event::GamepadAdded { .. }
            | Event::GamepadRemoved { .. } => Ok(event.to_ll().as_ref().map(|raw| {
                // SAFETY: SDL's safe Event conversion selects the active raw union member, and
                // this constructor copies every payload before the temporary raw value is dropped.
                unsafe { Self::from_callback_raw(raw) }
            })),
            _ => Ok(None),
        }
    }

    unsafe fn from_callback_raw(raw: &SDL_Event) -> Self {
        let application = unsafe { ApplicationEvent::from_callback_raw(raw) };
        let imgui = match raw.event_type() {
            SDL_EVENT_MOUSE_MOTION => Some(ImGuiEvent::MouseMotion(unsafe { raw.motion })),
            SDL_EVENT_MOUSE_WHEEL => Some(ImGuiEvent::MouseWheel(unsafe { raw.wheel })),
            SDL_EVENT_MOUSE_BUTTON_DOWN | SDL_EVENT_MOUSE_BUTTON_UP => {
                Some(ImGuiEvent::MouseButton(unsafe { raw.button }))
            }
            SDL_EVENT_TEXT_INPUT => {
                let event = unsafe { raw.text };
                Some(ImGuiEvent::TextInput {
                    timestamp: event.timestamp,
                    window_id: event.windowID,
                    text: unsafe { CStr::from_ptr(event.text) }.to_owned(),
                })
            }
            SDL_EVENT_KEY_DOWN | SDL_EVENT_KEY_UP => Some(ImGuiEvent::Keyboard(unsafe { raw.key })),
            SDL_EVENT_DISPLAY_ORIENTATION
            | SDL_EVENT_DISPLAY_ADDED
            | SDL_EVENT_DISPLAY_REMOVED
            | SDL_EVENT_DISPLAY_MOVED
            | SDL_EVENT_DISPLAY_CONTENT_SCALE_CHANGED
            | SDL_EVENT_DISPLAY_USABLE_BOUNDS_CHANGED => {
                Some(ImGuiEvent::Display(unsafe { raw.display }))
            }
            SDL_EVENT_WINDOW_MOUSE_ENTER
            | SDL_EVENT_WINDOW_MOUSE_LEAVE
            | SDL_EVENT_WINDOW_FOCUS_GAINED
            | SDL_EVENT_WINDOW_FOCUS_LOST
            | SDL_EVENT_WINDOW_CLOSE_REQUESTED
            | SDL_EVENT_WINDOW_MOVED
            | SDL_EVENT_WINDOW_RESIZED => Some(ImGuiEvent::Window(unsafe { raw.window })),
            SDL_EVENT_GAMEPAD_ADDED | SDL_EVENT_GAMEPAD_REMOVED => {
                Some(ImGuiEvent::GamepadDevice(unsafe { raw.gdevice }))
            }
            _ => None,
        };

        let drop = match raw.event_type() {
            SDL_EVENT_DROP_BEGIN
            | SDL_EVENT_DROP_FILE
            | SDL_EVENT_DROP_TEXT
            | SDL_EVENT_DROP_POSITION
            | SDL_EVENT_DROP_COMPLETE => {
                let event = unsafe { raw.drop };
                let copy = |pointer: *const std::ffi::c_char| {
                    if pointer.is_null() {
                        None
                    } else {
                        Some(
                            unsafe { CStr::from_ptr(pointer) }
                                .to_string_lossy()
                                .into_owned(),
                        )
                    }
                };
                let payload = match event.r#type {
                    SDL_EVENT_DROP_BEGIN => Sdl3DropPayload::Begin,
                    SDL_EVENT_DROP_FILE => {
                        Sdl3DropPayload::File(copy(event.data).unwrap_or_default())
                    }
                    SDL_EVENT_DROP_TEXT => {
                        Sdl3DropPayload::Text(copy(event.data).unwrap_or_default())
                    }
                    SDL_EVENT_DROP_POSITION => Sdl3DropPayload::Position,
                    _ => Sdl3DropPayload::Complete,
                };
                Some(Sdl3DropEvent {
                    timestamp: event.timestamp,
                    window_id: event.windowID.0,
                    position: (event.r#type != SDL_EVENT_DROP_BEGIN
                        && event.x.is_finite()
                        && event.y.is_finite())
                    .then_some([event.x, event.y]),
                    source: copy(event.source),
                    payload,
                    observed_target: None,
                })
            }
            _ => None,
        };
        Self {
            application,
            imgui,
            drop,
        }
    }

    fn coalescing_key(&self) -> Option<CoalescingKey> {
        match &self.imgui {
            Some(ImGuiEvent::MouseMotion(event)) => {
                Some(CoalescingKey::MouseMotion(event.windowID.0))
            }
            Some(ImGuiEvent::Window(event)) => match event.r#type {
                SDL_EVENT_WINDOW_MOVED => Some(CoalescingKey::WindowMoved(event.windowID.0)),
                SDL_EVENT_WINDOW_RESIZED => Some(CoalescingKey::WindowResized(event.windowID.0)),
                _ => None,
            },
            Some(ImGuiEvent::Display(event)) => match event.r#type {
                SDL_EVENT_DISPLAY_ORIENTATION => {
                    Some(CoalescingKey::DisplayOrientation(event.displayID.0))
                }
                SDL_EVENT_DISPLAY_MOVED => Some(CoalescingKey::DisplayMoved(event.displayID.0)),
                SDL_EVENT_DISPLAY_CONTENT_SCALE_CHANGED => {
                    Some(CoalescingKey::DisplayContentScaleChanged(event.displayID.0))
                }
                SDL_EVENT_DISPLAY_USABLE_BOUNDS_CHANGED => {
                    Some(CoalescingKey::DisplayUsableBoundsChanged(event.displayID.0))
                }
                _ => None,
            },
            Some(
                ImGuiEvent::MouseWheel(_)
                | ImGuiEvent::MouseButton(_)
                | ImGuiEvent::TextInput { .. }
                | ImGuiEvent::Keyboard(_)
                | ImGuiEvent::GamepadDevice(_),
            ) => None,
            None => match self.application {
                ApplicationEvent::WindowPixelSizeChanged { window_id } => {
                    Some(CoalescingKey::WindowPixelSizeChanged(window_id))
                }
                ApplicationEvent::Quit
                | ApplicationEvent::Escape
                | ApplicationEvent::WindowCloseRequested { .. }
                | ApplicationEvent::Other => None,
            },
        }
    }

    pub(crate) fn with_raw_event<R>(&self, callback: impl FnOnce(Option<&SDL_Event>) -> R) -> R {
        let raw = match &self.imgui {
            Some(ImGuiEvent::MouseMotion(event)) => Some(SDL_Event { motion: *event }),
            Some(ImGuiEvent::MouseWheel(event)) => Some(SDL_Event { wheel: *event }),
            Some(ImGuiEvent::MouseButton(event)) => Some(SDL_Event { button: *event }),
            Some(ImGuiEvent::TextInput {
                timestamp,
                window_id,
                text,
            }) => Some(SDL_Event {
                text: SDL_TextInputEvent {
                    r#type: SDL_EVENT_TEXT_INPUT,
                    reserved: 0,
                    timestamp: *timestamp,
                    windowID: *window_id,
                    text: text.as_ptr(),
                    ..Default::default()
                },
            }),
            Some(ImGuiEvent::Keyboard(event)) => Some(SDL_Event { key: *event }),
            Some(ImGuiEvent::Display(event)) => Some(SDL_Event { display: *event }),
            Some(ImGuiEvent::Window(event)) => Some(SDL_Event { window: *event }),
            Some(ImGuiEvent::GamepadDevice(event)) => Some(SDL_Event { gdevice: *event }),
            None => None,
        };
        callback(raw.as_ref())
    }

    /// Whether this event requests application exit for `main_window_id`.
    pub fn requests_exit(&self, main_window_id: u32) -> bool {
        match self.application {
            ApplicationEvent::Quit | ApplicationEvent::Escape => true,
            ApplicationEvent::WindowCloseRequested { window_id } => window_id == main_window_id,
            ApplicationEvent::WindowPixelSizeChanged { .. } | ApplicationEvent::Other => false,
        }
    }

    /// Whether this event invalidates the main window's drawable surface size.
    pub fn is_pixel_size_changed_for(&self, window_id: u32) -> bool {
        self.application == ApplicationEvent::WindowPixelSizeChanged { window_id }
    }

    /// Returns owned file/text/source/position payloads without replaying them through ImGui.
    pub fn drop_event(&self) -> Option<&Sdl3DropEvent> {
        self.drop.as_ref()
    }
}

struct CallbackEventNode {
    event: Sdl3CallbackEvent,
    previous: Option<usize>,
    next: Option<usize>,
    coalescible_previous: Option<usize>,
    coalescible_next: Option<usize>,
    coalescing_key: Option<CoalescingKey>,
}

/// A bounded insertion-ordered queue with O(1) state-event coalescing and eviction.
#[derive(Default)]
struct CallbackEventQueue {
    slots: Vec<Option<CallbackEventNode>>,
    free_slots: Vec<usize>,
    head: Option<usize>,
    tail: Option<usize>,
    coalescible_head: Option<usize>,
    coalescible_tail: Option<usize>,
    coalescing: HashMap<CoalescingKey, usize>,
}

impl CallbackEventQueue {
    fn len(&self) -> usize {
        self.slots.len() - self.free_slots.len()
    }

    fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    fn coalescible_len(&self) -> usize {
        self.coalescing.len()
    }

    fn find_coalescible(&self, key: CoalescingKey) -> Option<usize> {
        self.coalescing.get(&key).copied()
    }

    fn oldest_coalescible(&self) -> Option<usize> {
        self.coalescible_head
    }

    fn push_back(&mut self, event: Sdl3CallbackEvent) {
        let coalescing_key = event.coalescing_key();
        let index = if let Some(index) = self.free_slots.pop() {
            debug_assert!(self.slots[index].is_none());
            index
        } else {
            self.slots.push(None);
            self.slots.len() - 1
        };
        let previous = self.tail;
        let coalescible_previous = coalescing_key.and(self.coalescible_tail);
        self.slots[index] = Some(CallbackEventNode {
            event,
            previous,
            next: None,
            coalescible_previous,
            coalescible_next: None,
            coalescing_key,
        });

        if let Some(previous) = previous {
            self.node_mut(previous).next = Some(index);
        } else {
            self.head = Some(index);
        }
        self.tail = Some(index);

        if let Some(key) = coalescing_key {
            if let Some(previous) = coalescible_previous {
                self.node_mut(previous).coalescible_next = Some(index);
            } else {
                self.coalescible_head = Some(index);
            }
            self.coalescible_tail = Some(index);
            debug_assert!(self.coalescing.insert(key, index).is_none());
        }
    }

    fn replace_and_move_to_back(&mut self, index: usize, event: Sdl3CallbackEvent) {
        debug_assert_eq!(
            self.node(index).coalescing_key,
            event.coalescing_key(),
            "a coalescing slot must retain its key"
        );
        self.node_mut(index).event = event;
        self.move_to_back(index);
        self.move_coalescible_to_back(index);
    }

    fn move_to_back(&mut self, index: usize) {
        if self.tail == Some(index) {
            return;
        }
        let (previous, next) = {
            let node = self.node(index);
            (node.previous, node.next)
        };
        if let Some(previous) = previous {
            self.node_mut(previous).next = next;
        } else {
            self.head = next;
        }
        if let Some(next) = next {
            self.node_mut(next).previous = previous;
        }

        let previous = self.tail;
        {
            let node = self.node_mut(index);
            node.previous = previous;
            node.next = None;
        }
        if let Some(previous) = previous {
            self.node_mut(previous).next = Some(index);
        } else {
            self.head = Some(index);
        }
        self.tail = Some(index);
    }

    fn move_coalescible_to_back(&mut self, index: usize) {
        if self.coalescible_tail == Some(index) {
            return;
        }
        let (previous, next) = {
            let node = self.node(index);
            (node.coalescible_previous, node.coalescible_next)
        };
        if let Some(previous) = previous {
            self.node_mut(previous).coalescible_next = next;
        } else {
            self.coalescible_head = next;
        }
        if let Some(next) = next {
            self.node_mut(next).coalescible_previous = previous;
        }

        let previous = self.coalescible_tail;
        {
            let node = self.node_mut(index);
            node.coalescible_previous = previous;
            node.coalescible_next = None;
        }
        if let Some(previous) = previous {
            self.node_mut(previous).coalescible_next = Some(index);
        } else {
            self.coalescible_head = Some(index);
        }
        self.coalescible_tail = Some(index);
    }

    fn remove(&mut self, index: usize) -> Sdl3CallbackEvent {
        let (previous, next, coalescible_previous, coalescible_next, coalescing_key) = {
            let node = self.node(index);
            (
                node.previous,
                node.next,
                node.coalescible_previous,
                node.coalescible_next,
                node.coalescing_key,
            )
        };

        if let Some(previous) = previous {
            self.node_mut(previous).next = next;
        } else {
            self.head = next;
        }
        if let Some(next) = next {
            self.node_mut(next).previous = previous;
        } else {
            self.tail = previous;
        }

        if let Some(key) = coalescing_key {
            if let Some(previous) = coalescible_previous {
                self.node_mut(previous).coalescible_next = coalescible_next;
            } else {
                self.coalescible_head = coalescible_next;
            }
            if let Some(next) = coalescible_next {
                self.node_mut(next).coalescible_previous = coalescible_previous;
            } else {
                self.coalescible_tail = coalescible_previous;
            }
            debug_assert_eq!(self.coalescing.remove(&key), Some(index));
        }

        let node = self.slots[index]
            .take()
            .expect("callback event queue links must reference occupied slots");
        self.free_slots.push(index);
        node.event
    }

    fn pop_front(&mut self) -> Option<Sdl3CallbackEvent> {
        self.head.map(|index| self.remove(index))
    }

    fn node(&self, index: usize) -> &CallbackEventNode {
        self.slots[index]
            .as_ref()
            .expect("callback event queue links must reference occupied slots")
    }

    fn node_mut(&mut self, index: usize) -> &mut CallbackEventNode {
        self.slots[index]
            .as_mut()
            .expect("callback event queue links must reference occupied slots")
    }

    #[cfg(test)]
    fn assert_invariants(&self) {
        use std::collections::HashSet;

        let occupied = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, node)| node.as_ref().map(|_| index))
            .collect::<HashSet<_>>();
        let free = self.free_slots.iter().copied().collect::<HashSet<_>>();
        assert_eq!(free.len(), self.free_slots.len(), "free slots repeat");
        assert!(occupied.is_disjoint(&free), "occupied slot is also free");
        assert_eq!(occupied.len() + free.len(), self.slots.len());
        assert_eq!(occupied.len(), self.len());

        let mut linked = HashSet::new();
        let mut previous = None;
        let mut current = self.head;
        while let Some(index) = current {
            assert!(linked.insert(index), "event links contain a cycle");
            let node = self.node(index);
            assert_eq!(node.previous, previous);
            previous = Some(index);
            current = node.next;
        }
        assert_eq!(previous, self.tail);
        assert_eq!(linked, occupied, "event links do not cover every node");

        let mut coalescible = HashSet::new();
        let mut previous = None;
        let mut current = self.coalescible_head;
        while let Some(index) = current {
            assert!(
                coalescible.insert(index),
                "coalescible links contain a cycle"
            );
            let node = self.node(index);
            assert!(node.coalescing_key.is_some());
            assert_eq!(node.coalescible_previous, previous);
            previous = Some(index);
            current = node.coalescible_next;
        }
        assert_eq!(previous, self.coalescible_tail);
        assert_eq!(coalescible.len(), self.coalescing.len());

        for (index, node) in self.slots.iter().enumerate() {
            let Some(node) = node else {
                continue;
            };
            match node.coalescing_key {
                Some(key) => {
                    assert!(coalescible.contains(&index));
                    assert_eq!(self.coalescing.get(&key), Some(&index));
                }
                None => {
                    assert!(!coalescible.contains(&index));
                    assert_eq!(node.coalescible_previous, None);
                    assert_eq!(node.coalescible_next, None);
                }
            }
        }
    }
}

/// A main-thread-owned callback batch drained from [`Sdl3CallbackEventHandoff`].
///
/// Events and deferred callback failures are detached atomically. Inspect [`Self::faults`] before
/// processing the events when callback failures are fatal to the application. Retained input and
/// lifecycle events remain available in this batch even when the callback path overflowed or
/// recovered a poisoned lock.
#[must_use = "callback batches contain events and faults that must be handled"]
pub struct Sdl3CallbackEventBatch {
    events: CallbackEventQueue,
    faults: Vec<Sdl3CallbackEventHandoffError>,
}

impl Sdl3CallbackEventBatch {
    /// Take the next event for main-thread processing.
    pub fn pop(&mut self) -> Option<Sdl3CallbackEvent> {
        self.events.pop_front()
    }

    /// Returns whether the batch contains no events.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Returns the number of events remaining in the batch.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Iterate over every deferred failure detached with this event batch.
    pub fn faults(&self) -> impl ExactSizeIterator<Item = Sdl3CallbackEventHandoffError> + '_ {
        self.faults.iter().copied()
    }

    /// Return the first deferred failure, if any.
    pub fn first_fault(&self) -> Option<Sdl3CallbackEventHandoffError> {
        self.faults.first().copied()
    }

    /// Return whether the callback path reported any deferred failures.
    pub fn has_faults(&self) -> bool {
        !self.faults.is_empty()
    }
}

impl Iterator for Sdl3CallbackEventBatch {
    type Item = Sdl3CallbackEvent;

    fn next(&mut self) -> Option<Self::Item> {
        self.pop()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len();
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for Sdl3CallbackEventBatch {}

#[derive(Default)]
struct FaultLatch {
    faults: VecDeque<Sdl3CallbackEventHandoffError>,
}

impl FaultLatch {
    fn record(&mut self, fault: Sdl3CallbackEventHandoffError) {
        match fault {
            Sdl3CallbackEventHandoffError::CallbackPanicked
            | Sdl3CallbackEventHandoffError::QueuePoisoned => {
                if !self.faults.contains(&fault) {
                    self.faults.push_back(fault);
                }
            }
            Sdl3CallbackEventHandoffError::QueueOverflow { dropped_events } => {
                if let Some(Sdl3CallbackEventHandoffError::QueueOverflow {
                    dropped_events: accumulated,
                }) = self.faults.iter_mut().find(|fault| {
                    matches!(fault, Sdl3CallbackEventHandoffError::QueueOverflow { .. })
                }) {
                    *accumulated = accumulated.saturating_add(dropped_events);
                } else {
                    self.faults.push_back(fault);
                }
            }
        }
        debug_assert!(self.faults.len() <= 3);
    }

    fn drain(&mut self) -> Vec<Sdl3CallbackEventHandoffError> {
        self.faults.drain(..).collect()
    }
}

#[derive(Default)]
struct CallbackHandoffState {
    events: CallbackEventQueue,
    faults: FaultLatch,
}

impl CallbackHandoffState {
    fn enqueue(&mut self, event: Sdl3CallbackEvent, capacity: usize, ordered_reserve: usize) {
        let coalescing_key = event.coalescing_key();
        if let Some(key) = coalescing_key
            && let Some(index) = self.events.find_coalescible(key)
        {
            self.events.replace_and_move_to_back(index, event);
            return;
        }

        let coalescible_limit = capacity.saturating_sub(ordered_reserve);
        if coalescing_key.is_some() {
            if self.events.len() < capacity && self.events.coalescible_len() < coalescible_limit {
                self.events.push_back(event);
            } else {
                self.record_overflow(1);
            }
            return;
        }

        if self.events.len() == capacity {
            if let Some(index) = self.events.oldest_coalescible() {
                self.events.remove(index);
                self.record_overflow(1);
            } else {
                self.record_overflow(1);
                return;
            }
        }
        self.events.push_back(event);
    }

    fn record_overflow(&mut self, dropped_events: usize) {
        self.faults
            .record(Sdl3CallbackEventHandoffError::QueueOverflow { dropped_events });
    }
}

/// Thread-safe handoff from `SDL_AppEvent` to main-thread event processing.
///
/// The callback path owns transient SDL payloads immediately and contains unwind-capable Rust
/// panics. [`Self::drain`] atomically releases retained events and any deferred failures, so a
/// continuously busy callback cannot starve main-thread event processing by repeatedly reporting
/// new failures first.
pub struct Sdl3CallbackEventHandoff {
    capacity: usize,
    ordered_reserve: usize,
    state: Mutex<CallbackHandoffState>,
    window_targets: Option<Arc<CallbackWindowTargets>>,
}

impl Default for Sdl3CallbackEventHandoff {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_CALLBACK_EVENT_CAPACITY)
    }
}

impl fmt::Debug for Sdl3CallbackEventHandoff {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Sdl3CallbackEventHandoff")
            .field("capacity", &self.capacity)
            .field("ordered_reserve", &self.ordered_reserve)
            .finish_non_exhaustive()
    }
}

impl Sdl3CallbackEventHandoff {
    /// Create a bounded callback handoff.
    ///
    /// One quarter of the capacity, with at least one slot, is reserved for ordered events such as
    /// key, text, button, focus, close, and quit notifications. High-frequency state events are
    /// coalesced by native window or display identity and cannot consume that reserve.
    ///
    /// # Panics
    ///
    /// Panics when `capacity` is zero.
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(
            capacity > 0,
            "SDL3 callback event capacity must be non-zero"
        );
        let ordered_reserve = capacity.div_ceil(4).max(1);
        Self {
            capacity,
            ordered_reserve,
            state: Mutex::new(CallbackHandoffState::default()),
            window_targets: None,
        }
    }

    /// Copy one event before SDL invalidates its callback payload.
    ///
    /// This is the only unsafe operation required by the normal SDL callback-mode integration.
    /// The method catches unwind-capable Rust panics and defers them to [`Self::drain`] so
    /// unwinding never crosses the C callback ABI. A failed event is not queued.
    ///
    /// # Safety
    ///
    /// `raw` must be the valid event supplied to the current SDL application callback, or an event
    /// constructed with equivalent validity invariants. Its type must name the active union
    /// member, every pointer reachable from that member must remain valid until this method
    /// returns, and the event must belong to the SDL runtime used by the backend that will process
    /// it.
    pub unsafe fn push_from_callback(&self, raw: &SDL_Event) {
        self.enqueue_with(|| {
            let mut event = unsafe { Sdl3CallbackEvent::from_callback_raw(raw) };
            if let Some(drop) = &mut event.drop {
                drop.observed_target = self
                    .window_targets
                    .as_ref()
                    .and_then(|targets| targets.observe(drop.window_id, drop.timestamp));
            }
            Some(event)
        });
    }

    pub(crate) fn for_window_targets(targets: Arc<CallbackWindowTargets>) -> Self {
        Self {
            window_targets: Some(targets),
            ..Self::default()
        }
    }

    fn enqueue_with(&self, make_event: impl FnOnce() -> Option<Sdl3CallbackEvent>) {
        let result = catch_unwind(AssertUnwindSafe(|| {
            if let Some(event) = make_event() {
                self.lock_state()
                    .enqueue(event, self.capacity, self.ordered_reserve);
            }
        }));
        if let Err(payload) = result {
            self.record_fault(Sdl3CallbackEventHandoffError::CallbackPanicked);
            dispose_panic_payload_without_unwinding(payload);
        }
    }

    /// Atomically detach the current events and deferred callback failures.
    ///
    /// Ordered input and lifecycle events preserve FIFO delivery; high-frequency state events
    /// retain only their latest position in that order. Faults do not block access to retained
    /// events, and events or faults arriving after this operation starts belong to a later batch.
    pub fn drain(&self) -> Sdl3CallbackEventBatch {
        let mut state = self.lock_state();
        Sdl3CallbackEventBatch {
            events: std::mem::take(&mut state.events),
            faults: state.faults.drain(),
        }
    }

    fn lock_state(&self) -> MutexGuard<'_, CallbackHandoffState> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                self.state.clear_poison();
                let mut state = poisoned.into_inner();
                state
                    .faults
                    .record(Sdl3CallbackEventHandoffError::QueuePoisoned);
                state
            }
        }
    }

    fn record_fault(&self, fault: Sdl3CallbackEventHandoffError) {
        self.lock_state().faults.record(fault);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;

    use super::*;

    fn text_event(text: &CStr) -> SDL_Event {
        SDL_Event {
            text: SDL_TextInputEvent {
                r#type: SDL_EVENT_TEXT_INPUT,
                reserved: 0,
                timestamp: 42,
                windowID: SDL_WindowID(7),
                text: text.as_ptr(),
                ..Default::default()
            },
        }
    }

    fn quit_event(timestamp: u64) -> SDL_Event {
        SDL_Event {
            quit: sdl3_sys::events::SDL_QuitEvent {
                r#type: SDL_EVENT_QUIT,
                reserved: 0,
                timestamp,
                ..Default::default()
            },
        }
    }

    fn mouse_motion_event(timestamp: u64, x: f32) -> SDL_Event {
        mouse_motion_event_for(7, timestamp, x)
    }

    fn mouse_motion_event_for(window_id: u32, timestamp: u64, x: f32) -> SDL_Event {
        SDL_Event {
            motion: SDL_MouseMotionEvent {
                r#type: SDL_EVENT_MOUSE_MOTION,
                timestamp,
                windowID: SDL_WindowID(window_id),
                x,
                ..Default::default()
            },
        }
    }

    #[test]
    fn text_input_is_owned_before_the_callback_returns() {
        let mut queue = thread::spawn(|| {
            let source = CString::new("queued text").unwrap();
            let raw = text_event(&source);
            let handoff = Sdl3CallbackEventHandoff::default();
            // SAFETY: `raw` has the active text member and its pointer remains valid here.
            unsafe { handoff.push_from_callback(&raw) };
            drop(source);
            handoff.drain()
        })
        .join()
        .expect("callback worker must not panic");

        let queued = queue.pop().expect("event must be queued");
        queued.with_raw_event(|raw| {
            let raw = raw.expect("text input is consumed by the ImGui backend");
            let text = unsafe { CStr::from_ptr(raw.text.text) };
            assert_eq!(text.to_bytes(), b"queued text");
        });
    }

    #[test]
    fn file_drop_owns_source_and_data_before_callback_memory_is_released() {
        let source = CString::new("file manager").unwrap();
        let data = CString::new("C:/tmp/queued.txt").unwrap();
        let raw = SDL_Event {
            drop: sdl3_sys::events::SDL_DropEvent {
                r#type: sdl3_sys::events::SDL_EVENT_DROP_FILE,
                reserved: 0,
                timestamp: 42,
                windowID: SDL_WindowID(7),
                x: 0.0,
                y: 0.0,
                source: source.as_ptr(),
                data: data.as_ptr(),
                ..Default::default()
            },
        };
        let handoff = Sdl3CallbackEventHandoff::with_capacity(2);
        // SAFETY: `raw` has the active drop member and both pointers remain valid here.
        unsafe { handoff.push_from_callback(&raw) };
        drop((source, data));

        let quit = quit_event(43);
        // SAFETY: `quit` has the active pointer-free quit member.
        unsafe { handoff.push_from_callback(&quit) };

        let mut events = handoff.drain();
        assert_eq!(events.len(), 2);
        let inert = events
            .pop()
            .expect("unsupported event summary must be retained");
        assert!(!inert.requests_exit(7));
        let drop = inert.drop_event().unwrap();
        assert_eq!(drop.source.as_deref(), Some("file manager"));
        assert_eq!(
            drop.payload,
            Sdl3DropPayload::File("C:/tmp/queued.txt".into())
        );
        assert_eq!(drop.position, Some([0.0, 0.0]));
        assert_eq!(drop.observed_target, None);
        inert.with_raw_event(|raw| assert!(raw.is_none()));
        assert!(
            events
                .pop()
                .expect("quit must use the available slot")
                .requests_exit(7)
        );
    }

    #[test]
    fn drop_sequences_keep_multiple_files_text_and_empty_hover_completion_in_order() {
        let handoff = Sdl3CallbackEventHandoff::default();
        let events = [
            (SDL_EVENT_DROP_BEGIN, None),
            (SDL_EVENT_DROP_FILE, Some(c"first.png")),
            (SDL_EVENT_DROP_FILE, Some(c"second.png")),
            (SDL_EVENT_DROP_TEXT, Some(c"asset notes")),
            (SDL_EVENT_DROP_COMPLETE, None),
            (SDL_EVENT_DROP_BEGIN, None),
            (SDL_EVENT_DROP_POSITION, None),
            (SDL_EVENT_DROP_COMPLETE, None),
        ];
        for (index, (kind, data)) in events.iter().enumerate() {
            let raw = SDL_Event {
                drop: sdl3_sys::events::SDL_DropEvent {
                    r#type: *kind,
                    timestamp: index as u64 + 1,
                    windowID: SDL_WindowID(7),
                    x: 12.5,
                    y: 18.25,
                    data: data.map_or(std::ptr::null(), |value| value.as_ptr()),
                    ..Default::default()
                },
            };
            unsafe { handoff.push_from_callback(&raw) };
        }
        let drops: Vec<_> = handoff
            .drain()
            .map(|event| event.drop_event().unwrap().clone())
            .collect();
        assert_eq!(drops.len(), 8);
        assert_eq!(drops[0].position, None);
        assert_eq!(drops[1].payload, Sdl3DropPayload::File("first.png".into()));
        assert_eq!(drops[2].payload, Sdl3DropPayload::File("second.png".into()));
        assert_eq!(
            drops[3].payload,
            Sdl3DropPayload::Text("asset notes".into())
        );
        assert_eq!(drops[6].payload, Sdl3DropPayload::Position);
        assert_eq!(drops[7].payload, Sdl3DropPayload::Complete);
    }

    #[test]
    fn reused_numeric_ids_and_missing_timestamps_do_not_claim_new_registrations_for_old_events() {
        let _guard = crate::tests::test_guard();
        let context = dear_imgui_rs::Context::create();
        let first = NativeWindowTarget {
            instance_id: dear_imgui_rs::NativeWindowInstanceId::allocate(context.id()).unwrap(),
            viewport_id: 1.into(),
            is_main: true,
        };
        let second = NativeWindowTarget {
            instance_id: dear_imgui_rs::NativeWindowInstanceId::allocate(context.id()).unwrap(),
            ..first
        };
        let targets = Arc::new(CallbackWindowTargets::default());
        targets.0.lock().unwrap().insert(
            7,
            PublishedWindowTarget {
                target: first,
                published_at: 100,
            },
        );
        let handoff = Sdl3CallbackEventHandoff::for_window_targets(Arc::clone(&targets));
        let raw = |timestamp| SDL_Event {
            drop: sdl3_sys::events::SDL_DropEvent {
                r#type: SDL_EVENT_DROP_FILE,
                timestamp,
                windowID: SDL_WindowID(7),
                data: c"asset.png".as_ptr(),
                ..Default::default()
            },
        };
        unsafe { handoff.push_from_callback(&raw(110)) };
        targets.retire(7, first);
        targets.0.lock().unwrap().insert(
            7,
            PublishedWindowTarget {
                target: second,
                published_at: 200,
            },
        );
        unsafe {
            handoff.push_from_callback(&raw(110));
            handoff.push_from_callback(&raw(0));
            handoff.push_from_callback(&raw(210));
        }
        let drops: Vec<_> = handoff
            .drain()
            .map(|event| event.drop_event().unwrap().clone())
            .collect();
        assert_eq!(drops[0].observed_target, Some(first));
        assert_eq!(drops[1].observed_target, None);
        assert_eq!(drops[2].observed_target, None);
        assert_eq!(drops[3].observed_target, Some(second));
    }

    #[test]
    fn display_usable_bounds_changes_reach_imgui() {
        let raw = SDL_Event {
            display: SDL_DisplayEvent {
                r#type: SDL_EVENT_DISPLAY_USABLE_BOUNDS_CHANGED,
                reserved: 0,
                timestamp: 42,
                displayID: SDL_DisplayID(1),
                data1: 0,
                data2: 0,
                ..Default::default()
            },
        };
        let handoff = Sdl3CallbackEventHandoff::default();
        // SAFETY: `raw` has the active display member and contains no pointer payload.
        unsafe { handoff.push_from_callback(&raw) };

        handoff
            .drain()
            .pop()
            .expect("event must be queued")
            .with_raw_event(|raw| {
                assert!(
                    raw.expect("display changes are consumed by the ImGui backend")
                        .event_type()
                        == SDL_EVENT_DISPLAY_USABLE_BOUNDS_CHANGED
                );
            });
    }

    #[test]
    fn application_summary_preserves_surface_and_exit_events() {
        let pixel_size = SDL_Event {
            window: SDL_WindowEvent {
                r#type: SDL_EVENT_WINDOW_PIXEL_SIZE_CHANGED,
                reserved: 0,
                timestamp: 42,
                windowID: SDL_WindowID(7),
                data1: 1920,
                data2: 1080,
                ..Default::default()
            },
        };
        let close = SDL_Event {
            window: SDL_WindowEvent {
                r#type: SDL_EVENT_WINDOW_CLOSE_REQUESTED,
                reserved: 0,
                timestamp: 43,
                windowID: SDL_WindowID(8),
                data1: 0,
                data2: 0,
                ..Default::default()
            },
        };
        let handoff = Sdl3CallbackEventHandoff::default();
        // SAFETY: both unions name their active pointer-free window member.
        unsafe {
            handoff.push_from_callback(&pixel_size);
            handoff.push_from_callback(&close);
        }

        let mut queue = handoff.drain();
        assert!(
            queue
                .pop()
                .expect("pixel-size event must be queued")
                .is_pixel_size_changed_for(7)
        );
        let close = queue.pop().expect("close event must be queued");
        assert!(!close.requests_exit(7));
        assert!(close.requests_exit(8));
    }

    #[test]
    fn callback_events_can_cross_from_a_worker_thread() {
        let handoff = Arc::new(Sdl3CallbackEventHandoff::default());
        let callback_handoff = Arc::clone(&handoff);
        thread::spawn(move || {
            let raw = SDL_Event {
                quit: sdl3_sys::events::SDL_QuitEvent {
                    r#type: SDL_EVENT_QUIT,
                    reserved: 0,
                    timestamp: 42,
                    ..Default::default()
                },
            };
            // SAFETY: `raw` has the active quit member and contains no pointer payload.
            unsafe { callback_handoff.push_from_callback(&raw) };
        })
        .join()
        .expect("callback worker must not panic");

        let event = handoff.drain().pop().expect("event must cross threads");
        assert!(event.requests_exit(1));
    }

    #[test]
    fn draining_one_batch_does_not_block_the_next_callback() {
        let handoff = Arc::new(Sdl3CallbackEventHandoff::default());
        let first = SDL_Event {
            quit: sdl3_sys::events::SDL_QuitEvent {
                r#type: SDL_EVENT_QUIT,
                reserved: 0,
                timestamp: 1,
                ..Default::default()
            },
        };
        // SAFETY: `first` has the active quit member and contains no pointer payload.
        unsafe { handoff.push_from_callback(&first) };
        let mut first_batch = handoff.drain();

        let callback_handoff = Arc::clone(&handoff);
        thread::spawn(move || {
            let second = SDL_Event {
                quit: sdl3_sys::events::SDL_QuitEvent {
                    r#type: SDL_EVENT_QUIT,
                    reserved: 0,
                    timestamp: 2,
                    ..Default::default()
                },
            };
            // SAFETY: `second` has the active quit member and contains no pointer payload.
            unsafe { callback_handoff.push_from_callback(&second) };
        })
        .join()
        .expect("the next callback must not wait for a retained batch");

        assert!(first_batch.pop().is_some());
        assert!(handoff.drain().pop().is_some());
    }

    #[test]
    fn callback_panic_is_contained_and_reported() {
        let handoff = Sdl3CallbackEventHandoff::default();

        handoff.enqueue_with(|| panic!("synthetic callback copy failure"));

        let batch = handoff.drain();
        assert_eq!(
            batch.first_fault(),
            Some(Sdl3CallbackEventHandoffError::CallbackPanicked)
        );
        assert!(batch.is_empty());
    }

    #[test]
    fn repeated_callback_failures_are_latched_without_unbounded_growth() {
        let handoff = Sdl3CallbackEventHandoff::default();

        handoff.enqueue_with(|| panic!("first synthetic callback copy failure"));
        handoff.enqueue_with(|| panic!("second synthetic callback copy failure"));

        let batch = handoff.drain();
        assert_eq!(
            batch.faults().collect::<Vec<_>>(),
            vec![Sdl3CallbackEventHandoffError::CallbackPanicked]
        );
        assert!(batch.is_empty());
    }

    #[test]
    fn high_frequency_state_events_coalesce_at_their_latest_ordered_position() {
        let handoff = Sdl3CallbackEventHandoff::with_capacity(4);
        let first_motion = mouse_motion_event(1, 10.0);
        let quit = quit_event(2);
        let latest_motion = mouse_motion_event(3, 30.0);
        // SAFETY: all unions name their active pointer-free members.
        unsafe {
            handoff.push_from_callback(&first_motion);
            handoff.push_from_callback(&quit);
            handoff.push_from_callback(&latest_motion);
        }

        let mut events = handoff.drain();
        assert_eq!(events.len(), 2);
        assert!(
            events
                .pop()
                .expect("quit must remain ordered")
                .requests_exit(7)
        );
        events
            .pop()
            .expect("latest mouse motion must be retained")
            .with_raw_event(|raw| {
                let raw = raw.expect("mouse motion is consumed by the ImGui backend");
                assert_eq!(unsafe { raw.motion.x }, 30.0);
                assert_eq!(unsafe { raw.motion.timestamp }, 3);
            });
    }

    #[test]
    fn high_frequency_state_events_reuse_their_indexed_queue_slot() {
        let handoff = Sdl3CallbackEventHandoff::with_capacity(4);
        for timestamp in 0..10_000 {
            let motion = mouse_motion_event(timestamp, timestamp as f32);
            // SAFETY: the union names its active pointer-free motion member.
            unsafe { handoff.push_from_callback(&motion) };
        }

        {
            let state = handoff.state.lock().unwrap();
            assert_eq!(state.events.len(), 1);
            assert_eq!(state.events.slots.len(), 1);
            assert_eq!(state.events.coalescible_len(), 1);
        }

        let mut batch = handoff.drain();
        batch
            .pop()
            .expect("the latest mouse motion must be retained")
            .with_raw_event(|raw| {
                let raw = raw.expect("mouse motion is consumed by the ImGui backend");
                assert_eq!(unsafe { raw.motion.timestamp }, 9_999);
                assert_eq!(unsafe { raw.motion.x }, 9_999.0);
            });
    }

    #[test]
    fn indexed_queue_matches_a_reference_model_under_mixed_operations() {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        enum ModelEvent {
            Motion { window_id: u32, timestamp: u64 },
            Quit,
        }

        impl ModelEvent {
            fn coalescing_key(self) -> Option<u32> {
                match self {
                    Self::Motion { window_id, .. } => Some(window_id),
                    Self::Quit => None,
                }
            }
        }

        const CAPACITY: usize = 7;
        const ORDERED_RESERVE: usize = 2;
        let handoff = Sdl3CallbackEventHandoff::with_capacity(CAPACITY);
        assert_eq!(handoff.ordered_reserve, ORDERED_RESERVE);
        let mut model = VecDeque::new();
        let mut dropped_events = 0usize;
        let mut random = 0x05ee_dcaf_ed15_ca11_u64;

        for timestamp in 0..5_000u64 {
            random = random
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let event = if random % 4 == 0 {
                ModelEvent::Quit
            } else {
                ModelEvent::Motion {
                    window_id: ((random >> 8) % 6) as u32 + 1,
                    timestamp,
                }
            };

            if let Some(key) = event.coalescing_key() {
                if let Some(index) = model
                    .iter()
                    .position(|queued: &ModelEvent| queued.coalescing_key() == Some(key))
                {
                    model.remove(index);
                    model.push_back(event);
                } else {
                    let coalescible = model
                        .iter()
                        .filter(|queued| queued.coalescing_key().is_some())
                        .count();
                    if model.len() < CAPACITY
                        && coalescible < CAPACITY.saturating_sub(ORDERED_RESERVE)
                    {
                        model.push_back(event);
                    } else {
                        dropped_events += 1;
                    }
                }
            } else {
                let mut retain = true;
                if model.len() == CAPACITY {
                    if let Some(index) = model
                        .iter()
                        .position(|queued| queued.coalescing_key().is_some())
                    {
                        model.remove(index);
                        dropped_events += 1;
                    } else {
                        dropped_events += 1;
                        retain = false;
                    }
                }
                if retain {
                    model.push_back(event);
                }
            }

            let raw = match event {
                ModelEvent::Motion {
                    window_id,
                    timestamp,
                } => mouse_motion_event_for(window_id, timestamp, timestamp as f32),
                ModelEvent::Quit => quit_event(timestamp),
            };
            // SAFETY: both generated unions name active pointer-free members.
            unsafe { handoff.push_from_callback(&raw) };
            handoff.state.lock().unwrap().events.assert_invariants();
        }

        let batch = handoff.drain();
        let faults = batch.faults().collect::<Vec<_>>();
        assert_eq!(
            faults,
            vec![Sdl3CallbackEventHandoffError::QueueOverflow { dropped_events }]
        );
        let actual = batch
            .map(|event| match event.imgui {
                Some(ImGuiEvent::MouseMotion(raw)) => ModelEvent::Motion {
                    window_id: raw.windowID.0,
                    timestamp: raw.timestamp,
                },
                None if event.application == ApplicationEvent::Quit => ModelEvent::Quit,
                _ => panic!("model generated an unexpected SDL callback event"),
            })
            .collect::<VecDeque<_>>();
        assert_eq!(actual, model);
    }

    #[test]
    fn ordered_events_evict_coalescible_state_before_overflowing() {
        let handoff = Sdl3CallbackEventHandoff::with_capacity(2);
        let motion = mouse_motion_event(1, 10.0);
        let first_quit = quit_event(2);
        let second_quit = quit_event(3);
        // SAFETY: all unions name their active pointer-free members.
        unsafe {
            handoff.push_from_callback(&motion);
            handoff.push_from_callback(&first_quit);
            handoff.push_from_callback(&second_quit);
        }

        let events = handoff.drain();
        assert_eq!(
            events.first_fault(),
            Some(Sdl3CallbackEventHandoffError::QueueOverflow { dropped_events: 1 })
        );
        assert_eq!(events.len(), 2);
        assert!(events.into_iter().all(|event| event.requests_exit(7)));
    }

    #[test]
    fn ordered_events_do_not_consume_the_coalescible_budget() {
        let handoff = Sdl3CallbackEventHandoff::with_capacity(4);
        for timestamp in 1..=3 {
            let event = quit_event(timestamp);
            // SAFETY: the union names its active pointer-free quit member.
            unsafe { handoff.push_from_callback(&event) };
        }
        let motion = mouse_motion_event(4, 24.0);
        // SAFETY: the union names its active pointer-free motion member.
        unsafe { handoff.push_from_callback(&motion) };

        assert_eq!(handoff.drain().len(), 4);
    }

    #[test]
    fn a_full_ordered_queue_reports_and_drops_new_events_without_growing() {
        let handoff = Sdl3CallbackEventHandoff::with_capacity(1);
        let first = quit_event(1);
        let second = quit_event(2);
        // SAFETY: both unions name their active pointer-free quit member.
        unsafe {
            handoff.push_from_callback(&first);
            handoff.push_from_callback(&second);
        }

        let batch = handoff.drain();
        assert_eq!(
            batch.first_fault(),
            Some(Sdl3CallbackEventHandoffError::QueueOverflow { dropped_events: 1 })
        );
        assert_eq!(batch.len(), 1);
    }

    #[test]
    fn panicking_panic_payload_drop_cannot_escape_the_callback_boundary() {
        struct PanickingPayload;

        impl Drop for PanickingPayload {
            fn drop(&mut self) {
                panic!("panic payload drop");
            }
        }

        let handoff = Sdl3CallbackEventHandoff::default();
        let result = catch_unwind(AssertUnwindSafe(|| {
            handoff.enqueue_with(|| std::panic::panic_any(PanickingPayload));
        }));

        assert!(result.is_ok());
        assert_eq!(
            handoff.drain().first_fault(),
            Some(Sdl3CallbackEventHandoffError::CallbackPanicked)
        );
    }

    #[test]
    fn drain_detaches_faults_and_events_together() {
        let handoff = Sdl3CallbackEventHandoff::default();
        handoff.enqueue_with(|| panic!("synthetic callback copy failure"));
        let raw = SDL_Event {
            quit: sdl3_sys::events::SDL_QuitEvent {
                r#type: SDL_EVENT_QUIT,
                reserved: 0,
                timestamp: 42,
                ..Default::default()
            },
        };
        // SAFETY: `raw` has the active quit member and contains no pointer payload.
        unsafe { handoff.push_from_callback(&raw) };

        let mut batch = handoff.drain();
        assert_eq!(
            batch.first_fault(),
            Some(Sdl3CallbackEventHandoffError::CallbackPanicked)
        );
        let event = batch.pop().expect("the valid event must be retained");
        assert!(event.requests_exit(1));
    }

    #[test]
    fn repeated_overflow_cannot_starve_retained_events() {
        let handoff = Sdl3CallbackEventHandoff::with_capacity(1);
        let first = quit_event(1);
        let dropped = quit_event(2);
        // SAFETY: both unions name their active pointer-free quit member.
        unsafe {
            handoff.push_from_callback(&first);
            handoff.push_from_callback(&dropped);
        }

        let mut first_batch = handoff.drain();

        let second = quit_event(3);
        let also_dropped = quit_event(4);
        // SAFETY: both unions name their active pointer-free quit member.
        unsafe {
            handoff.push_from_callback(&second);
            handoff.push_from_callback(&also_dropped);
        }

        assert_eq!(
            first_batch.first_fault(),
            Some(Sdl3CallbackEventHandoffError::QueueOverflow { dropped_events: 1 })
        );
        assert!(
            first_batch
                .pop()
                .expect("the first retained event must remain available")
                .requests_exit(7)
        );

        let mut second_batch = handoff.drain();
        assert_eq!(
            second_batch.first_fault(),
            Some(Sdl3CallbackEventHandoffError::QueueOverflow { dropped_events: 1 })
        );
        assert!(
            second_batch
                .pop()
                .expect("the next retained event must make progress")
                .requests_exit(7)
        );
    }

    #[test]
    fn one_batch_retains_all_distinct_fault_kinds_in_observation_order() {
        let handoff = Sdl3CallbackEventHandoff::with_capacity(1);
        let first = quit_event(1);
        let dropped = quit_event(2);
        // SAFETY: both unions name their active pointer-free quit member.
        unsafe {
            handoff.push_from_callback(&first);
            handoff.push_from_callback(&dropped);
        }
        handoff.enqueue_with(|| panic!("synthetic callback copy failure"));

        let batch = handoff.drain();
        assert_eq!(
            batch.faults().collect::<Vec<_>>(),
            vec![
                Sdl3CallbackEventHandoffError::QueueOverflow { dropped_events: 1 },
                Sdl3CallbackEventHandoffError::CallbackPanicked,
            ]
        );
        assert!(batch.has_faults());
    }

    #[test]
    fn poisoned_queue_is_recovered_and_reported() {
        let handoff = Arc::new(Sdl3CallbackEventHandoff::default());
        let callback_handoff = Arc::clone(&handoff);
        let result = thread::spawn(move || {
            let _state = callback_handoff.state.lock().unwrap();
            panic!("synthetic queue poison");
        })
        .join();
        assert!(result.is_err());

        let batch = handoff.drain();
        assert_eq!(
            batch.first_fault(),
            Some(Sdl3CallbackEventHandoffError::QueuePoisoned)
        );
        assert!(batch.is_empty());
    }
}

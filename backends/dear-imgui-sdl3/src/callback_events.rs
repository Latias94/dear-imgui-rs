//! Owned event handoff for SDL3 callback-mode applications.
//!
//! SDL may invoke `SDL_AppEvent` away from the main thread while Dear ImGui and SDL video work
//! remain main-thread-bound. This module copies the exact SDL payloads consumed by the official
//! backend before the callback returns and defers processing to the main thread.

use std::collections::VecDeque;
use std::ffi::{CStr, CString};
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, MutexGuard};

use sdl3::event::{DisplayEvent, Event, WindowEvent};
use sdl3_sys::events::{
    SDL_DisplayEvent, SDL_EVENT_DISPLAY_ADDED, SDL_EVENT_DISPLAY_CONTENT_SCALE_CHANGED,
    SDL_EVENT_DISPLAY_MOVED, SDL_EVENT_DISPLAY_ORIENTATION, SDL_EVENT_DISPLAY_REMOVED,
    SDL_EVENT_DISPLAY_USABLE_BOUNDS_CHANGED, SDL_EVENT_GAMEPAD_ADDED, SDL_EVENT_GAMEPAD_REMOVED,
    SDL_EVENT_KEY_DOWN, SDL_EVENT_KEY_UP, SDL_EVENT_MOUSE_BUTTON_DOWN, SDL_EVENT_MOUSE_BUTTON_UP,
    SDL_EVENT_MOUSE_MOTION, SDL_EVENT_MOUSE_WHEEL, SDL_EVENT_QUIT, SDL_EVENT_TEXT_INPUT,
    SDL_EVENT_WINDOW_CLOSE_REQUESTED, SDL_EVENT_WINDOW_FOCUS_GAINED, SDL_EVENT_WINDOW_FOCUS_LOST,
    SDL_EVENT_WINDOW_MOUSE_ENTER, SDL_EVENT_WINDOW_MOUSE_LEAVE, SDL_EVENT_WINDOW_MOVED,
    SDL_EVENT_WINDOW_PIXEL_SIZE_CHANGED, SDL_EVENT_WINDOW_RESIZED, SDL_Event,
    SDL_GamepadDeviceEvent, SDL_KeyboardEvent, SDL_MouseButtonEvent, SDL_MouseMotionEvent,
    SDL_MouseWheelEvent, SDL_TextInputEvent, SDL_WindowEvent,
};
use sdl3_sys::keycode::SDLK_ESCAPE;
#[cfg(test)]
use sdl3_sys::video::SDL_DisplayID;
use sdl3_sys::video::SDL_WindowID;

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
/// Pointer-bearing event variants that the official SDL3 backend does not consume are represented
/// only by their application summary and are never replayed through Dear ImGui.
pub struct Sdl3CallbackEvent {
    application: ApplicationEvent,
    imgui: Option<ImGuiEvent>,
}

impl fmt::Debug for Sdl3CallbackEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Sdl3CallbackEvent")
            .field("application", &self.application)
            .field("has_imgui_payload", &self.imgui.is_some())
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
            | Event::ControllerDeviceAdded { .. }
            | Event::ControllerDeviceRemoved { .. } => Ok(event.to_ll().as_ref().map(|raw| {
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

        Self { application, imgui }
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
}

/// A main-thread-owned FIFO batch drained from [`Sdl3CallbackEventHandoff`].
#[derive(Default)]
pub struct Sdl3CallbackEventQueue {
    events: VecDeque<Sdl3CallbackEvent>,
}

impl Sdl3CallbackEventQueue {
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
}

impl Iterator for Sdl3CallbackEventQueue {
    type Item = Sdl3CallbackEvent;

    fn next(&mut self) -> Option<Self::Item> {
        self.pop()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len();
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for Sdl3CallbackEventQueue {}

#[derive(Default)]
struct CallbackHandoffState {
    events: Sdl3CallbackEventQueue,
    faults: VecDeque<Sdl3CallbackEventHandoffError>,
}

/// Thread-safe handoff from `SDL_AppEvent` to main-thread event processing.
///
/// The callback path owns transient SDL payloads immediately and contains unwind-capable Rust
/// panics. [`Self::try_drain`] reports a contained failure before releasing the retained events.
pub struct Sdl3CallbackEventHandoff {
    state: Mutex<CallbackHandoffState>,
}

impl Default for Sdl3CallbackEventHandoff {
    fn default() -> Self {
        Self {
            state: Mutex::new(CallbackHandoffState::default()),
        }
    }
}

impl fmt::Debug for Sdl3CallbackEventHandoff {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Sdl3CallbackEventHandoff")
            .finish_non_exhaustive()
    }
}

impl Sdl3CallbackEventHandoff {
    /// Copy one event before SDL invalidates its callback payload.
    ///
    /// This is the only unsafe operation required by the normal SDL callback-mode integration.
    /// The method catches unwind-capable Rust panics and defers them to [`Self::try_drain`] so
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
        self.enqueue_with(|| unsafe { Sdl3CallbackEvent::from_callback_raw(raw) });
    }

    fn enqueue_with(&self, make_event: impl FnOnce() -> Sdl3CallbackEvent) {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let event = make_event();
            self.lock_state().events.events.push_back(event);
        }));
        if let Err(payload) = result {
            self.record_fault(Sdl3CallbackEventHandoffError::CallbackPanicked);
            dispose_panic_payload_without_unwinding(payload);
        }
    }

    /// Return a main-thread-owned event batch after reporting any earlier callback failure.
    ///
    /// When this returns an error, queued events remain available. Handle the error and retry on
    /// the next iteration to preserve FIFO delivery.
    pub fn try_drain(&self) -> Result<Sdl3CallbackEventQueue, Sdl3CallbackEventHandoffError> {
        let mut state = self.lock_state();
        if let Some(fault) = state.faults.pop_front() {
            return Err(fault);
        }
        Ok(std::mem::take(&mut state.events))
    }

    fn lock_state(&self) -> MutexGuard<'_, CallbackHandoffState> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                self.state.clear_poison();
                let mut state = poisoned.into_inner();
                state
                    .faults
                    .push_back(Sdl3CallbackEventHandoffError::QueuePoisoned);
                state
            }
        }
    }

    fn record_fault(&self, fault: Sdl3CallbackEventHandoffError) {
        self.lock_state().faults.push_back(fault);
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
            handoff.try_drain().unwrap()
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
    fn unsupported_pointer_payload_is_not_replayed() {
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
            },
        };
        let handoff = Sdl3CallbackEventHandoff::default();
        // SAFETY: `raw` has the active drop member and both pointers remain valid here.
        unsafe { handoff.push_from_callback(&raw) };
        drop((source, data));

        handoff
            .try_drain()
            .unwrap()
            .pop()
            .expect("event must be queued")
            .with_raw_event(|raw| assert!(raw.is_none()));
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
            },
        };
        let handoff = Sdl3CallbackEventHandoff::default();
        // SAFETY: `raw` has the active display member and contains no pointer payload.
        unsafe { handoff.push_from_callback(&raw) };

        handoff
            .try_drain()
            .unwrap()
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
            },
        };
        let handoff = Sdl3CallbackEventHandoff::default();
        // SAFETY: both unions name their active pointer-free window member.
        unsafe {
            handoff.push_from_callback(&pixel_size);
            handoff.push_from_callback(&close);
        }

        let mut queue = handoff.try_drain().unwrap();
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
                },
            };
            // SAFETY: `raw` has the active quit member and contains no pointer payload.
            unsafe { callback_handoff.push_from_callback(&raw) };
        })
        .join()
        .expect("callback worker must not panic");

        let event = handoff
            .try_drain()
            .unwrap()
            .pop()
            .expect("event must cross threads");
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
            },
        };
        // SAFETY: `first` has the active quit member and contains no pointer payload.
        unsafe { handoff.push_from_callback(&first) };
        let mut first_batch = handoff.try_drain().unwrap();

        let callback_handoff = Arc::clone(&handoff);
        thread::spawn(move || {
            let second = SDL_Event {
                quit: sdl3_sys::events::SDL_QuitEvent {
                    r#type: SDL_EVENT_QUIT,
                    reserved: 0,
                    timestamp: 2,
                },
            };
            // SAFETY: `second` has the active quit member and contains no pointer payload.
            unsafe { callback_handoff.push_from_callback(&second) };
        })
        .join()
        .expect("the next callback must not wait for a retained batch");

        assert!(first_batch.pop().is_some());
        assert!(handoff.try_drain().unwrap().pop().is_some());
    }

    #[test]
    fn callback_panic_is_contained_and_reported() {
        let handoff = Sdl3CallbackEventHandoff::default();

        handoff.enqueue_with(|| panic!("synthetic callback copy failure"));

        assert!(matches!(
            handoff.try_drain(),
            Err(Sdl3CallbackEventHandoffError::CallbackPanicked)
        ));
        assert!(handoff.try_drain().unwrap().is_empty());
    }

    #[test]
    fn independent_callback_failures_are_reported_in_fifo_order() {
        let handoff = Sdl3CallbackEventHandoff::default();

        handoff.enqueue_with(|| panic!("first synthetic callback copy failure"));
        handoff.enqueue_with(|| panic!("second synthetic callback copy failure"));

        assert!(matches!(
            handoff.try_drain(),
            Err(Sdl3CallbackEventHandoffError::CallbackPanicked)
        ));
        assert!(matches!(
            handoff.try_drain(),
            Err(Sdl3CallbackEventHandoffError::CallbackPanicked)
        ));
        assert!(handoff.try_drain().unwrap().is_empty());
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
        assert!(matches!(
            handoff.try_drain(),
            Err(Sdl3CallbackEventHandoffError::CallbackPanicked)
        ));
    }

    #[test]
    fn checked_drain_reports_fault_without_discarding_later_events() {
        let handoff = Sdl3CallbackEventHandoff::default();
        handoff.enqueue_with(|| panic!("synthetic callback copy failure"));
        let raw = SDL_Event {
            quit: sdl3_sys::events::SDL_QuitEvent {
                r#type: SDL_EVENT_QUIT,
                reserved: 0,
                timestamp: 42,
            },
        };
        // SAFETY: `raw` has the active quit member and contains no pointer payload.
        unsafe { handoff.push_from_callback(&raw) };

        assert!(matches!(
            handoff.try_drain(),
            Err(Sdl3CallbackEventHandoffError::CallbackPanicked)
        ));
        let event = handoff
            .try_drain()
            .expect("the retained queue must remain drainable")
            .pop()
            .expect("the valid event must be retained");
        assert!(event.requests_exit(1));
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

        assert!(matches!(
            handoff.try_drain(),
            Err(Sdl3CallbackEventHandoffError::QueuePoisoned)
        ));
        assert!(handoff.try_drain().unwrap().is_empty());
    }
}

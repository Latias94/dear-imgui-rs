//! Safe event handoff for SDL3 main-callback examples.
//!
//! SDL can invoke `SDL_AppEvent` on a worker thread, while Dear ImGui, SDL video, and every
//! renderer in these examples must stay on the main thread. This queue copies the high-level
//! event immediately and preserves the low-level payload Dear ImGui consumes until
//! `SDL_AppIterate` replays it on the main thread.

use std::collections::VecDeque;
use std::ffi::CString;

use sdl3::event::{Event, WindowEvent};
use sdl3::keyboard::Keycode;
use sdl3::sys::events::{
    SDL_DisplayEvent, SDL_EVENT_DISPLAY_ADDED, SDL_EVENT_DISPLAY_CONTENT_SCALE_CHANGED,
    SDL_EVENT_DISPLAY_MOVED, SDL_EVENT_DISPLAY_ORIENTATION, SDL_EVENT_DISPLAY_REMOVED,
    SDL_EVENT_DISPLAY_USABLE_BOUNDS_CHANGED, SDL_EVENT_GAMEPAD_ADDED, SDL_EVENT_GAMEPAD_REMOVED,
    SDL_EVENT_KEY_DOWN, SDL_EVENT_KEY_UP, SDL_EVENT_MOUSE_BUTTON_DOWN, SDL_EVENT_MOUSE_BUTTON_UP,
    SDL_EVENT_MOUSE_MOTION, SDL_EVENT_MOUSE_WHEEL, SDL_EVENT_TEXT_INPUT,
    SDL_EVENT_WINDOW_CLOSE_REQUESTED, SDL_EVENT_WINDOW_FOCUS_GAINED, SDL_EVENT_WINDOW_FOCUS_LOST,
    SDL_EVENT_WINDOW_MOUSE_ENTER, SDL_EVENT_WINDOW_MOUSE_LEAVE, SDL_EVENT_WINDOW_MOVED,
    SDL_EVENT_WINDOW_RESIZED, SDL_Event, SDL_GamepadDeviceEvent, SDL_KeyboardEvent,
    SDL_MouseButtonEvent, SDL_MouseMotionEvent, SDL_MouseWheelEvent, SDL_TextInputEvent,
    SDL_WindowEvent,
};
#[cfg(test)]
use sdl3::sys::video::SDL_DisplayID;
use sdl3::sys::video::SDL_WindowID;

/// Prefer a responsive frame cadence while a native platform modal loop drives callbacks.
pub fn configure_main_callback_rate() {
    sdl3::hint::set_with_priority("SDL_MAIN_CALLBACK_RATE", "120", &sdl3::hint::Hint::Default);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum QueuedAppEvent {
    Quit,
    Escape,
    WindowCloseRequested { window_id: u32 },
    WindowPixelSizeChanged { window_id: u32 },
    Other,
}

impl QueuedAppEvent {
    fn from_event(event: &Event) -> Self {
        match event {
            Event::Quit { .. } => Self::Quit,
            Event::KeyDown {
                keycode: Some(Keycode::Escape),
                ..
            } => Self::Escape,
            Event::Window {
                window_id,
                win_event: WindowEvent::CloseRequested,
                ..
            } => Self::WindowCloseRequested {
                window_id: *window_id,
            },
            Event::Window {
                window_id,
                win_event: WindowEvent::PixelSizeChanged(_, _),
                ..
            } => Self::WindowPixelSizeChanged {
                window_id: *window_id,
            },
            _ => Self::Other,
        }
    }
}

/// A callback-thread event whose application state and ImGui payload are fully owned.
pub struct QueuedSdl3Event {
    app_event: QueuedAppEvent,
    imgui_event: Option<QueuedImGuiEvent>,
}

enum QueuedImGuiEvent {
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

impl QueuedSdl3Event {
    fn from_raw(raw: &SDL_Event) -> Self {
        // Convert while SDL owns the event. The resulting enum is reduced to a fully owned
        // application summary before returning, so pointer-bearing `SDL_Event` variants never
        // cross the callback-thread boundary.
        let event = Event::from_ll(*raw);
        let app_event = QueuedAppEvent::from_event(&event);
        let imgui_event = match raw.event_type() {
            SDL_EVENT_MOUSE_MOTION => Some(QueuedImGuiEvent::MouseMotion(unsafe { raw.motion })),
            SDL_EVENT_MOUSE_WHEEL => Some(QueuedImGuiEvent::MouseWheel(unsafe { raw.wheel })),
            SDL_EVENT_MOUSE_BUTTON_DOWN | SDL_EVENT_MOUSE_BUTTON_UP => {
                Some(QueuedImGuiEvent::MouseButton(unsafe { raw.button }))
            }
            SDL_EVENT_TEXT_INPUT => match &event {
                Event::TextInput {
                    timestamp,
                    window_id,
                    text,
                } => Some(QueuedImGuiEvent::TextInput {
                    timestamp: *timestamp,
                    window_id: SDL_WindowID(*window_id),
                    // SDL text input is a C string, so this cannot contain an interior NUL.
                    text: CString::new(text.as_bytes())
                        .expect("SDL text input must not contain an interior NUL"),
                }),
                _ => None,
            },
            SDL_EVENT_KEY_DOWN | SDL_EVENT_KEY_UP => {
                Some(QueuedImGuiEvent::Keyboard(unsafe { raw.key }))
            }
            SDL_EVENT_DISPLAY_ORIENTATION
            | SDL_EVENT_DISPLAY_ADDED
            | SDL_EVENT_DISPLAY_REMOVED
            | SDL_EVENT_DISPLAY_MOVED
            | SDL_EVENT_DISPLAY_CONTENT_SCALE_CHANGED
            | SDL_EVENT_DISPLAY_USABLE_BOUNDS_CHANGED => {
                Some(QueuedImGuiEvent::Display(unsafe { raw.display }))
            }
            SDL_EVENT_WINDOW_MOUSE_ENTER
            | SDL_EVENT_WINDOW_MOUSE_LEAVE
            | SDL_EVENT_WINDOW_FOCUS_GAINED
            | SDL_EVENT_WINDOW_FOCUS_LOST
            | SDL_EVENT_WINDOW_CLOSE_REQUESTED
            | SDL_EVENT_WINDOW_MOVED
            | SDL_EVENT_WINDOW_RESIZED => Some(QueuedImGuiEvent::Window(unsafe { raw.window })),
            SDL_EVENT_GAMEPAD_ADDED | SDL_EVENT_GAMEPAD_REMOVED => {
                Some(QueuedImGuiEvent::GamepadDevice(unsafe { raw.gdevice }))
            }
            _ => None,
        };

        Self {
            app_event,
            imgui_event,
        }
    }

    /// Replay the low-level event consumed by the Dear ImGui SDL3 backend.
    ///
    /// Events that the backend does not consume invoke `callback` with `None`. The reconstructed
    /// `SDL_Event` remains valid only for the duration of the callback.
    pub fn with_imgui_event<R>(&self, callback: impl FnOnce(Option<&SDL_Event>) -> R) -> R {
        match &self.imgui_event {
            Some(QueuedImGuiEvent::MouseMotion(event)) => {
                let raw = SDL_Event { motion: *event };
                callback(Some(&raw))
            }
            Some(QueuedImGuiEvent::MouseWheel(event)) => {
                let raw = SDL_Event { wheel: *event };
                callback(Some(&raw))
            }
            Some(QueuedImGuiEvent::MouseButton(event)) => {
                let raw = SDL_Event { button: *event };
                callback(Some(&raw))
            }
            Some(QueuedImGuiEvent::TextInput {
                timestamp,
                window_id,
                text,
            }) => {
                let raw = SDL_Event {
                    text: SDL_TextInputEvent {
                        r#type: SDL_EVENT_TEXT_INPUT,
                        reserved: 0,
                        timestamp: *timestamp,
                        windowID: *window_id,
                        text: text.as_ptr(),
                    },
                };
                callback(Some(&raw))
            }
            Some(QueuedImGuiEvent::Keyboard(event)) => {
                let raw = SDL_Event { key: *event };
                callback(Some(&raw))
            }
            Some(QueuedImGuiEvent::Display(event)) => {
                let raw = SDL_Event { display: *event };
                callback(Some(&raw))
            }
            Some(QueuedImGuiEvent::Window(event)) => {
                let raw = SDL_Event { window: *event };
                callback(Some(&raw))
            }
            Some(QueuedImGuiEvent::GamepadDevice(event)) => {
                let raw = SDL_Event { gdevice: *event };
                callback(Some(&raw))
            }
            None => callback(None),
        }
    }

    /// Whether this event invalidates the main window's drawable surface size.
    pub fn is_pixel_size_changed_for(&self, window_id: u32) -> bool {
        self.app_event == QueuedAppEvent::WindowPixelSizeChanged { window_id }
    }
}

/// FIFO handoff from `SDL_AppEvent` to `SDL_AppIterate`.
#[derive(Default)]
pub struct Sdl3CallbackEventQueue {
    events: VecDeque<QueuedSdl3Event>,
}

impl Sdl3CallbackEventQueue {
    /// Copy one callback event while SDL's transient payload pointers remain valid.
    pub fn push(&mut self, raw: &SDL_Event) {
        self.events.push_back(QueuedSdl3Event::from_raw(raw));
    }

    /// Take the next event for main-thread processing.
    pub fn pop(&mut self) -> Option<QueuedSdl3Event> {
        self.events.pop_front()
    }
}

/// Whether an application should leave its callback loop after this event.
pub fn requests_exit(event: &QueuedSdl3Event, main_window_id: u32) -> bool {
    match event.app_event {
        QueuedAppEvent::Quit | QueuedAppEvent::Escape => true,
        QueuedAppEvent::WindowCloseRequested { window_id } => window_id == main_window_id,
        QueuedAppEvent::WindowPixelSizeChanged { .. } | QueuedAppEvent::Other => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;
    use std::thread;

    #[test]
    fn text_input_is_owned_before_the_callback_returns() {
        let mut queue = thread::spawn(|| {
            let source = CString::new("queued text").unwrap();
            let raw = SDL_Event {
                text: SDL_TextInputEvent {
                    r#type: SDL_EVENT_TEXT_INPUT,
                    reserved: 0,
                    timestamp: 42,
                    windowID: SDL_WindowID(7),
                    text: source.as_ptr(),
                },
            };
            let mut queue = Sdl3CallbackEventQueue::default();
            queue.push(&raw);
            drop(source);
            queue
        })
        .join()
        .expect("callback worker must not panic");

        let queued = queue.pop().expect("event must be queued");
        queued.with_imgui_event(|raw| {
            let raw = raw.expect("text input is consumed by the ImGui backend");
            let text = unsafe { CStr::from_ptr(raw.text.text) };
            assert_eq!(text.to_bytes(), b"queued text");
        });
    }

    #[test]
    fn drop_payload_is_not_replayed_after_the_callback_returns() {
        let mut queue = thread::spawn(|| {
            let source = CString::new("file manager").unwrap();
            let data = CString::new("C:/tmp/queued.txt").unwrap();
            let raw = SDL_Event {
                drop: sdl3::sys::events::SDL_DropEvent {
                    r#type: sdl3::sys::events::SDL_EVENT_DROP_FILE,
                    reserved: 0,
                    timestamp: 42,
                    windowID: SDL_WindowID(7),
                    x: 0.0,
                    y: 0.0,
                    source: source.as_ptr(),
                    data: data.as_ptr(),
                },
            };
            let mut queue = Sdl3CallbackEventQueue::default();
            queue.push(&raw);
            drop((source, data));
            queue
        })
        .join()
        .expect("callback worker must not panic");

        let queued = queue.pop().expect("event must be queued");
        queued.with_imgui_event(|raw| assert!(raw.is_none()));
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
        let mut queue = Sdl3CallbackEventQueue::default();
        queue.push(&raw);

        let queued = queue.pop().expect("event must be queued");
        queued.with_imgui_event(|raw| {
            assert!(
                raw.expect("display changes are consumed by the ImGui backend")
                    .event_type()
                    == SDL_EVENT_DISPLAY_USABLE_BOUNDS_CHANGED
            );
        });
    }

    #[test]
    fn pixel_size_changes_are_preserved_for_surface_reconfiguration() {
        let raw = SDL_Event {
            window: SDL_WindowEvent {
                r#type: sdl3::sys::events::SDL_EVENT_WINDOW_PIXEL_SIZE_CHANGED,
                reserved: 0,
                timestamp: 42,
                windowID: SDL_WindowID(7),
                data1: 1920,
                data2: 1080,
            },
        };
        let mut queue = Sdl3CallbackEventQueue::default();
        queue.push(&raw);

        assert!(
            queue
                .pop()
                .expect("event must be queued")
                .is_pixel_size_changed_for(7)
        );
    }

    #[test]
    fn exit_requests_are_limited_to_the_main_window() {
        let escape = QueuedSdl3Event {
            app_event: QueuedAppEvent::Escape,
            imgui_event: None,
        };
        let secondary_close = QueuedSdl3Event {
            app_event: QueuedAppEvent::WindowCloseRequested { window_id: 2 },
            imgui_event: None,
        };
        assert!(requests_exit(&escape, 1));
        assert!(!requests_exit(&secondary_close, 1));
    }

    #[test]
    fn callback_events_can_cross_from_a_worker_thread() {
        let mut queue = thread::spawn(|| {
            let raw = SDL_Event {
                quit: sdl3::sys::events::SDL_QuitEvent {
                    r#type: sdl3::sys::events::SDL_EVENT_QUIT,
                    reserved: 0,
                    timestamp: 42,
                },
            };
            let mut queue = Sdl3CallbackEventQueue::default();
            queue.push(&raw);
            queue
        })
        .join()
        .expect("callback worker must not panic");

        let event = queue.pop().expect("event must cross to the main thread");
        assert!(requests_exit(&event, 1));
    }
}

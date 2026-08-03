use std::ffi::CString;

use sdl3::event::{DisplayEvent, Event, WindowEvent};
use sdl3_sys::events::{
    SDL_DisplayEvent, SDL_EVENT_DISPLAY_USABLE_BOUNDS_CHANGED, SDL_EVENT_TEXT_INPUT, SDL_Event,
    SDL_TextInputEvent,
};
use sdl3_sys::video::SDL_WindowID;

use super::{Sdl3BackendError, ffi};

fn with_imgui_event<R>(
    event: &Event,
    callback: impl FnOnce(&SDL_Event) -> R,
) -> Result<Option<R>, Sdl3BackendError> {
    match event {
        Event::TextInput {
            timestamp,
            window_id,
            text,
        } => {
            let text = CString::new(text.as_bytes())
                .map_err(|_| Sdl3BackendError::TextInputContainsNul)?;
            let raw = SDL_Event {
                text: SDL_TextInputEvent {
                    r#type: SDL_EVENT_TEXT_INPUT,
                    reserved: 0,
                    timestamp: *timestamp,
                    windowID: SDL_WindowID(*window_id),
                    text: text.as_ptr(),
                },
            };
            Ok(Some(callback(&raw)))
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
            let raw = SDL_Event {
                display: SDL_DisplayEvent {
                    r#type: SDL_EVENT_DISPLAY_USABLE_BOUNDS_CHANGED,
                    timestamp: *timestamp,
                    ..Default::default()
                },
            };
            Ok(Some(callback(&raw)))
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
        | Event::ControllerDeviceRemoved { .. } => Ok(event.to_ll().as_ref().map(callback)),
        _ => Ok(None),
    }
}

pub(crate) fn process_owned_event(event: &Event) -> Result<bool, Sdl3BackendError> {
    with_imgui_event(event, |event| unsafe { process_raw_sys_event(event) })
        .map(|processed| processed.unwrap_or(false))
}

/// # Safety
///
/// `event` must contain the active SDL union variant named by its type. Every pointer reachable
/// from that variant must remain valid for the duration of this call. The call must execute on the
/// SDL thread while the matching Dear ImGui SDL3 backend is current.
pub(crate) unsafe fn process_raw_sys_event(event: &SDL_Event) -> bool {
    unsafe { ffi::ImGui_ImplSDL3_ProcessEvent_Rust(event) }
}

#[cfg(test)]
mod tests {
    use std::ffi::CStr;

    use super::*;

    #[test]
    fn text_input_owns_bytes_for_the_entire_raw_call() {
        let event = Event::TextInput {
            timestamp: 42,
            window_id: 7,
            text: "hello".to_owned(),
        };

        let text = with_imgui_event(&event, |raw| unsafe {
            CStr::from_ptr(raw.text.text).to_str().unwrap().to_owned()
        })
        .unwrap();

        assert_eq!(text.as_deref(), Some("hello"));
    }

    #[test]
    fn text_input_rejects_interior_nul_without_entering_ffi() {
        let event = Event::TextInput {
            timestamp: 42,
            window_id: 7,
            text: "hello\0world".to_owned(),
        };
        let mut called = false;

        let result = with_imgui_event(&event, |_| called = true);

        assert!(matches!(
            result,
            Err(Sdl3BackendError::TextInputContainsNul)
        ));
        assert!(!called);
    }

    #[test]
    fn unsupported_pointer_bearing_events_never_reach_ffi() {
        let event = Event::User {
            timestamp: 42,
            window_id: 7,
            type_: sdl3_sys::events::SDL_EVENT_USER.0,
            code: 0,
            data1: std::ptr::dangling_mut::<std::ffi::c_void>(),
            data2: std::ptr::dangling_mut::<std::ffi::c_void>(),
        };
        let mut called = false;

        let result = with_imgui_event(&event, |_| called = true).unwrap();

        assert_eq!(result, None);
        assert!(!called);
    }

    #[test]
    fn sdl_3_4_usable_bounds_event_reaches_the_safe_backend_path() {
        let event = Event::Unknown {
            timestamp: 84,
            type_: SDL_EVENT_DISPLAY_USABLE_BOUNDS_CHANGED.0,
        };

        let display = with_imgui_event(&event, |raw| unsafe { raw.display }).unwrap();

        assert_eq!(
            display.map(|event| (event.r#type.0, event.timestamp)),
            Some((SDL_EVENT_DISPLAY_USABLE_BOUNDS_CHANGED.0, 84))
        );
    }
}

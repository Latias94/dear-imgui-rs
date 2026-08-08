use sdl3::event::Event;
use sdl3_sys::events::SDL_Event;

use super::{Sdl3BackendError, Sdl3CallbackEvent, ffi};

pub(crate) fn process_owned_event(event: &Event) -> Result<bool, Sdl3BackendError> {
    Ok(Sdl3CallbackEvent::from_owned_event(event)?
        .as_ref()
        .is_some_and(process_callback_owned_event))
}

pub(crate) fn process_callback_owned_event(event: &Sdl3CallbackEvent) -> bool {
    event.with_raw_event(|event| {
        event.is_some_and(|event| {
            // SAFETY: Sdl3CallbackEvent reconstructs the active union member and retains every
            // pointer payload for this call.
            unsafe { process_raw_sys_event(event) }
        })
    })
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
    use sdl3_sys::events::SDL_EVENT_DISPLAY_USABLE_BOUNDS_CHANGED;

    #[test]
    fn text_input_owns_bytes_for_the_entire_raw_call() {
        let event = Event::TextInput {
            timestamp: 42,
            window_id: 7,
            text: "hello".to_owned(),
        };

        let text = Sdl3CallbackEvent::from_owned_event(&event)
            .unwrap()
            .expect("text input must reach Dear ImGui")
            .with_raw_event(|raw| {
                raw.map(|raw| unsafe { CStr::from_ptr(raw.text.text).to_str().unwrap().to_owned() })
            });

        assert_eq!(text.as_deref(), Some("hello"));
    }

    #[test]
    fn text_input_rejects_interior_nul_without_entering_ffi() {
        let event = Event::TextInput {
            timestamp: 42,
            window_id: 7,
            text: "hello\0world".to_owned(),
        };
        let result = Sdl3CallbackEvent::from_owned_event(&event);

        assert!(matches!(
            result,
            Err(Sdl3BackendError::TextInputContainsNul)
        ));
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
        let result = Sdl3CallbackEvent::from_owned_event(&event).unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn sdl_3_4_usable_bounds_event_reaches_the_safe_backend_path() {
        let event = Event::Unknown {
            timestamp: 84,
            type_: SDL_EVENT_DISPLAY_USABLE_BOUNDS_CHANGED.0,
        };

        let display = Sdl3CallbackEvent::from_owned_event(&event)
            .unwrap()
            .expect("usable-bounds changes must reach Dear ImGui")
            .with_raw_event(|raw| raw.map(|raw| unsafe { raw.display }));

        assert_eq!(
            display.map(|event| (event.r#type.0, event.timestamp)),
            Some((SDL_EVENT_DISPLAY_USABLE_BOUNDS_CHANGED.0, 84))
        );
    }
}

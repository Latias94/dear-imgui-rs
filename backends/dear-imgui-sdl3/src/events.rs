use super::*;

/// Poll the next SDL3 event as a low-level `SDL_Event`.
///
/// This mirrors the C++ SDL3 examples and is useful when you want to feed both
/// Dear ImGui and your own event handling from the same low-level event stream.
pub fn sdl3_poll_event_ll() -> Option<SDL_Event> {
    let mut raw = std::mem::MaybeUninit::<SDL_Event>::uninit();
    let has_event = unsafe { sdl3_sys::events::SDL_PollEvent(raw.as_mut_ptr()) };
    if has_event {
        Some(unsafe { raw.assume_init() })
    } else {
        None
    }
}

pub(crate) fn process_sys_event(event: &SDL_Event) -> bool {
    unsafe { ffi::ImGui_ImplSDL3_ProcessEvent_Rust(event) }
}

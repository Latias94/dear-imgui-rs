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

/// Process a single low-level SDL3 event with ImGui's SDL3 backend.
///
/// Returns `true` if Dear ImGui consumed the event.
///
/// This thin compatibility helper operates on Dear ImGui's current context.
/// Prefer [`process_sys_event_for_context`] or an RAII backend owner in
/// multi-context code.
pub fn process_sys_event(event: &SDL_Event) -> bool {
    unsafe { ffi::ImGui_ImplSDL3_ProcessEvent_Rust(event) }
}

/// Process a single low-level SDL3 event for a specific ImGui context.
///
/// Returns `true` if Dear ImGui consumed the event.
pub fn process_sys_event_for_context(imgui: &mut Context, event: &SDL_Event) -> bool {
    with_context(imgui, "process_sys_event_for_context()", || {
        process_sys_event(event)
    })
}

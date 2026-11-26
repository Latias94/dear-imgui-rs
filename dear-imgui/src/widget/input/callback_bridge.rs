use super::buffers::{StringInputBuffer, resize_string_input_buffer};
use super::callbacks::{HistoryDirection, InputTextCallbackHandler, TextCallbackData};
use crate::sys;
use std::any::Any;
use std::ffi::{c_char, c_int, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

struct StringCallbackState<T> {
    buffer: *mut Vec<u8>,
    handler: T,
    panic: Option<Box<dyn Any + Send>>,
}

pub(super) fn with_string_input_buffer<T: InputTextCallbackHandler>(
    target: &mut String,
    capacity_hint: Option<usize>,
    handler: T,
    build: impl FnOnce(*mut c_char, usize, sys::ImGuiInputTextCallback, *mut c_void) -> bool,
) -> bool {
    let mut input = StringInputBuffer::new(target, capacity_hint);
    let mut state = StringCallbackState {
        buffer: &raw mut input.bytes,
        handler,
        panic: None,
    };
    let size = input.bytes.len();
    let ptr = input.bytes.as_mut_ptr().cast();
    let changed = build(
        ptr,
        size,
        Some(string_callback_router::<T>),
        (&raw mut state).cast(),
    );
    drop(input);
    let panic = state.panic.take();
    drop(state);
    if let Some(payload) = panic {
        resume_unwind(payload);
    }
    changed
}

/// Continues to process native resize requests after a user callback panics.
unsafe extern "C" fn string_callback_router<T: InputTextCallbackHandler>(
    data: *mut sys::ImGuiInputTextCallbackData,
) -> c_int {
    if data.is_null() {
        return 0;
    }
    let state = unsafe { (*data).UserData.cast::<StringCallbackState<T>>() };
    let Some(state) = (unsafe { state.as_mut() }) else {
        return 0;
    };
    let event = unsafe { (*data).EventFlag };
    if event == sys::ImGuiInputTextFlags_CallbackResize as i32 {
        unsafe { resize_string_input_buffer(&mut *state.buffer, &mut *data) };
        return 0;
    }

    let fallback = c_int::from(event == sys::ImGuiInputTextFlags_CallbackCharFilter as i32);
    if state.panic.is_some() {
        return fallback;
    }
    match catch_unwind(AssertUnwindSafe(|| unsafe {
        dispatch_user_callback(&mut state.handler, data, event)
    })) {
        Ok(result) => result,
        Err(payload) => {
            state.panic = Some(payload);
            fallback
        }
    }
}

unsafe fn dispatch_user_callback<T: InputTextCallbackHandler>(
    handler: &mut T,
    data: *mut sys::ImGuiInputTextCallbackData,
    event: sys::ImGuiInputTextFlags,
) -> c_int {
    match event {
        value if value == sys::ImGuiInputTextFlags_CallbackCompletion as i32 => {
            handler.on_completion(unsafe { TextCallbackData::new(&mut *data) });
        }
        value if value == sys::ImGuiInputTextFlags_CallbackHistory as i32 => {
            let direction = if unsafe { (*data).EventKey } == sys::ImGuiKey_UpArrow {
                HistoryDirection::Up
            } else {
                HistoryDirection::Down
            };
            handler.on_history(direction, unsafe { TextCallbackData::new(&mut *data) });
        }
        value if value == sys::ImGuiInputTextFlags_CallbackAlways as i32 => {
            handler.on_always(unsafe { TextCallbackData::new(&mut *data) });
        }
        value if value == sys::ImGuiInputTextFlags_CallbackEdit as i32 => {
            handler.on_edit(unsafe { TextCallbackData::new(&mut *data) });
        }
        value if value == sys::ImGuiInputTextFlags_CallbackCharFilter as i32 => {
            let Some(character) = char::from_u32(unsafe { (*data).EventChar } as u32)
                .and_then(|character| handler.char_filter(character))
                .and_then(|character| sys::ImWchar::try_from(character as u32).ok())
            else {
                return 1;
            };
            unsafe { (*data).EventChar = character };
        }
        _ => (),
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RecordingHandler {
        char_filter_seen: Option<char>,
        history_seen: Option<HistoryDirection>,
    }

    impl InputTextCallbackHandler for RecordingHandler {
        fn char_filter(&mut self, character: char) -> Option<char> {
            self.char_filter_seen = Some(character);
            Some('z')
        }

        fn on_history(&mut self, direction: HistoryDirection, _data: TextCallbackData<'_>) {
            self.history_seen = Some(direction);
        }
    }

    #[test]
    fn string_callback_router_dispatches_char_filter_and_history() {
        let mut buffer = b"abc\0".to_vec();
        let mut state = StringCallbackState {
            buffer: &raw mut buffer,
            handler: RecordingHandler::default(),
            panic: None,
        };
        let mut data = sys::ImGuiInputTextCallbackData {
            Buf: buffer.as_mut_ptr().cast(),
            BufTextLen: 3,
            BufSize: 4,
            UserData: (&raw mut state).cast(),
            EventFlag: sys::ImGuiInputTextFlags_CallbackCharFilter as i32,
            EventChar: 'a' as sys::ImWchar,
            ..Default::default()
        };

        assert_eq!(
            unsafe { string_callback_router::<RecordingHandler>(&mut data) },
            0
        );
        assert_eq!(state.handler.char_filter_seen, Some('a'));
        assert_eq!(data.EventChar, 'z' as sys::ImWchar);

        data.EventFlag = sys::ImGuiInputTextFlags_CallbackHistory as i32;
        data.EventKey = sys::ImGuiKey_UpArrow;
        assert_eq!(
            unsafe { string_callback_router::<RecordingHandler>(&mut data) },
            0
        );
        assert_eq!(state.handler.history_seen, Some(HistoryDirection::Up));
    }

    #[test]
    fn string_callback_router_skips_user_code_but_resizes_after_panic() {
        struct PanicOnCharacter(usize);

        impl InputTextCallbackHandler for PanicOnCharacter {
            fn char_filter(&mut self, _: char) -> Option<char> {
                self.0 += 1;
                panic!("character filter panicked");
            }
        }

        let mut buffer = b"abc\0".to_vec();
        let mut state = StringCallbackState {
            buffer: &raw mut buffer,
            handler: PanicOnCharacter(0),
            panic: None,
        };
        let mut data = sys::ImGuiInputTextCallbackData {
            Buf: buffer.as_mut_ptr().cast(),
            BufTextLen: 3,
            BufSize: 4,
            UserData: (&raw mut state).cast(),
            EventFlag: sys::ImGuiInputTextFlags_CallbackCharFilter as i32,
            EventChar: 'a' as sys::ImWchar,
            ..Default::default()
        };

        assert_eq!(
            unsafe { string_callback_router::<PanicOnCharacter>(&mut data) },
            1
        );
        assert_eq!(
            unsafe { string_callback_router::<PanicOnCharacter>(&mut data) },
            1
        );
        assert_eq!(state.handler.0, 1);

        data.EventFlag = sys::ImGuiInputTextFlags_CallbackResize as i32;
        data.BufSize = 32;
        assert_eq!(
            unsafe { string_callback_router::<PanicOnCharacter>(&mut data) },
            0
        );
        assert_eq!(buffer.len(), 32);
        assert_eq!(data.Buf, buffer.as_mut_ptr().cast());
        assert_eq!(
            state.panic.unwrap().downcast_ref::<&str>().copied(),
            Some("character filter panicked")
        );
    }
}

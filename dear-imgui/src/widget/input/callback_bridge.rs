use super::buffers::resize_string_input_buffer;
use super::callbacks::{HistoryDirection, InputTextCallbackHandler, TextCallbackData};
use crate::string::ImString;
use crate::sys;
use std::ffi::{c_int, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};

pub(in crate::widget::input) struct StringResizeCallbackState {
    buffer: *mut Vec<u8>,
}

impl StringResizeCallbackState {
    pub(in crate::widget::input) fn new(buffer: &mut Vec<u8>) -> Self {
        Self {
            buffer: buffer as *mut Vec<u8>,
        }
    }

    pub(in crate::widget::input) fn user_ptr(&mut self) -> *mut c_void {
        self as *mut Self as *mut c_void
    }
}

pub(in crate::widget::input) struct StringCallbackState<T> {
    buffer: *mut Vec<u8>,
    handler: T,
}

impl<T> StringCallbackState<T> {
    pub(in crate::widget::input) fn new(buffer: &mut Vec<u8>, handler: T) -> Self {
        Self {
            buffer: buffer as *mut Vec<u8>,
            handler,
        }
    }

    pub(in crate::widget::input) fn user_ptr(&mut self) -> *mut c_void {
        self as *mut Self as *mut c_void
    }
}

pub(in crate::widget::input) extern "C" fn im_string_resize_callback(
    data: *mut sys::ImGuiInputTextCallbackData,
) -> c_int {
    resize_im_string_with_panic_message(data, "dear-imgui-rs: panic in ImString resize callback")
}

pub(in crate::widget::input) extern "C" fn im_string_multiline_resize_callback(
    data: *mut sys::ImGuiInputTextCallbackData,
) -> c_int {
    resize_im_string_with_panic_message(
        data,
        "dear-imgui-rs: panic in ImString multiline resize callback",
    )
}

pub(in crate::widget::input) extern "C" fn string_multiline_resize_callback(
    data: *mut sys::ImGuiInputTextCallbackData,
) -> c_int {
    if data.is_null() {
        return 0;
    }

    abort_on_panic(
        "dear-imgui-rs: panic in multiline InputText resize callback",
        || unsafe {
            let state = (*data).UserData as *mut StringResizeCallbackState;
            route_string_resize_callback(state, data)
        },
    )
}

pub(in crate::widget::input) extern "C" fn string_callback_router<T: InputTextCallbackHandler>(
    data: *mut sys::ImGuiInputTextCallbackData,
) -> c_int {
    route_string_callback_with_panic_message::<T>(
        data,
        "dear-imgui-rs: panic in InputText callback",
    )
}

pub(in crate::widget::input) extern "C" fn string_multiline_callback_router<
    T: InputTextCallbackHandler,
>(
    data: *mut sys::ImGuiInputTextCallbackData,
) -> c_int {
    route_string_callback_with_panic_message::<T>(
        data,
        "dear-imgui-rs: panic in InputText multiline callback",
    )
}

fn resize_im_string_with_panic_message(
    data: *mut sys::ImGuiInputTextCallbackData,
    panic_message: &str,
) -> c_int {
    if data.is_null() {
        return 0;
    }

    abort_on_panic(panic_message, || unsafe { resize_im_string(data) })
}

fn route_string_callback_with_panic_message<T: InputTextCallbackHandler>(
    data: *mut sys::ImGuiInputTextCallbackData,
    panic_message: &str,
) -> c_int {
    if data.is_null() {
        return 0;
    }

    abort_on_panic(panic_message, || unsafe {
        let state = (*data).UserData as *mut StringCallbackState<T>;
        route_string_callback(state, data)
    })
}

fn abort_on_panic(message: &str, f: impl FnOnce() -> c_int) -> c_int {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(value) => value,
        Err(_) => {
            eprintln!("{message}");
            std::process::abort();
        }
    }
}

unsafe fn resize_im_string(data: *mut sys::ImGuiInputTextCallbackData) -> c_int {
    let event_flag = unsafe { (*data).EventFlag as i32 };
    if event_flag != sys::ImGuiInputTextFlags_CallbackResize as i32 {
        return 0;
    }

    let user_data = unsafe { (*data).UserData as *mut ImString };
    let Some(im_string) = (unsafe { user_data.as_mut() }) else {
        return 0;
    };

    let requested_i32 = unsafe { (*data).BufSize };
    if requested_i32 < 0 {
        return 0;
    }

    let requested = requested_i32 as usize;
    im_string.ensure_buf_size(requested);
    unsafe {
        (*data).Buf = im_string.as_mut_ptr();
        (*data).BufDirty = true;
    }
    0
}

unsafe fn route_string_resize_callback(
    state: *mut StringResizeCallbackState,
    data: *mut sys::ImGuiInputTextCallbackData,
) -> c_int {
    let event_flag = unsafe { (*data).EventFlag as i32 };
    if event_flag != sys::ImGuiInputTextFlags_CallbackResize as i32 {
        return 0;
    }

    let Some(state) = (unsafe { state.as_mut() }) else {
        return 0;
    };
    let Some(buffer) = (unsafe { state.buffer.as_mut() }) else {
        return 0;
    };

    unsafe {
        debug_assert_eq!(buffer.as_ptr() as *const _, (*data).Buf);
        resize_string_input_buffer(buffer, (*data).BufSize, data)
    }
}

unsafe fn route_string_callback<T: InputTextCallbackHandler>(
    state: *mut StringCallbackState<T>,
    data: *mut sys::ImGuiInputTextCallbackData,
) -> c_int {
    let Some(state) = (unsafe { state.as_mut() }) else {
        return 0;
    };
    let Some(buffer) = (unsafe { state.buffer.as_mut() }) else {
        return 0;
    };

    let event_flag = unsafe { (*data).EventFlag as i32 };
    match event_flag {
        value if value == sys::ImGuiInputTextFlags_CallbackResize as i32 => unsafe {
            debug_assert_eq!(buffer.as_ptr() as *const _, (*data).Buf);
            resize_string_input_buffer(buffer, (*data).BufSize, data)
        },
        value if value == sys::ImGuiInputTextFlags_CallbackCompletion as i32 => {
            let info = unsafe { TextCallbackData::new(data) };
            state.handler.on_completion(info);
            0
        }
        value if value == sys::ImGuiInputTextFlags_CallbackHistory as i32 => {
            let dir = unsafe { history_direction(data) };
            let info = unsafe { TextCallbackData::new(data) };
            state.handler.on_history(dir, info);
            0
        }
        value if value == sys::ImGuiInputTextFlags_CallbackAlways as i32 => {
            let info = unsafe { TextCallbackData::new(data) };
            state.handler.on_always(info);
            0
        }
        value if value == sys::ImGuiInputTextFlags_CallbackEdit as i32 => {
            let info = unsafe { TextCallbackData::new(data) };
            state.handler.on_edit(info);
            0
        }
        value if value == sys::ImGuiInputTextFlags_CallbackCharFilter as i32 => {
            dispatch_char_filter(&mut state.handler, data);
            0
        }
        _ => 0,
    }
}

unsafe fn history_direction(data: *mut sys::ImGuiInputTextCallbackData) -> HistoryDirection {
    let key = unsafe { (*data).EventKey };
    if key == sys::ImGuiKey_UpArrow {
        HistoryDirection::Up
    } else {
        HistoryDirection::Down
    }
}

fn dispatch_char_filter<T: InputTextCallbackHandler>(
    handler: &mut T,
    data: *mut sys::ImGuiInputTextCallbackData,
) {
    let ch = unsafe { std::char::from_u32((*data).EventChar as u32).unwrap_or('\0') };
    let new_ch = handler.char_filter(ch).map(|c| c as u32).unwrap_or(0);
    unsafe {
        (*data).EventChar = sys::ImWchar::try_from(new_ch).unwrap_or(0 as sys::ImWchar);
    }
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
        fn char_filter(&mut self, c: char) -> Option<char> {
            self.char_filter_seen = Some(c);
            Some('z')
        }

        fn on_history(&mut self, direction: HistoryDirection, _data: TextCallbackData<'_>) {
            self.history_seen = Some(direction);
        }
    }

    fn callback_data_for_buffer(buffer: &mut [u8]) -> sys::ImGuiInputTextCallbackData {
        let mut data = sys::ImGuiInputTextCallbackData::default();
        data.Buf = buffer.as_mut_ptr().cast();
        data.BufTextLen = buffer.len().saturating_sub(1) as i32;
        data.BufSize = buffer.len() as i32;
        data
    }

    #[test]
    fn string_callback_router_dispatches_char_filter() {
        let mut buffer = b"abc\0".to_vec();
        let mut callback_state = StringCallbackState::new(&mut buffer, RecordingHandler::default());
        let mut data = callback_data_for_buffer(&mut buffer);
        data.UserData = callback_state.user_ptr();
        data.EventFlag = sys::ImGuiInputTextFlags_CallbackCharFilter as i32;
        data.EventChar = 'a' as sys::ImWchar;

        assert_eq!(string_callback_router::<RecordingHandler>(&mut data), 0);

        assert_eq!(callback_state.handler.char_filter_seen, Some('a'));
        assert_eq!(data.EventChar, 'z' as sys::ImWchar);
    }

    #[test]
    fn string_callback_router_dispatches_history_direction() {
        let mut buffer = b"abc\0".to_vec();
        let mut callback_state = StringCallbackState::new(&mut buffer, RecordingHandler::default());
        let mut data = callback_data_for_buffer(&mut buffer);
        data.UserData = callback_state.user_ptr();
        data.EventFlag = sys::ImGuiInputTextFlags_CallbackHistory as i32;
        data.EventKey = sys::ImGuiKey_UpArrow;

        assert_eq!(string_callback_router::<RecordingHandler>(&mut data), 0);

        assert_eq!(
            callback_state.handler.history_seen,
            Some(HistoryDirection::Up)
        );
    }

    #[test]
    fn string_resize_callback_updates_buffer_pointer() {
        let mut buffer = b"abc\0".to_vec();
        let mut callback_state = StringResizeCallbackState::new(&mut buffer);
        let mut data = callback_data_for_buffer(&mut buffer);
        data.UserData = callback_state.user_ptr();
        data.EventFlag = sys::ImGuiInputTextFlags_CallbackResize as i32;
        data.BufSize = 32;

        assert_eq!(string_multiline_resize_callback(&mut data), 0);

        assert_eq!(buffer.len(), 32);
        assert_eq!(data.Buf, buffer.as_mut_ptr().cast());
        assert!(data.BufDirty);
    }
}

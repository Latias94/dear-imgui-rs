use super::buffers::{
    finish_string_input_buffer, make_string_input_buffer, resize_string_input_buffer,
};
use super::callbacks::{
    HistoryDirection, InputTextCallback, InputTextCallbackHandler, TextCallbackData,
};
use super::validation::validate_input_multiline_flags;
use crate::InputTextMultilineFlags;
use crate::string::ImString;
use crate::sys;
use crate::ui::Ui;
use std::borrow::Cow;
use std::ffi::{c_int, c_void};

/// Builder for multiline text input widget
#[derive(Debug)]
#[must_use]
pub struct InputTextMultiline<'ui, 'p> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    buf: &'p mut String,
    size: [f32; 2],
    flags: InputTextMultilineFlags,
    capacity_hint: Option<usize>,
}

/// Builder for multiline text input backed by ImString (zero-copy)
#[derive(Debug)]
#[must_use]
pub struct InputTextMultilineImStr<'ui, 'p> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    buf: &'p mut ImString,
    size: [f32; 2],
    flags: InputTextMultilineFlags,
}

impl<'ui, 'p> InputTextMultilineImStr<'ui, 'p> {
    pub fn new(
        ui: &'ui Ui,
        label: impl Into<Cow<'ui, str>>,
        buf: &'p mut ImString,
        size: impl Into<[f32; 2]>,
    ) -> Self {
        Self {
            ui,
            label: label.into(),
            buf,
            size: size.into(),
            flags: InputTextMultilineFlags::NONE,
        }
    }
    pub fn flags(mut self, flags: InputTextMultilineFlags) -> Self {
        self.flags = flags;
        self
    }
    pub fn read_only(mut self, v: bool) -> Self {
        self.flags.set(InputTextMultilineFlags::READ_ONLY, v);
        self
    }
    pub fn build(self) -> bool {
        let label_ptr = self.ui.scratch_txt(self.label.as_ref());
        let buf_size = self.buf.capacity_with_nul().max(1);
        self.buf.ensure_buf_size(buf_size);
        let buf_ptr = self.buf.as_mut_ptr();
        let user_ptr = self.buf as *mut ImString as *mut c_void;
        let size_vec: sys::ImVec2 = self.size.into();

        extern "C" fn resize_cb_imstr(data: *mut sys::ImGuiInputTextCallbackData) -> c_int {
            if data.is_null() {
                return 0;
            }
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                if ((*data).EventFlag as i32) == (sys::ImGuiInputTextFlags_CallbackResize as i32) {
                    let user_data = (*data).UserData as *mut ImString;
                    if user_data.is_null() {
                        return;
                    }

                    let im = &mut *user_data;
                    let requested_i32 = (*data).BufSize;
                    if requested_i32 < 0 {
                        return;
                    }
                    let requested = requested_i32 as usize;
                    im.ensure_buf_size(requested);
                    (*data).Buf = im.as_mut_ptr();
                    (*data).BufDirty = true;
                }
            }));
            if res.is_err() {
                eprintln!("dear-imgui-rs: panic in ImString multiline resize callback");
                std::process::abort();
            }
            0
        }

        validate_input_multiline_flags("InputTextMultilineImStr::build()", self.flags);
        let flags = self.flags.raw() | sys::ImGuiInputTextFlags_CallbackResize as i32;
        let result = unsafe {
            sys::igInputTextMultiline(
                label_ptr,
                buf_ptr,
                buf_size,
                size_vec,
                flags,
                Some(resize_cb_imstr),
                user_ptr,
            )
        };
        // Ensure ImString logical length reflects actual text (scan to NUL)
        unsafe { self.buf.refresh_len() };
        result
    }
}
impl<'ui, 'p> InputTextMultiline<'ui, 'p> {
    /// Creates a new multiline text input builder
    pub fn new(
        ui: &'ui Ui,
        label: impl Into<Cow<'ui, str>>,
        buf: &'p mut String,
        size: impl Into<[f32; 2]>,
    ) -> Self {
        Self {
            ui,
            label: label.into(),
            buf,
            size: size.into(),
            flags: InputTextMultilineFlags::NONE,
            capacity_hint: None,
        }
    }

    /// Sets the flags for the input
    pub fn flags(mut self, flags: InputTextMultilineFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Hint a minimum buffer capacity to reduce reallocations for large fields
    pub fn capacity_hint(mut self, cap: usize) -> Self {
        self.capacity_hint = Some(cap);
        self
    }

    /// Makes the input read-only
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.flags
            .set(InputTextMultilineFlags::READ_ONLY, read_only);
        self
    }

    /// Builds the multiline text input widget
    pub fn build(self) -> bool {
        let label_ptr = self.ui.scratch_txt(self.label.as_ref());

        let mut input_buffer = make_string_input_buffer(self.buf, self.capacity_hint);
        let capacity = input_buffer.len();
        let buf_ptr = input_buffer.as_mut_ptr() as *mut std::os::raw::c_char;

        #[repr(C)]
        struct UserData {
            buffer: *mut Vec<u8>,
        }

        extern "C" fn callback_router(data: *mut sys::ImGuiInputTextCallbackData) -> c_int {
            if data.is_null() {
                return 0;
            }

            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                let event_flag = (*data).EventFlag as i32;
                match event_flag {
                    value if value == sys::ImGuiInputTextFlags_CallbackResize as i32 => {
                        let user_ptr = (*data).UserData as *mut UserData;
                        if user_ptr.is_null() {
                            return 0;
                        }

                        let user = &mut *user_ptr;
                        if user.buffer.is_null() {
                            return 0;
                        }
                        let buffer = &mut *user.buffer;
                        debug_assert_eq!(buffer.as_ptr() as *const _, (*data).Buf);
                        resize_string_input_buffer(buffer, (*data).BufSize, data)
                    }
                    _ => 0,
                }
            }));

            match res {
                Ok(v) => v,
                Err(_) => {
                    eprintln!("dear-imgui-rs: panic in multiline InputText resize callback");
                    std::process::abort();
                }
            }
        }

        let mut user_data = UserData {
            buffer: &mut input_buffer as *mut Vec<u8>,
        };
        let user_ptr = &mut user_data as *mut _ as *mut c_void;

        let size_vec: sys::ImVec2 = self.size.into();
        validate_input_multiline_flags("InputTextMultiline::build()", self.flags);
        let flags = self.flags.raw() | sys::ImGuiInputTextFlags_CallbackResize as i32;
        let result = unsafe {
            sys::igInputTextMultiline(
                label_ptr,
                buf_ptr,
                capacity,
                size_vec,
                flags,
                Some(callback_router),
                user_ptr,
            )
        };

        finish_string_input_buffer(self.buf, input_buffer);
        result
    }

    /// Enable ImGui callbacks for this multiline input and attach a handler.
    pub fn callback<T2: InputTextCallbackHandler>(
        mut self,
        callbacks: InputTextCallback,
        handler: T2,
    ) -> InputTextMultilineWithCb<'ui, 'p, T2> {
        // Note: ImGui forbids CallbackHistory/Completion with Multiline.
        // We intentionally do NOT enable them here to avoid assertions.
        if callbacks.contains(InputTextCallback::ALWAYS) {
            self.flags |= InputTextMultilineFlags::from_bits_retain(
                sys::ImGuiInputTextFlags_CallbackAlways as i32,
            );
        }
        if callbacks.contains(InputTextCallback::CHAR_FILTER) {
            self.flags |= InputTextMultilineFlags::from_bits_retain(
                sys::ImGuiInputTextFlags_CallbackCharFilter as i32,
            );
        }
        if callbacks.contains(InputTextCallback::EDIT) {
            self.flags |= InputTextMultilineFlags::from_bits_retain(
                sys::ImGuiInputTextFlags_CallbackEdit as i32,
            );
        }

        InputTextMultilineWithCb {
            ui: self.ui,
            label: self.label,
            buf: self.buf,
            size: self.size,
            flags: self.flags,
            capacity_hint: self.capacity_hint,
            handler,
        }
    }
}

/// Multiline InputText with attached callback handler
pub struct InputTextMultilineWithCb<'ui, 'p, T> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    buf: &'p mut String,
    size: [f32; 2],
    flags: InputTextMultilineFlags,
    capacity_hint: Option<usize>,
    handler: T,
}

impl<'ui, 'p, T: InputTextCallbackHandler> InputTextMultilineWithCb<'ui, 'p, T> {
    pub fn build(self) -> bool {
        let label_ptr = self.ui.scratch_txt(self.label.as_ref());

        let mut input_buffer = make_string_input_buffer(self.buf, self.capacity_hint);
        let capacity = input_buffer.len();
        let buf_ptr = input_buffer.as_mut_ptr() as *mut std::os::raw::c_char;

        #[repr(C)]
        struct UserData<T> {
            buffer: *mut Vec<u8>,
            handler: T,
        }

        extern "C" fn callback_router<T: InputTextCallbackHandler>(
            data: *mut sys::ImGuiInputTextCallbackData,
        ) -> c_int {
            if data.is_null() {
                return 0;
            }

            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let user_ptr = unsafe { (*data).UserData as *mut UserData<T> };
                if user_ptr.is_null() {
                    return 0;
                }
                let user = unsafe { &mut *user_ptr };
                if user.buffer.is_null() {
                    return 0;
                }

                let event_flag = unsafe { (*data).EventFlag as i32 };
                match event_flag {
                    value if value == sys::ImGuiInputTextFlags_CallbackResize as i32 => unsafe {
                        let buffer = &mut *user.buffer;
                        debug_assert_eq!(buffer.as_ptr() as *const _, (*data).Buf);
                        resize_string_input_buffer(buffer, (*data).BufSize, data)
                    },
                    value if value == sys::ImGuiInputTextFlags_CallbackCompletion as i32 => {
                        let info = unsafe { TextCallbackData::new(data) };
                        user.handler.on_completion(info);
                        0
                    }
                    value if value == sys::ImGuiInputTextFlags_CallbackHistory as i32 => {
                        let key = unsafe { (*data).EventKey };
                        let dir = if key == sys::ImGuiKey_UpArrow {
                            HistoryDirection::Up
                        } else {
                            HistoryDirection::Down
                        };
                        let info = unsafe { TextCallbackData::new(data) };
                        user.handler.on_history(dir, info);
                        0
                    }
                    value if value == InputTextMultilineFlags::CALLBACK_ALWAYS.bits() => {
                        let info = unsafe { TextCallbackData::new(data) };
                        user.handler.on_always(info);
                        0
                    }
                    value if value == InputTextMultilineFlags::CALLBACK_EDIT.bits() => {
                        let info = unsafe { TextCallbackData::new(data) };
                        user.handler.on_edit(info);
                        0
                    }
                    value if value == InputTextMultilineFlags::CALLBACK_CHAR_FILTER.bits() => {
                        let ch = unsafe {
                            std::char::from_u32((*data).EventChar as u32).unwrap_or('\0')
                        };
                        let new_ch = user.handler.char_filter(ch).map(|c| c as u32).unwrap_or(0);
                        unsafe {
                            (*data).EventChar =
                                sys::ImWchar::try_from(new_ch).unwrap_or(0 as sys::ImWchar);
                        }
                        0
                    }
                    _ => 0,
                }
            }));

            match res {
                Ok(v) => v,
                Err(_) => {
                    eprintln!("dear-imgui-rs: panic in InputText multiline callback");
                    std::process::abort();
                }
            }
        }

        let mut user_data = UserData {
            buffer: &mut input_buffer as *mut Vec<u8>,
            handler: self.handler,
        };
        let user_ptr = &mut user_data as *mut _ as *mut c_void;

        let size_vec: sys::ImVec2 = self.size.into();
        validate_input_multiline_flags("InputTextMultilineWithCb::build()", self.flags);
        let flags = self.flags.raw() | sys::ImGuiInputTextFlags_CallbackResize as i32;
        let result = unsafe {
            sys::igInputTextMultiline(
                label_ptr,
                buf_ptr,
                capacity,
                size_vec,
                flags,
                Some(callback_router::<T>),
                user_ptr,
            )
        };

        finish_string_input_buffer(self.buf, input_buffer);
        result
    }
}

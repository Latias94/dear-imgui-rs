use super::buffers::{
    finish_string_input_buffer, make_string_input_buffer, resize_string_input_buffer,
};
use super::callbacks::{
    HistoryDirection, InputTextCallback, InputTextCallbackHandler, PassthroughCallback,
    TextCallbackData,
};
use super::validation::validate_input_text_flags;
use crate::InputTextFlags;
use crate::string::ImString;
use crate::sys;
use crate::ui::Ui;
use std::borrow::Cow;
use std::ffi::{c_int, c_void};
use std::marker::PhantomData;

/// Builder for a text input widget
#[must_use]
pub struct InputText<'ui, 'p, L = Cow<'ui, str>, H = Cow<'ui, str>, T = PassthroughCallback> {
    ui: &'ui Ui,
    label: L,
    buf: &'p mut String,
    flags: InputTextFlags,
    capacity_hint: Option<usize>,
    hint: Option<H>,
    callback_handler: T,
    _phantom: PhantomData<&'ui ()>,
}

/// Builder for a text input backed by ImString (zero-copy)
#[must_use]
pub struct InputTextImStr<'ui, 'p, L = Cow<'ui, str>, H = Cow<'ui, str>, T = PassthroughCallback> {
    ui: &'ui Ui,
    label: L,
    buf: &'p mut ImString,
    flags: InputTextFlags,
    hint: Option<H>,
    callback_handler: T,
    _phantom: PhantomData<&'ui ()>,
}

impl<'ui, 'p> InputTextImStr<'ui, 'p, Cow<'ui, str>, Cow<'ui, str>, PassthroughCallback> {
    pub fn new(ui: &'ui Ui, label: impl Into<Cow<'ui, str>>, buf: &'p mut ImString) -> Self {
        Self {
            ui,
            label: label.into(),
            buf,
            flags: InputTextFlags::empty(),
            hint: None,
            callback_handler: PassthroughCallback,
            _phantom: PhantomData,
        }
    }
}

impl<'ui, 'p, L: AsRef<str>, H: AsRef<str>, T> InputTextImStr<'ui, 'p, L, H, T> {
    pub fn flags(mut self, flags: InputTextFlags) -> Self {
        self.flags = flags;
        self
    }
    pub fn hint<H2: AsRef<str>>(self, hint: H2) -> InputTextImStr<'ui, 'p, L, H2, T> {
        InputTextImStr {
            ui: self.ui,
            label: self.label,
            buf: self.buf,
            flags: self.flags,
            hint: Some(hint),
            callback_handler: self.callback_handler,
            _phantom: PhantomData,
        }
    }
    pub fn read_only(mut self, ro: bool) -> Self {
        self.flags.set(InputTextFlags::READ_ONLY, ro);
        self
    }
    pub fn password(mut self, pw: bool) -> Self {
        self.flags.set(InputTextFlags::PASSWORD, pw);
        self
    }
    pub fn auto_select_all(mut self, v: bool) -> Self {
        self.flags.set(InputTextFlags::AUTO_SELECT_ALL, v);
        self
    }
    pub fn enter_returns_true(mut self, v: bool) -> Self {
        self.flags.set(InputTextFlags::ENTER_RETURNS_TRUE, v);
        self
    }

    pub fn build(self) -> bool {
        let (label_ptr, hint_ptr) = self.ui.scratch_txt_with_opt(
            self.label.as_ref(),
            self.hint.as_ref().map(|hint| hint.as_ref()),
        );
        let buf_size = self.buf.capacity_with_nul().max(1);
        self.buf.ensure_buf_size(buf_size);
        let buf_ptr = self.buf.as_mut_ptr();
        let user_ptr = self.buf as *mut ImString as *mut c_void;

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
                eprintln!("dear-imgui-rs: panic in ImString resize callback");
                std::process::abort();
            }
            0
        }

        let flags = self.flags | InputTextFlags::CALLBACK_RESIZE;
        validate_input_text_flags("InputTextImStr::build()", flags, false);
        let result = unsafe {
            if hint_ptr.is_null() {
                sys::igInputText(
                    label_ptr,
                    buf_ptr,
                    buf_size,
                    flags.raw(),
                    Some(resize_cb_imstr),
                    user_ptr,
                )
            } else {
                sys::igInputTextWithHint(
                    label_ptr,
                    hint_ptr,
                    buf_ptr,
                    buf_size,
                    flags.raw(),
                    Some(resize_cb_imstr),
                    user_ptr,
                )
            }
        };
        // Ensure ImString logical length reflects actual text (scan to NUL)
        unsafe { self.buf.refresh_len() };
        result
    }
}
impl<'ui, 'p> InputText<'ui, 'p, Cow<'ui, str>, Cow<'ui, str>, PassthroughCallback> {
    /// Creates a new text input builder
    pub fn new(ui: &'ui Ui, label: impl Into<Cow<'ui, str>>, buf: &'p mut String) -> Self {
        Self {
            ui,
            label: label.into(),
            buf,
            flags: InputTextFlags::NONE,
            capacity_hint: None,
            hint: None,
            callback_handler: PassthroughCallback,
            _phantom: PhantomData,
        }
    }
}

impl<'ui, 'p, L, H, T> InputText<'ui, 'p, L, H, T> {
    /// Sets the flags for the input
    pub fn flags(mut self, flags: InputTextFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Hint a minimum buffer capacity to reduce reallocations for large fields
    pub fn capacity_hint(mut self, cap: usize) -> Self {
        self.capacity_hint = Some(cap);
        self
    }

    /// Sets a hint text
    pub fn hint(self, hint: impl Into<Cow<'ui, str>>) -> InputText<'ui, 'p, L, Cow<'ui, str>, T> {
        InputText {
            ui: self.ui,
            label: self.label,
            buf: self.buf,
            flags: self.flags,
            capacity_hint: self.capacity_hint,
            hint: Some(hint.into()),
            callback_handler: self.callback_handler,
            _phantom: PhantomData,
        }
    }

    /// Sets a callback handler for the input text
    pub fn callback<T2: InputTextCallbackHandler>(
        self,
        callback_handler: T2,
    ) -> InputText<'ui, 'p, L, H, T2> {
        InputText {
            ui: self.ui,
            label: self.label,
            buf: self.buf,
            flags: self.flags,
            capacity_hint: self.capacity_hint,
            hint: self.hint,
            callback_handler,
            _phantom: PhantomData,
        }
    }

    /// Sets callback flags for the input text
    pub fn callback_flags(mut self, callback_flags: InputTextCallback) -> Self {
        self.flags |= InputTextFlags::from_bits_truncate(callback_flags.bits() as i32);
        self
    }

    /// Makes the input read-only
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.flags.set(InputTextFlags::READ_ONLY, read_only);
        self
    }

    /// Enables password mode
    pub fn password(mut self, password: bool) -> Self {
        self.flags.set(InputTextFlags::PASSWORD, password);
        self
    }

    /// Enables auto-select all when first taking focus
    pub fn auto_select_all(mut self, auto_select: bool) -> Self {
        self.flags.set(InputTextFlags::AUTO_SELECT_ALL, auto_select);
        self
    }

    /// Makes Enter key return true instead of adding new line
    pub fn enter_returns_true(mut self, enter_returns: bool) -> Self {
        self.flags
            .set(InputTextFlags::ENTER_RETURNS_TRUE, enter_returns);
        self
    }

    /// Allows only decimal characters (0123456789.+-*/)
    pub fn chars_decimal(mut self, decimal: bool) -> Self {
        self.flags.set(InputTextFlags::CHARS_DECIMAL, decimal);
        self
    }

    /// Allows only hexadecimal characters (0123456789ABCDEFabcdef)
    pub fn chars_hexadecimal(mut self, hex: bool) -> Self {
        self.flags.set(InputTextFlags::CHARS_HEXADECIMAL, hex);
        self
    }

    /// Turns a..z into A..Z
    pub fn chars_uppercase(mut self, uppercase: bool) -> Self {
        self.flags.set(InputTextFlags::CHARS_UPPERCASE, uppercase);
        self
    }

    /// Filters out spaces and tabs
    pub fn chars_no_blank(mut self, no_blank: bool) -> Self {
        self.flags.set(InputTextFlags::CHARS_NO_BLANK, no_blank);
        self
    }
}

// Implementation for all InputText types
impl<'ui, 'p, L, H, T> InputText<'ui, 'p, L, H, T>
where
    L: AsRef<str>,
    H: AsRef<str>,
    T: InputTextCallbackHandler,
{
    /// Builds the text input widget
    pub fn build(self) -> bool {
        let (label_ptr, hint_ptr) = self.ui.scratch_txt_with_opt(
            self.label.as_ref(),
            self.hint.as_ref().map(|hint| hint.as_ref()),
        );

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

                let event_flag =
                    unsafe { InputTextFlags::from_bits_truncate((*data).EventFlag as i32) };
                match event_flag {
                    InputTextFlags::CALLBACK_RESIZE => unsafe {
                        let buffer = &mut *user.buffer;
                        debug_assert_eq!(buffer.as_ptr() as *const _, (*data).Buf);
                        resize_string_input_buffer(buffer, (*data).BufSize, data)
                    },
                    InputTextFlags::CALLBACK_COMPLETION => {
                        let info = unsafe { TextCallbackData::new(data) };
                        user.handler.on_completion(info);
                        0
                    }
                    InputTextFlags::CALLBACK_HISTORY => {
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
                    InputTextFlags::CALLBACK_ALWAYS => {
                        let info = unsafe { TextCallbackData::new(data) };
                        user.handler.on_always(info);
                        0
                    }
                    InputTextFlags::CALLBACK_EDIT => {
                        let info = unsafe { TextCallbackData::new(data) };
                        user.handler.on_edit(info);
                        0
                    }
                    InputTextFlags::CALLBACK_CHAR_FILTER => {
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
                    eprintln!("dear-imgui-rs: panic in InputText callback");
                    std::process::abort();
                }
            }
        }

        let mut user_data = UserData {
            buffer: &mut input_buffer as *mut Vec<u8>,
            handler: self.callback_handler,
        };
        let user_ptr = &mut user_data as *mut _ as *mut c_void;

        let flags = self.flags | InputTextFlags::CALLBACK_RESIZE;
        validate_input_text_flags("InputText::build()", flags, false);
        let result = unsafe {
            if hint_ptr.is_null() {
                sys::igInputText(
                    label_ptr,
                    buf_ptr,
                    capacity,
                    flags.raw(),
                    Some(callback_router::<T>),
                    user_ptr,
                )
            } else {
                sys::igInputTextWithHint(
                    label_ptr,
                    hint_ptr,
                    buf_ptr,
                    capacity,
                    flags.raw(),
                    Some(callback_router::<T>),
                    user_ptr,
                )
            }
        };

        finish_string_input_buffer(self.buf, input_buffer);
        result
    }
}

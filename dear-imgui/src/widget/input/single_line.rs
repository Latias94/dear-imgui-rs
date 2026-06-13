use super::buffers::{finish_string_input_buffer, make_string_input_buffer};
use super::callback_bridge::{
    StringCallbackState, im_string_resize_callback, string_callback_router,
};
use super::callbacks::{InputTextCallback, InputTextCallbackHandler, PassthroughCallback};
use super::validation::validate_input_text_flags;
use crate::InputTextFlags;
use crate::string::ImString;
use crate::sys;
use crate::ui::Ui;
use std::borrow::Cow;
use std::ffi::c_void;
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

        validate_input_text_flags("InputTextImStr::build()", self.flags);
        let flags = self.flags.raw() | sys::ImGuiInputTextFlags_CallbackResize as i32;
        let result = self.ui.run_with_bound_context(|| unsafe {
            if hint_ptr.is_null() {
                sys::igInputText(
                    label_ptr,
                    buf_ptr,
                    buf_size,
                    flags,
                    Some(im_string_resize_callback),
                    user_ptr,
                )
            } else {
                sys::igInputTextWithHint(
                    label_ptr,
                    hint_ptr,
                    buf_ptr,
                    buf_size,
                    flags,
                    Some(im_string_resize_callback),
                    user_ptr,
                )
            }
        });
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
        self.flags |= InputTextFlags::from_bits_retain(callback_flags.bits() as i32);
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

        let mut callback_state = StringCallbackState::new(&mut input_buffer, self.callback_handler);
        let user_ptr = callback_state.user_ptr();

        validate_input_text_flags("InputText::build()", self.flags);
        let flags = self.flags.raw() | sys::ImGuiInputTextFlags_CallbackResize as i32;
        let result = self.ui.run_with_bound_context(|| unsafe {
            if hint_ptr.is_null() {
                sys::igInputText(
                    label_ptr,
                    buf_ptr,
                    capacity,
                    flags,
                    Some(string_callback_router::<T>),
                    user_ptr,
                )
            } else {
                sys::igInputTextWithHint(
                    label_ptr,
                    hint_ptr,
                    buf_ptr,
                    capacity,
                    flags,
                    Some(string_callback_router::<T>),
                    user_ptr,
                )
            }
        });

        finish_string_input_buffer(self.buf, input_buffer);
        result
    }
}

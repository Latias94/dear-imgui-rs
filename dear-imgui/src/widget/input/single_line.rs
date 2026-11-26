use super::callback_bridge::with_string_input_buffer;
use super::callbacks::{InputTextCallback, InputTextCallbackHandler, PassthroughCallback};
use super::validation::validate_input_text_flags;
use crate::InputTextFlags;
use crate::sys;
use crate::ui::Ui;
use std::borrow::Cow;

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
        validate_input_text_flags("InputText::build()", self.flags);
        let flags = self.flags.raw() | sys::ImGuiInputTextFlags_CallbackResize as i32;
        with_string_input_buffer(
            self.buf,
            self.capacity_hint,
            self.callback_handler,
            |buf, size, callback, user_data| {
                self.ui.with_scratch_buffer(|scratch| {
                    let (label, hint) = scratch.scratch_txt_with_opt(self.label, self.hint);
                    self.ui.run_input_widget(label, || unsafe {
                        if hint.is_null() {
                            sys::igInputText(label, buf, size, flags, callback, user_data)
                        } else {
                            sys::igInputTextWithHint(
                                label, hint, buf, size, flags, callback, user_data,
                            )
                        }
                    })
                })
            },
        )
    }
}

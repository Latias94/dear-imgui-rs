use super::buffers::{finish_string_input_buffer, make_string_input_buffer};
use super::callback_bridge::{
    StringCallbackState, StringResizeCallbackState, im_string_multiline_resize_callback,
    string_multiline_callback_router, string_multiline_resize_callback,
};
use super::callbacks::{InputTextCallback, InputTextCallbackHandler};
use super::validation::validate_input_multiline_flags;
use crate::InputTextMultilineFlags;
use crate::string::ImString;
use crate::sys;
use crate::ui::Ui;
use std::borrow::Cow;
use std::ffi::c_void;

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

        validate_input_multiline_flags("InputTextMultilineImStr::build()", self.flags);
        let flags = self.flags.raw() | sys::ImGuiInputTextFlags_CallbackResize as i32;
        let result = unsafe {
            sys::igInputTextMultiline(
                label_ptr,
                buf_ptr,
                buf_size,
                size_vec,
                flags,
                Some(im_string_multiline_resize_callback),
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

        let mut callback_state = StringResizeCallbackState::new(&mut input_buffer);
        let user_ptr = callback_state.user_ptr();

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
                Some(string_multiline_resize_callback),
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

        let mut callback_state = StringCallbackState::new(&mut input_buffer, self.handler);
        let user_ptr = callback_state.user_ptr();

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
                Some(string_multiline_callback_router::<T>),
                user_ptr,
            )
        };

        finish_string_input_buffer(self.buf, input_buffer);
        result
    }
}

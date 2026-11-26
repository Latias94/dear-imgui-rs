use super::callback_bridge::with_string_input_buffer;
use super::callbacks::{InputTextCallback, InputTextCallbackHandler, PassthroughCallback};
use super::validation::validate_input_multiline_flags;
use crate::InputTextMultilineFlags;
use crate::sys;
use crate::ui::Ui;
use std::borrow::Cow;

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
        validate_input_multiline_flags("InputTextMultiline::build()", self.flags);
        self.build_with_handler(PassthroughCallback)
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
            input: self,
            handler,
        }
    }

    fn build_with_handler(self, handler: impl InputTextCallbackHandler) -> bool {
        let widget_size: sys::ImVec2 = self.size.into();
        let flags = self.flags.raw() | sys::ImGuiInputTextFlags_CallbackResize as i32;
        with_string_input_buffer(
            self.buf,
            self.capacity_hint,
            handler,
            |buf, size, callback, user_data| {
                self.ui.with_scratch_buffer(|scratch| {
                    let label = scratch.scratch_txt(self.label);
                    self.ui.run_input_widget(label, || unsafe {
                        sys::igInputTextMultiline(
                            label,
                            buf,
                            size,
                            widget_size,
                            flags,
                            callback,
                            user_data,
                        )
                    })
                })
            },
        )
    }
}

/// Multiline InputText with attached callback handler
pub struct InputTextMultilineWithCb<'ui, 'p, T> {
    input: InputTextMultiline<'ui, 'p>,
    handler: T,
}

impl<'ui, 'p, T: InputTextCallbackHandler> InputTextMultilineWithCb<'ui, 'p, T> {
    pub fn build(self) -> bool {
        validate_input_multiline_flags("InputTextMultilineWithCb::build()", self.input.flags);
        self.input.build_with_handler(self.handler)
    }
}

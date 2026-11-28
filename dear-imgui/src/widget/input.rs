//! Text and scalar inputs
//!
//! Single-line and multiline text inputs backed by `String`, plus numeric
//! inputs. Text input builders provide flags and callback hooks.
//!
//! Quick examples:
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! // Text
//! let mut s = String::from("hello");
//! ui.input_text("Name", &mut s).build();
//!
//! // Numbers
//! let mut i = 0i32;
//! let mut f = 1.0f32;
//! ui.input_int("Count", &mut i);
//! ui.input_float("Scale", &mut f);
//! ```
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]
// NOTE: Keep explicit `as i32`/`as u32` casts when bridging bindgen-generated flags into the
// Dear ImGui C ABI. Bindgen may represent the same C enum/typedef with different Rust integer
// types across platforms/toolchains; our wrappers intentionally pin the expected width/sign at
// the FFI call sites.

use crate::internal::{DataTypeKind, component_count_i32};
use crate::{
    ContextError, InputScalarFlags, InputTextFlags, InputTextMultilineFlags, NumericFormat, Ui, sys,
};
use std::any::Any;
use std::borrow::Cow;
use std::ffi::{c_char, c_int, c_void};
use std::mem;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::ptr;
use std::str::Utf8Error;

/// # Input Widgets
impl Ui {
    /// Creates a single-line text input widget builder.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let mut text = String::new();
    /// if ui.input_text("Label", &mut text).build() {
    ///     println!("Text changed: {}", text);
    /// }
    /// ```
    #[doc(alias = "InputText", alias = "InputTextWithHint")]
    pub fn input_text<'ui, 'p>(
        &'ui self,
        label: impl Into<Cow<'ui, str>>,
        buf: &'p mut String,
    ) -> InputText<'ui, 'p> {
        InputText::new(self, label, buf)
    }

    /// Creates a multi-line text input widget builder.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let mut text = String::new();
    /// if ui.input_text_multiline("Label", &mut text, [200.0, 100.0]).build() {
    ///     println!("Text changed: {}", text);
    /// }
    /// ```
    #[doc(alias = "InputTextMultiline")]
    pub fn input_text_multiline<'ui, 'p>(
        &'ui self,
        label: impl Into<Cow<'ui, str>>,
        buf: &'p mut String,
        size: impl Into<[f32; 2]>,
    ) -> InputTextMultiline<'ui, 'p> {
        InputTextMultiline::new(self, label, buf, size)
    }

    /// Creates an integer input widget.
    ///
    /// Returns true if the value was edited.
    #[doc(alias = "InputInt")]
    pub fn input_int(&self, label: impl AsRef<str>, value: &mut i32) -> bool {
        self.input_int_config(label.as_ref()).build(value)
    }

    /// Creates a float input widget.
    ///
    /// Returns true if the value was edited.
    #[doc(alias = "InputFloat")]
    pub fn input_float(&self, label: impl AsRef<str>, value: &mut f32) -> bool {
        self.input_float_config(label.as_ref()).build(value)
    }

    /// Creates a double input widget.
    ///
    /// Returns true if the value was edited.
    #[doc(alias = "InputDouble")]
    pub fn input_double(&self, label: impl AsRef<str>, value: &mut f64) -> bool {
        self.input_double_config(label.as_ref()).build(value)
    }

    /// Creates an integer input builder
    pub fn input_int_config<'ui>(&'ui self, label: impl Into<Cow<'ui, str>>) -> InputInt<'ui> {
        InputInt::new(self, label)
    }

    /// Creates a float input builder
    pub fn input_float_config<'ui>(&'ui self, label: impl Into<Cow<'ui, str>>) -> InputFloat<'ui> {
        InputFloat::new(self, label)
    }

    /// Creates a double input builder
    pub fn input_double_config<'ui>(
        &'ui self,
        label: impl Into<Cow<'ui, str>>,
    ) -> InputDouble<'ui> {
        InputDouble::new(self, label)
    }

    /// Shows an input field for a scalar value. This is not limited to `f32` and `i32` and can be used with
    /// any primitive scalar type e.g. `u8` and `f64`.
    #[doc(alias = "InputScalar")]
    pub fn input_scalar<'p, L, T>(&self, label: L, value: &'p mut T) -> InputScalar<'_, 'p, T, L>
    where
        L: AsRef<str>,
        T: DataTypeKind,
    {
        InputScalar::new(self, label, value)
    }

    /// Shows a horizontal array of scalar value input fields. See [`input_scalar`].
    ///
    /// [`input_scalar`]: Self::input_scalar
    #[doc(alias = "InputScalarN")]
    pub fn input_scalar_n<'p, L, T>(
        &self,
        label: L,
        values: &'p mut [T],
    ) -> InputScalarN<'_, 'p, T, L>
    where
        L: AsRef<str>,
        T: DataTypeKind,
    {
        InputScalarN::new(self, label, values)
    }

    /// Widget to edit two floats
    #[doc(alias = "InputFloat2")]
    pub fn input_float2<'p, L>(&self, label: L, value: &'p mut [f32; 2]) -> InputFloat2<'_, 'p, L>
    where
        L: AsRef<str>,
    {
        InputFloat2::new(self, label, value)
    }

    /// Widget to edit three floats
    #[doc(alias = "InputFloat3")]
    pub fn input_float3<'p, L>(&self, label: L, value: &'p mut [f32; 3]) -> InputFloat3<'_, 'p, L>
    where
        L: AsRef<str>,
    {
        InputFloat3::new(self, label, value)
    }

    /// Widget to edit four floats
    #[doc(alias = "InputFloat4")]
    pub fn input_float4<'p, L>(&self, label: L, value: &'p mut [f32; 4]) -> InputFloat4<'_, 'p, L>
    where
        L: AsRef<str>,
    {
        InputFloat4::new(self, label, value)
    }

    /// Widget to edit two integers
    #[doc(alias = "InputInt2")]
    pub fn input_int2<'p, L>(&self, label: L, value: &'p mut [i32; 2]) -> InputInt2<'_, 'p, L>
    where
        L: AsRef<str>,
    {
        InputInt2::new(self, label, value)
    }

    /// Widget to edit three integers
    #[doc(alias = "InputInt3")]
    pub fn input_int3<'p, L>(&self, label: L, value: &'p mut [i32; 3]) -> InputInt3<'_, 'p, L>
    where
        L: AsRef<str>,
    {
        InputInt3::new(self, label, value)
    }

    /// Widget to edit four integers
    #[doc(alias = "InputInt4")]
    pub fn input_int4<'p, L>(&self, label: L, value: &'p mut [i32; 4]) -> InputInt4<'_, 'p, L>
    where
        L: AsRef<str>,
    {
        InputInt4::new(self, label, value)
    }
}

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
        let flags = self.flags.raw() | sys::ImGuiInputTextFlags_CallbackResize;
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
            self.flags |=
                InputTextMultilineFlags::from_bits_retain(sys::ImGuiInputTextFlags_CallbackAlways);
        }
        if callbacks.contains(InputTextCallback::CHAR_FILTER) {
            self.flags |= InputTextMultilineFlags::from_bits_retain(
                sys::ImGuiInputTextFlags_CallbackCharFilter,
            );
        }
        if callbacks.contains(InputTextCallback::EDIT) {
            self.flags |=
                InputTextMultilineFlags::from_bits_retain(sys::ImGuiInputTextFlags_CallbackEdit);
        }

        InputTextMultilineWithCb {
            input: self,
            handler,
        }
    }

    fn build_with_handler(self, handler: impl InputTextCallbackHandler) -> bool {
        let widget_size: sys::ImVec2 = self.size.into();
        let flags = self.flags.raw() | sys::ImGuiInputTextFlags_CallbackResize;
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

// InputText Callback System
// =========================

bitflags::bitflags! {
    /// Callback flags for InputText widgets
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct InputTextCallback: u32 {
        /// Call user function on pressing TAB (for completion handling)
        const COMPLETION = sys::ImGuiInputTextFlags_CallbackCompletion as u32;
        /// Call user function on pressing Up/Down arrows (for history handling)
        const HISTORY = sys::ImGuiInputTextFlags_CallbackHistory as u32;
        /// Call user function every time. User code may query cursor position, modify text buffer.
        const ALWAYS = sys::ImGuiInputTextFlags_CallbackAlways as u32;
        /// Call user function to filter character.
        const CHAR_FILTER = sys::ImGuiInputTextFlags_CallbackCharFilter as u32;
        /// Callback on buffer edit (note that InputText already returns true on edit, the
        /// callback is useful mainly to manipulate the underlying buffer while focus is active)
        const EDIT = sys::ImGuiInputTextFlags_CallbackEdit as u32;
    }
}

/// Direction for history navigation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryDirection {
    /// Up arrow key pressed
    Up,
    /// Down arrow key pressed
    Down,
}

/// This trait provides an interface which ImGui will call on InputText callbacks.
///
/// Each method requires its matching callback flag. If a callback panics, the
/// widget skips further user callbacks and resumes the panic after Dear ImGui
/// has returned and the Rust string has been restored.
pub trait InputTextCallbackHandler {
    /// Filters a char -- returning a `None` means that the char is removed,
    /// and returning another char substitutes it out.
    ///
    /// To make ImGui run this callback, use [InputTextCallback::CHAR_FILTER].
    fn char_filter(&mut self, c: char) -> Option<char> {
        Some(c)
    }

    /// Called when the user presses the completion key (TAB by default).
    ///
    /// To make ImGui run this callback, use [InputTextCallback::COMPLETION].
    fn on_completion(&mut self, _data: TextCallbackData<'_>) {}

    /// Called when the user presses Up/Down arrow keys for history navigation.
    ///
    /// To make ImGui run this callback, use [InputTextCallback::HISTORY].
    fn on_history(&mut self, _direction: HistoryDirection, _data: TextCallbackData<'_>) {}

    /// Called every frame when the input text is active.
    ///
    /// To make ImGui run this callback, use [InputTextCallback::ALWAYS].
    fn on_always(&mut self, _data: TextCallbackData<'_>) {}

    /// Called when the text buffer is edited.
    ///
    /// To make ImGui run this callback, use [InputTextCallback::EDIT].
    fn on_edit(&mut self, _data: TextCallbackData<'_>) {}
}

/// This struct provides methods to edit the underlying text buffer that
/// Dear ImGui manipulates. Primarily, it gives [remove_chars](Self::remove_chars),
/// [insert_chars](Self::insert_chars), and mutable access to what text is selected.
pub struct TextCallbackData<'cb>(&'cb mut sys::ImGuiInputTextCallbackData);

impl<'cb> TextCallbackData<'cb> {
    /// `data` must describe a live native text-edit callback buffer for `'cb`.
    unsafe fn new(data: &'cb mut sys::ImGuiInputTextCallbackData) -> Self {
        Self(data)
    }

    fn data(&self) -> &sys::ImGuiInputTextCallbackData {
        self.0
    }

    fn data_mut(&mut self) -> &mut sys::ImGuiInputTextCallbackData {
        self.0
    }

    fn valid_text_len(&self) -> usize {
        let data = self.data();
        assert!(!data.Buf.is_null(), "internal imgui error: Buf was null");
        assert!(
            data.BufTextLen >= 0,
            "internal imgui error: BufTextLen was negative"
        );
        let editable_resize = data.Flags & sys::ImGuiInputTextFlags_CallbackResize != 0
            && data.Flags & sys::ImGuiInputTextFlags_ReadOnly == 0;
        let size = if editable_resize {
            // InputTextEx passes TextA to edit callbacks before resizing the caller's buffer.
            // BufSize can still describe that smaller caller buffer after typing or pasting.
            assert!(!data.Ctx.is_null(), "missing text callback context");
            let text = unsafe { &(*data.Ctx).InputTextState.TextA };
            assert_eq!(data.Buf, text.Data, "unexpected text callback buffer");
            assert!(
                text.Size <= text.Capacity,
                "internal imgui error: callback text buffer size exceeded capacity"
            );
            text.Size
        } else {
            data.BufSize
        };
        assert!(
            data.BufTextLen < size,
            "internal imgui error: callback text must fit its buffer with a trailing NUL"
        );
        data.BufTextLen as usize
    }

    fn valid_text_len_i32(&self) -> i32 {
        self.valid_text_len() as i32
    }

    fn position(name: &str, pos: i32, len: usize) -> usize {
        let pos = usize::try_from(pos).unwrap_or_else(|_| {
            panic!("internal imgui error: {name} was negative");
        });
        assert!(
            pos <= len,
            "internal imgui error: {name} exceeded BufTextLen"
        );
        pos
    }

    fn position_to_i32(name: &str, pos: usize) -> i32 {
        i32::try_from(pos).unwrap_or_else(|_| {
            panic!("{name} exceeded ImGui's i32 position range");
        })
    }

    fn assert_byte_boundary(text: &str, name: &str, pos: usize) {
        assert!(
            text.is_char_boundary(pos),
            "{name} must lie on a UTF-8 character boundary"
        );
    }

    fn text_bytes(&self) -> &[u8] {
        let len = self.valid_text_len();
        unsafe { std::slice::from_raw_parts(self.data().Buf.cast(), len) }
    }

    fn expect_utf8(&self) -> &str {
        self.str()
            .expect("invalid UTF-8 in InputText callback buffer")
    }

    /// Borrows the callback text, excluding the trailing NUL, or returns its UTF-8 error.
    ///
    /// # Panics
    ///
    /// Panics if native callback metadata does not describe an in-bounds buffer.
    pub fn str(&self) -> Result<&str, Utf8Error> {
        std::str::from_utf8(self.text_bytes())
    }

    /// Borrows valid text or replaces malformed UTF-8 sequences with U+FFFD.
    /// Uses the same buffer bounds checks as [`Self::str`].
    pub fn str_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(self.text_bytes())
    }

    /// Get the current cursor position
    pub fn cursor_pos(&self) -> usize {
        let len = self.valid_text_len();
        Self::position("CursorPos", self.data().CursorPos, len)
    }

    /// Set the cursor position
    pub fn set_cursor_pos(&mut self, pos: usize) {
        let text = self.expect_utf8();
        assert!(pos <= text.len(), "cursor position out of bounds");
        Self::assert_byte_boundary(text, "cursor position", pos);
        self.data_mut().CursorPos = Self::position_to_i32("cursor position", pos);
    }

    /// Get the selection start position
    pub fn selection_start(&self) -> usize {
        let len = self.valid_text_len();
        Self::position("SelectionStart", self.data().SelectionStart, len)
    }

    /// Set the selection start position
    pub fn set_selection_start(&mut self, pos: usize) {
        let text = self.expect_utf8();
        assert!(pos <= text.len(), "selection start out of bounds");
        Self::assert_byte_boundary(text, "selection start", pos);
        self.data_mut().SelectionStart = Self::position_to_i32("selection start", pos);
    }

    /// Get the selection end position
    pub fn selection_end(&self) -> usize {
        let len = self.valid_text_len();
        Self::position("SelectionEnd", self.data().SelectionEnd, len)
    }

    /// Set the selection end position
    pub fn set_selection_end(&mut self, pos: usize) {
        let text = self.expect_utf8();
        assert!(pos <= text.len(), "selection end out of bounds");
        Self::assert_byte_boundary(text, "selection end", pos);
        self.data_mut().SelectionEnd = Self::position_to_i32("selection end", pos);
    }

    /// Select all text
    pub fn select_all(&mut self) {
        let len = self.valid_text_len_i32();
        let data = self.data_mut();
        data.SelectionStart = 0;
        data.SelectionEnd = len;
    }

    /// Clear selection
    pub fn clear_selection(&mut self) {
        let cursor_pos = Self::position_to_i32("cursor position", self.cursor_pos());
        let data = self.data_mut();
        data.SelectionStart = cursor_pos;
        data.SelectionEnd = cursor_pos;
    }

    /// Returns true if there is a selection
    pub fn has_selection(&self) -> bool {
        self.data().SelectionStart != self.data().SelectionEnd
    }

    /// Delete characters in the range [pos, pos+bytes_count)
    pub fn remove_chars(&mut self, pos: usize, bytes_count: usize) {
        let text = self.expect_utf8();
        let end = pos
            .checked_add(bytes_count)
            .expect("delete range overflowed usize");
        assert!(end <= text.len(), "delete range out of bounds");
        Self::assert_byte_boundary(text, "delete start", pos);
        Self::assert_byte_boundary(text, "delete end", end);
        let pos = Self::position_to_i32("delete start", pos);
        let bytes_count = Self::position_to_i32("delete byte count", bytes_count);
        unsafe {
            sys::ImGuiInputTextCallbackData_DeleteChars(self.0, pos, bytes_count);
        }
    }

    /// Insert text at the given byte position. Insertion stops at the first NUL.
    /// A read-only widget's callback can edit only within the caller's existing buffer.
    pub fn insert_chars(&mut self, pos: usize, text: &str) {
        let current = self.expect_utf8();
        assert!(pos <= current.len(), "insert position out of bounds");
        Self::assert_byte_boundary(current, "insert position", pos);
        let text = self.insertion_prefix(text, current.len());
        if text.is_empty() {
            return;
        }
        // InsertChars grows TextA using int arithmetic (up to 4 * insertion length).
        assert!(
            text.len() <= i32::MAX as usize / 4
                && current.len() + text.len().max(256) <= i32::MAX as usize - 2,
            "inserted text exceeds Dear ImGui's i32 growth range"
        );
        let pos = Self::position_to_i32("insert position", pos);
        let text_ptr = text.as_ptr() as *const std::os::raw::c_char;
        unsafe {
            sys::ImGuiInputTextCallbackData_InsertChars(
                self.0,
                pos,
                text_ptr,
                text_ptr.add(text.len()),
            );
        }
    }

    fn insertion_prefix<'a>(&self, text: &'a str, current_len: usize) -> &'a str {
        let text = text.split_once('\0').map_or(text, |(prefix, _)| prefix);
        let data = self.data();
        if data.Flags & sys::ImGuiInputTextFlags_ReadOnly != 0 {
            let available = data.BufSize as usize - current_len - 1;
            &text[..text.floor_char_boundary(available)]
        } else {
            text
        }
    }

    /// Gives access to the underlying byte array MUTABLY.
    ///
    /// ## Safety
    ///
    /// This is very unsafe, and the following invariants must be
    /// upheld:
    /// 1. Keep the data valid UTF-8 and free of NUL bytes.
    /// 2. After editing the string, call [set_dirty].
    ///
    /// To truncate the string, please use [remove_chars]. To extend
    /// the string, please use [insert_chars] and [push_str].
    ///
    /// This function should have highly limited usage, but could be for
    /// editing certain characters in the buffer based on some external condition.
    ///
    /// [remove_chars]: Self::remove_chars
    /// [set_dirty]: Self::set_dirty
    /// [insert_chars]: Self::insert_chars
    /// [push_str]: Self::push_str
    pub unsafe fn str_as_bytes_mut(&mut self) -> &mut [u8] {
        let len = self.valid_text_len();
        unsafe { std::slice::from_raw_parts_mut(self.data_mut().Buf.cast(), len) }
    }

    /// Sets the dirty flag on the text to imgui, indicating that
    /// it should reapply this string to its internal state.
    ///
    /// **NB:** You only need to use this method if you're using `[str_as_bytes_mut]`.
    /// If you use the helper methods [remove_chars] and [insert_chars],
    /// this will be set for you. However, this is no downside to setting
    /// the dirty flag spuriously except the minor CPU time imgui will spend.
    ///
    /// [str_as_bytes_mut]: Self::str_as_bytes_mut
    /// [remove_chars]: Self::remove_chars
    /// [insert_chars]: Self::insert_chars
    pub fn set_dirty(&mut self) {
        self.data_mut().BufDirty = true;
    }

    /// Returns the selected text directly. Note that if no text is selected,
    /// an empty str slice will be returned.
    pub fn selected(&self) -> &str {
        let text = self.expect_utf8();
        let start = self.selection_start().min(self.selection_end());
        let end = self.selection_start().max(self.selection_end());
        assert!(end <= text.len(), "selection range out of bounds");
        Self::assert_byte_boundary(text, "selection start", start);
        Self::assert_byte_boundary(text, "selection end", end);
        &text[start..end]
    }

    /// Appends text using the same NUL and read-only capacity rules as [`Self::insert_chars`].
    /// Editable buffers grow as needed.
    pub fn push_str(&mut self, text: &str) {
        let current_len = self.valid_text_len();
        self.insert_chars(current_len, text);
    }
}

/// This is a ZST which implements InputTextCallbackHandler as a passthrough.
///
/// If you do not set a callback handler, this will be used (but will never
/// actually run, since you will not have passed imgui any flags).
pub struct PassthroughCallback;
impl InputTextCallbackHandler for PassthroughCallback {}

/// Builder for integer input widget
#[derive(Debug)]
#[must_use]
pub struct InputInt<'ui> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    step: i32,
    step_fast: i32,
    flags: InputScalarFlags,
}

impl<'ui> InputInt<'ui> {
    /// Creates a new integer input builder
    pub fn new(ui: &'ui Ui, label: impl Into<Cow<'ui, str>>) -> Self {
        Self {
            ui,
            label: label.into(),
            step: 1,
            step_fast: 100,
            flags: InputScalarFlags::NONE,
        }
    }

    /// Sets the step value
    pub fn step(mut self, step: i32) -> Self {
        self.step = step;
        self
    }

    /// Sets the fast step value
    pub fn step_fast(mut self, step_fast: i32) -> Self {
        self.step_fast = step_fast;
        self
    }

    /// Sets the flags for the input
    pub fn flags(mut self, flags: InputScalarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the integer input widget
    pub fn build(self, value: &mut i32) -> bool {
        validate_input_scalar_flags("InputInt::build()", self.flags);
        self.ui.with_scratch_buffer(|scratch| {
            let label = scratch.scratch_txt(self.label);
            self.ui.run_with_bound_context(|| unsafe {
                sys::igInputInt(label, value, self.step, self.step_fast, self.flags.raw())
            })
        })
    }
}

/// Builder for float input widget
#[derive(Debug)]
#[must_use]
pub struct InputFloat<'ui, F = &'static str> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    step: f32,
    step_fast: f32,
    display_format: Option<F>,
    flags: InputScalarFlags,
}

impl<'ui> InputFloat<'ui> {
    /// Creates a new float input builder
    pub fn new(ui: &'ui Ui, label: impl Into<Cow<'ui, str>>) -> Self {
        Self {
            ui,
            label: label.into(),
            step: 0.0,
            step_fast: 0.0,
            display_format: None,
            flags: InputScalarFlags::NONE,
        }
    }
}

impl<'ui, F: AsRef<str>> InputFloat<'ui, F> {
    /// Sets the step value
    pub fn step(mut self, step: f32) -> Self {
        self.step = step;
        self
    }

    /// Sets the fast step value
    pub fn step_fast(mut self, step_fast: f32) -> Self {
        self.step_fast = step_fast;
        self
    }

    /// Sets the validated display format.
    pub fn display_format<'fmt>(
        self,
        display_format: NumericFormat<'fmt, f32>,
    ) -> InputFloat<'ui, NumericFormat<'fmt, f32>> {
        InputFloat {
            ui: self.ui,
            label: self.label,
            step: self.step,
            step_fast: self.step_fast,
            display_format: Some(display_format),
            flags: self.flags,
        }
    }

    /// Validates and sets a C-style display format.
    pub fn try_display_format<'fmt>(
        self,
        display_format: impl Into<Cow<'fmt, str>>,
    ) -> Result<InputFloat<'ui, NumericFormat<'fmt, f32>>, ContextError> {
        Ok(self.display_format(NumericFormat::new(display_format)?))
    }

    /// Sets the flags for the input
    pub fn flags(mut self, flags: InputScalarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the float input widget
    pub fn build(self, value: &mut f32) -> bool {
        validate_input_scalar_flags("InputFloat::build()", self.flags);
        let format = self
            .display_format
            .as_ref()
            .map(AsRef::as_ref)
            .unwrap_or("%.3f");
        self.ui.with_scratch_buffer(|scratch| {
            let (label, format) = scratch.scratch_txt_two(self.label, format);
            self.ui.run_with_bound_context(|| unsafe {
                sys::igInputFloat(
                    label,
                    value,
                    self.step,
                    self.step_fast,
                    format,
                    self.flags.raw(),
                )
            })
        })
    }
}

/// Builder for double input widget
#[derive(Debug)]
#[must_use]
pub struct InputDouble<'ui, F = &'static str> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    step: f64,
    step_fast: f64,
    display_format: Option<F>,
    flags: InputScalarFlags,
}

impl<'ui> InputDouble<'ui> {
    /// Creates a new double input builder
    pub fn new(ui: &'ui Ui, label: impl Into<Cow<'ui, str>>) -> Self {
        Self {
            ui,
            label: label.into(),
            step: 0.0,
            step_fast: 0.0,
            display_format: None,
            flags: InputScalarFlags::NONE,
        }
    }
}

impl<'ui, F: AsRef<str>> InputDouble<'ui, F> {
    /// Sets the step value
    pub fn step(mut self, step: f64) -> Self {
        self.step = step;
        self
    }

    /// Sets the fast step value
    pub fn step_fast(mut self, step_fast: f64) -> Self {
        self.step_fast = step_fast;
        self
    }

    /// Sets the validated display format.
    pub fn display_format<'fmt>(
        self,
        display_format: NumericFormat<'fmt, f64>,
    ) -> InputDouble<'ui, NumericFormat<'fmt, f64>> {
        InputDouble {
            ui: self.ui,
            label: self.label,
            step: self.step,
            step_fast: self.step_fast,
            display_format: Some(display_format),
            flags: self.flags,
        }
    }

    /// Validates and sets a C-style display format.
    pub fn try_display_format<'fmt>(
        self,
        display_format: impl Into<Cow<'fmt, str>>,
    ) -> Result<InputDouble<'ui, NumericFormat<'fmt, f64>>, ContextError> {
        Ok(self.display_format(NumericFormat::new(display_format)?))
    }

    /// Sets the flags for the input
    pub fn flags(mut self, flags: InputScalarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the double input widget
    pub fn build(self, value: &mut f64) -> bool {
        validate_input_scalar_flags("InputDouble::build()", self.flags);
        let format = self
            .display_format
            .as_ref()
            .map(AsRef::as_ref)
            .unwrap_or("%.6f");
        self.ui.with_scratch_buffer(|scratch| {
            let (label, format) = scratch.scratch_txt_two(self.label, format);
            self.ui.run_with_bound_context(|| unsafe {
                sys::igInputDouble(
                    label,
                    value,
                    self.step,
                    self.step_fast,
                    format,
                    self.flags.raw(),
                )
            })
        })
    }
}

/// Builder for a 2-component float input widget.
#[must_use]
pub struct InputFloat2<'ui, 'p, L, F = &'static str> {
    label: L,
    value: &'p mut [f32; 2],
    display_format: Option<F>,
    flags: InputScalarFlags,
    ui: &'ui Ui,
}

impl<'ui, 'p, L: AsRef<str>> InputFloat2<'ui, 'p, L> {
    /// Constructs a new input float2 builder.
    #[doc(alias = "InputFloat2")]
    pub fn new(ui: &'ui Ui, label: L, value: &'p mut [f32; 2]) -> Self {
        InputFloat2 {
            label,
            value,
            display_format: None,
            flags: InputScalarFlags::empty(),
            ui,
        }
    }
}

impl<'ui, 'p, L: AsRef<str>, F: AsRef<str>> InputFloat2<'ui, 'p, L, F> {
    /// Sets the validated display format.
    pub fn display_format<'fmt>(
        self,
        display_format: NumericFormat<'fmt, f32>,
    ) -> InputFloat2<'ui, 'p, L, NumericFormat<'fmt, f32>> {
        InputFloat2 {
            label: self.label,
            value: self.value,
            display_format: Some(display_format),
            flags: self.flags,
            ui: self.ui,
        }
    }

    /// Validates and sets a C-style display format.
    pub fn try_display_format<'fmt>(
        self,
        display_format: impl Into<Cow<'fmt, str>>,
    ) -> Result<InputFloat2<'ui, 'p, L, NumericFormat<'fmt, f32>>, ContextError> {
        Ok(self.display_format(NumericFormat::new(display_format)?))
    }

    /// Sets the input text flags
    #[inline]
    pub fn flags(mut self, flags: InputScalarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the input float2 widget.
    ///
    /// Returns true if any value was changed.
    pub fn build(self) -> bool {
        validate_input_scalar_flags("InputFloat2::build()", self.flags);
        self.ui.with_scratch_buffer(|scratch| {
            let (label, format) = scratch.scratch_txt_with_opt(self.label, self.display_format);
            self.ui.run_with_bound_context(|| unsafe {
                sys::igInputFloat2(label, self.value.as_mut_ptr(), format, self.flags.raw())
            })
        })
    }
}

/// Builder for a 3-component float input widget.
#[must_use]
pub struct InputFloat3<'ui, 'p, L, F = &'static str> {
    label: L,
    value: &'p mut [f32; 3],
    display_format: Option<F>,
    flags: InputScalarFlags,
    ui: &'ui Ui,
}

impl<'ui, 'p, L: AsRef<str>> InputFloat3<'ui, 'p, L> {
    /// Constructs a new input float3 builder.
    #[doc(alias = "InputFloat3")]
    pub fn new(ui: &'ui Ui, label: L, value: &'p mut [f32; 3]) -> Self {
        InputFloat3 {
            label,
            value,
            display_format: None,
            flags: InputScalarFlags::empty(),
            ui,
        }
    }
}

impl<'ui, 'p, L: AsRef<str>, F: AsRef<str>> InputFloat3<'ui, 'p, L, F> {
    /// Sets the validated display format.
    pub fn display_format<'fmt>(
        self,
        display_format: NumericFormat<'fmt, f32>,
    ) -> InputFloat3<'ui, 'p, L, NumericFormat<'fmt, f32>> {
        InputFloat3 {
            label: self.label,
            value: self.value,
            display_format: Some(display_format),
            flags: self.flags,
            ui: self.ui,
        }
    }

    /// Validates and sets a C-style display format.
    pub fn try_display_format<'fmt>(
        self,
        display_format: impl Into<Cow<'fmt, str>>,
    ) -> Result<InputFloat3<'ui, 'p, L, NumericFormat<'fmt, f32>>, ContextError> {
        Ok(self.display_format(NumericFormat::new(display_format)?))
    }

    /// Sets the input text flags
    #[inline]
    pub fn flags(mut self, flags: InputScalarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the input float3 widget.
    ///
    /// Returns true if any value was changed.
    pub fn build(self) -> bool {
        validate_input_scalar_flags("InputFloat3::build()", self.flags);
        self.ui.with_scratch_buffer(|scratch| {
            let (label, format) = scratch.scratch_txt_with_opt(self.label, self.display_format);
            self.ui.run_with_bound_context(|| unsafe {
                sys::igInputFloat3(label, self.value.as_mut_ptr(), format, self.flags.raw())
            })
        })
    }
}

/// Builder for a 4-component float input widget.
#[must_use]
pub struct InputFloat4<'ui, 'p, L, F = &'static str> {
    label: L,
    value: &'p mut [f32; 4],
    display_format: Option<F>,
    flags: InputScalarFlags,
    ui: &'ui Ui,
}

impl<'ui, 'p, L: AsRef<str>> InputFloat4<'ui, 'p, L> {
    /// Constructs a new input float4 builder.
    #[doc(alias = "InputFloat4")]
    pub fn new(ui: &'ui Ui, label: L, value: &'p mut [f32; 4]) -> Self {
        InputFloat4 {
            label,
            value,
            display_format: None,
            flags: InputScalarFlags::empty(),
            ui,
        }
    }
}

impl<'ui, 'p, L: AsRef<str>, F: AsRef<str>> InputFloat4<'ui, 'p, L, F> {
    /// Sets the validated display format.
    pub fn display_format<'fmt>(
        self,
        display_format: NumericFormat<'fmt, f32>,
    ) -> InputFloat4<'ui, 'p, L, NumericFormat<'fmt, f32>> {
        InputFloat4 {
            label: self.label,
            value: self.value,
            display_format: Some(display_format),
            flags: self.flags,
            ui: self.ui,
        }
    }

    /// Validates and sets a C-style display format.
    pub fn try_display_format<'fmt>(
        self,
        display_format: impl Into<Cow<'fmt, str>>,
    ) -> Result<InputFloat4<'ui, 'p, L, NumericFormat<'fmt, f32>>, ContextError> {
        Ok(self.display_format(NumericFormat::new(display_format)?))
    }

    /// Sets the input text flags
    #[inline]
    pub fn flags(mut self, flags: InputScalarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the input float4 widget.
    ///
    /// Returns true if any value was changed.
    pub fn build(self) -> bool {
        validate_input_scalar_flags("InputFloat4::build()", self.flags);
        self.ui.with_scratch_buffer(|scratch| {
            let (label, format) = scratch.scratch_txt_with_opt(self.label, self.display_format);
            self.ui.run_with_bound_context(|| unsafe {
                sys::igInputFloat4(label, self.value.as_mut_ptr(), format, self.flags.raw())
            })
        })
    }
}

/// Builder for a 2-component int input widget.
#[must_use]
pub struct InputInt2<'ui, 'p, L> {
    label: L,
    value: &'p mut [i32; 2],
    flags: InputScalarFlags,
    ui: &'ui Ui,
}

impl<'ui, 'p, L: AsRef<str>> InputInt2<'ui, 'p, L> {
    /// Constructs a new input int2 builder.
    #[doc(alias = "InputInt2")]
    pub fn new(ui: &'ui Ui, label: L, value: &'p mut [i32; 2]) -> Self {
        InputInt2 {
            label,
            value,
            flags: InputScalarFlags::empty(),
            ui,
        }
    }

    /// Sets the input text flags
    #[inline]
    pub fn flags(mut self, flags: InputScalarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the input int2 widget.
    ///
    /// Returns true if any value was changed.
    pub fn build(self) -> bool {
        validate_input_scalar_flags("InputInt2::build()", self.flags);
        self.ui.with_scratch_buffer(|scratch| {
            let label = scratch.scratch_txt(self.label);
            self.ui.run_with_bound_context(|| unsafe {
                sys::igInputInt2(label, self.value.as_mut_ptr(), self.flags.raw())
            })
        })
    }
}

/// Builder for a 3-component int input widget.
#[must_use]
pub struct InputInt3<'ui, 'p, L> {
    label: L,
    value: &'p mut [i32; 3],
    flags: InputScalarFlags,
    ui: &'ui Ui,
}

impl<'ui, 'p, L: AsRef<str>> InputInt3<'ui, 'p, L> {
    /// Constructs a new input int3 builder.
    #[doc(alias = "InputInt3")]
    pub fn new(ui: &'ui Ui, label: L, value: &'p mut [i32; 3]) -> Self {
        InputInt3 {
            label,
            value,
            flags: InputScalarFlags::empty(),
            ui,
        }
    }

    /// Sets the input text flags
    #[inline]
    pub fn flags(mut self, flags: InputScalarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the input int3 widget.
    ///
    /// Returns true if any value was changed.
    pub fn build(self) -> bool {
        validate_input_scalar_flags("InputInt3::build()", self.flags);
        self.ui.with_scratch_buffer(|scratch| {
            let label = scratch.scratch_txt(self.label);
            self.ui.run_with_bound_context(|| unsafe {
                sys::igInputInt3(label, self.value.as_mut_ptr(), self.flags.raw())
            })
        })
    }
}

/// Builder for a 4-component int input widget.
#[must_use]
pub struct InputInt4<'ui, 'p, L> {
    label: L,
    value: &'p mut [i32; 4],
    flags: InputScalarFlags,
    ui: &'ui Ui,
}

impl<'ui, 'p, L: AsRef<str>> InputInt4<'ui, 'p, L> {
    /// Constructs a new input int4 builder.
    #[doc(alias = "InputInt4")]
    pub fn new(ui: &'ui Ui, label: L, value: &'p mut [i32; 4]) -> Self {
        InputInt4 {
            label,
            value,
            flags: InputScalarFlags::empty(),
            ui,
        }
    }

    /// Sets the input text flags
    #[inline]
    pub fn flags(mut self, flags: InputScalarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the input int4 widget.
    ///
    /// Returns true if any value was changed.
    pub fn build(self) -> bool {
        validate_input_scalar_flags("InputInt4::build()", self.flags);
        self.ui.with_scratch_buffer(|scratch| {
            let label = scratch.scratch_txt(self.label);
            self.ui.run_with_bound_context(|| unsafe {
                sys::igInputInt4(label, self.value.as_mut_ptr(), self.flags.raw())
            })
        })
    }
}

/// Builder for an input scalar widget.
#[must_use]
pub struct InputScalar<'ui, 'p, T, L, F = &'static str> {
    value: &'p mut T,
    label: L,
    step: Option<T>,
    step_fast: Option<T>,
    display_format: Option<F>,
    flags: InputScalarFlags,
    ui: &'ui Ui,
}

impl<'ui, 'p, L: AsRef<str>, T: DataTypeKind> InputScalar<'ui, 'p, T, L> {
    /// Constructs a new input scalar builder.
    #[doc(alias = "InputScalar")]
    pub fn new(ui: &'ui Ui, label: L, value: &'p mut T) -> Self {
        InputScalar {
            value,
            label,
            step: None,
            step_fast: None,
            display_format: None,
            flags: InputScalarFlags::empty(),
            ui,
        }
    }
}

impl<'ui, 'p, L: AsRef<str>, T: DataTypeKind, F: AsRef<str>> InputScalar<'ui, 'p, T, L, F> {
    /// Sets the validated display format.
    pub fn display_format<'fmt>(
        self,
        display_format: NumericFormat<'fmt, T>,
    ) -> InputScalar<'ui, 'p, T, L, NumericFormat<'fmt, T>> {
        InputScalar {
            value: self.value,
            label: self.label,
            step: self.step,
            step_fast: self.step_fast,
            display_format: Some(display_format),
            flags: self.flags,
            ui: self.ui,
        }
    }

    /// Validates and sets a C-style display format.
    pub fn try_display_format<'fmt>(
        self,
        display_format: impl Into<Cow<'fmt, str>>,
    ) -> Result<InputScalar<'ui, 'p, T, L, NumericFormat<'fmt, T>>, ContextError> {
        Ok(self.display_format(NumericFormat::new(display_format)?))
    }

    /// Sets the step value for the input
    #[inline]
    pub fn step(mut self, value: T) -> Self {
        self.step = Some(value);
        self
    }

    /// Sets the fast step value for the input
    #[inline]
    pub fn step_fast(mut self, value: T) -> Self {
        self.step_fast = Some(value);
        self
    }

    /// Sets the input text flags
    #[inline]
    pub fn flags(mut self, flags: InputScalarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds an input scalar that is bound to the given value.
    ///
    /// Returns true if the value was changed.
    pub fn build(self) -> bool {
        validate_input_scalar_flags("InputScalar::build()", self.flags);
        self.ui.with_scratch_buffer(|scratch| {
            let (label, format) = scratch.scratch_txt_with_opt(self.label, self.display_format);
            self.ui.run_with_bound_context(|| unsafe {
                sys::igInputScalar(
                    label,
                    T::KIND as i32,
                    self.value as *mut T as *mut c_void,
                    self.step
                        .as_ref()
                        .map(|step| step as *const T)
                        .unwrap_or(ptr::null()) as *const c_void,
                    self.step_fast
                        .as_ref()
                        .map(|step| step as *const T)
                        .unwrap_or(ptr::null()) as *const c_void,
                    format,
                    self.flags.raw(),
                )
            })
        })
    }
}

/// Builder for an input scalar array widget.
#[must_use]
pub struct InputScalarN<'ui, 'p, T, L, F = &'static str> {
    values: &'p mut [T],
    label: L,
    step: Option<T>,
    step_fast: Option<T>,
    display_format: Option<F>,
    flags: InputScalarFlags,
    ui: &'ui Ui,
}

impl<'ui, 'p, L: AsRef<str>, T: DataTypeKind> InputScalarN<'ui, 'p, T, L> {
    /// Constructs a new input scalar array builder.
    #[doc(alias = "InputScalarN")]
    pub fn new(ui: &'ui Ui, label: L, values: &'p mut [T]) -> Self {
        InputScalarN {
            values,
            label,
            step: None,
            step_fast: None,
            display_format: None,
            flags: InputScalarFlags::empty(),
            ui,
        }
    }
}

impl<'ui, 'p, L: AsRef<str>, T: DataTypeKind, F: AsRef<str>> InputScalarN<'ui, 'p, T, L, F> {
    /// Sets the validated display format.
    pub fn display_format<'fmt>(
        self,
        display_format: NumericFormat<'fmt, T>,
    ) -> InputScalarN<'ui, 'p, T, L, NumericFormat<'fmt, T>> {
        InputScalarN {
            values: self.values,
            label: self.label,
            step: self.step,
            step_fast: self.step_fast,
            display_format: Some(display_format),
            flags: self.flags,
            ui: self.ui,
        }
    }

    /// Validates and sets a C-style display format.
    pub fn try_display_format<'fmt>(
        self,
        display_format: impl Into<Cow<'fmt, str>>,
    ) -> Result<InputScalarN<'ui, 'p, T, L, NumericFormat<'fmt, T>>, ContextError> {
        Ok(self.display_format(NumericFormat::new(display_format)?))
    }

    /// Sets the step value for the input
    #[inline]
    pub fn step(mut self, value: T) -> Self {
        self.step = Some(value);
        self
    }

    /// Sets the fast step value for the input
    #[inline]
    pub fn step_fast(mut self, value: T) -> Self {
        self.step_fast = Some(value);
        self
    }

    /// Sets the input text flags
    #[inline]
    pub fn flags(mut self, flags: InputScalarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds a horizontal array of multiple input scalars attached to the given slice.
    ///
    /// Returns true if any value was changed.
    pub fn build(self) -> bool {
        validate_input_scalar_flags("InputScalarN::build()", self.flags);
        let count = component_count_i32("InputScalarN::build()", self.values.len());
        self.ui.with_scratch_buffer(|scratch| {
            let (label, format) = scratch.scratch_txt_with_opt(self.label, self.display_format);
            self.ui.run_with_bound_context(|| unsafe {
                sys::igInputScalarN(
                    label,
                    T::KIND as i32,
                    self.values.as_mut_ptr() as *mut c_void,
                    count,
                    self.step
                        .as_ref()
                        .map(|step| step as *const T)
                        .unwrap_or(ptr::null()) as *const c_void,
                    self.step_fast
                        .as_ref()
                        .map(|step| step as *const T)
                        .unwrap_or(ptr::null()) as *const c_void,
                    format,
                    self.flags.raw(),
                )
            })
        })
    }
}

const MAX_INPUT_BUFFER_SIZE: usize = i32::MAX as usize - 1;

fn validate_input_buffer_size(size: usize) {
    // InputTextEx takes an int and allocates its internal TextA with buf_size + 1.
    assert!(
        size <= MAX_INPUT_BUFFER_SIZE,
        "InputText buffer exceeds Dear ImGui's i32 size range"
    );
}

struct StringInputBuffer<'a> {
    target: &'a mut String,
    bytes: Vec<u8>,
}

impl<'a> StringInputBuffer<'a> {
    fn new(target: &'a mut String, capacity_hint: Option<usize>) -> Self {
        let spare = capacity_hint.unwrap_or(0);
        let needed = target.len().saturating_add(spare).saturating_add(1);
        validate_input_buffer_size(needed);
        let size = target.capacity().min(MAX_INPUT_BUFFER_SIZE).max(needed);

        if spare == 0 {
            // A full String only needs a terminator to display its existing contents.
            target.reserve_exact(1);
        } else {
            // Amortize the requested spare bytes as the field grows across frames.
            target.reserve(spare + 1);
        }
        let mut bytes = mem::take(target).into_bytes();
        bytes.resize(size, 0);
        Self { target, bytes }
    }
}

impl Drop for StringInputBuffer<'_> {
    fn drop(&mut self) {
        finish_string_input_buffer(self.target, mem::take(&mut self.bytes));
    }
}

fn resize_string_input_buffer(buffer: &mut Vec<u8>, requested: i32) {
    if let Ok(requested) = usize::try_from(requested)
        && requested <= MAX_INPUT_BUFFER_SIZE
        && requested > buffer.len()
        && buffer.try_reserve(requested - buffer.len()).is_ok()
    {
        buffer.resize(requested, 0);
    }
}

fn finish_string_input_buffer(target: &mut String, mut buffer: Vec<u8>) {
    let len = buffer
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(buffer.len());
    buffer.truncate(len);

    let preserved_capacity = target.capacity().max(buffer.capacity());
    let mut text = match String::from_utf8(buffer) {
        Ok(text) => text,
        Err(err) => {
            let bytes = err.into_bytes();
            String::from_utf8_lossy(&bytes).into_owned()
        }
    };
    if text.capacity() < preserved_capacity {
        text.reserve(preserved_capacity.saturating_sub(text.len()));
    }
    *target = text;
}

struct StringCallbackState<T> {
    buffer: *mut Vec<u8>,
    handler: T,
    panic: Option<Box<dyn Any + Send>>,
}

fn with_string_input_buffer<T: InputTextCallbackHandler>(
    target: &mut String,
    capacity_hint: Option<usize>,
    handler: T,
    build: impl FnOnce(*mut c_char, usize, sys::ImGuiInputTextCallback, *mut c_void) -> bool,
) -> bool {
    let mut input = StringInputBuffer::new(target, capacity_hint);
    let size = input.bytes.len();
    let ptr = input.bytes.as_mut_ptr().cast();
    let mut state = StringCallbackState {
        buffer: &raw mut input.bytes,
        handler,
        panic: None,
    };
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
    if event == sys::ImGuiInputTextFlags_CallbackResize {
        let buffer = unsafe { &mut *state.buffer };
        let requested = unsafe { (*data).BufSize };
        let previous_len = buffer.len();
        let resized = catch_unwind(AssertUnwindSafe(|| {
            resize_string_input_buffer(buffer, requested);
        }));
        unsafe {
            if buffer.len() != previous_len {
                (*data).BufDirty = true;
            }
            (*data).Buf = buffer.as_mut_ptr().cast();
            (*data).BufSize = buffer.len().min(MAX_INPUT_BUFFER_SIZE) as i32;
        }
        if let Err(payload) = resized {
            if state.panic.is_none() {
                state.panic = Some(payload);
            } else {
                std::mem::forget(payload);
            }
        }
        return 0;
    }

    let fallback = c_int::from(event == sys::ImGuiInputTextFlags_CallbackCharFilter);
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
        value if value == sys::ImGuiInputTextFlags_CallbackCompletion => {
            handler.on_completion(unsafe { TextCallbackData::new(&mut *data) });
        }
        value if value == sys::ImGuiInputTextFlags_CallbackHistory => {
            let direction = if unsafe { (*data).EventKey } == sys::ImGuiKey_UpArrow {
                HistoryDirection::Up
            } else {
                HistoryDirection::Down
            };
            handler.on_history(direction, unsafe { TextCallbackData::new(&mut *data) });
        }
        value if value == sys::ImGuiInputTextFlags_CallbackAlways => {
            handler.on_always(unsafe { TextCallbackData::new(&mut *data) });
        }
        value if value == sys::ImGuiInputTextFlags_CallbackEdit => {
            handler.on_edit(unsafe { TextCallbackData::new(&mut *data) });
        }
        value if value == sys::ImGuiInputTextFlags_CallbackCharFilter => {
            let Some(character) = char::from_u32(unsafe { (*data).EventChar })
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

fn assert_no_unsupported_bits(caller: &str, ty: &str, bits: i32, all: i32) {
    let unsupported = bits & !all;
    assert!(
        unsupported == 0,
        "{caller} received unsupported {ty} bits: 0x{unsupported:X}"
    );
}

fn validate_input_text_flags(caller: &str, flags: InputTextFlags) {
    assert_no_unsupported_bits(
        caller,
        "InputTextFlags",
        flags.bits(),
        InputTextFlags::all().bits(),
    );
    assert!(
        !flags.contains(InputTextFlags::CALLBACK_COMPLETION)
            || !flags.contains(InputTextFlags::ALLOW_TAB_INPUT),
        "{caller} cannot combine CALLBACK_COMPLETION with ALLOW_TAB_INPUT"
    );
}

fn validate_input_multiline_flags(caller: &str, flags: InputTextMultilineFlags) {
    assert_no_unsupported_bits(
        caller,
        "InputTextMultilineFlags",
        flags.bits(),
        InputTextMultilineFlags::all().bits(),
    );
}

fn validate_input_scalar_flags(caller: &str, flags: InputScalarFlags) {
    assert_no_unsupported_bits(
        caller,
        "InputScalarFlags",
        flags.bits(),
        InputScalarFlags::all().bits(),
    );
}

#[cfg(test)]
mod tests;

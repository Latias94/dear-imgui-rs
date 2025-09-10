use crate::sys;
use crate::ui::Ui;
use crate::InputTextFlags;
use std::ffi::c_int;
use std::marker::PhantomData;

/// # Input Widgets
impl Ui {
    /// Creates a single-line text input widget builder.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let mut text = String::new();
    /// if ui.input_text("Label", &mut text).build() {
    ///     println!("Text changed: {}", text);
    /// }
    /// ```
    #[doc(alias = "InputText", alias = "InputTextWithHint")]
    pub fn input_text<'p>(
        &self,
        label: impl AsRef<str>,
        buf: &'p mut String,
    ) -> InputText<'_, 'p, String, String, PassthroughCallback> {
        InputText::new(self, label, buf)
    }

    /// Creates a multi-line text input widget builder.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let mut text = String::new();
    /// if ui.input_text_multiline("Label", &mut text, [200.0, 100.0]).build() {
    ///     println!("Text changed: {}", text);
    /// }
    /// ```
    #[doc(alias = "InputTextMultiline")]
    pub fn input_text_multiline<'p>(
        &self,
        label: impl AsRef<str>,
        buf: &'p mut String,
        size: impl Into<[f32; 2]>,
    ) -> InputTextMultiline<'_, 'p> {
        InputTextMultiline::new(self, label, buf, size)
    }

    /// Creates an integer input widget.
    ///
    /// Returns true if the value was edited.
    #[doc(alias = "InputInt")]
    pub fn input_int(&self, label: impl AsRef<str>, value: &mut i32) -> bool {
        self.input_int_config(label).build(value)
    }

    /// Creates a float input widget.
    ///
    /// Returns true if the value was edited.
    #[doc(alias = "InputFloat")]
    pub fn input_float(&self, label: impl AsRef<str>, value: &mut f32) -> bool {
        self.input_float_config(label).build(value)
    }

    /// Creates a double input widget.
    ///
    /// Returns true if the value was edited.
    #[doc(alias = "InputDouble")]
    pub fn input_double(&self, label: impl AsRef<str>, value: &mut f64) -> bool {
        self.input_double_config(label).build(value)
    }

    /// Creates an integer input builder
    pub fn input_int_config(&self, label: impl AsRef<str>) -> InputInt<'_> {
        InputInt::new(self, label)
    }

    /// Creates a float input builder
    pub fn input_float_config(&self, label: impl AsRef<str>) -> InputFloat<'_> {
        InputFloat::new(self, label)
    }

    /// Creates a double input builder
    pub fn input_double_config(&self, label: impl AsRef<str>) -> InputDouble<'_> {
        InputDouble::new(self, label)
    }
}

/// Builder for a text input widget
#[must_use]
pub struct InputText<'ui, 'p, L = String, H = String, T = PassthroughCallback> {
    ui: &'ui Ui,
    label: L,
    buf: &'p mut String,
    flags: InputTextFlags,
    hint: Option<H>,
    callback_handler: T,
    _phantom: PhantomData<&'ui ()>,
}

impl<'ui, 'p> InputText<'ui, 'p, String, String, PassthroughCallback> {
    /// Creates a new text input builder
    pub fn new(ui: &'ui Ui, label: impl AsRef<str>, buf: &'p mut String) -> Self {
        Self {
            ui,
            label: label.as_ref().to_string(),
            buf,
            flags: InputTextFlags::CALLBACK_RESIZE,
            hint: None,
            callback_handler: PassthroughCallback,
            _phantom: PhantomData,
        }
    }
}

impl<'ui, 'p, L, H, T> InputText<'ui, 'p, L, H, T> {
    /// Sets the flags for the input
    pub fn flags(mut self, flags: InputTextFlags) -> Self {
        self.flags = flags | InputTextFlags::CALLBACK_RESIZE;
        self
    }

    /// Sets a hint text
    pub fn hint<H2: AsRef<str>>(self, hint: H2) -> InputText<'ui, 'p, L, H2, T> {
        InputText {
            ui: self.ui,
            label: self.label,
            buf: self.buf,
            flags: self.flags,
            hint: Some(hint),
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
        let label_ptr = self.ui.scratch_txt(self.label.as_ref());
        let hint_ptr = if let Some(ref hint) = self.hint {
            self.ui.scratch_txt(hint.as_ref())
        } else {
            std::ptr::null()
        };

        // Prepare buffer for ImGui
        let mut buffer = self.buf.clone().into_bytes();
        buffer.resize(256, 0); // Reserve space for input

        let result = unsafe {
            if hint_ptr.is_null() {
                sys::ImGui_InputText(
                    label_ptr,
                    buffer.as_mut_ptr() as *mut std::os::raw::c_char,
                    buffer.len(),
                    self.flags.bits(),
                    Some(callback),
                    self.buf as *mut String as *mut std::ffi::c_void,
                )
            } else {
                sys::ImGui_InputTextWithHint(
                    label_ptr,
                    hint_ptr,
                    buffer.as_mut_ptr() as *mut std::os::raw::c_char,
                    buffer.len(),
                    self.flags.bits(),
                    Some(callback),
                    self.buf as *mut String as *mut std::ffi::c_void,
                )
            }
        };

        // Update the string if changed
        if result {
            if let Some(null_pos) = buffer.iter().position(|&b| b == 0) {
                buffer.truncate(null_pos);
            }
            if let Ok(new_string) = String::from_utf8(buffer) {
                *self.buf = new_string;
            }
        }

        result
    }
}

/// Builder for multiline text input widget
#[derive(Debug)]
#[must_use]
pub struct InputTextMultiline<'ui, 'p> {
    ui: &'ui Ui,
    label: String,
    buf: &'p mut String,
    size: [f32; 2],
    flags: InputTextFlags,
}

impl<'ui, 'p> InputTextMultiline<'ui, 'p> {
    /// Creates a new multiline text input builder
    pub fn new(
        ui: &'ui Ui,
        label: impl AsRef<str>,
        buf: &'p mut String,
        size: impl Into<[f32; 2]>,
    ) -> Self {
        Self {
            ui,
            label: label.as_ref().to_string(),
            buf,
            size: size.into(),
            flags: InputTextFlags::NONE,
        }
    }

    /// Sets the flags for the input
    pub fn flags(mut self, flags: InputTextFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Makes the input read-only
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.flags.set(InputTextFlags::READ_ONLY, read_only);
        self
    }

    /// Builds the multiline text input widget
    pub fn build(self) -> bool {
        let label_ptr = self.ui.scratch_txt(&self.label);

        // Prepare buffer for ImGui
        let mut buffer = self.buf.clone().into_bytes();
        buffer.resize(1024, 0); // Reserve more space for multiline

        let size_vec: sys::ImVec2 = self.size.into();
        let result = unsafe {
            sys::ImGui_InputTextMultiline(
                label_ptr,
                buffer.as_mut_ptr() as *mut std::os::raw::c_char,
                buffer.len(),
                &size_vec,
                self.flags.bits(),
                None,
                std::ptr::null_mut(),
            )
        };

        // Update the string if changed
        if result {
            if let Some(null_pos) = buffer.iter().position(|&b| b == 0) {
                buffer.truncate(null_pos);
            }
            if let Ok(new_string) = String::from_utf8(buffer) {
                *self.buf = new_string;
            }
        }

        result
    }
}

/// Builder for integer input widget
#[derive(Debug)]
#[must_use]
pub struct InputInt<'ui> {
    ui: &'ui Ui,
    label: String,
    step: i32,
    step_fast: i32,
    flags: InputTextFlags,
}

impl<'ui> InputInt<'ui> {
    /// Creates a new integer input builder
    pub fn new(ui: &'ui Ui, label: impl AsRef<str>) -> Self {
        Self {
            ui,
            label: label.as_ref().to_string(),
            step: 1,
            step_fast: 100,
            flags: InputTextFlags::NONE,
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
    pub fn flags(mut self, flags: InputTextFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the integer input widget
    pub fn build(self, value: &mut i32) -> bool {
        let label_ptr = self.ui.scratch_txt(&self.label);
        unsafe {
            sys::ImGui_InputInt(
                label_ptr,
                value as *mut i32,
                self.step,
                self.step_fast,
                self.flags.bits(),
            )
        }
    }
}

/// Builder for float input widget
#[derive(Debug)]
#[must_use]
pub struct InputFloat<'ui> {
    ui: &'ui Ui,
    label: String,
    step: f32,
    step_fast: f32,
    format: Option<String>,
    flags: InputTextFlags,
}

impl<'ui> InputFloat<'ui> {
    /// Creates a new float input builder
    pub fn new(ui: &'ui Ui, label: impl AsRef<str>) -> Self {
        Self {
            ui,
            label: label.as_ref().to_string(),
            step: 0.0,
            step_fast: 0.0,
            format: None,
            flags: InputTextFlags::NONE,
        }
    }

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

    /// Sets the display format
    pub fn format(mut self, format: impl AsRef<str>) -> Self {
        self.format = Some(format.as_ref().to_string());
        self
    }

    /// Sets the flags for the input
    pub fn flags(mut self, flags: InputTextFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the float input widget
    pub fn build(self, value: &mut f32) -> bool {
        let label_ptr = self.ui.scratch_txt(&self.label);
        let format_ptr = self.ui.scratch_txt_opt(self.format.as_ref());
        let format_ptr = if format_ptr.is_null() {
            self.ui.scratch_txt("%.3f")
        } else {
            format_ptr
        };

        unsafe {
            sys::ImGui_InputFloat(
                label_ptr,
                value as *mut f32,
                self.step,
                self.step_fast,
                format_ptr,
                self.flags.bits(),
            )
        }
    }
}

/// Builder for double input widget
#[derive(Debug)]
#[must_use]
pub struct InputDouble<'ui> {
    ui: &'ui Ui,
    label: String,
    step: f64,
    step_fast: f64,
    format: Option<String>,
    flags: InputTextFlags,
}

impl<'ui> InputDouble<'ui> {
    /// Creates a new double input builder
    pub fn new(ui: &'ui Ui, label: impl AsRef<str>) -> Self {
        Self {
            ui,
            label: label.as_ref().to_string(),
            step: 0.0,
            step_fast: 0.0,
            format: None,
            flags: InputTextFlags::NONE,
        }
    }

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

    /// Sets the display format
    pub fn format(mut self, format: impl AsRef<str>) -> Self {
        self.format = Some(format.as_ref().to_string());
        self
    }

    /// Sets the flags for the input
    pub fn flags(mut self, flags: InputTextFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the double input widget
    pub fn build(self, value: &mut f64) -> bool {
        let label_ptr = self.ui.scratch_txt(&self.label);
        let format_ptr = self.ui.scratch_txt_opt(self.format.as_ref());
        let format_ptr = if format_ptr.is_null() {
            self.ui.scratch_txt("%.6f")
        } else {
            format_ptr
        };

        unsafe {
            sys::ImGui_InputDouble(
                label_ptr,
                value as *mut f64,
                self.step,
                self.step_fast,
                format_ptr,
                self.flags.bits(),
            )
        }
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
/// Each method is called *if and only if* the corresponding flag for each
/// method is passed to ImGui in the `callback` builder.
pub trait InputTextCallbackHandler {
    /// Filters a char -- returning a `None` means that the char is removed,
    /// and returning another char substitutes it out.
    ///
    /// To make ImGui run this callback, use [InputTextCallback::CHAR_FILTER].
    fn char_filter(&mut self, _c: char) -> Option<char> {
        None
    }

    /// Called when the user presses the completion key (TAB by default).
    ///
    /// To make ImGui run this callback, use [InputTextCallback::COMPLETION].
    fn on_completion(&mut self, _data: TextCallbackData) {}

    /// Called when the user presses Up/Down arrow keys for history navigation.
    ///
    /// To make ImGui run this callback, use [InputTextCallback::HISTORY].
    fn on_history(&mut self, _direction: HistoryDirection, _data: TextCallbackData) {}

    /// Called every frame when the input text is active.
    ///
    /// To make ImGui run this callback, use [InputTextCallback::ALWAYS].
    fn on_always(&mut self, _data: TextCallbackData) {}

    /// Called when the text buffer is edited.
    ///
    /// To make ImGui run this callback, use [InputTextCallback::EDIT].
    fn on_edit(&mut self, _data: TextCallbackData) {}
}

/// This struct provides methods to edit the underlying text buffer that
/// Dear ImGui manipulates. Primarily, it gives [remove_chars](Self::remove_chars),
/// [insert_chars](Self::insert_chars), and mutable access to what text is selected.
pub struct TextCallbackData(*mut sys::ImGuiInputTextCallbackData);

impl TextCallbackData {
    /// Creates the buffer.
    unsafe fn new(data: *mut sys::ImGuiInputTextCallbackData) -> Self {
        Self(data)
    }

    /// Get a reference to the text callback buffer's str.
    pub fn str(&self) -> &str {
        unsafe {
            std::str::from_utf8(std::slice::from_raw_parts(
                (*(self.0)).Buf as *const _,
                (*(self.0)).BufTextLen as usize,
            ))
            .expect("internal imgui error -- it boofed a utf8")
        }
    }

    /// Get the current cursor position
    pub fn cursor_pos(&self) -> usize {
        unsafe { (*(self.0)).CursorPos as usize }
    }

    /// Set the cursor position
    pub fn set_cursor_pos(&mut self, pos: usize) {
        unsafe {
            (*(self.0)).CursorPos = pos as i32;
        }
    }

    /// Get the selection start position
    pub fn selection_start(&self) -> usize {
        unsafe { (*(self.0)).SelectionStart as usize }
    }

    /// Set the selection start position
    pub fn set_selection_start(&mut self, pos: usize) {
        unsafe {
            (*(self.0)).SelectionStart = pos as i32;
        }
    }

    /// Get the selection end position
    pub fn selection_end(&self) -> usize {
        unsafe { (*(self.0)).SelectionEnd as usize }
    }

    /// Set the selection end position
    pub fn set_selection_end(&mut self, pos: usize) {
        unsafe {
            (*(self.0)).SelectionEnd = pos as i32;
        }
    }

    /// Select all text
    pub fn select_all(&mut self) {
        unsafe {
            (*(self.0)).SelectionStart = 0;
            (*(self.0)).SelectionEnd = (*(self.0)).BufTextLen;
        }
    }

    /// Clear selection
    pub fn clear_selection(&mut self) {
        unsafe {
            (*(self.0)).SelectionStart = (*(self.0)).CursorPos;
            (*(self.0)).SelectionEnd = (*(self.0)).CursorPos;
        }
    }

    /// Returns true if there is a selection
    pub fn has_selection(&self) -> bool {
        unsafe { (*(self.0)).SelectionStart != (*(self.0)).SelectionEnd }
    }

    /// Delete characters in the range [pos, pos+bytes_count)
    pub fn remove_chars(&mut self, pos: usize, bytes_count: usize) {
        unsafe {
            sys::ImGuiInputTextCallbackData_DeleteChars(self.0, pos as i32, bytes_count as i32);
        }
    }

    /// Insert text at the given position
    pub fn insert_chars(&mut self, pos: usize, text: &str) {
        let text_cstr = format!("{}\0", text);
        unsafe {
            sys::ImGuiInputTextCallbackData_InsertChars(
                self.0,
                pos as i32,
                text_cstr.as_ptr() as *const std::os::raw::c_char,
                text_cstr.as_ptr().add(text.len()) as *const std::os::raw::c_char,
            );
        }
    }
}

/// This is a ZST which implements InputTextCallbackHandler as a passthrough.
///
/// If you do not set a callback handler, this will be used (but will never
/// actually run, since you will not have passed imgui any flags).
pub struct PassthroughCallback;
impl InputTextCallbackHandler for PassthroughCallback {}

/// This is our default callback function that routes ImGui callbacks to our trait methods.
extern "C" fn callback(data: *mut sys::ImGuiInputTextCallbackData) -> c_int {
    let event_flag = unsafe { InputTextFlags::from_bits_truncate((*data).EventFlag) };
    let buffer_ptr = unsafe { (*data).UserData as *mut String };

    // Handle different callback types
    match event_flag {
        InputTextFlags::CALLBACK_RESIZE => {
            unsafe {
                let requested_size = (*data).BufSize as usize;
                let buffer = &mut *buffer_ptr;

                // Confirm that we ARE working with our string
                debug_assert_eq!(buffer.as_ptr() as *const _, (*data).Buf);

                if requested_size > buffer.capacity() {
                    let additional_bytes = requested_size - buffer.len();

                    // Reserve more data
                    buffer.reserve(additional_bytes);

                    (*data).Buf = buffer.as_mut_ptr() as *mut _;
                    (*data).BufDirty = true;
                }
            }
        }
        _ => {
            // For other callbacks, we need the actual callback handler
            // This will only work for non-PassthroughCallback types
            // PassthroughCallback should never trigger these callbacks
        }
    }

    0
}

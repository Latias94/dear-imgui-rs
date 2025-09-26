#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]
use crate::InputTextFlags;
use crate::internal::DataTypeKind;
use crate::string::ImString;
use crate::sys;
use crate::ui::Ui;
use std::ffi::{c_int, c_void};
use std::marker::PhantomData;
use std::ptr;

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

    /// Creates a single-line text input backed by ImString (zero-copy)
    pub fn input_text_imstr<'p>(
        &self,
        label: impl AsRef<str>,
        buf: &'p mut ImString,
    ) -> InputTextImStr<'_, 'p, String, String, PassthroughCallback> {
        InputTextImStr::new(self, label, buf)
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

    /// Creates a multi-line text input backed by ImString (zero-copy)
    pub fn input_text_multiline_imstr<'p>(
        &self,
        label: impl AsRef<str>,
        buf: &'p mut ImString,
        size: impl Into<[f32; 2]>,
    ) -> InputTextMultilineImStr<'_, 'p> {
        InputTextMultilineImStr::new(self, label, buf, size)
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
pub struct InputText<'ui, 'p, L = String, H = String, T = PassthroughCallback> {
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
pub struct InputTextImStr<'ui, 'p, L = String, H = String, T = PassthroughCallback> {
    ui: &'ui Ui,
    label: L,
    buf: &'p mut ImString,
    flags: InputTextFlags,
    hint: Option<H>,
    callback_handler: T,
    _phantom: PhantomData<&'ui ()>,
}

impl<'ui, 'p> InputTextImStr<'ui, 'p, String, String, PassthroughCallback> {
    pub fn new(ui: &'ui Ui, label: impl AsRef<str>, buf: &'p mut ImString) -> Self {
        Self {
            ui,
            label: label.as_ref().to_string(),
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
        let label_ptr = self.ui.scratch_txt(self.label.as_ref());
        let hint_ptr = if let Some(ref hint) = self.hint {
            self.ui.scratch_txt(hint.as_ref())
        } else {
            std::ptr::null()
        };
        let buf_ptr = self.buf.as_mut_ptr();
        let buf_size = self.buf.capacity_with_nul();
        let user_ptr = self.buf as *mut ImString as *mut c_void;

        extern "C" fn resize_cb_imstr(data: *mut sys::ImGuiInputTextCallbackData) -> c_int {
            unsafe {
                if (*data).EventFlag == (sys::ImGuiInputTextFlags_CallbackResize as i32) {
                    let im = &mut *((*data).UserData as *mut ImString);
                    let requested = (*data).BufSize as usize;
                    if im.0.len() < requested {
                        im.0.resize(requested, 0);
                    }
                    (*data).Buf = im.as_mut_ptr();
                    (*data).BufDirty = true;
                }
            }
            0
        }

        let flags = self.flags | InputTextFlags::CALLBACK_RESIZE;
        unsafe {
            if hint_ptr.is_null() {
                sys::igInputText(
                    label_ptr,
                    buf_ptr,
                    buf_size,
                    flags.bits(),
                    Some(resize_cb_imstr),
                    user_ptr,
                )
            } else {
                sys::igInputTextWithHint(
                    label_ptr,
                    hint_ptr,
                    buf_ptr,
                    buf_size,
                    flags.bits(),
                    Some(resize_cb_imstr),
                    user_ptr,
                )
            }
        }
    }
}
impl<'ui, 'p> InputText<'ui, 'p, String, String, PassthroughCallback> {
    /// Creates a new text input builder
    pub fn new(ui: &'ui Ui, label: impl AsRef<str>, buf: &'p mut String) -> Self {
        Self {
            ui,
            label: label.as_ref().to_string(),
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
    pub fn hint<H2: AsRef<str>>(self, hint: H2) -> InputText<'ui, 'p, L, H2, T> {
        InputText {
            ui: self.ui,
            label: self.label,
            buf: self.buf,
            flags: self.flags,
            capacity_hint: self.capacity_hint,
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
        let label_ptr = self.ui.scratch_txt(self.label.as_ref());
        let hint_ptr = if let Some(ref hint) = self.hint {
            self.ui.scratch_txt(hint.as_ref())
        } else {
            std::ptr::null()
        };

        // Prepare an owned, growable buffer with trailing NUL and capacity headroom
        let mut init = self.buf.as_bytes().to_vec();
        if !init.ends_with(&[0]) {
            init.push(0);
        }
        let user_cap = self.capacity_hint.unwrap_or(0);
        let min_cap = (init.len() + 64).max(256).max(user_cap);
        if init.len() < min_cap {
            init.resize(min_cap, 0);
        }
        let mut owned = Box::new(init);
        let buf_ptr = owned.as_mut_ptr() as *mut std::os::raw::c_char;
        let buf_size = owned.len();
        let user_ptr = (&mut *owned) as *mut Vec<u8> as *mut c_void;

        extern "C" fn resize_callback_vec(data: *mut sys::ImGuiInputTextCallbackData) -> c_int {
            unsafe {
                if (*data).EventFlag == (sys::ImGuiInputTextFlags_CallbackResize as i32) {
                    let vec_ptr = (*data).UserData as *mut Vec<u8>;
                    if !vec_ptr.is_null() {
                        let buf = &mut *vec_ptr;
                        let requested = (*data).BufSize as usize;
                        if buf.len() < requested {
                            buf.resize(requested, 0);
                        }
                        (*data).Buf = buf.as_mut_ptr() as *mut _;
                        (*data).BufDirty = true;
                    }
                }
            }
            0
        }

        let flags = self.flags | InputTextFlags::CALLBACK_RESIZE;

        let result = unsafe {
            if hint_ptr.is_null() {
                sys::igInputText(
                    label_ptr,
                    buf_ptr,
                    buf_size,
                    flags.bits(),
                    Some(resize_callback_vec),
                    user_ptr,
                )
            } else {
                sys::igInputTextWithHint(
                    label_ptr,
                    hint_ptr,
                    buf_ptr,
                    buf_size,
                    flags.bits(),
                    Some(resize_callback_vec),
                    user_ptr,
                )
            }
        };

        // Update the string if changed
        if result {
            let slice: &[u8] = &owned;
            let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
            if let Ok(new_string) = String::from_utf8(slice[..end].to_vec()) {
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
    capacity_hint: Option<usize>,
}

/// Builder for multiline text input backed by ImString (zero-copy)
#[derive(Debug)]
#[must_use]
pub struct InputTextMultilineImStr<'ui, 'p> {
    ui: &'ui Ui,
    label: String,
    buf: &'p mut ImString,
    size: [f32; 2],
    flags: InputTextFlags,
}

impl<'ui, 'p> InputTextMultilineImStr<'ui, 'p> {
    pub fn new(
        ui: &'ui Ui,
        label: impl AsRef<str>,
        buf: &'p mut ImString,
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
    pub fn flags(mut self, flags: InputTextFlags) -> Self {
        self.flags = flags;
        self
    }
    pub fn read_only(mut self, v: bool) -> Self {
        self.flags.set(InputTextFlags::READ_ONLY, v);
        self
    }
    pub fn build(self) -> bool {
        let label_ptr = self.ui.scratch_txt(&self.label);
        let buf_ptr = self.buf.as_mut_ptr();
        let buf_size = self.buf.capacity_with_nul();
        let user_ptr = self.buf as *mut ImString as *mut c_void;
        let size_vec: sys::ImVec2 = self.size.into();

        extern "C" fn resize_cb_imstr(data: *mut sys::ImGuiInputTextCallbackData) -> c_int {
            unsafe {
                if (*data).EventFlag == (sys::ImGuiInputTextFlags_CallbackResize as i32) {
                    let im = &mut *((*data).UserData as *mut ImString);
                    let requested = (*data).BufSize as usize;
                    if im.0.len() < requested {
                        im.0.resize(requested, 0);
                    }
                    (*data).Buf = im.as_mut_ptr();
                    (*data).BufDirty = true;
                }
            }
            0
        }

        let flags = self.flags | InputTextFlags::CALLBACK_RESIZE;
        unsafe {
            sys::igInputTextMultiline(
                label_ptr,
                buf_ptr,
                buf_size,
                size_vec,
                flags.bits(),
                Some(resize_cb_imstr),
                user_ptr,
            )
        }
    }
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
            capacity_hint: None,
        }
    }

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

    /// Makes the input read-only
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.flags.set(InputTextFlags::READ_ONLY, read_only);
        self
    }

    /// Builds the multiline text input widget
    pub fn build(self) -> bool {
        let label_ptr = self.ui.scratch_txt(&self.label);

        // Prepare an owned, growable buffer with trailing NUL and capacity headroom
        let mut init = self.buf.as_bytes().to_vec();
        if !init.ends_with(&[0]) {
            init.push(0);
        }
        let user_cap = self.capacity_hint.unwrap_or(0);
        let min_cap = (init.len() + 128).max(1024).max(user_cap);
        if init.len() < min_cap {
            init.resize(min_cap, 0);
        }
        let mut owned = Box::new(init);
        let buf_ptr = owned.as_mut_ptr() as *mut std::os::raw::c_char;
        let buf_size = owned.len();
        let user_ptr = (&mut *owned) as *mut Vec<u8> as *mut c_void;

        extern "C" fn resize_callback_vec(data: *mut sys::ImGuiInputTextCallbackData) -> c_int {
            unsafe {
                if (*data).EventFlag == (sys::ImGuiInputTextFlags_CallbackResize as i32) {
                    let vec_ptr = (*data).UserData as *mut Vec<u8>;
                    if !vec_ptr.is_null() {
                        let buf = &mut *vec_ptr;
                        let requested = (*data).BufSize as usize;
                        if buf.len() < requested {
                            buf.resize(requested, 0);
                        }
                        (*data).Buf = buf.as_mut_ptr() as *mut _;
                        (*data).BufDirty = true;
                    }
                }
            }
            0
        }

        let size_vec: sys::ImVec2 = self.size.into();
        let flags = self.flags | InputTextFlags::CALLBACK_RESIZE;
        let result = unsafe {
            sys::igInputTextMultiline(
                label_ptr,
                buf_ptr,
                buf_size,
                size_vec,
                flags.bits(),
                Some(resize_callback_vec),
                user_ptr,
            )
        };

        if result {
            let slice: &[u8] = &owned;
            let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
            if let Ok(s) = String::from_utf8(slice[..end].to_vec()) {
                *self.buf = s;
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
            sys::igInputInt(
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
            sys::igInputFloat(
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
            sys::igInputDouble(
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

    /// Gives access to the underlying byte array MUTABLY.
    ///
    /// ## Safety
    ///
    /// This is very unsafe, and the following invariants must be
    /// upheld:
    /// 1. Keep the data utf8 valid.
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
        unsafe {
            let str = std::str::from_utf8_mut(std::slice::from_raw_parts_mut(
                (*(self.0)).Buf as *const _ as *mut _,
                (*(self.0)).BufTextLen as usize,
            ))
            .expect("internal imgui error -- it boofed a utf8");

            str.as_bytes_mut()
        }
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
        unsafe {
            (*(self.0)).BufDirty = true;
        }
    }

    /// Returns the selected text directly. Note that if no text is selected,
    /// an empty str slice will be returned.
    pub fn selected(&self) -> &str {
        let start = self.selection_start().min(self.selection_end());
        let end = self.selection_start().max(self.selection_end());
        &self.str()[start..end]
    }

    /// Pushes the given str to the end of this buffer. If this
    /// would require the String to resize, it will be resized.
    /// This is automatically handled.
    pub fn push_str(&mut self, text: &str) {
        let current_len = unsafe { (*(self.0)).BufTextLen as usize };
        self.insert_chars(current_len, text);
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

/// Builder for an input scalar widget.
#[must_use]
pub struct InputScalar<'ui, 'p, T, L, F = &'static str> {
    value: &'p mut T,
    label: L,
    step: Option<T>,
    step_fast: Option<T>,
    display_format: Option<F>,
    flags: InputTextFlags,
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
            flags: InputTextFlags::empty(),
            ui,
        }
    }
}

impl<'ui, 'p, L: AsRef<str>, T: DataTypeKind, F: AsRef<str>> InputScalar<'ui, 'p, T, L, F> {
    /// Sets the display format using *a C-style printf string*
    pub fn display_format<F2: AsRef<str>>(
        self,
        display_format: F2,
    ) -> InputScalar<'ui, 'p, T, L, F2> {
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
    pub fn flags(mut self, flags: InputTextFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds an input scalar that is bound to the given value.
    ///
    /// Returns true if the value was changed.
    pub fn build(self) -> bool {
        unsafe {
            let (one, two) = self
                .ui
                .scratch_txt_with_opt(self.label, self.display_format);

            sys::igInputScalar(
                one,
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
                two,
                self.flags.bits() as i32,
            )
        }
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
    flags: InputTextFlags,
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
            flags: InputTextFlags::empty(),
            ui,
        }
    }
}

impl<'ui, 'p, L: AsRef<str>, T: DataTypeKind, F: AsRef<str>> InputScalarN<'ui, 'p, T, L, F> {
    /// Sets the display format using *a C-style printf string*
    pub fn display_format<F2: AsRef<str>>(
        self,
        display_format: F2,
    ) -> InputScalarN<'ui, 'p, T, L, F2> {
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
    pub fn flags(mut self, flags: InputTextFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds a horizontal array of multiple input scalars attached to the given slice.
    ///
    /// Returns true if any value was changed.
    pub fn build(self) -> bool {
        unsafe {
            let (one, two) = self
                .ui
                .scratch_txt_with_opt(self.label, self.display_format);

            sys::igInputScalarN(
                one,
                T::KIND as i32,
                self.values.as_mut_ptr() as *mut c_void,
                self.values.len() as i32,
                self.step
                    .as_ref()
                    .map(|step| step as *const T)
                    .unwrap_or(ptr::null()) as *const c_void,
                self.step_fast
                    .as_ref()
                    .map(|step| step as *const T)
                    .unwrap_or(ptr::null()) as *const c_void,
                two,
                self.flags.bits() as i32,
            )
        }
    }
}

/// Builder for a 2-component float input widget.
#[must_use]
pub struct InputFloat2<'ui, 'p, L, F = &'static str> {
    label: L,
    value: &'p mut [f32; 2],
    display_format: Option<F>,
    flags: InputTextFlags,
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
            flags: InputTextFlags::empty(),
            ui,
        }
    }
}

impl<'ui, 'p, L: AsRef<str>, F: AsRef<str>> InputFloat2<'ui, 'p, L, F> {
    /// Sets the display format using *a C-style printf string*
    pub fn display_format<F2: AsRef<str>>(self, display_format: F2) -> InputFloat2<'ui, 'p, L, F2> {
        InputFloat2 {
            label: self.label,
            value: self.value,
            display_format: Some(display_format),
            flags: self.flags,
            ui: self.ui,
        }
    }

    /// Sets the input text flags
    #[inline]
    pub fn flags(mut self, flags: InputTextFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the input float2 widget.
    ///
    /// Returns true if any value was changed.
    pub fn build(self) -> bool {
        unsafe {
            let (one, two) = self
                .ui
                .scratch_txt_with_opt(self.label, self.display_format);

            sys::igInputFloat2(one, self.value.as_mut_ptr(), two, self.flags.bits() as i32)
        }
    }
}

/// Builder for a 3-component float input widget.
#[must_use]
pub struct InputFloat3<'ui, 'p, L, F = &'static str> {
    label: L,
    value: &'p mut [f32; 3],
    display_format: Option<F>,
    flags: InputTextFlags,
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
            flags: InputTextFlags::empty(),
            ui,
        }
    }
}

impl<'ui, 'p, L: AsRef<str>, F: AsRef<str>> InputFloat3<'ui, 'p, L, F> {
    /// Sets the display format using *a C-style printf string*
    pub fn display_format<F2: AsRef<str>>(self, display_format: F2) -> InputFloat3<'ui, 'p, L, F2> {
        InputFloat3 {
            label: self.label,
            value: self.value,
            display_format: Some(display_format),
            flags: self.flags,
            ui: self.ui,
        }
    }

    /// Sets the input text flags
    #[inline]
    pub fn flags(mut self, flags: InputTextFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the input float3 widget.
    ///
    /// Returns true if any value was changed.
    pub fn build(self) -> bool {
        unsafe {
            let (one, two) = self
                .ui
                .scratch_txt_with_opt(self.label, self.display_format);

            sys::igInputFloat3(one, self.value.as_mut_ptr(), two, self.flags.bits() as i32)
        }
    }
}

/// Builder for a 4-component float input widget.
#[must_use]
pub struct InputFloat4<'ui, 'p, L, F = &'static str> {
    label: L,
    value: &'p mut [f32; 4],
    display_format: Option<F>,
    flags: InputTextFlags,
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
            flags: InputTextFlags::empty(),
            ui,
        }
    }
}

impl<'ui, 'p, L: AsRef<str>, F: AsRef<str>> InputFloat4<'ui, 'p, L, F> {
    /// Sets the display format using *a C-style printf string*
    pub fn display_format<F2: AsRef<str>>(self, display_format: F2) -> InputFloat4<'ui, 'p, L, F2> {
        InputFloat4 {
            label: self.label,
            value: self.value,
            display_format: Some(display_format),
            flags: self.flags,
            ui: self.ui,
        }
    }

    /// Sets the input text flags
    #[inline]
    pub fn flags(mut self, flags: InputTextFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the input float4 widget.
    ///
    /// Returns true if any value was changed.
    pub fn build(self) -> bool {
        unsafe {
            let (one, two) = self
                .ui
                .scratch_txt_with_opt(self.label, self.display_format);

            sys::igInputFloat4(one, self.value.as_mut_ptr(), two, self.flags.bits() as i32)
        }
    }
}

/// Builder for a 2-component int input widget.
#[must_use]
pub struct InputInt2<'ui, 'p, L> {
    label: L,
    value: &'p mut [i32; 2],
    flags: InputTextFlags,
    ui: &'ui Ui,
}

impl<'ui, 'p, L: AsRef<str>> InputInt2<'ui, 'p, L> {
    /// Constructs a new input int2 builder.
    #[doc(alias = "InputInt2")]
    pub fn new(ui: &'ui Ui, label: L, value: &'p mut [i32; 2]) -> Self {
        InputInt2 {
            label,
            value,
            flags: InputTextFlags::empty(),
            ui,
        }
    }

    /// Sets the input text flags
    #[inline]
    pub fn flags(mut self, flags: InputTextFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the input int2 widget.
    ///
    /// Returns true if any value was changed.
    pub fn build(self) -> bool {
        unsafe {
            let label_cstr = self.ui.scratch_txt(self.label);

            sys::igInputInt2(
                label_cstr,
                self.value.as_mut_ptr(),
                self.flags.bits() as i32,
            )
        }
    }
}

/// Builder for a 3-component int input widget.
#[must_use]
pub struct InputInt3<'ui, 'p, L> {
    label: L,
    value: &'p mut [i32; 3],
    flags: InputTextFlags,
    ui: &'ui Ui,
}

impl<'ui, 'p, L: AsRef<str>> InputInt3<'ui, 'p, L> {
    /// Constructs a new input int3 builder.
    #[doc(alias = "InputInt3")]
    pub fn new(ui: &'ui Ui, label: L, value: &'p mut [i32; 3]) -> Self {
        InputInt3 {
            label,
            value,
            flags: InputTextFlags::empty(),
            ui,
        }
    }

    /// Sets the input text flags
    #[inline]
    pub fn flags(mut self, flags: InputTextFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the input int3 widget.
    ///
    /// Returns true if any value was changed.
    pub fn build(self) -> bool {
        unsafe {
            let label_cstr = self.ui.scratch_txt(self.label);

            sys::igInputInt3(
                label_cstr,
                self.value.as_mut_ptr(),
                self.flags.bits() as i32,
            )
        }
    }
}

/// Builder for a 4-component int input widget.
#[must_use]
pub struct InputInt4<'ui, 'p, L> {
    label: L,
    value: &'p mut [i32; 4],
    flags: InputTextFlags,
    ui: &'ui Ui,
}

impl<'ui, 'p, L: AsRef<str>> InputInt4<'ui, 'p, L> {
    /// Constructs a new input int4 builder.
    #[doc(alias = "InputInt4")]
    pub fn new(ui: &'ui Ui, label: L, value: &'p mut [i32; 4]) -> Self {
        InputInt4 {
            label,
            value,
            flags: InputTextFlags::empty(),
            ui,
        }
    }

    /// Sets the input text flags
    #[inline]
    pub fn flags(mut self, flags: InputTextFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the input int4 widget.
    ///
    /// Returns true if any value was changed.
    pub fn build(self) -> bool {
        unsafe {
            let label_cstr = self.ui.scratch_txt(self.label);

            sys::igInputInt4(
                label_cstr,
                self.value.as_mut_ptr(),
                self.flags.bits() as i32,
            )
        }
    }
}

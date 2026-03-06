//! Text and scalar inputs
//!
//! Single-line and multi-line text inputs backed by `String` or `ImString`
//! (zero-copy), plus number input helpers. Builders provide flags and
//! callback hooks for validation and behavior tweaks.
//!
//! Quick examples:
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! // Text (String)
//! let mut s = String::from("hello");
//! ui.input_text("Name", &mut s).build();
//!
//! // Text (ImString, zero-copy)
//! let mut im = ImString::with_capacity(64);
//! ui.input_text_imstr("ImStr", &mut im).build();
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
use crate::InputTextFlags;
use crate::internal::DataTypeKind;
use crate::string::ImString;
use crate::sys;
use crate::ui::Ui;
use std::borrow::Cow;
use std::ffi::{c_int, c_void};
use std::marker::PhantomData;
use std::ptr;

mod callbacks;
mod numeric;

pub use callbacks::*;
pub use numeric::*;

pub(super) fn zero_string_spare_capacity(buf: &mut String) {
    let len = buf.len();
    let cap = buf.capacity();
    if cap > len {
        unsafe {
            let dst = buf.as_mut_ptr().add(len);
            ptr::write_bytes(dst, 0, cap - len);
        }
    }
}

pub(super) fn zero_string_new_capacity(buf: &mut String, old_cap: usize) {
    let new_cap = buf.capacity();
    if new_cap > old_cap {
        unsafe {
            let dst = buf.as_mut_ptr().add(old_cap);
            ptr::write_bytes(dst, 0, new_cap - old_cap);
        }
    }
}

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

    /// Creates a single-line text input backed by ImString (zero-copy)
    pub fn input_text_imstr<'ui, 'p>(
        &'ui self,
        label: impl Into<Cow<'ui, str>>,
        buf: &'p mut ImString,
    ) -> InputTextImStr<'ui, 'p> {
        InputTextImStr::new(self, label, buf)
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

    /// Creates a multi-line text input backed by ImString (zero-copy)
    pub fn input_text_multiline_imstr<'ui, 'p>(
        &'ui self,
        label: impl Into<Cow<'ui, str>>,
        buf: &'p mut ImString,
        size: impl Into<[f32; 2]>,
    ) -> InputTextMultilineImStr<'ui, 'p> {
        InputTextMultilineImStr::new(self, label, buf, size)
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

        if let Some(extra) = self.capacity_hint {
            let needed = extra.saturating_sub(self.buf.capacity().saturating_sub(self.buf.len()));
            if needed > 0 {
                self.buf.reserve(needed);
            }
        }

        // Ensure temporary NUL terminator
        self.buf.push('\0');
        // Ensure any uninitialized bytes are set to NUL so trimming does not read UB.
        zero_string_spare_capacity(self.buf);
        let capacity = self.buf.capacity();
        let buf_ptr = self.buf.as_mut_ptr() as *mut std::os::raw::c_char;

        #[repr(C)]
        struct UserData<T> {
            container: *mut String,
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
                if user.container.is_null() {
                    return 0;
                }

                let event_flag =
                    unsafe { InputTextFlags::from_bits_truncate((*data).EventFlag as i32) };
                match event_flag {
                    InputTextFlags::CALLBACK_RESIZE => unsafe {
                        let requested_i32 = (*data).BufSize;
                        if requested_i32 < 0 {
                            return 0;
                        }
                        let requested = requested_i32 as usize;
                        let s = &mut *user.container;
                        debug_assert_eq!(s.as_ptr() as *const _, (*data).Buf);
                        if requested > s.capacity() {
                            let old_cap = s.capacity();
                            let additional = requested.saturating_sub(s.len());
                            s.reserve(additional);
                            zero_string_new_capacity(s, old_cap);
                            (*data).Buf = s.as_mut_ptr() as *mut _;
                            (*data).BufDirty = true;
                        }
                        0
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
            container: self.buf as *mut String,
            handler: self.callback_handler,
        };
        let user_ptr = &mut user_data as *mut _ as *mut c_void;

        let flags = self.flags | InputTextFlags::CALLBACK_RESIZE;
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

        // Trim to first NUL (remove pushed terminator)
        let cap = unsafe { (&*user_data.container).capacity() };
        let slice = unsafe { std::slice::from_raw_parts((&*user_data.container).as_ptr(), cap) };
        if let Some(len) = slice.iter().position(|&b| b == 0) {
            unsafe { (&mut *user_data.container).as_mut_vec().set_len(len) };
        }
        result
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
    flags: InputTextFlags,
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
    flags: InputTextFlags,
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

        let flags = self.flags | InputTextFlags::CALLBACK_RESIZE;
        let result = unsafe {
            sys::igInputTextMultiline(
                label_ptr,
                buf_ptr,
                buf_size,
                size_vec,
                flags.raw(),
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
        let label_ptr = self.ui.scratch_txt(self.label.as_ref());

        // Optional pre-reserve
        if let Some(extra) = self.capacity_hint {
            let needed = extra.saturating_sub(self.buf.capacity().saturating_sub(self.buf.len()));
            if needed > 0 {
                self.buf.reserve(needed);
            }
        }

        // Ensure a NUL terminator and use String's capacity directly
        self.buf.push('\0');
        // Ensure any uninitialized bytes are set to NUL so trimming does not read UB.
        zero_string_spare_capacity(self.buf);
        let capacity = self.buf.capacity();
        let buf_ptr = self.buf.as_mut_ptr() as *mut std::os::raw::c_char;

        #[repr(C)]
        struct UserData {
            container: *mut String,
        }

        extern "C" fn callback_router(data: *mut sys::ImGuiInputTextCallbackData) -> c_int {
            if data.is_null() {
                return 0;
            }

            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                let event_flag = InputTextFlags::from_bits_truncate((*data).EventFlag as i32);
                match event_flag {
                    InputTextFlags::CALLBACK_RESIZE => {
                        let user_ptr = (*data).UserData as *mut UserData;
                        if user_ptr.is_null() {
                            return 0;
                        }
                        let requested_i32 = (*data).BufSize;
                        if requested_i32 < 0 {
                            return 0;
                        }
                        let requested = requested_i32 as usize;

                        let user = &mut *user_ptr;
                        if user.container.is_null() {
                            return 0;
                        }
                        let s = &mut *user.container;
                        debug_assert_eq!(s.as_ptr() as *const _, (*data).Buf);
                        if requested > s.capacity() {
                            let old_cap = s.capacity();
                            let additional = requested.saturating_sub(s.len());
                            s.reserve(additional);
                            zero_string_new_capacity(s, old_cap);
                            (*data).Buf = s.as_mut_ptr() as *mut _;
                            (*data).BufDirty = true;
                        }
                        0
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
            container: self.buf as *mut String,
        };
        let user_ptr = &mut user_data as *mut _ as *mut c_void;

        let size_vec: sys::ImVec2 = self.size.into();
        let flags = self.flags | InputTextFlags::CALLBACK_RESIZE;
        let result = unsafe {
            sys::igInputTextMultiline(
                label_ptr,
                buf_ptr,
                capacity,
                size_vec,
                flags.raw(),
                Some(callback_router),
                user_ptr,
            )
        };

        // Trim at NUL to restore real length
        let cap = unsafe { (&*user_data.container).capacity() };
        let slice = unsafe { std::slice::from_raw_parts((&*user_data.container).as_ptr(), cap) };
        if let Some(len) = slice.iter().position(|&b| b == 0) {
            unsafe { (&mut *user_data.container).as_mut_vec().set_len(len) };
        }
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
            self.flags.insert(InputTextFlags::CALLBACK_ALWAYS);
        }
        if callbacks.contains(InputTextCallback::CHAR_FILTER) {
            self.flags.insert(InputTextFlags::CALLBACK_CHAR_FILTER);
        }
        if callbacks.contains(InputTextCallback::EDIT) {
            self.flags.insert(InputTextFlags::CALLBACK_EDIT);
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
    flags: InputTextFlags,
    capacity_hint: Option<usize>,
    handler: T,
}

impl<'ui, 'p, T: InputTextCallbackHandler> InputTextMultilineWithCb<'ui, 'p, T> {
    pub fn build(self) -> bool {
        let label_ptr = self.ui.scratch_txt(self.label.as_ref());

        if let Some(extra) = self.capacity_hint {
            let needed = extra.saturating_sub(self.buf.capacity().saturating_sub(self.buf.len()));
            if needed > 0 {
                self.buf.reserve(needed);
            }
        }

        // Ensure NUL terminator
        self.buf.push('\0');
        // Ensure any uninitialized bytes are set to NUL so trimming does not read UB.
        zero_string_spare_capacity(self.buf);
        let capacity = self.buf.capacity();
        let buf_ptr = self.buf.as_mut_ptr() as *mut std::os::raw::c_char;

        #[repr(C)]
        struct UserData<T> {
            container: *mut String,
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
                if user.container.is_null() {
                    return 0;
                }

                let event_flag =
                    unsafe { InputTextFlags::from_bits_truncate((*data).EventFlag as i32) };
                match event_flag {
                    InputTextFlags::CALLBACK_RESIZE => unsafe {
                        let requested_i32 = (*data).BufSize;
                        if requested_i32 < 0 {
                            return 0;
                        }
                        let requested = requested_i32 as usize;
                        let s = &mut *user.container;
                        debug_assert_eq!(s.as_ptr() as *const _, (*data).Buf);
                        if requested > s.capacity() {
                            let old_cap = s.capacity();
                            let additional = requested.saturating_sub(s.len());
                            s.reserve(additional);
                            zero_string_new_capacity(s, old_cap);
                            (*data).Buf = s.as_mut_ptr() as *mut _;
                            (*data).BufDirty = true;
                        }
                        0
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
                    eprintln!("dear-imgui-rs: panic in InputText multiline callback");
                    std::process::abort();
                }
            }
        }

        let mut user_data = UserData {
            container: self.buf as *mut String,
            handler: self.handler,
        };
        let user_ptr = &mut user_data as *mut _ as *mut c_void;

        let size_vec: sys::ImVec2 = self.size.into();
        let flags = self.flags | InputTextFlags::CALLBACK_RESIZE;
        let result = unsafe {
            sys::igInputTextMultiline(
                label_ptr,
                buf_ptr,
                capacity,
                size_vec,
                flags.raw(),
                Some(callback_router::<T>),
                user_ptr,
            )
        };

        // Trim at NUL
        let cap = unsafe { (&*user_data.container).capacity() };
        let slice = unsafe { std::slice::from_raw_parts((&*user_data.container).as_ptr(), cap) };
        if let Some(len) = slice.iter().position(|&b| b == 0) {
            unsafe { (&mut *user_data.container).as_mut_vec().set_len(len) };
        }
        result
    }
}

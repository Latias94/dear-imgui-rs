use super::multiline::{InputTextMultiline, InputTextMultilineImStr};
use super::numeric::{
    InputDouble, InputFloat, InputFloat2, InputFloat3, InputFloat4, InputInt, InputInt2, InputInt3,
    InputInt4, InputScalar, InputScalarN,
};
use super::single_line::{InputText, InputTextImStr};
use crate::internal::DataTypeKind;
use crate::string::ImString;
use crate::ui::Ui;
use std::borrow::Cow;

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

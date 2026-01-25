//! Buttons
//!
//! Push-button widgets with optional sizing and configuration helpers.
//!
use crate::Ui;
use crate::sys;
use std::borrow::Cow;

impl Ui {
    /// Creates a button with the given label
    #[doc(alias = "Button")]
    pub fn button(&self, label: impl AsRef<str>) -> bool {
        self.button_config(label.as_ref()).build()
    }

    /// Creates a button with the given label and size
    #[doc(alias = "Button")]
    pub fn button_with_size(&self, label: impl AsRef<str>, size: impl Into<[f32; 2]>) -> bool {
        self.button_config(label.as_ref()).size(size).build()
    }

    /// Creates a button builder
    pub fn button_config<'ui>(&'ui self, label: impl Into<Cow<'ui, str>>) -> Button<'ui> {
        Button::new(self, label)
    }
}

impl Ui {
    /// Creates a checkbox
    #[doc(alias = "Checkbox")]
    pub fn checkbox(&self, label: impl AsRef<str>, value: &mut bool) -> bool {
        let label_ptr = self.scratch_txt(label);
        unsafe { sys::igCheckbox(label_ptr, value) }
    }

    /// Creates a radio button
    #[doc(alias = "RadioButton")]
    pub fn radio_button(&self, label: impl AsRef<str>, active: bool) -> bool {
        let label_ptr = self.scratch_txt(label);
        unsafe { sys::igRadioButton_Bool(label_ptr, active) }
    }

    /// Creates a radio button with integer value
    #[doc(alias = "RadioButton")]
    pub fn radio_button_int(&self, label: impl AsRef<str>, v: &mut i32, v_button: i32) -> bool {
        let label_ptr = self.scratch_txt(label);
        unsafe { sys::igRadioButton_IntPtr(label_ptr, v, v_button) }
    }

    /// Creates a radio button suitable for choosing an arbitrary value.
    ///
    /// Returns true if this radio button was clicked.
    #[doc(alias = "RadioButtonBool")]
    pub fn radio_button_bool(&self, label: impl AsRef<str>, active: bool) -> bool {
        let label_ptr = self.scratch_txt(label);
        unsafe { sys::igRadioButton_Bool(label_ptr, active) }
    }

    /// Renders a checkbox suitable for toggling bit flags using a mask.
    ///
    /// Returns true if this checkbox was clicked.
    ///
    /// This matches the semantics of Dear ImGui's `CheckboxFlags()` helpers:
    /// the checkbox is checked when `(*flags & mask) == mask`, and clicking it
    /// toggles the bits in `mask`.
    #[doc(alias = "CheckboxFlags")]
    pub fn checkbox_flags<T>(&self, label: impl AsRef<str>, flags: &mut T, mask: T) -> bool
    where
        T: Copy
            + PartialEq
            + std::ops::BitOrAssign
            + std::ops::BitAndAssign
            + std::ops::BitAnd<Output = T>
            + std::ops::Not<Output = T>,
    {
        let mut value = *flags & mask == mask;
        let pressed = self.checkbox(label, &mut value);
        if pressed {
            if value {
                *flags |= mask;
            } else {
                *flags &= !mask;
            }
        }
        pressed
    }
}

/// Builder for button widget
pub struct Button<'ui> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    size: Option<[f32; 2]>,
}

impl<'ui> Button<'ui> {
    /// Creates a new button builder
    pub fn new(ui: &'ui Ui, label: impl Into<Cow<'ui, str>>) -> Self {
        Self {
            ui,
            label: label.into(),
            size: None,
        }
    }

    /// Sets the size of the button
    pub fn size(mut self, size: impl Into<[f32; 2]>) -> Self {
        self.size = Some(size.into());
        self
    }

    /// Builds the button
    pub fn build(self) -> bool {
        let label_ptr = self.ui.scratch_txt(self.label.as_ref());
        let size = self.size.unwrap_or([0.0, 0.0]);
        let size_vec: sys::ImVec2 = size.into();
        unsafe { sys::igButton(label_ptr, size_vec) }
    }
}

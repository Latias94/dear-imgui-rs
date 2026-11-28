//! Buttons
//!
//! Push-button widgets with optional sizing and configuration helpers.
//!
use crate::Ui;
use crate::sys;
use crate::validation::assert_finite_vec2;
use std::borrow::Cow;

impl Ui {
    /// Creates a button with the given label
    #[doc(alias = "Button")]
    #[inline]
    pub fn button(&self, label: impl AsRef<str>) -> bool {
        self.with_scratch_txt(label, |label_ptr| unsafe {
            sys::igButton(label_ptr, [0.0, 0.0].into())
        })
    }

    /// Creates a button with the given label and size
    #[doc(alias = "Button")]
    pub fn button_with_size(&self, label: impl AsRef<str>, size: impl Into<[f32; 2]>) -> bool {
        let size = size.into();
        assert_finite_vec2("Ui::button_with_size()", "size", size);
        self.with_scratch_txt(label, |label_ptr| unsafe {
            sys::igButton(label_ptr, size.into())
        })
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
        self.with_scratch_txt(label, |label_ptr| unsafe {
            sys::igCheckbox(label_ptr, value)
        })
    }

    /// Creates a radio button
    #[doc(alias = "RadioButton")]
    pub fn radio_button(&self, label: impl AsRef<str>, active: bool) -> bool {
        self.with_scratch_txt(label, |label_ptr| unsafe {
            sys::igRadioButton_Bool(label_ptr, active)
        })
    }

    /// Creates a radio button with integer value
    #[doc(alias = "RadioButton")]
    pub fn radio_button_int(&self, label: impl AsRef<str>, v: &mut i32, v_button: i32) -> bool {
        self.with_scratch_txt(label, |label_ptr| unsafe {
            sys::igRadioButton_IntPtr(label_ptr, v, v_button)
        })
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
        match self.size {
            Some(size) => self.ui.button_with_size(self.label, size),
            None => self.ui.button(self.label),
        }
    }
}

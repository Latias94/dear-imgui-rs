use super::super::validation::validate_input_scalar_flags;
use crate::ui::Ui;
use crate::{InputScalarFlags, sys};

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
    pub fn flags(mut self, flags: InputScalarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the input float2 widget.
    ///
    /// Returns true if any value was changed.
    pub fn build(self) -> bool {
        validate_input_scalar_flags("InputFloat2::build()", self.flags);
        unsafe {
            let (one, two) = self
                .ui
                .scratch_txt_with_opt(self.label, self.display_format);

            sys::igInputFloat2(one, self.value.as_mut_ptr(), two, self.flags.raw())
        }
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
    pub fn flags(mut self, flags: InputScalarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the input float3 widget.
    ///
    /// Returns true if any value was changed.
    pub fn build(self) -> bool {
        validate_input_scalar_flags("InputFloat3::build()", self.flags);
        unsafe {
            let (one, two) = self
                .ui
                .scratch_txt_with_opt(self.label, self.display_format);

            sys::igInputFloat3(one, self.value.as_mut_ptr(), two, self.flags.raw())
        }
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
    pub fn flags(mut self, flags: InputScalarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the input float4 widget.
    ///
    /// Returns true if any value was changed.
    pub fn build(self) -> bool {
        validate_input_scalar_flags("InputFloat4::build()", self.flags);
        unsafe {
            let (one, two) = self
                .ui
                .scratch_txt_with_opt(self.label, self.display_format);

            sys::igInputFloat4(one, self.value.as_mut_ptr(), two, self.flags.raw())
        }
    }
}

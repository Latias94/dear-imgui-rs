use crate::InputTextFlags;
use crate::internal::DataTypeKind;
use crate::sys;
use crate::ui::Ui;
use std::borrow::Cow;
use std::ffi::c_void;
use std::ptr;

/// Builder for integer input widget
#[derive(Debug)]
#[must_use]
pub struct InputInt<'ui> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    step: i32,
    step_fast: i32,
    flags: InputTextFlags,
}

impl<'ui> InputInt<'ui> {
    /// Creates a new integer input builder
    pub fn new(ui: &'ui Ui, label: impl Into<Cow<'ui, str>>) -> Self {
        Self {
            ui,
            label: label.into(),
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
        let label_ptr = self.ui.scratch_txt(self.label.as_ref());
        unsafe {
            sys::igInputInt(
                label_ptr,
                value as *mut i32,
                self.step,
                self.step_fast,
                self.flags.raw(),
            )
        }
    }
}

/// Builder for float input widget
#[derive(Debug)]
#[must_use]
pub struct InputFloat<'ui> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    step: f32,
    step_fast: f32,
    format: Option<Cow<'ui, str>>,
    flags: InputTextFlags,
}

impl<'ui> InputFloat<'ui> {
    /// Creates a new float input builder
    pub fn new(ui: &'ui Ui, label: impl Into<Cow<'ui, str>>) -> Self {
        Self {
            ui,
            label: label.into(),
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
    pub fn format(mut self, format: impl Into<Cow<'ui, str>>) -> Self {
        self.format = Some(format.into());
        self
    }

    /// Sets the flags for the input
    pub fn flags(mut self, flags: InputTextFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the float input widget
    pub fn build(self, value: &mut f32) -> bool {
        let format = self.format.as_deref().unwrap_or("%.3f");
        let (label_ptr, format_ptr) = self.ui.scratch_txt_two(self.label.as_ref(), format);

        unsafe {
            sys::igInputFloat(
                label_ptr,
                value as *mut f32,
                self.step,
                self.step_fast,
                format_ptr,
                self.flags.raw(),
            )
        }
    }
}

/// Builder for double input widget
#[derive(Debug)]
#[must_use]
pub struct InputDouble<'ui> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    step: f64,
    step_fast: f64,
    format: Option<Cow<'ui, str>>,
    flags: InputTextFlags,
}

impl<'ui> InputDouble<'ui> {
    /// Creates a new double input builder
    pub fn new(ui: &'ui Ui, label: impl Into<Cow<'ui, str>>) -> Self {
        Self {
            ui,
            label: label.into(),
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
    pub fn format(mut self, format: impl Into<Cow<'ui, str>>) -> Self {
        self.format = Some(format.into());
        self
    }

    /// Sets the flags for the input
    pub fn flags(mut self, flags: InputTextFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the double input widget
    pub fn build(self, value: &mut f64) -> bool {
        let format = self.format.as_deref().unwrap_or("%.6f");
        let (label_ptr, format_ptr) = self.ui.scratch_txt_two(self.label.as_ref(), format);

        unsafe {
            sys::igInputDouble(
                label_ptr,
                value as *mut f64,
                self.step,
                self.step_fast,
                format_ptr,
                self.flags.raw(),
            )
        }
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
                self.flags.raw(),
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
        let count = match i32::try_from(self.values.len()) {
            Ok(n) => n,
            Err(_) => return false,
        };
        unsafe {
            let (one, two) = self
                .ui
                .scratch_txt_with_opt(self.label, self.display_format);

            sys::igInputScalarN(
                one,
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
                two,
                self.flags.raw(),
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

            sys::igInputFloat4(one, self.value.as_mut_ptr(), two, self.flags.raw())
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

            sys::igInputInt2(label_cstr, self.value.as_mut_ptr(), self.flags.raw())
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

            sys::igInputInt3(label_cstr, self.value.as_mut_ptr(), self.flags.raw())
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

            sys::igInputInt4(label_cstr, self.value.as_mut_ptr(), self.flags.raw())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{zero_string_new_capacity, zero_string_spare_capacity};

    #[test]
    fn zero_string_spare_capacity_writes_nul_bytes() {
        let mut s = String::with_capacity(16);
        s.push_str("abc");
        let len = s.len();
        let cap = s.capacity();

        zero_string_spare_capacity(&mut s);

        unsafe {
            let bytes = std::slice::from_raw_parts(s.as_ptr(), cap);
            assert_eq!(&bytes[..len], b"abc");
            assert!(bytes[len..].iter().all(|&b| b == 0));
        }
    }

    #[test]
    fn zero_string_new_capacity_writes_new_region() {
        let mut s = String::with_capacity(4);
        s.push_str("abc");
        let old_cap = s.capacity();

        s.reserve(64);
        let new_cap = s.capacity();
        assert!(new_cap > old_cap);

        zero_string_new_capacity(&mut s, old_cap);

        unsafe {
            let tail = std::slice::from_raw_parts(s.as_ptr().add(old_cap), new_cap - old_cap);
            assert!(tail.iter().all(|&b| b == 0));
        }
    }
}

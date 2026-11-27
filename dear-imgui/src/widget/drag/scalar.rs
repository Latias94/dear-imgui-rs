use std::borrow::Cow;
use std::os::raw::c_void;
use std::ptr;

use crate::internal::{DataTypeKind, component_count_i32};
use crate::sys;
use crate::{NumericFormat, NumericFormatError, Ui};

use super::DragFlags;
use super::validation::validate_drag_flags;

/// Builder for a drag slider widget
#[derive(Clone, Debug)]
#[must_use]
pub struct Drag<'ui, T, L, F = &'static str> {
    ui: &'ui Ui,
    label: L,
    speed: f32,
    min: Option<T>,
    max: Option<T>,
    display_format: Option<F>,
    flags: DragFlags,
}

impl<'ui, L: AsRef<str>, T: DataTypeKind> Drag<'ui, T, L> {
    /// Constructs a new drag slider builder
    #[doc(alias = "DragScalar", alias = "DragScalarN")]
    pub(super) fn new(ui: &'ui Ui, label: L) -> Self {
        Drag {
            ui,
            label,
            speed: 1.0,
            min: None,
            max: None,
            display_format: None,
            flags: DragFlags::empty(),
        }
    }
}

impl<'ui, L: AsRef<str>, T: DataTypeKind, F: AsRef<str>> Drag<'ui, T, L, F> {
    /// Sets the range (inclusive)
    pub fn range(mut self, min: T, max: T) -> Self {
        self.min = Some(min);
        self.max = Some(max);
        self
    }

    /// Sets the value increment for a movement of one pixel
    ///
    /// Example: speed=0.2 means mouse needs to move 5 pixels to increase the slider value by 1
    pub fn speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }

    /// Sets the validated display format.
    pub fn display_format<'fmt>(
        self,
        display_format: NumericFormat<'fmt, T>,
    ) -> Drag<'ui, T, L, NumericFormat<'fmt, T>> {
        Drag {
            ui: self.ui,
            label: self.label,
            speed: self.speed,
            min: self.min,
            max: self.max,
            display_format: Some(display_format),
            flags: self.flags,
        }
    }

    /// Validates and sets a C-style display format.
    pub fn try_display_format<'fmt>(
        self,
        display_format: impl Into<Cow<'fmt, str>>,
    ) -> Result<Drag<'ui, T, L, NumericFormat<'fmt, T>>, NumericFormatError> {
        Ok(self.display_format(NumericFormat::new(display_format)?))
    }

    /// Replaces all current settings with the given flags
    pub fn flags(mut self, flags: DragFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds a drag slider that is bound to the given value
    ///
    /// Returns true if the slider value was changed
    pub fn build(self, value: &mut T) -> bool {
        validate_drag_flags("Drag::build()", self.flags);
        self.ui
            .with_scratch_txt_with_opt(self.label, self.display_format, |label, format| unsafe {
                sys::igDragScalar(
                    label,
                    T::KIND as i32,
                    value as *mut T as *mut c_void,
                    self.speed,
                    self.min
                        .as_ref()
                        .map(|min| min as *const T)
                        .unwrap_or(ptr::null()) as *const c_void,
                    self.max
                        .as_ref()
                        .map(|max| max as *const T)
                        .unwrap_or(ptr::null()) as *const c_void,
                    format,
                    self.flags.bits(),
                )
            })
    }

    /// Builds a horizontal array of multiple drag sliders attached to the given slice
    ///
    /// Returns true if any slider value was changed
    pub fn build_array(self, values: &mut [T]) -> bool {
        validate_drag_flags("Drag::build_array()", self.flags);
        let count = component_count_i32("Drag::build_array()", values.len());
        if self.flags.contains(DragFlags::COLOR_MARKERS) {
            assert!(
                count <= 4,
                "Drag::build_array() supports at most 4 components with COLOR_MARKERS"
            );
        }
        self.ui
            .with_scratch_txt_with_opt(self.label, self.display_format, |label, format| unsafe {
                sys::igDragScalarN(
                    label,
                    T::KIND as i32,
                    values.as_mut_ptr() as *mut c_void,
                    count,
                    self.speed,
                    self.min
                        .as_ref()
                        .map(|min| min as *const T)
                        .unwrap_or(ptr::null()) as *const c_void,
                    self.max
                        .as_ref()
                        .map(|max| max as *const T)
                        .unwrap_or(ptr::null()) as *const c_void,
                    format,
                    self.flags.bits(),
                )
            })
    }
}

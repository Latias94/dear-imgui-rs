use std::os::raw::c_void;
use std::ptr;

use crate::Ui;
use crate::internal::{DataTypeKind, component_count_i32};
use crate::sys;

use super::DragFlags;
use super::validation::validate_drag_flags;

/// Builder for a drag slider widget
#[derive(Clone, Debug)]
#[must_use]
pub struct Drag<T, L, F = &'static str> {
    label: L,
    speed: f32,
    min: Option<T>,
    max: Option<T>,
    display_format: Option<F>,
    flags: DragFlags,
}

impl<L: AsRef<str>, T: DataTypeKind> Drag<T, L> {
    /// Constructs a new drag slider builder
    #[doc(alias = "DragScalar", alias = "DragScalarN")]
    pub fn new(label: L) -> Self {
        Drag {
            label,
            speed: 1.0,
            min: None,
            max: None,
            display_format: None,
            flags: DragFlags::empty(),
        }
    }
}

impl<L: AsRef<str>, T: DataTypeKind, F: AsRef<str>> Drag<T, L, F> {
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

    /// Sets the display format using *a C-style printf string*
    pub fn display_format<F2: AsRef<str>>(self, display_format: F2) -> Drag<T, L, F2> {
        Drag {
            label: self.label,
            speed: self.speed,
            min: self.min,
            max: self.max,
            display_format: Some(display_format),
            flags: self.flags,
        }
    }

    /// Replaces all current settings with the given flags
    pub fn flags(mut self, flags: DragFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds a drag slider that is bound to the given value
    ///
    /// Returns true if the slider value was changed
    pub fn build(self, ui: &Ui, value: &mut T) -> bool {
        validate_drag_flags("Drag::build()", self.flags);
        unsafe {
            let (one, two) = ui.scratch_txt_with_opt(self.label, self.display_format);

            sys::igDragScalar(
                one,
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
                two,
                self.flags.bits(),
            )
        }
    }

    /// Builds a horizontal array of multiple drag sliders attached to the given slice
    ///
    /// Returns true if any slider value was changed
    pub fn build_array(self, ui: &Ui, values: &mut [T]) -> bool {
        validate_drag_flags("Drag::build_array()", self.flags);
        let count = component_count_i32("Drag::build_array()", values.len());
        if self.flags.contains(DragFlags::COLOR_MARKERS) {
            assert!(
                count <= 4,
                "Drag::build_array() supports at most 4 components with COLOR_MARKERS"
            );
        }
        unsafe {
            let (one, two) = ui.scratch_txt_with_opt(self.label, self.display_format);

            sys::igDragScalarN(
                one,
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
                two,
                self.flags.bits(),
            )
        }
    }
}

use crate::Ui;
use crate::internal::DataTypeKind;
use crate::sys;

use super::DragFlags;
use super::validation::validate_drag_flags;

/// Builder for a drag range slider widget
#[derive(Clone, Debug)]
#[must_use]
pub struct DragRange<T, L, F = &'static str, M = &'static str> {
    label: L,
    speed: f32,
    min: Option<T>,
    max: Option<T>,
    display_format: Option<F>,
    max_display_format: Option<M>,
    flags: DragFlags,
}

impl<T: DataTypeKind, L: AsRef<str>> DragRange<T, L> {
    /// Constructs a new drag range slider builder
    #[doc(alias = "DragIntRange2", alias = "DragFloatRange2")]
    pub fn new(label: L) -> DragRange<T, L> {
        DragRange {
            label,
            speed: 1.0,
            min: None,
            max: None,
            display_format: None,
            max_display_format: None,
            flags: DragFlags::NONE,
        }
    }
}

impl<T, L, F, M> DragRange<T, L, F, M>
where
    T: DataTypeKind,
    L: AsRef<str>,
    F: AsRef<str>,
    M: AsRef<str>,
{
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
    pub fn display_format<F2: AsRef<str>>(self, display_format: F2) -> DragRange<T, L, F2, M> {
        DragRange {
            label: self.label,
            speed: self.speed,
            min: self.min,
            max: self.max,
            display_format: Some(display_format),
            max_display_format: self.max_display_format,
            flags: self.flags,
        }
    }

    /// Sets the display format for the max value using *a C-style printf string*
    pub fn max_display_format<M2: AsRef<str>>(
        self,
        max_display_format: M2,
    ) -> DragRange<T, L, F, M2> {
        DragRange {
            label: self.label,
            speed: self.speed,
            min: self.min,
            max: self.max,
            display_format: self.display_format,
            max_display_format: Some(max_display_format),
            flags: self.flags,
        }
    }

    /// Replaces all current settings with the given flags
    pub fn flags(mut self, flags: DragFlags) -> Self {
        self.flags = flags;
        self
    }
}

impl<L, F, M> DragRange<f32, L, F, M>
where
    L: AsRef<str>,
    F: AsRef<str>,
    M: AsRef<str>,
{
    /// Builds a drag range slider that is bound to the given min/max values
    ///
    /// Returns true if the slider value was changed
    #[doc(alias = "DragFloatRange2")]
    pub fn build(self, ui: &Ui, min: &mut f32, max: &mut f32) -> bool {
        validate_drag_flags("DragRange::build()", self.flags);
        ui.run_with_bound_context(|| unsafe {
            let buffer = &mut *ui.scratch_buffer().get();
            buffer.refresh_buffer();

            let label_start = buffer.push(self.label);
            let display_format = self.display_format.as_ref().map(|v| buffer.push(v));
            let max_display_format = self.max_display_format.as_ref().map(|v| buffer.push(v));

            let label = buffer.offset(label_start);
            let display_format = display_format
                .map(|v| buffer.offset(v))
                .unwrap_or_else(std::ptr::null);
            let max_display_format = max_display_format
                .map(|v| buffer.offset(v))
                .unwrap_or_else(std::ptr::null);

            sys::igDragFloatRange2(
                label,
                min as *mut f32,
                max as *mut f32,
                self.speed,
                self.min.unwrap_or(0.0),
                self.max.unwrap_or(0.0),
                display_format,
                max_display_format,
                self.flags.bits(),
            )
        })
    }
}

impl<L, F, M> DragRange<i32, L, F, M>
where
    L: AsRef<str>,
    F: AsRef<str>,
    M: AsRef<str>,
{
    /// Builds a drag range slider that is bound to the given min/max values
    ///
    /// Returns true if the slider value was changed
    #[doc(alias = "DragIntRange2")]
    pub fn build(self, ui: &Ui, min: &mut i32, max: &mut i32) -> bool {
        validate_drag_flags("DragRange::build()", self.flags);
        ui.run_with_bound_context(|| unsafe {
            let buffer = &mut *ui.scratch_buffer().get();
            buffer.refresh_buffer();

            let label_start = buffer.push(self.label);
            let display_format = self.display_format.as_ref().map(|v| buffer.push(v));
            let max_display_format = self.max_display_format.as_ref().map(|v| buffer.push(v));

            let label = buffer.offset(label_start);
            let display_format = display_format
                .map(|v| buffer.offset(v))
                .unwrap_or_else(std::ptr::null);
            let max_display_format = max_display_format
                .map(|v| buffer.offset(v))
                .unwrap_or_else(std::ptr::null);

            sys::igDragIntRange2(
                label,
                min as *mut i32,
                max as *mut i32,
                self.speed,
                self.min.unwrap_or(0),
                self.max.unwrap_or(0),
                display_format,
                max_display_format,
                self.flags.bits(),
            )
        })
    }
}

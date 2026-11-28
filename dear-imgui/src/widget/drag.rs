//! Drag slider widgets for numeric input
//!
//! Drag sliders allow users to modify numeric values by dragging with the mouse.
//! They provide a more intuitive way to adjust values compared to text input.

use crate::internal::{DataTypeKind, component_count_i32};
use crate::{ContextError, NumericFormat, Ui, sys};
use std::borrow::Cow;
use std::ffi::{c_char, c_void};
use std::ptr;

impl Ui {
    /// Creates a new drag slider widget. Returns true if the value has been edited.
    pub fn drag<T: AsRef<str>, K: DataTypeKind>(&self, label: T, value: &mut K) -> bool {
        self.drag_config(label).build(value)
    }

    /// Creates a new unbuilt Drag.
    pub fn drag_config<T: AsRef<str>, K: DataTypeKind>(&self, label: T) -> Drag<'_, K, T> {
        Drag::new(self, label)
    }

    /// Creates a drag float slider.
    #[doc(alias = "DragFloat")]
    pub fn drag_float(&self, label: impl AsRef<str>, value: &mut f32) -> bool {
        self.drag(label, value)
    }

    /// Creates a drag float slider builder.
    #[doc(alias = "DragFloat")]
    pub fn drag_float_config<L: AsRef<str>>(&self, label: L) -> Drag<'_, f32, L> {
        self.drag_config(label)
    }

    /// Creates a drag integer slider.
    #[doc(alias = "DragInt")]
    pub fn drag_int(&self, label: impl AsRef<str>, value: &mut i32) -> bool {
        self.drag(label, value)
    }

    /// Creates a drag integer slider builder.
    #[doc(alias = "DragInt")]
    pub fn drag_int_config<L: AsRef<str>>(&self, label: L) -> Drag<'_, i32, L> {
        self.drag_config(label)
    }

    /// Creates a drag float range slider.
    #[doc(alias = "DragFloatRange2")]
    pub fn drag_float_range2(&self, label: impl AsRef<str>, min: &mut f32, max: &mut f32) -> bool {
        self.drag_float_range2_config(label).build(min, max)
    }

    /// Creates a drag float range slider builder.
    #[doc(alias = "DragFloatRange2")]
    pub fn drag_float_range2_config<L: AsRef<str>>(&self, label: L) -> DragRange<'_, f32, L> {
        DragRange::new(self, label)
    }

    /// Creates a drag integer range slider.
    #[doc(alias = "DragIntRange2")]
    pub fn drag_int_range2(&self, label: impl AsRef<str>, min: &mut i32, max: &mut i32) -> bool {
        self.drag_int_range2_config(label).build(min, max)
    }

    /// Creates a drag integer range slider builder.
    #[doc(alias = "DragIntRange2")]
    pub fn drag_int_range2_config<L: AsRef<str>>(&self, label: L) -> DragRange<'_, i32, L> {
        DragRange::new(self, label)
    }

    /// Creates a drag float2 slider (2 floats)
    #[doc(alias = "DragFloat2")]
    pub fn drag_float2(&self, label: impl AsRef<str>, values: &mut [f32; 2]) -> bool {
        self.with_scratch_txt(label, |label_cstr| unsafe {
            sys::igDragFloat2(
                label_cstr,
                values.as_mut_ptr(),
                1.0,
                0.0,
                0.0,
                ptr::null(),
                0,
            )
        })
    }

    /// Creates a drag float3 slider (3 floats)
    #[doc(alias = "DragFloat3")]
    pub fn drag_float3(&self, label: impl AsRef<str>, values: &mut [f32; 3]) -> bool {
        self.with_scratch_txt(label, |label_cstr| unsafe {
            sys::igDragFloat3(
                label_cstr,
                values.as_mut_ptr(),
                1.0,
                0.0,
                0.0,
                ptr::null(),
                0,
            )
        })
    }

    /// Creates a drag float4 slider (4 floats)
    #[doc(alias = "DragFloat4")]
    pub fn drag_float4(&self, label: impl AsRef<str>, values: &mut [f32; 4]) -> bool {
        self.with_scratch_txt(label, |label_cstr| unsafe {
            sys::igDragFloat4(
                label_cstr,
                values.as_mut_ptr(),
                1.0,
                0.0,
                0.0,
                ptr::null(),
                0,
            )
        })
    }

    /// Creates a drag int2 slider (2 ints)
    #[doc(alias = "DragInt2")]
    pub fn drag_int2(&self, label: impl AsRef<str>, values: &mut [i32; 2]) -> bool {
        self.with_scratch_txt(label, |label_cstr| unsafe {
            sys::igDragInt2(label_cstr, values.as_mut_ptr(), 1.0, 0, 0, ptr::null(), 0)
        })
    }

    /// Creates a drag int3 slider (3 ints)
    #[doc(alias = "DragInt3")]
    pub fn drag_int3(&self, label: impl AsRef<str>, values: &mut [i32; 3]) -> bool {
        self.with_scratch_txt(label, |label_cstr| unsafe {
            sys::igDragInt3(label_cstr, values.as_mut_ptr(), 1.0, 0, 0, ptr::null(), 0)
        })
    }

    /// Creates a drag int4 slider (4 ints)
    #[doc(alias = "DragInt4")]
    pub fn drag_int4(&self, label: impl AsRef<str>, values: &mut [i32; 4]) -> bool {
        self.with_scratch_txt(label, |label_cstr| unsafe {
            sys::igDragInt4(label_cstr, values.as_mut_ptr(), 1.0, 0, 0, ptr::null(), 0)
        })
    }
}

bitflags::bitflags! {
    /// Flags for drag widgets.
    ///
    /// Dear ImGui shares the underlying `ImGuiSliderFlags` enum between drag
    /// and slider widgets, but `WRAP_AROUND` is supported by DragXXX only.
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct DragFlags: i32 {
        /// No flags.
        const NONE = 0;
        /// Wrap the value around when exceeding the current range.
        const WRAP_AROUND = sys::ImGuiSliderFlags_WrapAround;
        /// Clamp on input when editing via CTRL+Click or direct text input.
        const CLAMP_ON_INPUT = sys::ImGuiSliderFlags_ClampOnInput;
        /// Clamp zero-range drags to avoid a zero-sized range.
        const CLAMP_ZERO_RANGE = sys::ImGuiSliderFlags_ClampZeroRange;
        /// Disable small "smart" speed tweaks for very small/large ranges.
        const NO_SPEED_TWEAKS = sys::ImGuiSliderFlags_NoSpeedTweaks;
        /// Clamp value to min/max bounds when input manually with CTRL+Click.
        const ALWAYS_CLAMP = sys::ImGuiSliderFlags_AlwaysClamp;
        /// Make the widget logarithmic.
        const LOGARITHMIC = sys::ImGuiSliderFlags_Logarithmic;
        /// Disable rounding underlying value to match the display format.
        const NO_ROUND_TO_FORMAT = sys::ImGuiSliderFlags_NoRoundToFormat;
        /// Disable CTRL+Click or Enter key allowing direct text input.
        const NO_INPUT = sys::ImGuiSliderFlags_NoInput;
        /// Draw R/G/B/A color markers on each component.
        ///
        /// Dear ImGui only defines four default component colors.
        const COLOR_MARKERS = sys::ImGuiSliderFlags_ColorMarkers;
    }
}

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
    fn new(ui: &'ui Ui, label: L) -> Self {
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
    ) -> Result<Drag<'ui, T, L, NumericFormat<'fmt, T>>, ContextError> {
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

/// Builder for a drag range slider widget
#[derive(Clone, Debug)]
#[must_use]
pub struct DragRange<'ui, T, L, F = &'static str, M = &'static str> {
    ui: &'ui Ui,
    label: L,
    speed: f32,
    min: Option<T>,
    max: Option<T>,
    display_format: Option<F>,
    max_display_format: Option<M>,
    flags: DragFlags,
}

impl<'ui, T: DataTypeKind, L: AsRef<str>> DragRange<'ui, T, L> {
    /// Constructs a new drag range slider builder
    #[doc(alias = "DragIntRange2", alias = "DragFloatRange2")]
    fn new(ui: &'ui Ui, label: L) -> Self {
        DragRange {
            ui,
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

impl<'ui, T, L, F, M> DragRange<'ui, T, L, F, M>
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

    /// Sets the validated display format.
    pub fn display_format<'fmt>(
        self,
        display_format: NumericFormat<'fmt, T>,
    ) -> DragRange<'ui, T, L, NumericFormat<'fmt, T>, M> {
        DragRange {
            ui: self.ui,
            label: self.label,
            speed: self.speed,
            min: self.min,
            max: self.max,
            display_format: Some(display_format),
            max_display_format: self.max_display_format,
            flags: self.flags,
        }
    }

    /// Validates and sets a C-style display format.
    pub fn try_display_format<'fmt>(
        self,
        display_format: impl Into<Cow<'fmt, str>>,
    ) -> Result<DragRange<'ui, T, L, NumericFormat<'fmt, T>, M>, ContextError> {
        Ok(self.display_format(NumericFormat::new(display_format)?))
    }

    /// Sets the validated display format for the maximum value.
    pub fn max_display_format<'fmt>(
        self,
        max_display_format: NumericFormat<'fmt, T>,
    ) -> DragRange<'ui, T, L, F, NumericFormat<'fmt, T>> {
        DragRange {
            ui: self.ui,
            label: self.label,
            speed: self.speed,
            min: self.min,
            max: self.max,
            display_format: self.display_format,
            max_display_format: Some(max_display_format),
            flags: self.flags,
        }
    }

    /// Validates and sets a C-style display format for the maximum value.
    pub fn try_max_display_format<'fmt>(
        self,
        max_display_format: impl Into<Cow<'fmt, str>>,
    ) -> Result<DragRange<'ui, T, L, F, NumericFormat<'fmt, T>>, ContextError> {
        Ok(self.max_display_format(NumericFormat::new(max_display_format)?))
    }

    /// Replaces all current settings with the given flags
    pub fn flags(mut self, flags: DragFlags) -> Self {
        self.flags = flags;
        self
    }

    fn with_scratch_labels(
        &self,
        submit: impl FnOnce(*const c_char, *const c_char, *const c_char) -> bool,
    ) -> bool {
        let ui = self.ui;
        ui.run_with_bound_context(|| {
            let label = self.label.as_ref();
            let format = self.display_format.as_ref().map(AsRef::as_ref);
            let max_format = self.max_display_format.as_ref().map(AsRef::as_ref);
            ui.with_scratch_buffer(|buffer| {
                buffer.refresh_buffer();
                let label = buffer.push(label);
                let format = format.map(|value| buffer.push(value));
                let max_format = max_format.map(|value| buffer.push(value));
                // Resolve offsets only after all strings have been appended.
                let (label, format, max_format) = unsafe {
                    (
                        buffer.offset(label),
                        format.map_or(std::ptr::null(), |offset| buffer.offset(offset)),
                        max_format.map_or(std::ptr::null(), |offset| buffer.offset(offset)),
                    )
                };
                submit(label, format, max_format)
            })
        })
    }
}

impl<L, F, M> DragRange<'_, f32, L, F, M>
where
    L: AsRef<str>,
    F: AsRef<str>,
    M: AsRef<str>,
{
    /// Builds a drag range slider that is bound to the given min/max values
    ///
    /// Returns true if the slider value was changed
    #[doc(alias = "DragFloatRange2")]
    pub fn build(self, min: &mut f32, max: &mut f32) -> bool {
        validate_drag_flags("DragRange::build()", self.flags);
        self.with_scratch_labels(|label, display_format, max_display_format| unsafe {
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

impl<L, F, M> DragRange<'_, i32, L, F, M>
where
    L: AsRef<str>,
    F: AsRef<str>,
    M: AsRef<str>,
{
    /// Builds a drag range slider that is bound to the given min/max values
    ///
    /// Returns true if the slider value was changed
    #[doc(alias = "DragIntRange2")]
    pub fn build(self, min: &mut i32, max: &mut i32) -> bool {
        validate_drag_flags("DragRange::build()", self.flags);
        self.with_scratch_labels(|label, display_format, max_display_format| unsafe {
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

fn validate_drag_flags(caller: &str, flags: DragFlags) {
    let unsupported = flags.bits() & !DragFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiSliderFlags bits: 0x{unsupported:X}"
    );
}

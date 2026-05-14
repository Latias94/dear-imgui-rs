//! Drag slider widgets for numeric input
//!
//! Drag sliders allow users to modify numeric values by dragging with the mouse.
//! They provide a more intuitive way to adjust values compared to text input.

use std::os::raw::c_void;
use std::ptr;

use crate::Ui;
use crate::internal::{DataTypeKind, component_count_i32};
use crate::sys;
use crate::widget::slider::SliderFlags;

fn validate_drag_flags(caller: &str, flags: DragFlags) {
    let unsupported = flags.bits() & !DragFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiSliderFlags bits: 0x{unsupported:X}"
    );
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
        const WRAP_AROUND = sys::ImGuiSliderFlags_WrapAround as i32;
        /// Clamp on input when editing via CTRL+Click or direct text input.
        const CLAMP_ON_INPUT = sys::ImGuiSliderFlags_ClampOnInput as i32;
        /// Clamp zero-range drags to avoid a zero-sized range.
        const CLAMP_ZERO_RANGE = sys::ImGuiSliderFlags_ClampZeroRange as i32;
        /// Disable small "smart" speed tweaks for very small/large ranges.
        const NO_SPEED_TWEAKS = sys::ImGuiSliderFlags_NoSpeedTweaks as i32;
        /// Clamp value to min/max bounds when input manually with CTRL+Click.
        const ALWAYS_CLAMP = sys::ImGuiSliderFlags_AlwaysClamp as i32;
        /// Make the widget logarithmic.
        const LOGARITHMIC = sys::ImGuiSliderFlags_Logarithmic as i32;
        /// Disable rounding underlying value to match the display format.
        const NO_ROUND_TO_FORMAT = sys::ImGuiSliderFlags_NoRoundToFormat as i32;
        /// Disable CTRL+Click or Enter key allowing direct text input.
        const NO_INPUT = sys::ImGuiSliderFlags_NoInput as i32;
        /// Draw R/G/B/A color markers on each component.
        ///
        /// Dear ImGui only defines four default component colors.
        const COLOR_MARKERS = sys::ImGuiSliderFlags_ColorMarkers as i32;
    }
}

impl From<SliderFlags> for DragFlags {
    fn from(flags: SliderFlags) -> Self {
        Self::from_bits_retain(flags.bits())
    }
}

impl Ui {
    /// Creates a new drag slider widget. Returns true if the value has been edited.
    pub fn drag<T: AsRef<str>, K: DataTypeKind>(&self, label: T, value: &mut K) -> bool {
        Drag::new(label).build(self, value)
    }

    /// Creates a new unbuilt Drag.
    pub fn drag_config<T: AsRef<str>, K: DataTypeKind>(&self, label: T) -> Drag<K, T> {
        Drag::new(label)
    }

    /// Creates a drag float2 slider (2 floats)
    #[doc(alias = "DragFloat2")]
    pub fn drag_float2(&self, label: impl AsRef<str>, values: &mut [f32; 2]) -> bool {
        unsafe {
            let label_cstr = self.scratch_txt(label);
            sys::igDragFloat2(
                label_cstr,
                values.as_mut_ptr(),
                1.0,
                0.0,
                0.0,
                ptr::null(),
                0,
            )
        }
    }

    /// Creates a drag float3 slider (3 floats)
    #[doc(alias = "DragFloat3")]
    pub fn drag_float3(&self, label: impl AsRef<str>, values: &mut [f32; 3]) -> bool {
        unsafe {
            let label_cstr = self.scratch_txt(label);
            sys::igDragFloat3(
                label_cstr,
                values.as_mut_ptr(),
                1.0,
                0.0,
                0.0,
                ptr::null(),
                0,
            )
        }
    }

    /// Creates a drag float4 slider (4 floats)
    #[doc(alias = "DragFloat4")]
    pub fn drag_float4(&self, label: impl AsRef<str>, values: &mut [f32; 4]) -> bool {
        unsafe {
            let label_cstr = self.scratch_txt(label);
            sys::igDragFloat4(
                label_cstr,
                values.as_mut_ptr(),
                1.0,
                0.0,
                0.0,
                ptr::null(),
                0,
            )
        }
    }

    /// Creates a drag int2 slider (2 ints)
    #[doc(alias = "DragInt2")]
    pub fn drag_int2(&self, label: impl AsRef<str>, values: &mut [i32; 2]) -> bool {
        unsafe {
            let label_cstr = self.scratch_txt(label);
            sys::igDragInt2(label_cstr, values.as_mut_ptr(), 1.0, 0, 0, ptr::null(), 0)
        }
    }

    /// Creates a drag int3 slider (3 ints)
    #[doc(alias = "DragInt3")]
    pub fn drag_int3(&self, label: impl AsRef<str>, values: &mut [i32; 3]) -> bool {
        unsafe {
            let label_cstr = self.scratch_txt(label);
            sys::igDragInt3(label_cstr, values.as_mut_ptr(), 1.0, 0, 0, ptr::null(), 0)
        }
    }

    /// Creates a drag int4 slider (4 ints)
    #[doc(alias = "DragInt4")]
    pub fn drag_int4(&self, label: impl AsRef<str>, values: &mut [i32; 4]) -> bool {
        unsafe {
            let label_cstr = self.scratch_txt(label);
            sys::igDragInt4(label_cstr, values.as_mut_ptr(), 1.0, 0, 0, ptr::null(), 0)
        }
    }
}

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
    pub fn flags(mut self, flags: impl Into<DragFlags>) -> Self {
        self.flags = flags.into();
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
    pub fn flags(mut self, flags: impl Into<DragFlags>) -> Self {
        self.flags = flags.into();
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
        unsafe {
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
        }
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
        unsafe {
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
        }
    }
}

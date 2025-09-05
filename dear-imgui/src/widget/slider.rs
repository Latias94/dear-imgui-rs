use crate::internal::DataTypeKind;
use crate::sys;
use crate::Ui;

/// Builder for slider widgets
pub struct Slider<'ui, T> {
    ui: &'ui Ui,
    label: String,
    min: T,
    max: T,
    format: Option<String>,
    flags: SliderFlags,
}

impl<'ui, T> Slider<'ui, T>
where
    T: DataTypeKind,
{
    /// Creates a new slider builder
    pub fn new(ui: &'ui Ui, label: impl AsRef<str>, min: T, max: T) -> Self {
        Self {
            ui,
            label: label.as_ref().to_string(),
            min,
            max,
            format: None,
            flags: SliderFlags::NONE,
        }
    }

    /// Sets the display format for the slider
    pub fn display_format(mut self, format: impl Into<String>) -> Self {
        self.format = Some(format.into());
        self
    }

    /// Sets the flags for the slider
    pub fn flags(mut self, flags: SliderFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the slider widget
    pub fn build(self, value: &mut T) -> bool {
        let label_ptr = self.ui.scratch_txt(&self.label);
        let format_ptr = self.ui.scratch_txt_opt(self.format.as_ref());

        unsafe {
            sys::ImGui_SliderScalar(
                label_ptr,
                T::KIND as i32,
                value as *mut T as *mut std::ffi::c_void,
                &self.min as *const T as *const std::ffi::c_void,
                &self.max as *const T as *const std::ffi::c_void,
                format_ptr,
                self.flags.bits(),
            )
        }
    }
}

bitflags::bitflags! {
    /// Flags for slider widgets
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct SliderFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Clamp value to min/max bounds when input manually with CTRL+Click. By default CTRL+Click allows going out of bounds.
        const ALWAYS_CLAMP = sys::ImGuiSliderFlags_AlwaysClamp;
        /// Make the widget logarithmic (linear otherwise). Consider using ImGuiSliderFlags_NoRoundToFormat with this if using a format-string with small amount of digits.
        const LOGARITHMIC = sys::ImGuiSliderFlags_Logarithmic;
        /// Disable rounding underlying value to match precision of the display format string (e.g. %.3f values are rounded to those 3 digits)
        const NO_ROUND_TO_FORMAT = sys::ImGuiSliderFlags_NoRoundToFormat;
        /// Disable CTRL+Click or Enter key allowing to input text directly into the widget
        const NO_INPUT = sys::ImGuiSliderFlags_NoInput;
    }
}

impl Ui {
    /// Creates a float slider
    #[doc(alias = "SliderFloat")]
    pub fn slider_f32(&self, label: impl AsRef<str>, value: &mut f32, min: f32, max: f32) -> bool {
        Slider::new(self, label, min, max).build(value)
    }

    /// Creates an integer slider
    #[doc(alias = "SliderInt")]
    pub fn slider_i32(&self, label: impl AsRef<str>, value: &mut i32, min: i32, max: i32) -> bool {
        Slider::new(self, label, min, max).build(value)
    }

    /// Creates a slider builder
    pub fn slider<T: DataTypeKind>(&self, label: impl AsRef<str>, min: T, max: T) -> Slider<'_, T> {
        Slider::new(self, label, min, max)
    }
}

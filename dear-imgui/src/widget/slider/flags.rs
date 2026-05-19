use crate::sys;

bitflags::bitflags! {
    /// Flags for slider widgets.
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct SliderFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Clamp on input when editing via CTRL+Click or direct text input.
        ///
        /// This is one of the two bits combined by `ALWAYS_CLAMP`.
        const CLAMP_ON_INPUT = sys::ImGuiSliderFlags_ClampOnInput as i32;
        /// Clamp zero-range sliders to avoid a zero-sized range.
        ///
        /// This is one of the two bits combined by `ALWAYS_CLAMP`.
        const CLAMP_ZERO_RANGE = sys::ImGuiSliderFlags_ClampZeroRange as i32;
        /// Disable small "smart" speed tweaks for very small/large ranges.
        ///
        /// Useful when precise, linear dragging behavior is desired.
        const NO_SPEED_TWEAKS = sys::ImGuiSliderFlags_NoSpeedTweaks as i32;
        /// Clamp value to min/max bounds when input manually with CTRL+Click. By default CTRL+Click allows going out of bounds.
        const ALWAYS_CLAMP = sys::ImGuiSliderFlags_AlwaysClamp as i32;
        /// Make the widget logarithmic (linear otherwise). Consider using ImGuiSliderFlags_NoRoundToFormat with this if using a format-string with small amount of digits.
        const LOGARITHMIC = sys::ImGuiSliderFlags_Logarithmic as i32;
        /// Disable rounding underlying value to match precision of the display format string (e.g. %.3f values are rounded to those 3 digits)
        const NO_ROUND_TO_FORMAT = sys::ImGuiSliderFlags_NoRoundToFormat as i32;
        /// Disable CTRL+Click or Enter key allowing to input text directly into the widget
        const NO_INPUT = sys::ImGuiSliderFlags_NoInput as i32;
        /// Draw R/G/B/A color markers on each component.
        ///
        /// Dear ImGui only defines four default component colors.
        const COLOR_MARKERS = sys::ImGuiSliderFlags_ColorMarkers as i32;
    }
}

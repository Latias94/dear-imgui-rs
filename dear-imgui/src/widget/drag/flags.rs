use crate::sys;
use crate::widget::slider::SliderFlags;

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

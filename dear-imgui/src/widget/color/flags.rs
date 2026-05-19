use super::validation::{
    assert_color_single_choice_mask, color_button_supported_mask, color_data_type_mask,
    color_display_mask, color_edit_supported_mask, color_input_mask, color_picker_mask,
    color_picker_supported_mask, validate_color_button_flags, validate_color_edit_flags,
    validate_color_picker_flags, validate_color_supported_bits,
};
use crate::sys;

bitflags::bitflags! {
    /// Independently composable flags accepted by `ColorEdit3()`,
    /// `ColorEdit4()`, and `SetColorEditOptions()`.
    #[repr(transparent)]
    #[derive(Copy, Clone, Debug, Default, Hash, Eq, PartialEq)]
    pub struct ColorEditFlags: u32 {
        /// No flags.
        const NONE = 0;
        /// Ignore Alpha component (will only read 3 components from the input pointer).
        const NO_ALPHA = sys::ImGuiColorEditFlags_NoAlpha as u32;
        /// Disable picker when clicking on color square.
        const NO_PICKER = sys::ImGuiColorEditFlags_NoPicker as u32;
        /// Disable toggling options menu when right-clicking on inputs/small preview.
        const NO_OPTIONS = sys::ImGuiColorEditFlags_NoOptions as u32;
        /// Disable color square preview next to the inputs.
        const NO_SMALL_PREVIEW = sys::ImGuiColorEditFlags_NoSmallPreview as u32;
        /// Disable inputs sliders/text widgets.
        const NO_INPUTS = sys::ImGuiColorEditFlags_NoInputs as u32;
        /// Disable tooltip when hovering the preview.
        const NO_TOOLTIP = sys::ImGuiColorEditFlags_NoTooltip as u32;
        /// Disable display of inline text label.
        const NO_LABEL = sys::ImGuiColorEditFlags_NoLabel as u32;
        /// Disable drag and drop target/source.
        const NO_DRAG_DROP = sys::ImGuiColorEditFlags_NoDragDrop as u32;
        /// Disable rendering R/G/B/A color markers.
        const NO_COLOR_MARKERS = sys::ImGuiColorEditFlags_NoColorMarkers as u32;
        /// Show vertical alpha bar/gradient in the picker.
        const ALPHA_BAR = sys::ImGuiColorEditFlags_AlphaBar as u32;
        /// Disable alpha in the preview.
        const ALPHA_OPAQUE = sys::ImGuiColorEditFlags_AlphaOpaque as u32;
        /// Disable the checkerboard background behind transparent colors.
        const ALPHA_NO_BG = sys::ImGuiColorEditFlags_AlphaNoBg as u32;
        /// Compatibility alias for [`Self::ALPHA_NO_BG`].
        const ALPHA_PREVIEW = Self::ALPHA_NO_BG.bits();
        /// Display half opaque / half checkerboard, instead of opaque.
        const ALPHA_PREVIEW_HALF = sys::ImGuiColorEditFlags_AlphaPreviewHalf as u32;
        /// Disable 0.0f..1.0f limits in RGBA edition.
        const HDR = sys::ImGuiColorEditFlags_HDR as u32;
    }
}

bitflags::bitflags! {
    /// Independently composable flags accepted by `ColorPicker3()` and
    /// `ColorPicker4()`.
    #[repr(transparent)]
    #[derive(Copy, Clone, Debug, Default, Hash, Eq, PartialEq)]
    pub struct ColorPickerFlags: u32 {
        /// No flags.
        const NONE = 0;
        /// Ignore Alpha component (will only read 3 components from the input pointer).
        const NO_ALPHA = sys::ImGuiColorEditFlags_NoAlpha as u32;
        /// Disable toggling options menu when right-clicking.
        const NO_OPTIONS = sys::ImGuiColorEditFlags_NoOptions as u32;
        /// Disable color square preview next to the inputs.
        const NO_SMALL_PREVIEW = sys::ImGuiColorEditFlags_NoSmallPreview as u32;
        /// Disable inputs sliders/text widgets.
        const NO_INPUTS = sys::ImGuiColorEditFlags_NoInputs as u32;
        /// Disable tooltip when hovering the preview.
        const NO_TOOLTIP = sys::ImGuiColorEditFlags_NoTooltip as u32;
        /// Disable display of inline text label.
        const NO_LABEL = sys::ImGuiColorEditFlags_NoLabel as u32;
        /// Disable bigger color preview on right side of the picker.
        const NO_SIDE_PREVIEW = sys::ImGuiColorEditFlags_NoSidePreview as u32;
        /// Show vertical alpha bar/gradient in the picker.
        const ALPHA_BAR = sys::ImGuiColorEditFlags_AlphaBar as u32;
        /// Disable alpha in the preview.
        const ALPHA_OPAQUE = sys::ImGuiColorEditFlags_AlphaOpaque as u32;
        /// Disable the checkerboard background behind transparent colors.
        const ALPHA_NO_BG = sys::ImGuiColorEditFlags_AlphaNoBg as u32;
        /// Compatibility alias for [`Self::ALPHA_NO_BG`].
        const ALPHA_PREVIEW = Self::ALPHA_NO_BG.bits();
        /// Display half opaque / half checkerboard, instead of opaque.
        const ALPHA_PREVIEW_HALF = sys::ImGuiColorEditFlags_AlphaPreviewHalf as u32;
        /// Disable 0.0f..1.0f limits in RGBA edition.
        const HDR = sys::ImGuiColorEditFlags_HDR as u32;
    }
}

bitflags::bitflags! {
    /// Independently composable flags accepted by `ColorButton()`.
    #[repr(transparent)]
    #[derive(Copy, Clone, Debug, Default, Hash, Eq, PartialEq)]
    pub struct ColorButtonFlags: u32 {
        /// No flags.
        const NONE = 0;
        /// Ignore Alpha component. For `ColorButton()` this does the same as [`Self::ALPHA_OPAQUE`].
        const NO_ALPHA = sys::ImGuiColorEditFlags_NoAlpha as u32;
        /// Disable tooltip when hovering the preview.
        const NO_TOOLTIP = sys::ImGuiColorEditFlags_NoTooltip as u32;
        /// Disable drag and drop source.
        const NO_DRAG_DROP = sys::ImGuiColorEditFlags_NoDragDrop as u32;
        /// Disable border (which is enforced by default).
        const NO_BORDER = sys::ImGuiColorEditFlags_NoBorder as u32;
        /// Disable alpha in the preview.
        const ALPHA_OPAQUE = sys::ImGuiColorEditFlags_AlphaOpaque as u32;
        /// Disable the checkerboard background behind transparent colors.
        const ALPHA_NO_BG = sys::ImGuiColorEditFlags_AlphaNoBg as u32;
        /// Compatibility alias for [`Self::ALPHA_NO_BG`].
        const ALPHA_PREVIEW = Self::ALPHA_NO_BG.bits();
        /// Display half opaque / half checkerboard, instead of opaque.
        const ALPHA_PREVIEW_HALF = sys::ImGuiColorEditFlags_AlphaPreviewHalf as u32;
    }
}

/// Single display mode for color edit widgets.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum ColorDisplayMode {
    Rgb,
    Hsv,
    Hex,
}

impl ColorDisplayMode {
    const fn raw(self) -> u32 {
        match self {
            Self::Rgb => sys::ImGuiColorEditFlags_DisplayRGB as u32,
            Self::Hsv => sys::ImGuiColorEditFlags_DisplayHSV as u32,
            Self::Hex => sys::ImGuiColorEditFlags_DisplayHex as u32,
        }
    }
}

/// Single numeric representation for color edit widgets and defaults.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum ColorDataType {
    Uint8,
    Float,
}

impl ColorDataType {
    const fn raw(self) -> u32 {
        match self {
            Self::Uint8 => sys::ImGuiColorEditFlags_Uint8 as u32,
            Self::Float => sys::ImGuiColorEditFlags_Float as u32,
        }
    }
}

/// Single picker implementation for color picker widgets and defaults.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum ColorPickerMode {
    HueBar,
    HueWheel,
}

impl ColorPickerMode {
    const fn raw(self) -> u32 {
        match self {
            Self::HueBar => sys::ImGuiColorEditFlags_PickerHueBar as u32,
            Self::HueWheel => sys::ImGuiColorEditFlags_PickerHueWheel as u32,
        }
    }
}

/// Single input/output color space for color edit and picker widgets.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum ColorInputMode {
    Rgb,
    Hsv,
}

impl ColorInputMode {
    const fn raw(self) -> u32 {
        match self {
            Self::Rgb => sys::ImGuiColorEditFlags_InputRGB as u32,
            Self::Hsv => sys::ImGuiColorEditFlags_InputHSV as u32,
        }
    }
}

bitflags::bitflags! {
    /// Display sub-editors visible inside `ColorPicker*()`.
    #[repr(transparent)]
    #[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
    pub struct ColorPickerDisplayFlags: u32 {
        const RGB = sys::ImGuiColorEditFlags_DisplayRGB as u32;
        const HSV = sys::ImGuiColorEditFlags_DisplayHSV as u32;
        const HEX = sys::ImGuiColorEditFlags_DisplayHex as u32;
    }
}

/// Options accepted by `ColorEdit3()`, `ColorEdit4()`, and
/// `SetColorEditOptions()`.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct ColorEditOptions {
    pub flags: ColorEditFlags,
    pub display_mode: Option<ColorDisplayMode>,
    pub data_type: Option<ColorDataType>,
    pub picker_mode: Option<ColorPickerMode>,
    pub input_mode: Option<ColorInputMode>,
}

impl ColorEditOptions {
    pub const fn new() -> Self {
        Self {
            flags: ColorEditFlags::NONE,
            display_mode: None,
            data_type: None,
            picker_mode: None,
            input_mode: None,
        }
    }

    pub fn flags(mut self, flags: ColorEditFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn display_mode(mut self, mode: ColorDisplayMode) -> Self {
        self.display_mode = Some(mode);
        self
    }

    pub fn data_type(mut self, data_type: ColorDataType) -> Self {
        self.data_type = Some(data_type);
        self
    }

    pub fn picker_mode(mut self, mode: ColorPickerMode) -> Self {
        self.picker_mode = Some(mode);
        self
    }

    pub fn input_mode(mut self, mode: ColorInputMode) -> Self {
        self.input_mode = Some(mode);
        self
    }

    pub fn bits(self) -> u32 {
        self.flags.bits()
            | self.display_mode.map_or(0, ColorDisplayMode::raw)
            | self.data_type.map_or(0, ColorDataType::raw)
            | self.picker_mode.map_or(0, ColorPickerMode::raw)
            | self.input_mode.map_or(0, ColorInputMode::raw)
    }

    pub(crate) fn validate(self, caller: &str) {
        validate_color_edit_flags(caller, self.flags);
        validate_color_supported_bits(caller, self.bits(), color_edit_supported_mask());
        assert_color_single_choice_mask(caller, self.bits(), color_display_mask(), "display mode");
        assert_color_single_choice_mask(caller, self.bits(), color_data_type_mask(), "data type");
        assert_color_single_choice_mask(caller, self.bits(), color_picker_mask(), "picker mode");
        assert_color_single_choice_mask(caller, self.bits(), color_input_mask(), "input mode");
    }
}

impl Default for ColorEditOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl From<ColorEditFlags> for ColorEditOptions {
    fn from(flags: ColorEditFlags) -> Self {
        Self::new().flags(flags)
    }
}

/// Options accepted by `ColorPicker3()` and `ColorPicker4()`.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct ColorPickerOptions {
    pub flags: ColorPickerFlags,
    pub display_flags: ColorPickerDisplayFlags,
    pub data_type: Option<ColorDataType>,
    pub picker_mode: Option<ColorPickerMode>,
    pub input_mode: Option<ColorInputMode>,
}

impl ColorPickerOptions {
    pub const fn new() -> Self {
        Self {
            flags: ColorPickerFlags::NONE,
            display_flags: ColorPickerDisplayFlags::empty(),
            data_type: None,
            picker_mode: None,
            input_mode: None,
        }
    }

    pub fn flags(mut self, flags: ColorPickerFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn display_flags(mut self, flags: ColorPickerDisplayFlags) -> Self {
        self.display_flags = flags;
        self
    }

    pub fn data_type(mut self, data_type: ColorDataType) -> Self {
        self.data_type = Some(data_type);
        self
    }

    pub fn picker_mode(mut self, mode: ColorPickerMode) -> Self {
        self.picker_mode = Some(mode);
        self
    }

    pub fn input_mode(mut self, mode: ColorInputMode) -> Self {
        self.input_mode = Some(mode);
        self
    }

    pub fn bits(self) -> u32 {
        self.flags.bits()
            | self.display_flags.bits()
            | self.data_type.map_or(0, ColorDataType::raw)
            | self.picker_mode.map_or(0, ColorPickerMode::raw)
            | self.input_mode.map_or(0, ColorInputMode::raw)
    }

    pub(crate) fn validate(self, caller: &str) {
        validate_color_picker_flags(caller, self.flags);
        let unsupported_display =
            self.display_flags.bits() & !ColorPickerDisplayFlags::all().bits();
        assert!(
            unsupported_display == 0,
            "{caller} received unsupported ColorPickerDisplayFlags bits: 0x{unsupported_display:X}"
        );
        validate_color_supported_bits(caller, self.bits(), color_picker_supported_mask());
        assert_color_single_choice_mask(caller, self.bits(), color_data_type_mask(), "data type");
        assert_color_single_choice_mask(caller, self.bits(), color_picker_mask(), "picker mode");
        assert_color_single_choice_mask(caller, self.bits(), color_input_mask(), "input mode");
    }
}

impl Default for ColorPickerOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl From<ColorPickerFlags> for ColorPickerOptions {
    fn from(flags: ColorPickerFlags) -> Self {
        Self::new().flags(flags)
    }
}

/// Options accepted by `ColorButton()`.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct ColorButtonOptions {
    pub flags: ColorButtonFlags,
    pub input_mode: Option<ColorInputMode>,
}

impl ColorButtonOptions {
    pub const fn new() -> Self {
        Self {
            flags: ColorButtonFlags::NONE,
            input_mode: None,
        }
    }

    pub fn flags(mut self, flags: ColorButtonFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn input_mode(mut self, mode: ColorInputMode) -> Self {
        self.input_mode = Some(mode);
        self
    }

    pub fn bits(self) -> u32 {
        self.flags.bits() | self.input_mode.map_or(0, ColorInputMode::raw)
    }

    pub(crate) fn validate(self, caller: &str) {
        validate_color_button_flags(caller, self.flags);
        validate_color_supported_bits(caller, self.bits(), color_button_supported_mask());
        assert_color_single_choice_mask(caller, self.bits(), color_input_mask(), "input mode");
    }
}

impl Default for ColorButtonOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl From<ColorButtonFlags> for ColorButtonOptions {
    fn from(flags: ColorButtonFlags) -> Self {
        Self::new().flags(flags)
    }
}

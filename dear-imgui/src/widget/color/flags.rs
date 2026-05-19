use super::validation::{
    assert_color_single_choice_mask, color_button_supported_mask, color_data_type_mask,
    color_display_mask, color_edit_supported_mask, color_input_mask, color_picker_mask,
    color_picker_supported_mask, validate_color_independent_flags, validate_color_supported_bits,
};
use crate::sys;

/// Flags for color edit widgets
#[repr(transparent)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct ColorEditFlags(u32);

impl ColorEditFlags {
    /// No flags
    pub const NONE: Self = Self(0);
    /// ColorEdit, ColorPicker, ColorButton: ignore Alpha component (will only read 3 components from the input pointer).
    pub const NO_ALPHA: Self = Self(sys::ImGuiColorEditFlags_NoAlpha as u32);
    /// ColorEdit: disable picker when clicking on color square.
    pub const NO_PICKER: Self = Self(sys::ImGuiColorEditFlags_NoPicker as u32);
    /// ColorEdit: disable toggling options menu when right-clicking on inputs/small preview.
    pub const NO_OPTIONS: Self = Self(sys::ImGuiColorEditFlags_NoOptions as u32);
    /// ColorEdit, ColorPicker: disable color square preview next to the inputs. (e.g. to show only the inputs)
    pub const NO_SMALL_PREVIEW: Self = Self(sys::ImGuiColorEditFlags_NoSmallPreview as u32);
    /// ColorEdit, ColorPicker: disable inputs sliders/text widgets (e.g. to show only the small preview color square).
    pub const NO_INPUTS: Self = Self(sys::ImGuiColorEditFlags_NoInputs as u32);
    /// ColorEdit, ColorPicker, ColorButton: disable tooltip when hovering the preview.
    pub const NO_TOOLTIP: Self = Self(sys::ImGuiColorEditFlags_NoTooltip as u32);
    /// ColorEdit, ColorPicker: disable display of inline text label (the label is still forwarded to the tooltip and picker).
    pub const NO_LABEL: Self = Self(sys::ImGuiColorEditFlags_NoLabel as u32);
    /// ColorPicker: disable bigger color preview on right side of the picker, use small color square preview instead.
    pub const NO_SIDE_PREVIEW: Self = Self(sys::ImGuiColorEditFlags_NoSidePreview as u32);
    /// ColorEdit: disable drag and drop target. ColorButton: disable drag and drop source.
    pub const NO_DRAG_DROP: Self = Self(sys::ImGuiColorEditFlags_NoDragDrop as u32);
    /// ColorButton: disable border (which is enforced by default)
    pub const NO_BORDER: Self = Self(sys::ImGuiColorEditFlags_NoBorder as u32);
    /// ColorEdit: disable rendering R/G/B/A color markers.
    pub const NO_COLOR_MARKERS: Self = Self(sys::ImGuiColorEditFlags_NoColorMarkers as u32);

    /// ColorEdit, ColorPicker: show vertical alpha bar/gradient in picker.
    pub const ALPHA_BAR: Self = Self(sys::ImGuiColorEditFlags_AlphaBar as u32);
    /// ColorEdit, ColorPicker, ColorButton: disable alpha in the preview.
    pub const ALPHA_OPAQUE: Self = Self(sys::ImGuiColorEditFlags_AlphaOpaque as u32);
    /// ColorEdit, ColorPicker, ColorButton: disable the checkerboard background behind transparent colors.
    pub const ALPHA_NO_BG: Self = Self(sys::ImGuiColorEditFlags_AlphaNoBg as u32);
    /// Compatibility alias for [`Self::ALPHA_NO_BG`].
    pub const ALPHA_PREVIEW: Self = Self::ALPHA_NO_BG;
    /// ColorEdit, ColorPicker, ColorButton: display half opaque / half checkerboard, instead of opaque.
    pub const ALPHA_PREVIEW_HALF: Self = Self(sys::ImGuiColorEditFlags_AlphaPreviewHalf as u32);
    /// (WIP) ColorEdit: Currently only disable 0.0f..1.0f limits in RGBA edition (note: you probably want to use ImGuiColorEditFlags_Float flag as well).
    pub const HDR: Self = Self(sys::ImGuiColorEditFlags_HDR as u32);

    /// Returns the underlying bits
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Returns true if all flags are set
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Returns all independently composable public color flags.
    pub const fn all() -> Self {
        Self(
            Self::NO_ALPHA.0
                | Self::NO_PICKER.0
                | Self::NO_OPTIONS.0
                | Self::NO_SMALL_PREVIEW.0
                | Self::NO_INPUTS.0
                | Self::NO_TOOLTIP.0
                | Self::NO_LABEL.0
                | Self::NO_SIDE_PREVIEW.0
                | Self::NO_DRAG_DROP.0
                | Self::NO_BORDER.0
                | Self::NO_COLOR_MARKERS.0
                | Self::ALPHA_BAR.0
                | Self::ALPHA_OPAQUE.0
                | Self::ALPHA_NO_BG.0
                | Self::ALPHA_PREVIEW_HALF.0
                | Self::HDR.0,
        )
    }
}

impl Default for ColorEditFlags {
    fn default() -> Self {
        Self::NONE
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
        validate_color_independent_flags(caller, self.flags);
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
    pub flags: ColorEditFlags,
    pub display_flags: ColorPickerDisplayFlags,
    pub data_type: Option<ColorDataType>,
    pub picker_mode: Option<ColorPickerMode>,
    pub input_mode: Option<ColorInputMode>,
}

impl ColorPickerOptions {
    pub const fn new() -> Self {
        Self {
            flags: ColorEditFlags::NONE,
            display_flags: ColorPickerDisplayFlags::empty(),
            data_type: None,
            picker_mode: None,
            input_mode: None,
        }
    }

    pub fn flags(mut self, flags: ColorEditFlags) -> Self {
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
        validate_color_independent_flags(caller, self.flags);
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

impl From<ColorEditFlags> for ColorPickerOptions {
    fn from(flags: ColorEditFlags) -> Self {
        Self::new().flags(flags)
    }
}

/// Options accepted by `ColorButton()`.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct ColorButtonOptions {
    pub flags: ColorEditFlags,
    pub input_mode: Option<ColorInputMode>,
}

impl ColorButtonOptions {
    pub const fn new() -> Self {
        Self {
            flags: ColorEditFlags::NONE,
            input_mode: None,
        }
    }

    pub fn flags(mut self, flags: ColorEditFlags) -> Self {
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
        validate_color_independent_flags(caller, self.flags);
        validate_color_supported_bits(caller, self.bits(), color_button_supported_mask());
        assert_color_single_choice_mask(caller, self.bits(), color_input_mask(), "input mode");
    }
}

impl Default for ColorButtonOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl From<ColorEditFlags> for ColorButtonOptions {
    fn from(flags: ColorEditFlags) -> Self {
        Self::new().flags(flags)
    }
}

impl std::ops::BitOr for ColorEditFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for ColorEditFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl std::ops::BitAnd for ColorEditFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl std::ops::BitAndAssign for ColorEditFlags {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl std::ops::BitXor for ColorEditFlags {
    type Output = Self;
    fn bitxor(self, rhs: Self) -> Self::Output {
        Self(self.0 ^ rhs.0)
    }
}

impl std::ops::BitXorAssign for ColorEditFlags {
    fn bitxor_assign(&mut self, rhs: Self) {
        self.0 ^= rhs.0;
    }
}

impl std::ops::Not for ColorEditFlags {
    type Output = Self;
    fn not(self) -> Self::Output {
        Self(!self.0)
    }
}

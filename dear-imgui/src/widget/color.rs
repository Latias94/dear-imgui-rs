//! Color widgets
//!
//! Color edit/picker/button widgets and their option flags. Useful for editing
//! RGBA values with different display/input modes.
//!
//! Upstream's normal alpha preview is the zero-bit default, not an alias for `ALPHA_NO_BG`:
//! ```compile_fail
//! let _ = dear_imgui_rs::ColorEditFlags::ALPHA_PREVIEW;
//! ```
//! ```compile_fail
//! let _ = dear_imgui_rs::ColorPickerFlags::ALPHA_PREVIEW;
//! ```
//! ```compile_fail
//! let _ = dear_imgui_rs::ColorButtonFlags::ALPHA_PREVIEW;
//! ```
//!

use crate::{Ui, sys};
use std::borrow::Cow;

/// # Color Edit Widgets
impl Ui {
    /// Creates a color edit widget for 3 components (RGB)
    #[doc(alias = "ColorEdit3")]
    pub fn color_edit3(&self, label: impl AsRef<str>, color: &mut [f32; 3]) -> bool {
        self.color_edit3_config(label.as_ref(), color).build()
    }

    /// Creates a color edit widget for 4 components (RGBA)
    #[doc(alias = "ColorEdit4")]
    pub fn color_edit4(&self, label: impl AsRef<str>, color: &mut [f32; 4]) -> bool {
        self.color_edit4_config(label.as_ref(), color).build()
    }

    /// Creates a color picker widget for 3 components (RGB)
    #[doc(alias = "ColorPicker3")]
    pub fn color_picker3(&self, label: impl AsRef<str>, color: &mut [f32; 3]) -> bool {
        self.color_picker3_config(label.as_ref(), color).build()
    }

    /// Creates a color picker widget for 4 components (RGBA)
    #[doc(alias = "ColorPicker4")]
    pub fn color_picker4(&self, label: impl AsRef<str>, color: &mut [f32; 4]) -> bool {
        self.color_picker4_config(label.as_ref(), color).build()
    }

    /// Creates a color button widget
    #[doc(alias = "ColorButton")]
    pub fn color_button(&self, desc_id: impl AsRef<str>, color: [f32; 4]) -> bool {
        self.color_button_config(desc_id.as_ref(), color).build()
    }

    /// Creates a color edit builder for 3 components
    pub fn color_edit3_config<'ui, 'p>(
        &'ui self,
        label: impl Into<Cow<'ui, str>>,
        color: &'p mut [f32; 3],
    ) -> ColorEdit3<'ui, 'p> {
        ColorEdit3::new(self, label, color)
    }

    /// Creates a color edit builder for 4 components
    pub fn color_edit4_config<'ui, 'p>(
        &'ui self,
        label: impl Into<Cow<'ui, str>>,
        color: &'p mut [f32; 4],
    ) -> ColorEdit4<'ui, 'p> {
        ColorEdit4::new(self, label, color)
    }

    /// Creates a color picker builder for 3 components
    pub fn color_picker3_config<'ui, 'p>(
        &'ui self,
        label: impl Into<Cow<'ui, str>>,
        color: &'p mut [f32; 3],
    ) -> ColorPicker3<'ui, 'p> {
        ColorPicker3::new(self, label, color)
    }

    /// Creates a color picker builder for 4 components
    pub fn color_picker4_config<'ui, 'p>(
        &'ui self,
        label: impl Into<Cow<'ui, str>>,
        color: &'p mut [f32; 4],
    ) -> ColorPicker4<'ui, 'p> {
        ColorPicker4::new(self, label, color)
    }

    /// Creates a color button builder
    pub fn color_button_config<'ui>(
        &'ui self,
        desc_id: impl Into<Cow<'ui, str>>,
        color: [f32; 4],
    ) -> ColorButton<'ui> {
        ColorButton::new(self, desc_id, color)
    }
}

bitflags::bitflags! {
    /// Independently composable flags accepted by `ColorEdit3()`,
    /// `ColorEdit4()`, and `Io::set_color_edit_options()`.
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
        /// Display half opaque / half checkerboard, instead of opaque.
        const ALPHA_PREVIEW_HALF = sys::ImGuiColorEditFlags_AlphaPreviewHalf as u32;
        /// Disable 0.0f..1.0f limits in RGBA edition.
        const HDR = sys::ImGuiColorEditFlags_HDR as u32;
        /// Disable rotation of the saturation/value triangle in the color picker.
        const PICKER_NO_ROTATE = sys::ImGuiColorEditFlags_PickerNoRotate as u32;
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
        /// Display half opaque / half checkerboard, instead of opaque.
        const ALPHA_PREVIEW_HALF = sys::ImGuiColorEditFlags_AlphaPreviewHalf as u32;
        /// Disable 0.0f..1.0f limits in RGBA edition.
        const HDR = sys::ImGuiColorEditFlags_HDR as u32;
        /// Disable rotation of the saturation/value triangle.
        const PICKER_NO_ROTATE = sys::ImGuiColorEditFlags_PickerNoRotate as u32;
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
/// `Io::set_color_edit_options()`.
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

    pub(crate) fn from_bits(bits: u32, caller: &str) -> Self {
        let options = Self {
            flags: ColorEditFlags::from_bits_retain(bits & ColorEditFlags::all().bits()),
            display_mode: match bits & color_display_mask() {
                0 => None,
                value if value == ColorDisplayMode::Rgb.raw() => Some(ColorDisplayMode::Rgb),
                value if value == ColorDisplayMode::Hsv.raw() => Some(ColorDisplayMode::Hsv),
                value if value == ColorDisplayMode::Hex.raw() => Some(ColorDisplayMode::Hex),
                _ => panic!("{caller} contains an invalid color display mode"),
            },
            data_type: match bits & color_data_type_mask() {
                0 => None,
                value if value == ColorDataType::Uint8.raw() => Some(ColorDataType::Uint8),
                value if value == ColorDataType::Float.raw() => Some(ColorDataType::Float),
                _ => panic!("{caller} contains an invalid color data type"),
            },
            picker_mode: match bits & color_picker_mask() {
                0 => None,
                value if value == ColorPickerMode::HueBar.raw() => Some(ColorPickerMode::HueBar),
                value if value == ColorPickerMode::HueWheel.raw() => {
                    Some(ColorPickerMode::HueWheel)
                }
                _ => panic!("{caller} contains an invalid color picker mode"),
            },
            input_mode: match bits & color_input_mask() {
                0 => None,
                value if value == ColorInputMode::Rgb.raw() => Some(ColorInputMode::Rgb),
                value if value == ColorInputMode::Hsv.raw() => Some(ColorInputMode::Hsv),
                _ => panic!("{caller} contains an invalid color input mode"),
            },
        };
        options.validate(caller);
        options
    }

    pub(crate) fn with_default_choices(self) -> Self {
        let mut bits = self.bits();
        let defaults = sys::ImGuiColorEditFlags_DefaultOptions_ as u32;
        for mask in [
            color_display_mask(),
            color_data_type_mask(),
            color_picker_mask(),
            color_input_mask(),
        ] {
            if bits & mask == 0 {
                bits |= defaults & mask;
            }
        }
        Self::from_bits(bits, "ColorEditOptions::with_default_choices()")
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

/// Builder for a 3-component color edit widget
#[derive(Debug)]
#[must_use]
pub struct ColorEdit3<'ui, 'p> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    color: &'p mut [f32; 3],
    flags: ColorEditOptions,
}

impl<'ui, 'p> ColorEdit3<'ui, 'p> {
    /// Creates a new color edit builder
    pub fn new(ui: &'ui Ui, label: impl Into<Cow<'ui, str>>, color: &'p mut [f32; 3]) -> Self {
        Self {
            ui,
            label: label.into(),
            color,
            flags: ColorEditOptions::new(),
        }
    }

    /// Sets the flags for the color edit
    pub fn flags(mut self, flags: impl Into<ColorEditOptions>) -> Self {
        self.flags = flags.into();
        self
    }

    /// Sets the display mode.
    pub fn display_mode(mut self, mode: ColorDisplayMode) -> Self {
        self.flags.display_mode = Some(mode);
        self
    }

    /// Sets the numeric data type.
    pub fn data_type(mut self, data_type: ColorDataType) -> Self {
        self.flags.data_type = Some(data_type);
        self
    }

    /// Sets the picker mode used by the popup picker.
    pub fn picker_mode(mut self, mode: ColorPickerMode) -> Self {
        self.flags.picker_mode = Some(mode);
        self
    }

    /// Sets the input/output color space.
    pub fn input_mode(mut self, mode: ColorInputMode) -> Self {
        self.flags.input_mode = Some(mode);
        self
    }

    /// Builds the color edit widget
    pub fn build(self) -> bool {
        self.flags.validate("ColorEdit3::build()");
        assert_finite_color3("ColorEdit3::build()", "color", &*self.color);
        self.ui
            .with_scratch_txt(self.label.as_ref(), |label_ptr| unsafe {
                sys::igColorEdit3(label_ptr, self.color.as_mut_ptr(), self.flags.bits() as i32)
            })
    }
}

/// Builder for a 4-component color edit widget
#[derive(Debug)]
#[must_use]
pub struct ColorEdit4<'ui, 'p> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    color: &'p mut [f32; 4],
    flags: ColorEditOptions,
}

impl<'ui, 'p> ColorEdit4<'ui, 'p> {
    /// Creates a new color edit builder
    pub fn new(ui: &'ui Ui, label: impl Into<Cow<'ui, str>>, color: &'p mut [f32; 4]) -> Self {
        Self {
            ui,
            label: label.into(),
            color,
            flags: ColorEditOptions::new(),
        }
    }

    /// Sets the flags for the color edit
    pub fn flags(mut self, flags: impl Into<ColorEditOptions>) -> Self {
        self.flags = flags.into();
        self
    }

    /// Sets the display mode.
    pub fn display_mode(mut self, mode: ColorDisplayMode) -> Self {
        self.flags.display_mode = Some(mode);
        self
    }

    /// Sets the numeric data type.
    pub fn data_type(mut self, data_type: ColorDataType) -> Self {
        self.flags.data_type = Some(data_type);
        self
    }

    /// Sets the picker mode used by the popup picker.
    pub fn picker_mode(mut self, mode: ColorPickerMode) -> Self {
        self.flags.picker_mode = Some(mode);
        self
    }

    /// Sets the input/output color space.
    pub fn input_mode(mut self, mode: ColorInputMode) -> Self {
        self.flags.input_mode = Some(mode);
        self
    }

    /// Builds the color edit widget
    pub fn build(self) -> bool {
        self.flags.validate("ColorEdit4::build()");
        assert_finite_color4("ColorEdit4::build()", "color", &*self.color);
        self.ui
            .with_scratch_txt(self.label.as_ref(), |label_ptr| unsafe {
                sys::igColorEdit4(label_ptr, self.color.as_mut_ptr(), self.flags.bits() as i32)
            })
    }
}

/// Builder for a 3-component color picker widget
#[derive(Debug)]
#[must_use]
pub struct ColorPicker3<'ui, 'p> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    color: &'p mut [f32; 3],
    flags: ColorPickerOptions,
}

impl<'ui, 'p> ColorPicker3<'ui, 'p> {
    /// Creates a new color picker builder
    pub fn new(ui: &'ui Ui, label: impl Into<Cow<'ui, str>>, color: &'p mut [f32; 3]) -> Self {
        Self {
            ui,
            label: label.into(),
            color,
            flags: ColorPickerOptions::new(),
        }
    }

    /// Sets the flags for the color picker
    pub fn flags(mut self, flags: impl Into<ColorPickerOptions>) -> Self {
        self.flags = flags.into();
        self
    }

    /// Sets the display sub-editors shown inside the picker.
    pub fn display_flags(mut self, flags: ColorPickerDisplayFlags) -> Self {
        self.flags.display_flags = flags;
        self
    }

    /// Sets the numeric data type.
    pub fn data_type(mut self, data_type: ColorDataType) -> Self {
        self.flags.data_type = Some(data_type);
        self
    }

    /// Sets the picker mode.
    pub fn picker_mode(mut self, mode: ColorPickerMode) -> Self {
        self.flags.picker_mode = Some(mode);
        self
    }

    /// Sets the input/output color space.
    pub fn input_mode(mut self, mode: ColorInputMode) -> Self {
        self.flags.input_mode = Some(mode);
        self
    }

    /// Builds the color picker widget
    pub fn build(self) -> bool {
        self.flags.validate("ColorPicker3::build()");
        assert_finite_color3("ColorPicker3::build()", "color", &*self.color);
        self.ui
            .with_scratch_txt(self.label.as_ref(), |label_ptr| unsafe {
                sys::igColorPicker3(label_ptr, self.color.as_mut_ptr(), self.flags.bits() as i32)
            })
    }
}

/// Builder for a 4-component color picker widget
#[derive(Debug)]
#[must_use]
pub struct ColorPicker4<'ui, 'p> {
    ui: &'ui Ui,
    label: Cow<'ui, str>,
    color: &'p mut [f32; 4],
    flags: ColorPickerOptions,
    ref_color: Option<[f32; 4]>,
}

impl<'ui, 'p> ColorPicker4<'ui, 'p> {
    /// Creates a new color picker builder
    pub fn new(ui: &'ui Ui, label: impl Into<Cow<'ui, str>>, color: &'p mut [f32; 4]) -> Self {
        Self {
            ui,
            label: label.into(),
            color,
            flags: ColorPickerOptions::new(),
            ref_color: None,
        }
    }

    /// Sets the flags for the color picker
    pub fn flags(mut self, flags: impl Into<ColorPickerOptions>) -> Self {
        self.flags = flags.into();
        self
    }

    /// Sets the display sub-editors shown inside the picker.
    pub fn display_flags(mut self, flags: ColorPickerDisplayFlags) -> Self {
        self.flags.display_flags = flags;
        self
    }

    /// Sets the numeric data type.
    pub fn data_type(mut self, data_type: ColorDataType) -> Self {
        self.flags.data_type = Some(data_type);
        self
    }

    /// Sets the picker mode.
    pub fn picker_mode(mut self, mode: ColorPickerMode) -> Self {
        self.flags.picker_mode = Some(mode);
        self
    }

    /// Sets the input/output color space.
    pub fn input_mode(mut self, mode: ColorInputMode) -> Self {
        self.flags.input_mode = Some(mode);
        self
    }

    /// Sets the reference color for comparison
    pub fn reference_color(mut self, ref_color: [f32; 4]) -> Self {
        self.ref_color = Some(ref_color);
        self
    }

    /// Builds the color picker widget
    pub fn build(self) -> bool {
        self.flags.validate("ColorPicker4::build()");
        assert_finite_color4("ColorPicker4::build()", "color", &*self.color);
        if let Some(ref_color) = &self.ref_color {
            assert_finite_color4("ColorPicker4::build()", "reference color", ref_color);
        }
        let ref_color_ptr = self
            .ref_color
            .as_ref()
            .map_or(std::ptr::null(), |c| c.as_ptr());

        self.ui
            .with_scratch_txt(self.label.as_ref(), |label_ptr| unsafe {
                sys::igColorPicker4(
                    label_ptr,
                    self.color.as_mut_ptr(),
                    self.flags.bits() as i32,
                    ref_color_ptr,
                )
            })
    }
}

/// Builder for a color button widget
#[derive(Debug)]
#[must_use]
pub struct ColorButton<'ui> {
    ui: &'ui Ui,
    desc_id: Cow<'ui, str>,
    color: [f32; 4],
    flags: ColorButtonOptions,
    size: [f32; 2],
}

impl<'ui> ColorButton<'ui> {
    /// Creates a new color button builder
    pub fn new(ui: &'ui Ui, desc_id: impl Into<Cow<'ui, str>>, color: [f32; 4]) -> Self {
        Self {
            ui,
            desc_id: desc_id.into(),
            color,
            flags: ColorButtonOptions::new(),
            size: [0.0, 0.0],
        }
    }

    /// Sets the flags for the color button
    pub fn flags(mut self, flags: impl Into<ColorButtonOptions>) -> Self {
        self.flags = flags.into();
        self
    }

    /// Sets the input color space.
    pub fn input_mode(mut self, mode: ColorInputMode) -> Self {
        self.flags.input_mode = Some(mode);
        self
    }

    /// Sets the size of the color button
    pub fn size(mut self, size: [f32; 2]) -> Self {
        self.size = size;
        self
    }

    /// Builds the color button widget
    pub fn build(self) -> bool {
        self.flags.validate("ColorButton::build()");
        assert_finite_color4("ColorButton::build()", "color", &self.color);
        assert_non_negative_finite_vec2("ColorButton::build()", "size", self.size);
        let size_vec: sys::ImVec2 = self.size.into();

        self.ui
            .with_scratch_txt(self.desc_id.as_ref(), |desc_id_ptr| unsafe {
                sys::igColorButton(
                    desc_id_ptr,
                    sys::ImVec4 {
                        x: self.color[0],
                        y: self.color[1],
                        z: self.color[2],
                        w: self.color[3],
                    },
                    self.flags.bits() as i32,
                    size_vec,
                )
            })
    }
}

#[inline]
const fn color_display_mask() -> u32 {
    sys::ImGuiColorEditFlags_DisplayMask_ as u32
}

#[inline]
const fn color_data_type_mask() -> u32 {
    sys::ImGuiColorEditFlags_DataTypeMask_ as u32
}

#[inline]
const fn color_picker_mask() -> u32 {
    sys::ImGuiColorEditFlags_PickerMask_ as u32
}

#[inline]
const fn color_input_mask() -> u32 {
    sys::ImGuiColorEditFlags_InputMask_ as u32
}

#[inline]
const fn color_choice_mask() -> u32 {
    color_display_mask() | color_data_type_mask() | color_picker_mask() | color_input_mask()
}

#[inline]
const fn color_edit_supported_mask() -> u32 {
    ColorEditFlags::all().bits() | color_choice_mask()
}

#[inline]
const fn color_picker_supported_mask() -> u32 {
    ColorPickerFlags::all().bits()
        | color_display_mask()
        | color_data_type_mask()
        | color_picker_mask()
        | color_input_mask()
}

#[inline]
const fn color_button_supported_mask() -> u32 {
    ColorButtonFlags::all().bits() | color_input_mask()
}

fn validate_color_edit_flags(caller: &str, flags: ColorEditFlags) {
    let unsupported = flags.bits() & !ColorEditFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ColorEditFlags bits: 0x{unsupported:X}"
    );
}

fn validate_color_picker_flags(caller: &str, flags: ColorPickerFlags) {
    let unsupported = flags.bits() & !ColorPickerFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ColorPickerFlags bits: 0x{unsupported:X}"
    );
}

fn validate_color_button_flags(caller: &str, flags: ColorButtonFlags) {
    let unsupported = flags.bits() & !ColorButtonFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ColorButtonFlags bits: 0x{unsupported:X}"
    );
}

fn validate_color_supported_bits(caller: &str, bits: u32, supported: u32) {
    let unsupported = bits & !supported;
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiColorEditFlags bits: 0x{unsupported:X}"
    );
}

fn assert_color_single_choice_mask(caller: &str, bits: u32, mask: u32, name: &str) {
    assert!(
        (bits & mask).count_ones() <= 1,
        "{caller} accepts at most one color {name}"
    );
}

fn assert_finite_color3(caller: &str, name: &str, color: &[f32; 3]) {
    assert!(
        color.iter().all(|component| component.is_finite()),
        "{caller} {name} must contain finite values"
    );
}

fn assert_finite_color4(caller: &str, name: &str, color: &[f32; 4]) {
    assert!(
        color.iter().all(|component| component.is_finite()),
        "{caller} {name} must contain finite values"
    );
}

fn assert_non_negative_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert!(
        value[0].is_finite() && value[1].is_finite(),
        "{caller} {name} must contain finite values"
    );
    assert!(
        value[0] >= 0.0 && value[1] >= 0.0,
        "{caller} {name} must contain non-negative values"
    );
}

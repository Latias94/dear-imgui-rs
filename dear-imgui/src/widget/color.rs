//! Color widgets
//!
//! Color edit/picker/button widgets and their option flags. Useful for editing
//! RGBA values with different display/input modes.
//!
use crate::sys;
use crate::ui::Ui;
use std::borrow::Cow;

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
    ColorEditFlags::all().bits()
        | color_display_mask()
        | color_data_type_mask()
        | color_picker_mask()
        | color_input_mask()
}

#[inline]
const fn color_button_supported_mask() -> u32 {
    ColorEditFlags::all().bits() | color_input_mask()
}

fn validate_color_independent_flags(caller: &str, flags: ColorEditFlags) {
    let unsupported = flags.bits() & !ColorEditFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received non-independent ImGuiColorEditFlags bits: 0x{unsupported:X}"
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

/// # Color Edit Widgets
impl Ui {
    /// Initializes default color editing/picking options.
    ///
    /// This configures the defaults used by the various `Color*` widgets (unless
    /// overridden per-call via flags). Users can still change many options via
    /// the right-click context menu unless `_NO_OPTIONS` is passed.
    #[doc(alias = "SetColorEditOptions")]
    pub fn set_color_edit_options(&self, options: impl Into<ColorEditOptions>) {
        let options = options.into();
        options.validate("Ui::set_color_edit_options()");
        unsafe { sys::igSetColorEditOptions(options.bits() as i32) }
    }

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
        let label_ptr = self.ui.scratch_txt(self.label.as_ref());
        unsafe { sys::igColorEdit3(label_ptr, self.color.as_mut_ptr(), self.flags.bits() as i32) }
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
        let label_ptr = self.ui.scratch_txt(self.label.as_ref());
        unsafe { sys::igColorEdit4(label_ptr, self.color.as_mut_ptr(), self.flags.bits() as i32) }
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
        let label_ptr = self.ui.scratch_txt(self.label.as_ref());
        unsafe { sys::igColorPicker3(label_ptr, self.color.as_mut_ptr(), self.flags.bits() as i32) }
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
        let label_ptr = self.ui.scratch_txt(self.label.as_ref());
        let ref_color_ptr = self
            .ref_color
            .as_ref()
            .map_or(std::ptr::null(), |c| c.as_ptr());

        unsafe {
            sys::igColorPicker4(
                label_ptr,
                self.color.as_mut_ptr(),
                self.flags.bits() as i32,
                ref_color_ptr,
            )
        }
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
        let desc_id_ptr = self.ui.scratch_txt(self.desc_id.as_ref());
        let size_vec: sys::ImVec2 = self.size.into();

        unsafe {
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
        }
    }
}

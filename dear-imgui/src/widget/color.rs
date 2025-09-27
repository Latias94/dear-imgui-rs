//! Color widgets
//!
//! Color edit/picker/button widgets and their option flags. Useful for editing
//! RGBA values with different display/input modes.
//!
use crate::sys;
use crate::ui::Ui;

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

    /// ColorEdit, ColorPicker: show vertical alpha bar/gradient in picker.
    pub const ALPHA_BAR: Self = Self(sys::ImGuiColorEditFlags_AlphaBar as u32);
    /// ColorEdit, ColorPicker, ColorButton: display preview as a transparent color over a checkerboard, instead of opaque.
    pub const ALPHA_PREVIEW: Self = Self(sys::ImGuiColorEditFlags_AlphaNoBg as u32);
    /// ColorEdit, ColorPicker, ColorButton: display half opaque / half checkerboard, instead of opaque.
    pub const ALPHA_PREVIEW_HALF: Self = Self(sys::ImGuiColorEditFlags_AlphaPreviewHalf as u32);
    /// (WIP) ColorEdit: Currently only disable 0.0f..1.0f limits in RGBA edition (note: you probably want to use ImGuiColorEditFlags_Float flag as well).
    pub const HDR: Self = Self(sys::ImGuiColorEditFlags_HDR as u32);
    /// ColorEdit: override _display_ type among RGB/HSV/Hex. ColorPicker: select any combination using one or more of RGB/HSV/Hex.
    pub const DISPLAY_RGB: Self = Self(sys::ImGuiColorEditFlags_DisplayRGB as u32);
    /// ColorEdit: override _display_ type among RGB/HSV/Hex. ColorPicker: select any combination using one or more of RGB/HSV/Hex.
    pub const DISPLAY_HSV: Self = Self(sys::ImGuiColorEditFlags_DisplayHSV as u32);
    /// ColorEdit: override _display_ type among RGB/HSV/Hex. ColorPicker: select any combination using one or more of RGB/HSV/Hex.
    pub const DISPLAY_HEX: Self = Self(sys::ImGuiColorEditFlags_DisplayHex as u32);
    /// ColorEdit, ColorPicker, ColorButton: _display_ values formatted as 0..255.
    pub const UINT8: Self = Self(sys::ImGuiColorEditFlags_Uint8 as u32);
    /// ColorEdit, ColorPicker, ColorButton: _display_ values formatted as 0.0f..1.0f floats instead of 0..255 integers. No round-trip of value via integers.
    pub const FLOAT: Self = Self(sys::ImGuiColorEditFlags_Float as u32);
    /// ColorPicker: bar for Hue, rectangle for Sat/Value.
    pub const PICKER_HUE_BAR: Self = Self(sys::ImGuiColorEditFlags_PickerHueBar as u32);
    /// ColorPicker: wheel for Hue, triangle for Sat/Value.
    pub const PICKER_HUE_WHEEL: Self = Self(sys::ImGuiColorEditFlags_PickerHueWheel as u32);
    /// ColorEdit, ColorPicker: input and output data in RGB format.
    pub const INPUT_RGB: Self = Self(sys::ImGuiColorEditFlags_InputRGB as u32);
    /// ColorEdit, ColorPicker: input and output data in HSV format.
    pub const INPUT_HSV: Self = Self(sys::ImGuiColorEditFlags_InputHSV as u32);

    /// Returns the underlying bits
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Returns true if all flags are set
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
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

/// # Color Edit Widgets
impl Ui {
    /// Creates a color edit widget for 3 components (RGB)
    #[doc(alias = "ColorEdit3")]
    pub fn color_edit3(&self, label: impl AsRef<str>, color: &mut [f32; 3]) -> bool {
        self.color_edit3_config(label, color).build()
    }

    /// Creates a color edit widget for 4 components (RGBA)
    #[doc(alias = "ColorEdit4")]
    pub fn color_edit4(&self, label: impl AsRef<str>, color: &mut [f32; 4]) -> bool {
        self.color_edit4_config(label, color).build()
    }

    /// Creates a color picker widget for 3 components (RGB)
    #[doc(alias = "ColorPicker3")]
    pub fn color_picker3(&self, label: impl AsRef<str>, color: &mut [f32; 3]) -> bool {
        self.color_picker3_config(label, color).build()
    }

    /// Creates a color picker widget for 4 components (RGBA)
    #[doc(alias = "ColorPicker4")]
    pub fn color_picker4(&self, label: impl AsRef<str>, color: &mut [f32; 4]) -> bool {
        self.color_picker4_config(label, color).build()
    }

    /// Creates a color button widget
    #[doc(alias = "ColorButton")]
    pub fn color_button(&self, desc_id: impl AsRef<str>, color: [f32; 4]) -> bool {
        self.color_button_config(desc_id, color).build()
    }

    /// Creates a color edit builder for 3 components
    pub fn color_edit3_config<'p>(
        &self,
        label: impl AsRef<str>,
        color: &'p mut [f32; 3],
    ) -> ColorEdit3<'_, 'p> {
        ColorEdit3::new(self, label, color)
    }

    /// Creates a color edit builder for 4 components
    pub fn color_edit4_config<'p>(
        &self,
        label: impl AsRef<str>,
        color: &'p mut [f32; 4],
    ) -> ColorEdit4<'_, 'p> {
        ColorEdit4::new(self, label, color)
    }

    /// Creates a color picker builder for 3 components
    pub fn color_picker3_config<'p>(
        &self,
        label: impl AsRef<str>,
        color: &'p mut [f32; 3],
    ) -> ColorPicker3<'_, 'p> {
        ColorPicker3::new(self, label, color)
    }

    /// Creates a color picker builder for 4 components
    pub fn color_picker4_config<'p>(
        &self,
        label: impl AsRef<str>,
        color: &'p mut [f32; 4],
    ) -> ColorPicker4<'_, 'p> {
        ColorPicker4::new(self, label, color)
    }

    /// Creates a color button builder
    pub fn color_button_config(
        &self,
        desc_id: impl AsRef<str>,
        color: [f32; 4],
    ) -> ColorButton<'_> {
        ColorButton::new(self, desc_id, color)
    }
}

/// Builder for a 3-component color edit widget
#[derive(Debug)]
#[must_use]
pub struct ColorEdit3<'ui, 'p> {
    ui: &'ui Ui,
    label: String,
    color: &'p mut [f32; 3],
    flags: ColorEditFlags,
}

impl<'ui, 'p> ColorEdit3<'ui, 'p> {
    /// Creates a new color edit builder
    pub fn new(ui: &'ui Ui, label: impl AsRef<str>, color: &'p mut [f32; 3]) -> Self {
        Self {
            ui,
            label: label.as_ref().to_string(),
            color,
            flags: ColorEditFlags::NONE,
        }
    }

    /// Sets the flags for the color edit
    pub fn flags(mut self, flags: ColorEditFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the color edit widget
    pub fn build(self) -> bool {
        let label_ptr = self.ui.scratch_txt(&self.label);
        unsafe { sys::igColorEdit3(label_ptr, self.color.as_mut_ptr(), self.flags.bits() as i32) }
    }
}

/// Builder for a 4-component color edit widget
#[derive(Debug)]
#[must_use]
pub struct ColorEdit4<'ui, 'p> {
    ui: &'ui Ui,
    label: String,
    color: &'p mut [f32; 4],
    flags: ColorEditFlags,
}

impl<'ui, 'p> ColorEdit4<'ui, 'p> {
    /// Creates a new color edit builder
    pub fn new(ui: &'ui Ui, label: impl AsRef<str>, color: &'p mut [f32; 4]) -> Self {
        Self {
            ui,
            label: label.as_ref().to_string(),
            color,
            flags: ColorEditFlags::NONE,
        }
    }

    /// Sets the flags for the color edit
    pub fn flags(mut self, flags: ColorEditFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the color edit widget
    pub fn build(self) -> bool {
        let label_ptr = self.ui.scratch_txt(&self.label);
        unsafe { sys::igColorEdit4(label_ptr, self.color.as_mut_ptr(), self.flags.bits() as i32) }
    }
}

/// Builder for a 3-component color picker widget
#[derive(Debug)]
#[must_use]
pub struct ColorPicker3<'ui, 'p> {
    ui: &'ui Ui,
    label: String,
    color: &'p mut [f32; 3],
    flags: ColorEditFlags,
}

impl<'ui, 'p> ColorPicker3<'ui, 'p> {
    /// Creates a new color picker builder
    pub fn new(ui: &'ui Ui, label: impl AsRef<str>, color: &'p mut [f32; 3]) -> Self {
        Self {
            ui,
            label: label.as_ref().to_string(),
            color,
            flags: ColorEditFlags::NONE,
        }
    }

    /// Sets the flags for the color picker
    pub fn flags(mut self, flags: ColorEditFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Builds the color picker widget
    pub fn build(self) -> bool {
        let label_ptr = self.ui.scratch_txt(&self.label);
        unsafe { sys::igColorPicker3(label_ptr, self.color.as_mut_ptr(), self.flags.bits() as i32) }
    }
}

/// Builder for a 4-component color picker widget
#[derive(Debug)]
#[must_use]
pub struct ColorPicker4<'ui, 'p> {
    ui: &'ui Ui,
    label: String,
    color: &'p mut [f32; 4],
    flags: ColorEditFlags,
    ref_color: Option<[f32; 4]>,
}

impl<'ui, 'p> ColorPicker4<'ui, 'p> {
    /// Creates a new color picker builder
    pub fn new(ui: &'ui Ui, label: impl AsRef<str>, color: &'p mut [f32; 4]) -> Self {
        Self {
            ui,
            label: label.as_ref().to_string(),
            color,
            flags: ColorEditFlags::NONE,
            ref_color: None,
        }
    }

    /// Sets the flags for the color picker
    pub fn flags(mut self, flags: ColorEditFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Sets the reference color for comparison
    pub fn reference_color(mut self, ref_color: [f32; 4]) -> Self {
        self.ref_color = Some(ref_color);
        self
    }

    /// Builds the color picker widget
    pub fn build(self) -> bool {
        let label_ptr = self.ui.scratch_txt(&self.label);
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
    desc_id: String,
    color: [f32; 4],
    flags: ColorEditFlags,
    size: [f32; 2],
}

impl<'ui> ColorButton<'ui> {
    /// Creates a new color button builder
    pub fn new(ui: &'ui Ui, desc_id: impl AsRef<str>, color: [f32; 4]) -> Self {
        Self {
            ui,
            desc_id: desc_id.as_ref().to_string(),
            color,
            flags: ColorEditFlags::NONE,
            size: [0.0, 0.0],
        }
    }

    /// Sets the flags for the color button
    pub fn flags(mut self, flags: ColorEditFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Sets the size of the color button
    pub fn size(mut self, size: [f32; 2]) -> Self {
        self.size = size;
        self
    }

    /// Builds the color button widget
    pub fn build(self) -> bool {
        let desc_id_ptr = self.ui.scratch_txt(&self.desc_id);
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

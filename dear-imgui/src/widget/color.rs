use crate::types::Color;
use crate::ui::Ui;
use dear_imgui_sys as sys;

/// Color widgets
///
/// This module contains all color-related UI components like color pickers, editors, etc.

bitflags::bitflags! {
    /// Flags for color edit widgets
    #[repr(transparent)]
    #[derive(Debug)]
    pub struct ColorEditFlags: i32 {
        /// No flags
        const NONE = 0;
        /// ColorEdit, ColorPicker, ColorButton: ignore Alpha component (will only read 3 components from the input pointer).
        const NO_ALPHA = sys::ImGuiColorEditFlags_NoAlpha;
        /// ColorEdit: disable picker when clicking on colored square.
        const NO_PICKER = sys::ImGuiColorEditFlags_NoPicker;
        /// ColorEdit: disable toggling options menu when right-clicking on inputs/small preview.
        const NO_OPTIONS = sys::ImGuiColorEditFlags_NoOptions;
        /// ColorEdit, ColorPicker: disable colored square preview next to the inputs. (e.g. to show only the inputs)
        const NO_SMALL_PREVIEW = sys::ImGuiColorEditFlags_NoSmallPreview;
        /// ColorEdit, ColorPicker: disable inputs sliders/text widgets (e.g. to show only the small preview colored square).
        const NO_INPUTS = sys::ImGuiColorEditFlags_NoInputs;
        /// ColorEdit, ColorPicker, ColorButton: disable tooltip when hovering the preview.
        const NO_TOOLTIP = sys::ImGuiColorEditFlags_NoTooltip;
        /// ColorEdit, ColorPicker: disable display of inline text label (the label is still forwarded to the tooltip and picker).
        const NO_LABEL = sys::ImGuiColorEditFlags_NoLabel;
        /// ColorPicker: disable bigger color preview on right side of the picker, use small colored square preview instead.
        const NO_SIDE_PREVIEW = sys::ImGuiColorEditFlags_NoSidePreview;
        /// ColorEdit: disable drag and drop target. ColorButton: disable drag and drop source.
        const NO_DRAG_DROP = sys::ImGuiColorEditFlags_NoDragDrop;
        /// ColorButton: disable border (which is enforced by default)
        const NO_BORDER = sys::ImGuiColorEditFlags_NoBorder;

        /// ColorEdit, ColorPicker: show vertical alpha bar/gradient in picker.
        const ALPHA_BAR = sys::ImGuiColorEditFlags_AlphaBar;
        /// ColorEdit, ColorPicker, ColorButton: display preview as a transparent color over a checkerboard, instead of opaque.
        const ALPHA_PREVIEW = sys::ImGuiColorEditFlags_AlphaPreview;
        /// ColorEdit, ColorPicker, ColorButton: display half opaque / half checkerboard, instead of opaque.
        const ALPHA_PREVIEW_HALF = sys::ImGuiColorEditFlags_AlphaPreviewHalf;
        /// (WIP) ColorEdit: Currently only disable 0.0f..1.0f limits in RGBA edition (note: you probably want to use ImGuiColorEditFlags_Float flag as well).
        const HDR = sys::ImGuiColorEditFlags_HDR;
        /// ColorEdit: display only as RGB. ColorPicker: disable inputs, display only RGB square.
        const DISPLAY_RGB = sys::ImGuiColorEditFlags_DisplayRGB;
        /// ColorEdit: display only as HSV. ColorPicker: disable inputs, display only HSV square.
        const DISPLAY_HSV = sys::ImGuiColorEditFlags_DisplayHSV;
        /// ColorEdit: display only as HEX. ColorPicker: disable inputs, display only HEX.
        const DISPLAY_HEX = sys::ImGuiColorEditFlags_DisplayHex;
        /// ColorEdit, ColorPicker, ColorButton: _display_ values formatted as 0..255.
        const UINT8 = sys::ImGuiColorEditFlags_Uint8;
        /// ColorEdit, ColorPicker, ColorButton: _display_ values formatted as 0.0f..1.0f floats instead of 0..255 integers. No round-trip of value via integers.
        const FLOAT = sys::ImGuiColorEditFlags_Float;
        /// ColorPicker: bar for Hue, rectangle for Sat/Value.
        const PICKER_HUE_BAR = sys::ImGuiColorEditFlags_PickerHueBar;
        /// ColorPicker: wheel for Hue, triangle for Sat/Value.
        const PICKER_HUE_WHEEL = sys::ImGuiColorEditFlags_PickerHueWheel;
        /// ColorEdit, ColorPicker: input and output data in RGB format.
        const INPUT_RGB = sys::ImGuiColorEditFlags_InputRGB;
        /// ColorEdit, ColorPicker: input and output data in HSV format.
        const INPUT_HSV = sys::ImGuiColorEditFlags_InputHSV;
    }
}

/// # Widgets: Color
impl<'frame> Ui<'frame> {
    /// Display a color picker/editor
    ///
    /// Returns `true` if the color was changed.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Color};
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut color = Color::rgb(1.0, 0.5, 0.0);
    /// # frame.window("Example").show(|ui| {
    /// if ui.color_edit("Color", &mut color) {
    ///     println!("Color changed to: {:?}", color);
    /// }
    /// # });
    /// ```
    pub fn color_edit(&mut self, label: impl AsRef<str>, color: &mut Color) -> bool {
        let mut color_array = [color.r(), color.g(), color.b(), color.a()];

        let changed = unsafe {
            sys::ImGui_ColorEdit4(
                self.scratch_txt(label),
                color_array.as_mut_ptr(),
                0, // Default flags
            )
        };

        if changed {
            *color = Color::rgba(
                color_array[0],
                color_array[1],
                color_array[2],
                color_array[3],
            );
        }

        changed
    }

    /// Display a color picker widget
    ///
    /// Returns `true` if the color was changed.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Color};
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut color = Color::rgb(1.0, 0.5, 0.0);
    /// # frame.window("Example").show(|ui| {
    /// if ui.color_picker("Pick Color", &mut color) {
    ///     println!("Color picked: {:?}", color);
    /// }
    /// # });
    /// ```
    pub fn color_picker(&mut self, label: impl AsRef<str>, color: &mut Color) -> bool {
        let mut color_array = [color.r(), color.g(), color.b(), color.a()];

        let changed = unsafe {
            sys::ImGui_ColorPicker4(
                self.scratch_txt(label),
                color_array.as_mut_ptr(),
                0,                // Default flags
                std::ptr::null(), // No reference color
            )
        };

        if changed {
            *color = Color::rgba(
                color_array[0],
                color_array[1],
                color_array[2],
                color_array[3],
            );
        }

        changed
    }

    /// Display a color button (clickable color swatch)
    ///
    /// Returns `true` if the button was clicked.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Color};
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let color = Color::rgb(1.0, 0.5, 0.0);
    /// # frame.window("Example").show(|ui| {
    /// if ui.color_button("color_btn", color) {
    ///     println!("Color button clicked!");
    /// }
    /// # });
    /// ```
    pub fn color_button(&mut self, desc_id: impl AsRef<str>, color: Color) -> bool {
        let color_vec = sys::ImVec4 {
            x: color.r(),
            y: color.g(),
            z: color.b(),
            w: color.a(),
        };

        unsafe {
            sys::ImGui_ColorButton(
                self.scratch_txt(desc_id),
                &color_vec as *const _,
                0,                                           // Default flags
                &sys::ImVec2 { x: 0.0, y: 0.0 } as *const _, // Default size
            )
        }
    }

    /// Display a color edit widget with flags
    ///
    /// Returns `true` if the color was changed.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Color};
    /// # use dear_imgui::widget::color::ColorEditFlags;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut color = Color::rgb(1.0, 0.5, 0.0);
    /// # frame.window("Example").show(|ui| {
    /// if ui.color_edit_with_flags("Color", &mut color, ColorEditFlags::NO_ALPHA | ColorEditFlags::FLOAT) {
    ///     println!("Color changed to: {:?}", color);
    /// }
    /// # });
    /// ```
    pub fn color_edit_with_flags(
        &mut self,
        label: impl AsRef<str>,
        color: &mut Color,
        flags: ColorEditFlags,
    ) -> bool {
        let mut color_array = [color.r(), color.g(), color.b(), color.a()];

        let changed = unsafe {
            sys::ImGui_ColorEdit4(
                self.scratch_txt(label),
                color_array.as_mut_ptr(),
                flags.bits(),
            )
        };

        if changed {
            *color = Color::rgba(
                color_array[0],
                color_array[1],
                color_array[2],
                color_array[3],
            );
        }

        changed
    }

    /// Display a 3-component color edit widget (RGB only)
    ///
    /// Returns `true` if the color was changed.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Color};
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut color = Color::rgb(1.0, 0.5, 0.0);
    /// # frame.window("Example").show(|ui| {
    /// if ui.color_edit3("RGB Color", &mut color) {
    ///     println!("Color changed to: {:?}", color);
    /// }
    /// # });
    /// ```
    pub fn color_edit3(&mut self, label: impl AsRef<str>, color: &mut Color) -> bool {
        let mut color_array = [color.r(), color.g(), color.b()];

        let changed = unsafe {
            sys::ImGui_ColorEdit3(
                self.scratch_txt(label),
                color_array.as_mut_ptr(),
                0, // Default flags
            )
        };

        if changed {
            *color = Color::rgb(color_array[0], color_array[1], color_array[2]);
        }

        changed
    }

    /// Display a 3-component color edit widget with flags (RGB only)
    ///
    /// Returns `true` if the color was changed.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Color};
    /// # use dear_imgui::widget::color::ColorEditFlags;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut color = Color::rgb(1.0, 0.5, 0.0);
    /// # frame.window("Example").show(|ui| {
    /// if ui.color_edit3_with_flags("RGB Color", &mut color, ColorEditFlags::DISPLAY_HEX) {
    ///     println!("Color changed to: {:?}", color);
    /// }
    /// # });
    /// ```
    pub fn color_edit3_with_flags(
        &mut self,
        label: impl AsRef<str>,
        color: &mut Color,
        flags: ColorEditFlags,
    ) -> bool {
        let mut color_array = [color.r(), color.g(), color.b()];

        let changed = unsafe {
            sys::ImGui_ColorEdit3(
                self.scratch_txt(label),
                color_array.as_mut_ptr(),
                flags.bits(),
            )
        };

        if changed {
            *color = Color::rgb(color_array[0], color_array[1], color_array[2]);
        }

        changed
    }

    /// Display a color picker widget with flags
    ///
    /// Returns `true` if the color was changed.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Color};
    /// # use dear_imgui::widget::color::ColorEditFlags;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut color = Color::rgb(1.0, 0.5, 0.0);
    /// # frame.window("Example").show(|ui| {
    /// if ui.color_picker_with_flags("Pick Color", &mut color, ColorEditFlags::PICKER_HUE_WHEEL) {
    ///     println!("Color picked: {:?}", color);
    /// }
    /// # });
    /// ```
    pub fn color_picker_with_flags(
        &mut self,
        label: impl AsRef<str>,
        color: &mut Color,
        flags: ColorEditFlags,
    ) -> bool {
        let mut color_array = [color.r(), color.g(), color.b(), color.a()];

        let changed = unsafe {
            sys::ImGui_ColorPicker4(
                self.scratch_txt(label),
                color_array.as_mut_ptr(),
                flags.bits(),
                std::ptr::null(), // No reference color
            )
        };

        if changed {
            *color = Color::rgba(
                color_array[0],
                color_array[1],
                color_array[2],
                color_array[3],
            );
        }

        changed
    }

    /// Display a 3-component color picker widget (RGB only)
    ///
    /// Returns `true` if the color was changed.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Color};
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut color = Color::rgb(1.0, 0.5, 0.0);
    /// # frame.window("Example").show(|ui| {
    /// if ui.color_picker3("Pick RGB Color", &mut color) {
    ///     println!("Color picked: {:?}", color);
    /// }
    /// # });
    /// ```
    pub fn color_picker3(&mut self, label: impl AsRef<str>, color: &mut Color) -> bool {
        let mut color_array = [color.r(), color.g(), color.b()];

        let changed = unsafe {
            sys::ImGui_ColorPicker3(
                self.scratch_txt(label),
                color_array.as_mut_ptr(),
                0, // Default flags
            )
        };

        if changed {
            *color = Color::rgb(color_array[0], color_array[1], color_array[2]);
        }

        changed
    }

    /// Display a 3-component color picker widget with flags (RGB only)
    ///
    /// Returns `true` if the color was changed.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Color};
    /// # use dear_imgui::widget::color::ColorEditFlags;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut color = Color::rgb(1.0, 0.5, 0.0);
    /// # frame.window("Example").show(|ui| {
    /// if ui.color_picker3_with_flags("Pick RGB Color", &mut color, ColorEditFlags::PICKER_HUE_BAR) {
    ///     println!("Color picked: {:?}", color);
    /// }
    /// # });
    /// ```
    pub fn color_picker3_with_flags(
        &mut self,
        label: impl AsRef<str>,
        color: &mut Color,
        flags: ColorEditFlags,
    ) -> bool {
        let mut color_array = [color.r(), color.g(), color.b()];

        let changed = unsafe {
            sys::ImGui_ColorPicker3(
                self.scratch_txt(label),
                color_array.as_mut_ptr(),
                flags.bits(),
            )
        };

        if changed {
            *color = Color::rgb(color_array[0], color_array[1], color_array[2]);
        }

        changed
    }
}

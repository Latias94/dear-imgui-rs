use crate::types::Color;
use crate::ui::Ui;
use dear_imgui_sys as sys;
/// Color widgets
///
/// This module contains all color-related UI components like color pickers, editors, etc.
use std::ffi::CString;

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
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        let mut color_array = [color.r(), color.g(), color.b(), color.a()];

        let changed = unsafe {
            sys::ImGui_ColorEdit4(
                c_label.as_ptr(),
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
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        let mut color_array = [color.r(), color.g(), color.b(), color.a()];

        let changed = unsafe {
            sys::ImGui_ColorPicker4(
                c_label.as_ptr(),
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
        let desc_id = desc_id.as_ref();
        let c_desc_id = CString::new(desc_id).unwrap_or_default();
        let color_vec = sys::ImVec4 {
            x: color.r(),
            y: color.g(),
            z: color.b(),
            w: color.a(),
        };

        unsafe {
            sys::ImGui_ColorButton(
                c_desc_id.as_ptr(),
                &color_vec as *const _,
                0,                                           // Default flags
                &sys::ImVec2 { x: 0.0, y: 0.0 } as *const _, // Default size
            )
        }
    }
}

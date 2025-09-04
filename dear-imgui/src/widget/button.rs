use crate::types::Vec2;
use crate::ui::Ui;
use dear_imgui_sys as sys;
/// Button widgets
///
/// This module contains all button-related UI components.
use std::ffi::CString;

/// # Widgets: Buttons
impl<'frame> Ui<'frame> {
    /// Display a button
    ///
    /// Returns `true` if the button was clicked.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.button("Click me") {
    ///     println!("Button was clicked!");
    /// }
    /// # });
    /// ```
    pub fn button(&mut self, label: impl AsRef<str>) -> bool {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        unsafe {
            sys::ImGui_Button(
                c_label.as_ptr(),
                &sys::ImVec2 { x: 0.0, y: 0.0 } as *const _,
            )
        }
    }

    /// Display a button with custom size
    ///
    /// Returns `true` if the button was clicked.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Vec2};
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.button_with_size("Big Button", Vec2::new(200.0, 50.0)) {
    ///     println!("Big button was clicked!");
    /// }
    /// # });
    /// ```
    pub fn button_with_size(&mut self, label: impl AsRef<str>, size: Vec2) -> bool {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        let size_vec = sys::ImVec2 {
            x: size.x,
            y: size.y,
        };
        unsafe { sys::ImGui_Button(c_label.as_ptr(), &size_vec as *const _) }
    }

    /// Display a small button (without padding)
    ///
    /// Returns `true` if the button was clicked.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.small_button("X") {
    ///     println!("Small button was clicked!");
    /// }
    /// # });
    /// ```
    pub fn small_button(&mut self, label: impl AsRef<str>) -> bool {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        unsafe { sys::ImGui_SmallButton(c_label.as_ptr()) }
    }

    /// Display an invisible button (for custom drawing or hit testing)
    ///
    /// Returns `true` if the button was clicked.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Vec2};
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.invisible_button("invisible_btn", Vec2::new(100.0, 50.0)) {
    ///     println!("Invisible button clicked!");
    /// }
    /// # });
    /// ```
    pub fn invisible_button(&mut self, str_id: impl AsRef<str>, size: Vec2) -> bool {
        let str_id = str_id.as_ref();
        let c_str_id = CString::new(str_id).unwrap_or_default();
        let size_vec = sys::ImVec2 {
            x: size.x,
            y: size.y,
        };
        unsafe {
            sys::ImGui_InvisibleButton(
                c_str_id.as_ptr(),
                &size_vec as *const _,
                0, // Default flags
            )
        }
    }

    /// Display an arrow button
    ///
    /// Returns `true` if the button was clicked.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.arrow_button("left_arrow", 0) { // 0 = Left
    ///     println!("Left arrow clicked!");
    /// }
    /// if ui.arrow_button("right_arrow", 1) { // 1 = Right
    ///     println!("Right arrow clicked!");
    /// }
    /// # });
    /// ```
    pub fn arrow_button(&mut self, str_id: impl AsRef<str>, dir: i32) -> bool {
        let str_id = str_id.as_ref();
        let c_str_id = CString::new(str_id).unwrap_or_default();
        unsafe { sys::ImGui_ArrowButton(c_str_id.as_ptr(), dir) }
    }
}

use crate::types::Color;
use crate::ui::Ui;
use dear_imgui_sys as sys;
/// Text display widgets
///
/// This module contains all text-related UI components.
use std::ffi::CString;

/// # Widgets: Text
impl<'frame> Ui<'frame> {
    /// Display text
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.text("Hello, world!");
    /// ui.text(format!("Counter: {}", 42));
    /// # });
    /// ```
    pub fn text(&mut self, text: impl AsRef<str>) {
        let text = text.as_ref();
        let c_text = CString::new(text).unwrap_or_default();
        unsafe {
            sys::ImGui_TextUnformatted(c_text.as_ptr(), std::ptr::null());
        }
    }

    /// Display colored text
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Color};
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.text_colored(Color::RED, "Error message!");
    /// ui.text_colored(Color::GREEN, "Success!");
    /// # });
    /// ```
    pub fn text_colored(&mut self, color: Color, text: impl AsRef<str>) {
        let text = text.as_ref();
        let c_text = CString::new(text).unwrap_or_default();
        let color_vec = sys::ImVec4 {
            x: color.r(),
            y: color.g(),
            z: color.b(),
            w: color.a(),
        };
        unsafe {
            sys::ImGui_TextColored(&color_vec as *const _, c_text.as_ptr());
        }
    }

    /// Display disabled (grayed out) text
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.text_disabled("This feature is not available");
    /// # });
    /// ```
    pub fn text_disabled(&mut self, text: impl AsRef<str>) {
        let text = text.as_ref();
        let c_text = CString::new(text).unwrap_or_default();
        unsafe {
            sys::ImGui_TextDisabled(c_text.as_ptr());
        }
    }

    /// Display wrapped text
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.text_wrapped("This is a very long text that will be wrapped to fit within the available width of the window or container.");
    /// # });
    /// ```
    pub fn text_wrapped(&mut self, text: impl AsRef<str>) {
        let text = text.as_ref();
        let c_text = CString::new(text).unwrap_or_default();
        unsafe {
            sys::ImGui_TextWrapped(c_text.as_ptr());
        }
    }

    /// Display bullet text (bullet + text in one call)
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.bullet_text("This is a bullet point");
    /// ui.bullet_text("This is another bullet point");
    /// # });
    /// ```
    pub fn bullet_text(&mut self, text: impl AsRef<str>) {
        let text = text.as_ref();
        let c_text = CString::new(text).unwrap_or_default();
        unsafe {
            sys::ImGui_BulletText(c_text.as_ptr());
        }
    }

    /// Display text + label combination aligned the same way as value+label widgets
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.label_text("Status", "Ready");
    /// ui.label_text("Count", "42");
    /// # });
    /// ```
    pub fn label_text(&mut self, label: impl AsRef<str>, text: impl AsRef<str>) {
        let label = label.as_ref();
        let text = text.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        let c_text = CString::new(text).unwrap_or_default();
        let fmt = CString::new("%s").unwrap();
        unsafe {
            sys::ImGui_LabelText(c_label.as_ptr(), fmt.as_ptr(), c_text.as_ptr());
        }
    }
}

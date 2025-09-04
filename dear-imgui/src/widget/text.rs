use crate::types::Color;
use crate::ui::Ui;
use dear_imgui_sys as sys;
use std::os::raw::c_char;

/// Format string for text functions that need printf-style formatting
static FMT: &[u8] = b"%s\0";

#[inline]
fn fmt_ptr() -> *const c_char {
    FMT.as_ptr() as *const c_char
}

/// Text display widgets
///
/// This module contains all text-related UI components.

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
        let s = text.as_ref();
        unsafe {
            let start = s.as_ptr();
            let end = start.add(s.len());
            sys::ImGui_TextUnformatted(start as *const c_char, end as *const c_char);
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
        let color_vec = sys::ImVec4 {
            x: color.r(),
            y: color.g(),
            z: color.b(),
            w: color.a(),
        };
        unsafe {
            sys::ImGui_TextColored(&color_vec as *const _, self.scratch_txt(text));
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
        unsafe {
            sys::ImGui_TextDisabled(self.scratch_txt(text));
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
        unsafe {
            sys::ImGui_TextWrapped(fmt_ptr(), self.scratch_txt(text));
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
        unsafe {
            sys::ImGui_BulletText(fmt_ptr(), self.scratch_txt(text));
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
        let (label_ptr, text_ptr) = self.scratch_txt_two(label, text);
        unsafe {
            sys::ImGui_LabelText(label_ptr, fmt_ptr(), text_ptr);
        }
    }
}

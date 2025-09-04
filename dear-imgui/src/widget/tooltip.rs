use crate::ui::Ui;
use dear_imgui_sys as sys;

/// Tooltip widgets
///
/// This module contains all tooltip-related UI components.

/// # Widgets: Tooltip
impl<'frame> Ui<'frame> {
    /// Set a tooltip for the last item
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.button("Hover me");
    /// ui.set_tooltip("This is a tooltip!");
    /// # });
    /// ```
    pub fn set_tooltip(&mut self, text: impl AsRef<str>) {
        unsafe {
            sys::ImGui_SetTooltip(self.scratch_txt(text));
        }
    }

    /// Begin a tooltip window
    ///
    /// Must call `end_tooltip()` after adding content.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.button("Hover me");
    /// if ui.is_item_hovered() {
    ///     ui.begin_tooltip();
    ///     ui.text("Custom tooltip content");
    ///     ui.text("With multiple lines");
    ///     ui.end_tooltip();
    /// }
    /// # });
    /// ```
    pub fn begin_tooltip(&mut self) {
        unsafe {
            sys::ImGui_BeginTooltip();
        }
    }

    /// End tooltip window (must be called after begin_tooltip)
    pub fn end_tooltip(&mut self) {
        unsafe {
            sys::ImGui_EndTooltip();
        }
    }

    /// Check if the last item is hovered
    ///
    /// Returns `true` if the last item is being hovered.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.button("Hover me");
    /// if ui.is_item_hovered() {
    ///     ui.set_tooltip("Button is hovered!");
    /// }
    /// # });
    /// ```
    pub fn is_item_hovered(&mut self) -> bool {
        unsafe {
            sys::ImGui_IsItemHovered(0) // Default flags
        }
    }

    /// Check if the last item is active (being clicked)
    ///
    /// Returns `true` if the last item is being clicked.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.button("Click me");
    /// if ui.is_item_active() {
    ///     ui.text("Button is being clicked!");
    /// }
    /// # });
    /// ```
    pub fn is_item_active(&mut self) -> bool {
        unsafe { sys::ImGui_IsItemActive() }
    }

    /// Check if the last item is focused
    ///
    /// Returns `true` if the last item has keyboard focus.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # let mut text = String::new();
    /// # frame.window("Example").show(|ui| {
    /// ui.input_text("Input", &mut text);
    /// if ui.is_item_focused() {
    ///     ui.text("Input field is focused!");
    /// }
    /// # });
    /// ```
    pub fn is_item_focused(&mut self) -> bool {
        unsafe { sys::ImGui_IsItemFocused() }
    }
}

use crate::ui::Ui;
use dear_imgui_sys as sys;

/// Column layout widgets
///
/// This module contains all column-related UI components for creating multi-column layouts.

/// # Widgets: Columns
impl<'frame> Ui<'frame> {
    /// Begin columns layout
    ///
    /// Start a multi-column layout. Must call `end_columns()` after adding content.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.columns(3, "my_columns", true);
    ///
    /// ui.text("Column 1");
    /// ui.next_column();
    ///
    /// ui.text("Column 2");
    /// ui.next_column();
    ///
    /// ui.text("Column 3");
    /// ui.next_column();
    ///
    /// ui.end_columns();
    /// # });
    /// ```
    pub fn columns(&mut self, count: i32, id: impl AsRef<str>, border: bool) {
        unsafe {
            sys::ImGui_Columns(count, self.scratch_txt(id), border);
        }
    }

    /// Move to the next column
    ///
    /// Move the cursor to the next column in a columns layout.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.columns(2, "cols", true);
    /// ui.text("First column");
    /// ui.next_column();
    /// ui.text("Second column");
    /// ui.end_columns();
    /// # });
    /// ```
    pub fn next_column(&mut self) {
        unsafe {
            sys::ImGui_NextColumn();
        }
    }

    /// Get current column index
    ///
    /// Returns the index of the current column (0-based).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.columns(3, "cols", true);
    /// let current = ui.get_column_index();
    /// ui.text(&format!("Current column: {}", current));
    /// # });
    /// ```
    pub fn get_column_index(&mut self) -> i32 {
        unsafe { sys::ImGui_GetColumnIndex() }
    }

    /// Get column width
    ///
    /// Returns the width of the specified column (-1 for current column).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.columns(2, "cols", true);
    /// let width = ui.get_column_width(-1);
    /// ui.text(&format!("Column width: {:.1}", width));
    /// # });
    /// ```
    pub fn get_column_width(&mut self, column_index: i32) -> f32 {
        unsafe { sys::ImGui_GetColumnWidth(column_index) }
    }

    /// Set column width
    ///
    /// Set the width of the specified column.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.columns(2, "cols", true);
    /// ui.set_column_width(0, 100.0);
    /// ui.text("Fixed width column");
    /// # });
    /// ```
    pub fn set_column_width(&mut self, column_index: i32, width: f32) {
        unsafe {
            sys::ImGui_SetColumnWidth(column_index, width);
        }
    }

    /// Get column offset
    ///
    /// Returns the offset of the specified column (-1 for current column).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.columns(2, "cols", true);
    /// let offset = ui.get_column_offset(-1);
    /// ui.text(&format!("Column offset: {:.1}", offset));
    /// # });
    /// ```
    pub fn get_column_offset(&mut self, column_index: i32) -> f32 {
        unsafe { sys::ImGui_GetColumnOffset(column_index) }
    }

    /// Set column offset
    ///
    /// Set the offset of the specified column.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.columns(2, "cols", true);
    /// ui.set_column_offset(1, 150.0);
    /// ui.text("Custom positioned column");
    /// # });
    /// ```
    pub fn set_column_offset(&mut self, column_index: i32, offset: f32) {
        unsafe {
            sys::ImGui_SetColumnOffset(column_index, offset);
        }
    }

    /// Get columns count
    ///
    /// Returns the number of columns in the current columns layout.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.columns(3, "cols", true);
    /// let count = ui.get_columns_count();
    /// ui.text(&format!("Total columns: {}", count));
    /// # });
    /// ```
    pub fn get_columns_count(&mut self) -> i32 {
        unsafe { sys::ImGui_GetColumnsCount() }
    }

    /// End columns layout
    ///
    /// End the current columns layout. Must be called after `columns()`.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// ui.columns(2, "cols", true);
    /// ui.text("Column content");
    /// ui.end_columns();
    /// # });
    /// ```
    pub fn end_columns(&mut self) {
        unsafe {
            sys::ImGui_Columns(1, std::ptr::null(), false);
        }
    }
}

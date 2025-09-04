use crate::ui::Ui;
use dear_imgui_sys as sys;
/// Table widgets
///
/// This module contains all table-related UI components.
use std::ffi::CString;

/// # Widgets: Table
impl<'frame> Ui<'frame> {
    /// Begin a table
    ///
    /// Returns `true` if the table is visible and should be populated.
    /// Must call `end_table()` if this returns true.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_table("MyTable", 3) {
    ///     ui.table_setup_column("Name");
    ///     ui.table_setup_column("Age");
    ///     ui.table_setup_column("City");
    ///     ui.table_headers_row();
    ///
    ///     ui.table_next_row();
    ///     ui.table_next_column();
    ///     ui.text("John");
    ///     ui.table_next_column();
    ///     ui.text("25");
    ///     ui.table_next_column();
    ///     ui.text("New York");
    ///
    ///     ui.end_table();
    /// }
    /// # });
    /// ```
    pub fn begin_table(&mut self, str_id: impl AsRef<str>, columns: i32) -> bool {
        let str_id = str_id.as_ref();
        let c_str_id = CString::new(str_id).unwrap_or_default();
        unsafe {
            sys::ImGui_BeginTable(
                c_str_id.as_ptr(),
                columns,
                0,                                           // Default flags
                &sys::ImVec2 { x: 0.0, y: 0.0 } as *const _, // Default outer_size
                0.0,                                         // Default inner_width
            )
        }
    }

    /// End table (must be called after begin_table returns true)
    pub fn end_table(&mut self) {
        unsafe {
            sys::ImGui_EndTable();
        }
    }

    /// Setup a table column
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_table("MyTable", 2) {
    ///     ui.table_setup_column("Column 1");
    ///     ui.table_setup_column("Column 2");
    ///     ui.table_headers_row();
    ///     ui.end_table();
    /// }
    /// # });
    /// ```
    pub fn table_setup_column(&mut self, label: impl AsRef<str>) {
        let label = label.as_ref();
        let c_label = CString::new(label).unwrap_or_default();
        unsafe {
            sys::ImGui_TableSetupColumn(
                c_label.as_ptr(),
                0,   // Default flags
                0.0, // Default init_width_or_weight
                0,   // Default user_id
            );
        }
    }

    /// Create table headers row
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_table("MyTable", 2) {
    ///     ui.table_setup_column("Name");
    ///     ui.table_setup_column("Value");
    ///     ui.table_headers_row(); // Creates clickable headers
    ///     ui.end_table();
    /// }
    /// # });
    /// ```
    pub fn table_headers_row(&mut self) {
        unsafe {
            sys::ImGui_TableHeadersRow();
        }
    }

    /// Move to next table row
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_table("MyTable", 2) {
    ///     ui.table_next_row();
    ///     ui.table_next_column();
    ///     ui.text("Cell 1");
    ///     ui.table_next_column();
    ///     ui.text("Cell 2");
    ///     ui.end_table();
    /// }
    /// # });
    /// ```
    pub fn table_next_row(&mut self) {
        unsafe {
            sys::ImGui_TableNextRow(0, 0.0); // Default flags and min_row_height
        }
    }

    /// Move to next table column
    ///
    /// Returns `true` if the column is visible.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_table("MyTable", 3) {
    ///     ui.table_next_row();
    ///     if ui.table_next_column() {
    ///         ui.text("Column 1");
    ///     }
    ///     if ui.table_next_column() {
    ///         ui.text("Column 2");
    ///     }
    ///     if ui.table_next_column() {
    ///         ui.text("Column 3");
    ///     }
    ///     ui.end_table();
    /// }
    /// # });
    /// ```
    pub fn table_next_column(&mut self) -> bool {
        unsafe { sys::ImGui_TableNextColumn() }
    }

    /// Set current table column index
    ///
    /// Returns `true` if the column is visible.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_table("MyTable", 3) {
    ///     ui.table_next_row();
    ///     if ui.table_set_column_index(1) { // Skip to column 1
    ///         ui.text("Second column");
    ///     }
    ///     ui.end_table();
    /// }
    /// # });
    /// ```
    pub fn table_set_column_index(&mut self, column_n: i32) -> bool {
        unsafe { sys::ImGui_TableSetColumnIndex(column_n) }
    }
}

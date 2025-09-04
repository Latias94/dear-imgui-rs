use crate::types::Vec2;
use crate::ui::Ui;
use dear_imgui_sys as sys;

/// Table widgets
///
/// This module contains all table-related UI components.

bitflags::bitflags! {
    /// Flags for table widgets
    #[repr(transparent)]
    #[derive(Debug)]
    pub struct TableFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Enable resizing columns.
        const RESIZABLE = sys::ImGuiTableFlags_Resizable;
        /// Enable reordering columns in header row (need calling TableSetupColumn() + TableHeadersRow() to display headers)
        const REORDERABLE = sys::ImGuiTableFlags_Reorderable;
        /// Enable hiding/disabling columns in context menu.
        const HIDEABLE = sys::ImGuiTableFlags_Hideable;
        /// Enable sorting. Call TableGetSortSpecs() to obtain sort specs. Also see ImGuiTableFlags_SortMulti and ImGuiTableFlags_SortTristate.
        const SORTABLE = sys::ImGuiTableFlags_Sortable;
        /// Disable persisting columns order, width and sort settings in the .ini file.
        const NO_SAVED_SETTINGS = sys::ImGuiTableFlags_NoSavedSettings;
        /// Right-click on columns body/contents will display table context menu. By default it is available in TableHeadersRow().
        const CONTEXT_MENU_IN_BODY = sys::ImGuiTableFlags_ContextMenuInBody;

        // Decorations
        /// Set each RowBg color with ImGuiCol_TableRowBg or ImGuiCol_TableRowBgAlt (equivalent of calling TableSetBgColor with ImGuiTableBgFlags_RowBg0 on each row manually)
        const ROW_BG = sys::ImGuiTableFlags_RowBg;
        /// Draw horizontal borders between rows.
        const BORDERS_INNER_H = sys::ImGuiTableFlags_BordersInnerH;
        /// Draw horizontal borders at the top and bottom.
        const BORDERS_OUTER_H = sys::ImGuiTableFlags_BordersOuterH;
        /// Draw vertical borders between columns.
        const BORDERS_INNER_V = sys::ImGuiTableFlags_BordersInnerV;
        /// Draw vertical borders on the left and right sides.
        const BORDERS_OUTER_V = sys::ImGuiTableFlags_BordersOuterV;
        /// Draw horizontal borders.
        const BORDERS_H = sys::ImGuiTableFlags_BordersInnerH | sys::ImGuiTableFlags_BordersOuterH;
        /// Draw vertical borders.
        const BORDERS_V = sys::ImGuiTableFlags_BordersInnerV | sys::ImGuiTableFlags_BordersOuterV;
        /// Draw inner borders.
        const BORDERS_INNER = sys::ImGuiTableFlags_BordersInnerV | sys::ImGuiTableFlags_BordersInnerH;
        /// Draw outer borders.
        const BORDERS_OUTER = sys::ImGuiTableFlags_BordersOuterV | sys::ImGuiTableFlags_BordersOuterH;
        /// Draw all borders.
        const BORDERS = sys::ImGuiTableFlags_BordersInnerV | sys::ImGuiTableFlags_BordersInnerH | sys::ImGuiTableFlags_BordersOuterV | sys::ImGuiTableFlags_BordersOuterH;
        /// [ALPHA] Disable vertical borders in columns Body (borders will always appears in Headers). -> May move to style
        const NO_BORDERS_IN_BODY = sys::ImGuiTableFlags_NoBordersInBody;
        /// [ALPHA] Disable vertical borders in columns Body until hovered for resize (borders will always appears in Headers). -> May move to style
        const NO_BORDERS_IN_BODY_UNTIL_RESIZE = sys::ImGuiTableFlags_NoBordersInBodyUntilResize;

        // Sizing Policy (read above for defaults)
        /// Columns default to _WidthFixed or _WidthAuto (if resizable or not resizable), matching contents width.
        const SIZING_FIXED_FIT = sys::ImGuiTableFlags_SizingFixedFit;
        /// Columns default to _WidthFixed or _WidthAuto (if resizable or not resizable), matching the maximum contents width of all columns. Implicitly enable ImGuiTableFlags_NoKeepColumnsVisible.
        const SIZING_FIXED_SAME = sys::ImGuiTableFlags_SizingFixedSame;
        /// Columns default to _WidthStretch with default weights proportional to each columns contents widths.
        const SIZING_STRETCH_PROP = sys::ImGuiTableFlags_SizingStretchProp;
        /// Columns default to _WidthStretch with default weights all equal, unless overridden by TableSetupColumn().
        const SIZING_STRETCH_SAME = sys::ImGuiTableFlags_SizingStretchSame;

        // Sizing Extra Options
        /// Make outer width auto-fit to columns, overriding outer_size.x value. Only available when ScrollX/ScrollY are disabled and Stretch columns are not used.
        const NO_HOST_EXTEND_X = sys::ImGuiTableFlags_NoHostExtendX;
        /// Make outer height stop exactly at outer_size.y (prevent auto-extending table past the limit). Only available when ScrollX/ScrollY are disabled. Data below the limit will be clipped and not visible.
        const NO_HOST_EXTEND_Y = sys::ImGuiTableFlags_NoHostExtendY;
        /// Disable keeping column always minimally visible when ScrollX is on and table gets too small. Not recommended.
        const NO_KEEP_COLUMNS_VISIBLE = sys::ImGuiTableFlags_NoKeepColumnsVisible;
        /// Disable distributing remainder width to stretched columns (width allocation on a 100-wide table with 3 columns: Without this flag: 33,33,34. With this flag: 33,33,33). With larger number of columns, resizing will appear to be less smooth.
        const PRECISE_WIDTHS = sys::ImGuiTableFlags_PreciseWidths;

        // Clipping
        /// Disable clipping rectangle for every individual columns (reduce draw command count, items will be able to overflow into other columns). Generally incompatible with TableSetupScrollFreeze().
        const NO_CLIP = sys::ImGuiTableFlags_NoClip;

        // Padding
        /// Default if BordersOuterV is on. Enable outer-most padding. Generally desirable if you have headers.
        const PAD_OUTER_X = sys::ImGuiTableFlags_PadOuterX;
        /// Default if BordersOuterV is off. Disable outer-most padding.
        const NO_PAD_OUTER_X = sys::ImGuiTableFlags_NoPadOuterX;
        /// Disable inner padding between columns (double inner padding if BordersOuterV is on, single inner padding if BordersOuterV is off).
        const NO_PAD_INNER_X = sys::ImGuiTableFlags_NoPadInnerX;

        // Scrolling
        /// Enable horizontal scrolling. Require 'outer_size' parameter of BeginTable() to specify the container size. Changes default sizing policy. Because this create a child window, ScrollY is currently generally recommended when using ScrollX.
        const SCROLL_X = sys::ImGuiTableFlags_ScrollX;
        /// Enable vertical scrolling. Require 'outer_size' parameter of BeginTable() to specify the container size.
        const SCROLL_Y = sys::ImGuiTableFlags_ScrollY;

        // Sorting
        /// Hold shift when clicking headers to sort on multiple column. TableGetSortSpecs() may return specs where (SpecsCount > 1).
        const SORT_MULTI = sys::ImGuiTableFlags_SortMulti;
        /// Allow no sorting, disable default sorting. TableGetSortSpecs() may return specs where (SpecsCount == 0).
        const SORT_TRISTATE = sys::ImGuiTableFlags_SortTristate;
    }
}

bitflags::bitflags! {
    /// Flags for table columns
    #[repr(transparent)]
    #[derive(Debug)]
    pub struct TableColumnFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Overriding/master disable flag: hide column, won't show in context menu (unlike calling TableSetColumnEnabled() which manipulates the user accessible state)
        const DISABLED = sys::ImGuiTableColumnFlags_Disabled;
        /// Default as a hidden/disabled column.
        const DEFAULT_HIDE = sys::ImGuiTableColumnFlags_DefaultHide;
        /// Default as a sorting column.
        const DEFAULT_SORT = sys::ImGuiTableColumnFlags_DefaultSort;
        /// Column will stretch. Preferable with horizontal scrolling disabled (default if table sizing policy is _SizingStretchSame or _SizingStretchProp).
        const WIDTH_STRETCH = sys::ImGuiTableColumnFlags_WidthStretch;
        /// Column will not stretch. Preferable with horizontal scrolling enabled (default if table sizing policy is _SizingFixedFit and table is resizable).
        const WIDTH_FIXED = sys::ImGuiTableColumnFlags_WidthFixed;
        /// Disable manual resizing.
        const NO_RESIZE = sys::ImGuiTableColumnFlags_NoResize;
        /// Disable manual reordering this column, this will also prevent other columns from crossing over this column.
        const NO_REORDER = sys::ImGuiTableColumnFlags_NoReorder;
        /// Disable ability to hide/disable this column.
        const NO_HIDE = sys::ImGuiTableColumnFlags_NoHide;
        /// Disable clipping for this column (all NoClip columns will render in a same draw command).
        const NO_CLIP = sys::ImGuiTableColumnFlags_NoClip;
        /// Disable ability to sort on this field (even if ImGuiTableFlags_Sortable is set on the table).
        const NO_SORT = sys::ImGuiTableColumnFlags_NoSort;
        /// Disable ability to sort in the ascending direction.
        const NO_SORT_ASCENDING = sys::ImGuiTableColumnFlags_NoSortAscending;
        /// Disable ability to sort in the descending direction.
        const NO_SORT_DESCENDING = sys::ImGuiTableColumnFlags_NoSortDescending;
        /// TableHeadersRow() will not submit label for this column. Convenient for some small columns. Name will still appear in context menu.
        const NO_HEADER_LABEL = sys::ImGuiTableColumnFlags_NoHeaderLabel;
        /// Disable header text width contribution to automatic column width.
        const NO_HEADER_WIDTH = sys::ImGuiTableColumnFlags_NoHeaderWidth;
        /// Make the initial sort direction Ascending when first sorting on this column (default).
        const PREFER_SORT_ASCENDING = sys::ImGuiTableColumnFlags_PreferSortAscending;
        /// Make the initial sort direction Descending when first sorting on this column.
        const PREFER_SORT_DESCENDING = sys::ImGuiTableColumnFlags_PreferSortDescending;
        /// Use current Indent value when entering cell (default for column 0).
        const INDENT_ENABLE = sys::ImGuiTableColumnFlags_IndentEnable;
        /// Ignore current Indent value when entering cell (default for columns > 0). Indentation changes _within_ the cell will still be honored.
        const INDENT_DISABLE = sys::ImGuiTableColumnFlags_IndentDisable;
    }
}

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
        unsafe {
            sys::ImGui_BeginTable(
                self.scratch_txt(str_id),
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
        unsafe {
            sys::ImGui_TableSetupColumn(
                self.scratch_txt(label),
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

    /// Begin a table with flags
    ///
    /// Returns `true` if the table is visible and should be populated.
    /// Must call `end_table()` if this returns true.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # use dear_imgui::widget::table::TableFlags;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_table_with_flags("MyTable", 3, TableFlags::BORDERS | TableFlags::RESIZABLE) {
    ///     ui.table_setup_column("Name");
    ///     ui.table_setup_column("Age");
    ///     ui.table_setup_column("City");
    ///     ui.table_headers_row();
    ///     ui.end_table();
    /// }
    /// # });
    /// ```
    pub fn begin_table_with_flags(
        &mut self,
        str_id: impl AsRef<str>,
        columns: i32,
        flags: TableFlags,
    ) -> bool {
        unsafe {
            sys::ImGui_BeginTable(
                self.scratch_txt(str_id),
                columns,
                flags.bits(),
                &sys::ImVec2 { x: 0.0, y: 0.0 } as *const _, // Default outer_size
                0.0,                                         // Default inner_width
            )
        }
    }

    /// Begin a table with full options
    ///
    /// Returns `true` if the table is visible and should be populated.
    /// Must call `end_table()` if this returns true.
    ///
    /// # Arguments
    ///
    /// * `str_id` - Unique identifier for the table
    /// * `columns` - Number of columns
    /// * `flags` - Table flags
    /// * `outer_size` - Outer size of the table
    /// * `inner_width` - Inner width when scrolling is enabled
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Vec2};
    /// # use dear_imgui::widget::table::TableFlags;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_table_ex("MyTable", 3, TableFlags::SCROLL_Y, Vec2::new(400.0, 200.0), 0.0) {
    ///     ui.table_setup_column("Name");
    ///     ui.table_setup_column("Age");
    ///     ui.table_setup_column("City");
    ///     ui.table_headers_row();
    ///     ui.end_table();
    /// }
    /// # });
    /// ```
    pub fn begin_table_ex(
        &mut self,
        str_id: impl AsRef<str>,
        columns: i32,
        flags: TableFlags,
        outer_size: Vec2,
        inner_width: f32,
    ) -> bool {
        let outer_size_vec = sys::ImVec2 {
            x: outer_size.x,
            y: outer_size.y,
        };
        unsafe {
            sys::ImGui_BeginTable(
                self.scratch_txt(str_id),
                columns,
                flags.bits(),
                &outer_size_vec as *const _,
                inner_width,
            )
        }
    }

    /// Setup a column with flags
    ///
    /// # Arguments
    ///
    /// * `label` - Column label (can be empty)
    /// * `flags` - Column flags
    /// * `init_width_or_weight` - Initial width (for fixed columns) or weight (for stretch columns)
    /// * `user_id` - User ID for the column
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # use dear_imgui::widget::table::{TableFlags, TableColumnFlags};
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_table_with_flags("MyTable", 3, TableFlags::RESIZABLE) {
    ///     ui.table_setup_column_ex("Name", TableColumnFlags::WIDTH_FIXED, 100.0, 0);
    ///     ui.table_setup_column_ex("Age", TableColumnFlags::WIDTH_FIXED, 50.0, 1);
    ///     ui.table_setup_column_ex("City", TableColumnFlags::WIDTH_STRETCH, 0.0, 2);
    ///     ui.table_headers_row();
    ///     ui.end_table();
    /// }
    /// # });
    /// ```
    pub fn table_setup_column_ex(
        &mut self,
        label: impl AsRef<str>,
        flags: TableColumnFlags,
        init_width_or_weight: f32,
        user_id: u32,
    ) {
        unsafe {
            sys::ImGui_TableSetupColumn(
                self.scratch_txt(label),
                flags.bits(),
                init_width_or_weight,
                user_id,
            );
        }
    }

    /// Setup scrolling freeze for the table
    ///
    /// # Arguments
    ///
    /// * `cols` - Number of columns to freeze
    /// * `rows` - Number of rows to freeze
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # use dear_imgui::widget::table::TableFlags;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_table_with_flags("MyTable", 3, TableFlags::SCROLL_X | TableFlags::SCROLL_Y) {
    ///     ui.table_setup_scroll_freeze(1, 1); // Freeze first column and first row
    ///     ui.table_setup_column("Name");
    ///     ui.table_setup_column("Age");
    ///     ui.table_setup_column("City");
    ///     ui.table_headers_row();
    ///     ui.end_table();
    /// }
    /// # });
    /// ```
    pub fn table_setup_scroll_freeze(&mut self, cols: i32, rows: i32) {
        unsafe {
            sys::ImGui_TableSetupScrollFreeze(cols, rows);
        }
    }

    /// Get the current table sort specifications
    ///
    /// Returns `None` if no sorting is active or available.
    /// The returned pointer is valid until the next call to `table_headers_row()` or `end_table()`.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # use dear_imgui::widget::table::TableFlags;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// if ui.begin_table_with_flags("MyTable", 3, TableFlags::SORTABLE) {
    ///     ui.table_setup_column("Name");
    ///     ui.table_setup_column("Age");
    ///     ui.table_setup_column("City");
    ///     ui.table_headers_row();
    ///
    ///     // Check for sorting changes
    ///     if let Some(sort_specs) = ui.table_get_sort_specs() {
    ///         if unsafe { (*sort_specs).SpecsDirty } {
    ///             // Sort your data here
    ///             unsafe { (*sort_specs).SpecsDirty = false; }
    ///         }
    ///     }
    ///
    ///     ui.end_table();
    /// }
    /// # });
    /// ```
    pub fn table_get_sort_specs(&mut self) -> Option<*mut sys::ImGuiTableSortSpecs> {
        let specs = unsafe { sys::ImGui_TableGetSortSpecs() };
        if specs.is_null() {
            None
        } else {
            Some(specs)
        }
    }

    /// Get the number of columns in the current table
    ///
    /// Returns 0 if not in a table.
    pub fn table_get_column_count(&self) -> i32 {
        unsafe { sys::ImGui_TableGetColumnCount() }
    }

    /// Get the current column index
    ///
    /// Returns -1 if not in a table.
    pub fn table_get_column_index(&self) -> i32 {
        unsafe { sys::ImGui_TableGetColumnIndex() }
    }

    /// Get the current row index
    ///
    /// Returns -1 if not in a table.
    pub fn table_get_row_index(&self) -> i32 {
        unsafe { sys::ImGui_TableGetRowIndex() }
    }

    /// Get the name of a column by index
    ///
    /// Returns empty string if column doesn't exist.
    pub fn table_get_column_name(&self, column_n: Option<i32>) -> String {
        let column_n = column_n.unwrap_or(-1);
        unsafe {
            let name_ptr = sys::ImGui_TableGetColumnName(column_n);
            if name_ptr.is_null() {
                String::new()
            } else {
                std::ffi::CStr::from_ptr(name_ptr)
                    .to_string_lossy()
                    .into_owned()
            }
        }
    }

    /// Get the flags of a column by index
    ///
    /// Returns NONE if column doesn't exist.
    pub fn table_get_column_flags(&self, column_n: Option<i32>) -> TableColumnFlags {
        let column_n = column_n.unwrap_or(-1);
        let flags = unsafe { sys::ImGui_TableGetColumnFlags(column_n) };
        TableColumnFlags::from_bits_truncate(flags)
    }
}

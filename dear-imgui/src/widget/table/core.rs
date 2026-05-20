use crate::ui::Ui;
use crate::widget::table::{
    SortDirection, TABLE_MAX_COLUMNS, TableBgTarget, TableBuilder, TableColumnFlags,
    TableColumnIndent, TableColumnIndex, TableColumnRef, TableColumnSetup, TableColumnStateFlags,
    TableColumnWidth, TableFlags, TableHoveredColumn, TableHoveredRow, TableOptions, TableRowFlags,
    TableRowIndex, TableSortSpecs, TableToken, assert_current_table, assert_current_table_cell,
    assert_current_table_has_flags, assert_current_table_row, assert_non_negative_finite_f32,
    assert_table_column_width_phase, assert_table_setup_phase, assert_valid_table_column,
    assert_valid_table_column_raw_in, current_table_if_any, optional_user_id_raw,
    resolve_table_column, table_column_count_to_i32, table_freeze_count_to_i32,
};
use crate::{Id, sys};
use std::borrow::Cow;
use std::ffi::CStr;

/// # Table Widgets
impl Ui {
    /// Start a Table builder for ergonomic setup + headers + options.
    ///
    /// Example
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # fn demo(ui: &Ui) {
    /// ui.table("perf")
    ///     .flags(TableFlags::RESIZABLE | TableFlags::SORTABLE)
    ///     .outer_size([600.0, 240.0])
    ///     .freeze(1, 1)
    ///     .column("Name").width(140.0).done()
    ///     .column("Value").weight(1.0).done()
    ///     .headers(true)
    ///     .build(|ui| {
    ///         ui.table_next_row();
    ///         ui.table_next_column(); ui.text("CPU");
    ///         ui.table_next_column(); ui.text("Intel");
    ///     });
    /// # }
    /// ```
    pub fn table<'ui>(&'ui self, str_id: impl Into<Cow<'ui, str>>) -> TableBuilder<'ui> {
        TableBuilder::new(self, str_id)
    }
    /// Begins a table with no flags and with standard sizing constraints.
    ///
    /// This does no work on styling the headers (the top row) -- see either
    /// [begin_table_header](Self::begin_table_header) or the more complex
    /// [table_setup_column](Self::table_setup_column).
    #[must_use = "if return is dropped immediately, table is ended immediately."]
    pub fn begin_table(
        &self,
        str_id: impl AsRef<str>,
        column_count: usize,
    ) -> Option<TableToken<'_>> {
        self.begin_table_with_flags(str_id, column_count, TableFlags::NONE)
    }

    /// Begins a table with flags and with standard sizing constraints.
    #[must_use = "if return is dropped immediately, table is ended immediately."]
    pub fn begin_table_with_flags(
        &self,
        str_id: impl AsRef<str>,
        column_count: usize,
        flags: impl Into<TableOptions>,
    ) -> Option<TableToken<'_>> {
        self.begin_table_with_sizing(str_id, column_count, flags, [0.0, 0.0], 0.0)
    }

    /// Begins a table with all flags and sizing constraints. This is the base method,
    /// and gives users the most flexibility.
    #[must_use = "if return is dropped immediately, table is ended immediately."]
    pub fn begin_table_with_sizing(
        &self,
        str_id: impl AsRef<str>,
        column_count: usize,
        flags: impl Into<TableOptions>,
        outer_size: impl Into<[f32; 2]>,
        inner_width: f32,
    ) -> Option<TableToken<'_>> {
        let options = flags.into();
        options.validate("Ui::begin_table_with_sizing()");
        assert!(
            inner_width.is_finite(),
            "Ui::begin_table_with_sizing() inner_width must be finite"
        );
        assert!(
            !options.flags.contains(TableFlags::SCROLL_X) || inner_width >= 0.0,
            "Ui::begin_table_with_sizing() inner_width must be non-negative when SCROLL_X is enabled"
        );
        let outer_size = outer_size.into();
        assert!(
            outer_size[0].is_finite() && outer_size[1].is_finite(),
            "Ui::begin_table_with_sizing() outer_size must contain finite values"
        );
        let str_id_ptr = self.scratch_txt(str_id);
        let outer_size_vec: sys::ImVec2 = outer_size.into();
        let column_count = table_column_count_to_i32(column_count);

        let should_render = unsafe {
            sys::igBeginTable(
                str_id_ptr,
                column_count,
                options.raw(),
                outer_size_vec,
                inner_width,
            )
        };

        if should_render {
            Some(TableToken::new(self))
        } else {
            None
        }
    }

    /// Begins a table with no flags and with standard sizing constraints.
    ///
    /// Takes an array of table header information, the length of which determines
    /// how many columns will be created.
    #[must_use = "if return is dropped immediately, table is ended immediately."]
    pub fn begin_table_header<Name: AsRef<str>, const N: usize>(
        &self,
        str_id: impl AsRef<str>,
        column_data: [TableColumnSetup<Name>; N],
    ) -> Option<TableToken<'_>> {
        self.begin_table_header_with_flags(str_id, column_data, TableFlags::NONE)
    }

    /// Begins a table with flags and with standard sizing constraints.
    ///
    /// Takes an array of table header information, the length of which determines
    /// how many columns will be created.
    #[must_use = "if return is dropped immediately, table is ended immediately."]
    pub fn begin_table_header_with_flags<Name: AsRef<str>, const N: usize>(
        &self,
        str_id: impl AsRef<str>,
        column_data: [TableColumnSetup<Name>; N],
        flags: impl Into<TableOptions>,
    ) -> Option<TableToken<'_>> {
        if let Some(token) = self.begin_table_with_flags(str_id, N, flags) {
            // Setup columns
            for column in &column_data {
                self.table_setup_column_with_indent(
                    &column.name,
                    column.flags,
                    column.width,
                    column.indent,
                    column.user_id,
                );
            }
            self.table_headers_row();
            Some(token)
        } else {
            None
        }
    }

    /// Setup a column for the current table
    pub fn table_setup_column(
        &self,
        label: impl AsRef<str>,
        flags: TableColumnFlags,
        width: Option<TableColumnWidth>,
        user_id: Option<Id>,
    ) {
        self.table_setup_column_with_indent(label, flags, width, None, user_id);
    }

    /// Setup a column for the current table, including explicit indent policy.
    pub fn table_setup_column_with_indent(
        &self,
        label: impl AsRef<str>,
        flags: TableColumnFlags,
        width: Option<TableColumnWidth>,
        indent: Option<TableColumnIndent>,
        user_id: Option<Id>,
    ) {
        let table = assert_current_table("Ui::table_setup_column_with_indent()");
        assert!(
            unsafe { i32::from((*table).DeclColumnsCount) < (*table).ColumnsCount },
            "Ui::table_setup_column_with_indent() called more times than the table column count"
        );
        assert_table_setup_phase("Ui::table_setup_column_with_indent()");
        flags.validate_for_setup("Ui::table_setup_column_with_indent()", width, indent);
        let init_width_or_weight = width.map_or(0.0, TableColumnWidth::value);
        assert!(
            init_width_or_weight.is_finite(),
            "Ui::table_setup_column_with_indent() width or weight must be finite"
        );
        let label_ptr = self.scratch_txt(label);
        let raw_flags = flags.bits()
            | width.map_or(0, TableColumnWidth::raw_flags)
            | indent.map_or(0, TableColumnIndent::raw_flags);
        let user_id = optional_user_id_raw(user_id, "Ui::table_setup_column_with_indent()");
        unsafe {
            sys::igTableSetupColumn(label_ptr, raw_flags, init_width_or_weight, user_id);
        }
    }

    /// Setup a column with a fixed initial width.
    pub fn table_setup_column_fixed_width(
        &self,
        label: impl AsRef<str>,
        flags: TableColumnFlags,
        width: f32,
        user_id: Option<Id>,
    ) {
        self.table_setup_column(label, flags, Some(TableColumnWidth::Fixed(width)), user_id);
    }

    /// Setup a column with a stretch weight.
    pub fn table_setup_column_stretch_weight(
        &self,
        label: impl AsRef<str>,
        flags: TableColumnFlags,
        weight: f32,
        user_id: Option<Id>,
    ) {
        self.table_setup_column(
            label,
            flags,
            Some(TableColumnWidth::Stretch(weight)),
            user_id,
        );
    }

    /// Submit all headers cells based on data provided to TableSetupColumn() + submit context menu
    pub fn table_headers_row(&self) {
        assert_current_table("Ui::table_headers_row()");
        unsafe {
            sys::igTableHeadersRow();
        }
    }

    /// Append into the next column (or first column of next row if currently in last column)
    pub fn table_next_column(&self) -> bool {
        unsafe { sys::igTableNextColumn() }
    }

    /// Append into the specified column
    pub fn table_set_column_index(&self, column: impl Into<TableColumnIndex>) -> bool {
        let column = column.into();
        let column_n = column.into_i32("Ui::table_set_column_index()");
        if let Some(table) = current_table_if_any() {
            assert_valid_table_column_raw_in(table, column_n, "Ui::table_set_column_index()");
        }
        unsafe { sys::igTableSetColumnIndex(column_n) }
    }

    /// Append into the next row
    pub fn table_next_row(&self) {
        self.table_next_row_with_flags(TableRowFlags::NONE, 0.0);
    }

    /// Append into the next row with flags and minimum height
    pub fn table_next_row_with_flags(&self, flags: TableRowFlags, min_row_height: f32) {
        unsafe {
            sys::igTableNextRow(flags.bits(), min_row_height);
        }
    }

    /// Freeze columns/rows so they stay visible when scrolling.
    #[doc(alias = "TableSetupScrollFreeze")]
    pub fn table_setup_scroll_freeze(&self, frozen_cols: usize, frozen_rows: usize) {
        assert_table_setup_phase("Ui::table_setup_scroll_freeze()");
        let frozen_cols = table_freeze_count_to_i32(
            "Ui::table_setup_scroll_freeze()",
            "frozen_cols",
            frozen_cols,
            TABLE_MAX_COLUMNS,
        );
        let frozen_rows = table_freeze_count_to_i32(
            "Ui::table_setup_scroll_freeze()",
            "frozen_rows",
            frozen_rows,
            128,
        );
        unsafe { sys::igTableSetupScrollFreeze(frozen_cols, frozen_rows) }
    }

    /// Submit one header cell at current column position.
    #[doc(alias = "TableHeader")]
    pub fn table_header(&self, label: impl AsRef<str>) {
        assert_current_table_cell("Ui::table_header()");
        let label_ptr = self.scratch_txt(label);
        unsafe { sys::igTableHeader(label_ptr) }
    }

    /// Return columns count.
    #[doc(alias = "TableGetColumnCount")]
    pub fn table_get_column_count(&self) -> usize {
        usize::try_from(unsafe { sys::igTableGetColumnCount() })
            .expect("Dear ImGui returned a negative table column count")
    }

    /// Return current column index, or `None` when no table cell is current.
    #[doc(alias = "TableGetColumnIndex")]
    pub fn table_get_column_index(&self) -> Option<TableColumnIndex> {
        current_table_if_any()?;
        let raw = unsafe { sys::igTableGetColumnIndex() };
        (raw >= 0).then(|| TableColumnIndex::from_i32(raw, "Ui::table_get_column_index()"))
    }

    /// Return current row index, or `None` when no table row is current.
    #[doc(alias = "TableGetRowIndex")]
    pub fn table_get_row_index(&self) -> Option<TableRowIndex> {
        current_table_if_any()?;
        let raw = unsafe { sys::igTableGetRowIndex() };
        (raw >= 0).then(|| TableRowIndex::from_i32(raw, "Ui::table_get_row_index()"))
    }

    /// Return the name of a column by index.
    #[doc(alias = "TableGetColumnName")]
    pub fn table_get_column_name(&self, column: impl Into<TableColumnRef>) -> &str {
        let column = column.into();
        let column_n = match column {
            TableColumnRef::Current => -1,
            TableColumnRef::Index(index) => index.into_i32("Ui::table_get_column_name()"),
        };
        if current_table_if_any().is_some() {
            resolve_table_column(column, "Ui::table_get_column_name()");
        }
        unsafe {
            let ptr = sys::igTableGetColumnName_Int(column_n);
            if ptr.is_null() {
                ""
            } else {
                CStr::from_ptr(ptr).to_str().unwrap_or("")
            }
        }
    }

    /// Return the flags of a column by index.
    #[doc(alias = "TableGetColumnFlags")]
    pub fn table_get_column_flags(
        &self,
        column: impl Into<TableColumnRef>,
    ) -> TableColumnStateFlags {
        let column = column.into();
        let column_n = match column {
            TableColumnRef::Current => -1,
            TableColumnRef::Index(index) => index.into_i32("Ui::table_get_column_flags()"),
        };
        if let Some(table) = current_table_if_any() {
            let column_count = unsafe { (*table).ColumnsCount };
            let resolved_column = match column {
                TableColumnRef::Current => unsafe { (*table).CurrentColumn },
                TableColumnRef::Index(_) => column_n,
            };
            assert!(
                (0..column_count).contains(&resolved_column),
                "Ui::table_get_column_flags() column index {resolved_column} is outside the current table column range 0..{column_count}"
            );
        }
        unsafe { TableColumnStateFlags::from_bits_retain(sys::igTableGetColumnFlags(column_n)) }
    }

    /// Enable/disable a column by index.
    #[doc(alias = "TableSetColumnEnabled")]
    pub fn table_set_column_enabled(&self, column: impl Into<TableColumnRef>, enabled: bool) {
        assert_current_table_has_flags(TableFlags::HIDEABLE, "Ui::table_set_column_enabled()");
        let column = column.into();
        let column_n = match column {
            TableColumnRef::Current => -1,
            TableColumnRef::Index(index) => index.into_i32("Ui::table_set_column_enabled()"),
        };
        resolve_table_column(column, "Ui::table_set_column_enabled()");
        unsafe { sys::igTableSetColumnEnabled(column_n, enabled) }
    }

    /// Return hovered column index, or -1 when none.
    #[doc(alias = "TableGetHoveredColumn")]
    pub fn table_get_hovered_column(&self) -> TableHoveredColumn {
        let raw = unsafe { sys::igTableGetHoveredColumn() };
        if raw < 0 {
            return TableHoveredColumn::None;
        }
        if let Some(table) = current_table_if_any() {
            let column_count = unsafe { (*table).ColumnsCount };
            if raw == column_count {
                return TableHoveredColumn::UnusedSpace;
            }
        }
        TableHoveredColumn::Column(TableColumnIndex::from_i32(
            raw,
            "Ui::table_get_hovered_column()",
        ))
    }

    /// Set column width (for fixed-width columns).
    #[doc(alias = "TableSetColumnWidth")]
    pub fn table_set_column_width(&self, column: impl Into<TableColumnIndex>, width: f32) {
        assert_table_column_width_phase("Ui::table_set_column_width()");
        let column_n = assert_valid_table_column(column.into(), "Ui::table_set_column_width()");
        assert_non_negative_finite_f32("Ui::table_set_column_width()", "width", width);
        unsafe { sys::igTableSetColumnWidth(column_n, width) }
    }

    /// Set a table background color target.
    ///
    /// Color must be an ImGui-packed ImU32 in ABGR order (IM_COL32).
    /// Use `crate::colors::Color::to_imgui_u32()` to convert RGBA floats.
    #[doc(alias = "TableSetBgColor")]
    pub fn table_set_cell_bg_color_u32(&self, color: u32, column: impl Into<TableColumnRef>) {
        let column = column.into();
        assert_current_table_row("Ui::table_set_cell_bg_color_u32()");
        let column_n = match column {
            TableColumnRef::Current => -1,
            TableColumnRef::Index(index) => index.into_i32("Ui::table_set_cell_bg_color_u32()"),
        };
        resolve_table_column(column, "Ui::table_set_cell_bg_color_u32()");
        unsafe { sys::igTableSetBgColor(TableBgTarget::CellBg as i32, color, column_n) }
    }

    /// Set a table cell background color using RGBA color (0..=1 floats).
    pub fn table_set_cell_bg_color(&self, rgba: [f32; 4], column: impl Into<TableColumnRef>) {
        let col = crate::colors::Color::from_array(rgba).to_imgui_u32();
        self.table_set_cell_bg_color_u32(col, column);
    }

    /// Set the first row background color for the current table row.
    #[doc(alias = "TableSetBgColor")]
    pub fn table_set_row_bg0_color_u32(&self, color: u32) {
        assert_current_table_row("Ui::table_set_row_bg0_color_u32()");
        unsafe { sys::igTableSetBgColor(TableBgTarget::RowBg0 as i32, color, -1) }
    }

    /// Set the first row background color using RGBA color (0..=1 floats).
    pub fn table_set_row_bg0_color(&self, rgba: [f32; 4]) {
        let col = crate::colors::Color::from_array(rgba).to_imgui_u32();
        self.table_set_row_bg0_color_u32(col);
    }

    /// Set the second row background color for the current table row.
    #[doc(alias = "TableSetBgColor")]
    pub fn table_set_row_bg1_color_u32(&self, color: u32) {
        assert_current_table_row("Ui::table_set_row_bg1_color_u32()");
        unsafe { sys::igTableSetBgColor(TableBgTarget::RowBg1 as i32, color, -1) }
    }

    /// Set the second row background color using RGBA color (0..=1 floats).
    pub fn table_set_row_bg1_color(&self, rgba: [f32; 4]) {
        let col = crate::colors::Color::from_array(rgba).to_imgui_u32();
        self.table_set_row_bg1_color_u32(col);
    }

    /// Return hovered row from the previous frame.
    #[doc(alias = "TableGetHoveredRow")]
    pub fn table_get_hovered_row(&self) -> TableHoveredRow {
        if current_table_if_any().is_none() {
            return TableHoveredRow::None;
        }
        let raw = unsafe { sys::igTableGetHoveredRow() };
        if raw < 0 {
            return TableHoveredRow::None;
        }
        TableHoveredRow::Row(TableRowIndex::from_i32(raw, "Ui::table_get_hovered_row()"))
    }

    /// Header row height in pixels.
    #[doc(alias = "TableGetHeaderRowHeight")]
    pub fn table_get_header_row_height(&self) -> f32 {
        unsafe { sys::igTableGetHeaderRowHeight() }
    }

    /// Set sort direction for a column. Optionally append to existing sort specs (multi-sort).
    #[doc(alias = "TableSetColumnSortDirection")]
    pub fn table_set_column_sort_direction(
        &self,
        column: impl Into<TableColumnIndex>,
        dir: SortDirection,
        append_to_sort_specs: bool,
    ) {
        let column_n =
            assert_valid_table_column(column.into(), "Ui::table_set_column_sort_direction()");
        unsafe { sys::igTableSetColumnSortDirection(column_n, dir.into(), append_to_sort_specs) }
    }

    /// Get current table sort specifications, if any.
    /// When non-None and `is_dirty()` is true, the application should sort its data and
    /// then call `clear_dirty()`.
    #[doc(alias = "TableGetSortSpecs")]
    pub fn table_get_sort_specs(&self) -> Option<TableSortSpecs<'_>> {
        unsafe {
            let ptr = sys::igTableGetSortSpecs();
            if ptr.is_null() {
                None
            } else {
                Some(TableSortSpecs::from_raw(ptr))
            }
        }
    }
}

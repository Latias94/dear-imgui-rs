//! Tables
//!
//! Modern multi-column layout and data table API with rich configuration.
//! Use `TableBuilder` to declare columns and build rows/cells ergonomically.
//!
//! Quick example (builder):
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! ui.table("perf")
//!     .flags(TableFlags::RESIZABLE)
//!     .column("Name").width(120.0).done()
//!     .column("Value").weight(1.0).done()
//!     .headers(true)
//!     .build(|ui| {
//!         ui.table_next_row();
//!         ui.table_next_column(); ui.text("CPU");
//!         ui.table_next_column(); ui.text("Intel");
//!     });
//! ```
//!
//! Table draw channels are intentionally closure-only because the upstream operations are
//! internal, cell-sensitive APIs. The former movable token types are unavailable:
//!
//! ```compile_fail
//! use dear_imgui_rs::TableBackgroundChannelToken;
//! ```
//!
//! ```compile_fail
//! use dear_imgui_rs::TableColumnChannelToken;
//! ```
//!
//! Quick example (manual API):
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! if let Some(_t) = ui.begin_table("t", 2) {
//!     ui.table_next_row();
//!     ui.table_next_column(); ui.text("A");
//!     ui.table_next_column(); ui.text("B");
//! }
//! ```
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]

use crate::draw::ImColor32;
use crate::internal::len_i32;
use crate::scope::{NativeScopePop, NativeScopeToken};
use crate::{Ui, sys};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
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
    ///
    /// # Panics
    ///
    /// Panics if `column_count` is zero or reaches Dear ImGui's column limit.
    #[must_use = "if return is dropped immediately, table is ended immediately."]
    #[doc(alias = "BeginTable")]
    pub fn begin_table(
        &self,
        str_id: impl AsRef<str>,
        column_count: usize,
    ) -> Option<TableToken<'_>> {
        self.begin_table_with_flags(str_id, column_count, TableFlags::NONE)
    }

    /// Begins a table with flags and with standard sizing constraints.
    ///
    /// # Panics
    ///
    /// Panics for an invalid column count or incompatible table options.
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
    ///
    /// # Panics
    ///
    /// Panics for an invalid column count or table option, non-finite sizing, a negative
    /// `inner_width` when horizontal scrolling is enabled, or an active table draw-channel scope
    /// on the current table cell.
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
        let outer_size_vec: sys::ImVec2 = outer_size.into();
        let column_count = table_column_count_to_i32(column_count);

        let should_render = self.with_scratch_txt(str_id, |str_id_ptr| {
            self.assert_can_begin_table();
            unsafe {
                sys::igBeginTable(
                    str_id_ptr,
                    column_count,
                    options.raw(),
                    outer_size_vec,
                    inner_width,
                )
            }
        });

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
    ///
    /// # Panics
    ///
    /// Panics if the array is empty, reaches Dear ImGui's column limit, or contains invalid column
    /// setup data.
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
    ///
    /// # Panics
    ///
    /// Panics for an invalid column count, table option, or column setup value.
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
                self.table_setup_column_with_indent_and_user_data(
                    &column.name,
                    column.flags,
                    column.width,
                    column.indent,
                    column.user_data,
                );
            }
            self.table_headers_row();
            Some(token)
        } else {
            None
        }
    }

    /// Setup a column for the current table.
    ///
    /// # Panics
    ///
    /// Panics outside a table, after table layout has started, after all declared columns have
    /// already been configured, or for an invalid flag/width combination or non-finite width.
    #[doc(alias = "TableSetupColumn")]
    pub fn table_setup_column(
        &self,
        label: impl AsRef<str>,
        flags: TableColumnFlags,
        width: Option<TableColumnWidth>,
    ) {
        self.table_setup_column_with_indent(label, flags, width, None);
    }

    /// Setup a column for the current table with opaque application data.
    ///
    /// # Panics
    ///
    /// Has the same validation and phase requirements as [`Ui::table_setup_column`].
    pub fn table_setup_column_with_user_data(
        &self,
        label: impl AsRef<str>,
        flags: TableColumnFlags,
        width: Option<TableColumnWidth>,
        user_data: impl Into<TableColumnUserData>,
    ) {
        self.table_setup_column_with_indent_and_user_data(label, flags, width, None, user_data);
    }

    /// Setup a column for the current table, including explicit indent policy.
    ///
    /// # Panics
    ///
    /// Has the same validation and phase requirements as [`Ui::table_setup_column`].
    pub fn table_setup_column_with_indent(
        &self,
        label: impl AsRef<str>,
        flags: TableColumnFlags,
        width: Option<TableColumnWidth>,
        indent: Option<TableColumnIndent>,
    ) {
        self.table_setup_column_with_indent_and_user_data(label, flags, width, indent, 0);
    }

    /// Setup a column with explicit indent policy and opaque application data.
    ///
    /// # Panics
    ///
    /// Panics outside a table, after table layout has started, after all declared columns have
    /// already been configured, or for invalid flags, indent/width combinations, or non-finite
    /// width/weight values.
    pub fn table_setup_column_with_indent_and_user_data(
        &self,
        label: impl AsRef<str>,
        flags: TableColumnFlags,
        width: Option<TableColumnWidth>,
        indent: Option<TableColumnIndent>,
        user_data: impl Into<TableColumnUserData>,
    ) {
        flags.validate_for_setup(
            "Ui::table_setup_column_with_indent_and_user_data()",
            width,
            indent,
        );
        let init_width_or_weight = width.map_or(0.0, TableColumnWidth::value);
        assert!(
            init_width_or_weight.is_finite(),
            "Ui::table_setup_column_with_indent_and_user_data() width or weight must be finite"
        );
        let raw_flags = flags.bits()
            | width.map_or(0, TableColumnWidth::raw_flags)
            | indent.map_or(0, TableColumnIndent::raw_flags);
        let user_data = user_data.into().get();
        self.with_scratch_txt(label, |label_ptr| {
            let table = assert_current_table("Ui::table_setup_column_with_indent_and_user_data()");
            assert!(
                unsafe { i32::from((*table).DeclColumnsCount) < (*table).ColumnsCount },
                "Ui::table_setup_column_with_indent_and_user_data() called more times than the table column count"
            );
            assert_table_setup_phase("Ui::table_setup_column_with_indent_and_user_data()");
            unsafe {
                sys::igTableSetupColumn(label_ptr, raw_flags, init_width_or_weight, user_data);
            }
        });
    }

    /// Setup a column with a fixed initial width.
    ///
    /// # Panics
    ///
    /// Has the same validation and phase requirements as [`Ui::table_setup_column`].
    pub fn table_setup_column_fixed_width(
        &self,
        label: impl AsRef<str>,
        flags: TableColumnFlags,
        width: f32,
    ) {
        self.table_setup_column(label, flags, Some(TableColumnWidth::Fixed(width)));
    }

    /// Setup a column with a stretch weight.
    ///
    /// # Panics
    ///
    /// Has the same validation and phase requirements as [`Ui::table_setup_column`].
    pub fn table_setup_column_stretch_weight(
        &self,
        label: impl AsRef<str>,
        flags: TableColumnFlags,
        weight: f32,
    ) {
        self.table_setup_column(label, flags, Some(TableColumnWidth::Stretch(weight)));
    }

    /// Submit all header cells based on data provided to `TableSetupColumn()` and submit the
    /// context-menu target.
    ///
    /// # Panics
    ///
    /// Panics outside a table, while a table draw-channel scope is active, or while this cell
    /// hosts a native extension.
    #[doc(alias = "TableHeadersRow")]
    pub fn table_headers_row(&self) {
        self.run_with_bound_context(|| {
            assert_current_table("Ui::table_headers_row()");
            self.assert_table_cell_can_change("Ui::table_headers_row()");
            unsafe {
                sys::igTableHeadersRow();
            }
        });
    }

    /// Append into the next column, or the first column of the next row when currently in the last
    /// column.
    ///
    /// Returns `false` when no table is current.
    ///
    /// # Panics
    ///
    /// Panics while a table draw-channel scope is active or this cell hosts a native extension.
    #[doc(alias = "TableNextColumn")]
    pub fn table_next_column(&self) -> bool {
        self.run_with_bound_context(|| {
            self.assert_table_cell_can_change("Ui::table_next_column()");
            unsafe { sys::igTableNextColumn() }
        })
    }

    /// Append into the specified column.
    ///
    /// Returns `false` when no table is current.
    ///
    /// # Panics
    ///
    /// Panics if `column` is outside the current table, a table draw-channel scope is active,
    /// or this cell hosts a native extension.
    #[doc(alias = "TableSetColumnIndex")]
    pub fn table_set_column_index(&self, column: impl Into<TableColumnIndex>) -> bool {
        let column = column.into();
        let column_n = column.into_i32("Ui::table_set_column_index()");
        self.run_with_bound_context(|| {
            self.assert_table_cell_can_change("Ui::table_set_column_index()");
            if let Some(table) = current_table_if_any() {
                assert_valid_table_column_raw_in(table, column_n, "Ui::table_set_column_index()");
            }
            unsafe { sys::igTableSetColumnIndex(column_n) }
        })
    }

    /// Append into the next row.
    ///
    /// # Panics
    ///
    /// Panics outside a table, while a table draw-channel scope is active, or while this cell
    /// hosts a native extension.
    #[doc(alias = "TableNextRow")]
    pub fn table_next_row(&self) {
        self.table_next_row_with_flags(TableRowFlags::NONE, 0.0);
    }

    /// Append into the next row with flags and minimum height.
    ///
    /// # Panics
    ///
    /// Panics outside a table, while a table draw-channel scope is active, while this cell
    /// hosts a native extension, or when `min_row_height` is negative or non-finite.
    pub fn table_next_row_with_flags(&self, flags: TableRowFlags, min_row_height: f32) {
        assert_non_negative_finite_f32(
            "Ui::table_next_row_with_flags()",
            "min_row_height",
            min_row_height,
        );
        self.run_with_bound_context(|| {
            assert_current_table("Ui::table_next_row_with_flags()");
            self.assert_table_cell_can_change("Ui::table_next_row_with_flags()");
            unsafe { sys::igTableNextRow(flags.bits(), min_row_height) };
        });
    }

    /// Freeze columns/rows so they stay visible when scrolling.
    ///
    /// # Panics
    ///
    /// Panics outside a table, after the table setup phase, or when either freeze count exceeds
    /// Dear ImGui's supported range.
    #[doc(alias = "TableSetupScrollFreeze")]
    pub fn table_setup_scroll_freeze(&self, frozen_cols: usize, frozen_rows: usize) {
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
        self.run_with_bound_context(|| {
            assert_table_setup_phase("Ui::table_setup_scroll_freeze()");
            unsafe { sys::igTableSetupScrollFreeze(frozen_cols, frozen_rows) }
        });
    }

    /// Submit one header cell at the current column position.
    ///
    /// # Panics
    ///
    /// Panics unless a table cell is current.
    #[doc(alias = "TableHeader")]
    pub fn table_header(&self, label: impl AsRef<str>) {
        self.with_scratch_txt(label, |label_ptr| {
            assert_current_table_cell("Ui::table_header()");
            unsafe { sys::igTableHeader(label_ptr) }
        });
    }

    /// Return the current table's column count, or zero when no table is current.
    #[doc(alias = "TableGetColumnCount")]
    pub fn table_get_column_count(&self) -> usize {
        usize::try_from(self.run_with_bound_context(|| unsafe { sys::igTableGetColumnCount() }))
            .expect("Dear ImGui returned a negative table column count")
    }

    /// Return current column index, or `None` when no table cell is current.
    #[doc(alias = "TableGetColumnIndex")]
    pub fn table_get_column_index(&self) -> Option<TableColumnIndex> {
        self.run_with_bound_context(|| {
            current_table_if_any()?;
            let raw = unsafe { sys::igTableGetColumnIndex() };
            (raw >= 0).then(|| TableColumnIndex::from_i32(raw, "Ui::table_get_column_index()"))
        })
    }

    /// Return current row index, or `None` when no table row is current.
    #[doc(alias = "TableGetRowIndex")]
    pub fn table_get_row_index(&self) -> Option<TableRowIndex> {
        self.run_with_bound_context(|| {
            current_table_if_any()?;
            let raw = unsafe { sys::igTableGetRowIndex() };
            (raw >= 0).then(|| TableRowIndex::from_i32(raw, "Ui::table_get_row_index()"))
        })
    }

    /// Return an owned copy of a column's name. It remains valid after further column setup
    /// and after the table or frame ends.
    ///
    /// Returns an empty string when no table is current.
    ///
    /// # Panics
    ///
    /// Panics when a table is current and the requested/current column is invalid.
    #[doc(alias = "TableGetColumnName")]
    pub fn table_get_column_name(&self, column: impl Into<TableColumnRef>) -> String {
        let column = column.into();
        self.run_with_bound_context(|| {
            let Some(table) = current_table_if_any() else {
                return String::new();
            };
            let column_n = resolve_table_column(table, column, "Ui::table_get_column_name()");
            unsafe {
                let ptr = sys::igTableGetColumnName_Int(column_n);
                if ptr.is_null() {
                    String::new()
                } else {
                    CStr::from_ptr(ptr).to_str().unwrap_or("").to_owned()
                }
            }
        })
    }

    /// Return the public flags of a column by index, omitting native implementation bits.
    ///
    /// Returns empty flags when no table is current.
    ///
    /// # Panics
    ///
    /// Panics when a table is current and the requested/current column is invalid.
    #[doc(alias = "TableGetColumnFlags")]
    pub fn table_get_column_flags(
        &self,
        column: impl Into<TableColumnRef>,
    ) -> TableColumnStateFlags {
        let column = column.into();
        self.run_with_bound_context(|| {
            let Some(table) = current_table_if_any() else {
                return TableColumnStateFlags::empty();
            };
            let column_n = resolve_table_column(table, column, "Ui::table_get_column_flags()");
            unsafe {
                TableColumnStateFlags::from_bits_truncate(sys::igTableGetColumnFlags(column_n))
            }
        })
    }

    /// Enable or disable a column by index.
    ///
    /// # Panics
    ///
    /// Panics outside a table, when the table lacks [`TableFlags::HIDEABLE`], or when the
    /// requested/current column is invalid.
    #[doc(alias = "TableSetColumnEnabled")]
    pub fn table_set_column_enabled(&self, column: impl Into<TableColumnRef>, enabled: bool) {
        let column = column.into();
        self.run_with_bound_context(|| {
            let table = assert_current_table_has_flags(
                TableFlags::HIDEABLE,
                "Ui::table_set_column_enabled()",
            );
            let column_n = resolve_table_column(table, column, "Ui::table_set_column_enabled()");
            unsafe { sys::igTableSetColumnEnabled(column_n, enabled) }
        });
    }

    /// Return the hovered column, unused table space, or [`TableHoveredColumn::None`] when no
    /// table column is hovered.
    #[doc(alias = "TableGetHoveredColumn")]
    pub fn table_get_hovered_column(&self) -> TableHoveredColumn {
        self.run_with_bound_context(|| {
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
        })
    }

    /// Set column width for a fixed-width column.
    ///
    /// # Panics
    ///
    /// Panics outside a table, after table layout is locked, before layout metrics are available,
    /// for an invalid column, or when `width` is negative or non-finite.
    #[doc(alias = "TableSetColumnWidth")]
    pub fn table_set_column_width(&self, column: impl Into<TableColumnIndex>, width: f32) {
        assert_non_negative_finite_f32("Ui::table_set_column_width()", "width", width);
        let column = column.into();
        self.run_with_bound_context(|| {
            assert_table_column_width_phase("Ui::table_set_column_width()");
            let column_n = assert_valid_table_column(column, "Ui::table_set_column_width()");
            unsafe { sys::igTableSetColumnWidth(column_n, width) }
        });
    }

    /// Set a table background color target.
    ///
    /// Color must be an ImGui-packed ImU32 in ABGR order (IM_COL32).
    /// Use `crate::colors::Color::to_imgui_u32()` to convert RGBA floats.
    ///
    /// # Panics
    ///
    /// Panics unless a table row is current or when the requested/current column is invalid.
    #[doc(alias = "TableSetBgColor")]
    pub fn table_set_cell_bg_color_u32(&self, color: u32, column: impl Into<TableColumnRef>) {
        let column = column.into();
        self.run_with_bound_context(|| {
            let table = assert_current_table_row("Ui::table_set_cell_bg_color_u32()");
            let column_n = resolve_table_column(table, column, "Ui::table_set_cell_bg_color_u32()");
            unsafe { sys::igTableSetBgColor(TableBgTarget::CellBg as i32, color, column_n) }
        });
    }

    /// Set a table cell background color using RGBA color (0..=1 floats).
    ///
    /// # Panics
    ///
    /// Has the same phase and column requirements as [`Ui::table_set_cell_bg_color_u32`].
    pub fn table_set_cell_bg_color(&self, rgba: [f32; 4], column: impl Into<TableColumnRef>) {
        let col = crate::colors::Color::from_array(rgba).to_imgui_u32();
        self.table_set_cell_bg_color_u32(col, column);
    }

    /// Set the first row background color for the current table row.
    ///
    /// # Panics
    ///
    /// Panics unless a table row is current.
    #[doc(alias = "TableSetBgColor")]
    pub fn table_set_row_bg0_color_u32(&self, color: u32) {
        self.run_with_bound_context(|| {
            assert_current_table_row("Ui::table_set_row_bg0_color_u32()");
            unsafe { sys::igTableSetBgColor(TableBgTarget::RowBg0 as i32, color, -1) }
        });
    }

    /// Set the first row background color using RGBA color (0..=1 floats).
    ///
    /// # Panics
    ///
    /// Panics unless a table row is current.
    pub fn table_set_row_bg0_color(&self, rgba: [f32; 4]) {
        let col = crate::colors::Color::from_array(rgba).to_imgui_u32();
        self.table_set_row_bg0_color_u32(col);
    }

    /// Set the second row background color for the current table row.
    ///
    /// # Panics
    ///
    /// Panics unless a table row is current.
    #[doc(alias = "TableSetBgColor")]
    pub fn table_set_row_bg1_color_u32(&self, color: u32) {
        self.run_with_bound_context(|| {
            assert_current_table_row("Ui::table_set_row_bg1_color_u32()");
            unsafe { sys::igTableSetBgColor(TableBgTarget::RowBg1 as i32, color, -1) }
        });
    }

    /// Set the second row background color using RGBA color (0..=1 floats).
    ///
    /// # Panics
    ///
    /// Panics unless a table row is current.
    pub fn table_set_row_bg1_color(&self, rgba: [f32; 4]) {
        let col = crate::colors::Color::from_array(rgba).to_imgui_u32();
        self.table_set_row_bg1_color_u32(col);
    }

    /// Return hovered row from the previous frame.
    #[doc(alias = "TableGetHoveredRow")]
    pub fn table_get_hovered_row(&self) -> TableHoveredRow {
        self.run_with_bound_context(|| {
            if current_table_if_any().is_none() {
                return TableHoveredRow::None;
            }
            let raw = unsafe { sys::igTableGetHoveredRow() };
            if raw < 0 {
                return TableHoveredRow::None;
            }
            TableHoveredRow::Row(TableRowIndex::from_i32(raw, "Ui::table_get_hovered_row()"))
        })
    }

    /// Header row height in pixels.
    ///
    /// # Panics
    ///
    /// Panics outside a table.
    #[doc(alias = "TableGetHeaderRowHeight")]
    pub fn table_get_header_row_height(&self) -> f32 {
        self.run_with_bound_context(|| {
            assert_current_table("Ui::table_get_header_row_height()");
            unsafe { sys::igTableGetHeaderRowHeight() }
        })
    }

    /// Set sort direction for a column. Optionally append to existing sort specs (multi-sort).
    ///
    /// # Panics
    ///
    /// Panics outside a table, when the table lacks [`TableFlags::SORTABLE`], for an invalid
    /// column, or when [`SortDirection::None`] is used without [`TableFlags::SORT_TRISTATE`].
    #[doc(alias = "TableSetColumnSortDirection")]
    pub fn table_set_column_sort_direction(
        &self,
        column: impl Into<TableColumnIndex>,
        dir: SortDirection,
        append_to_sort_specs: bool,
    ) {
        let column = column.into();
        self.run_with_bound_context(|| {
            let table = assert_current_table("Ui::table_set_column_sort_direction()");
            let table_flags = TableFlags::from_bits_retain(unsafe { (*table).Flags });
            assert!(
                table_flags.contains(TableFlags::SORTABLE),
                "Ui::table_set_column_sort_direction() requires the current table to have SORTABLE"
            );
            if dir == SortDirection::None {
                assert!(
                    table_flags.contains(TableFlags::SORT_TRISTATE),
                    "Ui::table_set_column_sort_direction() requires SORT_TRISTATE for SortDirection::None"
                );
            }
            let column_n =
                assert_valid_table_column(column, "Ui::table_set_column_sort_direction()");
            unsafe {
                sys::igTableSetColumnSortDirection(column_n, dir.into(), append_to_sort_specs)
            }
        });
    }

    /// Get current table sort specifications, if any.
    /// When non-None and `is_dirty()` is true, the application should sort its data and
    /// then call [`TableSortSpecs::clear_dirty`] while this table is still current. Returns `None`
    /// outside a table or when the table has no sort specifications. On a sortable table, this may
    /// lock table layout, so finish all setup calls first.
    #[doc(alias = "TableGetSortSpecs")]
    pub fn table_get_sort_specs(&self) -> Option<TableSortSpecs> {
        self.run_with_bound_context(|| unsafe {
            let table = current_table_if_any()?;
            let ptr = sys::igTableGetSortSpecs();
            if ptr.is_null() {
                None
            } else {
                Some(TableSortSpecs::from_raw(self, table, ptr))
            }
        })
    }

    /// Maximum label width used for angled headers when enabled in style/options.
    ///
    /// # Panics
    ///
    /// Panics outside a table.
    #[doc(alias = "TableGetHeaderAngledMaxLabelWidth")]
    pub fn table_get_header_angled_max_label_width(&self) -> f32 {
        self.run_with_bound_context(|| {
            assert_current_table("Ui::table_get_header_angled_max_label_width()");
            unsafe { sys::igTableGetHeaderAngledMaxLabelWidth() }
        })
    }

    /// Submit an angled headers row (requires style/flags enabling angled headers).
    ///
    /// # Panics
    ///
    /// Panics outside a table, after the first row has started, or while a table draw-channel scope
    /// is active.
    #[doc(alias = "TableAngledHeadersRow")]
    pub fn table_angled_headers_row(&self) {
        self.run_with_bound_context(|| {
            assert_table_before_first_row("Ui::table_angled_headers_row()");
            self.assert_table_cell_can_change("Ui::table_angled_headers_row()");
            unsafe { sys::igTableAngledHeadersRow() };
        });
    }

    // Removed legacy TableAngledHeadersRowEx(flags) wrapper; use `table_angled_headers_row_ex_with_data`.

    /// Submit angled headers row with explicit data (Ex variant).
    ///
    /// - `row_id`: ImGuiID for the row. Use 0 for automatic if not needed.
    /// - `angle`: Angle in radians for headers.
    /// - `max_label_width`: Maximum label width for angled headers.
    /// - `headers`: Per-column header data.
    ///
    /// # Panics
    ///
    /// Panics outside a table, after the first row has started, while a table draw-channel scope is
    /// active, for a non-finite/out-of-range angle, for a negative/non-finite maximum width, for an
    /// invalid column, or when `headers` are not ordered left-to-right without duplicates.
    pub fn table_angled_headers_row_ex_with_data(
        &self,
        row_id: u32,
        angle: f32,
        max_label_width: f32,
        headers: &[TableHeaderData],
    ) {
        assert!(
            angle.is_finite(),
            "Ui::table_angled_headers_row_ex_with_data() angle must be finite"
        );
        assert!(
            (-std::f32::consts::FRAC_PI_2..std::f32::consts::FRAC_PI_2).contains(&angle),
            "Ui::table_angled_headers_row_ex_with_data() angle must be between -PI/2 and PI/2"
        );
        assert_non_negative_finite_f32(
            "Ui::table_angled_headers_row_ex_with_data()",
            "max_label_width",
            max_label_width,
        );
        if headers.is_empty() {
            self.table_angled_headers_row();
            return;
        }
        let count = len_i32(
            "Ui::table_angled_headers_row_ex_with_data()",
            "headers",
            headers.len(),
        );
        let mut data: Vec<sys::ImGuiTableHeaderData> = Vec::with_capacity(headers.len());
        self.run_with_bound_context(|| {
            let table =
                assert_table_before_first_row("Ui::table_angled_headers_row_ex_with_data()");
            self.assert_table_cell_can_change(
                "Ui::table_angled_headers_row_ex_with_data()",
            );
            let columns = unsafe { (*table).Columns.Data };
            assert!(
                !columns.is_null(),
                "Ui::table_angled_headers_row_ex_with_data() table columns are unavailable"
            );
            let mut previous_display_order = -1;
            for h in headers {
                let column_n = assert_valid_table_column_in(
                    table,
                    h.index,
                    "Ui::table_angled_headers_row_ex_with_data()",
                );
                let display_order = i32::from(unsafe {
                    (*columns.add(column_n as usize)).DisplayOrder
                });
                assert!(
                    display_order > previous_display_order,
                    "Ui::table_angled_headers_row_ex_with_data() headers must be ordered left to right without duplicates"
                );
                previous_display_order = display_order;
                data.push(sys::ImGuiTableHeaderData {
                    Index: h
                        .index
                        .into_imgui_column_idx("Ui::table_angled_headers_row_ex_with_data()"),
                    TextColor: u32::from(h.text_color),
                    BgColor0: u32::from(h.bg_color0),
                    BgColor1: u32::from(h.bg_color1),
                });
            }
            unsafe {
                sys::igTableAngledHeadersRowEx(
                    row_id,
                    angle,
                    max_label_width,
                    data.as_ptr(),
                    count,
                );
            }
        });
    }

    /// Run a closure while drawing into the current table's background channel.
    ///
    /// The channel cannot escape this closure. Row, column, nested-channel, and table-end
    /// transitions are rejected before FFI while it is active.
    ///
    /// # Panics
    ///
    /// Panics if there is no current table cell, another table channel is active, or this
    /// cell hosts a native extension.
    #[doc(
        alias = "TablePushBackgroundChannel",
        alias = "TablePopBackgroundChannel"
    )]
    pub fn with_table_background_channel<R>(&self, f: impl FnOnce() -> R) -> R {
        self.run_with_bound_context(|| {
            assert_current_table_cell("Ui::with_table_background_channel()");
            self.assert_table_cell_can_change("Ui::with_table_background_channel()");
            unsafe { sys::igTablePushBackgroundChannel() };
        });
        let guard = TableChannelGuard::background(self);
        let result = f();
        drop(guard);
        result
    }

    /// Run a closure while drawing into a selected table column channel.
    ///
    /// The channel cannot escape this closure. Row, column, nested-channel, and table-end
    /// transitions are rejected before FFI while it is active.
    ///
    /// # Panics
    ///
    /// Panics if there is no current table cell, `column` is invalid, another table channel is
    /// active, or this cell hosts a native extension.
    #[doc(alias = "TablePushColumnChannel", alias = "TablePopColumnChannel")]
    pub fn with_table_column_channel<R>(
        &self,
        column: impl Into<TableColumnIndex>,
        f: impl FnOnce() -> R,
    ) -> R {
        let column = column.into();
        self.run_with_bound_context(|| {
            assert_current_table_cell("Ui::with_table_column_channel()");
            self.assert_table_cell_can_change("Ui::with_table_column_channel()");
            let column_n = assert_valid_table_column(column, "Ui::with_table_column_channel()");
            unsafe { sys::igTablePushColumnChannel(column_n) };
        });
        let guard = TableChannelGuard::column(self);
        let result = f();
        drop(guard);
        result
    }

    /// Open the table context menu for the current/default column.
    ///
    /// # Panics
    ///
    /// Panics outside a table or when an explicit column is outside the current table.
    #[doc(alias = "TableOpenContextMenu")]
    pub fn table_open_context_menu(&self, target: impl Into<TableContextMenuTarget>) {
        let target = target.into();
        self.run_with_bound_context(|| {
            let table = assert_current_table("Ui::table_open_context_menu()");
            let column_n = match target {
                TableContextMenuTarget::CurrentColumn => -1,
                TableContextMenuTarget::Column(index) => {
                    assert_valid_table_column_in(table, index, "Ui::table_open_context_menu()")
                }
                TableContextMenuTarget::Table => unsafe { (*table).ColumnsCount },
            };
            unsafe { sys::igTableOpenContextMenu(column_n) }
        });
    }
}

/// Builder for ImGui tables with columns + headers + sizing/freeze options.
#[derive(Debug)]
pub struct TableBuilder<'ui> {
    ui: &'ui Ui,
    id: Cow<'ui, str>,
    flags: TableFlags,
    sizing_policy: Option<TableSizingPolicy>,
    outer_size: [f32; 2],
    inner_width: f32,
    columns: Vec<TableColumnSetup<Cow<'ui, str>>>,
    use_headers: bool,
    freeze: Option<(usize, usize)>,
}

impl<'ui> TableBuilder<'ui> {
    /// Create a new TableBuilder. Prefer using `Ui::table("id")`.
    pub fn new(ui: &'ui Ui, str_id: impl Into<Cow<'ui, str>>) -> Self {
        Self {
            ui,
            id: str_id.into(),
            flags: TableFlags::NONE,
            sizing_policy: None,
            outer_size: [0.0, 0.0],
            inner_width: 0.0,
            columns: Vec::new(),
            use_headers: false,
            freeze: None,
        }
    }

    /// Set table flags
    pub fn flags(mut self, flags: TableFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set the table sizing policy.
    pub fn sizing_policy(mut self, policy: TableSizingPolicy) -> Self {
        self.sizing_policy = Some(policy);
        self
    }

    /// Set outer size (width, height). Default `[0, 0]`.
    pub fn outer_size(mut self, size: [f32; 2]) -> Self {
        self.outer_size = size;
        self
    }

    /// Set inner width. Default 0.0
    pub fn inner_width(mut self, width: f32) -> Self {
        self.inner_width = width;
        self
    }

    /// Freeze columns/rows so they stay visible when scrolling
    pub fn freeze(mut self, frozen_cols: usize, frozen_rows: usize) -> Self {
        self.freeze = Some((frozen_cols, frozen_rows));
        self
    }

    /// Begin defining a column using a chainable ColumnBuilder.
    /// Call `.done()` to return to the TableBuilder.
    pub fn column(self, name: impl Into<Cow<'ui, str>>) -> ColumnBuilder<'ui> {
        ColumnBuilder::new(self, name)
    }

    /// Replace columns with provided list
    pub fn columns<Name: Into<Cow<'ui, str>>>(
        mut self,
        cols: impl IntoIterator<Item = TableColumnSetup<Name>>,
    ) -> Self {
        self.columns = cols
            .into_iter()
            .map(|column| column.map_name(Into::into))
            .collect();
        self
    }

    /// Add a single column setup
    pub fn add_column<Name: Into<Cow<'ui, str>>>(mut self, col: TableColumnSetup<Name>) -> Self {
        self.columns.push(col.map_name(Into::into));
        self
    }

    /// Auto submit headers row from `TableSetupColumn()` entries
    pub fn headers(mut self, enabled: bool) -> Self {
        self.use_headers = enabled;
        self
    }

    /// Build the table and run a closure to emit rows/cells.
    ///
    /// Configuration setters intentionally defer validation until this method so the builder stays
    /// cheap and composable. The table is ended during unwinding if validation or `f` panics.
    ///
    /// # Panics
    ///
    /// Panics for zero/excessive columns, incompatible table or column flags, non-finite sizing or
    /// width/weight values, invalid horizontal-scroll width, excessive freeze counts, an active
    /// table draw-channel scope, or any panic raised by `f`.
    pub fn build(self, f: impl FnOnce(&Ui)) {
        let mut options = TableOptions::from(self.flags);
        if let Some(policy) = self.sizing_policy {
            options = options.sizing_policy(policy);
        }
        let Some(token) = self.ui.begin_table_with_sizing(
            self.id.as_ref(),
            self.columns.len(),
            options,
            self.outer_size,
            self.inner_width,
        ) else {
            return;
        };

        if let Some((fc, fr)) = self.freeze {
            self.ui.table_setup_scroll_freeze(fc, fr);
        }

        if !self.columns.is_empty() {
            for col in &self.columns {
                self.ui.table_setup_column_with_indent_and_user_data(
                    col.name.as_ref(),
                    col.flags,
                    col.width,
                    col.indent,
                    col.user_data,
                );
            }
            if self.use_headers {
                self.ui.table_headers_row();
            }
        }

        f(self.ui);

        // drop token to end table
        token.end();
    }
}

/// Chainable builder for a single column. Use `.done()` to return to the table builder.
#[derive(Debug)]
pub struct ColumnBuilder<'ui> {
    parent: TableBuilder<'ui>,
    column: TableColumnSetup<Cow<'ui, str>>,
}

impl<'ui> ColumnBuilder<'ui> {
    fn new(parent: TableBuilder<'ui>, name: impl Into<Cow<'ui, str>>) -> Self {
        Self {
            parent,
            column: TableColumnSetup::new(name.into()),
        }
    }

    /// Set column flags.
    pub fn flags(mut self, flags: TableColumnFlags) -> Self {
        self.column.flags = flags;
        self
    }

    /// Set fixed width or stretch weight (ImGui uses same field for both).
    pub fn width(mut self, width: f32) -> Self {
        self.column.width = Some(TableColumnWidth::Fixed(width));
        self
    }

    /// Alias of `width()` to express stretch weights.
    pub fn weight(mut self, weight: f32) -> Self {
        self.column.width = Some(TableColumnWidth::Stretch(weight));
        self
    }

    /// Set this column's indentation policy.
    pub fn indent(mut self, indent: TableColumnIndent) -> Self {
        self.column.indent = Some(indent);
        self
    }

    /// Enable or disable indentation for this column.
    pub fn indent_enabled(mut self, enabled: bool) -> Self {
        self.column.indent = Some(if enabled {
            TableColumnIndent::Enable
        } else {
            TableColumnIndent::Disable
        });
        self
    }

    /// Toggle angled header flag.
    pub fn angled_header(mut self, enabled: bool) -> Self {
        if enabled {
            self.column.flags.insert(TableColumnFlags::ANGLED_HEADER);
        } else {
            self.column.flags.remove(TableColumnFlags::ANGLED_HEADER);
        }
        self
    }

    /// Sets the opaque user data associated with this column.
    ///
    /// See [`TableColumnSetup::user_data`] for value semantics.
    pub fn user_data(mut self, user_data: impl Into<TableColumnUserData>) -> Self {
        self.column.user_data = user_data.into();
        self
    }

    /// Finish this column and return to the table builder.
    pub fn done(mut self) -> TableBuilder<'ui> {
        self.parent.columns.push(self.column);
        self.parent
    }
}

#[cfg(feature = "serde")]
crate::validation::impl_serde_i32_flags!(TableFlags, TableColumnFlags, TableColumnStateFlags);

bitflags::bitflags! {
    /// Independent flags for table widgets.
    ///
    /// The table sizing policy is a single-choice setting represented by
    /// [`TableSizingPolicy`].
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct TableFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Enable resizing columns
        const RESIZABLE = sys::ImGuiTableFlags_Resizable;
        /// Enable reordering columns in header row (need calling TableSetupColumn() + TableHeadersRow() to display headers)
        const REORDERABLE = sys::ImGuiTableFlags_Reorderable;
        /// Enable hiding/disabling columns in context menu
        const HIDEABLE = sys::ImGuiTableFlags_Hideable;
        /// Enable sorting. Call TableGetSortSpecs() to obtain sort specs. Also see ImGuiTableFlags_SortMulti and ImGuiTableFlags_SortTristate.
        const SORTABLE = sys::ImGuiTableFlags_Sortable;
        /// Disable persisting columns order, width and sort settings in the .ini file
        const NO_SAVED_SETTINGS = sys::ImGuiTableFlags_NoSavedSettings;
        /// Right-click on columns body/contents will display table context menu. By default it is available in TableHeadersRow().
        const CONTEXT_MENU_IN_BODY = sys::ImGuiTableFlags_ContextMenuInBody;
        /// Set each RowBg color with ImGuiCol_TableRowBg or ImGuiCol_TableRowBgAlt (equivalent of calling TableSetBgColor with ImGuiTableBgFlags_RowBg0 on each row manually)
        const ROW_BG = sys::ImGuiTableFlags_RowBg;
        /// Draw horizontal borders between rows
        const BORDERS_INNER_H = sys::ImGuiTableFlags_BordersInnerH;
        /// Draw horizontal borders at the top and bottom
        const BORDERS_OUTER_H = sys::ImGuiTableFlags_BordersOuterH;
        /// Draw vertical borders between columns
        const BORDERS_INNER_V = sys::ImGuiTableFlags_BordersInnerV;
        /// Draw vertical borders on the left and right sides
        const BORDERS_OUTER_V = sys::ImGuiTableFlags_BordersOuterV;
        /// Draw horizontal borders
        const BORDERS_H = sys::ImGuiTableFlags_BordersH;
        /// Draw vertical borders
        const BORDERS_V = sys::ImGuiTableFlags_BordersV;
        /// Draw inner borders
        const BORDERS_INNER = sys::ImGuiTableFlags_BordersInner;
        /// Draw outer borders
        const BORDERS_OUTER = sys::ImGuiTableFlags_BordersOuter;
        /// Draw all borders
        const BORDERS = sys::ImGuiTableFlags_Borders;
        /// [ALPHA] Disable vertical borders in columns Body (borders will always appears in Headers). -> May move to style
        const NO_BORDERS_IN_BODY = sys::ImGuiTableFlags_NoBordersInBody;
        /// [ALPHA] Disable vertical borders in columns Body until hovered for resize (borders will always appears in Headers). -> May move to style
        const NO_BORDERS_IN_BODY_UNTIL_RESIZE = sys::ImGuiTableFlags_NoBordersInBodyUntilResize;
        /// Make outer width auto-fit to columns, overriding outer_size.x value. Only available when ScrollX/ScrollY are disabled and Stretch columns are not used.
        const NO_HOST_EXTEND_X = sys::ImGuiTableFlags_NoHostExtendX;
        /// Make outer height stop exactly at outer_size.y (prevent auto-extending table past the limit). Only available when ScrollX/ScrollY are disabled. Data below the limit will be clipped and not visible.
        const NO_HOST_EXTEND_Y = sys::ImGuiTableFlags_NoHostExtendY;
        /// Disable keeping column always minimally visible when ScrollX is on and table gets too small. Not recommended if columns are resizable.
        const NO_KEEP_COLUMNS_VISIBLE = sys::ImGuiTableFlags_NoKeepColumnsVisible;
        /// Disable distributing remainder width to stretched columns (width allocation on a 100-wide table with 3 columns: Without this flag: 33,33,34. With this flag: 33,33,33). With larger number of columns, resizing will appear to be less smooth.
        const PRECISE_WIDTHS = sys::ImGuiTableFlags_PreciseWidths;
        /// Disable clipping rectangle for every individual columns (reduce draw command count, items will be able to overflow into other columns). Generally incompatible with TableSetupScrollFreeze().
        const NO_CLIP = sys::ImGuiTableFlags_NoClip;
        /// Default if BordersOuterV is on. Enable outer-most padding. Generally desirable if you have headers.
        const PAD_OUTER_X = sys::ImGuiTableFlags_PadOuterX;
        /// Default if BordersOuterV is off. Disable outer-most padding.
        const NO_PAD_OUTER_X = sys::ImGuiTableFlags_NoPadOuterX;
        /// Disable inner padding between columns (double inner padding if BordersOuterV is on, single inner padding if BordersOuterV is off).
        const NO_PAD_INNER_X = sys::ImGuiTableFlags_NoPadInnerX;
        /// Enable horizontal scrolling. Require 'outer_size' parameter of BeginTable() to specify the container size. Changes default sizing policy. Because this creates a child window, ScrollY is currently generally recommended when using ScrollX.
        const SCROLL_X = sys::ImGuiTableFlags_ScrollX;
        /// Enable vertical scrolling. Require 'outer_size' parameter of BeginTable() to specify the container size.
        const SCROLL_Y = sys::ImGuiTableFlags_ScrollY;
        /// Hold shift when clicking headers to sort on multiple column. TableGetSortSpecs() may return specs where (SpecsCount > 1).
        const SORT_MULTI = sys::ImGuiTableFlags_SortMulti;
        /// Allow no sorting, disable default sorting. TableGetSortSpecs() may return specs where (SpecsCount == 0).
        const SORT_TRISTATE = sys::ImGuiTableFlags_SortTristate;
        /// Highlight column headers when hovered (may not be visible if table header is declaring a background color)
        const HIGHLIGHT_HOVERED_COLUMN = sys::ImGuiTableFlags_HighlightHoveredColumn;
    }
}

/// Single-choice table sizing policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TableSizingPolicy {
    /// Columns default to fixed/auto widths matching contents width.
    FixedFit,
    /// Fixed/auto widths matching the maximum contents width of all columns.
    FixedSame,
    /// Stretch columns with weights proportional to contents widths.
    StretchProp,
    /// Stretch columns with equal weights unless overridden per column.
    StretchSame,
}

impl TableSizingPolicy {
    #[inline]
    const fn raw(self) -> i32 {
        match self {
            Self::FixedFit => sys::ImGuiTableFlags_SizingFixedFit,
            Self::FixedSame => sys::ImGuiTableFlags_SizingFixedSame,
            Self::StretchProp => sys::ImGuiTableFlags_SizingStretchProp,
            Self::StretchSame => sys::ImGuiTableFlags_SizingStretchSame,
        }
    }
}

/// Complete table options assembled from independent flags and an optional
/// single sizing policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TableOptions {
    pub flags: TableFlags,
    pub sizing_policy: Option<TableSizingPolicy>,
}

impl Default for TableOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl TableOptions {
    pub const fn new() -> Self {
        Self {
            flags: TableFlags::NONE,
            sizing_policy: None,
        }
    }

    pub fn flags(mut self, flags: TableFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn sizing_policy(mut self, policy: TableSizingPolicy) -> Self {
        self.sizing_policy = Some(policy);
        self
    }

    pub fn bits(self) -> i32 {
        self.raw()
    }

    #[inline]
    pub(crate) fn raw(self) -> i32 {
        self.flags.bits() | self.sizing_policy.map_or(0, TableSizingPolicy::raw)
    }

    #[inline]
    pub(crate) fn validate(self, caller: &str) {
        let unsupported_flags = self.flags.bits() & !TableFlags::all().bits();
        assert!(
            unsupported_flags == 0,
            "{caller} received non-independent ImGuiTableFlags bits: 0x{unsupported_flags:X}"
        );
        let bits = self.raw();
        let sizing_mask = sys::ImGuiTableFlags_SizingMask_;
        let supported = TableFlags::all().bits() | sizing_mask;
        let unsupported = bits & !supported;
        assert!(
            unsupported == 0,
            "{caller} received unsupported ImGuiTableFlags bits: 0x{unsupported:X}"
        );
        let sizing_policy = bits & sizing_mask;
        assert!(
            is_valid_table_sizing_policy(sizing_policy),
            "{caller} received invalid table sizing policy bits: 0x{sizing_policy:X}"
        );
    }
}

#[inline]
const fn is_valid_table_sizing_policy(bits: i32) -> bool {
    bits == 0
        || bits == TableSizingPolicy::FixedFit.raw()
        || bits == TableSizingPolicy::FixedSame.raw()
        || bits == TableSizingPolicy::StretchProp.raw()
        || bits == TableSizingPolicy::StretchSame.raw()
}

impl From<TableFlags> for TableOptions {
    fn from(flags: TableFlags) -> Self {
        Self::new().flags(flags)
    }
}

bitflags::bitflags! {
    /// Independent flags accepted by `TableSetupColumn()`.
    ///
    /// The fixed/stretch width mode and indent mode are single-choice settings
    /// represented by [`TableColumnWidth`] and [`TableColumnIndent`].
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct TableColumnFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Hide column and omit it from the context menu.
        const DISABLED = sys::ImGuiTableColumnFlags_Disabled;
        /// Default to a hidden/disabled column.
        const DEFAULT_HIDE = sys::ImGuiTableColumnFlags_DefaultHide;
        /// Default to a sorting column.
        const DEFAULT_SORT = sys::ImGuiTableColumnFlags_DefaultSort;
        /// Disable manual resizing
        const NO_RESIZE = sys::ImGuiTableColumnFlags_NoResize;
        /// Disable manual reordering this column
        const NO_REORDER = sys::ImGuiTableColumnFlags_NoReorder;
        /// Disable ability to hide/disable this column
        const NO_HIDE = sys::ImGuiTableColumnFlags_NoHide;
        /// Disable clipping for this column
        const NO_CLIP = sys::ImGuiTableColumnFlags_NoClip;
        /// Disable ability to sort on this field
        const NO_SORT = sys::ImGuiTableColumnFlags_NoSort;
        /// Disable ability to sort in the ascending direction
        const NO_SORT_ASCENDING = sys::ImGuiTableColumnFlags_NoSortAscending;
        /// Disable ability to sort in the descending direction
        const NO_SORT_DESCENDING = sys::ImGuiTableColumnFlags_NoSortDescending;
        /// TableHeadersRow() will not submit label for this column
        const NO_HEADER_LABEL = sys::ImGuiTableColumnFlags_NoHeaderLabel;
        /// Disable header text width contribution to automatic column width
        const NO_HEADER_WIDTH = sys::ImGuiTableColumnFlags_NoHeaderWidth;
        /// Make the initial sort direction Ascending when first sorting on this column
        const PREFER_SORT_ASCENDING = sys::ImGuiTableColumnFlags_PreferSortAscending;
        /// Make the initial sort direction Descending when first sorting on this column
        const PREFER_SORT_DESCENDING = sys::ImGuiTableColumnFlags_PreferSortDescending;
        /// Display an angled header for this column (when angled headers feature is enabled)
        const ANGLED_HEADER = sys::ImGuiTableColumnFlags_AngledHeader;
    }
}

/// Single-choice width mode for a table column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TableColumnWidth {
    /// Initial value is interpreted as a fixed width in pixels.
    Fixed(f32),
    /// Initial value is interpreted as a stretch weight.
    Stretch(f32),
}

impl TableColumnWidth {
    pub const fn fixed(width: f32) -> Self {
        Self::Fixed(width)
    }

    pub const fn stretch(weight: f32) -> Self {
        Self::Stretch(weight)
    }

    #[inline]
    pub(crate) const fn raw_flags(self) -> i32 {
        match self {
            Self::Fixed(_) => sys::ImGuiTableColumnFlags_WidthFixed,
            Self::Stretch(_) => sys::ImGuiTableColumnFlags_WidthStretch,
        }
    }

    #[inline]
    pub(crate) const fn value(self) -> f32 {
        match self {
            Self::Fixed(value) | Self::Stretch(value) => value,
        }
    }
}

/// Single-choice indentation policy for a table column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TableColumnIndent {
    /// Use the current indent value when entering the column.
    Enable,
    /// Disable indentation for the column.
    Disable,
}

impl TableColumnIndent {
    #[inline]
    pub const fn bits(self) -> i32 {
        self.raw_flags()
    }

    #[inline]
    pub(crate) const fn raw_flags(self) -> i32 {
        match self {
            Self::Enable => sys::ImGuiTableColumnFlags_IndentEnable,
            Self::Disable => sys::ImGuiTableColumnFlags_IndentDisable,
        }
    }
}

bitflags::bitflags! {
    /// Flags returned by `TableGetColumnFlags()`.
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct TableColumnStateFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Overriding/master disable flag: hide column and omit it from the context menu.
        const DISABLED = sys::ImGuiTableColumnFlags_Disabled;
        /// Default to a hidden/disabled column.
        const DEFAULT_HIDE = sys::ImGuiTableColumnFlags_DefaultHide;
        /// Default to a sorting column.
        const DEFAULT_SORT = sys::ImGuiTableColumnFlags_DefaultSort;
        /// Overriding width becomes fixed width
        const WIDTH_FIXED = sys::ImGuiTableColumnFlags_WidthFixed;
        /// Overriding width becomes weight
        const WIDTH_STRETCH = sys::ImGuiTableColumnFlags_WidthStretch;
        /// Disable manual resizing
        const NO_RESIZE = sys::ImGuiTableColumnFlags_NoResize;
        /// Disable manual reordering this column
        const NO_REORDER = sys::ImGuiTableColumnFlags_NoReorder;
        /// Disable ability to hide/disable this column
        const NO_HIDE = sys::ImGuiTableColumnFlags_NoHide;
        /// Disable clipping for this column
        const NO_CLIP = sys::ImGuiTableColumnFlags_NoClip;
        /// Disable ability to sort on this field
        const NO_SORT = sys::ImGuiTableColumnFlags_NoSort;
        /// Disable ability to sort in the ascending direction
        const NO_SORT_ASCENDING = sys::ImGuiTableColumnFlags_NoSortAscending;
        /// Disable ability to sort in the descending direction
        const NO_SORT_DESCENDING = sys::ImGuiTableColumnFlags_NoSortDescending;
        /// TableHeadersRow() will not submit label for this column
        const NO_HEADER_LABEL = sys::ImGuiTableColumnFlags_NoHeaderLabel;
        /// Disable header text width contribution to automatic column width
        const NO_HEADER_WIDTH = sys::ImGuiTableColumnFlags_NoHeaderWidth;
        /// Make the initial sort direction Ascending when first sorting on this column
        const PREFER_SORT_ASCENDING = sys::ImGuiTableColumnFlags_PreferSortAscending;
        /// Make the initial sort direction Descending when first sorting on this column
        const PREFER_SORT_DESCENDING = sys::ImGuiTableColumnFlags_PreferSortDescending;
        /// Use current Indent value when entering cell
        const INDENT_ENABLE = sys::ImGuiTableColumnFlags_IndentEnable;
        /// Disable indenting for this column
        const INDENT_DISABLE = sys::ImGuiTableColumnFlags_IndentDisable;
        /// Display an angled header for this column (when angled headers feature is enabled)
        const ANGLED_HEADER = sys::ImGuiTableColumnFlags_AngledHeader;
        /// Status: is enabled == not hidden
        const IS_ENABLED = sys::ImGuiTableColumnFlags_IsEnabled;
        /// Status: is visible == is enabled AND not clipped by scrolling
        const IS_VISIBLE = sys::ImGuiTableColumnFlags_IsVisible;
        /// Status: is currently part of the sort specs
        const IS_SORTED = sys::ImGuiTableColumnFlags_IsSorted;
        /// Status: is hovered by mouse
        const IS_HOVERED = sys::ImGuiTableColumnFlags_IsHovered;
    }
}

impl From<TableColumnFlags> for TableColumnStateFlags {
    fn from(flags: TableColumnFlags) -> Self {
        Self::from_bits_retain(flags.bits())
    }
}

impl TableColumnFlags {
    #[inline]
    pub(crate) fn validate_for_setup(
        self,
        caller: &str,
        width: Option<TableColumnWidth>,
        indent: Option<TableColumnIndent>,
    ) {
        let unsupported_flags = self.bits() & !TableColumnFlags::all().bits();
        assert!(
            unsupported_flags == 0,
            "{caller} received non-independent ImGuiTableColumnFlags bits: 0x{unsupported_flags:X}"
        );
        let bits = self.bits()
            | width.map_or(0, TableColumnWidth::raw_flags)
            | indent.map_or(0, TableColumnIndent::raw_flags);
        let width_mask = sys::ImGuiTableColumnFlags_WidthMask_;
        let indent_mask = sys::ImGuiTableColumnFlags_IndentMask_;
        let supported = TableColumnFlags::all().bits() | width_mask | indent_mask;
        let unsupported = bits & !supported;
        assert!(
            unsupported == 0,
            "{caller} received unsupported ImGuiTableColumnFlags bits: 0x{unsupported:X}"
        );
        assert!(
            (bits & width_mask).count_ones() <= 1,
            "{caller} accepts at most one table column width policy"
        );
        assert!(
            (bits & indent_mask).count_ones() <= 1,
            "{caller} accepts at most one table column indent policy"
        );
    }
}

bitflags::bitflags! {
    /// Flags for table rows
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct TableRowFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Identify header row (set default background color + width of all columns)
        const HEADERS = sys::ImGuiTableRowFlags_Headers;
    }
}

#[cfg(feature = "serde")]
crate::validation::impl_serde_i32_flags!(TableRowFlags);

/// Target for table background colors.
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TableBgTarget {
    /// No background target
    None = sys::ImGuiTableBgTarget_None,
    /// First alternating row background
    RowBg0 = sys::ImGuiTableBgTarget_RowBg0,
    /// Second alternating row background
    RowBg1 = sys::ImGuiTableBgTarget_RowBg1,
    /// Cell background
    CellBg = sys::ImGuiTableBgTarget_CellBg,
}

/// Opaque application data associated with a table column.
///
/// Dear ImGui copies this value unchanged into table sort specifications. Zero is a valid value;
/// it is not treated as an absent ID or hashed through Dear ImGui's widget ID stack.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TableColumnUserData(u32);

impl TableColumnUserData {
    /// Creates opaque table column user data.
    #[inline]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the unchanged opaque value.
    #[inline]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl From<u32> for TableColumnUserData {
    #[inline]
    fn from(value: u32) -> Self {
        Self::new(value)
    }
}

impl From<TableColumnUserData> for u32 {
    #[inline]
    fn from(value: TableColumnUserData) -> Self {
        value.get()
    }
}

/// Table column setup information
#[derive(Clone, Debug)]
pub struct TableColumnSetup<Name> {
    pub name: Name,
    pub flags: TableColumnFlags,
    pub width: Option<TableColumnWidth>,
    pub indent: Option<TableColumnIndent>,
    pub user_data: TableColumnUserData,
}

impl<Name> TableColumnSetup<Name> {
    /// Creates a new table column setup
    pub fn new(name: Name) -> Self {
        Self {
            name,
            flags: TableColumnFlags::NONE,
            width: None,
            indent: None,
            user_data: TableColumnUserData::default(),
        }
    }

    /// Sets the column flags
    pub fn flags(mut self, flags: TableColumnFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Sets a fixed initial column width in pixels.
    pub fn fixed_width(mut self, width: f32) -> Self {
        self.width = Some(TableColumnWidth::Fixed(width));
        self
    }

    /// Sets an initial stretch weight for this column.
    pub fn stretch_weight(mut self, weight: f32) -> Self {
        self.width = Some(TableColumnWidth::Stretch(weight));
        self
    }

    /// Sets this column's indentation policy.
    pub fn indent(mut self, indent: TableColumnIndent) -> Self {
        self.indent = Some(indent);
        self
    }

    /// Enables or disables indentation for this column.
    pub fn indent_enabled(mut self, enabled: bool) -> Self {
        self.indent = Some(if enabled {
            TableColumnIndent::Enable
        } else {
            TableColumnIndent::Disable
        });
        self
    }

    /// Sets the opaque user data associated with this column.
    ///
    /// Dear ImGui returns the value unchanged in table sort specifications. The default is zero,
    /// which remains ordinary application data rather than an absence sentinel.
    ///
    /// The old `user_id` API intentionally has no compatibility alias because widget IDs and
    /// opaque column data have different semantics.
    ///
    /// ```compile_fail
    /// # use dear_imgui_rs::TableColumnSetup;
    /// let _ = TableColumnSetup::new("column").user_id(1_u32);
    /// ```
    pub fn user_data(mut self, user_data: impl Into<TableColumnUserData>) -> Self {
        self.user_data = user_data.into();
        self
    }

    pub(crate) fn map_name<M>(self, map: impl FnOnce(Name) -> M) -> TableColumnSetup<M> {
        TableColumnSetup {
            name: map(self.name),
            flags: self.flags,
            width: self.width,
            indent: self.indent,
            user_data: self.user_data,
        }
    }
}

pub(crate) const TABLE_MAX_COLUMNS: usize = 512;

/// Concrete zero-based table column index.
///
/// This represents a real table column only. Dear ImGui's `-1` current/default
/// sentinel is represented by [`TableColumnRef::Current`] instead.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TableColumnIndex(usize);

impl TableColumnIndex {
    /// The first table column.
    pub const ZERO: Self = Self(0);

    /// Create a table column index from a Rust `usize`.
    #[inline]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Return the zero-based Rust index.
    #[inline]
    pub const fn get(self) -> usize {
        self.0
    }

    #[inline]
    pub(crate) fn into_i32(self, caller: &str) -> i32 {
        i32::try_from(self.0).unwrap_or_else(|_| {
            panic!("{caller} column index exceeded Dear ImGui's i32 range");
        })
    }

    #[inline]
    pub(crate) fn into_imgui_column_idx(self, caller: &str) -> sys::ImGuiTableColumnIdx {
        sys::ImGuiTableColumnIdx::try_from(self.0).unwrap_or_else(|_| {
            panic!("{caller} column index exceeded Dear ImGui's ImGuiTableColumnIdx range");
        })
    }

    #[inline]
    pub(crate) fn from_imgui_column_idx(raw: sys::ImGuiTableColumnIdx, caller: &str) -> Self {
        assert!(raw >= 0, "{caller} returned a negative table column index");
        Self(
            usize::try_from(raw)
                .expect("non-negative Dear ImGui table column index must fit usize"),
        )
    }

    #[inline]
    pub(crate) fn from_i32(raw: i32, caller: &str) -> Self {
        assert!(raw >= 0, "{caller} returned a negative table column index");
        Self(usize::try_from(raw).expect("non-negative table column index must fit usize"))
    }
}

impl From<usize> for TableColumnIndex {
    #[inline]
    fn from(index: usize) -> Self {
        Self::new(index)
    }
}

impl From<TableColumnIndex> for usize {
    #[inline]
    fn from(index: TableColumnIndex) -> Self {
        index.get()
    }
}

impl PartialEq<usize> for TableColumnIndex {
    #[inline]
    fn eq(&self, other: &usize) -> bool {
        self.get() == *other
    }
}

impl PartialEq<TableColumnIndex> for usize {
    #[inline]
    fn eq(&self, other: &TableColumnIndex) -> bool {
        *self == other.get()
    }
}

/// Table column selector for APIs that accept Dear ImGui's current-column sentinel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TableColumnRef {
    /// Use the table's current column.
    #[default]
    Current,
    /// Use a concrete table column index.
    Index(TableColumnIndex),
}

impl TableColumnRef {
    /// Current table column.
    pub const CURRENT: Self = Self::Current;

    /// Select a concrete table column.
    #[inline]
    pub const fn index(index: TableColumnIndex) -> Self {
        Self::Index(index)
    }
}

impl From<TableColumnIndex> for TableColumnRef {
    #[inline]
    fn from(index: TableColumnIndex) -> Self {
        Self::Index(index)
    }
}

impl From<usize> for TableColumnRef {
    #[inline]
    fn from(index: usize) -> Self {
        Self::Index(TableColumnIndex::new(index))
    }
}

/// Result of [`crate::Ui::table_get_hovered_column`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TableHoveredColumn {
    /// The table is not hovered.
    None,
    /// A concrete table column is hovered.
    Column(TableColumnIndex),
    /// The unused space after the right-most visible column is hovered.
    UnusedSpace,
}

/// Concrete zero-based table row index.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TableRowIndex(usize);

impl TableRowIndex {
    /// The first table row.
    pub const ZERO: Self = Self(0);

    /// Create a table row index from a Rust `usize`.
    #[inline]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Return the zero-based Rust index.
    #[inline]
    pub const fn get(self) -> usize {
        self.0
    }

    #[inline]
    pub(crate) fn from_i32(raw: i32, caller: &str) -> Self {
        assert!(raw >= 0, "{caller} returned a negative table row index");
        Self(usize::try_from(raw).expect("non-negative table row index must fit usize"))
    }
}

impl From<usize> for TableRowIndex {
    #[inline]
    fn from(index: usize) -> Self {
        Self::new(index)
    }
}

impl From<TableRowIndex> for usize {
    #[inline]
    fn from(index: TableRowIndex) -> Self {
        index.get()
    }
}

impl PartialEq<usize> for TableRowIndex {
    #[inline]
    fn eq(&self, other: &usize) -> bool {
        self.get() == *other
    }
}

impl PartialEq<TableRowIndex> for usize {
    #[inline]
    fn eq(&self, other: &TableRowIndex) -> bool {
        *self == other.get()
    }
}

/// Result of [`crate::Ui::table_get_hovered_row`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TableHoveredRow {
    /// The table is not hovered.
    None,
    /// A table row index is hovered.
    Row(TableRowIndex),
}

impl TableHoveredRow {
    /// Return the hovered concrete row, if any.
    #[inline]
    pub const fn row(self) -> Option<TableRowIndex> {
        match self {
            Self::Row(index) => Some(index),
            Self::None => None,
        }
    }
}

impl TableHoveredColumn {
    /// Return the hovered concrete column, if any.
    #[inline]
    pub const fn column(self) -> Option<TableColumnIndex> {
        match self {
            Self::Column(index) => Some(index),
            Self::None | Self::UnusedSpace => None,
        }
    }
}

/// Target column for opening a table context menu.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TableContextMenuTarget {
    /// Use Dear ImGui's default: current column when inside a column, otherwise table-level.
    #[default]
    CurrentColumn,
    /// Open the context menu for a concrete column.
    Column(TableColumnIndex),
    /// Force a table-level context menu even when a column is current.
    Table,
}

impl From<TableColumnIndex> for TableContextMenuTarget {
    #[inline]
    fn from(index: TableColumnIndex) -> Self {
        Self::Column(index)
    }
}

impl From<usize> for TableContextMenuTarget {
    #[inline]
    fn from(index: usize) -> Self {
        Self::Column(TableColumnIndex::new(index))
    }
}

/// Safe description of a single angled header cell.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TableHeaderData {
    pub index: TableColumnIndex,
    pub text_color: ImColor32,
    pub bg_color0: ImColor32,
    pub bg_color1: ImColor32,
}

impl TableHeaderData {
    pub fn new(
        index: impl Into<TableColumnIndex>,
        text_color: ImColor32,
        bg_color0: ImColor32,
        bg_color1: ImColor32,
    ) -> Self {
        Self {
            index: index.into(),
            text_color,
            bg_color0,
            bg_color1,
        }
    }
}

/// Sorting direction for table columns.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum SortDirection {
    None = sys::ImGuiSortDirection_None as u8,
    Ascending = sys::ImGuiSortDirection_Ascending as u8,
    Descending = sys::ImGuiSortDirection_Descending as u8,
}

impl From<SortDirection> for sys::ImGuiSortDirection {
    #[inline]
    fn from(value: SortDirection) -> sys::ImGuiSortDirection {
        match value {
            SortDirection::None => sys::ImGuiSortDirection_None,
            SortDirection::Ascending => sys::ImGuiSortDirection_Ascending,
            SortDirection::Descending => sys::ImGuiSortDirection_Descending,
        }
    }
}

/// One column sort spec.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TableColumnSortSpec {
    pub column_user_data: TableColumnUserData,
    pub column_index: TableColumnIndex,
    pub sort_order: i16,
    pub sort_direction: SortDirection,
}

/// Owned snapshot of the current table sort specifications.
///
/// The column data remains valid after the table ends. Clearing Dear ImGui's dirty flag is still
/// a table-scoped operation, so [`Self::clear_dirty`] validates that the source table is current in
/// the same frame before mutating native state.
#[derive(Debug)]
pub struct TableSortSpecs {
    specs: Box<[TableColumnSortSpec]>,
    dirty: bool,
    source_context: crate::ContextId,
    source_scope: crate::scope::TableScope,
}

impl TableSortSpecs {
    /// # Safety
    /// `table` and `raw` must belong to `ui`'s current table in the current frame.
    pub(crate) unsafe fn from_raw(
        ui: &Ui,
        table: *mut sys::ImGuiTable,
        raw: *mut sys::ImGuiTableSortSpecs,
    ) -> Self {
        debug_assert_eq!(unsafe { sys::igGetCurrentTable() }, table);
        let raw = unsafe { raw.as_ref() }.expect("table sort specs pointer must not be null");
        let source_scope = ui
            .current_native_scope()
            .table()
            .expect("TableSortSpecs::from_raw() requires a current table");
        Self {
            specs: unsafe { sort_columns(raw) }.collect(),
            dirty: raw.SpecsDirty,
            source_context: ui.context_id(),
            source_scope,
        }
    }

    /// Whether the specs are marked dirty by dear imgui (you should resort your data).
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clear the native dirty flag after applying this snapshot.
    ///
    /// Returns `true` when the snapshot was already clean or its sort order was acknowledged.
    /// If the table now requests a different order, returns `false` and leaves that request
    /// pending. Use [`Ui::table_get_sort_specs`] to read the current order.
    ///
    /// # Panics
    ///
    /// Panics if `ui` belongs to another Context or if the exact frame, window `Begin`, and table
    /// instance that produced the snapshot is no longer current. The snapshot data itself remains
    /// readable after any of those conditions; only native acknowledgement is table-scoped.
    pub fn clear_dirty(&mut self, ui: &Ui) -> bool {
        if !self.dirty {
            return true;
        }
        assert_eq!(
            ui.context_id(),
            self.source_context,
            "TableSortSpecs::clear_dirty() requires the Ui that produced this snapshot"
        );
        ui.run_with_bound_context(|| unsafe {
            assert_eq!(
                ui.current_native_scope().table(),
                Some(self.source_scope),
                "TableSortSpecs::clear_dirty() source table is no longer current"
            );
            let raw = sys::igTableGetSortSpecs()
                .as_mut()
                .expect("TableSortSpecs::clear_dirty() current table has no sort specifications");
            if !sort_columns(raw).eq(self.specs.iter().copied()) {
                return false;
            }
            raw.SpecsDirty = false;
            self.dirty = false;
            true
        })
    }

    /// Number of column specs.
    pub fn len(&self) -> usize {
        self.specs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterate over column sort specs.
    pub fn iter(&self) -> std::slice::Iter<'_, TableColumnSortSpec> {
        self.specs.iter()
    }
}

// The caller keeps the native columns alive and unchanged for the duration of iteration.
unsafe fn sort_columns(
    raw: &sys::ImGuiTableSortSpecs,
) -> impl ExactSizeIterator<Item = TableColumnSortSpec> + '_ {
    let count = usize::try_from(raw.SpecsCount)
        .expect("Dear ImGui returned a negative table sort specification count");
    let source = raw.Specs;
    assert!(
        count == 0 || !source.is_null(),
        "Dear ImGui returned null table sort specification data for a non-empty snapshot"
    );
    (0..count).map(move |index| {
        let spec = unsafe { &*source.add(index) };
        let direction = match spec.SortDirection as u8 {
            value if value == sys::ImGuiSortDirection_Ascending as u8 => SortDirection::Ascending,
            value if value == sys::ImGuiSortDirection_Descending as u8 => SortDirection::Descending,
            _ => SortDirection::None,
        };
        TableColumnSortSpec {
            column_user_data: TableColumnUserData::new(spec.ColumnUserID),
            column_index: TableColumnIndex::from_imgui_column_idx(
                spec.ColumnIndex,
                "TableSortSpecs::from_raw()",
            ),
            sort_order: spec.SortOrder,
            sort_direction: direction,
        }
    })
}

/// Tracks a table that can be ended by calling `.end()` or by dropping
#[must_use]
#[doc(alias = "EndTable")]
pub struct TableToken<'ui> {
    scope: NativeScopeToken<'ui>,
}

impl<'ui> TableToken<'ui> {
    /// Creates a new table token
    pub(crate) fn new(ui: &'ui Ui) -> Self {
        Self {
            scope: ui.begin_native_scope(NativeScopePop::EndTable, "TableToken"),
        }
    }

    /// Ends the table.
    ///
    /// # Panics
    ///
    /// Panics before FFI if a nested table or any native scope created inside this table is still
    /// active, or if this table is no longer the current native table instance. Create scopes
    /// outside the table when they intentionally need to span the table lifetime.
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl<'ui> Drop for TableToken<'ui> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}

struct TableChannelGuard<'ui> {
    scope: NativeScopeToken<'ui>,
}

impl<'ui> TableChannelGuard<'ui> {
    fn background(ui: &'ui Ui) -> Self {
        Self {
            scope: ui.begin_native_scope(
                NativeScopePop::PopTableBackgroundChannel,
                "table background channel",
            ),
        }
    }

    fn column(ui: &'ui Ui) -> Self {
        Self {
            scope: ui.begin_native_scope(
                NativeScopePop::PopTableColumnChannel,
                "table column channel",
            ),
        }
    }
}

impl Drop for TableChannelGuard<'_> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}

pub(crate) fn table_column_count_to_i32(column_count: usize) -> i32 {
    assert!(
        column_count > 0,
        "table column_count must be greater than zero"
    );
    assert!(
        column_count < TABLE_MAX_COLUMNS,
        "table column_count must be less than {TABLE_MAX_COLUMNS}"
    );
    i32::try_from(column_count).expect("table column_count exceeded ImGui's i32 range")
}

pub(crate) fn table_freeze_count_to_i32(caller: &str, name: &str, count: usize, max: usize) -> i32 {
    assert!(count < max, "{caller} {name} must be less than {max}");
    i32::try_from(count).expect("table freeze count exceeded ImGui's i32 range")
}

fn current_table() -> *mut sys::ImGuiTable {
    unsafe { sys::igGetCurrentTable() }
}

pub(crate) fn current_table_if_any() -> Option<*mut sys::ImGuiTable> {
    let table = current_table();
    (!table.is_null()).then_some(table)
}

pub(crate) fn assert_current_table(caller: &str) -> *mut sys::ImGuiTable {
    let table = current_table();
    assert!(
        !table.is_null(),
        "{caller} must be called inside a BeginTable/EndTable scope"
    );
    table
}

pub(crate) fn assert_valid_table_column_raw_in(
    table: *mut sys::ImGuiTable,
    column_n: i32,
    caller: &str,
) {
    let column_count = unsafe { (*table).ColumnsCount };
    assert!(
        (0..column_count).contains(&column_n),
        "{caller} column index {column_n} is outside the current table column range 0..{column_count}"
    );
}

pub(crate) fn assert_valid_table_column_in(
    table: *mut sys::ImGuiTable,
    column: TableColumnIndex,
    caller: &str,
) -> i32 {
    let column_n = column.into_i32(caller);
    assert_valid_table_column_raw_in(table, column_n, caller);
    column_n
}

pub(crate) fn assert_valid_table_column(column: TableColumnIndex, caller: &str) -> i32 {
    let table = assert_current_table(caller);
    assert_valid_table_column_in(table, column, caller)
}

pub(crate) fn assert_non_negative_finite_f32(caller: &str, name: &str, value: f32) {
    assert!(value.is_finite(), "{caller} {name} must be finite");
    assert!(value >= 0.0, "{caller} {name} must be non-negative");
}

pub(crate) fn resolve_table_column(
    table: *mut sys::ImGuiTable,
    column: TableColumnRef,
    caller: &str,
) -> i32 {
    let column_n = match column {
        TableColumnRef::Current => unsafe { (*table).CurrentColumn },
        TableColumnRef::Index(index) => index.into_i32(caller),
    };
    assert_valid_table_column_raw_in(table, column_n, caller);
    column_n
}

pub(crate) fn assert_current_table_has_flags(
    flags: TableFlags,
    caller: &str,
) -> *mut sys::ImGuiTable {
    let table = assert_current_table(caller);
    let table_flags = TableFlags::from_bits_retain(unsafe { (*table).Flags });
    assert!(
        table_flags.contains(flags),
        "{caller} requires the current table to have {flags:?}"
    );
    table
}

pub(crate) fn assert_table_setup_phase(caller: &str) {
    let table = assert_current_table(caller);
    assert!(
        !unsafe { (*table).IsLayoutLocked },
        "{caller} must be called before the first table row or column"
    );
}

pub(crate) fn assert_table_column_width_phase(caller: &str) {
    let table = assert_current_table(caller);
    assert!(
        !unsafe { (*table).IsLayoutLocked },
        "{caller} must be called before the table layout is locked"
    );
    assert!(
        unsafe { (*table).MinColumnWidth > 0.0 },
        "{caller} requires Dear ImGui table layout metrics to be initialized"
    );
}

pub(crate) fn assert_current_table_cell(caller: &str) {
    let table = assert_current_table(caller);
    let (current_column, column_count) = unsafe { ((*table).CurrentColumn, (*table).ColumnsCount) };
    assert!(
        (0..column_count).contains(&current_column),
        "{caller} must be called while a table cell is current"
    );
}

pub(crate) fn assert_current_table_row(caller: &str) -> *mut sys::ImGuiTable {
    let table = assert_current_table(caller);
    assert!(
        unsafe { (*table).CurrentRow } >= 0,
        "{caller} must be called while a table row is current"
    );
    table
}

pub(crate) fn assert_table_before_first_row(caller: &str) -> *mut sys::ImGuiTable {
    let table = assert_current_table(caller);
    assert!(
        unsafe { (*table).CurrentRow } == -1,
        "{caller} must be called before the first table row"
    );
    table
}

#[cfg(test)]
mod tests;

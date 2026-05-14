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
use crate::sys;
use crate::ui::Ui;
use crate::widget::{
    TableColumnFlags, TableColumnIndent, TableColumnStateFlags, TableColumnWidth, TableFlags,
    TableOptions, TableSizingPolicy,
};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::ffi::CStr;

const TABLE_MAX_COLUMNS: usize = 512;

fn table_column_count_to_i32(column_count: usize) -> i32 {
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

fn current_table() -> *mut sys::ImGuiTable {
    unsafe { sys::igGetCurrentTable() }
}

fn assert_current_table(caller: &str) -> *mut sys::ImGuiTable {
    let table = current_table();
    assert!(
        !table.is_null(),
        "{caller} must be called inside a BeginTable/EndTable scope"
    );
    table
}

fn assert_valid_table_column(column_n: i32, caller: &str) {
    let table = assert_current_table(caller);
    let column_count = unsafe { (*table).ColumnsCount };
    assert!(
        (0..column_count).contains(&column_n),
        "{caller} column index {column_n} is outside the current table column range 0..{column_count}"
    );
}

fn assert_current_table_cell(caller: &str) {
    let table = assert_current_table(caller);
    let (current_column, column_count) = unsafe { ((*table).CurrentColumn, (*table).ColumnsCount) };
    assert!(
        (0..column_count).contains(&current_column),
        "{caller} must be called while a table cell is current"
    );
}

/// Table column setup information
#[derive(Clone, Debug)]
pub struct TableColumnSetup<Name> {
    pub name: Name,
    pub flags: TableColumnFlags,
    pub width: Option<TableColumnWidth>,
    pub indent: Option<TableColumnIndent>,
    pub user_id: u32,
}

impl<Name> TableColumnSetup<Name> {
    /// Creates a new table column setup
    pub fn new(name: Name) -> Self {
        Self {
            name,
            flags: TableColumnFlags::NONE,
            width: None,
            indent: None,
            user_id: 0,
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

    /// Sets the user ID
    pub fn user_id(mut self, id: u32) -> Self {
        self.user_id = id;
        self
    }
}

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
        let str_id_ptr = self.scratch_txt(str_id);
        let outer_size_vec: sys::ImVec2 = outer_size.into().into();
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
        user_id: u32,
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
        user_id: u32,
    ) {
        let label_ptr = self.scratch_txt(label);
        let raw_flags = flags.bits()
            | width.map_or(0, TableColumnWidth::raw_flags)
            | indent.map_or(0, TableColumnIndent::raw_flags);
        let init_width_or_weight = width.map_or(0.0, TableColumnWidth::value);
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
        user_id: u32,
    ) {
        self.table_setup_column(label, flags, Some(TableColumnWidth::Fixed(width)), user_id);
    }

    /// Setup a column with a stretch weight.
    pub fn table_setup_column_stretch_weight(
        &self,
        label: impl AsRef<str>,
        flags: TableColumnFlags,
        weight: f32,
        user_id: u32,
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
        unsafe {
            sys::igTableHeadersRow();
        }
    }

    /// Append into the next column (or first column of next row if currently in last column)
    pub fn table_next_column(&self) -> bool {
        unsafe { sys::igTableNextColumn() }
    }

    /// Append into the specified column
    pub fn table_set_column_index(&self, column_n: i32) -> bool {
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
    pub fn table_setup_scroll_freeze(&self, frozen_cols: i32, frozen_rows: i32) {
        unsafe { sys::igTableSetupScrollFreeze(frozen_cols, frozen_rows) }
    }

    /// Submit one header cell at current column position.
    #[doc(alias = "TableHeader")]
    pub fn table_header(&self, label: impl AsRef<str>) {
        let label_ptr = self.scratch_txt(label);
        unsafe { sys::igTableHeader(label_ptr) }
    }

    /// Return columns count.
    #[doc(alias = "TableGetColumnCount")]
    pub fn table_get_column_count(&self) -> i32 {
        unsafe { sys::igTableGetColumnCount() }
    }

    /// Return current column index.
    #[doc(alias = "TableGetColumnIndex")]
    pub fn table_get_column_index(&self) -> i32 {
        unsafe { sys::igTableGetColumnIndex() }
    }

    /// Return current row index.
    #[doc(alias = "TableGetRowIndex")]
    pub fn table_get_row_index(&self) -> i32 {
        unsafe { sys::igTableGetRowIndex() }
    }

    /// Return the name of a column by index.
    #[doc(alias = "TableGetColumnName")]
    pub fn table_get_column_name(&self, column_n: i32) -> &str {
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
    pub fn table_get_column_flags(&self, column_n: i32) -> TableColumnStateFlags {
        unsafe { TableColumnStateFlags::from_bits_truncate(sys::igTableGetColumnFlags(column_n)) }
    }

    /// Enable/disable a column by index.
    #[doc(alias = "TableSetColumnEnabled")]
    pub fn table_set_column_enabled(&self, column_n: i32, enabled: bool) {
        unsafe { sys::igTableSetColumnEnabled(column_n, enabled) }
    }

    /// Return hovered column index, or -1 when none.
    #[doc(alias = "TableGetHoveredColumn")]
    pub fn table_get_hovered_column(&self) -> i32 {
        unsafe { sys::igTableGetHoveredColumn() }
    }

    /// Set column width (for fixed-width columns).
    #[doc(alias = "TableSetColumnWidth")]
    pub fn table_set_column_width(&self, column_n: i32, width: f32) {
        unsafe { sys::igTableSetColumnWidth(column_n, width) }
    }

    /// Set a table background color target.
    ///
    /// Color must be an ImGui-packed ImU32 in ABGR order (IM_COL32).
    /// Use `crate::colors::Color::to_imgui_u32()` to convert RGBA floats.
    #[doc(alias = "TableSetBgColor")]
    pub fn table_set_bg_color_u32(&self, target: TableBgTarget, color: u32, column_n: i32) {
        unsafe { sys::igTableSetBgColor(target as i32, color, column_n) }
    }

    /// Set a table background color target using RGBA color (0..=1 floats).
    pub fn table_set_bg_color(&self, target: TableBgTarget, rgba: [f32; 4], column_n: i32) {
        // Pack to ImGui's ABGR layout.
        let col = crate::colors::Color::from_array(rgba).to_imgui_u32();
        unsafe { sys::igTableSetBgColor(target as i32, col, column_n) }
    }

    /// Return hovered row index, or -1 when none.
    #[doc(alias = "TableGetHoveredRow")]
    pub fn table_get_hovered_row(&self) -> i32 {
        unsafe { sys::igTableGetHoveredRow() }
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
        column_n: i32,
        dir: SortDirection,
        append_to_sort_specs: bool,
    ) {
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

bitflags::bitflags! {
    /// Flags for table rows
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct TableRowFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Identify header row (set default background color + width of all columns)
        const HEADERS = sys::ImGuiTableRowFlags_Headers as i32;
    }
}

#[cfg(feature = "serde")]
impl Serialize for TableRowFlags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i32(self.bits())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for TableRowFlags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bits = i32::deserialize(deserializer)?;
        Ok(TableRowFlags::from_bits_truncate(bits))
    }
}

/// Target for table background colors.
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TableBgTarget {
    /// No background target
    None = sys::ImGuiTableBgTarget_None as i32,
    /// First alternating row background
    RowBg0 = sys::ImGuiTableBgTarget_RowBg0 as i32,
    /// Second alternating row background
    RowBg1 = sys::ImGuiTableBgTarget_RowBg1 as i32,
    /// Cell background
    CellBg = sys::ImGuiTableBgTarget_CellBg as i32,
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
#[derive(Copy, Clone, Debug)]
pub struct TableColumnSortSpec {
    pub column_user_id: u32,
    pub column_index: i16,
    pub sort_order: i16,
    pub sort_direction: SortDirection,
}

/// Table sort specs view.
pub struct TableSortSpecs<'a> {
    raw: *mut sys::ImGuiTableSortSpecs,
    _marker: std::marker::PhantomData<&'a Ui>,
}

impl<'a> TableSortSpecs<'a> {
    /// # Safety
    /// `raw` must be a valid pointer returned by ImGui_TableGetSortSpecs for the current table.
    pub(crate) unsafe fn from_raw(raw: *mut sys::ImGuiTableSortSpecs) -> Self {
        Self {
            raw,
            _marker: std::marker::PhantomData,
        }
    }

    /// Whether the specs are marked dirty by dear imgui (you should resort your data).
    pub fn is_dirty(&self) -> bool {
        unsafe { (*self.raw).SpecsDirty }
    }

    /// Clear the dirty flag after you've applied sorting to your data.
    pub fn clear_dirty(&mut self) {
        unsafe { (*self.raw).SpecsDirty = false }
    }

    /// Number of column specs.
    pub fn len(&self) -> usize {
        unsafe { (*self.raw).SpecsCount as usize }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterate over column sort specs.
    pub fn iter(&self) -> TableSortSpecsIter<'_> {
        TableSortSpecsIter {
            specs: self,
            index: 0,
        }
    }
}

/// Iterator over [`TableColumnSortSpec`].
pub struct TableSortSpecsIter<'a> {
    specs: &'a TableSortSpecs<'a>,
    index: usize,
}

impl<'a> Iterator for TableSortSpecsIter<'a> {
    type Item = TableColumnSortSpec;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.specs.len() {
            return None;
        }
        unsafe {
            let ptr = (*self.specs.raw).Specs;
            if ptr.is_null() {
                return None;
            }
            let spec = &*ptr.add(self.index);
            self.index += 1;
            let d = spec.SortDirection as u8;
            let dir = if d == sys::ImGuiSortDirection_None as u8 {
                SortDirection::None
            } else if d == sys::ImGuiSortDirection_Ascending as u8 {
                SortDirection::Ascending
            } else if d == sys::ImGuiSortDirection_Descending as u8 {
                SortDirection::Descending
            } else {
                SortDirection::None
            };
            Some(TableColumnSortSpec {
                column_user_id: spec.ColumnUserID,
                column_index: spec.ColumnIndex,
                sort_order: spec.SortOrder,
                sort_direction: dir,
            })
        }
    }
}

/// Tracks a table that can be ended by calling `.end()` or by dropping
#[must_use]
pub struct TableToken<'ui> {
    _ui: &'ui Ui,
}

impl<'ui> TableToken<'ui> {
    /// Creates a new table token
    fn new(ui: &'ui Ui) -> Self {
        TableToken { _ui: ui }
    }

    /// Ends the table
    pub fn end(self) {
        // The drop implementation will handle the actual ending
    }
}

impl<'ui> Drop for TableToken<'ui> {
    fn drop(&mut self) {
        unsafe {
            sys::igEndTable();
        }
    }
}

/// Tracks a pushed table background draw channel.
#[must_use = "dropping the token pops the table background draw channel immediately"]
pub struct TableBackgroundChannelToken<'ui> {
    _ui: &'ui Ui,
}

impl<'ui> TableBackgroundChannelToken<'ui> {
    fn new(ui: &'ui Ui) -> Self {
        Self { _ui: ui }
    }

    /// Pops the table background draw channel.
    pub fn pop(self) {}

    /// Pops the table background draw channel.
    pub fn end(self) {}
}

impl Drop for TableBackgroundChannelToken<'_> {
    fn drop(&mut self) {
        unsafe {
            sys::igTablePopBackgroundChannel();
        }
    }
}

/// Tracks a pushed table column draw channel.
#[must_use = "dropping the token pops the table column draw channel immediately"]
pub struct TableColumnChannelToken<'ui> {
    _ui: &'ui Ui,
}

impl<'ui> TableColumnChannelToken<'ui> {
    fn new(ui: &'ui Ui) -> Self {
        Self { _ui: ui }
    }

    /// Pops the table column draw channel.
    pub fn pop(self) {}

    /// Pops the table column draw channel.
    pub fn end(self) {}
}

impl Drop for TableColumnChannelToken<'_> {
    fn drop(&mut self) {
        unsafe {
            sys::igTablePopColumnChannel();
        }
    }
}

// ============================================================================
// Additional table convenience APIs
// ============================================================================

/// Safe description of a single angled header cell.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TableHeaderData {
    pub index: i16,
    pub text_color: ImColor32,
    pub bg_color0: ImColor32,
    pub bg_color1: ImColor32,
}

impl TableHeaderData {
    pub fn new(
        index: i16,
        text_color: ImColor32,
        bg_color0: ImColor32,
        bg_color1: ImColor32,
    ) -> Self {
        Self {
            index,
            text_color,
            bg_color0,
            bg_color1,
        }
    }
}
impl Ui {
    /// Maximum label width used for angled headers (when enabled in style/options).
    #[doc(alias = "TableGetHeaderAngledMaxLabelWidth")]
    pub fn table_get_header_angled_max_label_width(&self) -> f32 {
        unsafe { sys::igTableGetHeaderAngledMaxLabelWidth() }
    }

    /// Submit angled headers row (requires style/flags enabling angled headers).
    #[doc(alias = "TableAngledHeadersRow")]
    pub fn table_angled_headers_row(&self) {
        unsafe { sys::igTableAngledHeadersRow() }
    }

    // Removed legacy TableAngledHeadersRowEx(flags) wrapper; use `table_angled_headers_row_ex_with_data`.

    /// Submit angled headers row with explicit data (Ex variant).
    ///
    /// - `row_id`: ImGuiID for the row. Use 0 for automatic if not needed.
    /// - `angle`: Angle in radians for headers.
    /// - `max_label_width`: Maximum label width for angled headers.
    /// - `headers`: Per-column header data.
    pub fn table_angled_headers_row_ex_with_data(
        &self,
        row_id: u32,
        angle: f32,
        max_label_width: f32,
        headers: &[TableHeaderData],
    ) {
        if headers.is_empty() {
            unsafe { sys::igTableAngledHeadersRow() }
            return;
        }
        let count = match i32::try_from(headers.len()) {
            Ok(n) => n,
            Err(_) => return,
        };
        let mut data: Vec<sys::ImGuiTableHeaderData> = Vec::with_capacity(headers.len());
        for h in headers {
            data.push(sys::ImGuiTableHeaderData {
                Index: h.index as sys::ImGuiTableColumnIdx,
                TextColor: u32::from(h.text_color),
                BgColor0: u32::from(h.bg_color0),
                BgColor1: u32::from(h.bg_color1),
            });
        }
        unsafe {
            sys::igTableAngledHeadersRowEx(row_id, angle, max_label_width, data.as_ptr(), count);
        }
    }

    /// Push background draw channel for the current table and return a token to pop it.
    #[doc(alias = "TablePushBackgroundChannel")]
    pub fn table_push_background_channel(&self) {
        assert_current_table_cell("Ui::table_push_background_channel()");
        unsafe { sys::igTablePushBackgroundChannel() }
    }

    /// Pop background draw channel for the current table.
    #[doc(alias = "TablePopBackgroundChannel")]
    pub fn table_pop_background_channel(&self) {
        assert_current_table_cell("Ui::table_pop_background_channel()");
        unsafe { sys::igTablePopBackgroundChannel() }
    }

    /// Push column draw channel for the given column index and return a token to pop it.
    #[doc(alias = "TablePushColumnChannel")]
    pub fn table_push_column_channel(&self, column_n: i32) {
        assert_valid_table_column(column_n, "Ui::table_push_column_channel()");
        unsafe { sys::igTablePushColumnChannel(column_n) }
    }

    /// Pop column draw channel.
    #[doc(alias = "TablePopColumnChannel")]
    pub fn table_pop_column_channel(&self) {
        assert_current_table_cell("Ui::table_pop_column_channel()");
        unsafe { sys::igTablePopColumnChannel() }
    }

    /// Push background draw channel for the current table and return a token to pop it.
    #[must_use = "dropping the token pops the table background draw channel immediately"]
    #[doc(alias = "TablePushBackgroundChannel")]
    pub fn table_background_channel(&self) -> TableBackgroundChannelToken<'_> {
        self.table_push_background_channel();
        TableBackgroundChannelToken::new(self)
    }

    /// Push column draw channel for the given column index and return a token to pop it.
    #[must_use = "dropping the token pops the table column draw channel immediately"]
    #[doc(alias = "TablePushColumnChannel")]
    pub fn table_column_channel(&self, column_n: i32) -> TableColumnChannelToken<'_> {
        self.table_push_column_channel(column_n);
        TableColumnChannelToken::new(self)
    }

    /// Run a closure after pushing table background channel (auto-pop on return).
    pub fn with_table_background_channel<R>(&self, f: impl FnOnce() -> R) -> R {
        let _token = self.table_background_channel();
        f()
    }

    /// Run a closure after pushing a table column channel (auto-pop on return).
    pub fn with_table_column_channel<R>(&self, column_n: i32, f: impl FnOnce() -> R) -> R {
        let _token = self.table_column_channel(column_n);
        f()
    }

    /// Open the table context menu for a given column (use -1 for current/default).
    #[doc(alias = "TableOpenContextMenu")]
    pub fn table_open_context_menu(&self, column_n: Option<i32>) {
        unsafe { sys::igTableOpenContextMenu(column_n.unwrap_or(-1)) }
    }
}

// (Optional) RAII versions could be added later if desirable

// ============================================================================
// TableBuilder: ergonomic table construction
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_context() -> crate::Context {
        let mut ctx = crate::Context::create();
        {
            let io = ctx.io_mut();
            io.set_display_size([128.0, 128.0]);
            io.set_delta_time(1.0 / 60.0);
        }
        let _ = ctx.font_atlas_mut().build();
        let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
        ctx
    }

    unsafe fn current_table_draw_channel() -> i32 {
        let table = assert_current_table("current_table_draw_channel()");
        let draw_list = unsafe { (*(*table).InnerWindow).DrawList };
        unsafe { (*draw_list)._Splitter._Current }
    }

    #[test]
    fn table_column_channel_is_popped_after_panic() {
        let mut ctx = setup_context();

        let ui = ctx.frame();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = ui.window("table_channel_panic").build(|| {
                let _table = ui.begin_table("table", 2).unwrap();
                ui.table_next_row();
                assert!(ui.table_set_column_index(0));
                let initial_channel = unsafe { current_table_draw_channel() };

                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ui.with_table_column_channel(1, || {
                        let pushed_channel = unsafe { current_table_draw_channel() };
                        assert_ne!(pushed_channel, initial_channel);
                        panic!("forced panic while table column channel is pushed");
                    });
                }));

                assert!(result.is_err());
                assert_eq!(unsafe { current_table_draw_channel() }, initial_channel);
            });
        }));

        assert!(result.is_ok());
    }

    #[test]
    fn begin_table_rejects_invalid_column_counts_before_ffi() {
        let mut ctx = setup_context();

        let ui = ctx.frame();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.begin_table("zero_columns", 0);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.begin_table("too_many_columns", TABLE_MAX_COLUMNS);
            }))
            .is_err()
        );
    }

    #[test]
    fn table_column_channel_rejects_out_of_range_column_before_ffi() {
        let mut ctx = setup_context();

        let ui = ctx.frame();
        let _ = ui.window("table_channel_oob").build(|| {
            let _table = ui.begin_table("table", 2).unwrap();
            ui.table_next_row();
            assert!(ui.table_set_column_index(0));

            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _token = ui.table_column_channel(-1);
                }))
                .is_err()
            );
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _token = ui.table_column_channel(2);
                }))
                .is_err()
            );
        });
    }

    #[test]
    fn table_channels_require_current_cell_before_ffi() {
        let mut ctx = setup_context();

        let ui = ctx.frame();
        let _ = ui.window("table_channel_cell_required").build(|| {
            let _table = ui.begin_table("table", 2).unwrap();

            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _token = ui.table_background_channel();
                }))
                .is_err()
            );
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ui.table_pop_column_channel();
                }))
                .is_err()
            );
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
    freeze: Option<(i32, i32)>,
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

    /// Set outer size (width, height). Default [0,0]
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
    pub fn freeze(mut self, frozen_cols: i32, frozen_rows: i32) -> Self {
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
        self.columns.clear();
        for c in cols {
            self.columns.push(TableColumnSetup {
                name: c.name.into(),
                flags: c.flags,
                width: c.width,
                indent: c.indent,
                user_id: c.user_id,
            });
        }
        self
    }

    /// Add a single column setup
    pub fn add_column<Name: Into<Cow<'ui, str>>>(mut self, col: TableColumnSetup<Name>) -> Self {
        self.columns.push(TableColumnSetup {
            name: col.name.into(),
            flags: col.flags,
            width: col.width,
            indent: col.indent,
            user_id: col.user_id,
        });
        self
    }

    /// Auto submit headers row from `TableSetupColumn()` entries
    pub fn headers(mut self, enabled: bool) -> Self {
        self.use_headers = enabled;
        self
    }

    /// Build the table and run a closure to emit rows/cells
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
                self.ui.table_setup_column_with_indent(
                    col.name.as_ref(),
                    col.flags,
                    col.width,
                    col.indent,
                    col.user_id,
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
    name: Cow<'ui, str>,
    flags: TableColumnFlags,
    width: Option<TableColumnWidth>,
    indent: Option<TableColumnIndent>,
    user_id: u32,
}

impl<'ui> ColumnBuilder<'ui> {
    fn new(parent: TableBuilder<'ui>, name: impl Into<Cow<'ui, str>>) -> Self {
        Self {
            parent,
            name: name.into(),
            flags: TableColumnFlags::NONE,
            width: None,
            indent: None,
            user_id: 0,
        }
    }

    /// Set column flags.
    pub fn flags(mut self, flags: TableColumnFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Set fixed width or stretch weight (ImGui uses same field for both).
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(TableColumnWidth::Fixed(width));
        self
    }

    /// Alias of `width()` to express stretch weights.
    pub fn weight(mut self, weight: f32) -> Self {
        self.width = Some(TableColumnWidth::Stretch(weight));
        self
    }

    /// Set this column's indentation policy.
    pub fn indent(mut self, indent: TableColumnIndent) -> Self {
        self.indent = Some(indent);
        self
    }

    /// Enable or disable indentation for this column.
    pub fn indent_enabled(mut self, enabled: bool) -> Self {
        self.indent = Some(if enabled {
            TableColumnIndent::Enable
        } else {
            TableColumnIndent::Disable
        });
        self
    }

    /// Toggle angled header flag.
    pub fn angled_header(mut self, enabled: bool) -> Self {
        if enabled {
            self.flags.insert(TableColumnFlags::ANGLED_HEADER);
        } else {
            self.flags.remove(TableColumnFlags::ANGLED_HEADER);
        }
        self
    }

    /// Set user id for this column.
    pub fn user_id(mut self, id: u32) -> Self {
        self.user_id = id;
        self
    }

    /// Finish this column and return to the table builder.
    pub fn done(mut self) -> TableBuilder<'ui> {
        self.parent.columns.push(TableColumnSetup {
            name: self.name,
            flags: self.flags,
            width: self.width,
            indent: self.indent,
            user_id: self.user_id,
        });
        self.parent
    }
}

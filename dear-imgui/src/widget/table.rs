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
use crate::widget::{TableColumnFlags, TableFlags};
use std::ffi::CStr;

/// Table column setup information
#[derive(Clone, Debug)]
pub struct TableColumnSetup<Name: AsRef<str>> {
    pub name: Name,
    pub flags: TableColumnFlags,
    pub init_width_or_weight: f32,
    pub user_id: u32,
}

impl<Name: AsRef<str>> TableColumnSetup<Name> {
    /// Creates a new table column setup
    pub fn new(name: Name) -> Self {
        Self {
            name,
            flags: TableColumnFlags::NONE,
            init_width_or_weight: 0.0,
            user_id: 0,
        }
    }

    /// Sets the column flags
    pub fn flags(mut self, flags: TableColumnFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Sets the initial width or weight
    pub fn init_width_or_weight(mut self, width: f32) -> Self {
        self.init_width_or_weight = width;
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
    pub fn table(&self, str_id: impl AsRef<str>) -> TableBuilder<'_> {
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
        flags: TableFlags,
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
        flags: TableFlags,
        outer_size: impl Into<[f32; 2]>,
        inner_width: f32,
    ) -> Option<TableToken<'_>> {
        let str_id_ptr = self.scratch_txt(str_id);
        let outer_size_vec: sys::ImVec2 = outer_size.into().into();

        let should_render = unsafe {
            sys::igBeginTable_Str(
                str_id_ptr,
                column_count as i32,
                flags.bits(),
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
        flags: TableFlags,
    ) -> Option<TableToken<'_>> {
        if let Some(token) = self.begin_table_with_flags(str_id, N, flags) {
            // Setup columns
            for column in &column_data {
                self.table_setup_column(
                    &column.name,
                    column.flags,
                    column.init_width_or_weight,
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
        init_width_or_weight: f32,
        user_id: u32,
    ) {
        let label_ptr = self.scratch_txt(label);
        unsafe {
            sys::igTableSetupColumn_Str(label_ptr, flags.bits(), init_width_or_weight, user_id);
        }
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
        unsafe { sys::igTableHeader_Str(label_ptr) }
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
    pub fn table_get_column_flags(&self, column_n: i32) -> TableColumnFlags {
        unsafe { TableColumnFlags::from_bits_truncate(sys::igTableGetColumnFlags(column_n)) }
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
    pub struct TableRowFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Identify header row (set default background color + width of all columns)
        const HEADERS = sys::ImGuiTableRowFlags_Headers as i32;
    }
}

/// Target for table background colors.
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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
    ui: &'ui Ui,
}

impl<'ui> TableToken<'ui> {
    /// Creates a new table token
    fn new(ui: &'ui Ui) -> Self {
        TableToken { ui }
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
            sys::igTableAngledHeadersRowEx(
                row_id,
                angle,
                max_label_width,
                data.as_ptr(),
                data.len() as i32,
            );
        }
    }

    /// Push background draw channel for the current table and return a token to pop it.
    #[doc(alias = "TablePushBackgroundChannel")]
    pub fn table_push_background_channel(&self) {
        unsafe { sys::igTablePushBackgroundChannel() }
    }

    /// Pop background draw channel for the current table.
    #[doc(alias = "TablePopBackgroundChannel")]
    pub fn table_pop_background_channel(&self) {
        unsafe { sys::igTablePopBackgroundChannel() }
    }

    /// Push column draw channel for the given column index and return a token to pop it.
    #[doc(alias = "TablePushColumnChannel")]
    pub fn table_push_column_channel(&self, column_n: i32) {
        unsafe { sys::igTablePushColumnChannel(column_n) }
    }

    /// Pop column draw channel.
    #[doc(alias = "TablePopColumnChannel")]
    pub fn table_pop_column_channel(&self) {
        unsafe { sys::igTablePopColumnChannel() }
    }

    /// Run a closure after pushing table background channel (auto-pop on return).
    pub fn with_table_background_channel<R>(&self, f: impl FnOnce() -> R) -> R {
        self.table_push_background_channel();
        let result = f();
        self.table_pop_background_channel();
        result
    }

    /// Run a closure after pushing a table column channel (auto-pop on return).
    pub fn with_table_column_channel<R>(&self, column_n: i32, f: impl FnOnce() -> R) -> R {
        self.table_push_column_channel(column_n);
        let result = f();
        self.table_pop_column_channel();
        result
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

/// Builder for ImGui tables with columns + headers + sizing/freeze options.
#[derive(Debug)]
pub struct TableBuilder<'ui> {
    ui: &'ui Ui,
    id: String,
    flags: TableFlags,
    outer_size: [f32; 2],
    inner_width: f32,
    columns: Vec<TableColumnSetup<String>>, // owned names
    use_headers: bool,
    freeze: Option<(i32, i32)>,
}

impl<'ui> TableBuilder<'ui> {
    /// Create a new TableBuilder. Prefer using `Ui::table("id")`.
    pub fn new(ui: &'ui Ui, str_id: impl AsRef<str>) -> Self {
        Self {
            ui,
            id: str_id.as_ref().to_string(),
            flags: TableFlags::NONE,
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
    pub fn column(self, name: impl AsRef<str>) -> ColumnBuilder<'ui> {
        ColumnBuilder::new(self, name)
    }

    /// Replace columns with provided list
    pub fn columns<Name: AsRef<str>>(
        mut self,
        cols: impl IntoIterator<Item = TableColumnSetup<Name>>,
    ) -> Self {
        self.columns.clear();
        for c in cols {
            self.columns.push(TableColumnSetup {
                name: c.name.as_ref().to_string(),
                flags: c.flags,
                init_width_or_weight: c.init_width_or_weight,
                user_id: c.user_id,
            });
        }
        self
    }

    /// Add a single column setup
    pub fn add_column<Name: AsRef<str>>(mut self, col: TableColumnSetup<Name>) -> Self {
        self.columns.push(TableColumnSetup {
            name: col.name.as_ref().to_string(),
            flags: col.flags,
            init_width_or_weight: col.init_width_or_weight,
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
        let Some(token) = self.ui.begin_table_with_sizing(
            &self.id,
            self.columns.len(),
            self.flags,
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
                self.ui.table_setup_column(
                    &col.name,
                    col.flags,
                    col.init_width_or_weight,
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
    name: String,
    flags: TableColumnFlags,
    init_width_or_weight: f32,
    user_id: u32,
}

impl<'ui> ColumnBuilder<'ui> {
    fn new(parent: TableBuilder<'ui>, name: impl AsRef<str>) -> Self {
        Self {
            parent,
            name: name.as_ref().to_string(),
            flags: TableColumnFlags::NONE,
            init_width_or_weight: 0.0,
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
        self.init_width_or_weight = width;
        self
    }

    /// Alias of `width()` to express stretch weights.
    pub fn weight(self, weight: f32) -> Self {
        self.width(weight)
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
            init_width_or_weight: self.init_width_or_weight,
            user_id: self.user_id,
        });
        self.parent
    }
}

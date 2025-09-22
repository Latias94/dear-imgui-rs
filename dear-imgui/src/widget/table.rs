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
            sys::ImGui_BeginTable(
                str_id_ptr,
                column_count as i32,
                flags.bits(),
                &outer_size_vec,
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
            sys::ImGui_TableSetupColumn(label_ptr, flags.bits(), init_width_or_weight, user_id);
        }
    }

    /// Submit all headers cells based on data provided to TableSetupColumn() + submit context menu
    pub fn table_headers_row(&self) {
        unsafe {
            sys::ImGui_TableHeadersRow();
        }
    }

    /// Append into the next column (or first column of next row if currently in last column)
    pub fn table_next_column(&self) -> bool {
        unsafe { sys::ImGui_TableNextColumn() }
    }

    /// Append into the specified column
    pub fn table_set_column_index(&self, column_n: i32) -> bool {
        unsafe { sys::ImGui_TableSetColumnIndex(column_n) }
    }

    /// Append into the next row
    pub fn table_next_row(&self) {
        self.table_next_row_with_flags(TableRowFlags::NONE, 0.0);
    }

    /// Append into the next row with flags and minimum height
    pub fn table_next_row_with_flags(&self, flags: TableRowFlags, min_row_height: f32) {
        unsafe {
            sys::ImGui_TableNextRow(flags.bits(), min_row_height);
        }
    }

    /// Freeze columns/rows so they stay visible when scrolling.
    #[doc(alias = "TableSetupScrollFreeze")]
    pub fn table_setup_scroll_freeze(&self, frozen_cols: i32, frozen_rows: i32) {
        unsafe { sys::ImGui_TableSetupScrollFreeze(frozen_cols, frozen_rows) }
    }

    /// Submit one header cell at current column position.
    #[doc(alias = "TableHeader")]
    pub fn table_header(&self, label: impl AsRef<str>) {
        let label_ptr = self.scratch_txt(label);
        unsafe { sys::ImGui_TableHeader(label_ptr) }
    }

    /// Return columns count.
    #[doc(alias = "TableGetColumnCount")]
    pub fn table_get_column_count(&self) -> i32 {
        unsafe { sys::ImGui_TableGetColumnCount() }
    }

    /// Return current column index.
    #[doc(alias = "TableGetColumnIndex")]
    pub fn table_get_column_index(&self) -> i32 {
        unsafe { sys::ImGui_TableGetColumnIndex() }
    }

    /// Return current row index.
    #[doc(alias = "TableGetRowIndex")]
    pub fn table_get_row_index(&self) -> i32 {
        unsafe { sys::ImGui_TableGetRowIndex() }
    }

    /// Return the name of a column by index.
    #[doc(alias = "TableGetColumnName")]
    pub fn table_get_column_name(&self, column_n: i32) -> &str {
        unsafe {
            let ptr = sys::ImGui_TableGetColumnName(column_n);
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
        unsafe { TableColumnFlags::from_bits_truncate(sys::ImGui_TableGetColumnFlags(column_n)) }
    }

    /// Enable/disable a column by index.
    #[doc(alias = "TableSetColumnEnabled")]
    pub fn table_set_column_enabled(&self, column_n: i32, enabled: bool) {
        unsafe { sys::ImGui_TableSetColumnEnabled(column_n, enabled) }
    }

    /// Return hovered column index, or -1 when none.
    #[doc(alias = "TableGetHoveredColumn")]
    pub fn table_get_hovered_column(&self) -> i32 {
        unsafe { sys::ImGui_TableGetHoveredColumn() }
    }

    /// Set column width (for fixed-width columns).
    #[doc(alias = "TableSetColumnWidth")]
    pub fn table_set_column_width(&self, column_n: i32, width: f32) {
        unsafe { sys::ImGui_TableSetColumnWidth(column_n, width) }
    }

    /// Set a table background color target.
    ///
    /// Color is provided as `ImU32` (e.g. from `u32` RGBA).
    #[doc(alias = "TableSetBgColor")]
    pub fn table_set_bg_color_u32(&self, target: TableBgTarget, color: u32, column_n: i32) {
        unsafe { sys::ImGui_TableSetBgColor(target as i32, color, column_n) }
    }

    /// Return hovered row index, or -1 when none.
    #[doc(alias = "TableGetHoveredRow")]
    pub fn table_get_hovered_row(&self) -> i32 {
        unsafe { sys::ImGui_TableGetHoveredRow() }
    }

    /// Header row height in pixels.
    #[doc(alias = "TableGetHeaderRowHeight")]
    pub fn table_get_header_row_height(&self) -> f32 {
        unsafe { sys::ImGui_TableGetHeaderRowHeight() }
    }

    /// Set sort direction for a column. Optionally append to existing sort specs (multi-sort).
    #[doc(alias = "TableSetColumnSortDirection")]
    pub fn table_set_column_sort_direction(
        &self,
        column_n: i32,
        dir: SortDirection,
        append_to_sort_specs: bool,
    ) {
        unsafe { sys::ImGui_TableSetColumnSortDirection(column_n, dir as u8, append_to_sort_specs) }
    }

    /// Get current table sort specifications, if any.
    /// When non-None and `is_dirty()` is true, the application should sort its data and
    /// then call `clear_dirty()`.
    #[doc(alias = "TableGetSortSpecs")]
    pub fn table_get_sort_specs(&self) -> Option<TableSortSpecs<'_>> {
        unsafe {
            let ptr = sys::ImGui_TableGetSortSpecs();
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
    None = sys::ImGuiSortDirection_None,
    Ascending = sys::ImGuiSortDirection_Ascending,
    Descending = sys::ImGuiSortDirection_Descending,
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
    pub unsafe fn from_raw(raw: *mut sys::ImGuiTableSortSpecs) -> Self {
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
            sys::ImGui_EndTable();
        }
    }
}

// ============================================================================
// Additional table convenience APIs
// ============================================================================

impl Ui {
    /// Maximum label width used for angled headers (when enabled in style/options).
    #[doc(alias = "TableGetHeaderAngledMaxLabelWidth")]
    pub fn table_get_header_angled_max_label_width(&self) -> f32 {
        unsafe { sys::ImGui_TableGetHeaderAngledMaxLabelWidth() }
    }

    /// Push background draw channel for the current table and return a token to pop it.
    #[doc(alias = "TablePushBackgroundChannel")]
    pub fn table_push_background_channel(&self) {
        unsafe { sys::ImGui_TablePushBackgroundChannel() }
    }

    /// Pop background draw channel for the current table.
    #[doc(alias = "TablePopBackgroundChannel")]
    pub fn table_pop_background_channel(&self) {
        unsafe { sys::ImGui_TablePopBackgroundChannel() }
    }

    /// Push column draw channel for the given column index and return a token to pop it.
    #[doc(alias = "TablePushColumnChannel")]
    pub fn table_push_column_channel(&self, column_n: i32) {
        unsafe { sys::ImGui_TablePushColumnChannel(column_n) }
    }

    /// Pop column draw channel.
    #[doc(alias = "TablePopColumnChannel")]
    pub fn table_pop_column_channel(&self) {
        unsafe { sys::ImGui_TablePopColumnChannel() }
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
}

// (Optional) RAII versions could be added later if desirable

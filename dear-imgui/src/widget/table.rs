use crate::sys;
use crate::ui::Ui;
use crate::widget::{TableColumnFlags, TableFlags};

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
                    column.flags.clone(),
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
}

bitflags::bitflags! {
    /// Flags for table rows
    #[repr(transparent)]
    pub struct TableRowFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Identify header row (set default background color + width of all columns)
        const HEADERS = sys::ImGuiTableRowFlags_Headers;
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

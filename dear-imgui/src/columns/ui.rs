use crate::sys;
use crate::{Id, Ui};

use super::counts::{column_count_from_i32, columns_count_to_i32};
use super::flags::{OldColumnFlags, validate_old_column_flags};
use super::index::{OldColumnIndex, OldColumnOffsetRef, OldColumnRef};
use super::numeric::assert_non_negative_f32;
use super::resolve::{
    assert_valid_column_in, resolve_column_offset_query_ref, resolve_column_offset_ref,
    resolve_column_query_ref,
};
use super::state::{assert_current_columns, assert_no_current_columns, current_columns};
use super::token::ColumnsToken;

/// # Columns
impl Ui {
    /// Creates columns layout.
    ///
    /// # Arguments
    /// * `count` - Number of columns (must be >= 1)
    /// * `id` - Optional ID for the columns (can be empty string)
    /// * `border` - Whether to draw borders between columns
    #[doc(alias = "Columns")]
    pub fn columns(&self, count: usize, id: impl AsRef<str>, border: bool) {
        let count = columns_count_to_i32(count, "Ui::columns()");
        unsafe { sys::igColumns(count, self.scratch_txt(id), border) }
    }

    /// Begin columns layout with advanced flags.
    ///
    /// # Arguments
    /// * `id` - ID for the columns
    /// * `count` - Number of columns (must be >= 1)
    /// * `flags` - Column flags
    #[doc(alias = "BeginColumns")]
    pub fn begin_columns(&self, id: impl AsRef<str>, count: usize, flags: OldColumnFlags) {
        let count = columns_count_to_i32(count, "Ui::begin_columns()");
        validate_old_column_flags("Ui::begin_columns()", flags);
        assert_no_current_columns("Ui::begin_columns()");
        unsafe { sys::igBeginColumns(self.scratch_txt(id), count, flags.bits()) }
    }

    /// Begin columns layout with advanced flags and return a token that ends columns on drop.
    #[doc(alias = "BeginColumns")]
    pub fn begin_columns_token(
        &self,
        id: impl AsRef<str>,
        count: usize,
        flags: OldColumnFlags,
    ) -> ColumnsToken<'_> {
        self.begin_columns(id, count, flags);
        ColumnsToken { ui: self }
    }

    /// End columns layout.
    #[doc(alias = "EndColumns")]
    pub fn end_columns(&self) {
        assert_current_columns("Ui::end_columns()");
        unsafe { sys::igEndColumns() }
    }

    /// Switches to the next column.
    ///
    /// If the current row is finished, switches to first column of the next row
    #[doc(alias = "NextColumn")]
    pub fn next_column(&self) {
        unsafe { sys::igNextColumn() }
    }

    /// Returns the index of the current column
    #[doc(alias = "GetColumnIndex")]
    pub fn current_column_index(&self) -> OldColumnIndex {
        OldColumnIndex::from_i32(
            unsafe { sys::igGetColumnIndex() },
            "Ui::current_column_index()",
        )
    }

    /// Returns the width of the current column (in pixels)
    #[doc(alias = "GetColumnWidth")]
    pub fn current_column_width(&self) -> f32 {
        unsafe { sys::igGetColumnWidth(-1) }
    }

    /// Returns the width of the given column (in pixels)
    #[doc(alias = "GetColumnWidth")]
    pub fn column_width(&self, column: impl Into<OldColumnRef>) -> f32 {
        let column_index = resolve_column_query_ref(column.into(), "Ui::column_width()");
        unsafe { sys::igGetColumnWidth(column_index) }
    }

    /// Sets the width of the current column (in pixels)
    #[doc(alias = "SetColumnWidth")]
    pub fn set_current_column_width(&self, width: f32) {
        assert_current_columns("Ui::set_current_column_width()");
        assert_non_negative_f32("Ui::set_current_column_width()", "width", width);
        unsafe { sys::igSetColumnWidth(-1, width) };
    }

    /// Sets the width of the given column (in pixels)
    #[doc(alias = "SetColumnWidth")]
    pub fn set_column_width(&self, column: impl Into<OldColumnIndex>, width: f32) {
        let columns = assert_current_columns("Ui::set_column_width()");
        let column_index = assert_valid_column_in(columns, column.into(), "Ui::set_column_width()");
        assert_non_negative_f32("Ui::set_column_width()", "width", width);
        unsafe { sys::igSetColumnWidth(column_index, width) };
    }

    /// Returns the offset of the current column (in pixels from the left side of the content region)
    #[doc(alias = "GetColumnOffset")]
    pub fn current_column_offset(&self) -> f32 {
        unsafe { sys::igGetColumnOffset(-1) }
    }

    /// Returns the offset of the given column (in pixels from the left side of the content region)
    #[doc(alias = "GetColumnOffset")]
    pub fn column_offset(&self, offset: impl Into<OldColumnOffsetRef>) -> f32 {
        let column_index = resolve_column_offset_query_ref(offset.into(), "Ui::column_offset()");
        unsafe { sys::igGetColumnOffset(column_index) }
    }

    /// Sets the offset of the current column (in pixels from the left side of the content region)
    #[doc(alias = "SetColumnOffset")]
    pub fn set_current_column_offset(&self, offset_x: f32) {
        assert_current_columns("Ui::set_current_column_offset()");
        assert_non_negative_f32("Ui::set_current_column_offset()", "offset_x", offset_x);
        unsafe { sys::igSetColumnOffset(-1, offset_x) };
    }

    /// Sets the offset of the given column (in pixels from the left side of the content region)
    #[doc(alias = "SetColumnOffset")]
    pub fn set_column_offset(&self, offset: impl Into<OldColumnOffsetRef>, offset_x: f32) {
        let column_index = resolve_column_offset_ref(offset.into(), "Ui::set_column_offset()");
        assert_non_negative_f32("Ui::set_column_offset()", "offset_x", offset_x);
        unsafe { sys::igSetColumnOffset(column_index, offset_x) };
    }

    /// Returns the current amount of columns
    #[doc(alias = "GetColumnsCount")]
    pub fn column_count(&self) -> usize {
        column_count_from_i32(unsafe { sys::igGetColumnsCount() }, "Ui::column_count()")
    }

    // ============================================================================
    // Advanced column utilities
    // ============================================================================

    /// Push column clip rect for the given column index.
    /// This is useful for custom drawing within columns.
    #[doc(alias = "PushColumnClipRect")]
    pub fn push_column_clip_rect(&self, column: impl Into<OldColumnIndex>) {
        let columns = assert_current_columns("Ui::push_column_clip_rect()");
        let column_index =
            assert_valid_column_in(columns, column.into(), "Ui::push_column_clip_rect()");
        unsafe { sys::igPushColumnClipRect(column_index) }
    }

    /// Push columns background for drawing.
    #[doc(alias = "PushColumnsBackground")]
    pub fn push_columns_background(&self) {
        assert_current_columns("Ui::push_columns_background()");
        unsafe { sys::igPushColumnsBackground() }
    }

    /// Pop columns background.
    #[doc(alias = "PopColumnsBackground")]
    pub fn pop_columns_background(&self) {
        assert_current_columns("Ui::pop_columns_background()");
        unsafe { sys::igPopColumnsBackground() }
    }

    /// Get columns ID for the given string ID and count.
    #[doc(alias = "GetColumnsID")]
    pub fn get_columns_id(&self, str_id: impl AsRef<str>, count: usize) -> Id {
        let count = columns_count_to_i32(count, "Ui::get_columns_id()");
        unsafe { Id::from(sys::igGetColumnsID(self.scratch_txt(str_id), count)) }
    }

    // ============================================================================
    // Column state utilities
    // ============================================================================

    /// Check if any column in the current legacy columns set is being resized.
    ///
    /// Returns `false` when the current window is not inside a legacy columns set.
    pub fn is_any_column_resizing(&self) -> bool {
        unsafe {
            let window = sys::igGetCurrentWindowRead();
            if window.is_null() {
                return false;
            }

            let columns = (*window).DC.CurrentColumns;
            if columns.is_null() {
                return false;
            }

            (*columns).IsBeingResized
        }
    }

    /// Get the total width of all columns.
    pub fn get_columns_total_width(&self) -> f32 {
        if current_columns().is_null() {
            return self.current_column_width();
        }

        let count = self.column_count();

        let mut total_width = 0.0;
        for i in 0..count {
            total_width += self.column_width(i);
        }
        total_width
    }

    /// Set all columns to equal width.
    pub fn set_columns_equal_width(&self) {
        let count = self.column_count();
        if count <= 1 {
            return;
        }

        let total_width = self.get_columns_total_width();
        let equal_width = total_width / count as f32;

        for i in 0..count {
            self.set_column_width(i, equal_width);
        }
    }

    /// Get column width as a percentage of total width.
    pub fn get_column_width_percentage(&self, column: impl Into<OldColumnRef>) -> f32 {
        let total_width = self.get_columns_total_width();
        if total_width <= 0.0 {
            return 0.0;
        }

        let column_width = self.column_width(column);
        (column_width / total_width) * 100.0
    }

    /// Set column width as a percentage of total width.
    pub fn set_column_width_percentage(&self, column: impl Into<OldColumnIndex>, percentage: f32) {
        assert_non_negative_f32(
            "Ui::set_column_width_percentage()",
            "percentage",
            percentage,
        );
        let total_width = self.get_columns_total_width();
        if total_width <= 0.0 {
            return;
        }

        let new_width = (total_width * percentage) / 100.0;
        self.set_column_width(column, new_width);
    }
}

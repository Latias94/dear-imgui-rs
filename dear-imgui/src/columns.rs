#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]
use crate::Ui;
use crate::sys;
use bitflags::bitflags;

bitflags! {
    /// Flags for old columns system
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct OldColumnFlags: i32 {
        /// No flags
        const NONE = sys::ImGuiOldColumnFlags_None as i32;
        /// Disable column dividers
        const NO_BORDER = sys::ImGuiOldColumnFlags_NoBorder as i32;
        /// Disable resizing columns by dragging dividers
        const NO_RESIZE = sys::ImGuiOldColumnFlags_NoResize as i32;
        /// Disable column width preservation when the total width changes
        const NO_PRESERVE_WIDTHS = sys::ImGuiOldColumnFlags_NoPreserveWidths as i32;
        /// Disable forcing columns to fit within window
        const NO_FORCE_WITHIN_WINDOW = sys::ImGuiOldColumnFlags_NoForceWithinWindow as i32;
        /// Restore pre-1.51 behavior of extending the parent window contents size
        const GROW_PARENT_CONTENTS_SIZE = sys::ImGuiOldColumnFlags_GrowParentContentsSize as i32;
    }
}

impl Default for OldColumnFlags {
    fn default() -> Self {
        OldColumnFlags::NONE
    }
}

/// # Columns
impl Ui {
    /// Creates columns layout.
    ///
    /// # Arguments
    /// * `count` - Number of columns (must be >= 1)
    /// * `id` - Optional ID for the columns (can be empty string)
    /// * `border` - Whether to draw borders between columns
    #[doc(alias = "Columns")]
    pub fn columns(&self, count: i32, id: impl AsRef<str>, border: bool) {
        unsafe { sys::igColumns(count, self.scratch_txt(id), border) }
    }

    /// Begin columns layout with advanced flags.
    ///
    /// # Arguments
    /// * `id` - ID for the columns
    /// * `count` - Number of columns (must be >= 1)
    /// * `flags` - Column flags
    #[doc(alias = "BeginColumns")]
    pub fn begin_columns(&self, id: impl AsRef<str>, count: i32, flags: OldColumnFlags) {
        unsafe { sys::igBeginColumns(self.scratch_txt(id), count, flags.bits()) }
    }

    /// End columns layout.
    #[doc(alias = "EndColumns")]
    pub fn end_columns(&self) {
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
    pub fn current_column_index(&self) -> i32 {
        unsafe { sys::igGetColumnIndex() }
    }

    /// Returns the width of the current column (in pixels)
    #[doc(alias = "GetColumnWidth")]
    pub fn current_column_width(&self) -> f32 {
        unsafe { sys::igGetColumnWidth(-1) }
    }

    /// Returns the width of the given column (in pixels)
    #[doc(alias = "GetColumnWidth")]
    pub fn column_width(&self, column_index: i32) -> f32 {
        unsafe { sys::igGetColumnWidth(column_index) }
    }

    /// Sets the width of the current column (in pixels)
    #[doc(alias = "SetColumnWidth")]
    pub fn set_current_column_width(&self, width: f32) {
        unsafe { sys::igSetColumnWidth(-1, width) };
    }

    /// Sets the width of the given column (in pixels)
    #[doc(alias = "SetColumnWidth")]
    pub fn set_column_width(&self, column_index: i32, width: f32) {
        unsafe { sys::igSetColumnWidth(column_index, width) };
    }

    /// Returns the offset of the current column (in pixels from the left side of the content region)
    #[doc(alias = "GetColumnOffset")]
    pub fn current_column_offset(&self) -> f32 {
        unsafe { sys::igGetColumnOffset(-1) }
    }

    /// Returns the offset of the given column (in pixels from the left side of the content region)
    #[doc(alias = "GetColumnOffset")]
    pub fn column_offset(&self, column_index: i32) -> f32 {
        unsafe { sys::igGetColumnOffset(column_index) }
    }

    /// Sets the offset of the current column (in pixels from the left side of the content region)
    #[doc(alias = "SetColumnOffset")]
    pub fn set_current_column_offset(&self, offset_x: f32) {
        unsafe { sys::igSetColumnOffset(-1, offset_x) };
    }

    /// Sets the offset of the given column (in pixels from the left side of the content region)
    #[doc(alias = "SetColumnOffset")]
    pub fn set_column_offset(&self, column_index: i32, offset_x: f32) {
        unsafe { sys::igSetColumnOffset(column_index, offset_x) };
    }

    /// Returns the current amount of columns
    #[doc(alias = "GetColumnsCount")]
    pub fn column_count(&self) -> i32 {
        unsafe { sys::igGetColumnsCount() }
    }

    // ============================================================================
    // Advanced column utilities
    // ============================================================================

    /// Push column clip rect for the given column index.
    /// This is useful for custom drawing within columns.
    #[doc(alias = "PushColumnClipRect")]
    pub fn push_column_clip_rect(&self, column_index: i32) {
        unsafe { sys::igPushColumnClipRect(column_index) }
    }

    /// Push columns background for drawing.
    #[doc(alias = "PushColumnsBackground")]
    pub fn push_columns_background(&self) {
        unsafe { sys::igPushColumnsBackground() }
    }

    /// Pop columns background.
    #[doc(alias = "PopColumnsBackground")]
    pub fn pop_columns_background(&self) {
        unsafe { sys::igPopColumnsBackground() }
    }

    /// Get columns ID for the given string ID and count.
    #[doc(alias = "GetColumnsID")]
    pub fn get_columns_id(&self, str_id: impl AsRef<str>, count: i32) -> u32 {
        unsafe { sys::igGetColumnsID(self.scratch_txt(str_id), count) }
    }

    // ============================================================================
    // Column state utilities
    // ============================================================================

    /// Check if any column is being resized.
    /// Note: This is a placeholder implementation as the underlying C++ function is not available
    pub fn is_any_column_resizing(&self) -> bool {
        // TODO: Implement when the proper C++ binding is available
        // The ImGui_GetCurrentWindow function is not available in our bindings
        false
    }

    /// Get the total width of all columns.
    pub fn get_columns_total_width(&self) -> f32 {
        let count = self.column_count();
        if count <= 0 {
            return 0.0;
        }

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
    pub fn get_column_width_percentage(&self, column_index: i32) -> f32 {
        let total_width = self.get_columns_total_width();
        if total_width <= 0.0 {
            return 0.0;
        }

        let column_width = self.column_width(column_index);
        (column_width / total_width) * 100.0
    }

    /// Set column width as a percentage of total width.
    pub fn set_column_width_percentage(&self, column_index: i32, percentage: f32) {
        let total_width = self.get_columns_total_width();
        if total_width <= 0.0 {
            return;
        }

        let new_width = (total_width * percentage) / 100.0;
        self.set_column_width(column_index, new_width);
    }
}

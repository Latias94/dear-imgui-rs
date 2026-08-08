use crate::draw::ImColor32;
use crate::internal::len_i32;
use crate::sys;
use crate::ui::Ui;
use crate::widget::table::{
    TableColumnIndex, TableContextMenuTarget, assert_current_table, assert_current_table_cell,
    assert_non_negative_finite_f32, assert_table_before_first_row, assert_valid_table_column,
    assert_valid_table_column_in,
};

use super::tokens::TableChannelGuard;

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
impl Ui {
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
            self.assert_no_active_table_channel("Ui::table_angled_headers_row()");
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
            self.assert_no_active_table_channel(
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
    /// Panics if there is no current table cell or another table channel is active.
    #[doc(
        alias = "TablePushBackgroundChannel",
        alias = "TablePopBackgroundChannel"
    )]
    pub fn with_table_background_channel<R>(&self, f: impl FnOnce() -> R) -> R {
        self.run_with_bound_context(|| {
            assert_current_table_cell("Ui::with_table_background_channel()");
            self.assert_no_active_table_channel("Ui::with_table_background_channel()");
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
    /// Panics if there is no current table cell, `column` is invalid, or another table channel is
    /// active.
    #[doc(alias = "TablePushColumnChannel", alias = "TablePopColumnChannel")]
    pub fn with_table_column_channel<R>(
        &self,
        column: impl Into<TableColumnIndex>,
        f: impl FnOnce() -> R,
    ) -> R {
        let column = column.into();
        self.run_with_bound_context(|| {
            assert_current_table_cell("Ui::with_table_column_channel()");
            self.assert_no_active_table_channel("Ui::with_table_column_channel()");
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

use crate::draw::ImColor32;
use crate::internal::len_i32;
use crate::sys;
use crate::ui::Ui;
use crate::widget::table::{
    TableBackgroundChannelToken, TableColumnChannelToken, TableColumnIndex, TableContextMenuTarget,
    assert_current_table, assert_current_table_cell, assert_valid_table_column,
    assert_valid_table_column_in,
};

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
    /// Maximum label width used for angled headers (when enabled in style/options).
    #[doc(alias = "TableGetHeaderAngledMaxLabelWidth")]
    pub fn table_get_header_angled_max_label_width(&self) -> f32 {
        self.run_with_bound_context(|| unsafe { sys::igTableGetHeaderAngledMaxLabelWidth() })
    }

    /// Submit angled headers row (requires style/flags enabling angled headers).
    #[doc(alias = "TableAngledHeadersRow")]
    pub fn table_angled_headers_row(&self) {
        self.run_with_bound_context(|| unsafe { sys::igTableAngledHeadersRow() });
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
            self.run_with_bound_context(|| unsafe { sys::igTableAngledHeadersRow() });
            return;
        }
        let count = len_i32(
            "Ui::table_angled_headers_row_ex_with_data()",
            "headers",
            headers.len(),
        );
        let mut data: Vec<sys::ImGuiTableHeaderData> = Vec::with_capacity(headers.len());
        self.run_with_bound_context(|| {
            let table = assert_current_table("Ui::table_angled_headers_row_ex_with_data()");
            for h in headers {
                assert_valid_table_column_in(
                    table,
                    h.index,
                    "Ui::table_angled_headers_row_ex_with_data()",
                );
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

    /// Push background draw channel for the current table and return a token to pop it.
    #[must_use = "dropping the token pops the table background draw channel immediately"]
    #[doc(alias = "TablePushBackgroundChannel")]
    pub fn table_background_channel(&self) -> TableBackgroundChannelToken<'_> {
        self.run_with_bound_context(|| {
            assert_current_table_cell("Ui::table_background_channel()");
            unsafe { sys::igTablePushBackgroundChannel() };
        });
        TableBackgroundChannelToken::new(self)
    }

    /// Push column draw channel for the given column index and return a token to pop it.
    #[must_use = "dropping the token pops the table column draw channel immediately"]
    #[doc(alias = "TablePushColumnChannel")]
    pub fn table_column_channel(
        &self,
        column: impl Into<TableColumnIndex>,
    ) -> TableColumnChannelToken<'_> {
        let column = column.into();
        self.run_with_bound_context(|| {
            assert_current_table_cell("Ui::table_column_channel()");
            let column_n = assert_valid_table_column(column, "Ui::table_column_channel()");
            unsafe { sys::igTablePushColumnChannel(column_n) };
        });
        TableColumnChannelToken::new(self)
    }

    /// Run a closure after pushing table background channel (auto-pop on return).
    pub fn with_table_background_channel<R>(&self, f: impl FnOnce() -> R) -> R {
        let _token = self.table_background_channel();
        f()
    }

    /// Run a closure after pushing a table column channel (auto-pop on return).
    pub fn with_table_column_channel<R>(
        &self,
        column: impl Into<TableColumnIndex>,
        f: impl FnOnce() -> R,
    ) -> R {
        let _token = self.table_column_channel(column);
        f()
    }

    /// Open the table context menu for the current/default column.
    #[doc(alias = "TableOpenContextMenu")]
    pub fn table_open_context_menu(&self, target: impl Into<TableContextMenuTarget>) {
        let target = target.into();
        self.run_with_bound_context(|| {
            let column_n = match target {
                TableContextMenuTarget::CurrentColumn => -1,
                TableContextMenuTarget::Column(index) => {
                    index.into_i32("Ui::table_open_context_menu()")
                }
                TableContextMenuTarget::Table => {
                    let table = assert_current_table("Ui::table_open_context_menu()");
                    unsafe { (*table).ColumnsCount }
                }
            };
            unsafe { sys::igTableOpenContextMenu(column_n) }
        });
    }
}

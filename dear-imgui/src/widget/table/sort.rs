use crate::sys;
use crate::ui::Ui;
use crate::widget::table::{TableColumnIndex, TableColumnUserData};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

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
    source_context: *mut sys::ImGuiContext,
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
        let (dirty, specs) = unsafe { copy_table_sort_specs(raw) };
        let source_scope = ui
            .current_native_scope()
            .table()
            .expect("TableSortSpecs::from_raw() requires a current table");
        Self {
            specs,
            dirty,
            source_context: ui.context_raw(),
            source_scope,
        }
    }

    /// Whether the specs are marked dirty by dear imgui (you should resort your data).
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clear the native dirty flag after applying this snapshot.
    ///
    /// # Panics
    ///
    /// Panics if `ui` belongs to another Context or if the exact frame, window `Begin`, and table
    /// instance that produced the snapshot is no longer current. The snapshot data itself remains
    /// readable after any of those conditions; only native acknowledgement is table-scoped.
    pub fn clear_dirty(&mut self, ui: &Ui) {
        if !self.dirty {
            return;
        }
        assert!(
            std::ptr::eq(ui.context_raw(), self.source_context),
            "TableSortSpecs::clear_dirty() requires the Ui that produced this snapshot"
        );
        ui.run_with_bound_context(|| unsafe {
            assert_eq!(
                ui.current_native_scope().table(),
                Some(self.source_scope),
                "TableSortSpecs::clear_dirty() source table is no longer current"
            );
            let raw = sys::igTableGetSortSpecs();
            assert!(
                !raw.is_null(),
                "TableSortSpecs::clear_dirty() current table has no sort specifications"
            );
            (*raw).SpecsDirty = false;
        });
        self.dirty = false;
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

pub(super) unsafe fn copy_table_sort_specs(
    raw: *mut sys::ImGuiTableSortSpecs,
) -> (bool, Box<[TableColumnSortSpec]>) {
    assert!(!raw.is_null(), "table sort specs pointer must not be null");
    let count = usize::try_from(unsafe { (*raw).SpecsCount })
        .expect("Dear ImGui returned a negative table sort specification count");
    let source = unsafe { (*raw).Specs };
    assert!(
        count == 0 || !source.is_null(),
        "Dear ImGui returned null table sort specification data for a non-empty snapshot"
    );
    let mut specs = Vec::with_capacity(count);
    for index in 0..count {
        let spec = unsafe { &*source.add(index) };
        let direction = match spec.SortDirection as u8 {
            value if value == sys::ImGuiSortDirection_Ascending as u8 => SortDirection::Ascending,
            value if value == sys::ImGuiSortDirection_Descending as u8 => SortDirection::Descending,
            _ => SortDirection::None,
        };
        specs.push(TableColumnSortSpec {
            column_user_data: TableColumnUserData::new(spec.ColumnUserID),
            column_index: TableColumnIndex::from_imgui_column_idx(
                spec.ColumnIndex,
                "TableSortSpecs::from_raw()",
            ),
            sort_order: spec.SortOrder,
            sort_direction: direction,
        });
    }
    (unsafe { (*raw).SpecsDirty }, specs.into_boxed_slice())
}

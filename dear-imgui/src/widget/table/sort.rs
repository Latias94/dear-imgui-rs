use crate::ui::Ui;
use crate::widget::table::{TableColumnIndex, optional_user_id_from_raw};
use crate::{Id, sys};
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
#[derive(Copy, Clone, Debug)]
pub struct TableColumnSortSpec {
    pub column_user_id: Option<Id>,
    pub column_index: TableColumnIndex,
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
                column_user_id: optional_user_id_from_raw(spec.ColumnUserID),
                column_index: TableColumnIndex::from_imgui_column_idx(
                    spec.ColumnIndex,
                    "TableSortSpecsIter::next()",
                ),
                sort_order: spec.SortOrder,
                sort_direction: dir,
            })
        }
    }
}

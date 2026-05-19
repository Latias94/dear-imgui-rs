use crate::sys;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

pub(crate) const TABLE_MAX_COLUMNS: usize = 512;

/// Concrete zero-based table column index.
///
/// This represents a real table column only. Dear ImGui's `-1` current/default
/// sentinel is represented by [`TableColumnRef::Current`] instead.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TableColumnIndex(usize);

impl TableColumnIndex {
    /// The first table column.
    pub const ZERO: Self = Self(0);

    /// Create a table column index from a Rust `usize`.
    #[inline]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Return the zero-based Rust index.
    #[inline]
    pub const fn get(self) -> usize {
        self.0
    }

    #[inline]
    pub(crate) fn into_i32(self, caller: &str) -> i32 {
        i32::try_from(self.0).unwrap_or_else(|_| {
            panic!("{caller} column index exceeded Dear ImGui's i32 range");
        })
    }

    #[inline]
    pub(crate) fn into_imgui_column_idx(self, caller: &str) -> sys::ImGuiTableColumnIdx {
        sys::ImGuiTableColumnIdx::try_from(self.0).unwrap_or_else(|_| {
            panic!("{caller} column index exceeded Dear ImGui's ImGuiTableColumnIdx range");
        })
    }

    #[inline]
    pub(crate) fn from_imgui_column_idx(raw: sys::ImGuiTableColumnIdx, caller: &str) -> Self {
        assert!(raw >= 0, "{caller} returned a negative table column index");
        Self(
            usize::try_from(raw)
                .expect("non-negative Dear ImGui table column index must fit usize"),
        )
    }

    #[inline]
    pub(crate) fn from_i32(raw: i32, caller: &str) -> Self {
        assert!(raw >= 0, "{caller} returned a negative table column index");
        Self(usize::try_from(raw).expect("non-negative table column index must fit usize"))
    }
}

impl From<usize> for TableColumnIndex {
    #[inline]
    fn from(index: usize) -> Self {
        Self::new(index)
    }
}

impl From<TableColumnIndex> for usize {
    #[inline]
    fn from(index: TableColumnIndex) -> Self {
        index.get()
    }
}

impl PartialEq<usize> for TableColumnIndex {
    #[inline]
    fn eq(&self, other: &usize) -> bool {
        self.get() == *other
    }
}

impl PartialEq<TableColumnIndex> for usize {
    #[inline]
    fn eq(&self, other: &TableColumnIndex) -> bool {
        *self == other.get()
    }
}

/// Table column selector for APIs that accept Dear ImGui's current-column sentinel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TableColumnRef {
    /// Use the table's current column.
    #[default]
    Current,
    /// Use a concrete table column index.
    Index(TableColumnIndex),
}

impl TableColumnRef {
    /// Current table column.
    pub const CURRENT: Self = Self::Current;

    /// Select a concrete table column.
    #[inline]
    pub const fn index(index: TableColumnIndex) -> Self {
        Self::Index(index)
    }
}

impl From<TableColumnIndex> for TableColumnRef {
    #[inline]
    fn from(index: TableColumnIndex) -> Self {
        Self::Index(index)
    }
}

impl From<usize> for TableColumnRef {
    #[inline]
    fn from(index: usize) -> Self {
        Self::Index(TableColumnIndex::new(index))
    }
}

/// Result of [`Ui::table_get_hovered_column`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TableHoveredColumn {
    /// The table is not hovered.
    None,
    /// A concrete table column is hovered.
    Column(TableColumnIndex),
    /// The unused space after the right-most visible column is hovered.
    UnusedSpace,
}

/// Concrete zero-based table row index.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TableRowIndex(usize);

impl TableRowIndex {
    /// The first table row.
    pub const ZERO: Self = Self(0);

    /// Create a table row index from a Rust `usize`.
    #[inline]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Return the zero-based Rust index.
    #[inline]
    pub const fn get(self) -> usize {
        self.0
    }

    #[inline]
    pub(crate) fn from_i32(raw: i32, caller: &str) -> Self {
        assert!(raw >= 0, "{caller} returned a negative table row index");
        Self(usize::try_from(raw).expect("non-negative table row index must fit usize"))
    }
}

impl From<usize> for TableRowIndex {
    #[inline]
    fn from(index: usize) -> Self {
        Self::new(index)
    }
}

impl From<TableRowIndex> for usize {
    #[inline]
    fn from(index: TableRowIndex) -> Self {
        index.get()
    }
}

impl PartialEq<usize> for TableRowIndex {
    #[inline]
    fn eq(&self, other: &usize) -> bool {
        self.get() == *other
    }
}

impl PartialEq<TableRowIndex> for usize {
    #[inline]
    fn eq(&self, other: &TableRowIndex) -> bool {
        *self == other.get()
    }
}

/// Result of [`Ui::table_get_hovered_row`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TableHoveredRow {
    /// The table is not hovered.
    None,
    /// A table row index is hovered.
    Row(TableRowIndex),
}

impl TableHoveredRow {
    /// Return the hovered concrete row, if any.
    #[inline]
    pub const fn row(self) -> Option<TableRowIndex> {
        match self {
            Self::Row(index) => Some(index),
            Self::None => None,
        }
    }
}

impl TableHoveredColumn {
    /// Return the hovered concrete column, if any.
    #[inline]
    pub const fn column(self) -> Option<TableColumnIndex> {
        match self {
            Self::Column(index) => Some(index),
            Self::None | Self::UnusedSpace => None,
        }
    }
}

/// Target column for opening a table context menu.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TableContextMenuTarget {
    /// Use Dear ImGui's default: current column when inside a column, otherwise table-level.
    #[default]
    CurrentColumn,
    /// Open the context menu for a concrete column.
    Column(TableColumnIndex),
    /// Force a table-level context menu even when a column is current.
    Table,
}

impl From<TableColumnIndex> for TableContextMenuTarget {
    #[inline]
    fn from(index: TableColumnIndex) -> Self {
        Self::Column(index)
    }
}

impl From<usize> for TableContextMenuTarget {
    #[inline]
    fn from(index: usize) -> Self {
        Self::Column(TableColumnIndex::new(index))
    }
}

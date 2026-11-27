use crate::sys;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

bitflags::bitflags! {
    /// Flags for table rows
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct TableRowFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Identify header row (set default background color + width of all columns)
        const HEADERS = sys::ImGuiTableRowFlags_Headers as i32;
    }
}

#[cfg(feature = "serde")]
crate::validation::impl_serde_i32_flags!(TableRowFlags);

/// Target for table background colors.
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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

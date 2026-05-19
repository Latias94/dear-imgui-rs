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
impl Serialize for TableRowFlags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i32(self.bits())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for TableRowFlags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bits = i32::deserialize(deserializer)?;
        Ok(TableRowFlags::from_bits_truncate(bits))
    }
}

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

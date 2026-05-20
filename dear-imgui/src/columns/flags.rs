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

pub(super) fn validate_old_column_flags(caller: &str, flags: OldColumnFlags) {
    let unsupported = flags.bits() & !OldColumnFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiOldColumnFlags bits: 0x{unsupported:X}"
    );
}

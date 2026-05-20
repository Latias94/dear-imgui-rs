use crate::sys;
use bitflags::bitflags;

bitflags! {
    /// Flags for focus detection
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct FocusedFlags: i32 {
        /// Return true if window is focused
        const NONE = sys::ImGuiFocusedFlags_None as i32;
        /// IsWindowFocused() only: Return true if any children of the window is focused
        const CHILD_WINDOWS = sys::ImGuiFocusedFlags_ChildWindows as i32;
        /// IsWindowFocused() only: Test from root window (top most parent of the current hierarchy)
        const ROOT_WINDOW = sys::ImGuiFocusedFlags_RootWindow as i32;
        /// IsWindowFocused() only: Return true if any window is focused
        const ANY_WINDOW = sys::ImGuiFocusedFlags_AnyWindow as i32;
        /// IsWindowFocused() only: Do not consider popup hierarchy
        const NO_POPUP_HIERARCHY = sys::ImGuiFocusedFlags_NoPopupHierarchy as i32;
        /// IsWindowFocused() only: Consider docking hierarchy
        const DOCK_HIERARCHY = sys::ImGuiFocusedFlags_DockHierarchy as i32;
        /// IsWindowFocused() only: Shortcut for `ROOT_WINDOW | CHILD_WINDOWS`.
        const ROOT_AND_CHILD_WINDOWS = sys::ImGuiFocusedFlags_RootAndChildWindows as i32;
    }
}

impl Default for FocusedFlags {
    fn default() -> Self {
        FocusedFlags::NONE
    }
}

pub(super) fn validate_focused_flags(caller: &str, flags: FocusedFlags) {
    let unsupported = flags.bits() & !FocusedFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiFocusedFlags bits: 0x{unsupported:X}"
    );
}

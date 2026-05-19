use crate::sys;

pub(super) fn validate_popup_flags(caller: &str, flags: PopupFlags) {
    let unsupported = flags.bits() & !PopupFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiPopupFlags bits: 0x{unsupported:X}"
    );
}

pub(super) fn validate_popup_query_flags(caller: &str, flags: PopupFlags) {
    validate_popup_flags(caller, flags);
    assert!(
        !flags.contains(PopupFlags::ANY_POPUP_LEVEL) || flags.contains(PopupFlags::ANY_POPUP_ID),
        "{caller} requires ANY_POPUP_ID when using ANY_POPUP_LEVEL with a string id"
    );
}

bitflags::bitflags! {
    /// Independent flags for popup functions.
    ///
    /// Context popup mouse button selection is a single-choice setting
    /// represented by [`PopupContextMouseButton`].
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct PopupFlags: i32 {
        /// No flags
        const NONE = sys::ImGuiPopupFlags_None as i32;
        /// Do not reopen the same popup if already open.
        const NO_REOPEN = sys::ImGuiPopupFlags_NoReopen as i32;
        /// For OpenPopup*(), BeginPopupContext*(): don't open if there's already a popup at the same level of the popup stack
        const NO_OPEN_OVER_EXISTING_POPUP = sys::ImGuiPopupFlags_NoOpenOverExistingPopup as i32;
        /// For BeginPopupContext*(): don't return true when hovering items, only when hovering empty space
        const NO_OPEN_OVER_ITEMS = sys::ImGuiPopupFlags_NoOpenOverItems as i32;
        /// For IsPopupOpen(): ignore the ImGuiID parameter and test for any popup
        const ANY_POPUP_ID = sys::ImGuiPopupFlags_AnyPopupId as i32;
        /// For IsPopupOpen(): search/test at any level of the popup stack (default test in the current level)
        const ANY_POPUP_LEVEL = sys::ImGuiPopupFlags_AnyPopupLevel as i32;
        /// For IsPopupOpen(): test for any popup
        const ANY_POPUP = Self::ANY_POPUP_ID.bits() | Self::ANY_POPUP_LEVEL.bits();
    }
}

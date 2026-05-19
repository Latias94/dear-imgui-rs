use crate::sys;

fn assert_supported_bits(caller: &str, ty: &str, bits: i32, supported: i32) {
    let unsupported = bits & !supported;
    assert!(
        unsupported == 0,
        "{caller} received unsupported {ty} bits: 0x{unsupported:X}"
    );
}

pub(super) fn validate_popup_open_flags(caller: &str, flags: PopupOpenFlags) {
    assert_supported_bits(
        caller,
        "PopupOpenFlags",
        flags.bits(),
        PopupOpenFlags::all().bits(),
    );
}

pub(super) fn validate_popup_context_flags(caller: &str, flags: PopupContextFlags) {
    assert_supported_bits(
        caller,
        "PopupContextFlags",
        flags.bits(),
        PopupContextFlags::all().bits(),
    );
}

pub(super) fn validate_popup_query_flags(caller: &str, flags: PopupQueryFlags) {
    assert_supported_bits(
        caller,
        "PopupQueryFlags",
        flags.bits(),
        PopupQueryFlags::all().bits(),
    );
    assert!(
        !flags.contains(PopupQueryFlags::ANY_POPUP_LEVEL)
            || flags.contains(PopupQueryFlags::ANY_POPUP_ID),
        "{caller} requires ANY_POPUP_ID when using ANY_POPUP_LEVEL with a string id"
    );
}

macro_rules! impl_popup_flag_raw {
    ($ty:ident) => {
        impl $ty {
            #[inline]
            pub(crate) fn raw(self) -> i32 {
                self.bits()
            }
        }
    };
}

bitflags::bitflags! {
    /// Independent flags accepted by `OpenPopup*()` call sites.
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct PopupOpenFlags: i32 {
        /// No flags
        const NONE = sys::ImGuiPopupFlags_None as i32;
        /// Do not reopen the same popup if already open.
        const NO_REOPEN = sys::ImGuiPopupFlags_NoReopen as i32;
        /// Don't open if there's already a popup at the same level of the popup stack.
        const NO_OPEN_OVER_EXISTING_POPUP = sys::ImGuiPopupFlags_NoOpenOverExistingPopup as i32;
    }
}

bitflags::bitflags! {
    /// Independent flags accepted by `OpenPopupOnItemClick()` and
    /// `BeginPopupContext*()` call sites.
    ///
    /// Mouse button selection is a single-choice setting represented by
    /// [`PopupContextMouseButton`](super::PopupContextMouseButton).
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct PopupContextFlags: i32 {
        /// No flags
        const NONE = sys::ImGuiPopupFlags_None as i32;
        /// Do not reopen the same popup if already open.
        const NO_REOPEN = sys::ImGuiPopupFlags_NoReopen as i32;
        /// Don't open if there's already a popup at the same level of the popup stack.
        const NO_OPEN_OVER_EXISTING_POPUP = sys::ImGuiPopupFlags_NoOpenOverExistingPopup as i32;
        /// For window context helpers: don't return true when hovering items, only when hovering empty space.
        const NO_OPEN_OVER_ITEMS = sys::ImGuiPopupFlags_NoOpenOverItems as i32;
    }
}

bitflags::bitflags! {
    /// Independent flags accepted by `IsPopupOpen()` string-id queries.
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct PopupQueryFlags: i32 {
        /// No flags
        const NONE = sys::ImGuiPopupFlags_None as i32;
        /// Ignore the string id and test for any popup at the current popup stack level.
        const ANY_POPUP_ID = sys::ImGuiPopupFlags_AnyPopupId as i32;
        /// Search/test at any level of the popup stack.
        ///
        /// With a string-id query this must be combined with [`ANY_POPUP_ID`](Self::ANY_POPUP_ID),
        /// matching Dear ImGui's string-id assertion.
        const ANY_POPUP_LEVEL = sys::ImGuiPopupFlags_AnyPopupLevel as i32;
        /// Test for any popup at any popup stack level.
        const ANY_POPUP = Self::ANY_POPUP_ID.bits() | Self::ANY_POPUP_LEVEL.bits();
    }
}

impl_popup_flag_raw!(PopupOpenFlags);
impl_popup_flag_raw!(PopupContextFlags);
impl_popup_flag_raw!(PopupQueryFlags);

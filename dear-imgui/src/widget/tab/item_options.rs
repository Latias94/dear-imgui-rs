use crate::sys;

bitflags::bitflags! {
    /// Independent flags for tab item widgets.
    ///
    /// Leading/trailing placement is represented by [`TabItemPlacement`].
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct TabItemFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Display a dot next to the title + tab is selected when clicking the X + closure is not assumed (will wait for user to stop submitting the tab). Otherwise closure is assumed when pressing the X, so if you keep submitting the tab may reappear at end of tab bar.
        const UNSAVED_DOCUMENT = sys::ImGuiTabItemFlags_UnsavedDocument as i32;
        /// Trigger flag to programmatically make the tab selected when calling BeginTabItem()
        const SET_SELECTED = sys::ImGuiTabItemFlags_SetSelected as i32;
        /// Disable behavior of closing tabs (that are submitted with p_open != NULL) with middle mouse button. You can still repro this behavior on user's side with if (IsItemHovered() && IsMouseClicked(2)) *p_open = false.
        const NO_CLOSE_WITH_MIDDLE_MOUSE_BUTTON = sys::ImGuiTabItemFlags_NoCloseWithMiddleMouseButton as i32;
        /// Don't call PushID(tab->ID)/PopID() on BeginTabItem()/EndTabItem()
        const NO_PUSH_ID = sys::ImGuiTabItemFlags_NoPushId as i32;
        /// Disable tooltip for the given tab
        const NO_TOOLTIP = sys::ImGuiTabItemFlags_NoTooltip as i32;
        /// Disable reordering this tab or having another tab cross over this tab
        const NO_REORDER = sys::ImGuiTabItemFlags_NoReorder as i32;
    }
}

/// Single placement option for a tab item or tab item button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TabItemPlacement {
    /// Position the tab on the leading side of the tab bar.
    Leading,
    /// Position the tab on the trailing side of the tab bar.
    Trailing,
}

impl TabItemPlacement {
    #[inline]
    const fn raw(self) -> i32 {
        match self {
            Self::Leading => sys::ImGuiTabItemFlags_Leading as i32,
            Self::Trailing => sys::ImGuiTabItemFlags_Trailing as i32,
        }
    }
}

/// Complete tab item options assembled from independent flags and optional
/// single placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TabItemOptions {
    pub flags: TabItemFlags,
    pub placement: Option<TabItemPlacement>,
}

impl TabItemOptions {
    pub const fn new() -> Self {
        Self {
            flags: TabItemFlags::NONE,
            placement: None,
        }
    }

    pub fn flags(mut self, flags: TabItemFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn placement(mut self, placement: TabItemPlacement) -> Self {
        self.placement = Some(placement);
        self
    }

    pub fn bits(self) -> i32 {
        self.raw()
    }

    #[inline]
    pub(crate) fn raw(self) -> i32 {
        self.flags.bits() | self.placement.map_or(0, TabItemPlacement::raw)
    }

    #[inline]
    pub(crate) fn validate_for_tab_item(self, caller: &str) {
        validate_tab_item_options(caller, self, false);
    }

    #[inline]
    pub(crate) fn validate_for_tab_button(self, caller: &str) {
        validate_tab_item_options(caller, self, true);
    }
}

fn validate_tab_item_options(caller: &str, options: TabItemOptions, allow_button_bits: bool) {
    let unsupported_flags = options.flags.bits() & !TabItemFlags::all().bits();
    assert!(
        unsupported_flags == 0,
        "{caller} received non-independent ImGuiTabItemFlags bits: 0x{unsupported_flags:X}"
    );
    let bits = options.raw();
    let mut supported = TabItemFlags::all().bits()
        | (sys::ImGuiTabItemFlags_Leading as i32)
        | (sys::ImGuiTabItemFlags_Trailing as i32);
    if allow_button_bits {
        supported |= sys::ImGuiTabItemFlags_Button as i32;
    }
    let unsupported = bits & !supported;
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiTabItemFlags bits: 0x{unsupported:X}"
    );
    let placement_mask = (sys::ImGuiTabItemFlags_Leading | sys::ImGuiTabItemFlags_Trailing) as i32;
    assert!(
        bits & placement_mask != placement_mask,
        "{caller} cannot combine LEADING with TRAILING"
    );
    if !allow_button_bits {
        assert!(
            bits & (sys::ImGuiTabItemFlags_Button as i32) == 0,
            "{caller} cannot use BUTTON; use TabItemButton instead"
        );
    }
}

impl Default for TabItemOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl From<TabItemFlags> for TabItemOptions {
    fn from(flags: TabItemFlags) -> Self {
        Self::new().flags(flags)
    }
}

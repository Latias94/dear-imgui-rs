use crate::sys;
use crate::ui::Ui;
use std::ptr;

bitflags::bitflags! {
    /// Flags for tab bar widgets
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct TabBarFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Allow manually dragging tabs to re-order them + New tabs are appended at the end of list
        const REORDERABLE = sys::ImGuiTabBarFlags_Reorderable;
        /// Automatically select new tabs when they appear
        const AUTO_SELECT_NEW_TABS = sys::ImGuiTabBarFlags_AutoSelectNewTabs;
        /// Disable buttons to open the tab list popup
        const TAB_LIST_POPUP_BUTTON = sys::ImGuiTabBarFlags_TabListPopupButton;
        /// Disable behavior of closing tabs (that are submitted with p_open != NULL) with middle mouse button. You can still repro this behavior on user's side with if (IsItemHovered() && IsMouseClicked(2)) *p_open = false.
        const NO_CLOSE_WITH_MIDDLE_MOUSE_BUTTON = sys::ImGuiTabBarFlags_NoCloseWithMiddleMouseButton;
        /// Disable scrolling buttons (apply when fitting policy is ImGuiTabBarFlags_FittingPolicyScroll)
        const NO_TAB_LIST_SCROLLING_BUTTONS = sys::ImGuiTabBarFlags_NoTabListScrollingButtons;
        /// Disable tooltips when hovering a tab
        const NO_TOOLTIP = sys::ImGuiTabBarFlags_NoTooltip;
        /// Draw selected tab with a different color
        const DRAW_SELECTED_OVERLINE = sys::ImGuiTabBarFlags_DrawSelectedOverline;
        /// Mixed fitting policy
        const FITTING_POLICY_MIXED = sys::ImGuiTabBarFlags_FittingPolicyMixed;
        /// Shrink tabs when they don't fit
        const FITTING_POLICY_SHRINK = sys::ImGuiTabBarFlags_FittingPolicyShrink;
        /// Add scroll buttons when tabs don't fit
        const FITTING_POLICY_SCROLL = sys::ImGuiTabBarFlags_FittingPolicyScroll;
        /// Mask for fitting policy flags
        const FITTING_POLICY_MASK = sys::ImGuiTabBarFlags_FittingPolicyMask_;
        /// Default fitting policy
        const FITTING_POLICY_DEFAULT = sys::ImGuiTabBarFlags_FittingPolicyDefault_;
    }
}

bitflags::bitflags! {
    /// Flags for tab item widgets
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct TabItemFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Display a dot next to the title + tab is selected when clicking the X + closure is not assumed (will wait for user to stop submitting the tab). Otherwise closure is assumed when pressing the X, so if you keep submitting the tab may reappear at end of tab bar.
        const UNSAVED_DOCUMENT = sys::ImGuiTabItemFlags_UnsavedDocument;
        /// Trigger flag to programmatically make the tab selected when calling BeginTabItem()
        const SET_SELECTED = sys::ImGuiTabItemFlags_SetSelected;
        /// Disable behavior of closing tabs (that are submitted with p_open != NULL) with middle mouse button. You can still repro this behavior on user's side with if (IsItemHovered() && IsMouseClicked(2)) *p_open = false.
        const NO_CLOSE_WITH_MIDDLE_MOUSE_BUTTON = sys::ImGuiTabItemFlags_NoCloseWithMiddleMouseButton;
        /// Don't call PushID(tab->ID)/PopID() on BeginTabItem()/EndTabItem()
        const NO_PUSH_ID = sys::ImGuiTabItemFlags_NoPushId;
        /// Disable tooltip for the given tab
        const NO_TOOLTIP = sys::ImGuiTabItemFlags_NoTooltip;
        /// Disable reordering this tab or having another tab cross over this tab
        const NO_REORDER = sys::ImGuiTabItemFlags_NoReorder;
        /// Enforce the tab position to the left of the tab bar (after the tab list popup button)
        const LEADING = sys::ImGuiTabItemFlags_Leading;
        /// Enforce the tab position to the right of the tab bar (before the scrolling buttons)
        const TRAILING = sys::ImGuiTabItemFlags_Trailing;
    }
}

/// Builder for a tab bar
#[derive(Debug)]
#[must_use]
pub struct TabBar<T> {
    id: T,
    flags: TabBarFlags,
}

impl<T: AsRef<str>> TabBar<T> {
    /// Creates a new tab bar builder
    #[doc(alias = "BeginTabBar")]
    pub fn new(id: T) -> Self {
        Self {
            id,
            flags: TabBarFlags::NONE,
        }
    }

    /// Enable/Disable the reorderable property
    ///
    /// Disabled by default
    pub fn reorderable(mut self, value: bool) -> Self {
        if value {
            self.flags |= TabBarFlags::REORDERABLE;
        } else {
            self.flags &= !TabBarFlags::REORDERABLE;
        }
        self
    }

    /// Set the flags of the tab bar
    ///
    /// Flags are empty by default
    pub fn flags(mut self, flags: TabBarFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Begins the tab bar and returns a token if successful
    pub fn begin(self, ui: &Ui) -> Option<TabBarToken<'_>> {
        ui.tab_bar_with_flags(self.id, self.flags)
    }

    /// Creates a tab bar and runs a closure to construct the contents.
    /// Returns the result of the closure, if it is called.
    ///
    /// Note: the closure is not called if no tabbar content is visible
    pub fn build<R, F: FnOnce() -> R>(self, ui: &Ui, f: F) -> Option<R> {
        self.begin(ui).map(|_tab| f())
    }
}

/// Token representing an active tab bar
#[derive(Debug)]
#[must_use]
pub struct TabBarToken<'ui> {
    ui: &'ui Ui,
}

impl<'ui> TabBarToken<'ui> {
    /// Creates a new tab bar token
    pub(crate) fn new(ui: &'ui Ui) -> Self {
        Self { ui }
    }

    /// Ends the tab bar
    pub fn end(self) {
        // Token is consumed, destructor will be called
    }
}

impl<'ui> Drop for TabBarToken<'ui> {
    fn drop(&mut self) {
        unsafe {
            sys::ImGui_EndTabBar();
        }
    }
}

/// Builder for a tab item
#[derive(Debug)]
#[must_use]
pub struct TabItem<'a, T> {
    label: T,
    opened: Option<&'a mut bool>,
    flags: TabItemFlags,
}

impl<'a, T: AsRef<str>> TabItem<'a, T> {
    /// Creates a new tab item builder
    #[doc(alias = "BeginTabItem")]
    pub fn new(label: T) -> Self {
        Self {
            label,
            opened: None,
            flags: TabItemFlags::NONE,
        }
    }

    /// Will open or close the tab.
    ///
    /// True to display the tab. Tab item is visible by default.
    pub fn opened(mut self, opened: &'a mut bool) -> Self {
        self.opened = Some(opened);
        self
    }

    /// Set the flags of the tab item.
    ///
    /// Flags are empty by default
    pub fn flags(mut self, flags: TabItemFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Begins the tab item and returns a token if successful
    pub fn begin(self, ui: &Ui) -> Option<TabItemToken<'_>> {
        ui.tab_item_with_flags(self.label, self.opened, self.flags)
    }

    /// Creates a tab item and runs a closure to construct the contents.
    /// Returns the result of the closure, if it is called.
    ///
    /// Note: the closure is not called if the tab item is not selected
    pub fn build<R, F: FnOnce() -> R>(self, ui: &Ui, f: F) -> Option<R> {
        self.begin(ui).map(|_tab| f())
    }
}

/// Token representing an active tab item
#[derive(Debug)]
#[must_use]
pub struct TabItemToken<'ui> {
    ui: &'ui Ui,
}

impl<'ui> TabItemToken<'ui> {
    /// Creates a new tab item token
    pub(crate) fn new(ui: &'ui Ui) -> Self {
        Self { ui }
    }

    /// Ends the tab item
    pub fn end(self) {
        // Token is consumed, destructor will be called
    }
}

impl<'ui> Drop for TabItemToken<'ui> {
    fn drop(&mut self) {
        unsafe {
            sys::ImGui_EndTabItem();
        }
    }
}

/// # Tab Widgets
impl Ui {
    /// Creates a tab bar and returns a tab bar token, allowing you to append
    /// Tab items afterwards. This passes no flags. To pass flags explicitly,
    /// use [tab_bar_with_flags](Self::tab_bar_with_flags).
    #[doc(alias = "BeginTabBar")]
    pub fn tab_bar(&self, id: impl AsRef<str>) -> Option<TabBarToken<'_>> {
        self.tab_bar_with_flags(id, TabBarFlags::NONE)
    }

    /// Creates a tab bar and returns a tab bar token, allowing you to append
    /// Tab items afterwards.
    #[doc(alias = "BeginTabBar")]
    pub fn tab_bar_with_flags(
        &self,
        id: impl AsRef<str>,
        flags: TabBarFlags,
    ) -> Option<TabBarToken<'_>> {
        let id_ptr = self.scratch_txt(id);
        let should_render = unsafe { sys::ImGui_BeginTabBar(id_ptr, flags.bits()) };

        if should_render {
            Some(TabBarToken::new(self))
        } else {
            None
        }
    }

    /// Creates a new tab item and returns a token if its contents are visible.
    ///
    /// By default, this doesn't pass an opened bool nor any flags. See [tab_item_with_opened]
    /// and [tab_item_with_flags] for more.
    ///
    /// [tab_item_with_opened]: Self::tab_item_with_opened
    /// [tab_item_with_flags]: Self::tab_item_with_flags
    #[doc(alias = "BeginTabItem")]
    pub fn tab_item(&self, label: impl AsRef<str>) -> Option<TabItemToken<'_>> {
        self.tab_item_with_flags(label, None, TabItemFlags::NONE)
    }

    /// Creates a new tab item and returns a token if its contents are visible.
    ///
    /// By default, this doesn't pass any flags. See [tab_item_with_flags] for more.
    #[doc(alias = "BeginTabItem")]
    pub fn tab_item_with_opened(
        &self,
        label: impl AsRef<str>,
        opened: &mut bool,
    ) -> Option<TabItemToken<'_>> {
        self.tab_item_with_flags(label, Some(opened), TabItemFlags::NONE)
    }

    /// Creates a new tab item and returns a token if its contents are visible.
    #[doc(alias = "BeginTabItem")]
    pub fn tab_item_with_flags(
        &self,
        label: impl AsRef<str>,
        opened: Option<&mut bool>,
        flags: TabItemFlags,
    ) -> Option<TabItemToken<'_>> {
        let label_ptr = self.scratch_txt(label);
        let opened_ptr = opened.map(|x| x as *mut bool).unwrap_or(ptr::null_mut());

        let should_render = unsafe { sys::ImGui_BeginTabItem(label_ptr, opened_ptr, flags.bits()) };

        if should_render {
            Some(TabItemToken::new(self))
        } else {
            None
        }
    }
}

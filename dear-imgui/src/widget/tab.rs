//! Tabs
//!
//! Tab bars and tab items for organizing content. Builders manage begin/end
//! lifetimes to help keep API usage balanced.
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]
use crate::sys;
use crate::ui::Ui;
use std::ptr;

bitflags::bitflags! {
    /// Independent flags for tab bar widgets.
    ///
    /// The fitting policy is a single-choice setting represented by
    /// [`TabBarFittingPolicy`].
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct TabBarFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Allow manually dragging tabs to re-order them + New tabs are appended at the end of list
        const REORDERABLE = sys::ImGuiTabBarFlags_Reorderable as i32;
        /// Automatically select new tabs when they appear
        const AUTO_SELECT_NEW_TABS = sys::ImGuiTabBarFlags_AutoSelectNewTabs as i32;
        /// Disable buttons to open the tab list popup
        const TAB_LIST_POPUP_BUTTON = sys::ImGuiTabBarFlags_TabListPopupButton as i32;
        /// Disable behavior of closing tabs (that are submitted with p_open != NULL) with middle mouse button. You can still repro this behavior on user's side with if (IsItemHovered() && IsMouseClicked(2)) *p_open = false.
        const NO_CLOSE_WITH_MIDDLE_MOUSE_BUTTON = sys::ImGuiTabBarFlags_NoCloseWithMiddleMouseButton as i32;
        /// Disable scrolling buttons (apply when fitting policy is ImGuiTabBarFlags_FittingPolicyScroll)
        const NO_TAB_LIST_SCROLLING_BUTTONS = sys::ImGuiTabBarFlags_NoTabListScrollingButtons as i32;
        /// Disable tooltips when hovering a tab
        const NO_TOOLTIP = sys::ImGuiTabBarFlags_NoTooltip as i32;
        /// Draw selected tab with a different color
        const DRAW_SELECTED_OVERLINE = sys::ImGuiTabBarFlags_DrawSelectedOverline as i32;
    }
}

/// Single fitting policy for a tab bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TabBarFittingPolicy {
    /// Use Dear ImGui's mixed default policy.
    Mixed,
    /// Shrink tabs when they do not fit.
    Shrink,
    /// Add scrolling when tabs do not fit.
    Scroll,
}

impl TabBarFittingPolicy {
    #[inline]
    const fn raw(self) -> i32 {
        match self {
            Self::Mixed => sys::ImGuiTabBarFlags_FittingPolicyMixed as i32,
            Self::Shrink => sys::ImGuiTabBarFlags_FittingPolicyShrink as i32,
            Self::Scroll => sys::ImGuiTabBarFlags_FittingPolicyScroll as i32,
        }
    }
}

/// Complete tab bar options assembled from independent flags and optional
/// single fitting policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TabBarOptions {
    pub flags: TabBarFlags,
    pub fitting_policy: Option<TabBarFittingPolicy>,
}

impl TabBarOptions {
    pub const fn new() -> Self {
        Self {
            flags: TabBarFlags::NONE,
            fitting_policy: None,
        }
    }

    pub fn flags(mut self, flags: TabBarFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn fitting_policy(mut self, policy: TabBarFittingPolicy) -> Self {
        self.fitting_policy = Some(policy);
        self
    }

    pub fn bits(self) -> i32 {
        self.raw()
    }

    #[inline]
    pub(crate) fn raw(self) -> i32 {
        self.flags.bits() | self.fitting_policy.map_or(0, TabBarFittingPolicy::raw)
    }
}

impl Default for TabBarOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl From<TabBarFlags> for TabBarOptions {
    fn from(flags: TabBarFlags) -> Self {
        Self::new().flags(flags)
    }
}

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

/// Builder for a tab bar
#[derive(Debug)]
#[must_use]
pub struct TabBar<T> {
    id: T,
    options: TabBarOptions,
}

impl<T: AsRef<str>> TabBar<T> {
    /// Creates a new tab bar builder
    #[doc(alias = "BeginTabBar")]
    pub fn new(id: T) -> Self {
        Self {
            id,
            options: TabBarOptions::new(),
        }
    }

    /// Enable/Disable the reorderable property
    ///
    /// Disabled by default
    pub fn reorderable(mut self, value: bool) -> Self {
        if value {
            self.options.flags |= TabBarFlags::REORDERABLE;
        } else {
            self.options.flags &= !TabBarFlags::REORDERABLE;
        }
        self
    }

    /// Set the flags of the tab bar
    ///
    /// Flags are empty by default
    pub fn flags(mut self, flags: impl Into<TabBarOptions>) -> Self {
        self.options = flags.into();
        self
    }

    /// Set the tab fitting policy.
    pub fn fitting_policy(mut self, policy: TabBarFittingPolicy) -> Self {
        self.options.fitting_policy = Some(policy);
        self
    }

    /// Begins the tab bar and returns a token if successful
    pub fn begin(self, ui: &Ui) -> Option<TabBarToken<'_>> {
        ui.tab_bar_with_flags(self.id, self.options)
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
    _ui: &'ui Ui,
}

impl<'ui> TabBarToken<'ui> {
    /// Creates a new tab bar token
    pub(crate) fn new(ui: &'ui Ui) -> Self {
        Self { _ui: ui }
    }

    /// Ends the tab bar
    pub fn end(self) {
        // Token is consumed, destructor will be called
    }
}

impl<'ui> Drop for TabBarToken<'ui> {
    fn drop(&mut self) {
        unsafe {
            sys::igEndTabBar();
        }
    }
}

/// Builder for a tab item
#[derive(Debug)]
#[must_use]
pub struct TabItem<'a, T> {
    label: T,
    opened: Option<&'a mut bool>,
    options: TabItemOptions,
}

impl<'a, T: AsRef<str>> TabItem<'a, T> {
    /// Creates a new tab item builder
    #[doc(alias = "BeginTabItem")]
    pub fn new(label: T) -> Self {
        Self {
            label,
            opened: None,
            options: TabItemOptions::new(),
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
    pub fn flags(mut self, flags: impl Into<TabItemOptions>) -> Self {
        self.options = flags.into();
        self
    }

    /// Set the tab placement.
    pub fn placement(mut self, placement: TabItemPlacement) -> Self {
        self.options.placement = Some(placement);
        self
    }

    /// Begins the tab item and returns a token if successful
    pub fn begin(self, ui: &Ui) -> Option<TabItemToken<'_>> {
        ui.tab_item_with_flags(self.label, self.opened, self.options)
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
    _ui: &'ui Ui,
}

impl<'ui> TabItemToken<'ui> {
    /// Creates a new tab item token
    pub(crate) fn new(ui: &'ui Ui) -> Self {
        Self { _ui: ui }
    }

    /// Ends the tab item
    pub fn end(self) {
        // Token is consumed, destructor will be called
    }
}

impl<'ui> Drop for TabItemToken<'ui> {
    fn drop(&mut self) {
        unsafe {
            sys::igEndTabItem();
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
        flags: impl Into<TabBarOptions>,
    ) -> Option<TabBarToken<'_>> {
        let options = flags.into();
        let id_ptr = self.scratch_txt(id);
        let should_render = unsafe { sys::igBeginTabBar(id_ptr, options.raw()) };

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
        self.tab_item_with_flags(label, None, TabItemOptions::new())
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
        self.tab_item_with_flags(label, Some(opened), TabItemOptions::new())
    }

    /// Creates a new tab item and returns a token if its contents are visible.
    #[doc(alias = "BeginTabItem")]
    pub fn tab_item_with_flags(
        &self,
        label: impl AsRef<str>,
        opened: Option<&mut bool>,
        flags: impl Into<TabItemOptions>,
    ) -> Option<TabItemToken<'_>> {
        let options = flags.into();
        let label_ptr = self.scratch_txt(label);
        let opened_ptr = opened.map(|x| x as *mut bool).unwrap_or(ptr::null_mut());

        let should_render = unsafe { sys::igBeginTabItem(label_ptr, opened_ptr, options.raw()) };

        if should_render {
            Some(TabItemToken::new(self))
        } else {
            None
        }
    }

    /// Creates a button on the current tab bar (e.g. to append a `+` new-tab button).
    #[doc(alias = "TabItemButton")]
    pub fn tab_item_button(&self, label: impl AsRef<str>) -> bool {
        self.tab_item_button_with_flags(label, TabItemOptions::new())
    }

    /// Creates a button on the current tab bar with explicit flags.
    #[doc(alias = "TabItemButton")]
    pub fn tab_item_button_with_flags(
        &self,
        label: impl AsRef<str>,
        flags: impl Into<TabItemOptions>,
    ) -> bool {
        unsafe { sys::igTabItemButton(self.scratch_txt(label), flags.into().raw()) }
    }

    /// Notifies Dear ImGui that a tab (or docked window) has been closed.
    #[doc(alias = "SetTabItemClosed")]
    pub fn set_tab_item_closed(&self, tab_or_docked_window_label: impl AsRef<str>) {
        unsafe { sys::igSetTabItemClosed(self.scratch_txt(tab_or_docked_window_label)) }
    }
}

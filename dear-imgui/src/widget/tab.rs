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

use crate::scope::{NativeScopePop, NativeScopeToken};
use crate::{Ui, sys};
use std::ptr;

/// # Tab Widgets
impl Ui {
    /// Configure a tab bar and render its contents with [`TabBar::build`].
    pub fn tab_bar_config<T: AsRef<str>>(&self, id: T) -> TabBar<'_, T> {
        TabBar::new(self, id)
    }

    /// Configure a tab item and render its contents with [`TabItem::build`].
    pub fn tab_item_config<T: AsRef<str>>(&self, label: T) -> TabItem<'_, T> {
        TabItem::new(self, label)
    }

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
        options.validate("Ui::tab_bar_with_flags()");
        let should_render = self.with_scratch_txt(id, |id_ptr| unsafe {
            sys::igBeginTabBar(id_ptr, options.raw())
        });

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
    /// By default, this doesn't pass any flags. See [`Self::tab_item_with_flags`] for more.
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
        options.validate_for_tab_item("Ui::tab_item_with_flags()");
        let opened_ptr = opened.map(|x| x as *mut bool).unwrap_or(ptr::null_mut());

        let should_render = self.with_scratch_txt(label, |label_ptr| unsafe {
            sys::igBeginTabItem(label_ptr, opened_ptr, options.raw())
        });

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
        let options = flags.into();
        options.validate_for_tab_button("Ui::tab_item_button_with_flags()");
        self.with_scratch_txt(label, |label_ptr| unsafe {
            sys::igTabItemButton(label_ptr, options.raw())
        })
    }

    /// Notifies Dear ImGui that a tab (or docked window) has been closed.
    #[doc(alias = "SetTabItemClosed")]
    pub fn set_tab_item_closed(&self, tab_or_docked_window_label: impl AsRef<str>) {
        self.with_scratch_txt(tab_or_docked_window_label, |label_ptr| unsafe {
            sys::igSetTabItemClosed(label_ptr)
        });
    }
}

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
            Self::Mixed => sys::ImGuiTabBarFlags_FittingPolicyMixed,
            Self::Shrink => sys::ImGuiTabBarFlags_FittingPolicyShrink,
            Self::Scroll => sys::ImGuiTabBarFlags_FittingPolicyScroll,
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

    #[inline]
    pub(crate) fn validate(self, caller: &str) {
        let unsupported_flags = self.flags.bits() & !TabBarFlags::all().bits();
        assert!(
            unsupported_flags == 0,
            "{caller} received non-independent ImGuiTabBarFlags bits: 0x{unsupported_flags:X}"
        );
        let bits = self.raw();
        let fitting_policy_mask = sys::ImGuiTabBarFlags_FittingPolicyMask_;
        let supported = TabBarFlags::all().bits() | fitting_policy_mask;
        let unsupported = bits & !supported;
        assert!(
            unsupported == 0,
            "{caller} received unsupported ImGuiTabBarFlags bits: 0x{unsupported:X}"
        );
        assert!(
            (bits & fitting_policy_mask).count_ones() <= 1,
            "{caller} accepts at most one tab-bar fitting policy"
        );
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

/// Builder for a tab bar
#[derive(Debug)]
#[must_use]
pub struct TabBar<'ui, T> {
    ui: &'ui Ui,
    id: T,
    options: TabBarOptions,
}

impl<'ui, T: AsRef<str>> TabBar<'ui, T> {
    /// Creates a new tab bar builder
    #[doc(alias = "BeginTabBar")]
    pub fn new(ui: &'ui Ui, id: T) -> Self {
        Self {
            ui,
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

    /// Creates a tab bar and runs a closure to construct the contents.
    /// Returns the result of the closure, if it is called.
    ///
    /// Note: the closure is not called if no tabbar content is visible
    pub fn build<R>(self, f: impl FnOnce() -> R) -> Option<R> {
        self.ui.with_bound_context(|| {
            let _token = self.ui.tab_bar_with_flags(self.id, self.options)?;
            Some(f())
        })
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
        /// Select the tab when closing and wait for the application to stop submitting it.
        const NO_ASSUMED_CLOSURE = sys::ImGuiTabItemFlags_NoAssumedClosure;
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
            Self::Leading => sys::ImGuiTabItemFlags_Leading,
            Self::Trailing => sys::ImGuiTabItemFlags_Trailing,
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
        | sys::ImGuiTabItemFlags_Leading
        | sys::ImGuiTabItemFlags_Trailing;
    if allow_button_bits {
        supported |= sys::ImGuiTabItemFlags_Button;
    }
    let unsupported = bits & !supported;
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiTabItemFlags bits: 0x{unsupported:X}"
    );
    let placement_mask = sys::ImGuiTabItemFlags_Leading | sys::ImGuiTabItemFlags_Trailing;
    assert!(
        bits & placement_mask != placement_mask,
        "{caller} cannot combine LEADING with TRAILING"
    );
    if !allow_button_bits {
        assert!(
            bits & sys::ImGuiTabItemFlags_Button == 0,
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

/// Builder for a tab item
#[derive(Debug)]
#[must_use]
pub struct TabItem<'ui, T> {
    ui: &'ui Ui,
    label: T,
    opened: Option<&'ui mut bool>,
    options: TabItemOptions,
}

impl<'ui, T: AsRef<str>> TabItem<'ui, T> {
    /// Creates a new tab item builder
    #[doc(alias = "BeginTabItem")]
    pub fn new(ui: &'ui Ui, label: T) -> Self {
        Self {
            ui,
            label,
            opened: None,
            options: TabItemOptions::new(),
        }
    }

    /// Will open or close the tab.
    ///
    /// True to display the tab. Tab item is visible by default.
    pub fn opened(mut self, opened: &'ui mut bool) -> Self {
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

    /// Creates a tab item and runs a closure to construct the contents.
    /// Returns the result of the closure, if it is called.
    ///
    /// Note: the closure is not called if the tab item is not selected
    pub fn build<R>(self, f: impl FnOnce() -> R) -> Option<R> {
        self.ui.with_bound_context(|| {
            let _token = self
                .ui
                .tab_item_with_flags(self.label, self.opened, self.options)?;
            Some(f())
        })
    }
}

/// Token representing an active tab bar.
///
/// Tab-bar and tab-item tokens from the same window must finish in reverse creation order and in
/// their originating window `Begin` scope. Prefer [`crate::TabBar::build`] for ordinary use.
#[derive(Debug)]
#[must_use]
#[doc(alias = "EndTabBar")]
pub struct TabBarToken<'ui> {
    scope: NativeScopeToken<'ui>,
}

impl<'ui> TabBarToken<'ui> {
    /// Creates a new tab bar token
    pub(crate) fn new(ui: &'ui Ui) -> Self {
        Self {
            scope: ui.begin_native_scope(NativeScopePop::EndTabBar, "TabBarToken"),
        }
    }

    /// Ends the tab bar
    ///
    /// # Panics
    ///
    /// Panics before FFI if a tab item or later tab-bar token is active, or if this token is
    /// outside its originating window `Begin` scope.
    pub fn end(self) {
        // Token is consumed, destructor will be called
    }
}

impl<'ui> Drop for TabBarToken<'ui> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}

/// Token representing an active tab item.
///
/// Tab-bar and tab-item tokens from the same window must finish in reverse creation order and in
/// their originating window `Begin` scope. Prefer [`crate::TabItem::build`] for ordinary use.
#[derive(Debug)]
#[must_use]
#[doc(alias = "EndTabItem")]
pub struct TabItemToken<'ui> {
    scope: NativeScopeToken<'ui>,
}

impl<'ui> TabItemToken<'ui> {
    /// Creates a new tab item token
    pub(crate) fn new(ui: &'ui Ui) -> Self {
        Self {
            scope: ui.begin_native_scope(NativeScopePop::EndTabItem, "TabItemToken"),
        }
    }

    /// Ends the tab item
    ///
    /// # Panics
    ///
    /// Panics before FFI if a later tab token is active or this token is outside its originating
    /// window `Begin` scope.
    pub fn end(self) {
        // Token is consumed, destructor will be called
    }
}

impl<'ui> Drop for TabItemToken<'ui> {
    fn drop(&mut self) {
        self.scope.finish();
    }
}

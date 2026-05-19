use crate::sys;

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

    #[inline]
    pub(crate) fn validate(self, caller: &str) {
        let unsupported_flags = self.flags.bits() & !TabBarFlags::all().bits();
        assert!(
            unsupported_flags == 0,
            "{caller} received non-independent ImGuiTabBarFlags bits: 0x{unsupported_flags:X}"
        );
        let bits = self.raw();
        let fitting_policy_mask = sys::ImGuiTabBarFlags_FittingPolicyMask_ as i32;
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

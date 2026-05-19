use crate::sys;

bitflags::bitflags! {
    /// Independent flags controlling multi-selection behavior.
    ///
    /// The click-selection policy, box-select mode, and scope are represented by
    /// [`MultiSelectClickPolicy`], [`MultiSelectBoxSelect`], and [`MultiSelectScopeKind`].
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct MultiSelectFlags: i32 {
        /// No flags.
        const NONE = sys::ImGuiMultiSelectFlags_None as i32;
        /// Single-selection scope. Ctrl/Shift range selection is disabled.
        const SINGLE_SELECT = sys::ImGuiMultiSelectFlags_SingleSelect as i32;
        /// Disable `Ctrl+A` "select all" shortcut.
        const NO_SELECT_ALL = sys::ImGuiMultiSelectFlags_NoSelectAll as i32;
        /// Disable range selection (Shift+click / Shift+arrow).
        const NO_RANGE_SELECT = sys::ImGuiMultiSelectFlags_NoRangeSelect as i32;
        /// Disable automatic selection of newly focused items.
        const NO_AUTO_SELECT = sys::ImGuiMultiSelectFlags_NoAutoSelect as i32;
        /// Disable automatic clearing of selection when focus moves within the scope.
        const NO_AUTO_CLEAR = sys::ImGuiMultiSelectFlags_NoAutoClear as i32;
        /// Disable automatic clearing when reselecting the same range.
        const NO_AUTO_CLEAR_ON_RESELECT =
            sys::ImGuiMultiSelectFlags_NoAutoClearOnReselect as i32;
        /// Disable drag-scrolling when box-selecting near edges of the scope.
        const BOX_SELECT_NO_SCROLL = sys::ImGuiMultiSelectFlags_BoxSelectNoScroll as i32;
        /// Clear selection when pressing Escape while the scope is focused.
        const CLEAR_ON_ESCAPE = sys::ImGuiMultiSelectFlags_ClearOnEscape as i32;
        /// Clear selection when clicking on empty space (void) inside the scope.
        const CLEAR_ON_CLICK_VOID = sys::ImGuiMultiSelectFlags_ClearOnClickVoid as i32;
        /// Disable default right-click behavior that selects item before opening a context menu.
        const NO_SELECT_ON_RIGHT_CLICK =
            sys::ImGuiMultiSelectFlags_NoSelectOnRightClick as i32;
    }
}

/// Box-selection geometry for multi-select scopes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MultiSelectBoxSelect {
    /// Same x-position/full-row items.
    OneDimensional,
    /// Arbitrary item layout, at a higher clipping cost.
    TwoDimensional,
}

impl MultiSelectBoxSelect {
    #[inline]
    const fn raw(self) -> i32 {
        match self {
            Self::OneDimensional => sys::ImGuiMultiSelectFlags_BoxSelect1d as i32,
            Self::TwoDimensional => sys::ImGuiMultiSelectFlags_BoxSelect2d as i32,
        }
    }
}

/// Click-selection policy for multi-select scopes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MultiSelectClickPolicy {
    /// Apply selection on mouse down for unselected items and on mouse up for
    /// selected items.
    Auto,
    /// Apply selection on mouse down for any clicked item.
    ClickAlways,
    /// Apply selection on mouse release for unselected items.
    ClickRelease,
}

impl MultiSelectClickPolicy {
    #[inline]
    const fn raw(self) -> i32 {
        match self {
            Self::Auto => sys::ImGuiMultiSelectFlags_SelectOnAuto as i32,
            Self::ClickAlways => sys::ImGuiMultiSelectFlags_SelectOnClickAlways as i32,
            Self::ClickRelease => sys::ImGuiMultiSelectFlags_SelectOnClickRelease as i32,
        }
    }
}

/// Scope for box-select and clear-on-empty-click behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MultiSelectScopeKind {
    /// Scope is the whole window.
    Window,
    /// Scope is the whole window and enables Dear ImGui's temporary X-axis navigation wrap helper.
    WindowWithNavWrapX,
    /// Scope is the rectangle between `BeginMultiSelect()` and `EndMultiSelect()`.
    Rect,
}

impl MultiSelectScopeKind {
    #[inline]
    const fn raw(self) -> i32 {
        match self {
            Self::Window => sys::ImGuiMultiSelectFlags_ScopeWindow as i32,
            Self::WindowWithNavWrapX => {
                (sys::ImGuiMultiSelectFlags_ScopeWindow | sys::ImGuiMultiSelectFlags_NavWrapX)
                    as i32
            }
            Self::Rect => sys::ImGuiMultiSelectFlags_ScopeRect as i32,
        }
    }
}

/// Complete multi-select options assembled from independent flags and an
/// optional single-choice policies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MultiSelectOptions {
    pub flags: MultiSelectFlags,
    pub click_policy: Option<MultiSelectClickPolicy>,
    pub box_select: Option<MultiSelectBoxSelect>,
    pub scope: Option<MultiSelectScopeKind>,
}

impl MultiSelectOptions {
    pub const fn new() -> Self {
        Self {
            flags: MultiSelectFlags::NONE,
            click_policy: None,
            box_select: None,
            scope: None,
        }
    }

    pub fn flags(mut self, flags: MultiSelectFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn click_policy(mut self, policy: MultiSelectClickPolicy) -> Self {
        self.click_policy = Some(policy);
        self
    }

    pub fn box_select(mut self, mode: MultiSelectBoxSelect) -> Self {
        self.box_select = Some(mode);
        self
    }

    pub fn scope(mut self, scope: MultiSelectScopeKind) -> Self {
        self.scope = Some(scope);
        self
    }

    pub fn bits(self) -> i32 {
        self.raw()
    }

    #[inline]
    pub(crate) fn raw(self) -> i32 {
        self.flags.bits()
            | self.click_policy.map_or(0, MultiSelectClickPolicy::raw)
            | self.box_select.map_or(0, MultiSelectBoxSelect::raw)
            | self.scope.map_or(0, MultiSelectScopeKind::raw)
    }
}

impl Default for MultiSelectOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl From<MultiSelectFlags> for MultiSelectOptions {
    fn from(flags: MultiSelectFlags) -> Self {
        Self::new().flags(flags)
    }
}

use crate::sys;
use bitflags::bitflags;

bitflags! {
    /// Independent input flags accepted by `Shortcut()`.
    ///
    /// The route policy is a single-choice setting represented by
    /// [`ShortcutRoute`].
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ShortcutFlags: i32 {
        const NONE = sys::ImGuiInputFlags_None as i32;
        const REPEAT = sys::ImGuiInputFlags_Repeat as i32;
        const ROUTE_FROM_ROOT_WINDOW = sys::ImGuiInputFlags_RouteFromRootWindow as i32;
    }
}

impl Default for ShortcutFlags {
    fn default() -> Self {
        ShortcutFlags::NONE
    }
}

bitflags! {
    /// Options accepted only by the global shortcut route.
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ShortcutGlobalRouteFlags: i32 {
        const NONE = sys::ImGuiInputFlags_None as i32;
        const OVER_FOCUSED = sys::ImGuiInputFlags_RouteOverFocused as i32;
        const OVER_ACTIVE = sys::ImGuiInputFlags_RouteOverActive as i32;
        const UNLESS_BG_FOCUSED = sys::ImGuiInputFlags_RouteUnlessBgFocused as i32;
    }
}

/// Single route policy for `Shortcut()` and `SetNextItemShortcut()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShortcutRoute {
    /// Route to the active item only.
    Active,
    /// Route to windows in the focus stack. This is Dear ImGui's `Shortcut()`
    /// default when no explicit route is provided.
    Focused,
    /// Focused route with higher priority than the active item.
    FocusedOverActive,
    /// Global route with optional global-only priority modifiers.
    Global(ShortcutGlobalRouteFlags),
    /// Poll keys directly without route registration.
    Always,
}

impl ShortcutRoute {
    #[inline]
    const fn raw(self) -> sys::ImGuiInputFlags {
        match self {
            Self::Active => sys::ImGuiInputFlags_RouteActive as sys::ImGuiInputFlags,
            Self::Focused => sys::ImGuiInputFlags_RouteFocused as sys::ImGuiInputFlags,
            Self::FocusedOverActive => {
                sys::ImGuiInputFlags_RouteFocused as sys::ImGuiInputFlags
                    | sys::ImGuiInputFlags_RouteOverActive as sys::ImGuiInputFlags
            }
            Self::Global(flags) => {
                sys::ImGuiInputFlags_RouteGlobal as sys::ImGuiInputFlags | flags.bits()
            }
            Self::Always => sys::ImGuiInputFlags_RouteAlways as sys::ImGuiInputFlags,
        }
    }

    /// Returns the underlying Dear ImGui bits for this route policy.
    pub const fn bits(self) -> i32 {
        self.raw()
    }
}

/// Complete shortcut options assembled from independent flags and an optional
/// single route policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShortcutOptions {
    pub flags: ShortcutFlags,
    pub route: Option<ShortcutRoute>,
}

impl ShortcutOptions {
    pub const fn new() -> Self {
        Self {
            flags: ShortcutFlags::NONE,
            route: None,
        }
    }

    pub fn flags(mut self, flags: ShortcutFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn route(mut self, route: ShortcutRoute) -> Self {
        self.route = Some(route);
        self
    }

    /// Returns the underlying Dear ImGui bits for these options.
    pub fn bits(self) -> i32 {
        self.raw()
    }

    #[inline]
    pub(crate) fn raw(self) -> sys::ImGuiInputFlags {
        self.flags.bits() | self.route.map_or(0, ShortcutRoute::raw)
    }
}

impl Default for ShortcutOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl From<ShortcutFlags> for ShortcutOptions {
    fn from(flags: ShortcutFlags) -> Self {
        Self::new().flags(flags)
    }
}

impl From<ShortcutRoute> for ShortcutOptions {
    fn from(route: ShortcutRoute) -> Self {
        Self::new().route(route)
    }
}

/// Backwards-compatible name for shortcut options.
pub type InputFlags = ShortcutOptions;

bitflags! {
    /// Flags specific to `SetNextItemShortcut()`.
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct NextItemShortcutFlags: i32 {
        const NONE = sys::ImGuiInputFlags_None as i32;
        const TOOLTIP = sys::ImGuiInputFlags_Tooltip as i32;
    }
}

/// Complete options accepted by `SetNextItemShortcut()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NextItemShortcutOptions {
    pub shortcut: ShortcutOptions,
    pub flags: NextItemShortcutFlags,
}

impl NextItemShortcutOptions {
    pub const fn new() -> Self {
        Self {
            shortcut: ShortcutOptions::new(),
            flags: NextItemShortcutFlags::NONE,
        }
    }

    pub fn shortcut(mut self, options: impl Into<ShortcutOptions>) -> Self {
        self.shortcut = options.into();
        self
    }

    pub fn flags(mut self, flags: ShortcutFlags) -> Self {
        self.shortcut.flags = flags;
        self
    }

    pub fn route(mut self, route: ShortcutRoute) -> Self {
        self.shortcut.route = Some(route);
        self
    }

    pub fn next_item_flags(mut self, flags: NextItemShortcutFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn tooltip(mut self, value: bool) -> Self {
        self.flags.set(NextItemShortcutFlags::TOOLTIP, value);
        self
    }

    /// Returns the underlying Dear ImGui bits for these options.
    pub fn bits(self) -> i32 {
        self.raw()
    }

    #[inline]
    pub(crate) fn raw(self) -> sys::ImGuiInputFlags {
        self.shortcut.raw() | self.flags.bits()
    }
}

impl Default for NextItemShortcutOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl From<ShortcutOptions> for NextItemShortcutOptions {
    fn from(shortcut: ShortcutOptions) -> Self {
        Self::new().shortcut(shortcut)
    }
}

impl From<ShortcutFlags> for NextItemShortcutOptions {
    fn from(flags: ShortcutFlags) -> Self {
        Self::new().flags(flags)
    }
}

impl From<ShortcutRoute> for NextItemShortcutOptions {
    fn from(route: ShortcutRoute) -> Self {
        Self::new().route(route)
    }
}

impl From<NextItemShortcutFlags> for NextItemShortcutOptions {
    fn from(flags: NextItemShortcutFlags) -> Self {
        Self::new().next_item_flags(flags)
    }
}

bitflags! {
    /// Input flags accepted by `SetItemKeyOwner()`.
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ItemKeyOwnerFlags: i32 {
        const NONE = sys::ImGuiInputFlags_None as i32;

        const LOCK_THIS_FRAME = sys::ImGuiInputFlags_LockThisFrame as i32;
        const LOCK_UNTIL_RELEASE = sys::ImGuiInputFlags_LockUntilRelease as i32;

        const COND_HOVERED = sys::ImGuiInputFlags_CondHovered as i32;
        const COND_ACTIVE = sys::ImGuiInputFlags_CondActive as i32;
    }
}

impl Default for ItemKeyOwnerFlags {
    fn default() -> Self {
        ItemKeyOwnerFlags::NONE
    }
}

impl ItemKeyOwnerFlags {
    #[inline]
    pub(crate) fn raw(self) -> sys::ImGuiInputFlags {
        self.bits() as sys::ImGuiInputFlags
    }
}

impl Default for NextItemShortcutFlags {
    fn default() -> Self {
        NextItemShortcutFlags::NONE
    }
}

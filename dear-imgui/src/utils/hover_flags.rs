use crate::sys;
use bitflags::bitflags;

bitflags! {
    /// Flags accepted by `Ui::is_window_hovered_with_flags()`.
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct WindowHoveredFlags: i32 {
        /// Return true if directly over the item/window, not obstructed by another window, not obstructed by an active popup or modal blocking inputs under them.
        const NONE = sys::ImGuiHoveredFlags_None as i32;
        /// IsWindowHovered() only: Return true if any children of the window is hovered
        const CHILD_WINDOWS = sys::ImGuiHoveredFlags_ChildWindows as i32;
        /// IsWindowHovered() only: Test from root window (top most parent of the current hierarchy)
        const ROOT_WINDOW = sys::ImGuiHoveredFlags_RootWindow as i32;
        /// IsWindowHovered() only: Return true if any window is hovered
        const ANY_WINDOW = sys::ImGuiHoveredFlags_AnyWindow as i32;
        /// IsWindowHovered() only: Do not consider popup hierarchy (do not treat popup emitter as parent of popup) (when used with _ChildWindows or _RootWindow)
        const NO_POPUP_HIERARCHY = sys::ImGuiHoveredFlags_NoPopupHierarchy as i32;
        /// IsWindowHovered() only: Consider docking hierarchy (treat dockspace host as parent of docked window) (when used with _ChildWindows or _RootWindow)
        const DOCK_HIERARCHY = sys::ImGuiHoveredFlags_DockHierarchy as i32;
        /// Return true even if a popup window is normally blocking access to this item/window
        const ALLOW_WHEN_BLOCKED_BY_POPUP = sys::ImGuiHoveredFlags_AllowWhenBlockedByPopup as i32;
        /// Return true even if an active item is blocking access to this item/window. Useful for Drag and Drop patterns.
        const ALLOW_WHEN_BLOCKED_BY_ACTIVE_ITEM = sys::ImGuiHoveredFlags_AllowWhenBlockedByActiveItem as i32;
        /// IsWindowHovered() only: Shortcut for `ROOT_WINDOW | CHILD_WINDOWS`.
        const ROOT_AND_CHILD_WINDOWS = sys::ImGuiHoveredFlags_RootAndChildWindows as i32;
        /// Shortcut for standard flags when using IsWindowHovered() + tooltip-style hover behavior.
        const FOR_TOOLTIP = sys::ImGuiHoveredFlags_ForTooltip as i32;
        /// Require mouse to be stationary for style.HoverStationaryDelay (~0.15 sec) at least one time.
        const STATIONARY = sys::ImGuiHoveredFlags_Stationary as i32;
    }
}

impl Default for WindowHoveredFlags {
    fn default() -> Self {
        WindowHoveredFlags::NONE
    }
}

bitflags! {
    /// Flags accepted by `Ui::is_item_hovered_with_flags()`.
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ItemHoveredFlags: i32 {
        /// Return true if directly over the item, not obstructed by another window, not obstructed by an active popup or modal blocking inputs under it.
        const NONE = sys::ImGuiHoveredFlags_None as i32;
        /// Return true even if a popup window is normally blocking access to this item/window
        const ALLOW_WHEN_BLOCKED_BY_POPUP = sys::ImGuiHoveredFlags_AllowWhenBlockedByPopup as i32;
        /// Return true even if an active item is blocking access to this item/window. Useful for Drag and Drop patterns.
        const ALLOW_WHEN_BLOCKED_BY_ACTIVE_ITEM = sys::ImGuiHoveredFlags_AllowWhenBlockedByActiveItem as i32;
        /// IsItemHovered() only: Return true even if the item uses AllowOverlap mode and is overlapped by another hoverable item.
        const ALLOW_WHEN_OVERLAPPED_BY_ITEM = sys::ImGuiHoveredFlags_AllowWhenOverlappedByItem as i32;
        /// IsItemHovered() only: Return true even if the item position is overlapped by another window.
        const ALLOW_WHEN_OVERLAPPED_BY_WINDOW = sys::ImGuiHoveredFlags_AllowWhenOverlappedByWindow as i32;
        /// IsItemHovered() only: Return true even if the position is obstructed or overlapped by another window
        const ALLOW_WHEN_OVERLAPPED = sys::ImGuiHoveredFlags_AllowWhenOverlapped as i32;
        /// IsItemHovered() only: Return true even if the item is disabled
        const ALLOW_WHEN_DISABLED = sys::ImGuiHoveredFlags_AllowWhenDisabled as i32;
        /// IsItemHovered() only: Disable using gamepad/keyboard navigation state when active, always query mouse.
        const NO_NAV_OVERRIDE = sys::ImGuiHoveredFlags_NoNavOverride as i32;
        /// IsItemHovered() only: test rectangle visibility with popup/active-item/overlap bypasses.
        const RECT_ONLY = sys::ImGuiHoveredFlags_RectOnly as i32;
        /// Shortcut for standard flags when using IsItemHovered() + SetTooltip() sequence.
        const FOR_TOOLTIP = sys::ImGuiHoveredFlags_ForTooltip as i32;
        /// Require mouse to be stationary for style.HoverStationaryDelay (~0.15 sec) _at least one time_. After this, can move on same item/window. Using the stationary test tends to reduces the need for a long delay.
        const STATIONARY = sys::ImGuiHoveredFlags_Stationary as i32;
        /// IsItemHovered() only: Return true immediately (default). As opposed to IsItemHovered() returning true only after style.HoverDelayNormal elapsed (~0.30 sec)
        const DELAY_NONE = sys::ImGuiHoveredFlags_DelayNone as i32;
        /// IsItemHovered() only: Return true after style.HoverDelayShort elapsed (~0.10 sec)
        const DELAY_SHORT = sys::ImGuiHoveredFlags_DelayShort as i32;
        /// IsItemHovered() only: Return true after style.HoverDelayNormal elapsed (~0.30 sec)
        const DELAY_NORMAL = sys::ImGuiHoveredFlags_DelayNormal as i32;
        /// IsItemHovered() only: Disable shared delay system where moving from one item to a neighboring item keeps the previous timer for a short time (standard for tooltips with long delays)
        const NO_SHARED_DELAY = sys::ImGuiHoveredFlags_NoSharedDelay as i32;
    }
}

impl Default for ItemHoveredFlags {
    fn default() -> Self {
        ItemHoveredFlags::NONE
    }
}

bitflags! {
    /// Flags stored in style tooltip hover defaults.
    ///
    /// This is the item-hover subset that can be expanded by
    /// `ItemHoveredFlags::FOR_TOOLTIP`; it intentionally excludes `FOR_TOOLTIP`
    /// itself to avoid recursive tooltip-style storage.
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct TooltipHoveredFlags: i32 {
        /// No flags
        const NONE = sys::ImGuiHoveredFlags_None as i32;
        /// Return true even if a popup window is normally blocking access to this item/window
        const ALLOW_WHEN_BLOCKED_BY_POPUP = sys::ImGuiHoveredFlags_AllowWhenBlockedByPopup as i32;
        /// Return true even if an active item is blocking access to this item/window. Useful for Drag and Drop patterns.
        const ALLOW_WHEN_BLOCKED_BY_ACTIVE_ITEM = sys::ImGuiHoveredFlags_AllowWhenBlockedByActiveItem as i32;
        /// Return true even if the item uses AllowOverlap mode and is overlapped by another hoverable item.
        const ALLOW_WHEN_OVERLAPPED_BY_ITEM = sys::ImGuiHoveredFlags_AllowWhenOverlappedByItem as i32;
        /// Return true even if the item position is overlapped by another window.
        const ALLOW_WHEN_OVERLAPPED_BY_WINDOW = sys::ImGuiHoveredFlags_AllowWhenOverlappedByWindow as i32;
        /// Return true even if the position is obstructed or overlapped by another window.
        const ALLOW_WHEN_OVERLAPPED = sys::ImGuiHoveredFlags_AllowWhenOverlapped as i32;
        /// Return true even if the item is disabled.
        const ALLOW_WHEN_DISABLED = sys::ImGuiHoveredFlags_AllowWhenDisabled as i32;
        /// Disable using gamepad/keyboard navigation state when active, always query mouse.
        const NO_NAV_OVERRIDE = sys::ImGuiHoveredFlags_NoNavOverride as i32;
        /// Test rectangle visibility with popup/active-item/overlap bypasses.
        const RECT_ONLY = sys::ImGuiHoveredFlags_RectOnly as i32;
        /// Require mouse to be stationary for style.HoverStationaryDelay at least one time.
        const STATIONARY = sys::ImGuiHoveredFlags_Stationary as i32;
        /// Return true immediately.
        const DELAY_NONE = sys::ImGuiHoveredFlags_DelayNone as i32;
        /// Return true after style.HoverDelayShort elapsed.
        const DELAY_SHORT = sys::ImGuiHoveredFlags_DelayShort as i32;
        /// Return true after style.HoverDelayNormal elapsed.
        const DELAY_NORMAL = sys::ImGuiHoveredFlags_DelayNormal as i32;
        /// Disable shared delay system where moving between items preserves the previous timer.
        const NO_SHARED_DELAY = sys::ImGuiHoveredFlags_NoSharedDelay as i32;
    }
}

impl Default for TooltipHoveredFlags {
    fn default() -> Self {
        TooltipHoveredFlags::NONE
    }
}

pub(crate) fn validate_window_hovered_flags(caller: &str, flags: WindowHoveredFlags) {
    let unsupported = flags.bits() & !WindowHoveredFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiHoveredFlags window bits: 0x{unsupported:X}"
    );
}

pub(crate) fn validate_item_hovered_flags(caller: &str, flags: ItemHoveredFlags) {
    let unsupported = flags.bits() & !ItemHoveredFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiHoveredFlags item bits: 0x{unsupported:X}"
    );
}

pub(crate) fn validate_tooltip_hovered_flags(caller: &str, flags: TooltipHoveredFlags) {
    let unsupported = flags.bits() & !TooltipHoveredFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiHoveredFlags tooltip bits: 0x{unsupported:X}"
    );
}

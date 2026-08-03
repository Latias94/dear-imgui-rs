use crate::sys;

bitflags::bitflags! {
    /// Flags accepted when submitting a dockspace.
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct DockNodeFlags: i32 {
        /// No flags
        const NONE = sys::ImGuiDockNodeFlags_None as i32;
        /// Don't display the dockspace node but keep it alive. Windows docked into this dockspace node won't be undocked.
        const KEEP_ALIVE_ONLY = sys::ImGuiDockNodeFlags_KeepAliveOnly as i32;
        /// Disable docking over the Central Node, which will be always kept empty.
        const NO_DOCKING_OVER_CENTRAL_NODE = sys::ImGuiDockNodeFlags_NoDockingOverCentralNode as i32;
        /// Enable passthru dockspace: 1) DockSpace() will render a ImGuiCol_WindowBg background covering everything excepted the Central Node when empty. 2) When Central Node is empty: let inputs pass-through + won't display a DockingEmptyBg background.
        const PASSTHRU_CENTRAL_NODE = sys::ImGuiDockNodeFlags_PassthruCentralNode as i32;
        /// Disable other windows/nodes from splitting this node.
        const NO_DOCKING_SPLIT = sys::ImGuiDockNodeFlags_NoDockingSplit as i32;
        /// Disable resizing node using the splitter/separators. Useful with programmatically setup dockspaces.
        const NO_RESIZE = sys::ImGuiDockNodeFlags_NoResize as i32;
        /// Tab bar will automatically hide when there is a single window in the dock node.
        const AUTO_HIDE_TAB_BAR = sys::ImGuiDockNodeFlags_AutoHideTabBar as i32;
        /// Disable undocking this node.
        const NO_UNDOCKING = sys::ImGuiDockNodeFlags_NoUndocking as i32;
    }
}

pub(crate) fn validate_dock_node_flags(caller: &str, flags: DockNodeFlags) {
    let unsupported = flags.bits() & !DockNodeFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiDockNodeFlags bits: 0x{unsupported:X}"
    );
}

bitflags::bitflags! {
    /// Dock-node policies contributed by every window using a [`crate::WindowClass`].
    ///
    /// Dear ImGui combines these flags across all windows in the same dock node. Dockspace
    /// submission controls such as [`DockNodeFlags::KEEP_ALIVE_ONLY`] and
    /// [`DockNodeFlags::PASSTHRU_CENTRAL_NODE`] are intentionally not part of this type.
    /// Several policies mirror version-pinned experimental Dear ImGui node behavior, but remain
    /// memory-safe because they do not accept caller-owned pointers or topology identities.
    ///
    /// These policies are a separate semantic domain from dockspace submission flags:
    ///
    /// ```compile_fail
    /// # use dear_imgui_rs::{DockNodeFlags, WindowClass};
    /// let class = WindowClass::default()
    ///     .dock_node_flags_override_set(DockNodeFlags::NO_RESIZE);
    /// # let _ = class;
    /// ```
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct WindowClassDockNodeFlags: i32 {
        /// No dock-node policy.
        const NONE = sys::ImGuiDockNodeFlags_None as i32;
        /// Prevent docking over a central node.
        const NO_DOCKING_OVER_CENTRAL_NODE =
            sys::ImGuiDockNodeFlags_NoDockingOverCentralNode as i32;
        /// Prevent other windows or nodes from splitting this node.
        const NO_DOCKING_SPLIT = sys::ImGuiDockNodeFlags_NoDockingSplit as i32;
        /// Prevent splitter resizing.
        const NO_RESIZE = sys::ImGuiDockNodeFlags_NoResize as i32;
        /// Hide the tab bar automatically when the node contains one window.
        const AUTO_HIDE_TAB_BAR = sys::ImGuiDockNodeFlags_AutoHideTabBar as i32;
        /// Prevent windows from being undocked from this node.
        const NO_UNDOCKING = sys::ImGuiDockNodeFlags_NoUndocking as i32;
        /// Never create a tab bar for this node.
        const NO_TAB_BAR = sys::ImGuiDockNodeFlags_NoTabBar as i32;
        /// Keep the existing tab bar hidden.
        const HIDDEN_TAB_BAR = sys::ImGuiDockNodeFlags_HiddenTabBar as i32;
        /// Hide the tab-bar window menu button.
        const NO_WINDOW_MENU_BUTTON = sys::ImGuiDockNodeFlags_NoWindowMenuButton as i32;
        /// Hide the tab-bar close button.
        const NO_CLOSE_BUTTON = sys::ImGuiDockNodeFlags_NoCloseButton as i32;
        /// Prevent horizontal splitter resizing.
        const NO_RESIZE_X = sys::ImGuiDockNodeFlags_NoResizeX as i32;
        /// Prevent vertical splitter resizing.
        const NO_RESIZE_Y = sys::ImGuiDockNodeFlags_NoResizeY as i32;
        /// Include docked windows in their host window's focus route.
        const DOCKED_WINDOWS_IN_FOCUS_ROUTE =
            sys::ImGuiDockNodeFlags_DockedWindowsInFocusRoute as i32;
        /// Prevent this node from splitting another node while docking.
        const NO_DOCKING_SPLIT_OTHER = sys::ImGuiDockNodeFlags_NoDockingSplitOther as i32;
        /// Prevent other payloads from docking over this node.
        const NO_DOCKING_OVER_ME = sys::ImGuiDockNodeFlags_NoDockingOverMe as i32;
        /// Prevent this payload from docking over another non-empty node.
        const NO_DOCKING_OVER_OTHER = sys::ImGuiDockNodeFlags_NoDockingOverOther as i32;
        /// Prevent this payload from docking over an empty node.
        const NO_DOCKING_OVER_EMPTY = sys::ImGuiDockNodeFlags_NoDockingOverEmpty as i32;
        /// Prevent all docking operations involving this node.
        const NO_DOCKING = sys::ImGuiDockNodeFlags_NoDocking as i32;
    }
}

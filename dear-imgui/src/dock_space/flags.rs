use crate::sys;

bitflags::bitflags! {
    /// Flags for dock nodes
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

// Re-export DockNodeFlags for convenience
pub use DockNodeFlags as DockFlags;

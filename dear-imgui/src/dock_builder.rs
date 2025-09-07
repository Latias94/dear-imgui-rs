//! DockBuilder API for programmatic dock layout creation
//!
//! This module provides the DockBuilder API which allows you to programmatically
//! create and manage dock layouts. This is useful for creating complex docking
//! layouts that would be difficult to achieve through manual docking.
//!
//! # Features
//!
//! This module is only available when the `docking` feature is enabled (default).
//!
//! # Basic Usage
//!
//! ```no_run
//! # use dear_imgui::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! // Create a dockspace
//! let dockspace_id = ui.dockspace_over_main_viewport();
//!
//! // Use DockBuilder to create a layout
//! let left_id = DockBuilder::split_node(dockspace_id, SplitDirection::Left, 0.3, None);
//! let right_id = DockBuilder::split_node(dockspace_id, SplitDirection::Right, 0.7, None);
//!
//! // Dock windows to specific nodes
//! DockBuilder::dock_window("Tool Panel", left_id);
//! DockBuilder::dock_window("Main View", right_id);
//!
//! // Finish the layout
//! DockBuilder::finish(dockspace_id);
//! ```

use crate::sys;
use std::ffi::CString;
use std::ptr;

/// Direction for splitting dock nodes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    /// Split to the left
    Left,
    /// Split to the right
    Right,
    /// Split upward
    Up,
    /// Split downward
    Down,
}

impl From<SplitDirection> for sys::ImGuiDir {
    fn from(dir: SplitDirection) -> Self {
        match dir {
            SplitDirection::Left => sys::ImGuiDir_Left,
            SplitDirection::Right => sys::ImGuiDir_Right,
            SplitDirection::Up => sys::ImGuiDir_Up,
            SplitDirection::Down => sys::ImGuiDir_Down,
        }
    }
}

/// DockBuilder API for programmatic dock layout creation
pub struct DockBuilder;

impl DockBuilder {
    /// Gets a dock node by its ID
    ///
    /// # Parameters
    ///
    /// * `node_id` - The ID of the dock node to retrieve
    ///
    /// # Returns
    ///
    /// A pointer to the dock node, or null if not found
    ///
    /// # Safety
    ///
    /// The returned pointer is only valid until the next ImGui frame.
    /// Do not store this pointer across frames.
    #[doc(alias = "DockBuilderGetNode")]
    pub fn get_node(node_id: sys::ImGuiID) -> *mut sys::ImGuiDockNode {
        unsafe { sys::ImGui_DockBuilderGetNode(node_id) }
    }

    /// Adds a new dock node
    ///
    /// # Parameters
    ///
    /// * `node_id` - The ID for the new dock node (use 0 to auto-generate)
    /// * `flags` - Dock node flags
    ///
    /// # Returns
    ///
    /// The ID of the created dock node
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui::*;
    /// let node_id = DockBuilder::add_node(0, DockNodeFlags::NO_RESIZE);
    /// ```
    #[doc(alias = "DockBuilderAddNode")]
    pub fn add_node(node_id: sys::ImGuiID, flags: crate::DockNodeFlags) -> sys::ImGuiID {
        unsafe { sys::ImGui_DockBuilderAddNode(node_id, flags.bits()) }
    }

    /// Removes a dock node
    ///
    /// # Parameters
    ///
    /// * `node_id` - The ID of the dock node to remove
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui::*;
    /// DockBuilder::remove_node(123);
    /// ```
    #[doc(alias = "DockBuilderRemoveNode")]
    pub fn remove_node(node_id: sys::ImGuiID) {
        unsafe { sys::ImGui_DockBuilderRemoveNode(node_id) }
    }

    /// Removes all docked windows from a node
    ///
    /// # Parameters
    ///
    /// * `node_id` - The ID of the dock node
    /// * `clear_settings_refs` - Whether to clear settings references
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui::*;
    /// DockBuilder::remove_node_docked_windows(123, true);
    /// ```
    #[doc(alias = "DockBuilderRemoveNodeDockedWindows")]
    pub fn remove_node_docked_windows(node_id: sys::ImGuiID, clear_settings_refs: bool) {
        unsafe { sys::ImGui_DockBuilderRemoveNodeDockedWindows(node_id, clear_settings_refs) }
    }

    /// Removes all child nodes from a dock node
    ///
    /// # Parameters
    ///
    /// * `node_id` - The ID of the dock node
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui::*;
    /// DockBuilder::remove_node_child_nodes(123);
    /// ```
    #[doc(alias = "DockBuilderRemoveNodeChildNodes")]
    pub fn remove_node_child_nodes(node_id: sys::ImGuiID) {
        unsafe { sys::ImGui_DockBuilderRemoveNodeChildNodes(node_id) }
    }

    /// Sets the position of a dock node
    ///
    /// # Parameters
    ///
    /// * `node_id` - The ID of the dock node
    /// * `pos` - The position in pixels
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui::*;
    /// DockBuilder::set_node_pos(123, [100.0, 50.0]);
    /// ```
    #[doc(alias = "DockBuilderSetNodePos")]
    pub fn set_node_pos(node_id: sys::ImGuiID, pos: [f32; 2]) {
        unsafe {
            let pos_vec = sys::ImVec2 {
                x: pos[0],
                y: pos[1],
            };
            sys::ImGui_DockBuilderSetNodePos(node_id, pos_vec)
        }
    }

    /// Sets the size of a dock node
    ///
    /// # Parameters
    ///
    /// * `node_id` - The ID of the dock node
    /// * `size` - The size in pixels
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui::*;
    /// DockBuilder::set_node_size(123, [800.0, 600.0]);
    /// ```
    #[doc(alias = "DockBuilderSetNodeSize")]
    pub fn set_node_size(node_id: sys::ImGuiID, size: [f32; 2]) {
        unsafe {
            let size_vec = sys::ImVec2 {
                x: size[0],
                y: size[1],
            };
            sys::ImGui_DockBuilderSetNodeSize(node_id, size_vec)
        }
    }

    /// Splits a dock node into two nodes
    ///
    /// # Parameters
    ///
    /// * `node_id` - The ID of the dock node to split
    /// * `split_dir` - The direction to split
    /// * `size_ratio_for_node_at_dir` - The size ratio for the new node (0.0 to 1.0)
    /// * `out_id_at_dir` - Optional output for the ID of the new node in the split direction
    ///
    /// # Returns
    ///
    /// The ID of the remaining node (opposite to the split direction)
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui::*;
    /// let dockspace_id = 1;
    /// let left_id = DockBuilder::split_node(dockspace_id, SplitDirection::Left, 0.3, None);
    /// ```
    #[doc(alias = "DockBuilderSplitNode")]
    pub fn split_node(
        node_id: sys::ImGuiID,
        split_dir: SplitDirection,
        size_ratio_for_node_at_dir: f32,
        out_id_at_dir: Option<&mut sys::ImGuiID>,
    ) -> sys::ImGuiID {
        unsafe {
            let out_ptr = if let Some(out) = out_id_at_dir {
                out as *mut _
            } else {
                ptr::null_mut()
            };
            sys::ImGui_DockBuilderSplitNode(
                node_id,
                split_dir.into(),
                size_ratio_for_node_at_dir,
                out_ptr,
                ptr::null_mut(),
            )
        }
    }

    /// Docks a window to a specific dock node
    ///
    /// # Parameters
    ///
    /// * `window_name` - The name of the window to dock
    /// * `node_id` - The ID of the dock node to dock the window to
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui::*;
    /// DockBuilder::dock_window("My Tool", 123);
    /// ```
    #[doc(alias = "DockBuilderDockWindow")]
    pub fn dock_window(window_name: &str, node_id: sys::ImGuiID) {
        let c_name = CString::new(window_name).expect("Window name contained null byte");
        unsafe { sys::ImGui_DockBuilderDockWindow(c_name.as_ptr(), node_id) }
    }

    /// Finishes the dock builder operations
    ///
    /// This function should be called after all dock builder operations are complete
    /// to finalize the layout.
    ///
    /// # Parameters
    ///
    /// * `node_id` - The root node ID of the dock layout
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui::*;
    /// // ... create layout ...
    /// DockBuilder::finish(dockspace_id);
    /// ```
    #[doc(alias = "DockBuilderFinish")]
    pub fn finish(node_id: sys::ImGuiID) {
        unsafe { sys::ImGui_DockBuilderFinish(node_id) }
    }
}

//! DockBuilder API for programmatic dock layout creation
//!
//! This module provides the DockBuilder API which allows you to programmatically
//! create and manage dock layouts. This is useful for creating complex docking
//! layouts that would be difficult to achieve through manual docking.
//!
//! # Notes
//!
//! Docking is always enabled in this crate; no feature flag required.
//!
//! # Basic Usage
//!
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! // Create a dockspace
//! let dockspace_id = ui.get_id("MyDockspace");
//! DockBuilder::add_node(dockspace_id, DockNodeFlags::NONE);
//!
//! // Split the dockspace: 30% left panel, 70% remaining
//! let (left_panel, main_area) = DockBuilder::split_node(
//!     dockspace_id,
//!     SplitDirection::Left,
//!     0.30
//! );
//!
//! // Further split the main area: 70% top, 30% bottom
//! let (top_area, bottom_area) = DockBuilder::split_node(
//!     main_area,
//!     SplitDirection::Down,
//!     0.30
//! );
//!
//! // Dock windows to specific nodes
//! DockBuilder::dock_window("Tool Panel", left_panel);
//! DockBuilder::dock_window("Main View", top_area);
//! DockBuilder::dock_window("Console", bottom_area);
//!
//! // Finish the layout
//! DockBuilder::finish(dockspace_id);
//! ```

use crate::Id;
use crate::sys;
use crate::ui::Ui;
use std::ffi::CString;
use std::marker::PhantomData;
use std::os::raw::c_void;
use std::slice;

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

/// Opaque reference to an ImGui dock node, valid for the duration of the current frame.
///
/// This wraps a raw `ImGuiDockNode*` and exposes a few read-only queries.
/// Instances are created via `DockBuilder::node()` / `DockBuilder::central_node()`
/// with a lifetime tied to a `Ui` reference.
pub struct DockNode<'ui> {
    raw: *mut sys::ImGuiDockNode,
    _phantom: PhantomData<&'ui Ui>,
}

/// Rectangle in screen space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeRect {
    pub min: [f32; 2],
    pub max: [f32; 2],
}

impl<'ui> DockNode<'ui> {
    /// Returns true if this node is the central node of its hierarchy
    pub fn is_central(&self) -> bool {
        unsafe { sys::ImGuiDockNode_IsCentralNode(self.raw) }
    }

    /// Returns true if this node is a dock space
    pub fn is_dock_space(&self) -> bool {
        unsafe { sys::ImGuiDockNode_IsDockSpace(self.raw) }
    }

    /// Returns true if this node is empty (no windows)
    pub fn is_empty(&self) -> bool {
        unsafe { sys::ImGuiDockNode_IsEmpty(self.raw) }
    }

    /// Returns true if this node is a split node
    pub fn is_split(&self) -> bool {
        unsafe { sys::ImGuiDockNode_IsSplitNode(self.raw) }
    }

    /// Returns true if this node is the root of its dock tree
    pub fn is_root(&self) -> bool {
        unsafe { sys::ImGuiDockNode_IsRootNode(self.raw) }
    }

    /// Returns true if this node is a floating node
    pub fn is_floating(&self) -> bool {
        unsafe { sys::ImGuiDockNode_IsFloatingNode(self.raw) }
    }

    /// Returns true if this node has its tab bar hidden
    pub fn is_hidden_tab_bar(&self) -> bool {
        unsafe { sys::ImGuiDockNode_IsHiddenTabBar(self.raw) }
    }

    /// Returns true if this node has no tab bar
    pub fn is_no_tab_bar(&self) -> bool {
        unsafe { sys::ImGuiDockNode_IsNoTabBar(self.raw) }
    }

    /// Returns true if this node is a leaf node
    pub fn is_leaf(&self) -> bool {
        unsafe { sys::ImGuiDockNode_IsLeafNode(self.raw) }
    }

    /// Returns the depth of this node within the dock tree
    pub fn depth(&self) -> i32 {
        unsafe { sys::igDockNodeGetDepth(self.raw as *const sys::ImGuiDockNode) as i32 }
    }

    /// Returns the menu button ID for this node
    pub fn window_menu_button_id(&self) -> sys::ImGuiID {
        unsafe { sys::igDockNodeGetWindowMenuButtonId(self.raw as *const sys::ImGuiDockNode) }
    }

    /// Returns the root node of this dock tree
    pub fn root<'a>(&self, _ui: &'a Ui) -> Option<DockNode<'a>> {
        let ptr = unsafe { sys::igDockNodeGetRootNode(self.raw) };
        if ptr.is_null() {
            None
        } else {
            Some(DockNode {
                raw: ptr,
                _phantom: PhantomData,
            })
        }
    }

    /// Returns true if `self` is in the hierarchy of `parent`
    pub fn is_in_hierarchy_of(&self, parent: &DockNode<'_>) -> bool {
        unsafe { sys::igDockNodeIsInHierarchyOf(self.raw, parent.raw) }
    }

    /// Returns the rectangle of this dock node in screen coordinates.
    pub fn rect(&self) -> NodeRect {
        let r = unsafe { sys::ImGuiDockNode_Rect(self.raw) };
        NodeRect {
            min: [r.Min.x, r.Min.y],
            max: [r.Max.x, r.Max.y],
        }
    }
}

impl DockBuilder {
    /// Returns a reference to a dock node by ID, scoped to the current frame.
    pub fn node<'ui>(_ui: &'ui Ui, node_id: Id) -> Option<DockNode<'ui>> {
        let ptr = unsafe { sys::igDockBuilderGetNode(node_id.into()) };
        if ptr.is_null() {
            None
        } else {
            Some(DockNode {
                raw: ptr,
                _phantom: PhantomData,
            })
        }
    }

    /// Returns the central node for a given dockspace ID, scoped to the current frame.
    pub fn central_node<'ui>(_ui: &'ui Ui, dockspace_id: Id) -> Option<DockNode<'ui>> {
        let ptr = unsafe { sys::igDockBuilderGetCentralNode(dockspace_id.into()) };
        if ptr.is_null() {
            None
        } else {
            Some(DockNode {
                raw: ptr,
                _phantom: PhantomData,
            })
        }
    }

    /// Returns true if a dock node with the given ID exists this frame.
    pub fn node_exists(ui: &Ui, node_id: Id) -> bool {
        Self::node(ui, node_id).is_some()
    }

    // Removed raw-pointer getter in favor of lifetime-scoped accessors.

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
    /// # use dear_imgui_rs::*;
    /// let node_id = DockBuilder::add_node(0.into(), DockNodeFlags::NO_RESIZE);
    /// ```
    #[doc(alias = "DockBuilderAddNode")]
    pub fn add_node(node_id: Id, flags: crate::DockNodeFlags) -> Id {
        unsafe { Id::from(sys::igDockBuilderAddNode(node_id.into(), flags.bits())) }
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
    /// # use dear_imgui_rs::*;
    /// DockBuilder::remove_node(123.into());
    /// ```
    #[doc(alias = "DockBuilderRemoveNode")]
    pub fn remove_node(node_id: Id) {
        unsafe { sys::igDockBuilderRemoveNode(node_id.into()) }
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
    /// # use dear_imgui_rs::*;
    /// DockBuilder::remove_node_docked_windows(123.into(), true);
    /// ```
    #[doc(alias = "DockBuilderRemoveNodeDockedWindows")]
    pub fn remove_node_docked_windows(node_id: Id, clear_settings_refs: bool) {
        unsafe { sys::igDockBuilderRemoveNodeDockedWindows(node_id.into(), clear_settings_refs) }
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
    /// # use dear_imgui_rs::*;
    /// DockBuilder::remove_node_child_nodes(123.into());
    /// ```
    #[doc(alias = "DockBuilderRemoveNodeChildNodes")]
    pub fn remove_node_child_nodes(node_id: Id) {
        unsafe { sys::igDockBuilderRemoveNodeChildNodes(node_id.into()) }
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
    /// # use dear_imgui_rs::*;
    /// DockBuilder::set_node_pos(123.into(), [100.0, 50.0]);
    /// ```
    #[doc(alias = "DockBuilderSetNodePos")]
    pub fn set_node_pos(node_id: Id, pos: [f32; 2]) {
        unsafe {
            let pos_vec = sys::ImVec2 {
                x: pos[0],
                y: pos[1],
            };
            sys::igDockBuilderSetNodePos(node_id.into(), pos_vec)
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
    /// # use dear_imgui_rs::*;
    /// DockBuilder::set_node_size(123.into(), [800.0, 600.0]);
    /// ```
    #[doc(alias = "DockBuilderSetNodeSize")]
    pub fn set_node_size(node_id: Id, size: [f32; 2]) {
        unsafe {
            let size_vec = sys::ImVec2 {
                x: size[0],
                y: size[1],
            };
            sys::igDockBuilderSetNodeSize(node_id.into(), size_vec)
        }
    }

    /// Splits a dock node into two child nodes.
    ///
    /// This function splits the specified dock node in the given direction, creating two child nodes.
    /// The original node becomes a parent node containing the two new child nodes.
    ///
    /// # Parameters
    ///
    /// * `node_id` - The ID of the dock node to split
    /// * `split_dir` - The direction to split (Left, Right, Up, or Down)
    /// * `size_ratio_for_node_at_dir` - The size ratio for the new node in the split direction (0.0 to 1.0)
    ///
    /// # Returns
    ///
    /// A tuple `(id_at_dir, id_at_opposite_dir)` containing:
    /// - `id_at_dir`: The ID of the new node in the split direction
    /// - `id_at_opposite_dir`: The ID of the new node in the opposite direction
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let dockspace_id = ui.get_id("MyDockspace");
    /// DockBuilder::add_node(dockspace_id, DockNodeFlags::NONE);
    ///
    /// // Split the dockspace: 20% left panel, 80% remaining
    /// let (left_panel, main_area) = DockBuilder::split_node(
    ///     dockspace_id,
    ///     SplitDirection::Left,
    ///     0.20
    /// );
    ///
    /// // Further split the main area: 70% top, 30% bottom
    /// let (top_area, bottom_area) = DockBuilder::split_node(
    ///     main_area,
    ///     SplitDirection::Down,
    ///     0.30
    /// );
    ///
    /// // Dock windows to the created nodes
    /// DockBuilder::dock_window("Left Panel", left_panel);
    /// DockBuilder::dock_window("Main View", top_area);
    /// DockBuilder::dock_window("Console", bottom_area);
    /// DockBuilder::finish(dockspace_id);
    /// ```
    ///
    /// # Notes
    ///
    /// - Make sure to call `DockBuilder::set_node_size()` before splitting if you want reliable split sizes
    /// - The original `node_id` becomes a parent node after splitting
    /// - Call `DockBuilder::finish()` after all layout operations are complete
    #[doc(alias = "DockBuilderSplitNode")]
    pub fn split_node(
        node_id: Id,
        split_dir: SplitDirection,
        size_ratio_for_node_at_dir: f32,
    ) -> (Id, Id) {
        unsafe {
            let mut id_at_dir: sys::ImGuiID = 0;
            let mut id_at_opposite: sys::ImGuiID = 0;
            let _ = sys::igDockBuilderSplitNode(
                node_id.into(),
                split_dir.into(),
                size_ratio_for_node_at_dir,
                &mut id_at_dir,
                &mut id_at_opposite,
            );
            (Id::from(id_at_dir), Id::from(id_at_opposite))
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
    /// # use dear_imgui_rs::*;
    /// DockBuilder::dock_window("My Tool", 123.into());
    /// ```
    #[doc(alias = "DockBuilderDockWindow")]
    pub fn dock_window(window_name: &str, node_id: Id) {
        let window_name_ptr = crate::string::tls_scratch_txt(window_name);
        unsafe { sys::igDockBuilderDockWindow(window_name_ptr, node_id.into()) }
    }

    // Removed raw-pointer central-node getter in favor of lifetime-scoped accessor.

    /// Copies a dockspace layout from `src_dockspace_id` to `dst_dockspace_id`.
    ///
    /// This variant does not provide window remap pairs and will copy windows by name.
    /// For advanced remapping, prefer using the raw sys bindings.
    #[doc(alias = "DockBuilderCopyDockSpace")]
    pub fn copy_dock_space(src_dockspace_id: Id, dst_dockspace_id: Id) {
        unsafe {
            sys::igDockBuilderCopyDockSpace(
                src_dockspace_id.into(),
                dst_dockspace_id.into(),
                std::ptr::null_mut(),
            )
        }
    }

    /// Copies a single dock node from `src_node_id` to `dst_node_id`.
    ///
    /// This variant does not return node remap pairs. For detailed remap output,
    /// use the raw sys bindings and provide an `ImVector_ImGuiID` buffer.
    #[doc(alias = "DockBuilderCopyNode")]
    pub fn copy_node(src_node_id: Id, dst_node_id: Id) {
        unsafe {
            sys::igDockBuilderCopyNode(src_node_id.into(), dst_node_id.into(), std::ptr::null_mut())
        }
    }

    /// Copies persistent window docking settings from `src_name` to `dst_name`.
    #[doc(alias = "DockBuilderCopyWindowSettings")]
    pub fn copy_window_settings(src_name: &str, dst_name: &str) {
        let (src_ptr, dst_ptr) = crate::string::tls_scratch_txt_two(src_name, dst_name);
        unsafe { sys::igDockBuilderCopyWindowSettings(src_ptr, dst_ptr) }
    }

    /// Copies a dockspace layout with explicit window name remapping.
    ///
    /// Provide pairs of (src_window_name, dst_window_name). The vector will be flattened
    /// into `[src, dst, src, dst, ...]` as expected by ImGui.
    #[doc(alias = "DockBuilderCopyDockSpace")]
    pub fn copy_dock_space_with_window_remap(
        src_dockspace_id: Id,
        dst_dockspace_id: Id,
        window_remaps: &[(&str, &str)],
    ) {
        // Build CStrings and a contiguous array of const char* pointers
        let mut cstrings: Vec<CString> = Vec::with_capacity(window_remaps.len() * 2);
        for (src, dst) in window_remaps {
            cstrings.push(CString::new(*src).expect("Source window name contained null byte"));
            cstrings.push(CString::new(*dst).expect("Destination window name contained null byte"));
        }
        let ptrs: Vec<*const i8> = cstrings.iter().map(|s| s.as_ptr()).collect();
        let mut boxed: Box<[*const i8]> = ptrs.into_boxed_slice();
        let mut vec_in = sys::ImVector_const_charPtr {
            Size: boxed.len() as i32,
            Capacity: boxed.len() as i32,
            Data: boxed.as_mut_ptr(),
        };
        unsafe {
            sys::igDockBuilderCopyDockSpace(
                src_dockspace_id.into(),
                dst_dockspace_id.into(),
                &mut vec_in,
            );
        }
        // keep boxed + cstrings alive until after the call
        drop(boxed);
        drop(cstrings);
    }

    /// Copies a node and returns the node ID remap pairs as a vector
    /// of `(old_id, new_id)` tuples.
    #[doc(alias = "DockBuilderCopyNode")]
    pub fn copy_node_with_remap_out(src_node_id: Id, dst_node_id: Id) -> Vec<(Id, Id)> {
        let mut out = sys::ImVector_ImGuiID::default();
        unsafe {
            sys::igDockBuilderCopyNode(src_node_id.into(), dst_node_id.into(), &mut out);
        }
        let mut result: Vec<(Id, Id)> = Vec::new();
        unsafe {
            if !out.Data.is_null() && out.Size > 0 {
                let len = out.Size as usize;
                let slice_ids = slice::from_raw_parts(out.Data, len);
                // Interpret as pairs
                for pair in slice_ids.chunks_exact(2) {
                    result.push((Id::from(pair[0]), Id::from(pair[1])));
                }
                // Free the buffer allocated by ImGui (ImVector uses ImGui::MemAlloc)
                sys::igMemFree(out.Data as *mut c_void);
            }
        }
        result
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
    /// # use dear_imgui_rs::*;
    /// // ... create layout ...
    /// let dockspace_id: Id = 1.into(); // placeholder dockspace id for example
    /// DockBuilder::finish(dockspace_id);
    /// ```
    #[doc(alias = "DockBuilderFinish")]
    pub fn finish(node_id: Id) {
        unsafe { sys::igDockBuilderFinish(node_id.into()) }
    }
}

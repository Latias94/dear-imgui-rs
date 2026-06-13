use super::validation::dock_node_depth_from_i32;
use crate::sys;
use crate::ui::Ui;

/// Opaque reference to an ImGui dock node, valid for the duration of the current frame.
///
/// This wraps a raw `ImGuiDockNode*` and exposes a few read-only queries.
/// Instances are created via `DockBuilder::node()` / `DockBuilder::central_node()`
/// with a lifetime tied to a `Ui` reference.
pub struct DockNode<'ui> {
    raw: *mut sys::ImGuiDockNode,
    ui: &'ui Ui,
}

/// Rectangle in screen space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeRect {
    pub min: [f32; 2],
    pub max: [f32; 2],
}

pub(super) fn new_dock_node<'ui>(ui: &'ui Ui, raw: *mut sys::ImGuiDockNode) -> DockNode<'ui> {
    DockNode { raw, ui }
}

impl<'ui> DockNode<'ui> {
    /// Returns true if this node is the central node of its hierarchy
    pub fn is_central(&self) -> bool {
        self.ui
            .run_with_bound_context(|| unsafe { sys::ImGuiDockNode_IsCentralNode(self.raw) })
    }

    /// Returns true if this node is a dock space
    pub fn is_dock_space(&self) -> bool {
        self.ui
            .run_with_bound_context(|| unsafe { sys::ImGuiDockNode_IsDockSpace(self.raw) })
    }

    /// Returns true if this node is empty (no windows)
    pub fn is_empty(&self) -> bool {
        self.ui
            .run_with_bound_context(|| unsafe { sys::ImGuiDockNode_IsEmpty(self.raw) })
    }

    /// Returns true if this node is a split node
    pub fn is_split(&self) -> bool {
        self.ui
            .run_with_bound_context(|| unsafe { sys::ImGuiDockNode_IsSplitNode(self.raw) })
    }

    /// Returns true if this node is the root of its dock tree
    pub fn is_root(&self) -> bool {
        self.ui
            .run_with_bound_context(|| unsafe { sys::ImGuiDockNode_IsRootNode(self.raw) })
    }

    /// Returns true if this node is a floating node
    pub fn is_floating(&self) -> bool {
        self.ui
            .run_with_bound_context(|| unsafe { sys::ImGuiDockNode_IsFloatingNode(self.raw) })
    }

    /// Returns true if this node has its tab bar hidden
    pub fn is_hidden_tab_bar(&self) -> bool {
        self.ui
            .run_with_bound_context(|| unsafe { sys::ImGuiDockNode_IsHiddenTabBar(self.raw) })
    }

    /// Returns true if this node has no tab bar
    pub fn is_no_tab_bar(&self) -> bool {
        self.ui
            .run_with_bound_context(|| unsafe { sys::ImGuiDockNode_IsNoTabBar(self.raw) })
    }

    /// Returns true if this node is a leaf node
    pub fn is_leaf(&self) -> bool {
        self.ui
            .run_with_bound_context(|| unsafe { sys::ImGuiDockNode_IsLeafNode(self.raw) })
    }

    /// Returns the depth of this node within the dock tree
    pub fn depth(&self) -> usize {
        self.ui.run_with_bound_context(|| {
            dock_node_depth_from_i32(unsafe {
                sys::igDockNodeGetDepth(self.raw as *const sys::ImGuiDockNode) as i32
            })
        })
    }

    /// Returns the menu button ID for this node
    pub fn window_menu_button_id(&self) -> crate::Id {
        self.ui.run_with_bound_context(|| unsafe {
            crate::Id::from(sys::igDockNodeGetWindowMenuButtonId(
                self.raw as *const sys::ImGuiDockNode,
            ))
        })
    }

    /// Returns the root node of this dock tree
    pub fn root(&self) -> Option<DockNode<'ui>> {
        let ptr = self
            .ui
            .run_with_bound_context(|| unsafe { sys::igDockNodeGetRootNode(self.raw) });
        if ptr.is_null() {
            None
        } else {
            Some(new_dock_node(self.ui, ptr))
        }
    }

    /// Returns true if `self` is in the hierarchy of `parent`
    pub fn is_in_hierarchy_of(&self, parent: &DockNode<'_>) -> bool {
        assert!(
            self.ui.context_raw() == parent.ui.context_raw(),
            "DockNode::is_in_hierarchy_of() requires nodes from the same ImGui context"
        );
        self.ui.run_with_bound_context(|| unsafe {
            sys::igDockNodeIsInHierarchyOf(self.raw, parent.raw)
        })
    }

    /// Returns the rectangle of this dock node in screen coordinates.
    pub fn rect(&self) -> NodeRect {
        let r = self
            .ui
            .run_with_bound_context(|| unsafe { sys::ImGuiDockNode_Rect(self.raw) });
        NodeRect {
            min: [r.Min.x, r.Min.y],
            max: [r.Max.x, r.Max.y],
        }
    }
}

use crate::Id;
use crate::sys;
use crate::ui::Ui;
use crate::widget::TreeNodeFlags;

use super::{TreeNode, TreeNodeId, TreeNodeToken};

/// # Tree Node Widgets
impl Ui {
    /// Constructs a new tree node with just a name, and pushes it.
    ///
    /// Use [tree_node_config] to access a builder to put additional
    /// configurations on the tree node.
    ///
    /// [tree_node_config]: Self::tree_node_config
    pub fn tree_node<I, T>(&self, id: I) -> Option<TreeNodeToken<'_>>
    where
        I: Into<TreeNodeId<T>>,
        T: AsRef<str>,
    {
        self.tree_node_config(id).push()
    }

    /// Constructs a new tree node builder.
    ///
    /// Use [tree_node] to build a simple node with just a name.
    ///
    /// [tree_node]: Self::tree_node
    pub fn tree_node_config<I, T>(&self, id: I) -> TreeNode<'_, T>
    where
        I: Into<TreeNodeId<T>>,
        T: AsRef<str>,
    {
        TreeNode::new(id.into(), self)
    }

    /// Creates a collapsing header widget
    #[doc(alias = "CollapsingHeader")]
    pub fn collapsing_header(&self, label: impl AsRef<str>, flags: TreeNodeFlags) -> bool {
        let label_ptr = self.scratch_txt(label);
        unsafe { sys::igCollapsingHeader_TreeNodeFlags(label_ptr, flags.bits()) }
    }

    /// Creates a collapsing header widget with a visibility tracking variable.
    ///
    /// Passing `visible` enables a close button on the header. When clicked, ImGui will set
    /// `*visible = false`. As with other immediate-mode widgets, you should stop submitting the
    /// header when `*visible == false`.
    #[doc(alias = "CollapsingHeader")]
    pub fn collapsing_header_with_visible(
        &self,
        label: impl AsRef<str>,
        visible: &mut bool,
        flags: TreeNodeFlags,
    ) -> bool {
        let label_ptr = self.scratch_txt(label);
        unsafe { sys::igCollapsingHeader_BoolPtr(label_ptr, visible as *mut bool, flags.bits()) }
    }

    /// Returns the distance from the start of a tree node to the label text.
    #[doc(alias = "GetTreeNodeToLabelSpacing")]
    pub fn tree_node_to_label_spacing(&self) -> f32 {
        unsafe { sys::igGetTreeNodeToLabelSpacing() }
    }

    /// Returns whether the tree node identified by `storage_id` is open in storage.
    #[doc(alias = "TreeNodeGetOpen")]
    pub fn tree_node_get_open(&self, storage_id: Id) -> bool {
        unsafe { sys::igTreeNodeGetOpen(storage_id.raw()) }
    }
}

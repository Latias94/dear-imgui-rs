use crate::Id;
use crate::sys;
use crate::ui::Ui;

use super::{TreeNode, TreeNodeFlags, TreeNodeId, TreeNodeToken};

/// # Tree Node Widgets
impl Ui {
    /// Constructs a new tree node with just a name, and pushes it.
    ///
    /// Use [tree_node_config] to access a builder to put additional
    /// configurations on the tree node.
    ///
    /// [tree_node_config]: Self::tree_node_config
    #[doc(alias = "TreeNode", alias = "TreeNodeEx")]
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

    /// Starts a tree indentation and ID scope without rendering a tree node.
    ///
    /// The returned token restores the tree depth, indentation, and ID stack
    /// when dropped.
    #[doc(alias = "TreePush")]
    pub fn tree_push(&self, id: impl AsRef<str>) -> TreeNodeToken<'_> {
        let id = self.scratch_txt(id);
        self.run_with_bound_context(|| unsafe { sys::igTreePush_Str(id) });
        TreeNodeToken::new(self)
    }

    /// Starts a tree indentation and ID scope using a pointer value as the ID.
    ///
    /// The pointer is used only as an identifier and is not dereferenced.
    #[doc(alias = "TreePush")]
    pub fn tree_push_ptr<T>(&self, id: *const T) -> TreeNodeToken<'_> {
        self.run_with_bound_context(|| unsafe { sys::igTreePush_Ptr(id.cast()) });
        TreeNodeToken::new(self)
    }

    /// Creates a collapsing header widget
    #[doc(alias = "CollapsingHeader")]
    pub fn collapsing_header(&self, label: impl AsRef<str>, flags: TreeNodeFlags) -> bool {
        let label_ptr = self.scratch_txt(label);
        self.run_with_bound_context(|| unsafe {
            sys::igCollapsingHeader_TreeNodeFlags(label_ptr, flags.bits())
        })
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
        self.run_with_bound_context(|| unsafe {
            sys::igCollapsingHeader_BoolPtr(label_ptr, visible as *mut bool, flags.bits())
        })
    }

    /// Returns the distance from the start of a tree node to the label text.
    #[doc(alias = "GetTreeNodeToLabelSpacing")]
    pub fn tree_node_to_label_spacing(&self) -> f32 {
        self.run_with_bound_context(|| unsafe { sys::igGetTreeNodeToLabelSpacing() })
    }

    /// Returns whether the tree node identified by `storage_id` is open in storage.
    #[doc(alias = "TreeNodeGetOpen")]
    pub fn tree_node_get_open(&self, storage_id: Id) -> bool {
        self.run_with_bound_context(|| unsafe { sys::igTreeNodeGetOpen(storage_id.raw()) })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn manual_tree_scopes_restore_tree_depth_and_id_stack() {
        let mut ctx = crate::Context::create();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        let _ = ctx.font_atlas_mut().build();
        let ui = ctx.frame();

        ui.window("tree_push").build(|| {
            let window = unsafe { crate::sys::igGetCurrentWindowRead() };
            let initial_depth = unsafe { (*window).DC.TreeDepth };
            let initial_id_stack_size = unsafe { (*window).IDStack.Size };

            {
                let _tree = ui.tree_push("string_scope");
                assert_eq!(unsafe { (*window).DC.TreeDepth }, initial_depth + 1);
                assert_eq!(unsafe { (*window).IDStack.Size }, initial_id_stack_size + 1);
            }

            assert_eq!(unsafe { (*window).DC.TreeDepth }, initial_depth);
            assert_eq!(unsafe { (*window).IDStack.Size }, initial_id_stack_size);

            let marker = 0_u8;
            let tree = ui.tree_push_ptr(&marker);
            assert_eq!(unsafe { (*window).DC.TreeDepth }, initial_depth + 1);
            tree.pop();
            assert_eq!(unsafe { (*window).DC.TreeDepth }, initial_depth);
            assert_eq!(unsafe { (*window).IDStack.Size }, initial_id_stack_size);
        });
    }

    #[test]
    fn tree_node_tokens_match_push_flags_and_keep_custom_ids_stable() {
        let mut ctx = crate::Context::create();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        let _ = ctx.font_atlas_mut().build();
        let ui = ctx.frame();

        ui.window("tree_node_tokens").build(|| {
            let window = unsafe { crate::sys::igGetCurrentWindowRead() };
            let initial_depth = unsafe { (*window).DC.TreeDepth };
            let initial_id_stack_size = unsafe { (*window).IDStack.Size };

            let no_push = ui
                .tree_node_config("no_push")
                .opened(true, crate::Condition::Always)
                .no_tree_push_on_open(true)
                .push()
                .expect("forced-open tree node");
            assert_eq!(unsafe { (*window).DC.TreeDepth }, initial_depth);
            assert_eq!(unsafe { (*window).IDStack.Size }, initial_id_stack_size);
            drop(no_push);
            assert_eq!(unsafe { (*window).DC.TreeDepth }, initial_depth);
            assert_eq!(unsafe { (*window).IDStack.Size }, initial_id_stack_size);

            let first = ui
                .tree_node_config("stable_id")
                .label("First label")
                .opened(true, crate::Condition::Always)
                .push()
                .expect("forced-open tree node");
            let first_id = ui.item_id();
            drop(first);

            let second = ui
                .tree_node_config("stable_id")
                .label("Second label")
                .opened(true, crate::Condition::Always)
                .push()
                .expect("forced-open tree node");
            let second_id = ui.item_id();
            drop(second);
            assert_eq!(first_id, second_id);

            let first_int = ui
                .tree_node_config(7_i32)
                .label("First integer label")
                .opened(true, crate::Condition::Always)
                .nav_left_jumps_back_here(true)
                .push()
                .expect("forced-open tree node");
            let first_int_id = ui.item_id();
            assert_eq!(unsafe { (*window).DC.TreeDepth }, initial_depth + 1);
            assert_eq!(unsafe { (*window).IDStack.Size }, initial_id_stack_size + 1);
            drop(first_int);

            let second_int = ui
                .tree_node_config(7_i32)
                .label("Second integer label")
                .opened(true, crate::Condition::Always)
                .push()
                .expect("forced-open tree node");
            let second_int_id = ui.item_id();
            drop(second_int);
            assert_eq!(first_int_id, second_int_id);
            assert_eq!(unsafe { (*window).DC.TreeDepth }, initial_depth);
            assert_eq!(unsafe { (*window).IDStack.Size }, initial_id_stack_size);
        });
    }
}

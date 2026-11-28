//! Trees and collapsing headers
//!
//! Tree nodes and collapsing headers for hierarchical content. See
//! `TreeNodeFlags` for customization options.
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]

use crate::scope::{NativeScopePop, NativeScopeToken};
use crate::{Condition, Id, Ui, sys};

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
        self.with_scratch_txt(id, |id_ptr| unsafe { sys::igTreePush_Str(id_ptr) });
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
        self.with_scratch_txt(label, |label_ptr| unsafe {
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
        self.with_scratch_txt(label, |label_ptr| unsafe {
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

bitflags::bitflags! {
    /// Flags for tree node widgets
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct TreeNodeFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Draw as selected
        const SELECTED = sys::ImGuiTreeNodeFlags_Selected;
        /// Draw frame with background (e.g. for CollapsingHeader)
        const FRAMED = sys::ImGuiTreeNodeFlags_Framed;
        /// Hit testing to allow subsequent widgets to overlap this one
        const ALLOW_ITEM_OVERLAP = sys::ImGuiTreeNodeFlags_AllowOverlap;
        /// Hit testing to allow subsequent widgets to overlap this one
        const ALLOW_OVERLAP = sys::ImGuiTreeNodeFlags_AllowOverlap;
        /// Don't do a TreePush() when open (e.g. for CollapsingHeader) = no extra indent nor pushing on ID stack
        const NO_TREE_PUSH_ON_OPEN = sys::ImGuiTreeNodeFlags_NoTreePushOnOpen;
        /// Don't automatically and temporarily open node when Logging is active (by default logging will automatically open tree nodes)
        const NO_AUTO_OPEN_ON_LOG = sys::ImGuiTreeNodeFlags_NoAutoOpenOnLog;
        /// Default node to be open
        const DEFAULT_OPEN = sys::ImGuiTreeNodeFlags_DefaultOpen;
        /// Need double-click to open node
        const OPEN_ON_DOUBLE_CLICK = sys::ImGuiTreeNodeFlags_OpenOnDoubleClick;
        /// Only open when clicking on the arrow part. If ImGuiTreeNodeFlags_OpenOnDoubleClick is also set, single-click arrow or double-click all box to open.
        const OPEN_ON_ARROW = sys::ImGuiTreeNodeFlags_OpenOnArrow;
        /// No collapsing, no arrow (use as a convenience for leaf nodes)
        const LEAF = sys::ImGuiTreeNodeFlags_Leaf;
        /// Display a bullet instead of arrow
        const BULLET = sys::ImGuiTreeNodeFlags_Bullet;
        /// Use FramePadding (even for an unframed text node) to vertically align text baseline to regular widget height. Equivalent to calling AlignTextToFramePadding().
        const FRAME_PADDING = sys::ImGuiTreeNodeFlags_FramePadding;
        /// Extend hit box to the right-most edge, even if not framed. This is not the default in order to allow adding other items on the same line. In the future we may refactor the hit system to be front-to-back, allowing natural overlaps and then this can become the default.
        const SPAN_AVAIL_WIDTH = sys::ImGuiTreeNodeFlags_SpanAvailWidth;
        /// Extend hit box to the left-most and right-most edges (bypass the indented area).
        const SPAN_FULL_WIDTH = sys::ImGuiTreeNodeFlags_SpanFullWidth;
        /// Narrow hit box and hover highlight to the label text width.
        const SPAN_LABEL_WIDTH = sys::ImGuiTreeNodeFlags_SpanLabelWidth;
        /// Frame spans all table columns while the label stays in its current column.
        const SPAN_ALL_COLUMNS = sys::ImGuiTreeNodeFlags_SpanAllColumns;
        /// Label will span all columns of its container table.
        const LABEL_SPAN_ALL_COLUMNS = sys::ImGuiTreeNodeFlags_LabelSpanAllColumns;
        /// (WIP) Nav: left direction goes to parent. Only for the tree node, not the tree push.
        const NAV_LEFT_JUMPS_BACK_HERE = sys::ImGuiTreeNodeFlags_NavLeftJumpsToParent;
        /// Framed header without a tree push or automatic opening during logging.
        const COLLAPSING_HEADER = sys::ImGuiTreeNodeFlags_CollapsingHeader;
        /// No tree hierarchy guide lines are drawn.
        const DRAW_LINES_NONE = sys::ImGuiTreeNodeFlags_DrawLinesNone;
        /// Draw full tree hierarchy guide lines.
        const DRAW_LINES_FULL = sys::ImGuiTreeNodeFlags_DrawLinesFull;
        /// Draw tree hierarchy guide lines only to nodes.
        const DRAW_LINES_TO_NODES = sys::ImGuiTreeNodeFlags_DrawLinesToNodes;
    }
}

/// Tree node ID that can be constructed from different types
#[derive(Copy, Clone, Debug)]
pub enum TreeNodeId<T> {
    Str(T),
    Ptr(*const u8),
    Int(i32),
}

impl<T> From<T> for TreeNodeId<T>
where
    T: AsRef<str>,
{
    fn from(s: T) -> Self {
        TreeNodeId::Str(s)
    }
}

impl From<*const u8> for TreeNodeId<&'static str> {
    fn from(ptr: *const u8) -> Self {
        TreeNodeId::Ptr(ptr)
    }
}

impl From<i32> for TreeNodeId<&'static str> {
    fn from(i: i32) -> Self {
        TreeNodeId::Int(i)
    }
}

/// Builder for a tree node widget
#[derive(Clone, Debug)]
#[must_use]
pub struct TreeNode<'a, T, L = &'static str> {
    id: TreeNodeId<T>,
    label: Option<L>,
    opened: bool,
    opened_cond: Option<Condition>,
    flags: TreeNodeFlags,
    ui: &'a Ui,
}

impl<'a, T: AsRef<str>> TreeNode<'a, T, &'static str> {
    fn new(id: TreeNodeId<T>, ui: &'a Ui) -> Self {
        TreeNode {
            id,
            label: None,
            opened: false,
            opened_cond: None,
            flags: TreeNodeFlags::NONE,
            ui,
        }
    }

    /// Sets a custom label for the tree node
    pub fn label<L: AsRef<str>>(self, label: L) -> TreeNode<'a, T, L> {
        TreeNode {
            id: self.id,
            label: Some(label),
            opened: self.opened,
            opened_cond: self.opened_cond,
            flags: self.flags,
            ui: self.ui,
        }
    }
}

impl<'a, T: AsRef<str>, L: AsRef<str>> TreeNode<'a, T, L> {
    /// Sets the opened state
    pub fn opened(mut self, opened: bool, cond: Condition) -> Self {
        self.opened = opened;
        self.opened_cond = Some(cond);
        self
    }

    /// Draw as selected
    pub fn selected(mut self, selected: bool) -> Self {
        self.flags.set(TreeNodeFlags::SELECTED, selected);
        self
    }

    /// Draw frame with background (e.g. for CollapsingHeader)
    pub fn framed(mut self, framed: bool) -> Self {
        self.flags.set(TreeNodeFlags::FRAMED, framed);
        self
    }

    /// Hit testing to allow subsequent widgets to overlap this one
    pub fn allow_item_overlap(mut self, allow: bool) -> Self {
        self.flags.set(TreeNodeFlags::ALLOW_ITEM_OVERLAP, allow);
        self
    }

    /// Don't do a TreePush() when open (e.g. for CollapsingHeader)
    pub fn no_tree_push_on_open(mut self, no_push: bool) -> Self {
        self.flags.set(TreeNodeFlags::NO_TREE_PUSH_ON_OPEN, no_push);
        self
    }

    /// Don't automatically and temporarily open node when Logging is active
    pub fn no_auto_open_on_log(mut self, no_auto: bool) -> Self {
        self.flags.set(TreeNodeFlags::NO_AUTO_OPEN_ON_LOG, no_auto);
        self
    }

    /// Default node to be open
    pub fn default_open(mut self, default_open: bool) -> Self {
        self.flags.set(TreeNodeFlags::DEFAULT_OPEN, default_open);
        self
    }

    /// Need double-click to open node
    pub fn open_on_double_click(mut self, double_click: bool) -> Self {
        self.flags
            .set(TreeNodeFlags::OPEN_ON_DOUBLE_CLICK, double_click);
        self
    }

    /// Only open when clicking on the arrow part
    pub fn open_on_arrow(mut self, arrow_only: bool) -> Self {
        self.flags.set(TreeNodeFlags::OPEN_ON_ARROW, arrow_only);
        self
    }

    /// No collapsing, no arrow (use as a convenience for leaf nodes)
    pub fn leaf(mut self, leaf: bool) -> Self {
        self.flags.set(TreeNodeFlags::LEAF, leaf);
        self
    }

    /// Display a bullet instead of arrow
    pub fn bullet(mut self, bullet: bool) -> Self {
        self.flags.set(TreeNodeFlags::BULLET, bullet);
        self
    }

    /// Use FramePadding to vertically align text baseline to regular widget height
    pub fn frame_padding(mut self, frame_padding: bool) -> Self {
        self.flags.set(TreeNodeFlags::FRAME_PADDING, frame_padding);
        self
    }

    /// Extend hit box to the right-most edge
    pub fn span_avail_width(mut self, span: bool) -> Self {
        self.flags.set(TreeNodeFlags::SPAN_AVAIL_WIDTH, span);
        self
    }

    /// Extend hit box to the left-most and right-most edges
    pub fn span_full_width(mut self, span: bool) -> Self {
        self.flags.set(TreeNodeFlags::SPAN_FULL_WIDTH, span);
        self
    }

    /// Left direction may move to this tree node from any of its child
    pub fn nav_left_jumps_back_here(mut self, nav: bool) -> Self {
        self.flags.set(TreeNodeFlags::NAV_LEFT_JUMPS_BACK_HERE, nav);
        self
    }

    /// Pushes a tree node and starts appending to it.
    ///
    /// Returns `Some(TreeNodeToken)` if the tree node is open. After content has been
    /// rendered, the token can be popped by calling `.pop()`.
    ///
    /// Returns `None` if the tree node is not open and no content should be rendered.
    pub fn push(self) -> Option<TreeNodeToken<'a>> {
        const FMT: &[u8; 3] = b"%s\0";
        let flags = self.flags.bits();
        let pushes_tree = !self.flags.contains(TreeNodeFlags::NO_TREE_PUSH_ON_OPEN);
        let set_next_item_open = || {
            if let Some(opened_cond) = self.opened_cond {
                unsafe { sys::igSetNextItemOpen(self.opened, opened_cond as i32) };
            }
        };

        let open = match &self.id {
            TreeNodeId::Str(id) => {
                if let Some(label) = self.label.as_ref() {
                    self.ui
                        .with_scratch_txt_two(id, label, |id_ptr, label_ptr| unsafe {
                            set_next_item_open();
                            sys::igTreeNodeEx_StrStr(id_ptr, flags, FMT.as_ptr().cast(), label_ptr)
                        })
                } else {
                    self.ui.with_scratch_txt(id, |label_ptr| unsafe {
                        set_next_item_open();
                        sys::igTreeNodeEx_Str(label_ptr, flags)
                    })
                }
            }
            TreeNodeId::Ptr(id) => {
                let label = self.label.as_ref().map_or("", AsRef::as_ref);
                self.ui.with_scratch_txt(label, |label_ptr| unsafe {
                    set_next_item_open();
                    sys::igTreeNodeEx_Ptr(id.cast(), flags, FMT.as_ptr().cast(), label_ptr)
                })
            }
            TreeNodeId::Int(id) => {
                let label = self.label.as_ref().map_or("", AsRef::as_ref);
                self.ui.with_scratch_txt(label, |label_ptr| unsafe {
                    set_next_item_open();
                    let id = sys::igGetID_Int(*id);
                    sys::igTreeNodeBehavior(id, flags, label_ptr, std::ptr::null())
                })
            }
        };

        if open {
            Some(TreeNodeToken::from_tree_node(self.ui, pushes_tree))
        } else {
            None
        }
    }
}

/// Tracks a tree node that can be popped by calling `.pop()` or by dropping.
///
/// Tree-node tokens share Dear ImGui's ID stack with [`crate::Ui::push_id`]. Tokens on that stack
/// must finish in reverse creation order and in their originating window `Begin` scope. Prefer a
/// tree-node closure builder for ordinary use.
#[must_use]
#[doc(alias = "TreePop")]
pub struct TreeNodeToken<'ui> {
    scope: Option<NativeScopeToken<'ui>>,
}

impl<'ui> TreeNodeToken<'ui> {
    /// Creates a token for an explicitly pushed tree scope.
    fn new(ui: &'ui Ui) -> Self {
        Self {
            scope: Some(ui.begin_native_scope(NativeScopePop::TreePop, "TreeNodeToken")),
        }
    }

    fn from_tree_node(ui: &'ui Ui, pop_tree_node: bool) -> Self {
        Self {
            scope: pop_tree_node
                .then(|| ui.begin_native_scope(NativeScopePop::TreePop, "TreeNodeToken")),
        }
    }

    /// Pops the tree node
    ///
    /// # Panics
    ///
    /// Panics before FFI if a later tree-node or ID token is active, or if this token is outside
    /// its originating window `Begin` scope.
    pub fn pop(self) {
        // The drop implementation will handle the actual popping
    }
}

impl Drop for TreeNodeToken<'_> {
    fn drop(&mut self) {
        if let Some(scope) = &mut self.scope {
            scope.finish();
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn manual_tree_scopes_restore_tree_depth_and_id_stack() {
        let mut ctx = crate::Context::create();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        ctx.font_atlas()
            .try_claim_legacy_renderer()
            .expect("legacy renderer font atlas should be available")
            .build();
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
        ctx.font_atlas()
            .try_claim_legacy_renderer()
            .expect("legacy renderer font atlas should be available")
            .build();
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

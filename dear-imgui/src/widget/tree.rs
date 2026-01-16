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
use crate::Condition;
use crate::sys;
use crate::ui::Ui;
use crate::widget::TreeNodeFlags;

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
        TreeNode {
            id: id.into(),
            label: None,
            opened: false,
            opened_cond: Condition::Never,
            flags: TreeNodeFlags::NONE,
            ui: self,
        }
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
}

/// Builder for a tree node widget
#[derive(Clone, Debug)]
#[must_use]
pub struct TreeNode<'a, T, L = &'static str> {
    id: TreeNodeId<T>,
    label: Option<L>,
    opened: bool,
    opened_cond: Condition,
    flags: TreeNodeFlags,
    ui: &'a Ui,
}

impl<'a, T: AsRef<str>> TreeNode<'a, T, &'static str> {
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
        self.opened_cond = cond;
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

    /// Span the tree node of its container table
    pub fn span_all_columns(mut self, value: bool) -> Self {
        self.flags.set(TreeNodeFlags::SPAN_ALL_COLUMNS, value);
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
        let open = unsafe {
            if self.opened_cond != Condition::Never {
                sys::igSetNextItemOpen(self.opened, self.opened_cond as i32);
            }

            match &self.id {
                TreeNodeId::Str(s) => {
                    if let Some(label) = self.label.as_ref() {
                        let (id_ptr, label_ptr) = self.ui.scratch_txt_two(s, label);
                        sys::igPushID_Str(id_ptr);
                        let open = sys::igTreeNodeEx_Str(label_ptr, self.flags.bits());
                        sys::igPopID();
                        open
                    } else {
                        let label_ptr = self.ui.scratch_txt(s);
                        sys::igTreeNodeEx_Str(label_ptr, self.flags.bits())
                    }
                }
                TreeNodeId::Ptr(ptr) => {
                    let label = self.label.as_ref().map_or("", |l| l.as_ref());
                    let label_ptr = self.ui.scratch_txt(label);
                    sys::igPushID_Ptr(*ptr as *const std::os::raw::c_void);
                    let open = sys::igTreeNodeEx_Str(label_ptr, self.flags.bits());
                    sys::igPopID();
                    open
                }
                TreeNodeId::Int(i) => {
                    let label = self.label.as_ref().map_or("", |l| l.as_ref());
                    let label_ptr = self.ui.scratch_txt(label);
                    sys::igPushID_Int(*i);
                    let open = sys::igTreeNodeEx_Str(label_ptr, self.flags.bits());
                    sys::igPopID();
                    open
                }
            }
        };

        if open {
            Some(TreeNodeToken::new(self.ui))
        } else {
            None
        }
    }
}

/// Tracks a tree node that can be popped by calling `.pop()` or by dropping
#[must_use]
pub struct TreeNodeToken<'ui> {
    ui: &'ui Ui,
}

impl<'ui> TreeNodeToken<'ui> {
    /// Creates a new tree node token
    fn new(ui: &'ui Ui) -> Self {
        TreeNodeToken { ui }
    }

    /// Pops the tree node
    pub fn pop(self) {
        // The drop implementation will handle the actual popping
    }
}

impl<'ui> Drop for TreeNodeToken<'ui> {
    fn drop(&mut self) {
        unsafe {
            sys::igTreePop();
        }
    }
}

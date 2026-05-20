use crate::sys;

bitflags::bitflags! {
    /// Flags for tree node widgets
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct TreeNodeFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Draw as selected
        const SELECTED = sys::ImGuiTreeNodeFlags_Selected as i32;
        /// Draw frame with background (e.g. for CollapsingHeader)
        const FRAMED = sys::ImGuiTreeNodeFlags_Framed as i32;
        /// Hit testing to allow subsequent widgets to overlap this one
        const ALLOW_ITEM_OVERLAP = sys::ImGuiTreeNodeFlags_AllowOverlap as i32;
        /// Hit testing to allow subsequent widgets to overlap this one
        const ALLOW_OVERLAP = sys::ImGuiTreeNodeFlags_AllowOverlap as i32;
        /// Don't do a TreePush() when open (e.g. for CollapsingHeader) = no extra indent nor pushing on ID stack
        const NO_TREE_PUSH_ON_OPEN = sys::ImGuiTreeNodeFlags_NoTreePushOnOpen as i32;
        /// Don't automatically and temporarily open node when Logging is active (by default logging will automatically open tree nodes)
        const NO_AUTO_OPEN_ON_LOG = sys::ImGuiTreeNodeFlags_NoAutoOpenOnLog as i32;
        /// Default node to be open
        const DEFAULT_OPEN = sys::ImGuiTreeNodeFlags_DefaultOpen as i32;
        /// Need double-click to open node
        const OPEN_ON_DOUBLE_CLICK = sys::ImGuiTreeNodeFlags_OpenOnDoubleClick as i32;
        /// Only open when clicking on the arrow part. If ImGuiTreeNodeFlags_OpenOnDoubleClick is also set, single-click arrow or double-click all box to open.
        const OPEN_ON_ARROW = sys::ImGuiTreeNodeFlags_OpenOnArrow as i32;
        /// No collapsing, no arrow (use as a convenience for leaf nodes)
        const LEAF = sys::ImGuiTreeNodeFlags_Leaf as i32;
        /// Display a bullet instead of arrow
        const BULLET = sys::ImGuiTreeNodeFlags_Bullet as i32;
        /// Use FramePadding (even for an unframed text node) to vertically align text baseline to regular widget height. Equivalent to calling AlignTextToFramePadding().
        const FRAME_PADDING = sys::ImGuiTreeNodeFlags_FramePadding as i32;
        /// Extend hit box to the right-most edge, even if not framed. This is not the default in order to allow adding other items on the same line. In the future we may refactor the hit system to be front-to-back, allowing natural overlaps and then this can become the default.
        const SPAN_AVAIL_WIDTH = sys::ImGuiTreeNodeFlags_SpanAvailWidth as i32;
        /// Extend hit box to the left-most and right-most edges (bypass the indented area).
        const SPAN_FULL_WIDTH = sys::ImGuiTreeNodeFlags_SpanFullWidth as i32;
        /// Narrow hit box and hover highlight to the label text width.
        const SPAN_LABEL_WIDTH = sys::ImGuiTreeNodeFlags_SpanLabelWidth as i32;
        /// Label will span all columns of its container table.
        const LABEL_SPAN_ALL_COLUMNS = sys::ImGuiTreeNodeFlags_LabelSpanAllColumns as i32;
        /// (WIP) Nav: left direction goes to parent. Only for the tree node, not the tree push.
        const NAV_LEFT_JUMPS_BACK_HERE = sys::ImGuiTreeNodeFlags_NavLeftJumpsToParent as i32;
        /// Combination of Leaf and NoTreePushOnOpen
        const COLLAPSING_HEADER =
            Self::FRAMED.bits() | Self::NO_TREE_PUSH_ON_OPEN.bits() | Self::NO_AUTO_OPEN_ON_LOG.bits();
        /// No tree hierarchy guide lines are drawn.
        const DRAW_LINES_NONE = sys::ImGuiTreeNodeFlags_DrawLinesNone as i32;
        /// Draw full tree hierarchy guide lines.
        const DRAW_LINES_FULL = sys::ImGuiTreeNodeFlags_DrawLinesFull as i32;
        /// Draw tree hierarchy guide lines only to nodes.
        const DRAW_LINES_TO_NODES = sys::ImGuiTreeNodeFlags_DrawLinesToNodes as i32;
    }
}

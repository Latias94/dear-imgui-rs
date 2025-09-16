use crate::sys;

pub mod button;
pub mod color;
pub mod combo;
pub mod drag;
pub mod image;
pub mod input;
pub mod list_box;
pub mod menu;
pub mod misc;
pub mod plot;
pub mod popup;
pub mod progress;
pub mod selectable;
pub mod slider;
pub mod tab;
pub mod table;
pub mod text;
pub mod tooltip;
pub mod tree;

// Re-export important types
pub use popup::PopupFlags;
pub use table::{TableBgTarget, TableColumnSetup};

// Widget implementations
pub use self::button::*;
pub use self::color::*;
pub use self::combo::*;
pub use self::drag::*;
pub use self::image::*;
pub use self::input::*;
pub use self::list_box::*;
pub use self::menu::*;
pub use self::misc::*;
pub use self::plot::*;
pub use self::popup::*;
pub use self::progress::*;
pub use self::selectable::*;
pub use self::slider::*;
pub use self::tab::*;
pub use self::table::*;
pub use self::tooltip::*;
pub use self::tree::*;

// ButtonFlags is defined in misc.rs and re-exported

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
        /// (WIP) Nav: left direction goes to parent. Only for the tree node, not the tree push.
        const NAV_LEFT_JUMPS_BACK_HERE = sys::ImGuiTreeNodeFlags_NavLeftJumpsToParent;
        /// Combination of Leaf and NoTreePushOnOpen
        const COLLAPSING_HEADER = Self::FRAMED.bits() | Self::NO_TREE_PUSH_ON_OPEN.bits();
    }
}

bitflags::bitflags! {
    /// Flags for combo box widgets
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct ComboBoxFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Align the popup toward the left by default
        const POPUP_ALIGN_LEFT = sys::ImGuiComboFlags_PopupAlignLeft;
        /// Max ~4 items visible. Tip: If you want your combo popup to be a specific size you can use SetNextWindowSizeConstraints() prior to calling BeginCombo()
        const HEIGHT_SMALL = sys::ImGuiComboFlags_HeightSmall;
        /// Max ~8 items visible (default)
        const HEIGHT_REGULAR = sys::ImGuiComboFlags_HeightRegular;
        /// Max ~20 items visible
        const HEIGHT_LARGE = sys::ImGuiComboFlags_HeightLarge;
        /// As many fitting items as possible
        const HEIGHT_LARGEST = sys::ImGuiComboFlags_HeightLargest;
        /// Display on the preview box without the square arrow button
        const NO_ARROW_BUTTON = sys::ImGuiComboFlags_NoArrowButton;
        /// Display only a square arrow button
        const NO_PREVIEW = sys::ImGuiComboFlags_NoPreview;
        /// Width dynamically calculated from preview contents
        const WIDTH_FIT_PREVIEW = sys::ImGuiComboFlags_WidthFitPreview;
    }
}

bitflags::bitflags! {
    /// Flags for table widgets
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct TableFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Enable resizing columns
        const RESIZABLE = sys::ImGuiTableFlags_Resizable;
        /// Enable reordering columns in header row (need calling TableSetupColumn() + TableHeadersRow() to display headers)
        const REORDERABLE = sys::ImGuiTableFlags_Reorderable;
        /// Enable hiding/disabling columns in context menu
        const HIDEABLE = sys::ImGuiTableFlags_Hideable;
        /// Enable sorting. Call TableGetSortSpecs() to obtain sort specs. Also see ImGuiTableFlags_SortMulti and ImGuiTableFlags_SortTristate.
        const SORTABLE = sys::ImGuiTableFlags_Sortable;
        /// Disable persisting columns order, width and sort settings in the .ini file
        const NO_SAVED_SETTINGS = sys::ImGuiTableFlags_NoSavedSettings;
        /// Right-click on columns body/contents will display table context menu. By default it is available in TableHeadersRow().
        const CONTEXT_MENU_IN_BODY = sys::ImGuiTableFlags_ContextMenuInBody;
        /// Set each RowBg color with ImGuiCol_TableRowBg or ImGuiCol_TableRowBgAlt (equivalent of calling TableSetBgColor with ImGuiTableBgFlags_RowBg0 on each row manually)
        const ROW_BG = sys::ImGuiTableFlags_RowBg;
        /// Draw horizontal borders between rows
        const BORDERS_INNER_H = sys::ImGuiTableFlags_BordersInnerH;
        /// Draw horizontal borders at the top and bottom
        const BORDERS_OUTER_H = sys::ImGuiTableFlags_BordersOuterH;
        /// Draw vertical borders between columns
        const BORDERS_INNER_V = sys::ImGuiTableFlags_BordersInnerV;
        /// Draw vertical borders on the left and right sides
        const BORDERS_OUTER_V = sys::ImGuiTableFlags_BordersOuterV;
        /// Draw horizontal borders
        const BORDERS_H = Self::BORDERS_INNER_H.bits() | Self::BORDERS_OUTER_H.bits();
        /// Draw vertical borders
        const BORDERS_V = Self::BORDERS_INNER_V.bits() | Self::BORDERS_OUTER_V.bits();
        /// Draw inner borders
        const BORDERS_INNER = Self::BORDERS_INNER_V.bits() | Self::BORDERS_INNER_H.bits();
        /// Draw outer borders
        const BORDERS_OUTER = Self::BORDERS_OUTER_V.bits() | Self::BORDERS_OUTER_H.bits();
        /// Draw all borders
        const BORDERS = Self::BORDERS_INNER.bits() | Self::BORDERS_OUTER.bits();
        /// [ALPHA] Disable vertical borders in columns Body (borders will always appears in Headers). -> May move to style
        const NO_BORDERS_IN_BODY = sys::ImGuiTableFlags_NoBordersInBody;
        /// [ALPHA] Disable vertical borders in columns Body until hovered for resize (borders will always appears in Headers). -> May move to style
        const NO_BORDERS_IN_BODY_UNTIL_RESIZE = sys::ImGuiTableFlags_NoBordersInBodyUntilResize;
        /// Columns default to _WidthFixed or _WidthAuto (if resizable or not resizable), matching contents width
        const SIZING_FIXED_FIT = sys::ImGuiTableFlags_SizingFixedFit;
        /// Columns default to _WidthFixed or _WidthAuto (if resizable or not resizable), matching the maximum contents width of all columns. Implicitly enable ImGuiTableFlags_NoKeepColumnsVisible.
        const SIZING_FIXED_SAME = sys::ImGuiTableFlags_SizingFixedSame;
        /// Columns default to _WidthStretch with default weights proportional to each columns contents widths.
        const SIZING_STRETCH_PROP = sys::ImGuiTableFlags_SizingStretchProp;
        /// Columns default to _WidthStretch with default weights all equal, unless overridden by TableSetupColumn().
        const SIZING_STRETCH_SAME = sys::ImGuiTableFlags_SizingStretchSame;
        /// Make outer width auto-fit to columns, overriding outer_size.x value. Only available when ScrollX/ScrollY are disabled and Stretch columns are not used.
        const NO_HOST_EXTEND_X = sys::ImGuiTableFlags_NoHostExtendX;
        /// Make outer height stop exactly at outer_size.y (prevent auto-extending table past the limit). Only available when ScrollX/ScrollY are disabled. Data below the limit will be clipped and not visible.
        const NO_HOST_EXTEND_Y = sys::ImGuiTableFlags_NoHostExtendY;
        /// Disable keeping column always minimally visible when ScrollX is on and table gets too small. Not recommended if columns are resizable.
        const NO_KEEP_COLUMNS_VISIBLE = sys::ImGuiTableFlags_NoKeepColumnsVisible;
        /// Disable distributing remainder width to stretched columns (width allocation on a 100-wide table with 3 columns: Without this flag: 33,33,34. With this flag: 33,33,33). With larger number of columns, resizing will appear to be less smooth.
        const PRECISE_WIDTHS = sys::ImGuiTableFlags_PreciseWidths;
        /// Disable clipping rectangle for every individual columns (reduce draw command count, items will be able to overflow into other columns). Generally incompatible with TableSetupScrollFreeze().
        const NO_CLIP = sys::ImGuiTableFlags_NoClip;
        /// Default if BordersOuterV is on. Enable outer-most padding. Generally desirable if you have headers.
        const PAD_OUTER_X = sys::ImGuiTableFlags_PadOuterX;
        /// Default if BordersOuterV is off. Disable outer-most padding.
        const NO_PAD_OUTER_X = sys::ImGuiTableFlags_NoPadOuterX;
        /// Disable inner padding between columns (double inner padding if BordersOuterV is on, single inner padding if BordersOuterV is off).
        const NO_PAD_INNER_X = sys::ImGuiTableFlags_NoPadInnerX;
        /// Enable horizontal scrolling. Require 'outer_size' parameter of BeginTable() to specify the container size. Changes default sizing policy. Because this creates a child window, ScrollY is currently generally recommended when using ScrollX.
        const SCROLL_X = sys::ImGuiTableFlags_ScrollX;
        /// Enable vertical scrolling. Require 'outer_size' parameter of BeginTable() to specify the container size.
        const SCROLL_Y = sys::ImGuiTableFlags_ScrollY;
        /// Hold shift when clicking headers to sort on multiple column. TableGetSortSpecs() may return specs where (SpecsCount > 1).
        const SORT_MULTI = sys::ImGuiTableFlags_SortMulti;
        /// Allow no sorting, disable default sorting. TableGetSortSpecs() may return specs where (SpecsCount == 0).
        const SORT_TRISTATE = sys::ImGuiTableFlags_SortTristate;
        /// Highlight column headers when hovered (may not be visible if table header is declaring a background color)
        const HIGHLIGHT_HOVERED_COLUMN = sys::ImGuiTableFlags_HighlightHoveredColumn;
    }
}

bitflags::bitflags! {
    /// Flags for table columns
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct TableColumnFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Overriding width becomes fixed width
        const WIDTH_FIXED = sys::ImGuiTableColumnFlags_WidthFixed;
        /// Overriding width becomes weight
        const WIDTH_STRETCH = sys::ImGuiTableColumnFlags_WidthStretch;
        /// Disable manual resizing
        const NO_RESIZE = sys::ImGuiTableColumnFlags_NoResize;
        /// Disable manual reordering this column
        const NO_REORDER = sys::ImGuiTableColumnFlags_NoReorder;
        /// Disable ability to hide/disable this column
        const NO_HIDE = sys::ImGuiTableColumnFlags_NoHide;
        /// Disable clipping for this column
        const NO_CLIP = sys::ImGuiTableColumnFlags_NoClip;
        /// Disable ability to sort on this field
        const NO_SORT = sys::ImGuiTableColumnFlags_NoSort;
        /// Disable ability to sort in the ascending direction
        const NO_SORT_ASCENDING = sys::ImGuiTableColumnFlags_NoSortAscending;
        /// Disable ability to sort in the descending direction
        const NO_SORT_DESCENDING = sys::ImGuiTableColumnFlags_NoSortDescending;
        /// TableHeadersRow() will not submit label for this column
        const NO_HEADER_LABEL = sys::ImGuiTableColumnFlags_NoHeaderLabel;
        /// Disable header text width contribution to automatic column width
        const NO_HEADER_WIDTH = sys::ImGuiTableColumnFlags_NoHeaderWidth;
        /// Make the initial sort direction Ascending when first sorting on this column
        const PREFER_SORT_ASCENDING = sys::ImGuiTableColumnFlags_PreferSortAscending;
        /// Make the initial sort direction Descending when first sorting on this column
        const PREFER_SORT_DESCENDING = sys::ImGuiTableColumnFlags_PreferSortDescending;
        /// Use current Indent value when entering cell
        const INDENT_ENABLE = sys::ImGuiTableColumnFlags_IndentEnable;
        /// Disable indenting for this column
        const INDENT_DISABLE = sys::ImGuiTableColumnFlags_IndentDisable;
        /// Status: is enabled == not hidden
        const IS_ENABLED = sys::ImGuiTableColumnFlags_IsEnabled;
        /// Status: is visible == is enabled AND not clipped by scrolling
        const IS_VISIBLE = sys::ImGuiTableColumnFlags_IsVisible;
        /// Status: is currently part of the sort specs
        const IS_SORTED = sys::ImGuiTableColumnFlags_IsSorted;
        /// Status: is hovered by mouse
        const IS_HOVERED = sys::ImGuiTableColumnFlags_IsHovered;
    }
}

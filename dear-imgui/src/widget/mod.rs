//! Standard widgets
//!
//! Collection of common Dear ImGui widgets exposed with an idiomatic Rust
//! API. Most widgets follow a small builder pattern for configuration, and
//! also provide convenience methods on [`Ui`].
//!
//! Examples:
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! // Buttons
//! if ui.button("Click me") { /* ... */ }
//!
//! // Sliders
//! let mut value = 0.5f32;
//! ui.slider_f32("Value", &mut value, 0.0, 1.0);
//!
//! // Inputs
//! let mut text = String::new();
//! ui.input_text("Name", &mut text).build();
//! ```
//!
//! Submodules group related widgets: `button`, `color`, `combo`, `drag`,
//! `image`, `input`, `list_box`, `menu`, `misc`, `plot`, `popup`, `progress`,
//! `selectable`, `slider`, `tab`, `table`, `text`, `tooltip`, `tree`.
//!
use crate::sys;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

pub mod button;
pub mod color;
pub mod combo;
pub mod drag;
pub mod image;
pub mod input;
pub mod list_box;
pub mod menu;
pub mod misc;
pub mod multi_select;
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
pub use table::{TableBgTarget, TableBuilder, TableColumnSetup};

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
pub use self::multi_select::*;
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
        const SELECTED = sys::ImGuiTreeNodeFlags_Selected as i32;
        /// Draw frame with background (e.g. for CollapsingHeader)
        const FRAMED = sys::ImGuiTreeNodeFlags_Framed as i32;
        /// Hit testing to allow subsequent widgets to overlap this one
        const ALLOW_ITEM_OVERLAP = sys::ImGuiTreeNodeFlags_AllowOverlap as i32;
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
        /// (WIP) Nav: left direction goes to parent. Only for the tree node, not the tree push.
        const NAV_LEFT_JUMPS_BACK_HERE = sys::ImGuiTreeNodeFlags_NavLeftJumpsToParent as i32;
        /// Combination of Leaf and NoTreePushOnOpen
        const COLLAPSING_HEADER = Self::FRAMED.bits() | Self::NO_TREE_PUSH_ON_OPEN.bits();
    }
}

bitflags::bitflags! {
    /// Independent flags for combo box widgets.
    ///
    /// Mutually exclusive preview and height choices are represented by
    /// [`ComboBoxPreviewMode`] and [`ComboBoxHeight`].
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct ComboBoxFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Align the popup toward the left by default
        const POPUP_ALIGN_LEFT = sys::ImGuiComboFlags_PopupAlignLeft as i32;
    }
}

/// Height policy for combo box popups.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ComboBoxHeight {
    /// Max roughly 4 items visible.
    Small,
    /// Max roughly 8 items visible.
    Regular,
    /// Max roughly 20 items visible.
    Large,
    /// As many fitting items as possible.
    Largest,
}

impl ComboBoxHeight {
    #[inline]
    const fn raw(self) -> i32 {
        match self {
            Self::Small => sys::ImGuiComboFlags_HeightSmall as i32,
            Self::Regular => sys::ImGuiComboFlags_HeightRegular as i32,
            Self::Large => sys::ImGuiComboFlags_HeightLarge as i32,
            Self::Largest => sys::ImGuiComboFlags_HeightLargest as i32,
        }
    }
}

/// Preview/arrow layout for a combo box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ComboBoxPreviewMode {
    /// Standard preview box with arrow button.
    #[default]
    Preview,
    /// Standard preview box without the square arrow button.
    PreviewNoArrowButton,
    /// Width dynamically calculated from preview contents.
    PreviewFit,
    /// Fit preview width without the square arrow button.
    PreviewFitNoArrowButton,
    /// Display only a square arrow button.
    NoPreview,
}

impl ComboBoxPreviewMode {
    #[inline]
    const fn raw(self) -> i32 {
        match self {
            Self::Preview => 0,
            Self::PreviewNoArrowButton => sys::ImGuiComboFlags_NoArrowButton as i32,
            Self::PreviewFit => sys::ImGuiComboFlags_WidthFitPreview as i32,
            Self::PreviewFitNoArrowButton => {
                sys::ImGuiComboFlags_WidthFitPreview as i32
                    | sys::ImGuiComboFlags_NoArrowButton as i32
            }
            Self::NoPreview => sys::ImGuiComboFlags_NoPreview as i32,
        }
    }
}

/// Complete combo box options assembled from independent flags and exclusive
/// mode selections.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ComboBoxOptions {
    pub flags: ComboBoxFlags,
    pub height: Option<ComboBoxHeight>,
    pub preview_mode: ComboBoxPreviewMode,
}

impl Default for ComboBoxOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl ComboBoxOptions {
    pub const fn new() -> Self {
        Self {
            flags: ComboBoxFlags::NONE,
            height: None,
            preview_mode: ComboBoxPreviewMode::Preview,
        }
    }

    pub fn flags(mut self, flags: ComboBoxFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn height(mut self, height: ComboBoxHeight) -> Self {
        self.height = Some(height);
        self
    }

    pub fn preview_mode(mut self, mode: ComboBoxPreviewMode) -> Self {
        self.preview_mode = mode;
        self
    }

    pub fn bits(self) -> i32 {
        self.raw()
    }

    #[inline]
    pub(crate) fn raw(self) -> i32 {
        self.flags.bits() | self.height.map_or(0, ComboBoxHeight::raw) | self.preview_mode.raw()
    }

    #[inline]
    pub(crate) fn validate(self, caller: &str) {
        let unsupported_flags = self.flags.bits() & !ComboBoxFlags::all().bits();
        assert!(
            unsupported_flags == 0,
            "{caller} received non-independent ImGuiComboFlags bits: 0x{unsupported_flags:X}"
        );
        let bits = self.raw();
        let supported =
            ComboBoxFlags::all().bits() | sys::ImGuiComboFlags_HeightMask_ | combo_preview_mask();
        let unsupported = bits & !supported;
        assert!(
            unsupported == 0,
            "{caller} received unsupported ImGuiComboFlags bits: 0x{unsupported:X}"
        );
        assert!(
            bits & (sys::ImGuiComboFlags_NoArrowButton | sys::ImGuiComboFlags_NoPreview)
                != (sys::ImGuiComboFlags_NoArrowButton | sys::ImGuiComboFlags_NoPreview),
            "{caller} cannot combine NO_ARROW_BUTTON with NO_PREVIEW"
        );
        assert!(
            bits & sys::ImGuiComboFlags_WidthFitPreview == 0
                || bits & sys::ImGuiComboFlags_NoPreview == 0,
            "{caller} cannot combine WIDTH_FIT_PREVIEW with NO_PREVIEW"
        );
        assert!(
            (bits & sys::ImGuiComboFlags_HeightMask_).count_ones() <= 1,
            "{caller} accepts at most one combo height policy"
        );
    }
}

#[inline]
const fn combo_preview_mask() -> i32 {
    sys::ImGuiComboFlags_NoArrowButton
        | sys::ImGuiComboFlags_NoPreview
        | sys::ImGuiComboFlags_WidthFitPreview
}

impl From<ComboBoxFlags> for ComboBoxOptions {
    fn from(flags: ComboBoxFlags) -> Self {
        Self::new().flags(flags)
    }
}

bitflags::bitflags! {
    /// Independent flags for table widgets.
    ///
    /// The table sizing policy is a single-choice setting represented by
    /// [`TableSizingPolicy`].
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct TableFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Enable resizing columns
        const RESIZABLE = sys::ImGuiTableFlags_Resizable as i32;
        /// Enable reordering columns in header row (need calling TableSetupColumn() + TableHeadersRow() to display headers)
        const REORDERABLE = sys::ImGuiTableFlags_Reorderable as i32;
        /// Enable hiding/disabling columns in context menu
        const HIDEABLE = sys::ImGuiTableFlags_Hideable as i32;
        /// Enable sorting. Call TableGetSortSpecs() to obtain sort specs. Also see ImGuiTableFlags_SortMulti and ImGuiTableFlags_SortTristate.
        const SORTABLE = sys::ImGuiTableFlags_Sortable as i32;
        /// Disable persisting columns order, width and sort settings in the .ini file
        const NO_SAVED_SETTINGS = sys::ImGuiTableFlags_NoSavedSettings as i32;
        /// Right-click on columns body/contents will display table context menu. By default it is available in TableHeadersRow().
        const CONTEXT_MENU_IN_BODY = sys::ImGuiTableFlags_ContextMenuInBody as i32;
        /// Set each RowBg color with ImGuiCol_TableRowBg or ImGuiCol_TableRowBgAlt (equivalent of calling TableSetBgColor with ImGuiTableBgFlags_RowBg0 on each row manually)
        const ROW_BG = sys::ImGuiTableFlags_RowBg as i32;
        /// Draw horizontal borders between rows
        const BORDERS_INNER_H = sys::ImGuiTableFlags_BordersInnerH as i32;
        /// Draw horizontal borders at the top and bottom
        const BORDERS_OUTER_H = sys::ImGuiTableFlags_BordersOuterH as i32;
        /// Draw vertical borders between columns
        const BORDERS_INNER_V = sys::ImGuiTableFlags_BordersInnerV as i32;
        /// Draw vertical borders on the left and right sides
        const BORDERS_OUTER_V = sys::ImGuiTableFlags_BordersOuterV as i32;
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
        const NO_BORDERS_IN_BODY = sys::ImGuiTableFlags_NoBordersInBody as i32;
        /// [ALPHA] Disable vertical borders in columns Body until hovered for resize (borders will always appears in Headers). -> May move to style
        const NO_BORDERS_IN_BODY_UNTIL_RESIZE = sys::ImGuiTableFlags_NoBordersInBodyUntilResize as i32;
        /// Make outer width auto-fit to columns, overriding outer_size.x value. Only available when ScrollX/ScrollY are disabled and Stretch columns are not used.
        const NO_HOST_EXTEND_X = sys::ImGuiTableFlags_NoHostExtendX as i32;
        /// Make outer height stop exactly at outer_size.y (prevent auto-extending table past the limit). Only available when ScrollX/ScrollY are disabled. Data below the limit will be clipped and not visible.
        const NO_HOST_EXTEND_Y = sys::ImGuiTableFlags_NoHostExtendY as i32;
        /// Disable keeping column always minimally visible when ScrollX is on and table gets too small. Not recommended if columns are resizable.
        const NO_KEEP_COLUMNS_VISIBLE = sys::ImGuiTableFlags_NoKeepColumnsVisible as i32;
        /// Disable distributing remainder width to stretched columns (width allocation on a 100-wide table with 3 columns: Without this flag: 33,33,34. With this flag: 33,33,33). With larger number of columns, resizing will appear to be less smooth.
        const PRECISE_WIDTHS = sys::ImGuiTableFlags_PreciseWidths as i32;
        /// Disable clipping rectangle for every individual columns (reduce draw command count, items will be able to overflow into other columns). Generally incompatible with TableSetupScrollFreeze().
        const NO_CLIP = sys::ImGuiTableFlags_NoClip as i32;
        /// Default if BordersOuterV is on. Enable outer-most padding. Generally desirable if you have headers.
        const PAD_OUTER_X = sys::ImGuiTableFlags_PadOuterX as i32;
        /// Default if BordersOuterV is off. Disable outer-most padding.
        const NO_PAD_OUTER_X = sys::ImGuiTableFlags_NoPadOuterX as i32;
        /// Disable inner padding between columns (double inner padding if BordersOuterV is on, single inner padding if BordersOuterV is off).
        const NO_PAD_INNER_X = sys::ImGuiTableFlags_NoPadInnerX as i32;
        /// Enable horizontal scrolling. Require 'outer_size' parameter of BeginTable() to specify the container size. Changes default sizing policy. Because this creates a child window, ScrollY is currently generally recommended when using ScrollX.
        const SCROLL_X = sys::ImGuiTableFlags_ScrollX as i32;
        /// Enable vertical scrolling. Require 'outer_size' parameter of BeginTable() to specify the container size.
        const SCROLL_Y = sys::ImGuiTableFlags_ScrollY as i32;
        /// Hold shift when clicking headers to sort on multiple column. TableGetSortSpecs() may return specs where (SpecsCount > 1).
        const SORT_MULTI = sys::ImGuiTableFlags_SortMulti as i32;
        /// Allow no sorting, disable default sorting. TableGetSortSpecs() may return specs where (SpecsCount == 0).
        const SORT_TRISTATE = sys::ImGuiTableFlags_SortTristate as i32;
        /// Highlight column headers when hovered (may not be visible if table header is declaring a background color)
        const HIGHLIGHT_HOVERED_COLUMN = sys::ImGuiTableFlags_HighlightHoveredColumn as i32;
    }
}

/// Single-choice table sizing policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TableSizingPolicy {
    /// Columns default to fixed/auto widths matching contents width.
    FixedFit,
    /// Fixed/auto widths matching the maximum contents width of all columns.
    FixedSame,
    /// Stretch columns with weights proportional to contents widths.
    StretchProp,
    /// Stretch columns with equal weights unless overridden per column.
    StretchSame,
}

impl TableSizingPolicy {
    #[inline]
    const fn raw(self) -> i32 {
        match self {
            Self::FixedFit => sys::ImGuiTableFlags_SizingFixedFit as i32,
            Self::FixedSame => sys::ImGuiTableFlags_SizingFixedSame as i32,
            Self::StretchProp => sys::ImGuiTableFlags_SizingStretchProp as i32,
            Self::StretchSame => sys::ImGuiTableFlags_SizingStretchSame as i32,
        }
    }
}

/// Complete table options assembled from independent flags and an optional
/// single sizing policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TableOptions {
    pub flags: TableFlags,
    pub sizing_policy: Option<TableSizingPolicy>,
}

impl Default for TableOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl TableOptions {
    pub const fn new() -> Self {
        Self {
            flags: TableFlags::NONE,
            sizing_policy: None,
        }
    }

    pub fn flags(mut self, flags: TableFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn sizing_policy(mut self, policy: TableSizingPolicy) -> Self {
        self.sizing_policy = Some(policy);
        self
    }

    pub fn bits(self) -> i32 {
        self.raw()
    }

    #[inline]
    pub(crate) fn raw(self) -> i32 {
        self.flags.bits() | self.sizing_policy.map_or(0, TableSizingPolicy::raw)
    }

    #[inline]
    pub(crate) fn validate(self, caller: &str) {
        let unsupported_flags = self.flags.bits() & !TableFlags::all().bits();
        assert!(
            unsupported_flags == 0,
            "{caller} received non-independent ImGuiTableFlags bits: 0x{unsupported_flags:X}"
        );
        let bits = self.raw();
        let supported = TableFlags::all().bits() | sys::ImGuiTableFlags_SizingMask_;
        let unsupported = bits & !supported;
        assert!(
            unsupported == 0,
            "{caller} received unsupported ImGuiTableFlags bits: 0x{unsupported:X}"
        );
        let sizing_policy = bits & sys::ImGuiTableFlags_SizingMask_;
        assert!(
            is_valid_table_sizing_policy(sizing_policy),
            "{caller} received invalid table sizing policy bits: 0x{sizing_policy:X}"
        );
    }
}

#[inline]
const fn is_valid_table_sizing_policy(bits: i32) -> bool {
    bits == 0
        || bits == TableSizingPolicy::FixedFit.raw()
        || bits == TableSizingPolicy::FixedSame.raw()
        || bits == TableSizingPolicy::StretchProp.raw()
        || bits == TableSizingPolicy::StretchSame.raw()
}

impl From<TableFlags> for TableOptions {
    fn from(flags: TableFlags) -> Self {
        Self::new().flags(flags)
    }
}

#[cfg(feature = "serde")]
impl Serialize for TableFlags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i32(self.bits())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for TableFlags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bits = i32::deserialize(deserializer)?;
        Ok(TableFlags::from_bits_truncate(bits))
    }
}

bitflags::bitflags! {
    /// Independent flags accepted by `TableSetupColumn()`.
    ///
    /// The fixed/stretch width mode and indent mode are single-choice settings
    /// represented by [`TableColumnWidth`] and [`TableColumnIndent`].
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct TableColumnFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Hide column and omit it from the context menu.
        const DISABLED = sys::ImGuiTableColumnFlags_Disabled as i32;
        /// Default to a hidden/disabled column.
        const DEFAULT_HIDE = sys::ImGuiTableColumnFlags_DefaultHide as i32;
        /// Default to a sorting column.
        const DEFAULT_SORT = sys::ImGuiTableColumnFlags_DefaultSort as i32;
        /// Disable manual resizing
        const NO_RESIZE = sys::ImGuiTableColumnFlags_NoResize as i32;
        /// Disable manual reordering this column
        const NO_REORDER = sys::ImGuiTableColumnFlags_NoReorder as i32;
        /// Disable ability to hide/disable this column
        const NO_HIDE = sys::ImGuiTableColumnFlags_NoHide as i32;
        /// Disable clipping for this column
        const NO_CLIP = sys::ImGuiTableColumnFlags_NoClip as i32;
        /// Disable ability to sort on this field
        const NO_SORT = sys::ImGuiTableColumnFlags_NoSort as i32;
        /// Disable ability to sort in the ascending direction
        const NO_SORT_ASCENDING = sys::ImGuiTableColumnFlags_NoSortAscending as i32;
        /// Disable ability to sort in the descending direction
        const NO_SORT_DESCENDING = sys::ImGuiTableColumnFlags_NoSortDescending as i32;
        /// TableHeadersRow() will not submit label for this column
        const NO_HEADER_LABEL = sys::ImGuiTableColumnFlags_NoHeaderLabel as i32;
        /// Disable header text width contribution to automatic column width
        const NO_HEADER_WIDTH = sys::ImGuiTableColumnFlags_NoHeaderWidth as i32;
        /// Make the initial sort direction Ascending when first sorting on this column
        const PREFER_SORT_ASCENDING = sys::ImGuiTableColumnFlags_PreferSortAscending as i32;
        /// Make the initial sort direction Descending when first sorting on this column
        const PREFER_SORT_DESCENDING = sys::ImGuiTableColumnFlags_PreferSortDescending as i32;
        /// Display an angled header for this column (when angled headers feature is enabled)
        const ANGLED_HEADER = sys::ImGuiTableColumnFlags_AngledHeader as i32;
    }
}

/// Single-choice width mode for a table column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TableColumnWidth {
    /// Initial value is interpreted as a fixed width in pixels.
    Fixed(f32),
    /// Initial value is interpreted as a stretch weight.
    Stretch(f32),
}

impl TableColumnWidth {
    pub const fn fixed(width: f32) -> Self {
        Self::Fixed(width)
    }

    pub const fn stretch(weight: f32) -> Self {
        Self::Stretch(weight)
    }

    #[inline]
    pub(crate) const fn raw_flags(self) -> i32 {
        match self {
            Self::Fixed(_) => sys::ImGuiTableColumnFlags_WidthFixed as i32,
            Self::Stretch(_) => sys::ImGuiTableColumnFlags_WidthStretch as i32,
        }
    }

    #[inline]
    pub(crate) const fn value(self) -> f32 {
        match self {
            Self::Fixed(value) | Self::Stretch(value) => value,
        }
    }
}

/// Single-choice indentation policy for a table column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TableColumnIndent {
    /// Use the current indent value when entering the column.
    Enable,
    /// Disable indentation for the column.
    Disable,
}

impl TableColumnIndent {
    #[inline]
    pub const fn bits(self) -> i32 {
        self.raw_flags()
    }

    #[inline]
    pub(crate) const fn raw_flags(self) -> i32 {
        match self {
            Self::Enable => sys::ImGuiTableColumnFlags_IndentEnable as i32,
            Self::Disable => sys::ImGuiTableColumnFlags_IndentDisable as i32,
        }
    }
}

bitflags::bitflags! {
    /// Flags returned by `TableGetColumnFlags()`.
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct TableColumnStateFlags: i32 {
        /// No flags
        const NONE = 0;
        /// Overriding/master disable flag: hide column and omit it from the context menu.
        const DISABLED = sys::ImGuiTableColumnFlags_Disabled as i32;
        /// Default to a hidden/disabled column.
        const DEFAULT_HIDE = sys::ImGuiTableColumnFlags_DefaultHide as i32;
        /// Default to a sorting column.
        const DEFAULT_SORT = sys::ImGuiTableColumnFlags_DefaultSort as i32;
        /// Overriding width becomes fixed width
        const WIDTH_FIXED = sys::ImGuiTableColumnFlags_WidthFixed as i32;
        /// Overriding width becomes weight
        const WIDTH_STRETCH = sys::ImGuiTableColumnFlags_WidthStretch as i32;
        /// Disable manual resizing
        const NO_RESIZE = sys::ImGuiTableColumnFlags_NoResize as i32;
        /// Disable manual reordering this column
        const NO_REORDER = sys::ImGuiTableColumnFlags_NoReorder as i32;
        /// Disable ability to hide/disable this column
        const NO_HIDE = sys::ImGuiTableColumnFlags_NoHide as i32;
        /// Disable clipping for this column
        const NO_CLIP = sys::ImGuiTableColumnFlags_NoClip as i32;
        /// Disable ability to sort on this field
        const NO_SORT = sys::ImGuiTableColumnFlags_NoSort as i32;
        /// Disable ability to sort in the ascending direction
        const NO_SORT_ASCENDING = sys::ImGuiTableColumnFlags_NoSortAscending as i32;
        /// Disable ability to sort in the descending direction
        const NO_SORT_DESCENDING = sys::ImGuiTableColumnFlags_NoSortDescending as i32;
        /// TableHeadersRow() will not submit label for this column
        const NO_HEADER_LABEL = sys::ImGuiTableColumnFlags_NoHeaderLabel as i32;
        /// Disable header text width contribution to automatic column width
        const NO_HEADER_WIDTH = sys::ImGuiTableColumnFlags_NoHeaderWidth as i32;
        /// Make the initial sort direction Ascending when first sorting on this column
        const PREFER_SORT_ASCENDING = sys::ImGuiTableColumnFlags_PreferSortAscending as i32;
        /// Make the initial sort direction Descending when first sorting on this column
        const PREFER_SORT_DESCENDING = sys::ImGuiTableColumnFlags_PreferSortDescending as i32;
        /// Use current Indent value when entering cell
        const INDENT_ENABLE = sys::ImGuiTableColumnFlags_IndentEnable as i32;
        /// Disable indenting for this column
        const INDENT_DISABLE = sys::ImGuiTableColumnFlags_IndentDisable as i32;
        /// Display an angled header for this column (when angled headers feature is enabled)
        const ANGLED_HEADER = sys::ImGuiTableColumnFlags_AngledHeader as i32;
        /// Status: is enabled == not hidden
        const IS_ENABLED = sys::ImGuiTableColumnFlags_IsEnabled as i32;
        /// Status: is visible == is enabled AND not clipped by scrolling
        const IS_VISIBLE = sys::ImGuiTableColumnFlags_IsVisible as i32;
        /// Status: is currently part of the sort specs
        const IS_SORTED = sys::ImGuiTableColumnFlags_IsSorted as i32;
        /// Status: is hovered by mouse
        const IS_HOVERED = sys::ImGuiTableColumnFlags_IsHovered as i32;
    }
}

impl From<TableColumnFlags> for TableColumnStateFlags {
    fn from(flags: TableColumnFlags) -> Self {
        Self::from_bits_retain(flags.bits())
    }
}

impl TableColumnFlags {
    #[inline]
    pub(crate) fn validate_for_setup(
        self,
        caller: &str,
        width: Option<TableColumnWidth>,
        indent: Option<TableColumnIndent>,
    ) {
        let unsupported_flags = self.bits() & !TableColumnFlags::all().bits();
        assert!(
            unsupported_flags == 0,
            "{caller} received non-independent ImGuiTableColumnFlags bits: 0x{unsupported_flags:X}"
        );
        let bits = self.bits()
            | width.map_or(0, TableColumnWidth::raw_flags)
            | indent.map_or(0, TableColumnIndent::raw_flags);
        let supported = TableColumnFlags::all().bits()
            | sys::ImGuiTableColumnFlags_WidthMask_
            | sys::ImGuiTableColumnFlags_IndentMask_;
        let unsupported = bits & !supported;
        assert!(
            unsupported == 0,
            "{caller} received unsupported ImGuiTableColumnFlags bits: 0x{unsupported:X}"
        );
        assert!(
            (bits & sys::ImGuiTableColumnFlags_WidthMask_).count_ones() <= 1,
            "{caller} accepts at most one table column width policy"
        );
        assert!(
            (bits & sys::ImGuiTableColumnFlags_IndentMask_).count_ones() <= 1,
            "{caller} accepts at most one table column indent policy"
        );
    }
}

#[cfg(feature = "serde")]
impl Serialize for TableColumnFlags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i32(self.bits())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for TableColumnFlags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bits = i32::deserialize(deserializer)?;
        Ok(TableColumnFlags::from_bits_truncate(bits))
    }
}

#[cfg(feature = "serde")]
impl Serialize for TableColumnStateFlags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i32(self.bits())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for TableColumnStateFlags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bits = i32::deserialize(deserializer)?;
        Ok(TableColumnStateFlags::from_bits_truncate(bits))
    }
}

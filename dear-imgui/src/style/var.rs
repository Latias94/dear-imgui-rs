/// A temporary change in user interface style
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum StyleVar {
    /// Global alpha applies to everything
    Alpha(f32),
    /// Additional alpha multiplier applied to disabled elements
    DisabledAlpha(f32),
    /// Padding within a window
    WindowPadding([f32; 2]),
    /// Rounding radius of window corners
    WindowRounding(f32),
    /// Thickness of border around windows
    WindowBorderSize(f32),
    /// Minimum window size
    WindowMinSize([f32; 2]),
    /// Alignment for title bar text
    WindowTitleAlign([f32; 2]),
    /// Rounding radius of child window corners
    ChildRounding(f32),
    /// Thickness of border around child windows
    ChildBorderSize(f32),
    /// Rounding radius of popup window corners
    PopupRounding(f32),
    /// Thickness of border around popup/tooltip windows
    PopupBorderSize(f32),
    /// Padding within a framed rectangle (used by most widgets)
    FramePadding([f32; 2]),
    /// Rounding radius of frame corners (used by most widgets)
    FrameRounding(f32),
    /// Rounding radius of image corners (used by Image() and ImageButton() widgets)
    ImageRounding(f32),
    /// Thickness of border around images
    ImageBorderSize(f32),
    /// Thickness of border around frames
    FrameBorderSize(f32),
    /// Horizontal and vertical spacing between widgets/lines
    ItemSpacing([f32; 2]),
    /// Horizontal and vertical spacing between within elements of a composed widget
    ItemInnerSpacing([f32; 2]),
    /// Horizontal indentation when e.g. entering a tree node
    IndentSpacing(f32),
    /// Padding within a table cell
    CellPadding([f32; 2]),
    /// Width of the vertical scrollbar, height of the horizontal scrollbar
    ScrollbarSize(f32),
    /// Rounding radius of scrollbar corners
    ScrollbarRounding(f32),
    /// Padding of scrollbar grab within its frame
    ScrollbarPadding(f32),
    /// Minimum width/height of a grab box for slider/scrollbar
    GrabMinSize(f32),
    /// Rounding radius of grabs corners
    GrabRounding(f32),
    /// Rounding radius of upper corners of tabs
    TabRounding(f32),
    /// Thickness of border around tabs
    TabBorderSize(f32),
    /// Minimum tab width before fitting policy shrink is applied
    TabMinWidthBase(f32),
    /// Minimum tab width after shrinking with the mixed fitting policy
    TabMinWidthShrink(f32),
    /// Thickness of the tab-bar separator
    TabBarBorderSize(f32),
    /// Thickness of the selected tab-bar overline
    TabBarOverlineSize(f32),
    /// Angle of angled table headers, in radians
    TableAngledHeadersAngle(f32),
    /// Alignment of angled table headers within the cell
    TableAngledHeadersTextAlign([f32; 2]),
    /// Thickness of tree hierarchy outlines
    TreeLinesSize(f32),
    /// Rounding radius of tree hierarchy outlines
    TreeLinesRounding(f32),
    /// Rounding radius of menu items and menus
    MenuItemRounding(f32),
    /// Rounding radius of selectable items
    SelectableRounding(f32),
    /// Rounding radius of drag and drop target highlights; negative values use frame rounding
    DragDropTargetRounding(f32),
    /// Alignment of button text when button is larger than text
    ButtonTextAlign([f32; 2]),
    /// Alignment of selectable text when selectable is larger than text
    SelectableTextAlign([f32; 2]),
    /// Thickness of border in `Separator()`
    SeparatorSize(f32),
    /// Thickness of border in `SeparatorText()`
    SeparatorTextBorderSize(f32),
    /// Alignment of text within the separator
    SeparatorTextAlign([f32; 2]),
    /// Padding around text in `SeparatorText()`
    SeparatorTextPadding([f32; 2]),
    /// Thickness of resizing border between docked windows
    DockingSeparatorSize(f32),
}

/// A two-component style variable whose X or Y component can be overridden.
///
/// Use this with [`Ui::push_style_var_x`](crate::Ui::push_style_var_x) or
/// [`Ui::push_style_var_y`](crate::Ui::push_style_var_y). Scalar style
/// variables are intentionally not representable by this type.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum StyleVarVec2 {
    /// Padding within a window.
    WindowPadding,
    /// Minimum window size.
    WindowMinSize,
    /// Alignment for title bar text.
    WindowTitleAlign,
    /// Padding within a framed rectangle.
    FramePadding,
    /// Horizontal and vertical spacing between widgets or lines.
    ItemSpacing,
    /// Spacing between elements of a composed widget.
    ItemInnerSpacing,
    /// Padding within a table cell.
    CellPadding,
    /// Alignment of angled table headers within the cell.
    TableAngledHeadersTextAlign,
    /// Alignment of button text when the button is larger than its text.
    ButtonTextAlign,
    /// Alignment of selectable text when the selectable is larger than its text.
    SelectableTextAlign,
    /// Alignment of text within a separator.
    SeparatorTextAlign,
    /// Padding around text in a separator.
    SeparatorTextPadding,
}

impl StyleVarVec2 {
    pub(crate) const fn raw(self) -> i32 {
        match self {
            Self::WindowPadding => crate::sys::ImGuiStyleVar_WindowPadding as i32,
            Self::WindowMinSize => crate::sys::ImGuiStyleVar_WindowMinSize as i32,
            Self::WindowTitleAlign => crate::sys::ImGuiStyleVar_WindowTitleAlign as i32,
            Self::FramePadding => crate::sys::ImGuiStyleVar_FramePadding as i32,
            Self::ItemSpacing => crate::sys::ImGuiStyleVar_ItemSpacing as i32,
            Self::ItemInnerSpacing => crate::sys::ImGuiStyleVar_ItemInnerSpacing as i32,
            Self::CellPadding => crate::sys::ImGuiStyleVar_CellPadding as i32,
            Self::TableAngledHeadersTextAlign => {
                crate::sys::ImGuiStyleVar_TableAngledHeadersTextAlign as i32
            }
            Self::ButtonTextAlign => crate::sys::ImGuiStyleVar_ButtonTextAlign as i32,
            Self::SelectableTextAlign => crate::sys::ImGuiStyleVar_SelectableTextAlign as i32,
            Self::SeparatorTextAlign => crate::sys::ImGuiStyleVar_SeparatorTextAlign as i32,
            Self::SeparatorTextPadding => crate::sys::ImGuiStyleVar_SeparatorTextPadding as i32,
        }
    }
}

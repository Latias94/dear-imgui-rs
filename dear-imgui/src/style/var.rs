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

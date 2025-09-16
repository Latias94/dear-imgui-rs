use crate::internal::RawWrapper;
use crate::sys;
use bitflags::bitflags;

/// User interface style/colors
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Style {
    /// Global alpha applies to everything
    pub alpha: f32,
    /// Additional alpha multiplier applied to disabled elements. Multiplies over current value of [`Style::alpha`].
    pub disabled_alpha: f32,
    /// Padding within a window
    pub window_padding: [f32; 2],
    /// Rounding radius of window corners.
    ///
    /// Set to 0.0 to have rectangular windows.
    /// Large values tend to lead to a variety of artifacts and are not recommended.
    pub window_rounding: f32,
    /// Thickness of border around windows.
    ///
    /// Generally set to 0.0 or 1.0 (other values are not well tested and cost more CPU/GPU).
    pub window_border_size: f32,
    /// Minimum window size
    pub window_min_size: [f32; 2],
    /// Alignment for title bar text
    pub window_title_align: [f32; 2],
    /// Side of the collapsing/docking button in the title bar (None/Left/Right)
    pub window_menu_button_position: Direction,
    /// Radius of child window corners rounding
    pub child_rounding: f32,
    /// Thickness of border around child windows
    pub child_border_size: f32,
    /// Radius of popup window corners rounding
    pub popup_rounding: f32,
    /// Thickness of border around popup/tooltip windows
    pub popup_border_size: f32,
    /// Padding within a framed rectangle (used by most widgets)
    pub frame_padding: [f32; 2],
    /// Radius of frame corners rounding
    pub frame_rounding: f32,
    /// Thickness of border around frames
    pub frame_border_size: f32,
    /// Horizontal and vertical spacing between widgets/lines
    pub item_spacing: [f32; 2],
    /// Horizontal and vertical spacing between within elements of a composed widget
    pub item_inner_spacing: [f32; 2],
    /// Padding within a table cell
    pub cell_padding: [f32; 2],
    /// Expand reactive bounding box for touch-based system where touch position is not accurate enough
    pub touch_extra_padding: [f32; 2],
    /// Horizontal indentation when e.g. entering a tree node
    pub indent_spacing: f32,
    /// Minimum horizontal spacing between two columns
    pub columns_min_spacing: f32,
    /// Width of the vertical scrollbar, height of the horizontal scrollbar
    pub scrollbar_size: f32,
    /// Radius of scrollbar corners rounding
    pub scrollbar_rounding: f32,
    /// Minimum width/height of a grab box for slider/scrollbar
    pub grab_min_size: f32,
    /// Radius of grabs corners rounding
    pub grab_rounding: f32,
    /// The size in pixels of the dead-zone around zero on logarithmic sliders that cross zero
    pub log_slider_deadzone: f32,
    /// Radius of upper corners of a tab
    pub tab_rounding: f32,
    /// Thickness of border around tabs
    pub tab_border_size: f32,
    /// Minimum width for close button to appears on an unselected tab when hovered
    pub tab_min_width_for_close_button: f32,
    /// Side of the color button in the ColorEdit4 widget (left/right)
    pub color_button_position: Direction,
    /// Alignment of button text when button is larger than text
    pub button_text_align: [f32; 2],
    /// Alignment of selectable text when selectable is larger than text
    pub selectable_text_align: [f32; 2],
    /// Window position are clamped to be visible within the display area or monitors by at least this amount
    pub display_window_padding: [f32; 2],
    /// If you cannot see the edges of your screen (e.g. on a TV) increase the safe area padding
    pub display_safe_area_padding: [f32; 2],
    /// Scale software rendered mouse cursor (when io.MouseDrawCursor is enabled)
    pub mouse_cursor_scale: f32,
    /// Enable anti-aliased lines/borders
    pub anti_aliased_lines: bool,
    /// Enable anti-aliased lines/borders using textures where possible
    pub anti_aliased_lines_use_tex: bool,
    /// Enable anti-aliased edges around filled shapes
    pub anti_aliased_fill: bool,
    /// Tessellation tolerance when using PathBezierCurveTo()
    pub curve_tessellation_tol: f32,
    /// Maximum error (in pixels) allowed when using AddCircle()/AddCircleFilled() or drawing rounded corner rectangles
    pub circle_tessellation_max_error: f32,
    /// Colors for various UI elements
    pub colors: [[f32; 4]; StyleColor::COUNT],
}

impl Style {
    /// Creates a new Style instance from the current context
    pub(crate) fn from_raw() -> Self {
        unsafe {
            let style_ptr = sys::ImGui_GetStyle();
            *(style_ptr as *const Style)
        }
    }
}

bitflags! {
    /// Flags for hover detection
    #[repr(transparent)]
    pub struct HoveredFlags: i32 {
        /// Return true if directly over the item/window, not obstructed by another window
        const NONE = sys::ImGuiHoveredFlags_None;
        /// Return true even if a child window is normally blocking access
        const CHILD_WINDOWS = sys::ImGuiHoveredFlags_ChildWindows;
        /// Return true even if an active item is blocking access
        const ALLOW_WHEN_BLOCKED_BY_ACTIVE_ITEM = sys::ImGuiHoveredFlags_AllowWhenBlockedByActiveItem;
        /// Return true even if the position is obstructed or overlapped by another window
        const ALLOW_WHEN_OVERLAPPED = sys::ImGuiHoveredFlags_AllowWhenOverlapped;
        /// Require mouse to be stationary for style.HoverStationaryDelay (~0.15 sec)
        const STATIONARY = sys::ImGuiHoveredFlags_Stationary;
        /// IsItemHovered() only: Return true immediately (default)
        const NO_DELAY_SHORT = sys::ImGuiHoveredFlags_DelayNone;
        /// IsItemHovered() only: Return true after HoverDelayShort elapsed (~0.15 sec)
        const DELAY_SHORT = sys::ImGuiHoveredFlags_DelayShort;
        /// IsItemHovered() only: Return true after HoverDelayNormal elapsed (~0.40 sec)
        const DELAY_NORMAL = sys::ImGuiHoveredFlags_DelayNormal;
        /// IsItemHovered() only: Disable shared delay system
        const NO_SHARED_DELAY = sys::ImGuiHoveredFlags_NoSharedDelay;
        /// For tooltip: Equivalent to Stationary + DelayShort
        const FOR_TOOLTIP = sys::ImGuiHoveredFlags_ForTooltip;
    }
}

/// A cardinal direction
#[repr(i32)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum Direction {
    None = sys::ImGuiDir_None,
    Left = sys::ImGuiDir_Left,
    Right = sys::ImGuiDir_Right,
    Up = sys::ImGuiDir_Up,
    Down = sys::ImGuiDir_Down,
}

/// Style color identifier
#[repr(i32)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum StyleColor {
    Text = sys::ImGuiCol_Text,
    TextDisabled = sys::ImGuiCol_TextDisabled,
    WindowBg = sys::ImGuiCol_WindowBg,
    ChildBg = sys::ImGuiCol_ChildBg,
    PopupBg = sys::ImGuiCol_PopupBg,
    Border = sys::ImGuiCol_Border,
    BorderShadow = sys::ImGuiCol_BorderShadow,
    FrameBg = sys::ImGuiCol_FrameBg,
    FrameBgHovered = sys::ImGuiCol_FrameBgHovered,
    FrameBgActive = sys::ImGuiCol_FrameBgActive,
    TitleBg = sys::ImGuiCol_TitleBg,
    TitleBgActive = sys::ImGuiCol_TitleBgActive,
    TitleBgCollapsed = sys::ImGuiCol_TitleBgCollapsed,
    MenuBarBg = sys::ImGuiCol_MenuBarBg,
    ScrollbarBg = sys::ImGuiCol_ScrollbarBg,
    ScrollbarGrab = sys::ImGuiCol_ScrollbarGrab,
    ScrollbarGrabHovered = sys::ImGuiCol_ScrollbarGrabHovered,
    ScrollbarGrabActive = sys::ImGuiCol_ScrollbarGrabActive,
    CheckMark = sys::ImGuiCol_CheckMark,
    SliderGrab = sys::ImGuiCol_SliderGrab,
    SliderGrabActive = sys::ImGuiCol_SliderGrabActive,
    Button = sys::ImGuiCol_Button,
    ButtonHovered = sys::ImGuiCol_ButtonHovered,
    ButtonActive = sys::ImGuiCol_ButtonActive,
    Header = sys::ImGuiCol_Header,
    HeaderHovered = sys::ImGuiCol_HeaderHovered,
    HeaderActive = sys::ImGuiCol_HeaderActive,
    Separator = sys::ImGuiCol_Separator,
    SeparatorHovered = sys::ImGuiCol_SeparatorHovered,
    SeparatorActive = sys::ImGuiCol_SeparatorActive,
    ResizeGrip = sys::ImGuiCol_ResizeGrip,
    ResizeGripHovered = sys::ImGuiCol_ResizeGripHovered,
    ResizeGripActive = sys::ImGuiCol_ResizeGripActive,
    Tab = sys::ImGuiCol_Tab,
    TabHovered = sys::ImGuiCol_TabHovered,
    // Newly added tab colors in docking branch
    TabSelected = sys::ImGuiCol_TabSelected,
    TabSelectedOverline = sys::ImGuiCol_TabSelectedOverline,
    TabDimmed = sys::ImGuiCol_TabDimmed,
    TabDimmedSelected = sys::ImGuiCol_TabDimmedSelected,
    TabDimmedSelectedOverline = sys::ImGuiCol_TabDimmedSelectedOverline,
    #[cfg(feature = "docking")]
    DockingPreview = sys::ImGuiCol_DockingPreview,
    #[cfg(feature = "docking")]
    DockingEmptyBg = sys::ImGuiCol_DockingEmptyBg,
    PlotLines = sys::ImGuiCol_PlotLines,
    PlotLinesHovered = sys::ImGuiCol_PlotLinesHovered,
    PlotHistogram = sys::ImGuiCol_PlotHistogram,
    PlotHistogramHovered = sys::ImGuiCol_PlotHistogramHovered,
    TableHeaderBg = sys::ImGuiCol_TableHeaderBg,
    TableBorderStrong = sys::ImGuiCol_TableBorderStrong,
    TableBorderLight = sys::ImGuiCol_TableBorderLight,
    TableRowBg = sys::ImGuiCol_TableRowBg,
    TableRowBgAlt = sys::ImGuiCol_TableRowBgAlt,
    TextSelectedBg = sys::ImGuiCol_TextSelectedBg,
    TextLink = sys::ImGuiCol_TextLink,
    TreeLines = sys::ImGuiCol_TreeLines,
    InputTextCursor = sys::ImGuiCol_InputTextCursor,
    DragDropTarget = sys::ImGuiCol_DragDropTarget,
    NavCursor = sys::ImGuiCol_NavCursor,
    NavWindowingHighlight = sys::ImGuiCol_NavWindowingHighlight,
    NavWindowingDimBg = sys::ImGuiCol_NavWindowingDimBg,
    ModalWindowDimBg = sys::ImGuiCol_ModalWindowDimBg,
}

impl StyleColor {
    pub const COUNT: usize = sys::ImGuiCol_COUNT as usize;
}

impl Default for Style {
    fn default() -> Self {
        Self {
            alpha: 1.0,
            disabled_alpha: 0.6,
            window_padding: [8.0, 8.0],
            window_rounding: 0.0,
            window_border_size: 1.0,
            window_min_size: [32.0, 32.0],
            window_title_align: [0.0, 0.5],
            window_menu_button_position: Direction::Left,
            child_rounding: 0.0,
            child_border_size: 1.0,
            popup_rounding: 0.0,
            popup_border_size: 1.0,
            frame_padding: [4.0, 3.0],
            frame_rounding: 0.0,
            frame_border_size: 0.0,
            item_spacing: [8.0, 4.0],
            item_inner_spacing: [4.0, 4.0],
            cell_padding: [4.0, 2.0],
            touch_extra_padding: [0.0, 0.0],
            indent_spacing: 21.0,
            columns_min_spacing: 6.0,
            scrollbar_size: 14.0,
            scrollbar_rounding: 9.0,
            grab_min_size: 12.0,
            grab_rounding: 0.0,
            log_slider_deadzone: 4.0,
            tab_rounding: 4.0,
            tab_border_size: 0.0,
            tab_min_width_for_close_button: 0.0,
            color_button_position: Direction::Right,
            button_text_align: [0.5, 0.5],
            selectable_text_align: [0.0, 0.0],
            display_window_padding: [19.0, 19.0],
            display_safe_area_padding: [3.0, 3.0],
            mouse_cursor_scale: 1.0,
            anti_aliased_lines: true,
            anti_aliased_lines_use_tex: true,
            anti_aliased_fill: true,
            curve_tessellation_tol: 1.25,
            circle_tessellation_max_error: 0.30,
            colors: [[1.00, 1.00, 1.00, 1.00]; StyleColor::COUNT],
        }
    }
}

impl Style {
    /// Get a color by style color identifier
    pub fn color(&self, color: StyleColor) -> [f32; 4] {
        self.colors[color as usize]
    }

    /// Set a color by style color identifier
    pub fn set_color(&mut self, color: StyleColor, value: [f32; 4]) {
        self.colors[color as usize] = value;
    }
}

impl RawWrapper for Style {
    type Raw = sys::ImGuiStyle;

    unsafe fn raw(&self) -> &Self::Raw {
        &*(self as *const _ as *const Self::Raw)
    }

    unsafe fn raw_mut(&mut self) -> &mut Self::Raw {
        &mut *(self as *mut _ as *mut Self::Raw)
    }
}

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
    /// Minimum width/height of a grab box for slider/scrollbar
    GrabMinSize(f32),
    /// Rounding radius of grabs corners
    GrabRounding(f32),
    /// Rounding radius of upper corners of tabs
    TabRounding(f32),
    /// Alignment of button text when button is larger than text
    ButtonTextAlign([f32; 2]),
    /// Alignment of selectable text when selectable is larger than text
    SelectableTextAlign([f32; 2]),
}

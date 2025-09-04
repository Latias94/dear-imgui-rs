//! Style system for Dear ImGui
//!
//! This module provides access to Dear ImGui's style system, which controls
//! the appearance of UI elements including colors, spacing, and rounding.

use crate::types::{Color, Vec2};
use dear_imgui_sys as sys;

/// Direction for various UI elements
#[repr(i32)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum Direction {
    None = sys::ImGuiDir_None,
    Left = sys::ImGuiDir_Left,
    Right = sys::ImGuiDir_Right,
    Up = sys::ImGuiDir_Up,
    Down = sys::ImGuiDir_Down,
}

/// Style color identifiers
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
    TabActive = sys::ImGuiCol_TabActive,
    TabUnfocused = sys::ImGuiCol_TabUnfocused,
    TabUnfocusedActive = sys::ImGuiCol_TabUnfocusedActive,
    DockingPreview = sys::ImGuiCol_DockingPreview,
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
    DragDropTarget = sys::ImGuiCol_DragDropTarget,
    NavHighlight = sys::ImGuiCol_NavHighlight,
    NavWindowingHighlight = sys::ImGuiCol_NavWindowingHighlight,
    NavWindowingDimBg = sys::ImGuiCol_NavWindowingDimBg,
    ModalWindowDimBg = sys::ImGuiCol_ModalWindowDimBg,
}

/// Style variable types for temporary style modifications
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum StyleVar {
    /// Global alpha applies to everything
    Alpha(f32),
    /// Additional alpha multiplier for disabled items
    DisabledAlpha(f32),
    /// Padding within a window
    WindowPadding(Vec2),
    /// Radius of window corners rounding
    WindowRounding(f32),
    /// Thickness of border around windows
    WindowBorderSize(f32),
    /// Minimum window size
    WindowMinSize(Vec2),
    /// Alignment for title bar text
    WindowTitleAlign(Vec2),
    /// Radius of child window corners rounding
    ChildRounding(f32),
    /// Thickness of border around child windows
    ChildBorderSize(f32),
    /// Radius of popup window corners rounding
    PopupRounding(f32),
    /// Thickness of border around popup/tooltip windows
    PopupBorderSize(f32),
    /// Padding within a framed rectangle (used by most widgets)
    FramePadding(Vec2),
    /// Radius of frame corners rounding
    FrameRounding(f32),
    /// Thickness of border around frames
    FrameBorderSize(f32),
    /// Horizontal and vertical spacing between widgets/lines
    ItemSpacing(Vec2),
    /// Horizontal and vertical spacing between elements of a composed widget
    ItemInnerSpacing(Vec2),
    /// Expand reactive bounding box for touch-based system
    TouchExtraPadding(Vec2),
    /// Horizontal indentation when entering a tree node
    IndentSpacing(f32),
    /// Minimum horizontal spacing between two columns
    ColumnsMinSpacing(f32),
    /// Width of the vertical scrollbar, height of the horizontal scrollbar
    ScrollbarSize(f32),
    /// Radius of scrollbar grab corners rounding
    ScrollbarRounding(f32),
    /// Minimum width/height of a grab box for slider/scrollbar
    GrabMinSize(f32),
    /// Radius of grab corners rounding
    GrabRounding(f32),
    /// Radius of upper corners of tabs
    TabRounding(f32),
    /// Thickness of border around tabs
    TabBorderSize(f32),
    /// Side of the collapsing/docking button in the title bar
    ButtonTextAlign(Vec2),
    /// Alignment of selectable text
    SelectableTextAlign(Vec2),
    /// Padding within a table cell
    CellPadding(Vec2),
}

/// Dear ImGui style structure
pub struct Style {
    raw: *mut sys::ImGuiStyle,
}

impl Style {
    /// Create a new Style wrapper from a raw pointer
    ///
    /// # Safety
    /// The caller must ensure the pointer is valid and lives as long as this Style
    pub(crate) unsafe fn from_raw(raw: *mut sys::ImGuiStyle) -> Self {
        Self { raw }
    }

    /// Get the raw ImGuiStyle pointer
    pub(crate) fn raw(&self) -> *const sys::ImGuiStyle {
        self.raw
    }

    /// Get the raw ImGuiStyle pointer (mutable)
    pub(crate) fn raw_mut(&mut self) -> *mut sys::ImGuiStyle {
        self.raw
    }

    /// Get global alpha
    pub fn alpha(&self) -> f32 {
        unsafe { (*self.raw).Alpha }
    }

    /// Set global alpha
    pub fn set_alpha(&mut self, alpha: f32) {
        unsafe {
            (*self.raw).Alpha = alpha;
        }
    }

    /// Get disabled alpha
    pub fn disabled_alpha(&self) -> f32 {
        unsafe { (*self.raw).DisabledAlpha }
    }

    /// Set disabled alpha
    pub fn set_disabled_alpha(&mut self, alpha: f32) {
        unsafe {
            (*self.raw).DisabledAlpha = alpha;
        }
    }

    /// Get window padding
    pub fn window_padding(&self) -> Vec2 {
        unsafe {
            let padding = (*self.raw).WindowPadding;
            Vec2::new(padding.x, padding.y)
        }
    }

    /// Set window padding
    pub fn set_window_padding(&mut self, padding: Vec2) {
        unsafe {
            (*self.raw).WindowPadding = sys::ImVec2 { x: padding.x, y: padding.y };
        }
    }

    /// Get window rounding
    pub fn window_rounding(&self) -> f32 {
        unsafe { (*self.raw).WindowRounding }
    }

    /// Set window rounding
    pub fn set_window_rounding(&mut self, rounding: f32) {
        unsafe {
            (*self.raw).WindowRounding = rounding;
        }
    }

    /// Get frame padding
    pub fn frame_padding(&self) -> Vec2 {
        unsafe {
            let padding = (*self.raw).FramePadding;
            Vec2::new(padding.x, padding.y)
        }
    }

    /// Set frame padding
    pub fn set_frame_padding(&mut self, padding: Vec2) {
        unsafe {
            (*self.raw).FramePadding = sys::ImVec2 { x: padding.x, y: padding.y };
        }
    }

    /// Get item spacing
    pub fn item_spacing(&self) -> Vec2 {
        unsafe {
            let spacing = (*self.raw).ItemSpacing;
            Vec2::new(spacing.x, spacing.y)
        }
    }

    /// Set item spacing
    pub fn set_item_spacing(&mut self, spacing: Vec2) {
        unsafe {
            (*self.raw).ItemSpacing = sys::ImVec2 { x: spacing.x, y: spacing.y };
        }
    }

    /// Get a color value
    pub fn color(&self, color: StyleColor) -> Color {
        unsafe {
            let color_vec = (*self.raw).Colors[color as usize];
            Color::rgba(color_vec.x, color_vec.y, color_vec.z, color_vec.w)
        }
    }

    /// Set a color value
    pub fn set_color(&mut self, color: StyleColor, value: Color) {
        unsafe {
            (*self.raw).Colors[color as usize] = sys::ImVec4 {
                x: value.r(),
                y: value.g(),
                z: value.b(),
                w: value.a(),
            };
        }
    }

    /// Scale all sizes in the style
    pub fn scale_all_sizes(&mut self, scale_factor: f32) {
        unsafe {
            sys::ImGuiStyle_ScaleAllSizes(self.raw, scale_factor);
        }
    }

    /// Use dark color scheme
    pub fn use_dark_colors(&mut self) {
        unsafe {
            sys::ImGui_StyleColorsDark(self.raw);
        }
    }

    /// Use light color scheme
    pub fn use_light_colors(&mut self) {
        unsafe {
            sys::ImGui_StyleColorsLight(self.raw);
        }
    }

    /// Use classic color scheme
    pub fn use_classic_colors(&mut self) {
        unsafe {
            sys::ImGui_StyleColorsClassic(self.raw);
        }
    }
}

/// Token for style variable stack operations
pub struct StyleVarToken<'a> {
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> StyleVarToken<'a> {
    pub(crate) fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }

    /// Pop the style variable from the stack
    pub fn pop(self) {
        // TODO: Implement actual style popping when FFI bindings are available
    }
}

impl<'a> Drop for StyleVarToken<'a> {
    fn drop(&mut self) {
        // TODO: Implement actual style popping when FFI bindings are available
    }
}

/// Push a style variable to the stack
pub fn push_style_var(style_var: StyleVar) -> StyleVarToken<'static> {
    // For now, we'll implement a basic version that doesn't actually push styles
    // This is a placeholder until we have the proper FFI bindings
    match style_var {
        StyleVar::Alpha(_) => {
            // TODO: Implement actual style pushing when FFI bindings are available
        }
        _ => {
            // TODO: Implement other style variables
        }
    }
    StyleVarToken::new()
}

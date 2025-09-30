//! Styling and colors
//!
//! High-level access to Dear ImGui style parameters and color table. Use this
//! module to read or tweak padding, rounding, sizes and retrieve or modify
//! named colors via [`StyleColor`].
//!
//! Example:
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! // Adjust style before building a frame
//! {
//!     let style = ctx.style_mut();
//!     style.set_window_rounding(6.0);
//!     style.set_color(StyleColor::WindowBg, [0.10, 0.10, 0.12, 1.0]);
//! }
//! // Optionally show the style editor for the current style
//! # let ui = ctx.frame();
//! ui.show_default_style_editor();
//! ```
//!
//! Quick example (temporary style color):
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! let c = ui.push_style_color(StyleColor::Text, [0.2, 1.0, 0.2, 1.0]);
//! ui.text("green text");
//! c.pop();
//! ```
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]
use crate::internal::RawWrapper;
use crate::sys;
use crate::utils::HoveredFlags;
use crate::widget::TreeNodeFlags;

/// User interface style/colors
///
/// Note: This is a transparent wrapper over `sys::ImGuiStyle` (v1.92+ layout).
/// Do not assume field layout here; use accessors or `raw()/raw_mut()` if needed.
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Style(pub(crate) sys::ImGuiStyle);

impl Style {
    /// Get a color by style color identifier
    pub fn color(&self, color: StyleColor) -> [f32; 4] {
        let c = self.0.Colors[color as usize];
        [c.x, c.y, c.z, c.w]
    }

    /// Set a color by style color identifier
    pub fn set_color(&mut self, color: StyleColor, value: [f32; 4]) {
        self.0.Colors[color as usize] = sys::ImVec4 {
            x: value[0],
            y: value[1],
            z: value[2],
            w: value[3],
        };
    }

    /// Get main font scale (formerly io.FontGlobalScale)
    pub fn font_scale_main(&self) -> f32 {
        self.0.FontScaleMain
    }

    /// Set main font scale (formerly io.FontGlobalScale)
    pub fn set_font_scale_main(&mut self, scale: f32) {
        self.0.FontScaleMain = scale;
    }

    /// Get DPI font scale (auto-overwritten if ConfigDpiScaleFonts=true)
    pub fn font_scale_dpi(&self) -> f32 {
        self.0.FontScaleDpi
    }

    /// Set DPI font scale
    pub fn set_font_scale_dpi(&mut self, scale: f32) {
        self.0.FontScaleDpi = scale;
    }

    /// Base size used by style for font sizing
    pub fn font_size_base(&self) -> f32 {
        self.0.FontSizeBase
    }

    pub fn set_font_size_base(&mut self, sz: f32) {
        self.0.FontSizeBase = sz;
    }

    // Common style accessors (typed, convenient)

    pub fn alpha(&self) -> f32 {
        self.0.Alpha
    }
    pub fn set_alpha(&mut self, v: f32) {
        self.0.Alpha = v;
    }

    pub fn disabled_alpha(&self) -> f32 {
        self.0.DisabledAlpha
    }
    pub fn set_disabled_alpha(&mut self, v: f32) {
        self.0.DisabledAlpha = v;
    }

    pub fn window_padding(&self) -> [f32; 2] {
        [self.0.WindowPadding.x, self.0.WindowPadding.y]
    }
    pub fn set_window_padding(&mut self, v: [f32; 2]) {
        self.0.WindowPadding = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn window_rounding(&self) -> f32 {
        self.0.WindowRounding
    }
    pub fn set_window_rounding(&mut self, v: f32) {
        self.0.WindowRounding = v;
    }

    pub fn window_border_size(&self) -> f32 {
        self.0.WindowBorderSize
    }
    pub fn set_window_border_size(&mut self, v: f32) {
        self.0.WindowBorderSize = v;
    }

    pub fn window_min_size(&self) -> [f32; 2] {
        [self.0.WindowMinSize.x, self.0.WindowMinSize.y]
    }
    pub fn set_window_min_size(&mut self, v: [f32; 2]) {
        self.0.WindowMinSize = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn window_title_align(&self) -> [f32; 2] {
        [self.0.WindowTitleAlign.x, self.0.WindowTitleAlign.y]
    }
    pub fn set_window_title_align(&mut self, v: [f32; 2]) {
        self.0.WindowTitleAlign = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn window_menu_button_position(&self) -> Direction {
        Direction::from(self.0.WindowMenuButtonPosition)
    }
    pub fn set_window_menu_button_position(&mut self, d: Direction) {
        self.0.WindowMenuButtonPosition = d.into();
    }

    pub fn child_rounding(&self) -> f32 {
        self.0.ChildRounding
    }
    pub fn set_child_rounding(&mut self, v: f32) {
        self.0.ChildRounding = v;
    }

    pub fn child_border_size(&self) -> f32 {
        self.0.ChildBorderSize
    }
    pub fn set_child_border_size(&mut self, v: f32) {
        self.0.ChildBorderSize = v;
    }

    pub fn popup_rounding(&self) -> f32 {
        self.0.PopupRounding
    }
    pub fn set_popup_rounding(&mut self, v: f32) {
        self.0.PopupRounding = v;
    }

    pub fn popup_border_size(&self) -> f32 {
        self.0.PopupBorderSize
    }
    pub fn set_popup_border_size(&mut self, v: f32) {
        self.0.PopupBorderSize = v;
    }

    pub fn frame_padding(&self) -> [f32; 2] {
        [self.0.FramePadding.x, self.0.FramePadding.y]
    }
    pub fn set_frame_padding(&mut self, v: [f32; 2]) {
        self.0.FramePadding = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn frame_rounding(&self) -> f32 {
        self.0.FrameRounding
    }
    pub fn set_frame_rounding(&mut self, v: f32) {
        self.0.FrameRounding = v;
    }

    pub fn frame_border_size(&self) -> f32 {
        self.0.FrameBorderSize
    }
    pub fn set_frame_border_size(&mut self, v: f32) {
        self.0.FrameBorderSize = v;
    }

    pub fn item_spacing(&self) -> [f32; 2] {
        [self.0.ItemSpacing.x, self.0.ItemSpacing.y]
    }
    pub fn set_item_spacing(&mut self, v: [f32; 2]) {
        self.0.ItemSpacing = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn item_inner_spacing(&self) -> [f32; 2] {
        [self.0.ItemInnerSpacing.x, self.0.ItemInnerSpacing.y]
    }
    pub fn set_item_inner_spacing(&mut self, v: [f32; 2]) {
        self.0.ItemInnerSpacing = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn cell_padding(&self) -> [f32; 2] {
        [self.0.CellPadding.x, self.0.CellPadding.y]
    }
    pub fn set_cell_padding(&mut self, v: [f32; 2]) {
        self.0.CellPadding = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn touch_extra_padding(&self) -> [f32; 2] {
        [self.0.TouchExtraPadding.x, self.0.TouchExtraPadding.y]
    }
    pub fn set_touch_extra_padding(&mut self, v: [f32; 2]) {
        self.0.TouchExtraPadding = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn indent_spacing(&self) -> f32 {
        self.0.IndentSpacing
    }
    pub fn set_indent_spacing(&mut self, v: f32) {
        self.0.IndentSpacing = v;
    }

    pub fn columns_min_spacing(&self) -> f32 {
        self.0.ColumnsMinSpacing
    }
    pub fn set_columns_min_spacing(&mut self, v: f32) {
        self.0.ColumnsMinSpacing = v;
    }

    pub fn scrollbar_size(&self) -> f32 {
        self.0.ScrollbarSize
    }
    pub fn set_scrollbar_size(&mut self, v: f32) {
        self.0.ScrollbarSize = v;
    }

    pub fn scrollbar_rounding(&self) -> f32 {
        self.0.ScrollbarRounding
    }
    pub fn set_scrollbar_rounding(&mut self, v: f32) {
        self.0.ScrollbarRounding = v;
    }

    pub fn grab_min_size(&self) -> f32 {
        self.0.GrabMinSize
    }
    pub fn set_grab_min_size(&mut self, v: f32) {
        self.0.GrabMinSize = v;
    }

    pub fn grab_rounding(&self) -> f32 {
        self.0.GrabRounding
    }
    pub fn set_grab_rounding(&mut self, v: f32) {
        self.0.GrabRounding = v;
    }

    pub fn log_slider_deadzone(&self) -> f32 {
        self.0.LogSliderDeadzone
    }
    pub fn set_log_slider_deadzone(&mut self, v: f32) {
        self.0.LogSliderDeadzone = v;
    }

    pub fn tab_rounding(&self) -> f32 {
        self.0.TabRounding
    }
    pub fn set_tab_rounding(&mut self, v: f32) {
        self.0.TabRounding = v;
    }

    pub fn tab_border_size(&self) -> f32 {
        self.0.TabBorderSize
    }
    pub fn set_tab_border_size(&mut self, v: f32) {
        self.0.TabBorderSize = v;
    }

    pub fn color_button_position(&self) -> Direction {
        Direction::from(self.0.ColorButtonPosition)
    }
    pub fn set_color_button_position(&mut self, d: Direction) {
        self.0.ColorButtonPosition = d.into();
    }

    pub fn button_text_align(&self) -> [f32; 2] {
        [self.0.ButtonTextAlign.x, self.0.ButtonTextAlign.y]
    }
    pub fn set_button_text_align(&mut self, v: [f32; 2]) {
        self.0.ButtonTextAlign = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn selectable_text_align(&self) -> [f32; 2] {
        [self.0.SelectableTextAlign.x, self.0.SelectableTextAlign.y]
    }
    pub fn set_selectable_text_align(&mut self, v: [f32; 2]) {
        self.0.SelectableTextAlign = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn display_window_padding(&self) -> [f32; 2] {
        [self.0.DisplayWindowPadding.x, self.0.DisplayWindowPadding.y]
    }
    pub fn set_display_window_padding(&mut self, v: [f32; 2]) {
        self.0.DisplayWindowPadding = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn display_safe_area_padding(&self) -> [f32; 2] {
        [
            self.0.DisplaySafeAreaPadding.x,
            self.0.DisplaySafeAreaPadding.y,
        ]
    }
    pub fn set_display_safe_area_padding(&mut self, v: [f32; 2]) {
        self.0.DisplaySafeAreaPadding = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn mouse_cursor_scale(&self) -> f32 {
        self.0.MouseCursorScale
    }
    pub fn set_mouse_cursor_scale(&mut self, v: f32) {
        self.0.MouseCursorScale = v;
    }

    pub fn anti_aliased_lines(&self) -> bool {
        self.0.AntiAliasedLines
    }
    pub fn set_anti_aliased_lines(&mut self, v: bool) {
        self.0.AntiAliasedLines = v;
    }

    pub fn anti_aliased_lines_use_tex(&self) -> bool {
        self.0.AntiAliasedLinesUseTex
    }
    pub fn set_anti_aliased_lines_use_tex(&mut self, v: bool) {
        self.0.AntiAliasedLinesUseTex = v;
    }

    pub fn anti_aliased_fill(&self) -> bool {
        self.0.AntiAliasedFill
    }
    pub fn set_anti_aliased_fill(&mut self, v: bool) {
        self.0.AntiAliasedFill = v;
    }

    pub fn curve_tessellation_tol(&self) -> f32 {
        self.0.CurveTessellationTol
    }
    pub fn set_curve_tessellation_tol(&mut self, v: f32) {
        self.0.CurveTessellationTol = v;
    }

    pub fn circle_tessellation_max_error(&self) -> f32 {
        self.0.CircleTessellationMaxError
    }
    pub fn set_circle_tessellation_max_error(&mut self, v: f32) {
        self.0.CircleTessellationMaxError = v;
    }

    // Newly exposed 1.92+ or less-common fields

    pub fn window_border_hover_padding(&self) -> f32 {
        self.0.WindowBorderHoverPadding
    }
    pub fn set_window_border_hover_padding(&mut self, v: f32) {
        self.0.WindowBorderHoverPadding = v;
    }

    pub fn scrollbar_padding(&self) -> f32 {
        self.0.ScrollbarPadding
    }
    pub fn set_scrollbar_padding(&mut self, v: f32) {
        self.0.ScrollbarPadding = v;
    }

    pub fn image_border_size(&self) -> f32 {
        self.0.ImageBorderSize
    }
    pub fn set_image_border_size(&mut self, v: f32) {
        self.0.ImageBorderSize = v;
    }

    pub fn tab_min_width_base(&self) -> f32 {
        self.0.TabMinWidthBase
    }
    pub fn set_tab_min_width_base(&mut self, v: f32) {
        self.0.TabMinWidthBase = v;
    }

    pub fn tab_min_width_shrink(&self) -> f32 {
        self.0.TabMinWidthShrink
    }
    pub fn set_tab_min_width_shrink(&mut self, v: f32) {
        self.0.TabMinWidthShrink = v;
    }

    pub fn tab_close_button_min_width_selected(&self) -> f32 {
        self.0.TabCloseButtonMinWidthSelected
    }
    pub fn set_tab_close_button_min_width_selected(&mut self, v: f32) {
        self.0.TabCloseButtonMinWidthSelected = v;
    }

    pub fn tab_close_button_min_width_unselected(&self) -> f32 {
        self.0.TabCloseButtonMinWidthUnselected
    }
    pub fn set_tab_close_button_min_width_unselected(&mut self, v: f32) {
        self.0.TabCloseButtonMinWidthUnselected = v;
    }

    pub fn tab_bar_border_size(&self) -> f32 {
        self.0.TabBarBorderSize
    }
    pub fn set_tab_bar_border_size(&mut self, v: f32) {
        self.0.TabBarBorderSize = v;
    }

    pub fn tab_bar_overline_size(&self) -> f32 {
        self.0.TabBarOverlineSize
    }
    pub fn set_tab_bar_overline_size(&mut self, v: f32) {
        self.0.TabBarOverlineSize = v;
    }

    pub fn table_angled_headers_angle(&self) -> f32 {
        self.0.TableAngledHeadersAngle
    }
    pub fn set_table_angled_headers_angle(&mut self, v: f32) {
        self.0.TableAngledHeadersAngle = v;
    }

    pub fn table_angled_headers_text_align(&self) -> [f32; 2] {
        [
            self.0.TableAngledHeadersTextAlign.x,
            self.0.TableAngledHeadersTextAlign.y,
        ]
    }
    pub fn set_table_angled_headers_text_align(&mut self, v: [f32; 2]) {
        self.0.TableAngledHeadersTextAlign = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn tree_lines_flags(&self) -> TreeNodeFlags {
        TreeNodeFlags::from_bits_truncate(self.0.TreeLinesFlags as i32)
    }
    pub fn set_tree_lines_flags(&mut self, flags: TreeNodeFlags) {
        self.0.TreeLinesFlags = flags.bits() as sys::ImGuiTreeNodeFlags;
    }

    pub fn tree_lines_size(&self) -> f32 {
        self.0.TreeLinesSize
    }
    pub fn set_tree_lines_size(&mut self, v: f32) {
        self.0.TreeLinesSize = v;
    }

    pub fn tree_lines_rounding(&self) -> f32 {
        self.0.TreeLinesRounding
    }
    pub fn set_tree_lines_rounding(&mut self, v: f32) {
        self.0.TreeLinesRounding = v;
    }

    pub fn separator_text_border_size(&self) -> f32 {
        self.0.SeparatorTextBorderSize
    }
    pub fn set_separator_text_border_size(&mut self, v: f32) {
        self.0.SeparatorTextBorderSize = v;
    }

    pub fn separator_text_align(&self) -> [f32; 2] {
        [self.0.SeparatorTextAlign.x, self.0.SeparatorTextAlign.y]
    }
    pub fn set_separator_text_align(&mut self, v: [f32; 2]) {
        self.0.SeparatorTextAlign = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn separator_text_padding(&self) -> [f32; 2] {
        [self.0.SeparatorTextPadding.x, self.0.SeparatorTextPadding.y]
    }
    pub fn set_separator_text_padding(&mut self, v: [f32; 2]) {
        self.0.SeparatorTextPadding = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn docking_node_has_close_button(&self) -> bool {
        self.0.DockingNodeHasCloseButton
    }
    pub fn set_docking_node_has_close_button(&mut self, v: bool) {
        self.0.DockingNodeHasCloseButton = v;
    }

    pub fn docking_separator_size(&self) -> f32 {
        self.0.DockingSeparatorSize
    }
    pub fn set_docking_separator_size(&mut self, v: f32) {
        self.0.DockingSeparatorSize = v;
    }

    pub fn hover_stationary_delay(&self) -> f32 {
        self.0.HoverStationaryDelay
    }
    pub fn set_hover_stationary_delay(&mut self, v: f32) {
        self.0.HoverStationaryDelay = v;
    }

    pub fn hover_delay_short(&self) -> f32 {
        self.0.HoverDelayShort
    }
    pub fn set_hover_delay_short(&mut self, v: f32) {
        self.0.HoverDelayShort = v;
    }

    pub fn hover_delay_normal(&self) -> f32 {
        self.0.HoverDelayNormal
    }
    pub fn set_hover_delay_normal(&mut self, v: f32) {
        self.0.HoverDelayNormal = v;
    }

    pub fn hover_flags_for_tooltip_mouse(&self) -> HoveredFlags {
        HoveredFlags::from_bits_truncate(self.0.HoverFlagsForTooltipMouse as i32)
    }
    pub fn set_hover_flags_for_tooltip_mouse(&mut self, flags: HoveredFlags) {
        self.0.HoverFlagsForTooltipMouse = flags.bits() as sys::ImGuiHoveredFlags;
    }

    pub fn hover_flags_for_tooltip_nav(&self) -> HoveredFlags {
        HoveredFlags::from_bits_truncate(self.0.HoverFlagsForTooltipNav as i32)
    }
    pub fn set_hover_flags_for_tooltip_nav(&mut self, flags: HoveredFlags) {
        self.0.HoverFlagsForTooltipNav = flags.bits() as sys::ImGuiHoveredFlags;
    }
}

// HoveredFlags are defined in utils.rs and re-exported at crate root.

/// A cardinal direction
#[repr(i32)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum Direction {
    None = sys::ImGuiDir_None as i32,
    Left = sys::ImGuiDir_Left as i32,
    Right = sys::ImGuiDir_Right as i32,
    Up = sys::ImGuiDir_Up as i32,
    Down = sys::ImGuiDir_Down as i32,
}

impl From<sys::ImGuiDir> for Direction {
    fn from(d: sys::ImGuiDir) -> Self {
        match d as i32 {
            x if x == sys::ImGuiDir_Left as i32 => Direction::Left,
            x if x == sys::ImGuiDir_Right as i32 => Direction::Right,
            x if x == sys::ImGuiDir_Up as i32 => Direction::Up,
            x if x == sys::ImGuiDir_Down as i32 => Direction::Down,
            _ => Direction::None,
        }
    }
}

impl From<Direction> for sys::ImGuiDir {
    fn from(d: Direction) -> Self {
        match d {
            Direction::None => sys::ImGuiDir_None,
            Direction::Left => sys::ImGuiDir_Left,
            Direction::Right => sys::ImGuiDir_Right,
            Direction::Up => sys::ImGuiDir_Up,
            Direction::Down => sys::ImGuiDir_Down,
        }
    }
}

/// Style color identifier
#[repr(i32)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum StyleColor {
    Text = sys::ImGuiCol_Text as i32,
    TextDisabled = sys::ImGuiCol_TextDisabled as i32,
    WindowBg = sys::ImGuiCol_WindowBg as i32,
    ChildBg = sys::ImGuiCol_ChildBg as i32,
    PopupBg = sys::ImGuiCol_PopupBg as i32,
    Border = sys::ImGuiCol_Border as i32,
    BorderShadow = sys::ImGuiCol_BorderShadow as i32,
    FrameBg = sys::ImGuiCol_FrameBg as i32,
    FrameBgHovered = sys::ImGuiCol_FrameBgHovered as i32,
    FrameBgActive = sys::ImGuiCol_FrameBgActive as i32,
    TitleBg = sys::ImGuiCol_TitleBg as i32,
    TitleBgActive = sys::ImGuiCol_TitleBgActive as i32,
    TitleBgCollapsed = sys::ImGuiCol_TitleBgCollapsed as i32,
    MenuBarBg = sys::ImGuiCol_MenuBarBg as i32,
    ScrollbarBg = sys::ImGuiCol_ScrollbarBg as i32,
    ScrollbarGrab = sys::ImGuiCol_ScrollbarGrab as i32,
    ScrollbarGrabHovered = sys::ImGuiCol_ScrollbarGrabHovered as i32,
    ScrollbarGrabActive = sys::ImGuiCol_ScrollbarGrabActive as i32,
    CheckMark = sys::ImGuiCol_CheckMark as i32,
    SliderGrab = sys::ImGuiCol_SliderGrab as i32,
    SliderGrabActive = sys::ImGuiCol_SliderGrabActive as i32,
    Button = sys::ImGuiCol_Button as i32,
    ButtonHovered = sys::ImGuiCol_ButtonHovered as i32,
    ButtonActive = sys::ImGuiCol_ButtonActive as i32,
    Header = sys::ImGuiCol_Header as i32,
    HeaderHovered = sys::ImGuiCol_HeaderHovered as i32,
    HeaderActive = sys::ImGuiCol_HeaderActive as i32,
    Separator = sys::ImGuiCol_Separator as i32,
    SeparatorHovered = sys::ImGuiCol_SeparatorHovered as i32,
    SeparatorActive = sys::ImGuiCol_SeparatorActive as i32,
    ResizeGrip = sys::ImGuiCol_ResizeGrip as i32,
    ResizeGripHovered = sys::ImGuiCol_ResizeGripHovered as i32,
    ResizeGripActive = sys::ImGuiCol_ResizeGripActive as i32,
    Tab = sys::ImGuiCol_Tab as i32,
    TabHovered = sys::ImGuiCol_TabHovered as i32,
    // Newly added tab colors in docking branch
    TabSelected = sys::ImGuiCol_TabSelected as i32,
    TabSelectedOverline = sys::ImGuiCol_TabSelectedOverline as i32,
    TabDimmed = sys::ImGuiCol_TabDimmed as i32,
    TabDimmedSelected = sys::ImGuiCol_TabDimmedSelected as i32,
    TabDimmedSelectedOverline = sys::ImGuiCol_TabDimmedSelectedOverline as i32,
    DockingPreview = sys::ImGuiCol_DockingPreview as i32,
    DockingEmptyBg = sys::ImGuiCol_DockingEmptyBg as i32,
    PlotLines = sys::ImGuiCol_PlotLines as i32,
    PlotLinesHovered = sys::ImGuiCol_PlotLinesHovered as i32,
    PlotHistogram = sys::ImGuiCol_PlotHistogram as i32,
    PlotHistogramHovered = sys::ImGuiCol_PlotHistogramHovered as i32,
    TableHeaderBg = sys::ImGuiCol_TableHeaderBg as i32,
    TableBorderStrong = sys::ImGuiCol_TableBorderStrong as i32,
    TableBorderLight = sys::ImGuiCol_TableBorderLight as i32,
    TableRowBg = sys::ImGuiCol_TableRowBg as i32,
    TableRowBgAlt = sys::ImGuiCol_TableRowBgAlt as i32,
    TextSelectedBg = sys::ImGuiCol_TextSelectedBg as i32,
    TextLink = sys::ImGuiCol_TextLink as i32,
    TreeLines = sys::ImGuiCol_TreeLines as i32,
    InputTextCursor = sys::ImGuiCol_InputTextCursor as i32,
    DragDropTarget = sys::ImGuiCol_DragDropTarget as i32,
    NavCursor = sys::ImGuiCol_NavCursor as i32,
    NavWindowingHighlight = sys::ImGuiCol_NavWindowingHighlight as i32,
    NavWindowingDimBg = sys::ImGuiCol_NavWindowingDimBg as i32,
    ModalWindowDimBg = sys::ImGuiCol_ModalWindowDimBg as i32,
}

impl StyleColor {
    pub const COUNT: usize = sys::ImGuiCol_COUNT as usize;
}

impl RawWrapper for Style {
    type Raw = sys::ImGuiStyle;

    unsafe fn raw(&self) -> &Self::Raw {
        &self.0
    }

    unsafe fn raw_mut(&mut self) -> &mut Self::Raw {
        &mut self.0
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

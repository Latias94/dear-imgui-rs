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
use crate::widget::{TableFlags, TableRowFlags};
use crate::window::WindowFlags;
use crate::Context;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::cell::UnsafeCell;

/// User interface style/colors
///
/// Note: This is a transparent wrapper over `sys::ImGuiStyle` (v1.92+ layout).
/// Do not assume field layout here; use accessors or `raw()/raw_mut()` if needed.
#[repr(transparent)]
#[derive(Debug)]
pub struct Style(pub(crate) UnsafeCell<sys::ImGuiStyle>);

// Ensure the wrapper stays layout-compatible with the sys bindings.
const _: [(); std::mem::size_of::<sys::ImGuiStyle>()] = [(); std::mem::size_of::<Style>()];
const _: [(); std::mem::align_of::<sys::ImGuiStyle>()] = [(); std::mem::align_of::<Style>()];

impl Style {
    #[inline]
    fn inner(&self) -> &sys::ImGuiStyle {
        // Safety: `Style` is a view into ImGui-owned style data. Dear ImGui can update style state
        // (e.g. via push/pop stacks or user code) while Rust holds `&Style`, so we store it behind
        // `UnsafeCell` to make that interior mutability explicit.
        unsafe { &*self.0.get() }
    }

    #[inline]
    fn inner_mut(&mut self) -> &mut sys::ImGuiStyle {
        // Safety: caller has `&mut Style`, so this is a unique Rust borrow for this wrapper.
        unsafe { &mut *self.0.get() }
    }

    /// Scales all sizes in the style
    pub fn scale_all_sizes(&mut self, scale_factor: f32) {
        unsafe {
            sys::ImGuiStyle_ScaleAllSizes(self.inner_mut(), scale_factor);
        }
    }

    /// Get a color by style color identifier
    pub fn color(&self, color: StyleColor) -> [f32; 4] {
        let c = self.inner().Colors[color as usize];
        [c.x, c.y, c.z, c.w]
    }

    /// Set a color by style color identifier
    pub fn set_color(&mut self, color: StyleColor, value: [f32; 4]) {
        self.inner_mut().Colors[color as usize] = sys::ImVec4 {
            x: value[0],
            y: value[1],
            z: value[2],
            w: value[3],
        };
    }

    /// Get main font scale (formerly io.FontGlobalScale)
    pub fn font_scale_main(&self) -> f32 {
        self.inner().FontScaleMain
    }

    /// Set main font scale (formerly io.FontGlobalScale)
    pub fn set_font_scale_main(&mut self, scale: f32) {
        self.inner_mut().FontScaleMain = scale;
    }

    /// Get DPI font scale (auto-overwritten if ConfigDpiScaleFonts=true)
    pub fn font_scale_dpi(&self) -> f32 {
        self.inner().FontScaleDpi
    }

    /// Set DPI font scale
    pub fn set_font_scale_dpi(&mut self, scale: f32) {
        self.inner_mut().FontScaleDpi = scale;
    }

    /// Base size used by style for font sizing
    pub fn font_size_base(&self) -> f32 {
        self.inner().FontSizeBase
    }

    pub fn set_font_size_base(&mut self, sz: f32) {
        self.inner_mut().FontSizeBase = sz;
    }

    // Common style accessors (typed, convenient)

    pub fn alpha(&self) -> f32 {
        self.inner().Alpha
    }
    pub fn set_alpha(&mut self, v: f32) {
        self.inner_mut().Alpha = v;
    }

    pub fn disabled_alpha(&self) -> f32 {
        self.inner().DisabledAlpha
    }
    pub fn set_disabled_alpha(&mut self, v: f32) {
        self.inner_mut().DisabledAlpha = v;
    }

    pub fn window_padding(&self) -> [f32; 2] {
        [self.inner().WindowPadding.x, self.inner().WindowPadding.y]
    }
    pub fn set_window_padding(&mut self, v: [f32; 2]) {
        self.inner_mut().WindowPadding = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn window_rounding(&self) -> f32 {
        self.inner().WindowRounding
    }
    pub fn set_window_rounding(&mut self, v: f32) {
        self.inner_mut().WindowRounding = v;
    }

    pub fn window_border_size(&self) -> f32 {
        self.inner().WindowBorderSize
    }
    pub fn set_window_border_size(&mut self, v: f32) {
        self.inner_mut().WindowBorderSize = v;
    }

    pub fn window_min_size(&self) -> [f32; 2] {
        [self.inner().WindowMinSize.x, self.inner().WindowMinSize.y]
    }
    pub fn set_window_min_size(&mut self, v: [f32; 2]) {
        self.inner_mut().WindowMinSize = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn window_title_align(&self) -> [f32; 2] {
        [
            self.inner().WindowTitleAlign.x,
            self.inner().WindowTitleAlign.y,
        ]
    }
    pub fn set_window_title_align(&mut self, v: [f32; 2]) {
        self.inner_mut().WindowTitleAlign = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn window_menu_button_position(&self) -> Direction {
        Direction::from(self.inner().WindowMenuButtonPosition)
    }
    pub fn set_window_menu_button_position(&mut self, d: Direction) {
        self.inner_mut().WindowMenuButtonPosition = d.into();
    }

    pub fn child_rounding(&self) -> f32 {
        self.inner().ChildRounding
    }
    pub fn set_child_rounding(&mut self, v: f32) {
        self.inner_mut().ChildRounding = v;
    }

    pub fn child_border_size(&self) -> f32 {
        self.inner().ChildBorderSize
    }
    pub fn set_child_border_size(&mut self, v: f32) {
        self.inner_mut().ChildBorderSize = v;
    }

    pub fn popup_rounding(&self) -> f32 {
        self.inner().PopupRounding
    }
    pub fn set_popup_rounding(&mut self, v: f32) {
        self.inner_mut().PopupRounding = v;
    }

    pub fn popup_border_size(&self) -> f32 {
        self.inner().PopupBorderSize
    }
    pub fn set_popup_border_size(&mut self, v: f32) {
        self.inner_mut().PopupBorderSize = v;
    }

    pub fn frame_padding(&self) -> [f32; 2] {
        [self.inner().FramePadding.x, self.inner().FramePadding.y]
    }
    pub fn set_frame_padding(&mut self, v: [f32; 2]) {
        self.inner_mut().FramePadding = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn frame_rounding(&self) -> f32 {
        self.inner().FrameRounding
    }
    pub fn set_frame_rounding(&mut self, v: f32) {
        self.inner_mut().FrameRounding = v;
    }

    pub fn frame_border_size(&self) -> f32 {
        self.inner().FrameBorderSize
    }
    pub fn set_frame_border_size(&mut self, v: f32) {
        self.inner_mut().FrameBorderSize = v;
    }

    pub fn item_spacing(&self) -> [f32; 2] {
        [self.inner().ItemSpacing.x, self.inner().ItemSpacing.y]
    }
    pub fn set_item_spacing(&mut self, v: [f32; 2]) {
        self.inner_mut().ItemSpacing = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn item_inner_spacing(&self) -> [f32; 2] {
        [
            self.inner().ItemInnerSpacing.x,
            self.inner().ItemInnerSpacing.y,
        ]
    }
    pub fn set_item_inner_spacing(&mut self, v: [f32; 2]) {
        self.inner_mut().ItemInnerSpacing = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn cell_padding(&self) -> [f32; 2] {
        [self.inner().CellPadding.x, self.inner().CellPadding.y]
    }
    pub fn set_cell_padding(&mut self, v: [f32; 2]) {
        self.inner_mut().CellPadding = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn touch_extra_padding(&self) -> [f32; 2] {
        [
            self.inner().TouchExtraPadding.x,
            self.inner().TouchExtraPadding.y,
        ]
    }
    pub fn set_touch_extra_padding(&mut self, v: [f32; 2]) {
        self.inner_mut().TouchExtraPadding = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn indent_spacing(&self) -> f32 {
        self.inner().IndentSpacing
    }
    pub fn set_indent_spacing(&mut self, v: f32) {
        self.inner_mut().IndentSpacing = v;
    }

    pub fn columns_min_spacing(&self) -> f32 {
        self.inner().ColumnsMinSpacing
    }
    pub fn set_columns_min_spacing(&mut self, v: f32) {
        self.inner_mut().ColumnsMinSpacing = v;
    }

    pub fn scrollbar_size(&self) -> f32 {
        self.inner().ScrollbarSize
    }
    pub fn set_scrollbar_size(&mut self, v: f32) {
        self.inner_mut().ScrollbarSize = v;
    }

    pub fn scrollbar_rounding(&self) -> f32 {
        self.inner().ScrollbarRounding
    }
    pub fn set_scrollbar_rounding(&mut self, v: f32) {
        self.inner_mut().ScrollbarRounding = v;
    }

    pub fn grab_min_size(&self) -> f32 {
        self.inner().GrabMinSize
    }
    pub fn set_grab_min_size(&mut self, v: f32) {
        self.inner_mut().GrabMinSize = v;
    }

    pub fn grab_rounding(&self) -> f32 {
        self.inner().GrabRounding
    }
    pub fn set_grab_rounding(&mut self, v: f32) {
        self.inner_mut().GrabRounding = v;
    }

    pub fn log_slider_deadzone(&self) -> f32 {
        self.inner().LogSliderDeadzone
    }
    pub fn set_log_slider_deadzone(&mut self, v: f32) {
        self.inner_mut().LogSliderDeadzone = v;
    }

    pub fn tab_rounding(&self) -> f32 {
        self.inner().TabRounding
    }
    pub fn set_tab_rounding(&mut self, v: f32) {
        self.inner_mut().TabRounding = v;
    }

    pub fn tab_border_size(&self) -> f32 {
        self.inner().TabBorderSize
    }
    pub fn set_tab_border_size(&mut self, v: f32) {
        self.inner_mut().TabBorderSize = v;
    }

    pub fn color_button_position(&self) -> Direction {
        Direction::from(self.inner().ColorButtonPosition)
    }
    pub fn set_color_button_position(&mut self, d: Direction) {
        self.inner_mut().ColorButtonPosition = d.into();
    }

    pub fn button_text_align(&self) -> [f32; 2] {
        [
            self.inner().ButtonTextAlign.x,
            self.inner().ButtonTextAlign.y,
        ]
    }
    pub fn set_button_text_align(&mut self, v: [f32; 2]) {
        self.inner_mut().ButtonTextAlign = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn selectable_text_align(&self) -> [f32; 2] {
        [
            self.inner().SelectableTextAlign.x,
            self.inner().SelectableTextAlign.y,
        ]
    }
    pub fn set_selectable_text_align(&mut self, v: [f32; 2]) {
        self.inner_mut().SelectableTextAlign = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn display_window_padding(&self) -> [f32; 2] {
        [
            self.inner().DisplayWindowPadding.x,
            self.inner().DisplayWindowPadding.y,
        ]
    }
    pub fn set_display_window_padding(&mut self, v: [f32; 2]) {
        self.inner_mut().DisplayWindowPadding = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn display_safe_area_padding(&self) -> [f32; 2] {
        [
            self.inner().DisplaySafeAreaPadding.x,
            self.inner().DisplaySafeAreaPadding.y,
        ]
    }
    pub fn set_display_safe_area_padding(&mut self, v: [f32; 2]) {
        self.inner_mut().DisplaySafeAreaPadding = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn mouse_cursor_scale(&self) -> f32 {
        self.inner().MouseCursorScale
    }
    pub fn set_mouse_cursor_scale(&mut self, v: f32) {
        self.inner_mut().MouseCursorScale = v;
    }

    pub fn anti_aliased_lines(&self) -> bool {
        self.inner().AntiAliasedLines
    }
    pub fn set_anti_aliased_lines(&mut self, v: bool) {
        self.inner_mut().AntiAliasedLines = v;
    }

    pub fn anti_aliased_lines_use_tex(&self) -> bool {
        self.inner().AntiAliasedLinesUseTex
    }
    pub fn set_anti_aliased_lines_use_tex(&mut self, v: bool) {
        self.inner_mut().AntiAliasedLinesUseTex = v;
    }

    pub fn anti_aliased_fill(&self) -> bool {
        self.inner().AntiAliasedFill
    }
    pub fn set_anti_aliased_fill(&mut self, v: bool) {
        self.inner_mut().AntiAliasedFill = v;
    }

    pub fn curve_tessellation_tol(&self) -> f32 {
        self.inner().CurveTessellationTol
    }
    pub fn set_curve_tessellation_tol(&mut self, v: f32) {
        self.inner_mut().CurveTessellationTol = v;
    }

    pub fn circle_tessellation_max_error(&self) -> f32 {
        self.inner().CircleTessellationMaxError
    }
    pub fn set_circle_tessellation_max_error(&mut self, v: f32) {
        self.inner_mut().CircleTessellationMaxError = v;
    }

    // Newly exposed 1.92+ or less-common fields

    pub fn window_border_hover_padding(&self) -> f32 {
        self.inner().WindowBorderHoverPadding
    }
    pub fn set_window_border_hover_padding(&mut self, v: f32) {
        self.inner_mut().WindowBorderHoverPadding = v;
    }

    pub fn scrollbar_padding(&self) -> f32 {
        self.inner().ScrollbarPadding
    }
    pub fn set_scrollbar_padding(&mut self, v: f32) {
        self.inner_mut().ScrollbarPadding = v;
    }

    pub fn image_border_size(&self) -> f32 {
        self.inner().ImageBorderSize
    }
    pub fn set_image_border_size(&mut self, v: f32) {
        self.inner_mut().ImageBorderSize = v;
    }

    pub fn tab_min_width_base(&self) -> f32 {
        self.inner().TabMinWidthBase
    }
    pub fn set_tab_min_width_base(&mut self, v: f32) {
        self.inner_mut().TabMinWidthBase = v;
    }

    pub fn tab_min_width_shrink(&self) -> f32 {
        self.inner().TabMinWidthShrink
    }
    pub fn set_tab_min_width_shrink(&mut self, v: f32) {
        self.inner_mut().TabMinWidthShrink = v;
    }

    pub fn tab_close_button_min_width_selected(&self) -> f32 {
        self.inner().TabCloseButtonMinWidthSelected
    }
    pub fn set_tab_close_button_min_width_selected(&mut self, v: f32) {
        self.inner_mut().TabCloseButtonMinWidthSelected = v;
    }

    pub fn tab_close_button_min_width_unselected(&self) -> f32 {
        self.inner().TabCloseButtonMinWidthUnselected
    }
    pub fn set_tab_close_button_min_width_unselected(&mut self, v: f32) {
        self.inner_mut().TabCloseButtonMinWidthUnselected = v;
    }

    pub fn tab_bar_border_size(&self) -> f32 {
        self.inner().TabBarBorderSize
    }
    pub fn set_tab_bar_border_size(&mut self, v: f32) {
        self.inner_mut().TabBarBorderSize = v;
    }

    pub fn tab_bar_overline_size(&self) -> f32 {
        self.inner().TabBarOverlineSize
    }
    pub fn set_tab_bar_overline_size(&mut self, v: f32) {
        self.inner_mut().TabBarOverlineSize = v;
    }

    pub fn table_angled_headers_angle(&self) -> f32 {
        self.inner().TableAngledHeadersAngle
    }
    pub fn set_table_angled_headers_angle(&mut self, v: f32) {
        self.inner_mut().TableAngledHeadersAngle = v;
    }

    pub fn table_angled_headers_text_align(&self) -> [f32; 2] {
        [
            self.inner().TableAngledHeadersTextAlign.x,
            self.inner().TableAngledHeadersTextAlign.y,
        ]
    }
    pub fn set_table_angled_headers_text_align(&mut self, v: [f32; 2]) {
        self.inner_mut().TableAngledHeadersTextAlign = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn tree_lines_flags(&self) -> TreeNodeFlags {
        TreeNodeFlags::from_bits_truncate(self.inner().TreeLinesFlags as i32)
    }
    pub fn set_tree_lines_flags(&mut self, flags: TreeNodeFlags) {
        self.inner_mut().TreeLinesFlags = flags.bits() as sys::ImGuiTreeNodeFlags;
    }

    pub fn tree_lines_size(&self) -> f32 {
        self.inner().TreeLinesSize
    }
    pub fn set_tree_lines_size(&mut self, v: f32) {
        self.inner_mut().TreeLinesSize = v;
    }

    pub fn tree_lines_rounding(&self) -> f32 {
        self.inner().TreeLinesRounding
    }
    pub fn set_tree_lines_rounding(&mut self, v: f32) {
        self.inner_mut().TreeLinesRounding = v;
    }

    pub fn separator_text_border_size(&self) -> f32 {
        self.inner().SeparatorTextBorderSize
    }
    pub fn set_separator_text_border_size(&mut self, v: f32) {
        self.inner_mut().SeparatorTextBorderSize = v;
    }

    pub fn separator_text_align(&self) -> [f32; 2] {
        [
            self.inner().SeparatorTextAlign.x,
            self.inner().SeparatorTextAlign.y,
        ]
    }
    pub fn set_separator_text_align(&mut self, v: [f32; 2]) {
        self.inner_mut().SeparatorTextAlign = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn separator_text_padding(&self) -> [f32; 2] {
        [
            self.inner().SeparatorTextPadding.x,
            self.inner().SeparatorTextPadding.y,
        ]
    }
    pub fn set_separator_text_padding(&mut self, v: [f32; 2]) {
        self.inner_mut().SeparatorTextPadding = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn docking_node_has_close_button(&self) -> bool {
        self.inner().DockingNodeHasCloseButton
    }
    pub fn set_docking_node_has_close_button(&mut self, v: bool) {
        self.inner_mut().DockingNodeHasCloseButton = v;
    }

    pub fn docking_separator_size(&self) -> f32 {
        self.inner().DockingSeparatorSize
    }
    pub fn set_docking_separator_size(&mut self, v: f32) {
        self.inner_mut().DockingSeparatorSize = v;
    }

    pub fn hover_stationary_delay(&self) -> f32 {
        self.inner().HoverStationaryDelay
    }
    pub fn set_hover_stationary_delay(&mut self, v: f32) {
        self.inner_mut().HoverStationaryDelay = v;
    }

    pub fn hover_delay_short(&self) -> f32 {
        self.inner().HoverDelayShort
    }
    pub fn set_hover_delay_short(&mut self, v: f32) {
        self.inner_mut().HoverDelayShort = v;
    }

    pub fn hover_delay_normal(&self) -> f32 {
        self.inner().HoverDelayNormal
    }
    pub fn set_hover_delay_normal(&mut self, v: f32) {
        self.inner_mut().HoverDelayNormal = v;
    }

    pub fn hover_flags_for_tooltip_mouse(&self) -> HoveredFlags {
        HoveredFlags::from_bits_truncate(self.inner().HoverFlagsForTooltipMouse as i32)
    }
    pub fn set_hover_flags_for_tooltip_mouse(&mut self, flags: HoveredFlags) {
        self.inner_mut().HoverFlagsForTooltipMouse = flags.bits() as sys::ImGuiHoveredFlags;
    }

    pub fn hover_flags_for_tooltip_nav(&self) -> HoveredFlags {
        HoveredFlags::from_bits_truncate(self.inner().HoverFlagsForTooltipNav as i32)
    }
    pub fn set_hover_flags_for_tooltip_nav(&mut self, flags: HoveredFlags) {
        self.inner_mut().HoverFlagsForTooltipNav = flags.bits() as sys::ImGuiHoveredFlags;
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
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
    DragDropTargetBg = sys::ImGuiCol_DragDropTargetBg as i32,
    UnsavedMarker = sys::ImGuiCol_UnsavedMarker as i32,
    NavCursor = sys::ImGuiCol_NavCursor as i32,
    NavWindowingHighlight = sys::ImGuiCol_NavWindowingHighlight as i32,
    NavWindowingDimBg = sys::ImGuiCol_NavWindowingDimBg as i32,
    ModalWindowDimBg = sys::ImGuiCol_ModalWindowDimBg as i32,
}

impl StyleColor {
    pub const COUNT: usize = sys::ImGuiCol_COUNT as usize;
}

impl Clone for Style {
    fn clone(&self) -> Self {
        Self(UnsafeCell::new(*self.inner()))
    }
}

impl PartialEq for Style {
    fn eq(&self, other: &Self) -> bool {
        *self.inner() == *other.inner()
    }
}

impl RawWrapper for Style {
    type Raw = sys::ImGuiStyle;

    unsafe fn raw(&self) -> &Self::Raw {
        self.inner()
    }

    unsafe fn raw_mut(&mut self) -> &mut Self::Raw {
        self.inner_mut()
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

/// Which base preset to start from when applying a [`Theme`].
///
/// This controls which built-in Dear ImGui color set is used as a starting
/// point before applying any overrides.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ThemePreset {
    /// Do not touch existing style colors; only apply explicit overrides.
    None,
    /// Use Dear ImGui's built-in dark preset.
    Dark,
    /// Use Dear ImGui's built-in light preset.
    Light,
    /// Use Dear ImGui's classic preset.
    Classic,
}

impl Default for ThemePreset {
    fn default() -> Self {
        ThemePreset::None
    }
}

/// A single color override for a given [`StyleColor`] entry.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct ColorOverride {
    /// Target style color to override.
    pub id: StyleColor,
    /// New RGBA color (0.0-1.0 range) to apply.
    pub rgba: [f32; 4],
}

/// High-level style tweaks that can be applied on top of a preset.
///
/// This does not expose the full `ImGuiStyle` surface, only the most commonly
/// themed fields. All fields are optional; `None` means "leave unchanged".
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct StyleTweaks {
    pub window_rounding: Option<f32>,
    pub frame_rounding: Option<f32>,
    pub tab_rounding: Option<f32>,

    pub window_padding: Option<[f32; 2]>,
    pub frame_padding: Option<[f32; 2]>,
    pub cell_padding: Option<[f32; 2]>,
    pub item_spacing: Option<[f32; 2]>,
    pub item_inner_spacing: Option<[f32; 2]>,

    pub scrollbar_size: Option<f32>,
    pub grab_min_size: Option<f32>,

    pub indent_spacing: Option<f32>,
    pub scrollbar_rounding: Option<f32>,
    pub grab_rounding: Option<f32>,
    pub window_border_size: Option<f32>,
    pub child_border_size: Option<f32>,
    pub popup_border_size: Option<f32>,
    pub frame_border_size: Option<f32>,
    pub tab_border_size: Option<f32>,
    pub child_rounding: Option<f32>,
    pub popup_rounding: Option<f32>,

    pub anti_aliased_lines: Option<bool>,
    pub anti_aliased_fill: Option<bool>,
}

impl Default for StyleTweaks {
    fn default() -> Self {
        Self {
            window_rounding: None,
            frame_rounding: None,
            tab_rounding: None,
            window_padding: None,
            frame_padding: None,
            cell_padding: None,
            item_spacing: None,
            item_inner_spacing: None,
            scrollbar_size: None,
            grab_min_size: None,
            indent_spacing: None,
            scrollbar_rounding: None,
            grab_rounding: None,
            window_border_size: None,
            child_border_size: None,
            popup_border_size: None,
            frame_border_size: None,
            tab_border_size: None,
            child_rounding: None,
            popup_rounding: None,
            anti_aliased_lines: None,
            anti_aliased_fill: None,
        }
    }
}

impl StyleTweaks {
    /// Apply these tweaks to the given style.
    pub fn apply(&self, style: &mut Style) {
        if let Some(v) = self.window_rounding {
            style.set_window_rounding(v);
        }
        if let Some(v) = self.frame_rounding {
            style.set_frame_rounding(v);
        }
        if let Some(v) = self.tab_rounding {
            style.set_tab_rounding(v);
        }

        if let Some(v) = self.window_padding {
            style.set_window_padding(v);
        }
        if let Some(v) = self.frame_padding {
            style.set_frame_padding(v);
        }
        if let Some(v) = self.cell_padding {
            style.set_cell_padding(v);
        }
        if let Some(v) = self.item_spacing {
            style.set_item_spacing(v);
        }
        if let Some(v) = self.item_inner_spacing {
            style.set_item_inner_spacing(v);
        }

        if let Some(v) = self.scrollbar_size {
            style.set_scrollbar_size(v);
        }
        if let Some(v) = self.grab_min_size {
            style.set_grab_min_size(v);
        }

        if let Some(v) = self.indent_spacing {
            style.set_indent_spacing(v);
        }
        if let Some(v) = self.scrollbar_rounding {
            style.set_scrollbar_rounding(v);
        }
        if let Some(v) = self.grab_rounding {
            style.set_grab_rounding(v);
        }
        if let Some(v) = self.window_border_size {
            style.set_window_border_size(v);
        }
        if let Some(v) = self.child_border_size {
            style.set_child_border_size(v);
        }
        if let Some(v) = self.popup_border_size {
            style.set_popup_border_size(v);
        }
        if let Some(v) = self.frame_border_size {
            style.set_frame_border_size(v);
        }
        if let Some(v) = self.tab_border_size {
            style.set_tab_border_size(v);
        }
        if let Some(v) = self.child_rounding {
            style.set_child_rounding(v);
        }
        if let Some(v) = self.popup_rounding {
            style.set_popup_rounding(v);
        }

        if let Some(v) = self.anti_aliased_lines {
            style.set_anti_aliased_lines(v);
        }
        if let Some(v) = self.anti_aliased_fill {
            style.set_anti_aliased_fill(v);
        }
    }
}

/// Window-related theme defaults (flags/behavior).
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct WindowTheme {
    /// Default flags for top-level windows.
    pub default_window_flags: Option<WindowFlags>,
    /// Default flags for popups/modals.
    pub popup_window_flags: Option<WindowFlags>,
}

/// Table-related theme defaults (flags/behavior).
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TableTheme {
    /// Default flags for tables created via `Ui::table` / `Ui::begin_table`.
    pub default_table_flags: Option<TableFlags>,
    /// Default row flags for data tables.
    pub default_row_flags: Option<TableRowFlags>,
}

/// High-level theme configuration for Dear ImGui.
///
/// A theme is applied in three stages:
/// 1) Choose a base preset (`Dark`/`Light`/`Classic` or `None`).
/// 2) Apply any explicit color overrides.
/// 3) Apply a small set of style tweaks.
///
/// Window/table defaults are provided as data and can be used by higher-level
/// helpers when building windows and tables.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Theme {
    /// Base preset to start from, before applying overrides.
    #[cfg_attr(feature = "serde", serde(default))]
    pub preset: ThemePreset,

    /// Color overrides on top of the preset.
    #[cfg_attr(feature = "serde", serde(default))]
    pub colors: Vec<ColorOverride>,

    /// Optional style tweaks on top of the preset.
    #[cfg_attr(feature = "serde", serde(default))]
    pub style: StyleTweaks,

    /// Window-related defaults (flags/behavior).
    #[cfg_attr(feature = "serde", serde(default))]
    pub windows: WindowTheme,

    /// Table-related defaults (flags/behavior).
    #[cfg_attr(feature = "serde", serde(default))]
    pub tables: TableTheme,
}

impl Theme {
    /// Apply this theme to a given style.
    ///
    /// This does not touch fonts or IO; it only updates `ImGuiStyle`.
    pub fn apply_to_style(&self, style: &mut Style) {
        // 1) Base preset
        match self.preset {
            ThemePreset::None => {}
            ThemePreset::Dark => unsafe {
                sys::igStyleColorsDark(style.raw_mut());
            },
            ThemePreset::Light => unsafe {
                sys::igStyleColorsLight(style.raw_mut());
            },
            ThemePreset::Classic => unsafe {
                sys::igStyleColorsClassic(style.raw_mut());
            },
        }

        // 2) Color overrides
        for c in &self.colors {
            style.set_color(c.id, c.rgba);
        }

        // 3) Common style tweaks
        self.style.apply(style);
    }

    /// Apply this theme to the given context (current style).
    pub fn apply_to_context(&self, ctx: &mut Context) {
        let style = ctx.style_mut();
        self.apply_to_style(style);
    }
}

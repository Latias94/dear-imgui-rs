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
mod theme;
mod var;

pub use theme::{ColorOverride, StyleTweaks, TableTheme, Theme, ThemePreset, WindowTheme};
pub use var::{StyleVar, StyleVarVec2};

use std::cell::UnsafeCell;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::internal::RawWrapper;
use crate::sys;
use crate::utils::{TooltipHoveredFlags, validate_tooltip_hovered_flags};
use crate::validation::{assert_finite_f32, assert_finite_vec2, assert_finite_vec4};

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

impl From<sys::ImGuiDir> for Direction {
    fn from(d: sys::ImGuiDir) -> Self {
        match d {
            x if x == sys::ImGuiDir_Left => Direction::Left,
            x if x == sys::ImGuiDir_Right => Direction::Right,
            x if x == sys::ImGuiDir_Up => Direction::Up,
            x if x == sys::ImGuiDir_Down => Direction::Down,
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
    CheckboxSelectedBg = sys::ImGuiCol_CheckboxSelectedBg,
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
    TextLink = sys::ImGuiCol_TextLink,
    TreeLines = sys::ImGuiCol_TreeLines,
    InputTextCursor = sys::ImGuiCol_InputTextCursor,
    DragDropTarget = sys::ImGuiCol_DragDropTarget,
    DragDropTargetBg = sys::ImGuiCol_DragDropTargetBg,
    UnsavedMarker = sys::ImGuiCol_UnsavedMarker,
    NavCursor = sys::ImGuiCol_NavCursor,
    NavWindowingHighlight = sys::ImGuiCol_NavWindowingHighlight,
    NavWindowingDimBg = sys::ImGuiCol_NavWindowingDimBg,
    ModalWindowDimBg = sys::ImGuiCol_ModalWindowDimBg,
}

impl StyleColor {
    pub const COUNT: usize = sys::ImGuiCol_COUNT as usize;
}

/// Tree hierarchy guide-line drawing mode stored in `ImGuiStyle::TreeLinesFlags`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TreeLineMode(i32);

impl TreeLineMode {
    /// No tree hierarchy guide lines are drawn.
    pub const NONE: Self = Self(sys::ImGuiTreeNodeFlags_DrawLinesNone);
    /// Draw full tree hierarchy guide lines.
    pub const FULL: Self = Self(sys::ImGuiTreeNodeFlags_DrawLinesFull);
    /// Draw tree hierarchy guide lines only to nodes.
    pub const TO_NODES: Self = Self(sys::ImGuiTreeNodeFlags_DrawLinesToNodes);

    #[inline]
    pub const fn bits(self) -> i32 {
        self.0
    }

    #[inline]
    pub(crate) const fn from_bits_retain(bits: i32) -> Self {
        Self(bits)
    }

    #[inline]
    pub(crate) fn validate(self, caller: &str) {
        assert!(
            matches!(self, Self::NONE | Self::FULL | Self::TO_NODES),
            "{caller} accepts only TreeLineMode::NONE, FULL, or TO_NODES"
        );
    }
}

// Hovered flag domain types are defined in utils.rs and re-exported at crate root.

const TABLE_ANGLED_HEADERS_MAX_ANGLE: f32 = 50.0 * std::f32::consts::PI / 180.0;

pub(crate) fn validate_style_color(caller: &str, name: &str, value: [f32; 4]) {
    assert_finite_vec4(caller, name, value);
}

fn assert_positive_f32(caller: &str, name: &str, value: f32) {
    assert_finite_f32(caller, name, value);
    assert!(value > 0.0, "{caller} {name} must be positive");
}

fn assert_non_negative_f32(caller: &str, name: &str, value: f32) {
    assert_finite_f32(caller, name, value);
    assert!(value >= 0.0, "{caller} {name} must be non-negative");
}

fn assert_non_negative_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert_finite_vec2(caller, name, value);
    assert!(
        value[0] >= 0.0 && value[1] >= 0.0,
        "{caller} {name} must contain non-negative values"
    );
}

fn assert_unit_f32(caller: &str, name: &str, value: f32) {
    assert_finite_f32(caller, name, value);
    assert!(
        (0.0..=1.0).contains(&value),
        "{caller} {name} must be between 0.0 and 1.0"
    );
}

fn assert_unit_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert_finite_vec2(caller, name, value);
    assert!(
        (0.0..=1.0).contains(&value[0]) && (0.0..=1.0).contains(&value[1]),
        "{caller} {name} must contain values between 0.0 and 1.0"
    );
}

fn assert_window_min_size(caller: &str, value: [f32; 2]) {
    assert_finite_vec2(caller, "value", value);
    assert!(
        value[0] >= 1.0 && value[1] >= 1.0,
        "{caller} value must contain values greater than or equal to 1.0"
    );
}

fn assert_tab_close_button_min_width(caller: &str, value: f32) {
    assert_finite_f32(caller, "value", value);
    assert!(
        value >= 0.0 || value == -1.0,
        "{caller} value must be non-negative, or -1.0 to always show the close button"
    );
}

fn assert_table_angled_headers_angle(caller: &str, value: f32) {
    assert_finite_f32(caller, "value", value);
    assert!(
        (-TABLE_ANGLED_HEADERS_MAX_ANGLE..=TABLE_ANGLED_HEADERS_MAX_ANGLE).contains(&value),
        "{caller} value must be between -50 and 50 degrees in radians"
    );
}

fn validate_window_menu_button_position(caller: &str, direction: Direction) {
    assert!(
        matches!(
            direction,
            Direction::None | Direction::Left | Direction::Right
        ),
        "{caller} accepts only Direction::None, Direction::Left, or Direction::Right"
    );
}

fn validate_color_button_position(caller: &str, direction: Direction) {
    assert!(
        matches!(direction, Direction::Left | Direction::Right),
        "{caller} accepts only Direction::Left or Direction::Right"
    );
}

pub(crate) fn validate_style_var(caller: &str, style_var: StyleVar) {
    use StyleVar::*;

    match style_var {
        Alpha(value) => assert_unit_f32(caller, "Alpha", value),
        DisabledAlpha(value) => assert_unit_f32(caller, "DisabledAlpha", value),
        WindowPadding(value) => assert_non_negative_vec2(caller, "WindowPadding", value),
        WindowRounding(value) => assert_non_negative_f32(caller, "WindowRounding", value),
        WindowBorderSize(value) => assert_non_negative_f32(caller, "WindowBorderSize", value),
        WindowMinSize(value) => assert_non_negative_vec2(caller, "WindowMinSize", value),
        WindowTitleAlign(value) => assert_unit_vec2(caller, "WindowTitleAlign", value),
        ChildRounding(value) => assert_non_negative_f32(caller, "ChildRounding", value),
        ChildBorderSize(value) => assert_non_negative_f32(caller, "ChildBorderSize", value),
        PopupRounding(value) => assert_non_negative_f32(caller, "PopupRounding", value),
        PopupBorderSize(value) => assert_non_negative_f32(caller, "PopupBorderSize", value),
        FramePadding(value) => assert_non_negative_vec2(caller, "FramePadding", value),
        FrameRounding(value) => assert_non_negative_f32(caller, "FrameRounding", value),
        ImageRounding(value) => assert_non_negative_f32(caller, "ImageRounding", value),
        ImageBorderSize(value) => assert_non_negative_f32(caller, "ImageBorderSize", value),
        FrameBorderSize(value) => assert_non_negative_f32(caller, "FrameBorderSize", value),
        ItemSpacing(value) => assert_non_negative_vec2(caller, "ItemSpacing", value),
        ItemInnerSpacing(value) => assert_non_negative_vec2(caller, "ItemInnerSpacing", value),
        IndentSpacing(value) => assert_non_negative_f32(caller, "IndentSpacing", value),
        CellPadding(value) => assert_non_negative_vec2(caller, "CellPadding", value),
        ScrollbarSize(value) => assert_non_negative_f32(caller, "ScrollbarSize", value),
        ScrollbarRounding(value) => assert_non_negative_f32(caller, "ScrollbarRounding", value),
        ScrollbarPadding(value) => assert_non_negative_f32(caller, "ScrollbarPadding", value),
        GrabMinSize(value) => assert_non_negative_f32(caller, "GrabMinSize", value),
        GrabRounding(value) => assert_non_negative_f32(caller, "GrabRounding", value),
        TabRounding(value) => assert_non_negative_f32(caller, "TabRounding", value),
        TabBorderSize(value) => assert_non_negative_f32(caller, "TabBorderSize", value),
        TabMinWidthBase(value) => assert_non_negative_f32(caller, "TabMinWidthBase", value),
        TabMinWidthShrink(value) => assert_non_negative_f32(caller, "TabMinWidthShrink", value),
        TabBarBorderSize(value) => assert_non_negative_f32(caller, "TabBarBorderSize", value),
        TabBarOverlineSize(value) => assert_non_negative_f32(caller, "TabBarOverlineSize", value),
        TableAngledHeadersAngle(value) => assert_table_angled_headers_angle(caller, value),
        TableAngledHeadersTextAlign(value) => {
            assert_unit_vec2(caller, "TableAngledHeadersTextAlign", value);
        }
        TreeLinesSize(value) => assert_non_negative_f32(caller, "TreeLinesSize", value),
        TreeLinesRounding(value) => assert_non_negative_f32(caller, "TreeLinesRounding", value),
        MenuItemRounding(value) => assert_non_negative_f32(caller, "MenuItemRounding", value),
        SelectableRounding(value) => assert_non_negative_f32(caller, "SelectableRounding", value),
        DragDropTargetRounding(value) => assert_finite_f32(caller, "DragDropTargetRounding", value),
        ButtonTextAlign(value) => assert_unit_vec2(caller, "ButtonTextAlign", value),
        SelectableTextAlign(value) => assert_unit_vec2(caller, "SelectableTextAlign", value),
        SeparatorSize(value) => assert_non_negative_f32(caller, "SeparatorSize", value),
        SeparatorTextBorderSize(value) => {
            assert_non_negative_f32(caller, "SeparatorTextBorderSize", value);
        }
        SeparatorTextAlign(value) => assert_unit_vec2(caller, "SeparatorTextAlign", value),
        SeparatorTextPadding(value) => {
            assert_non_negative_vec2(caller, "SeparatorTextPadding", value);
        }
        DockingSeparatorSize(value) => {
            assert_non_negative_f32(caller, "DockingSeparatorSize", value);
        }
    }
}

pub(crate) fn validate_style_var_component(caller: &str, style_var: StyleVarVec2, value: f32) {
    use StyleVarVec2::*;

    match style_var {
        WindowTitleAlign
        | TableAngledHeadersTextAlign
        | ButtonTextAlign
        | SelectableTextAlign
        | SeparatorTextAlign => assert_unit_f32(caller, "value", value),
        WindowPadding | WindowMinSize | FramePadding | ItemSpacing | ItemInnerSpacing
        | CellPadding | SeparatorTextPadding => assert_non_negative_f32(caller, "value", value),
    }
}

impl Style {
    /// Scale every size and spacing value using Dear ImGui's DPI-aware rules.
    ///
    /// This also updates Dear ImGui's internal main style scale and preserves
    /// sentinel values used by selected style fields.
    #[doc(alias = "ScaleAllSizes")]
    pub fn scale_all_sizes(&mut self, scale_factor: f32) {
        assert!(
            scale_factor.is_finite() && scale_factor > 0.0,
            "Style::scale_all_sizes() scale_factor must be finite and positive"
        );
        unsafe {
            sys::ImGuiStyle_ScaleAllSizes(self.inner_mut(), scale_factor);
        }
    }

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

    /// Get a color by style color identifier
    pub fn color(&self, color: StyleColor) -> [f32; 4] {
        let c = self.inner().Colors[color as usize];
        [c.x, c.y, c.z, c.w]
    }

    /// Set a color by style color identifier
    pub fn set_color(&mut self, color: StyleColor, value: [f32; 4]) {
        validate_style_color("Style::set_color()", "value", value);
        self.inner_mut().Colors[color as usize] = sys::ImVec4 {
            x: value[0],
            y: value[1],
            z: value[2],
            w: value[3],
        };
    }

    // Common style accessors (typed, convenient)

    pub fn alpha(&self) -> f32 {
        self.inner().Alpha
    }
    pub fn set_alpha(&mut self, v: f32) {
        assert_unit_f32("Style::set_alpha()", "v", v);
        self.inner_mut().Alpha = v;
    }

    pub fn disabled_alpha(&self) -> f32 {
        self.inner().DisabledAlpha
    }
    pub fn set_disabled_alpha(&mut self, v: f32) {
        assert_unit_f32("Style::set_disabled_alpha()", "v", v);
        self.inner_mut().DisabledAlpha = v;
    }

    pub fn window_padding(&self) -> [f32; 2] {
        [self.inner().WindowPadding.x, self.inner().WindowPadding.y]
    }
    pub fn set_window_padding(&mut self, v: [f32; 2]) {
        assert_non_negative_vec2("Style::set_window_padding()", "v", v);
        self.inner_mut().WindowPadding = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn window_rounding(&self) -> f32 {
        self.inner().WindowRounding
    }
    pub fn set_window_rounding(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_window_rounding()", "v", v);
        self.inner_mut().WindowRounding = v;
    }

    pub fn window_border_size(&self) -> f32 {
        self.inner().WindowBorderSize
    }
    pub fn set_window_border_size(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_window_border_size()", "v", v);
        self.inner_mut().WindowBorderSize = v;
    }

    pub fn window_min_size(&self) -> [f32; 2] {
        [self.inner().WindowMinSize.x, self.inner().WindowMinSize.y]
    }
    pub fn set_window_min_size(&mut self, v: [f32; 2]) {
        assert_window_min_size("Style::set_window_min_size()", v);
        self.inner_mut().WindowMinSize = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn window_title_align(&self) -> [f32; 2] {
        [
            self.inner().WindowTitleAlign.x,
            self.inner().WindowTitleAlign.y,
        ]
    }
    pub fn set_window_title_align(&mut self, v: [f32; 2]) {
        assert_unit_vec2("Style::set_window_title_align()", "v", v);
        self.inner_mut().WindowTitleAlign = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn window_menu_button_position(&self) -> Direction {
        Direction::from(self.inner().WindowMenuButtonPosition)
    }
    pub fn set_window_menu_button_position(&mut self, d: Direction) {
        validate_window_menu_button_position("Style::set_window_menu_button_position()", d);
        self.inner_mut().WindowMenuButtonPosition = d.into();
    }

    pub fn child_rounding(&self) -> f32 {
        self.inner().ChildRounding
    }
    pub fn set_child_rounding(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_child_rounding()", "v", v);
        self.inner_mut().ChildRounding = v;
    }

    pub fn child_border_size(&self) -> f32 {
        self.inner().ChildBorderSize
    }
    pub fn set_child_border_size(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_child_border_size()", "v", v);
        self.inner_mut().ChildBorderSize = v;
    }

    pub fn popup_rounding(&self) -> f32 {
        self.inner().PopupRounding
    }
    pub fn set_popup_rounding(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_popup_rounding()", "v", v);
        self.inner_mut().PopupRounding = v;
    }

    pub fn popup_border_size(&self) -> f32 {
        self.inner().PopupBorderSize
    }
    pub fn set_popup_border_size(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_popup_border_size()", "v", v);
        self.inner_mut().PopupBorderSize = v;
    }

    pub fn frame_padding(&self) -> [f32; 2] {
        [self.inner().FramePadding.x, self.inner().FramePadding.y]
    }
    pub fn set_frame_padding(&mut self, v: [f32; 2]) {
        assert_non_negative_vec2("Style::set_frame_padding()", "v", v);
        self.inner_mut().FramePadding = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn frame_rounding(&self) -> f32 {
        self.inner().FrameRounding
    }
    pub fn set_frame_rounding(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_frame_rounding()", "v", v);
        self.inner_mut().FrameRounding = v;
    }

    pub fn image_rounding(&self) -> f32 {
        self.inner().ImageRounding
    }
    pub fn set_image_rounding(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_image_rounding()", "v", v);
        self.inner_mut().ImageRounding = v;
    }

    pub fn frame_border_size(&self) -> f32 {
        self.inner().FrameBorderSize
    }
    pub fn set_frame_border_size(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_frame_border_size()", "v", v);
        self.inner_mut().FrameBorderSize = v;
    }

    pub fn item_spacing(&self) -> [f32; 2] {
        [self.inner().ItemSpacing.x, self.inner().ItemSpacing.y]
    }
    pub fn set_item_spacing(&mut self, v: [f32; 2]) {
        assert_non_negative_vec2("Style::set_item_spacing()", "v", v);
        self.inner_mut().ItemSpacing = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn item_inner_spacing(&self) -> [f32; 2] {
        [
            self.inner().ItemInnerSpacing.x,
            self.inner().ItemInnerSpacing.y,
        ]
    }
    pub fn set_item_inner_spacing(&mut self, v: [f32; 2]) {
        assert_non_negative_vec2("Style::set_item_inner_spacing()", "v", v);
        self.inner_mut().ItemInnerSpacing = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn cell_padding(&self) -> [f32; 2] {
        [self.inner().CellPadding.x, self.inner().CellPadding.y]
    }
    pub fn set_cell_padding(&mut self, v: [f32; 2]) {
        assert_non_negative_vec2("Style::set_cell_padding()", "v", v);
        self.inner_mut().CellPadding = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn touch_extra_padding(&self) -> [f32; 2] {
        [
            self.inner().TouchExtraPadding.x,
            self.inner().TouchExtraPadding.y,
        ]
    }
    pub fn set_touch_extra_padding(&mut self, v: [f32; 2]) {
        assert_non_negative_vec2("Style::set_touch_extra_padding()", "v", v);
        self.inner_mut().TouchExtraPadding = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn indent_spacing(&self) -> f32 {
        self.inner().IndentSpacing
    }
    pub fn set_indent_spacing(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_indent_spacing()", "v", v);
        self.inner_mut().IndentSpacing = v;
    }

    pub fn columns_min_spacing(&self) -> f32 {
        self.inner().ColumnsMinSpacing
    }
    pub fn set_columns_min_spacing(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_columns_min_spacing()", "v", v);
        self.inner_mut().ColumnsMinSpacing = v;
    }

    pub fn scrollbar_size(&self) -> f32 {
        self.inner().ScrollbarSize
    }
    pub fn set_scrollbar_size(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_scrollbar_size()", "v", v);
        self.inner_mut().ScrollbarSize = v;
    }

    pub fn scrollbar_rounding(&self) -> f32 {
        self.inner().ScrollbarRounding
    }
    pub fn set_scrollbar_rounding(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_scrollbar_rounding()", "v", v);
        self.inner_mut().ScrollbarRounding = v;
    }

    pub fn grab_min_size(&self) -> f32 {
        self.inner().GrabMinSize
    }
    pub fn set_grab_min_size(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_grab_min_size()", "v", v);
        self.inner_mut().GrabMinSize = v;
    }

    pub fn grab_rounding(&self) -> f32 {
        self.inner().GrabRounding
    }
    pub fn set_grab_rounding(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_grab_rounding()", "v", v);
        self.inner_mut().GrabRounding = v;
    }

    pub fn log_slider_deadzone(&self) -> f32 {
        self.inner().LogSliderDeadzone
    }
    pub fn set_log_slider_deadzone(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_log_slider_deadzone()", "v", v);
        self.inner_mut().LogSliderDeadzone = v;
    }

    pub fn tab_rounding(&self) -> f32 {
        self.inner().TabRounding
    }
    pub fn set_tab_rounding(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_tab_rounding()", "v", v);
        self.inner_mut().TabRounding = v;
    }

    pub fn tab_border_size(&self) -> f32 {
        self.inner().TabBorderSize
    }
    pub fn set_tab_border_size(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_tab_border_size()", "v", v);
        self.inner_mut().TabBorderSize = v;
    }

    pub fn color_button_position(&self) -> Direction {
        Direction::from(self.inner().ColorButtonPosition)
    }
    pub fn set_color_button_position(&mut self, d: Direction) {
        validate_color_button_position("Style::set_color_button_position()", d);
        self.inner_mut().ColorButtonPosition = d.into();
    }

    pub fn button_text_align(&self) -> [f32; 2] {
        [
            self.inner().ButtonTextAlign.x,
            self.inner().ButtonTextAlign.y,
        ]
    }
    pub fn set_button_text_align(&mut self, v: [f32; 2]) {
        assert_unit_vec2("Style::set_button_text_align()", "v", v);
        self.inner_mut().ButtonTextAlign = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn selectable_text_align(&self) -> [f32; 2] {
        [
            self.inner().SelectableTextAlign.x,
            self.inner().SelectableTextAlign.y,
        ]
    }
    pub fn set_selectable_text_align(&mut self, v: [f32; 2]) {
        assert_unit_vec2("Style::set_selectable_text_align()", "v", v);
        self.inner_mut().SelectableTextAlign = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn display_window_padding(&self) -> [f32; 2] {
        [
            self.inner().DisplayWindowPadding.x,
            self.inner().DisplayWindowPadding.y,
        ]
    }
    pub fn set_display_window_padding(&mut self, v: [f32; 2]) {
        assert_non_negative_vec2("Style::set_display_window_padding()", "v", v);
        self.inner_mut().DisplayWindowPadding = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn display_safe_area_padding(&self) -> [f32; 2] {
        [
            self.inner().DisplaySafeAreaPadding.x,
            self.inner().DisplaySafeAreaPadding.y,
        ]
    }
    pub fn set_display_safe_area_padding(&mut self, v: [f32; 2]) {
        assert_non_negative_vec2("Style::set_display_safe_area_padding()", "v", v);
        self.inner_mut().DisplaySafeAreaPadding = sys::ImVec2 { x: v[0], y: v[1] };
    }

    /// Get main font scale (formerly io.FontGlobalScale)
    pub fn font_scale_main(&self) -> f32 {
        self.inner().FontScaleMain
    }

    /// Set main font scale (formerly io.FontGlobalScale)
    pub fn set_font_scale_main(&mut self, scale: f32) {
        assert_positive_f32("Style::set_font_scale_main()", "scale", scale);
        self.inner_mut().FontScaleMain = scale;
    }

    /// Get DPI font scale (auto-overwritten if ConfigDpiScaleFonts=true)
    pub fn font_scale_dpi(&self) -> f32 {
        self.inner().FontScaleDpi
    }

    /// Set DPI font scale
    pub fn set_font_scale_dpi(&mut self, scale: f32) {
        assert_positive_f32("Style::set_font_scale_dpi()", "scale", scale);
        self.inner_mut().FontScaleDpi = scale;
    }

    /// Base size used by style for font sizing
    pub fn font_size_base(&self) -> f32 {
        self.inner().FontSizeBase
    }

    pub fn set_font_size_base(&mut self, sz: f32) {
        assert_non_negative_f32("Style::set_font_size_base()", "sz", sz);
        self.inner_mut().FontSizeBase = sz;
    }

    pub fn color_marker_size(&self) -> f32 {
        self.inner().ColorMarkerSize
    }
    pub fn set_color_marker_size(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_color_marker_size()", "v", v);
        self.inner_mut().ColorMarkerSize = v;
    }

    pub fn separator_size(&self) -> f32 {
        self.inner().SeparatorSize
    }
    pub fn set_separator_size(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_separator_size()", "v", v);
        self.inner_mut().SeparatorSize = v;
    }

    pub fn separator_text_border_size(&self) -> f32 {
        self.inner().SeparatorTextBorderSize
    }
    pub fn set_separator_text_border_size(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_separator_text_border_size()", "v", v);
        self.inner_mut().SeparatorTextBorderSize = v;
    }

    pub fn separator_text_align(&self) -> [f32; 2] {
        [
            self.inner().SeparatorTextAlign.x,
            self.inner().SeparatorTextAlign.y,
        ]
    }
    pub fn set_separator_text_align(&mut self, v: [f32; 2]) {
        assert_unit_vec2("Style::set_separator_text_align()", "v", v);
        self.inner_mut().SeparatorTextAlign = sys::ImVec2 { x: v[0], y: v[1] };
    }

    pub fn separator_text_padding(&self) -> [f32; 2] {
        [
            self.inner().SeparatorTextPadding.x,
            self.inner().SeparatorTextPadding.y,
        ]
    }
    pub fn set_separator_text_padding(&mut self, v: [f32; 2]) {
        assert_non_negative_vec2("Style::set_separator_text_padding()", "v", v);
        self.inner_mut().SeparatorTextPadding = sys::ImVec2 { x: v[0], y: v[1] };
    }

    // Newly exposed 1.92+ or less-common fields

    pub fn window_border_hover_padding(&self) -> f32 {
        self.inner().WindowBorderHoverPadding
    }
    pub fn set_window_border_hover_padding(&mut self, v: f32) {
        assert_positive_f32("Style::set_window_border_hover_padding()", "v", v);
        self.inner_mut().WindowBorderHoverPadding = v;
    }

    pub fn scrollbar_padding(&self) -> f32 {
        self.inner().ScrollbarPadding
    }
    pub fn set_scrollbar_padding(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_scrollbar_padding()", "v", v);
        self.inner_mut().ScrollbarPadding = v;
    }

    pub fn image_border_size(&self) -> f32 {
        self.inner().ImageBorderSize
    }
    pub fn set_image_border_size(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_image_border_size()", "v", v);
        self.inner_mut().ImageBorderSize = v;
    }

    pub fn tab_min_width_base(&self) -> f32 {
        self.inner().TabMinWidthBase
    }
    pub fn set_tab_min_width_base(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_tab_min_width_base()", "v", v);
        self.inner_mut().TabMinWidthBase = v;
    }

    pub fn tab_min_width_shrink(&self) -> f32 {
        self.inner().TabMinWidthShrink
    }
    pub fn set_tab_min_width_shrink(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_tab_min_width_shrink()", "v", v);
        self.inner_mut().TabMinWidthShrink = v;
    }

    pub fn tab_close_button_min_width_selected(&self) -> f32 {
        self.inner().TabCloseButtonMinWidthSelected
    }
    pub fn set_tab_close_button_min_width_selected(&mut self, v: f32) {
        assert_tab_close_button_min_width("Style::set_tab_close_button_min_width_selected()", v);
        self.inner_mut().TabCloseButtonMinWidthSelected = v;
    }

    pub fn tab_close_button_min_width_unselected(&self) -> f32 {
        self.inner().TabCloseButtonMinWidthUnselected
    }
    pub fn set_tab_close_button_min_width_unselected(&mut self, v: f32) {
        assert_tab_close_button_min_width("Style::set_tab_close_button_min_width_unselected()", v);
        self.inner_mut().TabCloseButtonMinWidthUnselected = v;
    }

    pub fn tab_bar_border_size(&self) -> f32 {
        self.inner().TabBarBorderSize
    }
    pub fn set_tab_bar_border_size(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_tab_bar_border_size()", "v", v);
        self.inner_mut().TabBarBorderSize = v;
    }

    pub fn tab_bar_overline_size(&self) -> f32 {
        self.inner().TabBarOverlineSize
    }
    pub fn set_tab_bar_overline_size(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_tab_bar_overline_size()", "v", v);
        self.inner_mut().TabBarOverlineSize = v;
    }

    pub fn table_angled_headers_angle(&self) -> f32 {
        self.inner().TableAngledHeadersAngle
    }
    pub fn set_table_angled_headers_angle(&mut self, v: f32) {
        assert_table_angled_headers_angle("Style::set_table_angled_headers_angle()", v);
        self.inner_mut().TableAngledHeadersAngle = v;
    }

    pub fn table_angled_headers_text_align(&self) -> [f32; 2] {
        [
            self.inner().TableAngledHeadersTextAlign.x,
            self.inner().TableAngledHeadersTextAlign.y,
        ]
    }
    pub fn set_table_angled_headers_text_align(&mut self, v: [f32; 2]) {
        assert_unit_vec2("Style::set_table_angled_headers_text_align()", "v", v);
        self.inner_mut().TableAngledHeadersTextAlign = sys::ImVec2 { x: v[0], y: v[1] };
    }

    /// Returns the rounding radius of menu items and menus.
    #[doc(alias = "MenuItemRounding")]
    pub fn menu_item_rounding(&self) -> f32 {
        self.inner().MenuItemRounding
    }

    /// Sets the rounding radius of menu items and menus.
    #[doc(alias = "MenuItemRounding")]
    pub fn set_menu_item_rounding(&mut self, value: f32) {
        assert_non_negative_f32("Style::set_menu_item_rounding()", "value", value);
        self.inner_mut().MenuItemRounding = value;
    }

    /// Returns the rounding radius of selectable items.
    #[doc(alias = "SelectableRounding")]
    pub fn selectable_rounding(&self) -> f32 {
        self.inner().SelectableRounding
    }

    /// Sets the rounding radius of selectable items.
    ///
    /// Non-zero values may make contiguous selections look discontinuous.
    #[doc(alias = "SelectableRounding")]
    pub fn set_selectable_rounding(&mut self, value: f32) {
        assert_non_negative_f32("Style::set_selectable_rounding()", "value", value);
        self.inner_mut().SelectableRounding = value;
    }

    pub fn tree_lines_mode(&self) -> TreeLineMode {
        TreeLineMode::from_bits_retain(self.inner().TreeLinesFlags)
    }

    pub fn set_tree_lines_mode(&mut self, mode: TreeLineMode) {
        mode.validate("Style::set_tree_lines_mode()");
        self.inner_mut().TreeLinesFlags = mode.bits() as sys::ImGuiTreeNodeFlags;
    }

    pub fn tree_lines_size(&self) -> f32 {
        self.inner().TreeLinesSize
    }
    pub fn set_tree_lines_size(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_tree_lines_size()", "v", v);
        self.inner_mut().TreeLinesSize = v;
    }

    pub fn tree_lines_rounding(&self) -> f32 {
        self.inner().TreeLinesRounding
    }
    pub fn set_tree_lines_rounding(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_tree_lines_rounding()", "v", v);
        self.inner_mut().TreeLinesRounding = v;
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
        assert_non_negative_f32("Style::set_docking_separator_size()", "v", v);
        self.inner_mut().DockingSeparatorSize = v;
    }

    pub fn drag_drop_target_rounding(&self) -> f32 {
        self.inner().DragDropTargetRounding
    }
    pub fn set_drag_drop_target_rounding(&mut self, v: f32) {
        assert_finite_f32("Style::set_drag_drop_target_rounding()", "v", v);
        self.inner_mut().DragDropTargetRounding = v;
    }

    pub fn drag_drop_target_border_size(&self) -> f32 {
        self.inner().DragDropTargetBorderSize
    }
    pub fn set_drag_drop_target_border_size(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_drag_drop_target_border_size()", "v", v);
        self.inner_mut().DragDropTargetBorderSize = v;
    }

    pub fn drag_drop_target_padding(&self) -> f32 {
        self.inner().DragDropTargetPadding
    }
    pub fn set_drag_drop_target_padding(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_drag_drop_target_padding()", "v", v);
        self.inner_mut().DragDropTargetPadding = v;
    }

    /// Returns the input-text cursor thickness.
    #[doc(alias = "InputTextCursorSize")]
    pub fn input_text_cursor_size(&self) -> f32 {
        self.inner().InputTextCursorSize
    }

    /// Sets the input-text cursor thickness.
    #[doc(alias = "InputTextCursorSize")]
    pub fn set_input_text_cursor_size(&mut self, value: f32) {
        assert_positive_f32("Style::set_input_text_cursor_size()", "value", value);
        self.inner_mut().InputTextCursorSize = value;
    }

    pub fn mouse_cursor_scale(&self) -> f32 {
        self.inner().MouseCursorScale
    }
    pub fn set_mouse_cursor_scale(&mut self, v: f32) {
        assert_positive_f32("Style::set_mouse_cursor_scale()", "v", v);
        self.inner_mut().MouseCursorScale = v;
    }

    pub fn anti_aliased_lines(&self) -> bool {
        self.inner().AntiAliasedLines
    }
    pub fn set_anti_aliased_lines(&mut self, v: bool) {
        self.inner_mut().AntiAliasedLines = v;
    }

    /// Returns whether open line ends are anti-aliased.
    #[doc(alias = "AntiAliasedLineEnds")]
    pub fn anti_aliased_line_ends(&self) -> bool {
        self.inner().AntiAliasedLineEnds
    }
    /// Sets anti-aliasing for open line ends.
    #[doc(alias = "AntiAliasedLineEnds")]
    pub fn set_anti_aliased_line_ends(&mut self, v: bool) {
        self.inner_mut().AntiAliasedLineEnds = v;
    }

    pub fn anti_aliased_fill(&self) -> bool {
        self.inner().AntiAliasedFill
    }
    pub fn set_anti_aliased_fill(&mut self, v: bool) {
        self.inner_mut().AntiAliasedFill = v;
    }

    /// Returns the maximum curve tessellation error in pixels.
    #[doc(alias = "CurveTessellationMaxError")]
    pub fn curve_tessellation_max_error(&self) -> f32 {
        self.inner().CurveTessellationMaxError
    }
    /// Sets the maximum curve tessellation error in pixels.
    ///
    /// To migrate from the former squared-pixel tolerance, pass its square root.
    #[doc(alias = "CurveTessellationMaxError")]
    pub fn set_curve_tessellation_max_error(&mut self, v: f32) {
        assert_positive_f32("Style::set_curve_tessellation_max_error()", "v", v);
        self.inner_mut().CurveTessellationMaxError = v;
    }

    pub fn circle_tessellation_max_error(&self) -> f32 {
        self.inner().CircleTessellationMaxError
    }
    pub fn set_circle_tessellation_max_error(&mut self, v: f32) {
        assert_positive_f32("Style::set_circle_tessellation_max_error()", "v", v);
        self.inner_mut().CircleTessellationMaxError = v;
    }

    pub fn hover_stationary_delay(&self) -> f32 {
        self.inner().HoverStationaryDelay
    }
    pub fn set_hover_stationary_delay(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_hover_stationary_delay()", "v", v);
        self.inner_mut().HoverStationaryDelay = v;
    }

    pub fn hover_delay_short(&self) -> f32 {
        self.inner().HoverDelayShort
    }
    pub fn set_hover_delay_short(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_hover_delay_short()", "v", v);
        self.inner_mut().HoverDelayShort = v;
    }

    pub fn hover_delay_normal(&self) -> f32 {
        self.inner().HoverDelayNormal
    }
    pub fn set_hover_delay_normal(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_hover_delay_normal()", "v", v);
        self.inner_mut().HoverDelayNormal = v;
    }

    pub fn hover_flags_for_tooltip_mouse(&self) -> TooltipHoveredFlags {
        TooltipHoveredFlags::from_bits_retain(self.inner().HoverFlagsForTooltipMouse)
    }
    pub fn set_hover_flags_for_tooltip_mouse(&mut self, flags: TooltipHoveredFlags) {
        validate_tooltip_hovered_flags("Style::set_hover_flags_for_tooltip_mouse()", flags);
        self.inner_mut().HoverFlagsForTooltipMouse = flags.bits() as sys::ImGuiHoveredFlags;
    }

    pub fn hover_flags_for_tooltip_nav(&self) -> TooltipHoveredFlags {
        TooltipHoveredFlags::from_bits_retain(self.inner().HoverFlagsForTooltipNav)
    }
    pub fn set_hover_flags_for_tooltip_nav(&mut self, flags: TooltipHoveredFlags) {
        validate_tooltip_hovered_flags("Style::set_hover_flags_for_tooltip_nav()", flags);
        self.inner_mut().HoverFlagsForTooltipNav = flags.bits() as sys::ImGuiHoveredFlags;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn scale_all_sizes_uses_upstream_rounding_and_rejects_invalid_factors() {
        let mut ctx = crate::Context::create();
        let style = ctx.style_mut();
        style.set_window_padding([3.0, 5.0]);
        style.scale_all_sizes(2.0);
        assert_eq!(style.window_padding(), [6.0, 10.0]);

        for scale_factor in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    style.scale_all_sizes(scale_factor);
                }))
                .is_err()
            );
        }
    }
}

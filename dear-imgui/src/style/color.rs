#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]

use super::{Style, validate_style_color};
use crate::sys;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

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
    CheckboxSelectedBg = sys::ImGuiCol_CheckboxSelectedBg as i32,
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

impl Style {
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
}

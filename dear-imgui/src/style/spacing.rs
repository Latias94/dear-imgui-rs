use super::validation::{
    assert_non_negative_f32, assert_non_negative_vec2, assert_unit_vec2,
    validate_color_button_position,
};
use super::{Direction, Style};
use crate::sys;

impl Style {
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
}

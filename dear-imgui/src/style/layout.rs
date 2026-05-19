use super::validation::{
    assert_non_negative_f32, assert_non_negative_vec2, assert_unit_f32, assert_unit_vec2,
    assert_window_min_size, validate_window_menu_button_position,
};
use super::{Direction, Style};
use crate::sys;

impl Style {
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
}

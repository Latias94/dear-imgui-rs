use super::Style;
use super::validation::{assert_non_negative_f32, assert_non_negative_vec2, assert_unit_vec2};
use crate::sys;

impl Style {
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
}

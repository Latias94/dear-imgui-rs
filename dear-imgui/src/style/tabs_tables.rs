use super::Style;
use super::validation::{
    assert_non_negative_f32, assert_positive_f32, assert_tab_close_button_min_width,
    assert_table_angled_headers_angle, assert_unit_vec2,
};
use crate::sys;

impl Style {
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
}

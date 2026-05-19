use super::Style;
use super::validation::{assert_finite_f32, assert_non_negative_f32};

impl Style {
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
}

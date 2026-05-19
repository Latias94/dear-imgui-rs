use super::Style;
use super::validation::assert_non_negative_f32;

impl Style {
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
}

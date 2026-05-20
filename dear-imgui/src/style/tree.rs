#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]

use super::Style;
use super::validation::{assert_non_negative_f32, validate_tree_lines_flags};
use crate::sys;
use crate::widget::TreeNodeFlags;

impl Style {
    pub fn tree_lines_flags(&self) -> TreeNodeFlags {
        TreeNodeFlags::from_bits_retain(self.inner().TreeLinesFlags as i32)
    }
    pub fn set_tree_lines_flags(&mut self, flags: TreeNodeFlags) {
        validate_tree_lines_flags("Style::set_tree_lines_flags()", flags);
        self.inner_mut().TreeLinesFlags = flags.bits() as sys::ImGuiTreeNodeFlags;
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
}

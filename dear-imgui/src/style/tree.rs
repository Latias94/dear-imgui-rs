#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]

use super::Style;
use super::validation::assert_non_negative_f32;
use crate::sys;

/// Tree hierarchy guide-line drawing mode stored in `ImGuiStyle::TreeLinesFlags`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TreeLineMode(i32);

impl TreeLineMode {
    /// No tree hierarchy guide lines are drawn.
    pub const NONE: Self = Self(sys::ImGuiTreeNodeFlags_DrawLinesNone as i32);
    /// Draw full tree hierarchy guide lines.
    pub const FULL: Self = Self(sys::ImGuiTreeNodeFlags_DrawLinesFull as i32);
    /// Draw tree hierarchy guide lines only to nodes.
    pub const TO_NODES: Self = Self(sys::ImGuiTreeNodeFlags_DrawLinesToNodes as i32);

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

impl Style {
    pub fn tree_lines_mode(&self) -> TreeLineMode {
        TreeLineMode::from_bits_retain(self.inner().TreeLinesFlags as i32)
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
}

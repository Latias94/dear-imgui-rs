use super::*;
use crate::validation::assert_finite_f32;

impl Ui {
    /// Set next item to be open by default.
    ///
    /// This is useful for tree nodes, collapsing headers, etc.
    #[doc(alias = "SetNextItemOpen")]
    pub fn set_next_item_open(&self, is_open: bool) {
        self.run_with_bound_context(|| unsafe {
            sys::igSetNextItemOpen(is_open, 0); // 0 = ImGuiCond_Always
        });
    }

    /// Set next item to be open by default with condition.
    #[doc(alias = "SetNextItemOpen")]
    pub fn set_next_item_open_with_cond(&self, is_open: bool, cond: crate::Condition) {
        self.run_with_bound_context(|| unsafe {
            sys::igSetNextItemOpen(is_open, cond as sys::ImGuiCond)
        });
    }

    /// Set next item width.
    ///
    /// Set to 0.0 for default width, >0.0 for explicit width, <0.0 for relative width.
    #[doc(alias = "SetNextItemWidth")]
    pub fn set_next_item_width(&self, item_width: f32) {
        assert_finite_f32("Ui::set_next_item_width()", "item_width", item_width);
        self.run_with_bound_context(|| unsafe {
            sys::igSetNextItemWidth(item_width);
        });
    }

    /// Display a text label with a boolean value (for quick debug UIs).
    #[doc(alias = "Value")]
    pub fn value_bool(&self, prefix: impl AsRef<str>, v: bool) {
        self.with_scratch_txt(prefix, |prefix| unsafe { sys::igValue_Bool(prefix, v) })
    }
}

use crate::input::MouseButton;
use crate::sys;
use crate::utils::non_negative_count_from_i32;

impl crate::ui::Ui {
    /// Returns a delayed single-click count, or an immediate count for repeated clicks.
    ///
    /// This uses the left mouse button and [`Io::mouse_single_click_delay`](crate::Io::mouse_single_click_delay).
    #[doc(alias = "GetItemClickedCountWithSingleClickDelay")]
    pub fn item_clicked_count_with_single_click_delay(&self) -> usize {
        self.item_clicked_count_with_single_click_delay_for(MouseButton::Left)
    }

    /// Returns a delayed single-click count for `button`, or an immediate count for repeated clicks.
    #[doc(alias = "GetItemClickedCountWithSingleClickDelay")]
    pub fn item_clicked_count_with_single_click_delay_for(&self, button: MouseButton) -> usize {
        non_negative_count_from_i32(
            "Ui::item_clicked_count_with_single_click_delay_for()",
            self.run_with_bound_context(|| unsafe {
                sys::igGetItemClickedCountWithSingleClickDelay(button.into(), -1.0)
            }),
        )
    }

    /// Returns a delayed single-click count using an explicit delay.
    ///
    /// Dear ImGui clamps the delay to remain longer than the configured double-click time.
    #[doc(alias = "GetItemClickedCountWithSingleClickDelay")]
    pub fn item_clicked_count_with_delay(&self, button: MouseButton, delay: f32) -> usize {
        assert!(
            delay.is_finite() && delay >= 0.0,
            "Ui::item_clicked_count_with_delay() delay must be finite and non-negative"
        );
        non_negative_count_from_i32(
            "Ui::item_clicked_count_with_delay()",
            self.run_with_bound_context(|| unsafe {
                sys::igGetItemClickedCountWithSingleClickDelay(button.into(), delay)
            }),
        )
    }

    /// Returns `true` if the last item open state was toggled
    #[doc(alias = "IsItemToggledOpen")]
    pub fn is_item_toggled_open(&self) -> bool {
        self.run_with_bound_context(|| unsafe { sys::igIsItemToggledOpen() })
    }

    /// Returns the upper-left bounding rectangle of the last item (screen space)
    #[doc(alias = "GetItemRectMin")]
    pub fn item_rect_min(&self) -> [f32; 2] {
        let rect = self.run_with_bound_context(|| unsafe { sys::igGetItemRectMin() });
        [rect.x, rect.y]
    }

    /// Returns the lower-right bounding rectangle of the last item (screen space)
    #[doc(alias = "GetItemRectMax")]
    pub fn item_rect_max(&self) -> [f32; 2] {
        let rect = self.run_with_bound_context(|| unsafe { sys::igGetItemRectMax() });
        [rect.x, rect.y]
    }

    /// Allows the next item to be overlapped by a subsequent item.
    #[doc(alias = "SetNextItemAllowOverlap")]
    pub fn set_next_item_allow_overlap(&self) {
        self.run_with_bound_context(|| unsafe { sys::igSetNextItemAllowOverlap() });
    }
}

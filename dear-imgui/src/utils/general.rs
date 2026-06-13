use super::counts::non_negative_count_from_i32;
use crate::sys;

impl crate::ui::Ui {
    /// Get global imgui time. Incremented by io.DeltaTime every frame.
    #[doc(alias = "GetTime")]
    pub fn time(&self) -> f64 {
        self.run_with_bound_context(|| unsafe { sys::igGetTime() })
    }

    /// Get global imgui frame count. Incremented by 1 every frame.
    #[doc(alias = "GetFrameCount")]
    pub fn frame_count(&self) -> usize {
        non_negative_count_from_i32(
            "Ui::frame_count()",
            self.run_with_bound_context(|| unsafe { sys::igGetFrameCount() }),
        )
    }

    /// Returns the width of an item based on the current layout state.
    #[doc(alias = "CalcItemWidth")]
    pub fn calc_item_width(&self) -> f32 {
        self.run_with_bound_context(|| unsafe { sys::igCalcItemWidth() })
    }
}

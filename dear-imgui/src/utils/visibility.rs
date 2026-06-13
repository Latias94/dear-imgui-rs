use super::validation::assert_finite_vec2;
use crate::sys;

impl crate::ui::Ui {
    /// Test if rectangle (of given size, starting from cursor position) is visible / not clipped.
    #[doc(alias = "IsRectVisible")]
    pub fn is_rect_visible(&self, size: [f32; 2]) -> bool {
        assert_finite_vec2("Ui::is_rect_visible()", "size", size);
        self.run_with_bound_context(|| unsafe {
            let size = sys::ImVec2 {
                x: size[0],
                y: size[1],
            };
            sys::igIsRectVisible_Nil(size)
        })
    }

    /// Test if rectangle (in screen space) is visible / not clipped.
    #[doc(alias = "IsRectVisible")]
    pub fn is_rect_visible_ex(&self, rect_min: [f32; 2], rect_max: [f32; 2]) -> bool {
        assert_finite_vec2("Ui::is_rect_visible_ex()", "rect_min", rect_min);
        assert_finite_vec2("Ui::is_rect_visible_ex()", "rect_max", rect_max);
        self.run_with_bound_context(|| unsafe {
            let rect_min = sys::ImVec2 {
                x: rect_min[0],
                y: rect_min[1],
            };
            let rect_max = sys::ImVec2 {
                x: rect_max[0],
                y: rect_max[1],
            };
            sys::igIsRectVisible_Vec2(rect_min, rect_max)
        })
    }
}

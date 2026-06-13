use super::validation::assert_finite_vec2;
use crate::Ui;
use crate::sys;

create_token!(
    /// Tracks a pushed clip rect that will be popped on drop.
    pub struct ClipRectToken<'ui>;

    /// Pops a clip rect pushed with [`Ui::push_clip_rect`].
    drop { unsafe { sys::igPopClipRect() } }
);

impl Ui {
    /// Push a clipping rectangle in screen space.
    #[doc(alias = "PushClipRect")]
    pub fn push_clip_rect(
        &self,
        min: impl Into<[f32; 2]>,
        max: impl Into<[f32; 2]>,
        intersect_with_current: bool,
    ) -> ClipRectToken<'_> {
        let min = min.into();
        let max = max.into();
        assert_finite_vec2("Ui::push_clip_rect()", "min", min);
        assert_finite_vec2("Ui::push_clip_rect()", "max", max);
        let min_v = sys::ImVec2 {
            x: min[0],
            y: min[1],
        };
        let max_v = sys::ImVec2 {
            x: max[0],
            y: max[1],
        };
        unsafe { sys::igPushClipRect(min_v, max_v, intersect_with_current) };
        ClipRectToken::new(self)
    }

    /// Run a closure with a clip rect pushed and automatically popped.
    pub fn with_clip_rect<R>(
        &self,
        min: impl Into<[f32; 2]>,
        max: impl Into<[f32; 2]>,
        intersect_with_current: bool,
        f: impl FnOnce() -> R,
    ) -> R {
        let _t = self.push_clip_rect(min, max, intersect_with_current);
        f()
    }

    /// Returns true if the specified rectangle (min,max) is visible (not clipped).
    #[doc(alias = "IsRectVisible")]
    pub fn is_rect_visible_min_max(
        &self,
        rect_min: impl Into<[f32; 2]>,
        rect_max: impl Into<[f32; 2]>,
    ) -> bool {
        let mn = rect_min.into();
        let mx = rect_max.into();
        assert_finite_vec2("Ui::is_rect_visible_min_max()", "rect_min", mn);
        assert_finite_vec2("Ui::is_rect_visible_min_max()", "rect_max", mx);
        let mn_v = sys::ImVec2 { x: mn[0], y: mn[1] };
        let mx_v = sys::ImVec2 { x: mx[0], y: mx[1] };
        unsafe { sys::igIsRectVisible_Vec2(mn_v, mx_v) }
    }

    /// Returns true if a rectangle of given size at the current cursor pos is visible.
    #[doc(alias = "IsRectVisible")]
    pub fn is_rect_visible_with_size(&self, size: impl Into<[f32; 2]>) -> bool {
        let s = size.into();
        assert_finite_vec2("Ui::is_rect_visible_with_size()", "size", s);
        let v = sys::ImVec2 { x: s[0], y: s[1] };
        unsafe { sys::igIsRectVisible_Nil(v) }
    }
}

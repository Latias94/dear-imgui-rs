use crate::sys;

use super::super::util::finite_vec2;
use super::{DrawListClipRectGuard, DrawListMut};

impl<'ui> DrawListMut<'ui> {
    /// Push a clip rectangle, optionally intersecting with the current clip rect.
    #[doc(alias = "PushClipRect")]
    pub fn push_clip_rect(
        &self,
        clip_rect_min: impl Into<sys::ImVec2>,
        clip_rect_max: impl Into<sys::ImVec2>,
        intersect_with_current: bool,
    ) {
        let clip_rect_min = finite_vec2(
            "DrawListMut::push_clip_rect()",
            "clip_rect_min",
            clip_rect_min,
        );
        let clip_rect_max = finite_vec2(
            "DrawListMut::push_clip_rect()",
            "clip_rect_max",
            clip_rect_max,
        );

        unsafe {
            sys::ImDrawList_PushClipRect(
                self.draw_list,
                clip_rect_min,
                clip_rect_max,
                intersect_with_current,
            )
        }
    }

    /// Push a full-screen clip rectangle.
    #[doc(alias = "PushClipRectFullScreen")]
    pub fn push_clip_rect_full_screen(&self) {
        unsafe { sys::ImDrawList_PushClipRectFullScreen(self.draw_list) }
    }

    /// Pop the last clip rectangle.
    #[doc(alias = "PopClipRect")]
    pub fn pop_clip_rect(&self) {
        unsafe { sys::ImDrawList_PopClipRect(self.draw_list) }
    }

    /// Get current minimum clip rectangle point.
    pub fn clip_rect_min(&self) -> [f32; 2] {
        let out = unsafe { sys::ImDrawList_GetClipRectMin(self.draw_list) };
        out.into()
    }

    /// Get current maximum clip rectangle point.
    pub fn clip_rect_max(&self) -> [f32; 2] {
        let out = unsafe { sys::ImDrawList_GetClipRectMax(self.draw_list) };
        out.into()
    }

    /// Convenience: push a clip rect, run f, pop.
    pub fn with_clip_rect<F>(
        &self,
        clip_rect_min: impl Into<sys::ImVec2>,
        clip_rect_max: impl Into<sys::ImVec2>,
        f: F,
    ) where
        F: FnOnce(),
    {
        self.push_clip_rect(clip_rect_min, clip_rect_max, false);
        let _clip_rect_guard = DrawListClipRectGuard { draw_list: self };
        f();
    }
}

use crate::sys;

use super::super::color::ImColor32;
use super::super::util::{assert_non_negative_f32, finite_vec2, finite_vec4};
use super::DrawListMut;

impl<'ui> DrawListMut<'ui> {
    /// Disable text pixel snapping while `f` runs, then restore draw-list defaults.
    ///
    /// Restoration also occurs if `f` panics. See [`Self::with_draw_flags`] for
    /// nesting limits and other configurable drawing options.
    pub fn with_text_no_pixel_snap<R>(&self, f: impl FnOnce() -> R) -> R {
        self.with_draw_flags(super::super::DrawFlags::TEXT_NO_PIXEL_SNAP, true, f)
    }

    /// Draw a text whose upper-left corner is at point `pos`.
    pub fn add_text(
        &self,
        pos: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
        text: impl AsRef<str>,
    ) {
        use std::os::raw::c_char;

        let text = text.as_ref();
        let pos = finite_vec2("DrawListMut::add_text()", "pos", pos);
        let col = col.into();

        self.with_raw_bound("DrawListMut::add_text()", |draw_list| unsafe {
            let start = text.as_ptr().cast::<c_char>();
            let end = start.add(text.len());
            sys::ImDrawList_AddText_Vec2(draw_list, pos, col.into(), start, end);
        });
    }

    /// Draw text with an explicit font and optional fine CPU clip rectangle.
    ///
    /// This mirrors Dear ImGui's `ImDrawList::AddText(ImFont*, ...)` overload.
    #[doc(alias = "AddText")]
    pub fn add_text_with_font(
        &self,
        font: crate::fonts::FontId,
        font_size: f32,
        pos: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
        text: impl AsRef<str>,
        wrap_width: f32,
        cpu_fine_clip_rect: Option<[f32; 4]>,
    ) {
        use std::os::raw::c_char;
        let text = text.as_ref();
        let pos = finite_vec2("DrawListMut::add_text_with_font()", "pos", pos);
        assert_non_negative_f32("DrawListMut::add_text_with_font()", "font_size", font_size);
        assert_non_negative_f32(
            "DrawListMut::add_text_with_font()",
            "wrap_width",
            wrap_width,
        );
        let col = col.into();
        let clip_vec4 = cpu_fine_clip_rect
            .map(|r| finite_vec4("DrawListMut::add_text_with_font()", "cpu_fine_clip_rect", r));
        let clip_ptr = match clip_vec4.as_ref() {
            Some(v) => v as *const sys::ImVec4,
            None => std::ptr::null(),
        };

        self.with_raw_bound("DrawListMut::add_text_with_font()", |draw_list| unsafe {
            let font_ptr = crate::fonts::validate_font_id_for_context(
                font,
                self.ui().context_raw(),
                "DrawListMut::add_text_with_font()",
            );
            let start = text.as_ptr().cast::<c_char>();
            let end = start.add(text.len());
            sys::ImDrawList_AddText_FontPtr(
                draw_list,
                font_ptr,
                font_size,
                pos,
                col.into(),
                start,
                end,
                wrap_width,
                clip_ptr,
            );
        });
    }
}

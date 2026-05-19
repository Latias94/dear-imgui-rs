use crate::sys;

use super::super::color::ImColor32;
use super::super::util::{assert_non_negative_f32, finite_vec2, finite_vec4};
use super::DrawListMut;

impl<'ui> DrawListMut<'ui> {
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

        unsafe {
            let start = text.as_ptr() as *const c_char;
            let end = (start as usize + text.len()) as *const c_char;
            sys::ImDrawList_AddText_Vec2(self.draw_list, pos, col.into(), start, end);
        }
    }

    /// Draw text with an explicit font and optional fine CPU clip rectangle.
    ///
    /// This mirrors Dear ImGui's `ImDrawList::AddText(ImFont*, ...)` overload.
    #[doc(alias = "AddText")]
    pub fn add_text_with_font(
        &self,
        font: &crate::fonts::Font,
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
        let font_ptr = crate::fonts::validate_font_for_current_context(
            font,
            "DrawListMut::add_text_with_font()",
        );

        let clip_vec4 = cpu_fine_clip_rect
            .map(|r| finite_vec4("DrawListMut::add_text_with_font()", "cpu_fine_clip_rect", r));
        let clip_ptr = match clip_vec4.as_ref() {
            Some(v) => v as *const sys::ImVec4,
            None => std::ptr::null(),
        };

        unsafe {
            let start = text.as_ptr() as *const c_char;
            let end = (start as usize + text.len()) as *const c_char;
            sys::ImDrawList_AddText_FontPtr(
                self.draw_list,
                font_ptr,
                font_size,
                pos,
                col.into(),
                start,
                end,
                wrap_width,
                clip_ptr,
            );
        }
    }
}

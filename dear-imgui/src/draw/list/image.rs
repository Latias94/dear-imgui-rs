use crate::sys;

use super::super::color::ImColor32;
use super::super::counts::DrawCornerFlags;
use super::super::util::{assert_corner_flags, assert_non_negative_f32, finite_vec2};
use super::DrawListMut;

impl<'ui> DrawListMut<'ui> {
    /// Add an image quad (axis-aligned). Tint via `col`.
    #[doc(alias = "AddImage")]
    pub fn add_image<'tex>(
        &self,
        texture: impl Into<crate::texture::TextureRef<'tex>>,
        p_min: impl Into<sys::ImVec2>,
        p_max: impl Into<sys::ImVec2>,
        uv_min: impl Into<sys::ImVec2>,
        uv_max: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
    ) {
        // Example:
        // let tex = texture::TextureId::new(5);
        // self.add_image(tex, [10.0,10.0], [110.0,110.0], [0.0,0.0], [1.0,1.0], Color::WHITE);
        let p_min = finite_vec2("DrawListMut::add_image()", "p_min", p_min);
        let p_max = finite_vec2("DrawListMut::add_image()", "p_max", p_max);
        let uv_min = finite_vec2("DrawListMut::add_image()", "uv_min", uv_min);
        let uv_max = finite_vec2("DrawListMut::add_image()", "uv_max", uv_max);
        let col = col.into().to_bits();
        let tex_ref = texture.into().raw();
        unsafe {
            sys::ImDrawList_AddImage(self.draw_list, tex_ref, p_min, p_max, uv_min, uv_max, col)
        }
    }

    /// Add an image with 4 arbitrary corners.
    #[doc(alias = "AddImageQuad")]
    pub fn add_image_quad<'tex>(
        &self,
        texture: impl Into<crate::texture::TextureRef<'tex>>,
        p1: impl Into<sys::ImVec2>,
        p2: impl Into<sys::ImVec2>,
        p3: impl Into<sys::ImVec2>,
        p4: impl Into<sys::ImVec2>,
        uv1: impl Into<sys::ImVec2>,
        uv2: impl Into<sys::ImVec2>,
        uv3: impl Into<sys::ImVec2>,
        uv4: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
    ) {
        // Example:
        // let tex = texture::TextureId::new(5);
        // self.add_image_quad(
        //     tex,
        //     [10.0,10.0], [110.0,20.0], [120.0,120.0], [5.0,100.0],
        //     [0.0,0.0], [1.0,0.0], [1.0,1.0], [0.0,1.0],
        //     Color::WHITE,
        // );
        let p1 = finite_vec2("DrawListMut::add_image_quad()", "p1", p1);
        let p2 = finite_vec2("DrawListMut::add_image_quad()", "p2", p2);
        let p3 = finite_vec2("DrawListMut::add_image_quad()", "p3", p3);
        let p4 = finite_vec2("DrawListMut::add_image_quad()", "p4", p4);
        let uv1 = finite_vec2("DrawListMut::add_image_quad()", "uv1", uv1);
        let uv2 = finite_vec2("DrawListMut::add_image_quad()", "uv2", uv2);
        let uv3 = finite_vec2("DrawListMut::add_image_quad()", "uv3", uv3);
        let uv4 = finite_vec2("DrawListMut::add_image_quad()", "uv4", uv4);
        let col = col.into().to_bits();
        let tex_ref = texture.into().raw();
        unsafe {
            sys::ImDrawList_AddImageQuad(
                self.draw_list,
                tex_ref,
                p1,
                p2,
                p3,
                p4,
                uv1,
                uv2,
                uv3,
                uv4,
                col,
            )
        }
    }

    /// Add an axis-aligned rounded image.
    #[doc(alias = "AddImageRounded")]
    pub fn add_image_rounded<'tex>(
        &self,
        texture: impl Into<crate::texture::TextureRef<'tex>>,
        p_min: impl Into<sys::ImVec2>,
        p_max: impl Into<sys::ImVec2>,
        uv_min: impl Into<sys::ImVec2>,
        uv_max: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
        rounding: f32,
        flags: DrawCornerFlags,
    ) {
        // Example:
        // let tex = texture::TextureId::new(5);
        // self.add_image_rounded(
        //     tex,
        //     [10.0,10.0], [110.0,110.0],
        //     [0.0,0.0], [1.0,1.0],
        //     Color::WHITE,
        //     8.0,
        //     DrawCornerFlags::ALL,
        // );
        let p_min = finite_vec2("DrawListMut::add_image_rounded()", "p_min", p_min);
        let p_max = finite_vec2("DrawListMut::add_image_rounded()", "p_max", p_max);
        let uv_min = finite_vec2("DrawListMut::add_image_rounded()", "uv_min", uv_min);
        let uv_max = finite_vec2("DrawListMut::add_image_rounded()", "uv_max", uv_max);
        assert_non_negative_f32("DrawListMut::add_image_rounded()", "rounding", rounding);
        assert_corner_flags("DrawListMut::add_image_rounded()", flags);
        let col = col.into().to_bits();
        let tex_ref = texture.into().raw();
        unsafe {
            sys::ImDrawList_AddImageRounded(
                self.draw_list,
                tex_ref,
                p_min,
                p_max,
                uv_min,
                uv_max,
                col,
                rounding,
                flags.bits() as sys::ImDrawFlags,
            )
        }
    }
}

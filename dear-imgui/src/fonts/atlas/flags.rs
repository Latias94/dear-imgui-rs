use bitflags::bitflags;

use super::FontAtlas;
use crate::sys;

bitflags! {
    /// Atlas texture options, configured before its first build.
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
    pub struct FontAtlasFlags: i32 {
        const NONE = sys::ImFontAtlasFlags_None as i32;
        /// Do not round the texture height to a power of two.
        const NO_POWER_OF_TWO_HEIGHT = sys::ImFontAtlasFlags_NoPowerOfTwoHeight as i32;
        /// Omit software mouse cursor images.
        const NO_MOUSE_CURSORS = sys::ImFontAtlasFlags_NoMouseCursors as i32;
        /// Omit line textures. This also disables line and line-end anti-aliasing.
        const NO_BAKED_LINES = sys::ImFontAtlasFlags_NoBakedLines as i32;
        /// Omit round-corner textures, using tessellated geometry instead.
        const NO_BAKED_ROUND_CORNERS = sys::ImFontAtlasFlags_NoBakedRoundCorners as i32;
    }
}

impl FontAtlas {
    /// Returns atlas texture options.
    pub fn flags(&self) -> FontAtlasFlags {
        FontAtlasFlags::from_bits_retain(unsafe { (*self.raw()).Flags })
    }

    /// Configures atlas texture options before its first build.
    ///
    /// # Panics
    ///
    /// Panics during an open frame, while texture data is borrowed, for unknown flags,
    /// or after building has started. Call [`Self::clear`] and repopulate the atlas
    /// before changing options on an existing atlas.
    pub fn set_flags(&self, flags: FontAtlasFlags) {
        self.assert_mutation_allowed("FontAtlas::set_flags()");
        assert!(
            flags.difference(FontAtlasFlags::all()).is_empty(),
            "FontAtlas::set_flags() requires known atlas flags"
        );
        let raw = self.raw();
        assert!(
            unsafe { (*raw).Builder.is_null() && !(*raw).TexIsBuilt },
            "FontAtlas::set_flags() requires an atlas that has not started building; clear and repopulate it first"
        );
        unsafe { (*raw).Flags = flags.bits() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_options_apply_before_build_and_reject_late_changes() {
        let mut ctx = crate::Context::create();
        let flags = FontAtlasFlags::NO_BAKED_LINES | FontAtlasFlags::NO_BAKED_ROUND_CORNERS;
        ctx.font_atlas().set_flags(flags);
        ctx.font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        assert_eq!(ctx.font_atlas().flags(), flags);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ctx.font_atlas().set_flags(FontAtlasFlags::NONE);
        }));
        assert!(result.is_err());
        assert_eq!(ctx.font_atlas().flags(), flags);

        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        ctx.frame().text("atlas options");
        assert!(!ctx.style().anti_aliased_lines());
        assert!(!ctx.style().anti_aliased_line_ends());
        let _ = ctx.render_legacy();

        ctx.font_atlas().clear();
        ctx.font_atlas().set_flags(FontAtlasFlags::NO_MOUSE_CURSORS);
        ctx.font_atlas()
            .add_font(&[crate::FontSource::default_font()]);
        ctx.font_atlas()
            .try_claim_legacy_renderer()
            .unwrap()
            .build();
        assert_eq!(ctx.font_atlas().flags(), FontAtlasFlags::NO_MOUSE_CURSORS);
    }

    #[test]
    fn no_baked_round_corners_tessellates_filled_and_outlined_rectangles() {
        fn draw_rectangle(flags: FontAtlasFlags, filled: bool) -> (Vec<crate::DrawVert>, [f32; 2]) {
            let mut ctx = crate::Context::create();
            ctx.font_atlas().set_flags(flags);
            ctx.font_atlas()
                .try_claim_legacy_renderer()
                .unwrap()
                .build();
            ctx.io_mut().set_display_size([128.0, 128.0]);
            ctx.io_mut().set_delta_time(1.0 / 60.0);
            let ui = ctx.frame();
            let white_uv = ui.font_tex_uv_white_pixel();
            ui.get_background_draw_list()
                .add_rect([20.0, 20.0], [100.0, 80.0], crate::draw::ImColor32::WHITE)
                .rounding(8.0)
                .thickness(2.0)
                .filled(filled)
                .build();
            let frame = ctx.render_legacy();
            let draw_data = frame.draw_data();
            assert_eq!(draw_data.draw_lists_count(), 1);
            let draw_list = draw_data.draw_lists().next().unwrap();
            assert!(!draw_list.idx_buffer().is_empty());
            assert!(
                draw_list
                    .idx_buffer()
                    .iter()
                    .all(|&index| usize::from(index) < draw_list.vtx_buffer().len())
            );
            (draw_list.vtx_buffer().to_vec(), white_uv)
        }

        for filled in [false, true] {
            let (baked, baked_white_uv) = draw_rectangle(FontAtlasFlags::NONE, filled);
            let (geometry, white_uv) =
                draw_rectangle(FontAtlasFlags::NO_BAKED_ROUND_CORNERS, filled);
            assert!(!baked.is_empty());
            assert!(!geometry.is_empty());
            assert!(baked.iter().any(|vertex| vertex.uv != baked_white_uv));
            assert!(baked.iter().any(|vertex| vertex.uv[1] != baked[0].uv[1]));
            if filled {
                assert!(geometry.iter().all(|vertex| vertex.uv == white_uv));
            } else {
                // Outlines still use the one-dimensional line AA texture.
                assert!(
                    geometry
                        .iter()
                        .all(|vertex| vertex.uv[1] == geometry[0].uv[1])
                );
            }

            let bounds = |vertices: &[crate::DrawVert]| {
                vertices.iter().fold(
                    [
                        f32::INFINITY,
                        f32::INFINITY,
                        f32::NEG_INFINITY,
                        f32::NEG_INFINITY,
                    ],
                    |bounds, vertex| {
                        assert!(vertex.pos.into_iter().chain(vertex.uv).all(f32::is_finite));
                        [
                            bounds[0].min(vertex.pos[0]),
                            bounds[1].min(vertex.pos[1]),
                            bounds[2].max(vertex.pos[0]),
                            bounds[3].max(vertex.pos[1]),
                        ]
                    },
                )
            };
            let baked_bounds = bounds(&baked);
            let geometry_bounds = bounds(&geometry);
            for (actual, expected) in geometry_bounds.into_iter().zip([20.0, 20.0, 100.0, 80.0]) {
                assert!((actual - expected).abs() <= 1.0);
            }
            for (actual, reference) in geometry_bounds.into_iter().zip(baked_bounds) {
                assert!((actual - reference).abs() <= 1.0);
            }
        }
    }
}

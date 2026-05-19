use super::id::validate_font_id_for_atlas;
use super::validation::RASTERIZER_MULTIPLY_MAX;
use super::*;

#[test]
fn font_config_glyph_exclude_ranges_converts_and_terminates() {
    let cfg = FontConfig::new().glyph_exclude_ranges(&[(0x41, 0x5a)]);
    assert!(!cfg.raw.GlyphExcludeRanges.is_null());
    unsafe {
        assert_eq!(*cfg.raw.GlyphExcludeRanges.add(0), 0x41 as sys::ImWchar);
        assert_eq!(*cfg.raw.GlyphExcludeRanges.add(1), 0x5a as sys::ImWchar);
        assert_eq!(*cfg.raw.GlyphExcludeRanges.add(2), 0);
    }
}

#[test]
fn font_config_glyph_exclude_ranges_accepts_non_bmp_when_wchar32() {
    if std::mem::size_of::<sys::ImWchar>() != 4 {
        return;
    }
    let cfg = FontConfig::new().glyph_exclude_ranges(&[(0x1_0000, 0x1_0001)]);
    assert!(!cfg.raw.GlyphExcludeRanges.is_null());
    unsafe {
        assert_eq!(*cfg.raw.GlyphExcludeRanges.add(0), 0x1_0000 as sys::ImWchar);
        assert_eq!(*cfg.raw.GlyphExcludeRanges.add(1), 0x1_0001 as sys::ImWchar);
        assert_eq!(*cfg.raw.GlyphExcludeRanges.add(2), 0);
    }
}

#[test]
fn font_config_glyph_exclude_ranges_rejects_out_of_range() {
    let out_of_range = if std::mem::size_of::<sys::ImWchar>() == 2 {
        0x1_0000
    } else {
        0x11_0000
    };
    let res = std::panic::catch_unwind(|| {
        let _ = FontConfig::new().glyph_exclude_ranges(&[(out_of_range, out_of_range)]);
    });
    assert!(res.is_err());
}

#[test]
fn font_config_glyph_exclude_ranges_rejects_reversed_ranges() {
    let res = std::panic::catch_unwind(|| {
        let _ = FontConfig::new().glyph_exclude_ranges(&[(0x42, 0x41)]);
    });
    assert!(res.is_err());
}

#[test]
fn font_config_rejects_invalid_numeric_inputs() {
    assert!(
        std::panic::catch_unwind(|| {
            let _ = FontConfig::new().size_pixels(f32::NAN);
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            let _ = FontConfig::new().size_pixels(-1.0);
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            let _ = FontConfig::new().glyph_offset([0.0, f32::INFINITY]);
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            let _ = FontConfig::new().glyph_min_advance_x(-1.0);
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            let _ = FontConfig::new()
                .glyph_min_advance_x(12.0)
                .glyph_max_advance_x(8.0);
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            let _ = FontConfig::new().glyph_extra_advance_x(f32::NAN);
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            let _ = FontConfig::new().rasterizer_multiply(-0.1);
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            let _ = FontConfig::new().rasterizer_multiply(RASTERIZER_MULTIPLY_MAX * 2.0);
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            let _ = FontConfig::new().rasterizer_density(0.0);
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            let _ = FontConfig::new().oversample_h(-1);
        })
        .is_err()
    );

    let cfg = FontConfig::new()
        .size_pixels(0.0)
        .glyph_offset([0.0, 0.0])
        .glyph_min_advance_x(0.0)
        .glyph_max_advance_x(f32::MAX)
        .glyph_extra_advance_x(-1.0)
        .rasterizer_multiply(256.0)
        .rasterizer_density(1.0)
        .oversample_h(0)
        .oversample_v(1);
    assert_eq!(cfg.raw.SizePixels, 0.0);
    assert_eq!(cfg.raw.GlyphExtraAdvanceX, -1.0);
    assert_eq!(cfg.raw.RasterizerMultiply, 256.0);
}

#[test]
fn discard_bakes_checks_unused_frame_count_before_ffi() {
    let mut atlas = FontAtlas::new();

    atlas.discard_bakes(0);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            atlas.discard_bakes(i32::MAX as usize + 1);
        }))
        .is_err()
    );
}

#[test]
fn font_atlas_rejects_glyph_metric_overrides_without_reference_size() {
    let mut atlas = FontAtlas::new();
    let cfg = FontConfig::new().glyph_offset([1.0, 0.0]);

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = atlas.add_font_from_memory_ttf(&[0u8; 10], 0.0, Some(&cfg), None);
        }))
        .is_err()
    );

    assert!(
        atlas
            .add_font_from_memory_ttf(&[0u8; 10], 13.0, Some(&cfg), None)
            .is_none()
    );

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let cfg = FontConfig::new()
                .font_data_owned_by_atlas(false)
                .glyph_min_advance_x(4.0);
            let _ = atlas.add_font_with_config(&cfg);
        }))
        .is_err()
    );
}

#[test]
fn add_font_with_config_rejects_missing_font_source_before_ffi() {
    let mut atlas = FontAtlas::new();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = atlas.add_font_with_config(&FontConfig::new());
        }))
        .is_err()
    );
}

#[test]
fn add_font_from_memory_ttf_rejects_too_small_buffers() {
    let mut ctx = crate::Context::create();
    let mut fonts = ctx.font_atlas_mut();
    assert!(
        fonts
            .add_font_from_memory_ttf(&[0u8; 10], 13.0, None, None)
            .is_none()
    );
}

#[test]
fn font_id_is_invalidated_by_clear_fonts_before_push_font_ffi() {
    let mut ctx = crate::Context::create();
    let font_id = {
        let mut fonts = ctx.font_atlas_mut();
        fonts.add_font(&[FontSource::default_font()])
    };
    {
        let mut fonts = ctx.font_atlas_mut();
        fonts.clear_fonts();
    }

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = validate_font_id_for_current_context(font_id, "test stale FontId");
    }));

    assert!(result.is_err());
}

#[test]
fn font_id_from_another_atlas_is_rejected_before_push_font_ffi() {
    let mut ctx_a = crate::Context::create();
    let font_id = {
        let mut fonts = ctx_a.font_atlas_mut();
        fonts.add_font(&[FontSource::default_font()])
    };
    let suspended_a = ctx_a.suspend();

    let mut ctx_b = crate::Context::create();
    let _ = ctx_b.font_atlas_mut().build();
    ctx_b.io_mut().set_display_size([128.0, 128.0]);
    ctx_b.io_mut().set_delta_time(1.0 / 60.0);
    let ui = ctx_b.frame();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _token = ui.push_font(font_id);
    }));

    assert!(result.is_err());

    drop(ctx_b);
    drop(suspended_a);
}

#[test]
fn font_id_from_shared_atlas_is_valid_through_another_atlas_view() {
    let shared_atlas = SharedFontAtlas::create();
    let raw = *shared_atlas.0;
    let font_id = {
        let mut atlas = unsafe { FontAtlas::from_raw(raw) };
        atlas.add_font(&[FontSource::default_font()])
    };

    let _ = validate_font_id_for_atlas(font_id, raw, "test shared FontId");
}

#[test]
fn font_sources_reject_invalid_sizes_before_ffi() {
    let mut atlas = FontAtlas::new();

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = atlas.add_font(&[FontSource::default_font_with_size(f32::NAN)]);
        }))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = atlas.add_font(&[FontSource::ttf_data_with_size(&[0u8; 10], -1.0)]);
        }))
        .is_err()
    );
}

#[test]
fn set_texture_id_preserves_managed_tex_data_reference() {
    let mut ctx = crate::Context::create();
    let mut fonts = ctx.font_atlas_mut();
    let _ = fonts.build();

    let raw_tex_data = fonts.get_tex_data();
    assert!(!raw_tex_data.is_null());

    let texture_id = crate::texture::TextureId::new(0x1234);
    fonts.set_texture_id(texture_id);

    let mut tex_ref = fonts.get_tex_ref();
    assert_eq!(tex_ref._TexData, raw_tex_data);

    let resolved = unsafe { sys::ImTextureRef_GetTexID(&mut tex_ref) };
    assert_eq!(resolved, texture_id.id() as sys::ImTextureID);
}

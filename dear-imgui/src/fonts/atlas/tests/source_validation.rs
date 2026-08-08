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
fn font_config_glyph_exclude_ranges_rejects_native_limit_overflow() {
    let ranges = [(1, 1); 33];
    assert!(std::panic::catch_unwind(|| FontConfig::new().glyph_exclude_ranges(&ranges)).is_err());
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
fn atlas_font_loader_must_be_selected_before_sources_are_added() {
    let ctx = crate::Context::create();
    let atlas = ctx.font_atlas();

    atlas
        .set_font_loader(FontLoader::stb_truetype())
        .expect("an empty atlas should allow selecting its loader");
    let _ = atlas.add_font(&[FontSource::default_font()]);

    assert_eq!(
        atlas.set_font_loader(FontLoader::stb_truetype()),
        Err(FontAtlasLoaderError::SourcesAlreadyAdded { source_count: 1 })
    );
}

#[test]
fn validated_stb_source_owns_bytes_and_pins_its_native_loader() {
    const FONT_DATA: &[u8] = include_bytes!(
        "../../../../../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/Roboto-Medium.ttf"
    );
    let ctx = crate::Context::create();
    let atlas = ctx.font_atlas();
    let validated = StbTrueTypeFontData::from_slice(FONT_DATA)
        .expect("the bundled Roboto font should satisfy the stb proof");
    let source = FontSource::stb_truetype_with_size(validated.clone(), 20.0);
    drop(validated);

    let font = atlas.add_font(&[source]);
    let sources = unsafe { &(*atlas.raw()).Sources };
    assert_eq!(sources.Size, 1);
    let native_source = unsafe { &*sources.Data };
    assert_eq!(
        native_source.FontLoader,
        FontLoader::stb_truetype().as_ptr()
    );
    assert_eq!(native_source.FontNo, 0);

    atlas
        .try_claim_legacy_renderer()
        .expect("the test requires a legacy font atlas")
        .build();
    assert!(font.is_loaded());
}

#[test]
fn validated_stb_source_supports_normal_merge_configuration() {
    const FONT_DATA: &[u8] = include_bytes!(
        "../../../../../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/Roboto-Medium.ttf"
    );
    let ctx = crate::Context::create();
    let atlas = ctx.font_atlas();

    let default = atlas.add_font(&[FontSource::default_font_with_size(16.0)]);
    atlas
        .try_claim_legacy_renderer()
        .expect("the baseline requires a legacy atlas build")
        .build();
    assert!(!default.is_glyph_in_font('Ω'));

    atlas.clear();
    let validated = StbTrueTypeFontData::from_slice(FONT_DATA).unwrap();
    let merged = FontSource::stb_truetype(validated).with_config(FontConfig::new());

    let font = atlas.add_font(&[FontSource::default_font_with_size(16.0), merged]);
    atlas
        .try_claim_legacy_renderer()
        .expect("the merged font requires a legacy atlas build")
        .build();
    assert_eq!(font.source_count(), 2);
    assert!(font.is_loaded());
    assert!(font.is_glyph_in_font('Ω'));
}

#[test]
fn validated_stb_source_rejects_loader_and_collection_overrides_before_ffi() {
    const FONT_DATA: &[u8] = include_bytes!(
        "../../../../../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/ProggyClean.ttf"
    );
    let validated = StbTrueTypeFontData::from_slice(FONT_DATA).unwrap();

    let mut loader_override = FontConfig::new();
    loader_override.raw.FontLoader = 1_usize as *const sys::ImFontLoader;
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = FontSource::stb_truetype(validated.clone()).with_config(loader_override);
        }))
        .is_err()
    );

    let mut collection_override = FontConfig::new();
    collection_override.raw.FontNo = 1;
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = FontSource::stb_truetype(validated).with_config(collection_override);
        }))
        .is_err()
    );
}

#[test]
fn validated_stb_file_reports_io_before_atlas_mutation() {
    let ctx = crate::Context::create();
    let atlas = ctx.font_atlas();
    let raw = atlas.raw();

    let error = StbTrueTypeFontData::from_file("this-font-file-must-not-exist.ttf").unwrap_err();
    assert!(matches!(error, StbTrueTypeFontLoadError::Io { .. }));
    assert_eq!(unsafe { (*raw).Fonts.Size }, 0);
    assert_eq!(unsafe { (*raw).Sources.Size }, 0);
}

#[test]
fn discard_bakes_checks_unused_frame_count_before_ffi() {
    let ctx = crate::Context::create();
    let atlas = ctx.font_atlas();

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
    const FONT_DATA: &[u8] = include_bytes!(
        "../../../../../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/ProggyClean.ttf"
    );
    let ctx = crate::Context::create();
    let atlas = ctx.font_atlas();
    let cfg = FontConfig::new().glyph_offset([1.0, 0.0]);

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let source = unsafe { FontSource::ttf_data(FONT_DATA) }.with_config(cfg.clone());
            let _ = atlas.add_font(&[source]);
        }))
        .is_err()
    );

    let source = unsafe { FontSource::ttf_data_with_size(FONT_DATA, 13.0) }.with_config(cfg);
    let _ = atlas.add_font(&[source]);
}

#[test]
fn raw_ttf_source_rejects_too_small_buffers_before_ffi() {
    let ctx = crate::Context::create();
    let fonts = ctx.font_atlas();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let source = unsafe { FontSource::ttf_data_with_size(&[0u8; 10], 13.0) };
            let _ = fonts.add_font(&[source]);
        }))
        .is_err()
    );
}

#[test]
fn compressed_font_sources_reject_truncated_headers_before_ffi() {
    let ctx = crate::Context::create();
    let fonts = ctx.font_atlas();

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let source = unsafe { FontSource::compressed_ttf_data_with_size(&[0; 15], 13.0) };
            let _ = fonts.add_font(&[source]);
        }))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let source = unsafe { FontSource::compressed_ttf_base85_with_size("!!!!!", 13.0) };
            let _ = fonts.add_font(&[source]);
        }))
        .is_err()
    );
}

#[test]
fn font_id_is_invalidated_by_clear_fonts_before_push_font_ffi() {
    let ctx = crate::Context::create();
    let font_id = {
        let fonts = ctx.font_atlas();
        fonts.add_font(&[FontSource::default_font()])
    };
    {
        let fonts = ctx.font_atlas();
        fonts.clear_fonts();
    }

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = validate_font_id_for_current_context(font_id, "test stale FontId");
    }));

    assert!(result.is_err());
}

#[test]
fn font_id_from_another_atlas_is_rejected_before_push_font_ffi() {
    let ctx_a = crate::Context::create();
    let font_id = {
        let fonts = ctx_a.font_atlas();
        fonts.add_font(&[FontSource::default_font()])
    };
    let suspended_a = ctx_a.suspend_or_panic();

    let mut ctx_b = crate::Context::create();
    ctx_b
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("legacy renderer font atlas should be available")
        .build();
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
    let raw = shared_atlas.as_ptr();
    let ctx = crate::Context::create_with_shared_font_atlas(shared_atlas.clone());
    let font_id = { ctx.font_atlas().add_font(&[FontSource::default_font()]) };

    let _ = validate_font_id_for_atlas(font_id, raw, "test shared FontId");
}

#[test]
fn font_sources_reject_invalid_sizes_before_ffi() {
    let ctx = crate::Context::create();
    let atlas = ctx.font_atlas();

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = atlas.add_font(&[FontSource::default_font_with_size(f32::NAN)]);
        }))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let source = unsafe { FontSource::ttf_data_with_size(&[0u8; 10], -1.0) };
            let _ = atlas.add_font(&[source]);
        }))
        .is_err()
    );
}

#[test]
fn add_font_validates_every_source_before_mutating_the_atlas() {
    let ctx = crate::Context::create();
    let atlas = ctx.font_atlas();
    let raw = atlas.raw();
    let fonts_before = unsafe { (*raw).Fonts.Size };
    let sources_before = unsafe { (*raw).Sources.Size };

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = atlas.add_font(&[
            FontSource::default_font(),
            FontSource::default_font_with_size(f32::NAN),
        ]);
    }));
    assert!(result.is_err());
    assert_eq!(unsafe { (*raw).Fonts.Size }, fonts_before);
    assert_eq!(unsafe { (*raw).Sources.Size }, sources_before);
}

#[test]
fn add_font_rejects_first_merge_source_before_mutating_an_empty_atlas() {
    let ctx = crate::Context::create();
    let atlas = ctx.font_atlas();
    let raw = atlas.raw();
    let source = FontSource::default_font().with_config(FontConfig::new().merge_mode(true));

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = atlas.add_font(&[source]);
        }))
        .is_err()
    );
    assert_eq!(unsafe { (*raw).Fonts.Size }, 0);
    assert_eq!(unsafe { (*raw).Sources.Size }, 0);
}

#[test]
fn add_font_rejects_explicit_merge_into_implicit_head_before_mutation() {
    let ctx = crate::Context::create();
    let atlas = ctx.font_atlas();
    let raw = atlas.raw();

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = atlas.add_font(&[
                FontSource::default_font(),
                FontSource::default_font_with_size(16.0),
            ]);
        }))
        .is_err()
    );
    assert_eq!(unsafe { (*raw).Fonts.Size }, 0);
    assert_eq!(unsafe { (*raw).Sources.Size }, 0);
}

#[test]
fn explicit_embedded_fonts_and_cache_compaction_are_safe() {
    let ctx = crate::Context::create();

    ctx.font_atlas().compact_cache();
    let bitmap = ctx.font_atlas().add_font(&[FontSource::default_bitmap()]);
    let vector = ctx.font_atlas().add_font(&[FontSource::default_vector()]);
    assert_ne!(bitmap, vector);

    ctx.font_atlas()
        .try_claim_legacy_renderer()
        .expect("legacy renderer font atlas should be available")
        .build();
    ctx.font_atlas().compact_cache();
}

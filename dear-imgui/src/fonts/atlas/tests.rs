use super::id::validate_font_id_for_atlas;
use super::validation::RASTERIZER_MULTIPLY_MAX;
use super::*;

fn reconcile_with_retry(
    frame: crate::render::PendingFrame<'_>,
) -> crate::render::ReconciledFrame<'_> {
    let feedback = frame
        .texture_requests()
        .iter()
        .map(crate::render::TextureRequest::retry)
        .collect::<Vec<_>>();
    frame
        .reconcile_texture_feedback(feedback)
        .expect("explicit retry outcomes must reconcile the frame")
}

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
    let ctx = crate::Context::create();
    let atlas = ctx.font_atlas();
    let cfg = FontConfig::new().glyph_offset([1.0, 0.0]);

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = unsafe { atlas.add_font_from_memory_ttf(&[0u8; 10], 0.0, Some(&cfg), None) };
        }))
        .is_err()
    );

    assert!(
        unsafe { atlas.add_font_from_memory_ttf(&[0u8; 10], 13.0, Some(&cfg), None) }.is_none()
    );
}

#[test]
fn add_font_with_config_rejects_missing_font_source_before_ffi() {
    let ctx = crate::Context::create();
    let atlas = ctx.font_atlas();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = atlas.add_font_with_config(&FontConfig::new());
        }))
        .is_err()
    );
}

#[test]
fn add_font_with_config_rejects_builtin_stb_without_font_data() {
    let ctx = crate::Context::create();
    let atlas = ctx.font_atlas();
    let config = FontConfig::new().font_loader(FontLoader::stb_truetype());
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = atlas.add_font_with_config(&config);
        }))
        .is_err()
    );
}

#[test]
fn add_font_from_memory_ttf_rejects_too_small_buffers() {
    let ctx = crate::Context::create();
    let fonts = ctx.font_atlas();
    assert!(unsafe { fonts.add_font_from_memory_ttf(&[0u8; 10], 13.0, None, None) }.is_none());
}

#[test]
fn compressed_font_entry_points_reject_truncated_headers_before_ffi() {
    let ctx = crate::Context::create();
    let fonts = ctx.font_atlas();

    assert!(
        unsafe { fonts.add_font_from_memory_compressed_ttf(&[0; 15], 13.0, None, None) }.is_none()
    );
    assert!(
        unsafe { fonts.add_font_from_memory_compressed_base85_ttf("!!!!!", 13.0, None, None,) }
            .is_none()
    );
}

#[test]
fn add_font_from_file_ttf_returns_none_before_ffi_when_file_is_missing() {
    let ctx = crate::Context::create();
    let fonts = ctx.font_atlas();
    assert!(
        unsafe {
            fonts.add_font_from_file_ttf("this-font-file-must-not-exist.ttf", 13.0, None, None)
        }
        .is_none()
    );
}

#[test]
fn file_font_entry_points_preserve_default_and_explicit_debug_names() {
    let ctx = crate::Context::create();
    let atlas = ctx.font_atlas();
    let font_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/ProggyClean.ttf");
    let font_path = font_path
        .to_str()
        .expect("the repository font path must be valid UTF-8");
    let debug_name = |font_id: crate::FontId| font_id.debug_name();

    let direct = unsafe { atlas.add_font_from_file_ttf(font_path, 13.0, None, None) }
        .expect("the direct file font should load");
    assert_eq!(debug_name(direct), "ProggyClean.ttf");

    let source = unsafe { FontSource::ttf_file_with_size(font_path, 13.0) };
    let structured = atlas.add_font(&[source]);
    assert_eq!(debug_name(structured), "ProggyClean.ttf");

    let config = FontConfig::new().name("custom debug name");
    let named = unsafe { atlas.add_font_from_file_ttf(font_path, 13.0, Some(&config), None) }
        .expect("the explicitly named file font should load");
    assert_eq!(debug_name(named), "custom debug name");
}

#[test]
fn add_font_keeps_structured_glyph_ranges_alive_until_atlas_clear() {
    const FONT_DATA: &[u8] = include_bytes!(
        "../../../../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/ProggyClean.ttf"
    );

    let ctx = crate::Context::create();
    let atlas = ctx.font_atlas();
    let ranges = vec![(0x20, 0x7e)];
    let font =
        unsafe { atlas.add_font_from_memory_ttf(FONT_DATA, 13.0, None, Some(ranges.as_slice())) };
    assert!(font.is_some());
    drop(ranges);

    let sources = unsafe { &(*atlas.raw()).Sources };
    assert_eq!(sources.Size, 1);
    let stored_ranges = unsafe { (*sources.Data).GlyphRanges };
    assert!(!stored_ranges.is_null());
    assert_eq!(unsafe { *stored_ranges }, 0x20 as sys::ImWchar);
    assert_eq!(unsafe { *stored_ranges.add(1) }, 0x7e as sys::ImWchar);
    assert_eq!(unsafe { *stored_ranges.add(2) }, 0);
    assert!(atlas.build());

    atlas.clear();
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
    let _ = ctx_b.font_atlas().build();
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
fn add_font_reads_every_file_before_mutating_the_atlas() {
    let ctx = crate::Context::create();
    let atlas = ctx.font_atlas();
    let raw = atlas.raw();
    let missing = unsafe { FontSource::ttf_file("this-font-file-must-not-exist.ttf") };

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = atlas.add_font(&[FontSource::default_font_with_size(13.0), missing]);
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
    let bitmap = ctx.font_atlas().add_font_default_bitmap(None);
    let vector = ctx.font_atlas().add_font_default_vector(None);
    assert_ne!(bitmap, vector);

    assert!(ctx.font_atlas().build());
    ctx.font_atlas().compact_cache();
}

#[test]
fn set_texture_id_preserves_managed_tex_data_reference() {
    let ctx = crate::Context::create();
    let fonts = ctx.font_atlas();
    let _ = fonts.build();

    let texture = fonts
        .tex_data()
        .expect("built atlas should have texture data");
    let raw_tex_data = texture.as_raw();

    let texture_id = crate::texture::TextureId::new(0x1234);
    unsafe {
        // The test models the sole legacy renderer owner for this atlas binding.
        fonts.set_texture_id(texture_id);
    }

    let mut tex_ref = unsafe { (*fonts.raw()).TexRef };
    assert_eq!(tex_ref._TexData.cast_const(), raw_tex_data);
    assert_eq!(fonts.texture_id(), texture_id);
    assert_eq!(texture.tex_id(), texture_id);

    let resolved = unsafe { sys::ImTextureRef_GetTexID(&mut tex_ref) };
    assert_eq!(resolved, sys::ImTextureID::from(texture_id));
}

#[test]
fn texture_lease_blocks_pixel_invalidation_until_drop() {
    let ctx = crate::Context::create();
    let atlas = ctx.font_atlas();
    assert!(atlas.build());

    let texture = atlas
        .tex_data()
        .expect("built atlas should have texture data");
    let pixels = texture
        .pixels()
        .expect("built atlas should have texture pixels");
    let first_pixel = pixels[0];

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        atlas.clear();
    }));
    assert!(result.is_err());
    assert_eq!(pixels[0], first_pixel);

    drop(texture);
    atlas.clear();
}

#[test]
fn forgotten_texture_lease_blocks_frame_advance_before_ffi() {
    let mut ctx = crate::Context::create();
    assert!(ctx.font_atlas().build());
    ctx.io_mut().set_display_size([128.0, 128.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);

    let texture = ctx
        .font_atlas()
        .tex_data()
        .expect("built atlas should have texture data");
    std::mem::forget(texture);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ctx.frame();
    }));
    assert!(result.is_err());
    assert_eq!(
        ctx.frame_lifecycle_state(),
        crate::FrameLifecycleState::Idle
    );
}

#[test]
fn clear_tex_data_rejects_a_managed_renderer_before_ffi() {
    let ctx = crate::Context::create();
    let atlas = ctx.font_atlas();
    assert!(atlas.build());
    let was_built = atlas.is_built();

    unsafe { (*atlas.raw()).RendererHasTextures = true };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        atlas.clear_tex_data();
    }));

    assert!(result.is_err());
    assert_eq!(atlas.is_built(), was_built);
    assert!(atlas.tex_data().is_some());
}

#[test]
fn structural_mutation_rejects_a_locked_legacy_frame_before_ffi() {
    let mut ctx = crate::Context::create();
    let _ = ctx.font_atlas().build();
    ctx.io_mut().set_display_size([128.0, 128.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);
    ctx.frame().text("open frame");

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ctx.font_atlas().clear_fonts();
    }));
    assert!(result.is_err());

    let _ = ctx.render_legacy();
}

#[test]
fn every_structural_mutator_rejects_an_unlocked_managed_frame_before_ffi() {
    let mut ctx = crate::Context::create();
    let consumer = ctx
        .create_synchronous_renderer_consumer()
        .expect("the managed renderer consumer should attach");
    let _ = ctx.font_atlas().add_font_default(None);
    ctx.io_mut().set_display_size([128.0, 128.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);
    ctx.io_mut()
        .set_backend_flags(crate::BackendFlags::RENDERER_HAS_TEXTURES);
    ctx.frame().text("open managed frame");

    let raw = ctx.font_atlas().raw();
    assert!(!unsafe { (*raw).Locked });
    let font_count = unsafe { (*raw).Fonts.Size };
    let add_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ctx.font_atlas().add_font_default(None);
    }));
    let clear_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ctx.font_atlas().clear();
    }));

    assert!(add_result.is_err());
    assert!(clear_result.is_err());
    assert_eq!(unsafe { (*raw).Fonts.Size }, font_count);
    let _ = ctx.render(&consumer);
}

#[test]
fn owned_atlas_rejects_legacy_to_managed_without_repopulation() {
    let mut ctx = crate::Context::create();
    let consumer = ctx
        .create_synchronous_renderer_consumer()
        .expect("the managed renderer consumer should attach");
    assert!(ctx.font_atlas().build());
    ctx.io_mut().set_display_size([128.0, 128.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);
    ctx.io_mut()
        .set_backend_flags(crate::BackendFlags::RENDERER_HAS_TEXTURES);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ctx.frame();
    }));
    assert!(result.is_err());

    ctx.font_atlas().clear();
    let _ = ctx.font_atlas().add_font_default(None);
    ctx.frame().text("managed after repopulation");
    assert!(reconcile_with_retry(ctx.render(&consumer)).valid());
}

#[test]
fn owned_atlas_rejects_legacy_to_managed_after_adding_another_font() {
    let mut ctx = crate::Context::create();
    assert!(ctx.font_atlas().build());
    let raw = ctx.font_atlas().raw();
    let builder = unsafe { (*raw).Builder };
    assert!(!builder.is_null());
    assert!(unsafe { (*builder).PreloadedAllGlyphsRanges });

    let _ = ctx.font_atlas().add_font_default(None);
    assert!(!unsafe { (*raw).TexIsBuilt });
    assert!(unsafe { (*builder).PreloadedAllGlyphsRanges });

    ctx.io_mut().set_display_size([128.0, 128.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);
    ctx.io_mut()
        .set_backend_flags(crate::BackendFlags::RENDERER_HAS_TEXTURES);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ctx.frame();
    }));
    assert!(result.is_err());
    assert_eq!(
        ctx.frame_lifecycle_state(),
        crate::FrameLifecycleState::Idle
    );
}

#[test]
fn owned_atlas_keeps_managed_mode_for_the_context_lifetime() {
    let mut ctx = crate::Context::create();
    let consumer = ctx
        .create_synchronous_renderer_consumer()
        .expect("the managed renderer consumer should attach");
    let _ = ctx.font_atlas().add_font_default(None);
    ctx.io_mut().set_display_size([128.0, 128.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);
    ctx.io_mut()
        .set_backend_flags(crate::BackendFlags::RENDERER_HAS_TEXTURES);
    ctx.frame().text("managed atlas");
    let _ = ctx.render(&consumer);

    ctx.io_mut().set_backend_flags(crate::BackendFlags::empty());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ctx.frame();
    }));
    assert!(result.is_err());

    assert!(ctx.font_atlas().build());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ctx.frame();
    }));
    assert!(result.is_err());
}

#[test]
fn shared_atlas_updates_once_per_context_frame_and_has_one_owner() {
    let shared_atlas = SharedFontAtlas::create();
    let raw = shared_atlas.as_ptr();
    let mut ctx_a = crate::Context::create_with_shared_font_atlas(shared_atlas.clone());
    assert_eq!(unsafe { (*raw).RefCount }, 1);

    assert!(ctx_a.font_atlas().build());
    ctx_a.io_mut().set_display_size([128.0, 128.0]);
    ctx_a.io_mut().set_delta_time(1.0 / 60.0);
    ctx_a.frame().text("context A");
    let _ = ctx_a.render_legacy();
    assert_eq!(unsafe { (*(*raw).Builder).FrameCount }, 0);

    let suspended_a = ctx_a.suspend_or_panic();
    let mut ctx_b = crate::Context::create_with_shared_font_atlas(shared_atlas.clone());
    assert_eq!(unsafe { (*raw).RefCount }, 2);
    ctx_b.io_mut().set_display_size([128.0, 128.0]);
    ctx_b.io_mut().set_delta_time(1.0 / 60.0);
    ctx_b.frame().text("context B");
    let _ = ctx_b.render_legacy();
    assert_eq!(unsafe { (*(*raw).Builder).FrameCount }, 1);

    drop(ctx_b);
    assert_eq!(unsafe { (*raw).RefCount }, 1);
    let ctx_a = suspended_a
        .activate()
        .expect("the first shared-atlas context should reactivate");
    drop(ctx_a);
    assert_eq!(unsafe { (*raw).RefCount }, 0);

    drop(shared_atlas);
}

#[test]
fn shared_atlas_is_legacy_only_until_one_context_remains() {
    let shared_atlas = SharedFontAtlas::create();
    let raw = shared_atlas.as_ptr();
    let mut ctx_a = crate::Context::create_with_shared_font_atlas(shared_atlas.clone());
    assert!(ctx_a.font_atlas().build());
    ctx_a.io_mut().set_display_size([128.0, 128.0]);
    ctx_a.io_mut().set_delta_time(1.0 / 60.0);
    ctx_a.frame().text("legacy context A");
    let _ = ctx_a.render_legacy();
    let suspended_a = ctx_a.suspend_or_panic();

    let mut ctx_b = crate::Context::create_with_shared_font_atlas(shared_atlas.clone());
    ctx_b.io_mut().set_display_size([128.0, 128.0]);
    ctx_b.io_mut().set_delta_time(1.0 / 60.0);
    ctx_b.frame().text("legacy context B");
    let _ = ctx_b.render_legacy();
    let builder = unsafe { (*raw).Builder };
    let frame_count = unsafe { (*builder).FrameCount };
    let texture = unsafe { (*raw).TexData };
    let status = unsafe { (*texture).Status };
    let texture_id = unsafe { (*texture).TexID };

    assert!(matches!(
        ctx_b.create_synchronous_renderer_consumer(),
        Err(
            crate::render::RendererConsumerError::SharedFontAtlasRequiresExclusiveContext {
                registered_contexts: 2,
            }
        )
    ));
    assert_eq!(unsafe { (*builder).FrameCount }, frame_count);
    assert_eq!(unsafe { (*texture).Status }, status);
    assert_eq!(unsafe { (*texture).TexID }, texture_id);

    drop(ctx_b);
    let mut ctx_a = suspended_a.activate().expect("context A should reactivate");
    ctx_a.font_atlas().clear();
    let _ = ctx_a.font_atlas().add_font_default(None);
    let consumer = ctx_a
        .create_synchronous_renderer_consumer()
        .expect("the remaining sole Context may attach a managed renderer");
    ctx_a
        .io_mut()
        .set_backend_flags(crate::BackendFlags::RENDERER_HAS_TEXTURES);
    ctx_a.frame().text("sole managed context");
    assert!(reconcile_with_retry(ctx_a.render(&consumer)).valid());
}

#[test]
fn managed_shared_atlas_rejects_a_second_context_before_native_creation() {
    let shared_atlas = SharedFontAtlas::create();
    let raw = shared_atlas.as_ptr();
    let mut first = crate::Context::create_with_shared_font_atlas(shared_atlas.clone());
    let consumer = first
        .create_synchronous_renderer_consumer()
        .expect("the sole Context should attach");
    let suspended = first.suspend_or_panic();

    assert!(matches!(
        crate::Context::try_create_with_shared_font_atlas(shared_atlas.clone()),
        Err(crate::ImGuiError::SharedFontAtlasManaged)
    ));
    assert_eq!(unsafe { (*raw).RefCount }, 1);

    let mut first = suspended
        .activate()
        .expect("the first Context remains usable");
    let _ = first.font_atlas().add_font_default(None);
    first.io_mut().set_display_size([128.0, 128.0]);
    first.io_mut().set_delta_time(1.0 / 60.0);
    first
        .io_mut()
        .set_backend_flags(crate::BackendFlags::RENDERER_HAS_TEXTURES);
    first.frame().text("still usable");
    assert!(reconcile_with_retry(first.render(&consumer)).valid());
}

#[test]
fn shared_atlas_assigns_a_fresh_managed_namespace_after_reentry() {
    fn capture_namespace(
        context: &mut crate::Context,
        consumer: &crate::render::SynchronousRendererConsumer,
    ) -> u64 {
        context.io_mut().set_display_size([128.0, 128.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .io_mut()
            .set_backend_flags(crate::BackendFlags::RENDERER_HAS_TEXTURES);
        context.frame().text("managed namespace");
        let pending = context.render(consumer);
        let namespace = pending
            .texture_requests()
            .iter()
            .find_map(|request| match request.texture() {
                crate::render::SnapshotTextureId::FontAtlas { stamp, .. } => Some(stamp),
                crate::render::SnapshotTextureId::User(_) => None,
            })
            .expect("managed atlas should emit a texture request");
        drop(pending);
        context
            .prepare_renderer_texture_reset(consumer)
            .unwrap()
            .commit();
        namespace
    }

    let shared_atlas = SharedFontAtlas::create();
    let first_namespace = {
        let mut context = crate::Context::create_with_shared_font_atlas(shared_atlas.clone());
        let consumer = context.create_synchronous_renderer_consumer().unwrap();
        let _ = context.font_atlas().add_font_default(None);
        capture_namespace(&mut context, &consumer)
    };
    let second_namespace = {
        let mut context = crate::Context::create_with_shared_font_atlas(shared_atlas.clone());
        let consumer = context.create_synchronous_renderer_consumer().unwrap();
        capture_namespace(&mut context, &consumer)
    };
    assert_ne!(first_namespace, second_namespace);
}

#[test]
fn shared_atlas_rejects_reentry_when_a_custom_renderer_skips_reset() {
    let shared_atlas = SharedFontAtlas::create();
    let raw = shared_atlas.as_ptr();
    let first_texture;

    {
        let mut context = crate::Context::create_with_shared_font_atlas(shared_atlas.clone());
        let _ = context.font_atlas().add_font_default(None);
        let consumer = context.create_synchronous_renderer_consumer().unwrap();
        context.prepare_frame(
            crate::FramePrepareOptions::new([128.0, 128.0], 1.0 / 60.0).renderer_has_textures(),
        );
        context.frame().text("first managed renderer");
        let pending = context.render(&consumer);
        assert!(pending.texture_requests().iter().any(|request| matches!(
            request.texture(),
            crate::render::SnapshotTextureId::FontAtlas { .. }
        )));
        let feedback = pending
            .texture_requests()
            .iter()
            .enumerate()
            .map(|(index, request)| match request.operation() {
                crate::render::TextureOp::Create { .. }
                | crate::render::TextureOp::Update { .. } => request
                    .uploaded(crate::TextureId::new(4_000 + index as u64))
                    .unwrap(),
                crate::render::TextureOp::Destroy => request.destroyed().unwrap(),
            })
            .collect::<Vec<_>>();
        let reconciled = pending.reconcile_texture_feedback(feedback).unwrap();
        drop(reconciled);

        first_texture = unsafe { (*raw).TexData };
        assert!(!first_texture.is_null());
        assert_eq!(unsafe { (*first_texture).Status }, sys::ImTextureStatus_OK);
        assert_ne!(unsafe { (*first_texture).TexID }, 0 as sys::ImTextureID);
    }

    assert_eq!(
        unsafe { (*first_texture).Status },
        sys::ImTextureStatus_OK,
        "Context teardown must not clear a binding without renderer release proof"
    );
    assert_ne!(unsafe { (*first_texture).TexID }, 0 as sys::ImTextureID);
    assert!(matches!(
        crate::Context::try_create_with_shared_font_atlas(shared_atlas.clone()),
        Err(crate::ImGuiError::SharedFontAtlasRendererReleasePending)
    ));
    assert_eq!(unsafe { (*raw).RefCount }, 0);
}

#[test]
fn shared_atlas_registration_uses_interior_mutability_not_a_unique_rust_borrow() {
    let shared_atlas = SharedFontAtlas::create();
    let raw = shared_atlas.as_ptr();
    let ctx = crate::Context::create_with_shared_font_atlas(shared_atlas.clone());
    let atlas = ctx.font_atlas();

    let suspended = crate::SuspendedContext::create_with_shared_font_atlas(shared_atlas.clone());
    assert_eq!(unsafe { (*raw).RefCount }, 2);
    assert_eq!(unsafe { sys::igGetCurrentContext() }, ctx.as_raw());

    let _ = atlas.add_font_default(None);
    drop(suspended);
    assert_eq!(unsafe { (*raw).RefCount }, 1);
    atlas.compact_cache();

    drop(ctx);
    drop(shared_atlas);
}

#[test]
fn dropping_an_open_shared_legacy_frame_unlocks_the_last_atlas_user() {
    let shared_atlas = SharedFontAtlas::create();
    let raw = shared_atlas.as_ptr();
    {
        let mut ctx = crate::Context::create_with_shared_font_atlas(shared_atlas.clone());
        assert!(ctx.font_atlas().build());
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        ctx.frame().text("open shared frame");
        assert!(unsafe { (*raw).Locked });
    }

    assert_eq!(unsafe { (*raw).RefCount }, 0);
    assert!(!unsafe { (*raw).Locked });

    let ctx = crate::Context::create_with_shared_font_atlas(shared_atlas.clone());
    let _ = ctx.font_atlas().add_font_default(None);
    drop(ctx);
    drop(shared_atlas);
}

#[test]
fn shared_atlas_rejects_mixed_renderer_texture_capabilities() {
    let shared_atlas = SharedFontAtlas::create();
    let mut ctx_a = crate::Context::create_with_shared_font_atlas(shared_atlas.clone());
    assert!(ctx_a.font_atlas().build());
    ctx_a.io_mut().set_display_size([128.0, 128.0]);
    ctx_a.io_mut().set_delta_time(1.0 / 60.0);
    ctx_a.frame().text("legacy renderer");
    let _ = ctx_a.render_legacy();
    let suspended_a = ctx_a.suspend_or_panic();

    let mut ctx_b = crate::Context::create_with_shared_font_atlas(shared_atlas);
    ctx_b.io_mut().set_display_size([128.0, 128.0]);
    ctx_b.io_mut().set_delta_time(1.0 / 60.0);
    let backend_flags = ctx_b.io().backend_flags() | crate::BackendFlags::RENDERER_HAS_TEXTURES;
    ctx_b.io_mut().set_backend_flags(backend_flags);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ctx_b.frame();
    }));
    assert!(result.is_err());

    drop(ctx_b);
    drop(suspended_a);
}

#[test]
fn shared_atlas_allows_mode_change_after_a_committed_renderer_reset() {
    let shared_atlas = SharedFontAtlas::create();
    let raw = shared_atlas.as_ptr();

    {
        let mut legacy = crate::Context::create_with_shared_font_atlas(shared_atlas.clone());
        assert!(legacy.font_atlas().build());
        legacy.io_mut().set_display_size([128.0, 128.0]);
        legacy.io_mut().set_delta_time(1.0 / 60.0);
        legacy.frame().text("legacy renderer");
        let _ = legacy.render_legacy();
    }
    assert_eq!(unsafe { (*raw).RefCount }, 0);

    {
        let mut managed = crate::Context::create_with_shared_font_atlas(shared_atlas.clone());
        managed.font_atlas().clear();
        let _ = managed.font_atlas().add_font_default(None);
        let consumer = managed
            .create_synchronous_renderer_consumer()
            .expect("the managed renderer consumer should attach");
        managed.io_mut().set_display_size([128.0, 128.0]);
        managed.io_mut().set_delta_time(1.0 / 60.0);
        let backend_flags =
            managed.io().backend_flags() | crate::BackendFlags::RENDERER_HAS_TEXTURES;
        managed.io_mut().set_backend_flags(backend_flags);
        managed.frame().text("managed renderer");
        drop(managed.render(&consumer));
        managed
            .prepare_renderer_texture_reset(&consumer)
            .unwrap()
            .commit();
    }
    assert_eq!(unsafe { (*raw).RefCount }, 0);
}

#[test]
fn shared_atlas_rejects_an_incompatible_renderer_transition_before_ffi() {
    let shared_atlas = SharedFontAtlas::create();

    {
        let mut legacy = crate::Context::create_with_shared_font_atlas(shared_atlas.clone());
        assert!(legacy.font_atlas().build());
        legacy.io_mut().set_display_size([128.0, 128.0]);
        legacy.io_mut().set_delta_time(1.0 / 60.0);
        legacy.frame().text("legacy renderer");
        let _ = legacy.render_legacy();
    }

    let mut managed = crate::Context::create_with_shared_font_atlas(shared_atlas);
    managed.io_mut().set_display_size([128.0, 128.0]);
    managed.io_mut().set_delta_time(1.0 / 60.0);
    let backend_flags = managed.io().backend_flags() | crate::BackendFlags::RENDERER_HAS_TEXTURES;
    managed.io_mut().set_backend_flags(backend_flags);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = managed.frame();
    }));
    assert!(result.is_err());
}

#[test]
fn shared_atlas_requires_legacy_preloading_after_managed_use() {
    let shared_atlas = SharedFontAtlas::create();

    {
        let mut managed = crate::Context::create_with_shared_font_atlas(shared_atlas.clone());
        let consumer = managed
            .create_synchronous_renderer_consumer()
            .expect("the managed renderer consumer should attach");
        let _ = managed.font_atlas().add_font_default(None);
        managed.io_mut().set_display_size([128.0, 128.0]);
        managed.io_mut().set_delta_time(1.0 / 60.0);
        managed
            .io_mut()
            .set_backend_flags(crate::BackendFlags::RENDERER_HAS_TEXTURES);
        managed.frame().text("managed atlas");
        drop(managed.render(&consumer));
        managed
            .prepare_renderer_texture_reset(&consumer)
            .unwrap()
            .commit();
    }

    let mut legacy = crate::Context::create_with_shared_font_atlas(shared_atlas);
    legacy.io_mut().set_display_size([128.0, 128.0]);
    legacy.io_mut().set_delta_time(1.0 / 60.0);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = legacy.frame();
    }));
    assert!(result.is_err());

    assert!(legacy.font_atlas().build());
    legacy.frame().text("legacy atlas after preload");
    assert!(legacy.render_legacy().valid());
}

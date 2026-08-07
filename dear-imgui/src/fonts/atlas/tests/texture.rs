#[test]
fn set_texture_id_preserves_legacy_tex_data_reference() {
    let ctx = crate::Context::create();
    let fonts = ctx.font_atlas();
    let legacy = fonts
        .try_claim_legacy_renderer()
        .expect("the test requires a legacy font atlas");
    legacy.build();

    let texture = legacy
        .tex_data()
        .expect("built atlas should have texture data");
    let raw_tex_data = texture.as_raw();

    let texture_id = crate::texture::TextureId::new(0x1234);
    unsafe {
        // The test models the sole legacy renderer owner for this atlas binding.
        legacy.set_texture_id(texture_id);
    }

    let mut tex_ref = unsafe { (*fonts.raw()).TexRef };
    assert_eq!(tex_ref._TexData.cast_const(), raw_tex_data);
    assert_eq!(legacy.texture_id(), texture_id);
    assert_eq!(texture.tex_id(), texture_id);

    let resolved = unsafe { sys::ImTextureRef_GetTexID(&mut tex_ref) };
    assert_eq!(resolved, sys::ImTextureID::from(texture_id));
}

#[test]
fn texture_lease_blocks_pixel_invalidation_until_drop() {
    let ctx = crate::Context::create();
    let atlas = ctx.font_atlas();
    let legacy = atlas
        .try_claim_legacy_renderer()
        .expect("the test requires a legacy font atlas");
    legacy.build();

    let texture = legacy
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
    ctx.font_atlas()
        .try_claim_legacy_renderer()
        .expect("legacy renderer font atlas should be available")
        .build();
    ctx.io_mut().set_display_size([128.0, 128.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);

    let legacy = ctx
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("the test requires a legacy font atlas");
    let texture = legacy
        .tex_data()
        .expect("built atlas should have texture data");
    std::mem::forget(texture);
    drop(legacy);

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
fn legacy_clear_cpu_texture_data_discards_pixels_without_removing_fonts() {
    let ctx = crate::Context::create();
    let atlas = ctx.font_atlas();
    let legacy = atlas
        .try_claim_legacy_renderer()
        .expect("the test requires a legacy font atlas");
    legacy.build();
    assert!(legacy.is_built());
    let source_count = unsafe { (*atlas.raw()).Sources.Size };
    assert!(
        legacy
            .tex_data()
            .is_some_and(|texture| texture.pixels().is_some())
    );

    legacy.clear_cpu_texture_data();

    assert_eq!(unsafe { (*atlas.raw()).Sources.Size }, source_count);
    assert!(
        legacy
            .tex_data()
            .is_some_and(|texture| texture.pixels().is_none())
    );
}

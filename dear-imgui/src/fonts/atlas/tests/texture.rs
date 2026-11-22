use super::reconcile_with_retry;

#[test]
fn frame_atlas_image_supports_managed_text_and_image_submission() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = crate::Context::create();
    let consumer = ctx.create_synchronous_renderer_consumer().unwrap();
    ctx.prepare_frame(
        crate::FramePrepareOptions::new([128.0, 128.0], 1.0 / 60.0).renderer_has_textures(),
    );

    let ui = ctx.frame();
    let image: crate::TextureRef<'_> = ui.font_atlas_texture().unwrap();
    ui.text("Draw text before submitting the atlas image");
    ui.image(image, [16.0, 16.0]);
    assert!(reconcile_with_retry(ctx.render(&consumer)).valid());
}

#[test]
fn legacy_atlas_pixels_can_be_borrowed_after_ending_the_frame() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = crate::Context::create();
    ctx.font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
    ctx.prepare_frame(crate::FramePrepareOptions::new([128.0, 128.0], 1.0 / 60.0));
    ctx.frame();

    let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let legacy = ctx.font_atlas().try_claim_legacy_renderer().unwrap();
        let _ = legacy.tex_data();
    }));
    assert!(rejected.is_err());
    assert!(ctx.end_frame());

    let legacy = ctx.font_atlas().try_claim_legacy_renderer().unwrap();
    let texture = legacy.tex_data().unwrap();
    assert!(!texture.pixels().unwrap().is_empty());
}

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
fn cleared_legacy_atlas_rebuilds_uploadable_pixels() {
    let _guard = crate::test_support::imgui_context_guard();
    let ctx = crate::Context::create();
    let atlas = ctx.font_atlas();
    let legacy = atlas
        .try_claim_legacy_renderer()
        .expect("the test requires a legacy font atlas");
    legacy.build();

    atlas.clear();
    let font = atlas.add_font(&[crate::FontSource::default_bitmap()]);
    legacy.build();

    assert!(font.is_loaded());
    let texture = legacy.tex_data().unwrap();
    let pixels = texture.pixels().unwrap();
    assert!(!pixels.is_empty());
    assert_eq!(pixels.len(), texture.pitch() * texture.height() as usize);
}

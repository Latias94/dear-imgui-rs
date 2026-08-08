use super::*;

#[test]
fn structural_mutation_rejects_a_locked_legacy_frame_before_ffi() {
    let mut ctx = crate::Context::create();
    ctx.font_atlas()
        .try_claim_legacy_renderer()
        .expect("legacy renderer font atlas should be available")
        .build();
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
    let _ = ctx.font_atlas().add_font(&[FontSource::default_font()]);
    ctx.io_mut().set_display_size([128.0, 128.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);
    ctx.io_mut()
        .set_backend_flags(crate::BackendFlags::RENDERER_HAS_TEXTURES);
    ctx.frame().text("open managed frame");

    let raw = ctx.font_atlas().raw();
    assert!(!unsafe { (*raw).Locked });
    let font_count = unsafe { (*raw).Fonts.Size };
    let add_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ctx.font_atlas().add_font(&[FontSource::default_font()]);
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
fn managed_atlas_rejects_legacy_capability_at_the_request() {
    let mut ctx = crate::Context::create();
    let consumer = ctx
        .create_synchronous_renderer_consumer()
        .expect("the managed renderer consumer should attach");

    assert_eq!(
        ctx.font_atlas().try_claim_legacy_renderer().unwrap_err(),
        crate::FontAtlasModeError::ManagedRendererActive
    );

    ctx.io_mut().set_display_size([128.0, 128.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);
    ctx.io_mut()
        .set_backend_flags(crate::BackendFlags::RENDERER_HAS_TEXTURES);
    ctx.frame().text("managed mode remained usable");
    assert!(reconcile_with_retry(ctx.render(&consumer)).valid());
}

#[test]
fn unclaimed_managed_frame_requires_an_explicit_renderer_consumer() {
    let mut ctx = crate::Context::create();
    ctx.io_mut().set_display_size([128.0, 128.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);
    ctx.io_mut()
        .set_backend_flags(crate::BackendFlags::RENDERER_HAS_TEXTURES);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ctx.frame();
    }));
    assert!(
        result.is_err(),
        "RENDERER_HAS_TEXTURES must not implicitly claim managed atlas ownership"
    );

    let _consumer = ctx
        .create_synchronous_renderer_consumer()
        .expect("a rejected frame must leave the atlas available for explicit managed admission");
}

#[test]
fn rejected_unclaimed_legacy_frame_does_not_poison_the_atlas_mode() {
    let mut ctx = crate::Context::create();
    ctx.io_mut().set_display_size([128.0, 128.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ctx.frame();
    }));
    assert!(result.is_err());

    let _consumer = ctx
        .create_synchronous_renderer_consumer()
        .expect("a rejected legacy frame must not implicitly claim legacy atlas ownership");
}

#[test]
fn owned_atlas_rejects_legacy_to_managed_after_adding_another_font() {
    let mut ctx = crate::Context::create();
    ctx.font_atlas()
        .try_claim_legacy_renderer()
        .expect("a fresh atlas should allow legacy rendering")
        .build();
    let raw = ctx.font_atlas().raw();
    let builder = unsafe { (*raw).Builder };
    assert!(!builder.is_null());
    assert!(unsafe { (*builder).PreloadedAllGlyphsRanges });

    let _ = ctx.font_atlas().add_font(&[FontSource::default_font()]);
    assert!(!unsafe { (*raw).TexIsBuilt });
    assert!(unsafe { (*builder).PreloadedAllGlyphsRanges });

    assert_eq!(
        ctx.create_synchronous_renderer_consumer().unwrap_err(),
        crate::RendererConsumerError::FontAtlasRequiresManagedRebuild
    );

    ctx.font_atlas().clear();
    let _ = ctx.font_atlas().add_font(&[FontSource::default_font()]);
    let _consumer = ctx
        .create_synchronous_renderer_consumer()
        .expect("a fully cleared and repopulated atlas should allow managed rendering");
}

#[test]
fn live_legacy_capability_keeps_the_atlas_claimed_across_clear() {
    let mut ctx = crate::Context::create();
    let legacy = ctx
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("a fresh atlas should allow legacy rendering");

    ctx.font_atlas().clear();
    drop(legacy);

    assert_eq!(
        ctx.create_synchronous_renderer_consumer().unwrap_err(),
        crate::RendererConsumerError::FontAtlasRequiresManagedRebuild
    );

    ctx.font_atlas().clear();
    let _consumer = ctx
        .create_synchronous_renderer_consumer()
        .expect("a clear after every legacy capability is dropped should release legacy mode");
}

#[test]
fn owned_atlas_keeps_managed_mode_for_the_context_lifetime() {
    let mut ctx = crate::Context::create();
    let consumer = ctx
        .create_synchronous_renderer_consumer()
        .expect("the managed renderer consumer should attach");
    let _ = ctx.font_atlas().add_font(&[FontSource::default_font()]);
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

    assert_eq!(
        ctx.font_atlas().try_claim_legacy_renderer().unwrap_err(),
        crate::FontAtlasModeError::ManagedRendererActive
    );
}

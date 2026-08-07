use super::*;

#[test]
fn shared_atlas_updates_once_per_context_frame_and_has_one_owner() {
    let shared_atlas = SharedFontAtlas::create();
    let raw = shared_atlas.as_ptr();
    let mut ctx_a = crate::Context::create_with_shared_font_atlas(shared_atlas.clone());
    assert_eq!(unsafe { (*raw).RefCount }, 1);

    ctx_a
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("a shared atlas starts in legacy mode")
        .build();
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
    ctx_a
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("legacy renderer font atlas should be available")
        .build();
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
    let _ = ctx_a.font_atlas().add_font(&[FontSource::default_font()]);
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
    let _ = first.font_atlas().add_font(&[FontSource::default_font()]);
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
        let _ = context.font_atlas().add_font(&[FontSource::default_font()]);
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
        let _ = context.font_atlas().add_font(&[FontSource::default_font()]);
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
    // SAFETY: `shared_atlas` owns `raw` for the duration of this scoped capability check.
    let atlas = unsafe { FontAtlas::from_raw(raw) };
    assert_eq!(
        atlas.try_claim_legacy_renderer().unwrap_err(),
        crate::FontAtlasModeError::RendererReleasePending
    );
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

    let _ = atlas.add_font(&[FontSource::default_font()]);
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
        ctx.font_atlas()
            .try_claim_legacy_renderer()
            .expect("legacy renderer font atlas should be available")
            .build();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        ctx.frame().text("open shared frame");
        assert!(unsafe { (*raw).Locked });
    }

    assert_eq!(unsafe { (*raw).RefCount }, 0);
    assert!(!unsafe { (*raw).Locked });

    let ctx = crate::Context::create_with_shared_font_atlas(shared_atlas.clone());
    let _ = ctx.font_atlas().add_font(&[FontSource::default_font()]);
    drop(ctx);
    drop(shared_atlas);
}

#[test]
fn shared_atlas_rejects_mixed_renderer_texture_capabilities() {
    let shared_atlas = SharedFontAtlas::create();
    let mut ctx_a = crate::Context::create_with_shared_font_atlas(shared_atlas.clone());
    ctx_a
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("legacy renderer font atlas should be available")
        .build();
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
        legacy
            .font_atlas()
            .try_claim_legacy_renderer()
            .expect("legacy renderer font atlas should be available")
            .build();
        legacy.io_mut().set_display_size([128.0, 128.0]);
        legacy.io_mut().set_delta_time(1.0 / 60.0);
        legacy.frame().text("legacy renderer");
        let _ = legacy.render_legacy();
    }
    assert_eq!(unsafe { (*raw).RefCount }, 0);

    {
        let mut managed = crate::Context::create_with_shared_font_atlas(shared_atlas.clone());
        managed.font_atlas().clear();
        let _ = managed.font_atlas().add_font(&[FontSource::default_font()]);
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
        legacy
            .font_atlas()
            .try_claim_legacy_renderer()
            .expect("legacy renderer font atlas should be available")
            .build();
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
        let _ = managed.font_atlas().add_font(&[FontSource::default_font()]);
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

    legacy
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("legacy renderer font atlas should be available")
        .build();
    legacy.frame().text("legacy atlas after preload");
    assert!(legacy.render_legacy().valid());
}

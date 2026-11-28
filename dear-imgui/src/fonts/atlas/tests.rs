use super::config::RASTERIZER_MULTIPLY_MAX;
use super::ownership::validate_font_id_for_atlas;
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

mod checked_bytes {
    use super::*;

    const PROGGY: &[u8] = include_bytes!(
        "../../../../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/ProggyClean.ttf"
    );

    #[test]
    fn checked_bytes_are_copied_for_deferred_glyph_access() {
        let context = crate::Context::create();
        let atlas = context.font_atlas();
        let bytes = PROGGY.to_vec();

        let font = atlas
            .try_add_font(&[FontSource::ttf_bytes(&bytes)])
            .expect("the built-in atlas loader should accept the test font");
        let source = unsafe { &*(*atlas.raw()).Sources.Data };
        assert!(source.FontDataOwnedByAtlas);
        assert_ne!(source.FontData.cast::<u8>().cast_const(), bytes.as_ptr());
        drop(bytes);

        atlas.try_claim_legacy_renderer().unwrap().build();
        assert_eq!(font.reference_size(), Some(13.0));
        assert!(font.is_glyph_in_font('A'));
    }

    #[test]
    fn checked_bytes_keep_the_stb_proof_when_atlas_selects_stb() {
        let context = crate::Context::create();
        let atlas = context.font_atlas();
        let stb = FontLoader::stb_truetype();
        atlas.set_font_loader(stb).unwrap();
        let raw = atlas.raw();

        let invalid_font = [0; 128];
        assert!(matches!(
            atlas.try_add_font(&[
                FontSource::default_font(),
                FontSource::ttf_bytes(&invalid_font),
            ]),
            Err(FontError::UnsupportedContainer { signature: 0 })
        ));
        assert_eq!(unsafe { (*raw).Fonts.Size }, 0);
        assert_eq!(unsafe { (*raw).Sources.Size }, 0);
        assert!(matches!(
            atlas.try_add_font(&[FontSource::ttf_bytes(PROGGY).font_index(1)]),
            Err(FontError::InvalidFontIndex { index: 1 })
        ));

        let font = atlas
            .try_add_font(&[FontSource::ttf_bytes(PROGGY)])
            .unwrap();
        assert_eq!(unsafe { (*(*raw).Sources.Data).FontLoader }, stb.as_ptr());
        assert!(font.is_glyph_in_font('A'));
    }

    #[test]
    fn checked_bytes_cannot_override_or_use_a_custom_loader() {
        let _guard = crate::test_support::imgui_context_guard();
        let stb = FontLoader::stb_truetype();
        let custom = unsafe { *stb.as_ptr() };
        let context = crate::Context::create();
        let override_config = FontConfig::new().font_loader(stb);
        assert!(matches!(
            context
                .font_atlas()
                .try_add_font(&[FontSource::ttf_bytes(PROGGY).with_config(override_config)]),
            Err(FontError::LoaderOverride)
        ));

        let atlas = context.font_atlas();
        unsafe { sys::ImFontAtlas_SetFontLoader(atlas.raw(), &custom) };
        let result = atlas.try_add_font(&[FontSource::ttf_bytes(PROGGY)]);
        unsafe { sys::ImFontAtlas_SetFontLoader(atlas.raw(), stb.as_ptr()) };
        assert!(matches!(result, Err(FontError::UnsupportedAtlasLoader)));
    }

    #[cfg(all(feature = "freetype", not(target_arch = "wasm32")))]
    #[test]
    fn checked_bytes_load_the_requested_freetype_collection_face() {
        const ROBOTO: &[u8] = include_bytes!(
            "../../../../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/Roboto-Medium.ttf"
        );

        let context = crate::Context::create();
        let atlas = context.font_atlas();
        let raw = atlas.raw();
        let collection = make_collection([PROGGY, ROBOTO]);
        assert!(matches!(
            atlas.try_add_font(&[FontSource::ttf_bytes(&collection).font_index(2)]),
            Err(FontError::InvalidFontIndex { index: 2 })
        ));
        const NAMED_INSTANCE_INDEX: u32 = 1 << 16;
        assert!(matches!(
            atlas.try_add_font(&[
                FontSource::ttf_bytes(&collection).font_index(NAMED_INSTANCE_INDEX)
            ]),
            Err(FontError::InvalidFontIndex {
                index: NAMED_INSTANCE_INDEX
            })
        ));
        assert_eq!(unsafe { (*raw).Sources.Size }, 0);

        let first = atlas
            .try_add_font(&[FontSource::ttf_bytes(&collection)])
            .unwrap();
        let second = atlas
            .try_add_font(&[FontSource::ttf_bytes(&collection).font_index(1)])
            .unwrap();
        let source = unsafe { &*(*raw).Sources.Data.add(1) };
        assert!(source.FontLoader.is_null());
        assert_eq!(source.FontNo, 1);
        drop(collection);

        assert!(first.is_glyph_in_font('A'));
        assert!(!first.is_glyph_in_font('Ж'));
        assert!(second.is_glyph_in_font('Ж'));
    }

    #[cfg(all(feature = "freetype", not(target_arch = "wasm32")))]
    #[test]
    fn checked_bytes_report_native_freetype_rejection() {
        let context = crate::Context::create();
        let atlas = context.font_atlas();
        let mut no_tables = [0; 128];
        no_tables[..4].copy_from_slice(b"\0\x01\0\0");

        assert!(matches!(
            atlas.try_add_font(&[FontSource::ttf_bytes(&no_tables)]),
            Err(FontError::NativeRejected)
        ));
        assert_eq!(unsafe { (*atlas.raw()).Fonts.Size }, 0);
        assert_eq!(unsafe { (*atlas.raw()).Sources.Size }, 0);
        assert!(atlas.try_add_font(&[FontSource::ttf_bytes(PROGGY)]).is_ok());
    }

    #[cfg(all(feature = "freetype", not(target_arch = "wasm32")))]
    #[test]
    fn checked_bytes_reject_a_collection_with_a_truncated_offset_directory() {
        let context = crate::Context::create();
        let atlas = context.font_atlas();
        let mut collection = make_collection([PROGGY, PROGGY]);
        collection[8..12].copy_from_slice(&u32::MAX.to_be_bytes());

        assert!(matches!(
            atlas.try_add_font(&[FontSource::ttf_bytes(&collection)]),
            Err(FontError::InvalidSfnt)
        ));
        assert_eq!(unsafe { (*atlas.raw()).Sources.Size }, 0);
    }

    #[cfg(all(feature = "freetype", not(target_arch = "wasm32")))]
    #[test]
    fn checked_bytes_reject_explicit_merge_into_an_implicit_freetype_face() {
        let context = crate::Context::create();
        let atlas = context.font_atlas();
        let source = FontSource::ttf_bytes(PROGGY);
        let merge = FontSource::ttf_bytes(PROGGY).with_config(FontConfig::new().size_pixels(16.0));

        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = atlas.try_add_font(&[source, merge]);
            }))
            .is_err()
        );
        assert_eq!(unsafe { (*atlas.raw()).Sources.Size }, 0);
    }

    #[cfg(all(feature = "freetype", not(target_arch = "wasm32")))]
    fn make_collection(fonts: [&[u8]; 2]) -> Vec<u8> {
        let mut collection = b"ttcf\0\x01\0\0\0\0\0\x02\0\0\0\0\0\0\0\0".to_vec();
        for (index, font) in fonts.into_iter().enumerate() {
            let offset = collection.len().next_multiple_of(4);
            collection.resize(offset, 0);
            collection.extend_from_slice(font);
            let offset_record = 12 + index * 4;
            collection[offset_record..offset_record + 4]
                .copy_from_slice(&(offset as u32).to_be_bytes());

            let table_count = usize::from(u16::from_be_bytes([font[4], font[5]]));
            let directory = &mut collection[offset + 12..offset + 12 + table_count * 16];
            for record in directory.as_chunks_mut::<16>().0 {
                let table_offset = u32::from_be_bytes(record[8..12].try_into().unwrap());
                record[8..12].copy_from_slice(&(table_offset + offset as u32).to_be_bytes());
            }
        }
        collection
    }
}

mod mode {
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

        assert!(matches!(
            ctx.font_atlas().try_claim_legacy_renderer(),
            Err(FontError::ManagedRendererActive)
        ));

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

        let _consumer = ctx.create_synchronous_renderer_consumer().expect(
            "a rejected frame must leave the atlas available for explicit managed admission",
        );
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
            crate::FrameError::FontAtlasRequiresManagedRebuild
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
            crate::FrameError::FontAtlasRequiresManagedRebuild
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

        assert!(matches!(
            ctx.font_atlas().try_claim_legacy_renderer(),
            Err(FontError::ManagedRendererActive)
        ));
    }
}

mod shared {
    use super::*;

    #[test]
    fn shared_atlas_keeps_its_owner_thread_after_the_last_context_drops() {
        let _guard = crate::test_support::imgui_context_guard();
        let context = crate::Context::create_with_shared_font_atlas(SharedFontAtlas::create());
        let retained = context.clone_shared_font_atlas().unwrap();
        drop(context);

        std::thread::spawn(|| {
            assert!(matches!(
                crate::Context::try_create(),
                Err(crate::ContextError::ThreadConflict)
            ));
            assert!(matches!(
                SharedFontAtlas::try_create(),
                Err(FontError::Context(crate::ContextError::ThreadConflict))
            ));
        })
        .join()
        .unwrap();

        drop(retained);
        std::thread::spawn(|| {
            let atlas = SharedFontAtlas::try_create().unwrap();
            let _context = crate::Context::try_create_with_shared_font_atlas(atlas).unwrap();
        })
        .join()
        .unwrap();
    }

    #[test]
    fn standalone_font_constructors_respect_a_live_context_on_another_thread() {
        let _guard = crate::test_support::imgui_context_guard();
        let _context = crate::Context::create();

        std::thread::spawn(|| {
            assert!(matches!(
                SharedFontAtlas::try_create(),
                Err(FontError::Context(crate::ContextError::ThreadConflict))
            ));
            let Err(error) = FontLoader::try_stb_truetype() else {
                panic!("another thread owns the native font loader");
            };
            assert!(matches!(
                &error,
                FontError::Context(crate::ContextError::ThreadConflict)
            ));
            assert!(matches!(
                std::error::Error::source(&error)
                    .and_then(|source| source.downcast_ref::<crate::ContextError>()),
                Some(crate::ContextError::ThreadConflict)
            ));

            let data = StbTrueTypeFontData::from_slice(include_bytes!(
                "../../../../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/ProggyClean.ttf"
            ))
            .unwrap();
            let _source = FontSource::stb_truetype(data).with_config(FontConfig::default());
        })
        .join()
        .unwrap();
    }

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

        let suspended_a = ctx_a;
        let mut ctx_b = crate::Context::create_with_shared_font_atlas(shared_atlas.clone());
        assert_eq!(unsafe { (*raw).RefCount }, 2);
        ctx_b.io_mut().set_display_size([128.0, 128.0]);
        ctx_b.io_mut().set_delta_time(1.0 / 60.0);
        ctx_b.frame().text("context B");
        let _ = ctx_b.render_legacy();
        assert_eq!(unsafe { (*(*raw).Builder).FrameCount }, 1);

        drop(ctx_b);
        assert_eq!(unsafe { (*raw).RefCount }, 1);
        let ctx_a = suspended_a;
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
        let suspended_a = ctx_a;

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
            Err(crate::FrameError::SharedFontAtlasRequiresExclusiveContext {
                registered_contexts: 2,
            })
        ));
        assert_eq!(unsafe { (*builder).FrameCount }, frame_count);
        assert_eq!(unsafe { (*texture).Status }, status);
        assert_eq!(unsafe { (*texture).TexID }, texture_id);

        drop(ctx_b);
        let mut ctx_a = suspended_a;
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
        let suspended = first;

        assert!(matches!(
            crate::Context::try_create_with_shared_font_atlas(shared_atlas.clone()),
            Err(crate::ContextError::SharedFontAtlasManaged)
        ));
        assert_eq!(unsafe { (*raw).RefCount }, 1);

        let mut first = suspended;
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
        assert!(matches!(
            atlas.try_claim_legacy_renderer(),
            Err(FontError::RendererReleasePending)
        ));
        assert!(matches!(
            crate::Context::try_create_with_shared_font_atlas(shared_atlas.clone()),
            Err(crate::ContextError::SharedFontAtlasRendererReleasePending)
        ));
        assert_eq!(unsafe { (*raw).RefCount }, 0);
    }

    #[test]
    fn shared_atlas_registration_uses_interior_mutability_not_a_unique_rust_borrow() {
        let shared_atlas = SharedFontAtlas::create();
        let raw = shared_atlas.as_ptr();
        let ctx = crate::Context::create_with_shared_font_atlas(shared_atlas.clone());
        let atlas = ctx.font_atlas();

        let other = ctx.binding().with_bound_context(|| {
            crate::Context::create_with_shared_font_atlas(shared_atlas.clone())
        });
        assert_eq!(unsafe { (*raw).RefCount }, 2);
        assert_eq!(unsafe { sys::igGetCurrentContext() }, ctx.as_raw());

        let _ = atlas.add_font(&[FontSource::default_font()]);
        drop(other);
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
        let suspended_a = ctx_a;

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
        let backend_flags =
            managed.io().backend_flags() | crate::BackendFlags::RENDERER_HAS_TEXTURES;
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
}

mod source_validation {
    use super::*;

    #[test]
    fn font_config_defaults_match_the_native_constructor() {
        let _guard = crate::test_support::imgui_context_guard();
        let _context = crate::Context::create();
        let expected = unsafe {
            let config = sys::ImFontConfig_ImFontConfig();
            assert!(!config.is_null());
            let expected = *config;
            sys::ImFontConfig_destroy(config);
            expected
        };
        assert_eq!(FontConfig::new().raw, expected);
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
        assert!(
            std::panic::catch_unwind(|| FontConfig::new().glyph_exclude_ranges(&ranges)).is_err()
        );
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

        assert!(matches!(
            atlas.set_font_loader(FontLoader::stb_truetype()),
            Err(FontError::SourcesAlreadyAdded { source_count: 1 })
        ));
    }

    #[test]
    fn validated_stb_source_owns_bytes_and_pins_its_native_loader() {
        const FONT_DATA: &[u8] = include_bytes!(
            "../../../../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/Roboto-Medium.ttf"
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
            "../../../../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/Roboto-Medium.ttf"
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
            "../../../../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/ProggyClean.ttf"
        );
        let validated = StbTrueTypeFontData::from_slice(FONT_DATA).unwrap();

        let mut loader_override = FontConfig::new();
        loader_override.raw.FontLoader = std::ptr::dangling();
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

        let error =
            StbTrueTypeFontData::from_file("this-font-file-must-not-exist.ttf").unwrap_err();
        assert!(matches!(error, FontError::Io { .. }));
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
            "../../../../dear-imgui-sys/third-party/cimgui/imgui/misc/fonts/ProggyClean.ttf"
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
        let suspended_a = ctx_a;

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
}

mod texture {
    use super::{reconcile_with_retry, sys};

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
}

mod flags {
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

mod loader {
    use super::FontLoader;

    #[test]
    fn builtin_font_loader_has_required_callbacks() {
        let _guard = crate::test_support::imgui_context_guard();
        let _owner = crate::Context::create();
        let loader = FontLoader::stb_truetype();
        let raw = loader.as_ptr();
        assert!(!raw.is_null());
        assert!(!unsafe { (*raw).Name }.is_null());
        assert!(unsafe { (*raw).FontBakedLoadGlyph }.is_some());
    }
}

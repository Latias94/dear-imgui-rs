use super::*;
use crate::ImguiAppExt;
use crate::render::lifecycle::initialize_imgui_gpu_resources;

#[test]
fn renderer_release_rejects_stale_and_cross_context_acknowledgements() {
    let context_a = test_context_id();
    let context_b = test_context_id();
    let releases = ImguiRendererReleases::default();
    let lease_a = releases.admit(context_a);
    let lease_b = releases.admit(context_b);

    assert!(!lease_a.request_release());
    assert!(!lease_b.request_release());
    let requested = releases.requested_releases();
    assert_eq!(requested.len(), 2);
    let generation_a = requested
        .iter()
        .find_map(|(context_id, generation)| (*context_id == context_a).then_some(*generation))
        .expect("Context A should request renderer release");
    let generation_b = requested
        .iter()
        .find_map(|(context_id, generation)| (*context_id == context_b).then_some(*generation))
        .expect("Context B should request renderer release");
    assert_ne!(generation_a, generation_b);

    assert!(!releases.acknowledge_release(
        context_b,
        generation_a,
        resources::ImguiRendererReleasePacket::default(),
    ));
    assert!(!releases.acknowledge_release(
        context_a,
        generation_b,
        resources::ImguiRendererReleasePacket::default(),
    ));
    assert_eq!(releases.requested_releases(), requested);

    assert!(releases.acknowledge_release(
        context_a,
        generation_a,
        resources::ImguiRendererReleasePacket::default(),
    ));
    assert!(lease_a.request_release());
    assert!(
        releases.release_requested(context_b),
        "acknowledging Context A must leave Context B's request intact"
    );
    assert_eq!(
        releases.requested_releases(),
        vec![(context_b, generation_b)]
    );

    lease_a.release_renderer_resources();
    lease_a.retire();
    assert!(releases.acknowledge_release(
        context_b,
        generation_b,
        resources::ImguiRendererReleasePacket::default(),
    ));
    lease_b.release_renderer_resources();
    lease_b.retire();
    assert_eq!(releases.len(), 0);
}

#[test]
fn renderer_recovery_coalesces_repeated_device_generations_per_context() {
    let context_a = test_context_id();
    let context_b = test_context_id();
    let releases = ImguiRendererReleases::default();
    let lease_a = releases.admit(context_a);
    let lease_b = releases.admit(context_b);

    assert!(releases.begin_device_recovery(1, HashMap::new()));
    assert!(releases.recovery_requested(context_a));
    assert!(releases.recovery_requested(context_b));
    assert!(
        !releases.begin_device_recovery(1, HashMap::new()),
        "the same render-device generation must be rejected as stale"
    );
    assert!(
        releases.begin_device_recovery(2, HashMap::new()),
        "a newer device generation must supersede unfinished recovery work"
    );

    lease_a.release_renderer_resources();
    assert!(
        releases.begin_device_recovery(3, HashMap::new()),
        "a newer device generation must supersede a Context after its old packet is released"
    );
    lease_a.finish_device_recovery();
    assert!(!releases.release_requested(context_a));
    assert!(
        releases.recovery_requested(context_b),
        "finishing Context A must not resume Context B"
    );

    lease_b.release_renderer_resources();
    lease_b.finish_device_recovery();
    assert!(!releases.release_requested(context_b));
}

#[test]
fn renderer_teardown_wins_when_device_recovery_interleaves() {
    let context_a = test_context_id();
    let context_b = test_context_id();
    let releases = ImguiRendererReleases::default();
    let lease_a = releases.admit(context_a);
    let lease_b = releases.admit(context_b);

    assert!(!lease_a.request_release());
    assert!(releases.begin_device_recovery(1, HashMap::new()));
    assert!(
        !releases.recovery_requested(context_a),
        "a Context already being removed must not re-enter renderer recovery"
    );
    assert!(releases.recovery_requested(context_b));
    assert!(
        lease_a.request_release(),
        "device recovery must transfer an existing teardown packet without another render ack"
    );

    lease_a.release_renderer_resources();
    lease_a.retire();
    assert_eq!(releases.len(), 1);
    assert!(releases.recovery_requested(context_b));

    lease_b.release_renderer_resources();
    lease_b.finish_device_recovery();
    assert!(!releases.release_requested(context_b));
}

#[test]
fn driver_skips_teardown_during_device_recovery_and_frames_other_context() {
    struct RecoveryContextPass;

    let mut app = crate::test_util::RenderTestApp::new();
    app.add_plugins(crate::test_util::render_extraction_plugin());
    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    app.add_plugins(crate::ImguiPlugin::default());
    let recovery_pass = app.declare_imgui_pass::<RecoveryContextPass>().unwrap();
    app.world_mut().spawn((Window::default(), PrimaryWindow));

    let (context_a, context_b) = {
        let mut contexts = app
            .world_mut()
            .get_non_send_mut::<crate::ImguiContexts>()
            .unwrap();
        let context_a = contexts.primary_id().unwrap().unwrap();
        let context_b = contexts
            .create(crate::ImguiContextConfig::new(&recovery_pass))
            .unwrap();
        (context_a, context_b)
    };
    let releases = app.world().resource::<ImguiRendererReleases>().clone();

    assert!(matches!(
        app.world_mut()
            .get_non_send_mut::<crate::ImguiContexts>()
            .unwrap()
            .try_remove_immediately(context_a),
        Err(crate::ImguiContextError::RemovalPending {
            context_id,
            reason:
                crate::context::ImguiContextRemovalPendingReason::RenderWorldReleasePending,
        }) if context_id == context_a
    ));
    assert!(
        releases.begin_device_recovery(1, HashMap::new()),
        "the test device generation must detach both renderer leases"
    );
    assert!(
        !releases.recovery_requested(context_a),
        "teardown must convert Context A's recovery packet into its final release"
    );
    assert!(
        releases.recovery_requested(context_b),
        "the live Context must enter renderer recovery independently"
    );

    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.update()))
        .expect("driver recovery must not panic while another Context is tearing down");
    {
        let contexts = app.world().get_non_send::<crate::ImguiContexts>().unwrap();
        assert_eq!(contexts.frame_index(context_a).unwrap(), 0);
        assert_eq!(
            contexts.frame_index(context_b).unwrap(),
            1,
            "Context B must recover and frame while Context A remains quarantined"
        );
        assert!(contexts.is_tearing_down(context_a));
    }
    assert!(!releases.recovery_requested(context_b));

    let removed = app
        .world_mut()
        .get_non_send_mut::<crate::ImguiContexts>()
        .unwrap()
        .try_remove_immediately(context_a)
        .expect("Context A must consume the recovery-detached release packet");
    assert_eq!(removed.id(), context_a);
    drop(removed);

    app.update();
    assert_eq!(
        app.world()
            .get_non_send::<crate::ImguiContexts>()
            .unwrap()
            .frame_index(context_b)
            .unwrap(),
        2,
        "Context B must remain live after Context A finishes removal"
    );
}

#[test]
fn explicit_shutdown_drains_render_world_acknowledgements_without_update() {
    let mut app = App::new();
    app.add_plugins(crate::test_util::render_extraction_plugin());
    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    app.add_plugins(crate::ImguiPlugin::default());
    app.world_mut().spawn((Window::default(), PrimaryWindow));

    let context_id = app
        .world()
        .get_non_send::<crate::ImguiContexts>()
        .unwrap()
        .primary_id()
        .unwrap()
        .unwrap();
    let releases = app.world().resource::<ImguiRendererReleases>().clone();

    app.shutdown_imgui().unwrap();

    assert!(app.world().get_non_send::<crate::ImguiContexts>().is_none());
    assert!(!releases.release_requested(context_id));
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[test]
fn renderer_release_and_viewport_drain_converge_without_resuming_frames() {
    let mut app = App::new();
    app.add_plugins(crate::test_util::render_extraction_plugin());
    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    app.add_plugins(crate::ImguiPlugin::new(
        crate::ImguiPluginConfig::default().with_multi_viewport(true),
    ));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    let primary_id = app
        .world()
        .get_non_send::<crate::ImguiContexts>()
        .unwrap()
        .primary_id()
        .unwrap()
        .unwrap();
    let viewport_id = imgui::Id::from(0xA11);
    assert!(
        app.world()
            .get_non_send::<crate::ImguiViewportBridge>()
            .unwrap()
            .queue_for_context(
                primary_id,
                crate::viewport::ImguiViewportCommand::Create(crate::ImguiViewportSnapshot {
                    id: viewport_id,
                    pos: [10.0, 20.0],
                    size: [320.0, 180.0],
                    dpi_scale: 1.0,
                    flags: imgui::ViewportFlags::IS_PLATFORM_WINDOW,
                },),
            ),
        "the primary Context must own a registered viewport bridge"
    );
    app.update();
    assert!(
        app.world()
            .get_non_send::<crate::ImguiViewportBridge>()
            .unwrap()
            .viewport_window(primary_id, viewport_id)
            .is_some()
    );

    let frame_index = app
        .world()
        .get_non_send::<crate::ImguiContexts>()
        .unwrap()
        .frame_index(primary_id)
        .unwrap();
    let releases = app.world().resource::<ImguiRendererReleases>().clone();
    app.sub_app_mut(RenderApp).update_schedule = None;
    assert!(matches!(
        app.world_mut()
            .get_non_send_mut::<crate::ImguiContexts>()
            .unwrap()
            .try_remove_immediately(primary_id),
        Err(crate::ImguiContextError::RemovalPending {
            context_id,
            reason: crate::context::ImguiContextRemovalPendingReason::ViewportWorldReleasePending,
        }) if context_id == primary_id
    ));
    assert!(
        app.world()
            .get_non_send::<crate::ImguiContexts>()
            .unwrap()
            .contains(primary_id)
            .unwrap(),
        "a pending removal must retain Context ownership for retry"
    );
    let requested = releases.requested_releases();
    let [(requested_context, generation)] = requested.as_slice() else {
        panic!("the primary Context should request renderer release");
    };
    assert_eq!(*requested_context, primary_id);

    app.update();
    assert_eq!(
        app.world()
            .get_non_send::<crate::ImguiContexts>()
            .unwrap()
            .frame_index(primary_id)
            .unwrap(),
        frame_index,
        "viewport drain must not resume native frame production"
    );
    assert!(
        app.world()
            .get_non_send::<crate::ImguiViewportBridge>()
            .unwrap()
            .viewport_window(primary_id, viewport_id)
            .is_none(),
        "viewport entities must drain without waiting for renderer acknowledgement"
    );

    assert!(matches!(
        app.world_mut()
            .get_non_send_mut::<crate::ImguiContexts>()
            .unwrap()
            .try_remove_immediately(primary_id),
        Err(crate::ImguiContextError::RemovalPending {
            context_id,
            reason: crate::context::ImguiContextRemovalPendingReason::RenderWorldReleasePending,
        }) if context_id == primary_id
    ));
    assert!(releases.acknowledge_release(
        primary_id,
        *generation,
        resources::ImguiRendererReleasePacket::default(),
    ));
    assert!(releases.release_requested(primary_id));

    let context = app
        .world_mut()
        .get_non_send_mut::<crate::ImguiContexts>()
        .unwrap()
        .try_remove_immediately(primary_id)
        .expect("viewport and renderer release acknowledgements must complete removal");
    assert_eq!(context.id(), primary_id);
    assert!(
        !app.world()
            .get_non_send::<crate::ImguiContexts>()
            .unwrap()
            .contains(primary_id)
            .unwrap(),
        "a completed retry must remove exactly the requested Context"
    );
}

#[test]
fn context_teardown_waits_for_render_world_release_before_native_mutation() {
    let context = managed_context();
    let mut owner = crate::context::ContextOwner::new(context);
    let releases = ImguiRendererReleases::default();
    let backend = crate::context::BackendAttachment {
        render_integration_installed: true,
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        viewport_bridge_registration: None,
        renderer_releases: Some(releases.clone()),
    };
    owner.preflight_renderer_admission(&backend).unwrap();
    owner.commit_renderer_admission(&backend).unwrap();
    let renderer_texture = imgui::TextureId::new(0xCAFE);
    let texture = owner
        .try_with_current_renderer_context(false, |context, consumer| {
            let consumer = consumer.expect("renderer admission must install a consumer");
            let texture = register_test_texture(context);
            let snapshot = managed_snapshot(context, consumer, Some(texture));
            let feedback =
                successful_texture_feedback(&snapshot, Some((texture, renderer_texture)));
            snapshot.commit(feedback).unwrap();
            context.poll_snapshot_completions().unwrap();
            Ok::<_, ()>(texture)
        })
        .unwrap();

    let error = owner
        .try_detach_backend()
        .expect_err("live render-world resources must prevent Context teardown");
    assert_eq!(
        error,
        crate::context::ImguiContextRemovalPendingReason::RenderWorldReleasePending
    );
    owner
        .try_with_current_renderer_context(false, |context, _| {
            context
                .with_texture(texture, |texture| {
                    assert_eq!(texture.status(), imgui::TextureStatus::OK);
                    assert_eq!(texture.texture_id(), renderer_texture);
                })
                .unwrap();
            Ok::<_, ()>(())
        })
        .unwrap();

    let requested = releases.requested_releases();
    let [(requested_context, generation)] = requested.as_slice() else {
        panic!("the owned Context should request renderer release");
    };
    assert!(releases.acknowledge_release(
        *requested_context,
        *generation,
        resources::ImguiRendererReleasePacket::default(),
    ));
    owner
        .try_detach_backend()
        .expect("acknowledged render-world release should allow teardown");
    assert_eq!(releases.len(), 0);
    owner
        .into_context()
        .with_texture(texture, |texture| {
            assert_eq!(texture.status(), imgui::TextureStatus::WantCreate);
            assert!(texture.texture_id().is_null());
        })
        .unwrap();
}

#[test]
fn device_recovery_resets_each_context_texture_and_resumes_its_consumer() {
    let releases = ImguiRendererReleases::default();
    let backend = crate::context::BackendAttachment {
        render_integration_installed: true,
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        viewport_bridge_registration: None,
        renderer_releases: Some(releases.clone()),
    };
    let make_owner = || {
        let context = managed_context();
        let context_id = context.id();
        let mut owner = crate::context::ContextOwner::new(context);
        owner.preflight_renderer_admission(&backend).unwrap();
        owner.commit_renderer_admission(&backend).unwrap();
        let texture = owner
            .try_with_current_renderer_context(false, |context, consumer| {
                let consumer = consumer.expect("renderer admission must install a consumer");
                let texture = register_test_texture(context);
                let snapshot = managed_snapshot(context, consumer, Some(texture));
                let feedback = successful_texture_feedback(
                    &snapshot,
                    Some((texture, imgui::TextureId::new(0xCAFE))),
                );
                snapshot.commit(feedback).unwrap();
                context.poll_snapshot_completions().unwrap();
                Ok::<_, std::convert::Infallible>(texture)
            })
            .unwrap_or_else(|never| match never {});
        (context_id, owner, texture)
    };
    let (context_a, mut owner_a, texture_a) = make_owner();
    let (context_b, mut owner_b, texture_b) = make_owner();

    assert!(releases.begin_device_recovery(1, HashMap::new()));
    assert!(releases.recovery_requested(context_a));
    assert!(releases.recovery_requested(context_b));
    assert!(owner_a.try_recover_renderer().is_ok());
    assert!(owner_b.try_recover_renderer().is_ok());

    for (owner, context_id, texture) in [
        (&mut owner_a, context_a, texture_a),
        (&mut owner_b, context_b, texture_b),
    ] {
        assert!(!releases.recovery_requested(context_id));
        owner
            .try_with_current_renderer_context(false, |context, consumer| {
                assert!(
                    consumer.is_some(),
                    "device recovery must resume the admitted renderer consumer"
                );
                context
                    .with_texture(texture, |texture| {
                        assert_eq!(texture.status(), imgui::TextureStatus::WantCreate);
                        assert!(texture.texture_id().is_null());
                    })
                    .unwrap();
                Ok::<_, std::convert::Infallible>(())
            })
            .unwrap_or_else(|never| match never {});
    }

    for owner in [&mut owner_a, &mut owner_b] {
        assert_eq!(
            owner.try_detach_backend(),
            Err(crate::context::ImguiContextRemovalPendingReason::RenderWorldReleasePending)
        );
    }
    for (context_id, generation) in releases.requested_releases() {
        assert!(releases.acknowledge_release(
            context_id,
            generation,
            resources::ImguiRendererReleasePacket::default(),
        ));
    }
    owner_a.try_detach_backend().unwrap();
    owner_b.try_detach_backend().unwrap();
    assert_eq!(releases.len(), 0);
}

#[test]
fn extracted_frame_commits_request_bound_create_update_and_destroy_feedback() {
    let mut context = managed_context();
    let consumer = context.create_detached_renderer_consumer().unwrap();

    let texture_data = imgui::texture::OwnedTextureData::from_pixels(
        imgui::TextureFormat::RGBA32,
        1,
        1,
        &[255, 0, 255, 255],
    )
    .unwrap();
    let texture = context.register_texture(texture_data);
    let renderer_texture = imgui::TextureId::new(0xBEEF);
    let mut extracted = ImguiExtractedRenderFrame::default();

    let create = managed_snapshot(&mut context, &consumer, Some(texture));
    assert_eq!(
        user_texture_request(&create, texture).kind(),
        imgui::render::snapshot::TextureRequestKind::Create
    );
    let feedback = successful_texture_feedback(&create, Some((texture, renderer_texture)));
    let feedback_count = feedback.len();
    let context_id = context.id();
    extracted.replace(context_id, pending_frame(1, create), Vec::new());
    extracted.extend_texture_feedback(context_id, feedback);
    assert!(extracted.commit_all().is_empty());
    let progress = context.poll_snapshot_completions().unwrap();
    assert_eq!(progress.committed(), 1);
    assert_eq!(progress.feedback_applied(), feedback_count);
    context
        .with_texture(texture, |texture| {
            assert_eq!(texture.status(), imgui::TextureStatus::OK);
            assert_eq!(texture.texture_id(), renderer_texture);
        })
        .unwrap();

    context
        .try_with_texture_mut(texture, |mut texture| {
            texture.replace_pixels(&[0, 255, 0, 255])
        })
        .unwrap();
    let update = managed_snapshot(&mut context, &consumer, Some(texture));
    assert_eq!(
        user_texture_request(&update, texture).kind(),
        imgui::render::snapshot::TextureRequestKind::Update
    );
    let feedback = successful_texture_feedback(&update, Some((texture, renderer_texture)));
    let feedback_count = feedback.len();
    extracted.replace(context_id, pending_frame(2, update), Vec::new());
    extracted.extend_texture_feedback(context_id, feedback);
    assert!(extracted.commit_all().is_empty());
    let progress = context.poll_snapshot_completions().unwrap();
    assert_eq!(progress.committed(), 1);
    assert_eq!(progress.feedback_applied(), feedback_count);

    context.remove_texture(texture).unwrap();
    let destroy = managed_snapshot(&mut context, &consumer, None);
    assert_eq!(
        user_texture_request(&destroy, texture).kind(),
        imgui::render::snapshot::TextureRequestKind::Destroy
    );
    let feedback = successful_texture_feedback(&destroy, None);
    let feedback_count = feedback.len();
    extracted.replace(context_id, pending_frame(3, destroy), Vec::new());
    extracted.extend_texture_feedback(context_id, feedback);
    assert!(extracted.commit_all().is_empty());
    let progress = context.poll_snapshot_completions().unwrap();
    assert_eq!(progress.committed(), 1);
    assert_eq!(progress.feedback_applied(), feedback_count);
    assert!(context.with_texture(texture, |_| ()).is_err());
}

#[test]
fn extracted_frame_reports_snapshot_commit_contract_errors() {
    let mut context = managed_context();
    let consumer = context.create_detached_renderer_consumer().unwrap();
    let texture = register_test_texture(&mut context);
    let snapshot = managed_snapshot(&mut context, &consumer, Some(texture));
    let expected_count = snapshot.texture_requests().len();
    let context_id = context.id();
    let mut extracted = ImguiExtractedRenderFrame::default();
    extracted.replace(context_id, pending_frame(1, snapshot), Vec::new());

    let errors = extracted.commit_all();
    assert!(matches!(
        errors.as_slice(),
        [(
            actual_context,
            imgui::FrameError::MissingFeedback { count, .. }
        )] if *actual_context == context_id && *count == expected_count
    ));

    let mailbox = crate::context::ImguiFrameMailbox::default();
    let expected_error = errors[0];
    for (context_id, source) in errors {
        mailbox.record_snapshot_commit_error(context_id, source);
    }
    assert_eq!(mailbox.take_snapshot_commit_errors(), vec![expected_error]);

    let progress = context.poll_snapshot_completions().unwrap();
    assert_eq!(progress.abandoned(), 1);
    assert_eq!(progress.committed(), 0);
}

#[test]
fn extracted_render_frame_is_move_only() {
    trait AmbiguousIfClone<Marker> {
        fn assert_not_clone() {}
    }
    impl<T: ?Sized> AmbiguousIfClone<()> for T {}
    struct Invalid;
    impl<T: Clone> AmbiguousIfClone<Invalid> for T {}

    let _ = <ImguiExtractedRenderFrame as AmbiguousIfClone<_>>::assert_not_clone;
}

#[test]
fn replacing_an_extracted_frame_abandons_only_the_previous_epoch() {
    let mut context = managed_context();
    let consumer = context.create_detached_renderer_consumer().unwrap();
    let first = managed_snapshot(&mut context, &consumer, None);
    let second = managed_snapshot(&mut context, &consumer, None);
    let second_feedback = retry_texture_feedback(&second);
    let mut extracted = ImguiExtractedRenderFrame::default();
    let context_id = context.id();

    extracted.replace(context_id, pending_frame(1, first), Vec::new());
    extracted.replace(context_id, pending_frame(2, second), Vec::new());
    extracted.extend_texture_feedback(context_id, second_feedback);
    assert!(extracted.commit_all().is_empty());

    let progress = context.poll_snapshot_completions().unwrap();
    assert_eq!(progress.abandoned(), 1);
    assert_eq!(progress.committed(), 1);
    assert_eq!(progress.watermark(), 2);
    assert!(extracted.commit_all().is_empty());
    let progress = context.poll_snapshot_completions().unwrap();
    assert_eq!(progress.watermark(), 2);
    assert_eq!(progress.committed(), 0);
    assert_eq!(progress.abandoned(), 0);
    assert_eq!(progress.feedback_applied(), 0);
}

#[test]
fn mailbox_epoch_jump_abandons_every_skipped_snapshot_before_committing_the_latest() {
    let mut context = managed_context();
    let consumer = context.create_detached_renderer_consumer().unwrap();
    let first = managed_snapshot(&mut context, &consumer, None);
    let second = managed_snapshot(&mut context, &consumer, None);
    let third = managed_snapshot(&mut context, &consumer, None);
    let third_feedback = retry_texture_feedback(&third);
    assert_eq!(first.epoch().sequence(), 1);
    assert_eq!(second.epoch().sequence(), 2);
    assert_eq!(third.epoch().sequence(), 3);

    let mailbox = crate::context::ImguiFrameMailbox::default();
    let mut extracted = ImguiExtractedRenderFrame::default();
    let context_id = context.id();
    mailbox.publish(context_id, pending_frame(1, first));
    mailbox.publish(context_id, pending_frame(2, second));
    let second = mailbox
        .take_all()
        .remove(&context_id)
        .expect("Context snapshot should remain in the mailbox");
    extracted.replace(context_id, second, Vec::new());
    mailbox.publish(context_id, pending_frame(3, third));
    let third = mailbox
        .take_all()
        .remove(&context_id)
        .expect("Context snapshot should remain in the mailbox");
    extracted.replace(context_id, third, Vec::new());
    extracted.extend_texture_feedback(context_id, third_feedback);
    assert!(extracted.commit_all().is_empty());

    let progress = context.poll_snapshot_completions().unwrap();
    assert_eq!(progress.abandoned(), 2);
    assert_eq!(progress.committed(), 1);
    assert_eq!(progress.watermark(), 3);
    mailbox.update_completion_watermark(context_id, progress.watermark());
    extracted.begin_extraction(0, mailbox.completion_watermarks());
    assert_eq!(extracted.completion_watermark(context_id), Some(3));
}

#[test]
fn replacing_context_a_mailbox_slot_does_not_touch_context_b() {
    let mut context_a = managed_context();
    let consumer_a = context_a.create_detached_renderer_consumer().unwrap();
    let first_a = managed_snapshot(&mut context_a, &consumer_a, None);
    let second_a = managed_snapshot(&mut context_a, &consumer_a, None);
    let second_a_feedback = retry_texture_feedback(&second_a);
    let context_a_id = context_a.id();

    let mut context_b = managed_context();
    let consumer_b = context_b.create_detached_renderer_consumer().unwrap();
    let first_b = managed_snapshot(&mut context_b, &consumer_b, None);
    let first_b_feedback = retry_texture_feedback(&first_b);
    let context_b_id = context_b.id();

    let mailbox = crate::context::ImguiFrameMailbox::default();
    mailbox.publish(context_a_id, pending_frame(1, first_a));
    mailbox.publish(context_b_id, pending_frame(1, first_b));
    mailbox.publish(context_a_id, pending_frame(2, second_a));
    assert_eq!(mailbox.len(), 2);

    let pending = mailbox.take_all();
    assert_eq!(pending.len(), 2);
    assert_eq!(pending[&context_a_id].frame_index, 2);
    assert_eq!(pending[&context_b_id].frame_index, 1);

    let mut extracted = ImguiExtractedRenderFrame::default();
    for (context_id, frame) in pending {
        extracted.replace(context_id, frame, Vec::new());
    }
    extracted.extend_texture_feedback(context_a_id, second_a_feedback);
    extracted.extend_texture_feedback(context_b_id, first_b_feedback);
    assert!(extracted.commit_all().is_empty());

    let progress_a = context_a.poll_snapshot_completions().unwrap();
    assert_eq!(progress_a.abandoned(), 1);
    assert_eq!(progress_a.committed(), 1);
    assert_eq!(progress_a.watermark(), 2);
    let progress_b = context_b.poll_snapshot_completions().unwrap();
    assert_eq!(progress_b.abandoned(), 0);
    assert_eq!(progress_b.committed(), 1);
    assert_eq!(progress_b.watermark(), 1);
}

#[test]
fn releasing_context_a_resource_packet_does_not_change_context_b() {
    let mut context_a = managed_context();
    let texture_a = imgui::render::SnapshotTextureId::User(register_test_texture(&mut context_a));
    let context_a_id = context_a.id();

    let mut context_b = managed_context();
    let texture_b = imgui::render::SnapshotTextureId::User(register_test_texture(&mut context_b));
    let context_b_id = context_b.id();

    let mut bindings = ImguiTextureBindGroups::default();
    bindings.destroy_managed_texture(texture_a, 5);
    bindings.destroy_managed_texture(texture_b, 5);

    let releases = ImguiRendererReleases::default();
    let lease_a = releases.admit(context_a_id);
    let lease_b = releases.admit(context_b_id);

    assert!(!lease_a.request_release());
    let requested = releases.requested_releases();
    let [(requested_context, generation)] = requested.as_slice() else {
        panic!("only Context A should request release");
    };
    assert_eq!(*requested_context, context_a_id);
    let packet = resources::ImguiRendererReleasePacket::new(
        bindings.take_managed_renderer_state(context_a_id),
        Vec::new(),
    );
    assert!(packet.is_empty());
    assert!(bindings.accepts_managed_texture_upload(texture_a));
    assert!(
        !bindings.accepts_managed_texture_upload(texture_b),
        "collecting Context A's release packet must retain Context B's destroy tombstone"
    );
    assert!(releases.acknowledge_release(context_a_id, *generation, packet));

    lease_a.release_renderer_resources();
    lease_a.retire();
    assert_eq!(releases.len(), 1);
    assert!(!releases.release_requested(context_b_id));
    assert!(releases.requested_releases().is_empty());

    assert!(!lease_b.request_release());
    let requested = releases.requested_releases();
    let [(requested_context, generation)] = requested.as_slice() else {
        panic!("Context B should remain independently releasable");
    };
    assert_eq!(*requested_context, context_b_id);
    assert!(releases.acknowledge_release(
        context_b_id,
        *generation,
        resources::ImguiRendererReleasePacket::default(),
    ));
    lease_b.release_renderer_resources();
    lease_b.retire();
    assert_eq!(releases.len(), 0);
}

#[test]
fn dropping_an_extracted_frame_abandons_its_epoch_once() {
    let mut context = managed_context();
    let consumer = context.create_detached_renderer_consumer().unwrap();
    let snapshot = managed_snapshot(&mut context, &consumer, None);
    let mut extracted = ImguiExtractedRenderFrame::default();
    let context_id = context.id();
    extracted.replace(context_id, pending_frame(1, snapshot), Vec::new());

    drop(extracted);

    let progress = context.poll_snapshot_completions().unwrap();
    assert_eq!(progress.abandoned(), 1);
    assert_eq!(progress.committed(), 0);
    assert_eq!(progress.watermark(), 1);
    let progress = context.poll_snapshot_completions().unwrap();
    assert_eq!(progress.watermark(), 1);
    assert_eq!(progress.committed(), 0);
    assert_eq!(progress.abandoned(), 0);
    assert_eq!(progress.feedback_applied(), 0);
}

#[test]
#[ignore = "requires DEAR_IMGUI_BEVY_GPU_HARNESS=1 and a working native wgpu adapter"]
fn repeated_render_startup_rebuilds_gpu_resources_and_freezes_each_context() {
    if std::env::var_os("DEAR_IMGUI_BEVY_GPU_HARNESS").is_none() {
        return;
    }

    let RenderHarnessResources {
        render_device,
        render_queue,
        pipeline_cache,
    } = initialize_render_harness_resources();
    let context_a = test_context_id();
    let context_b = test_context_id();
    let releases = ImguiRendererReleases::default();
    let lease_a = releases.admit(context_a);
    let lease_b = releases.admit(context_b);
    let mut world = World::new();
    world.insert_resource(render_device);
    world.insert_resource(render_queue);
    world.insert_resource(pipeline_cache);
    world.insert_resource(ImguiRenderPipeline::default());
    world.insert_resource(resources::ImguiRenderDeviceState::default());
    world.insert_resource(releases.clone());
    world.insert_resource(crate::context::ImguiFrameMailbox::default());
    world.insert_resource(ImguiExtractedRenderFrame::default());
    world.insert_resource(ImguiPreparedRenderFrame::default());

    initialize_imgui_gpu_resources(&mut world);
    assert!(world.contains_resource::<ImguiPipelineGpuResources>());
    assert!(!releases.recovery_requested(context_a));
    assert!(!releases.recovery_requested(context_b));

    initialize_imgui_gpu_resources(&mut world);
    assert!(world.contains_resource::<ImguiPipelineGpuResources>());
    assert!(releases.recovery_requested(context_a));
    assert!(releases.recovery_requested(context_b));

    lease_a.release_renderer_resources();
    lease_a.finish_device_recovery();
    lease_b.release_renderer_resources();
    lease_b.finish_device_recovery();
}

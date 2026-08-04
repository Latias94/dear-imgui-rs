use super::*;
use bevy_ecs::prelude::{NonSend, Query};
use bevy_ecs::schedule::Schedule;

fn settle_test_decorated_client_placements(
    mut windows: Query<&mut Window>,
    bridge: NonSend<ImguiViewportBridge>,
) {
    for context in bridge.contexts() {
        crate::viewport::settle_pending_client_placements(&mut windows, &context, |_| {
            Some([0.0, 15.0])
        });
    }
}

fn reset_platform_geometry_requests(app: &mut App, viewport_id: imgui::Id) {
    with_primary_context(app, |_context| {
        let raw_viewport = unsafe { sys::igFindViewportByID(viewport_id.raw()) };
        assert!(
            !raw_viewport.is_null(),
            "the secondary viewport must remain in Dear ImGui's registry"
        );
        let viewport = unsafe { imgui::Viewport::from_raw_mut(raw_viewport) };
        viewport.set_platform_request_move(false);
        viewport.set_platform_request_resize(false);
    });
}

fn platform_geometry_requests(app: &mut App, viewport_id: imgui::Id) -> (bool, bool) {
    let raw_viewport = resolve_live_viewport(app, viewport_id);
    let viewport = unsafe { imgui::Viewport::from_raw_mut(raw_viewport) };
    (
        viewport.platform_request_move(),
        viewport.platform_request_resize(),
    )
}

fn prepare_secondary_viewport_frame(
    app: &mut App,
    context_bridge: &crate::viewport::ImguiViewportBridgeContext,
    primary_window: Entity,
    entity: Entity,
    viewport_id: imgui::Id,
    feedback: ImguiViewportFeedback,
) {
    reset_platform_geometry_requests(app, viewport_id);
    let instance_id = context_bridge
        .instance_for_id(viewport_id)
        .expect("the viewport route should have a stable instance");
    with_primary_context(app, |context| {
        crate::viewport::prepare_platform_viewports_for_frame(
            context,
            context_bridge,
            primary_window,
            &Window::default(),
            &[],
            std::iter::once((entity, instance_id, feedback)),
            crate::viewport::NativeViewportFrameSupport::new(
                true,
                crate::viewport::native_window::DesktopPositionSupport::Available,
            ),
        )
        .expect("native geometry polling should preserve the platform callback contract");
    });
}

#[test]
fn viewport_native_geometry_polling_requests_imgui_platform_sync_without_pending_requests() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    let primary_window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    let context_id = primary_context_id(&app);
    let (id, entity) = create_live_secondary_viewport(&mut app);
    let context_bridge = app
        .world()
        .non_send::<ImguiViewportBridge>()
        .context(context_id)
        .expect("the primary Context bridge should remain registered");
    let instance_id = context_bridge
        .instance_for_id(id)
        .expect("the viewport route should have a stable instance");
    let initial = ImguiViewportFeedback {
        pos: [100.0, 200.0],
        size: [320.0, 180.0],
        framebuffer_scale: [1.0, 1.0],
        dpi_scale: 1.0,
        focused: true,
        minimized: false,
    };
    let moved = ImguiViewportFeedback {
        pos: [420.0, 630.0],
        ..initial
    };
    context_bridge.remove_viewport_feedback(instance_id);
    context_bridge.set_viewport_feedback(instance_id, initial);

    prepare_secondary_viewport_frame(&mut app, &context_bridge, primary_window, entity, id, moved);
    assert_eq!(context_bridge.viewport_feedback(id), Some(moved));
    assert_eq!(
        platform_geometry_requests(&mut app, id),
        (true, false),
        "OS window moves must ask Dear ImGui to pull only the platform position"
    );

    let resized = ImguiViewportFeedback {
        size: [640.0, 360.0],
        ..moved
    };
    prepare_secondary_viewport_frame(
        &mut app,
        &context_bridge,
        primary_window,
        entity,
        id,
        resized,
    );
    assert_eq!(context_bridge.viewport_feedback(id), Some(resized));
    assert_eq!(
        platform_geometry_requests(&mut app, id),
        (false, true),
        "OS window resizes must ask Dear ImGui to pull only the platform size"
    );

    destroy_live_secondary_viewport(&mut app, id);
}

#[test]
fn pending_size_request_reconciles_matching_and_constrained_native_extents() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    let primary_window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    let context_id = primary_context_id(&app);
    let (id, entity) = create_live_secondary_viewport(&mut app);
    let context_bridge = app
        .world()
        .non_send::<ImguiViewportBridge>()
        .context(context_id)
        .expect("the primary Context bridge should remain registered");
    let instance_id = context_bridge
        .instance_for_id(id)
        .expect("the viewport route should have a stable instance");
    let initial = ImguiViewportFeedback {
        pos: [100.0, 200.0],
        size: [320.0, 180.0],
        framebuffer_scale: [1.0, 1.0],
        dpi_scale: 1.0,
        focused: true,
        minimized: false,
    };
    let requested_size = [640.0, 360.0];
    context_bridge.remove_viewport_feedback(instance_id);
    context_bridge.set_viewport_feedback(instance_id, initial);
    context_bridge.record_size_request(instance_id, requested_size, 1.0);
    assert!(
        context_bridge
            .inner
            .state
            .borrow()
            .record(
                context_bridge
                    .instance_for_id(id)
                    .expect("the viewport route should have a stable instance")
            )
            .is_some_and(|record| record.geometry.has_requested_size())
    );

    let matching = ImguiViewportFeedback {
        size: requested_size,
        ..initial
    };
    prepare_secondary_viewport_frame(
        &mut app,
        &context_bridge,
        primary_window,
        entity,
        id,
        matching,
    );
    assert_eq!(platform_geometry_requests(&mut app, id), (false, false));

    context_bridge.record_size_request(instance_id, requested_size, 1.0);
    let constrained = ImguiViewportFeedback {
        size: [600.0, 340.0],
        ..matching
    };
    prepare_secondary_viewport_frame(
        &mut app,
        &context_bridge,
        primary_window,
        entity,
        id,
        constrained,
    );
    assert_eq!(
        platform_geometry_requests(&mut app, id),
        (false, true),
        "a constrained size must make Dear ImGui adopt the actual native client extent"
    );

    destroy_live_secondary_viewport(&mut app, id);
}

#[test]
fn minimized_viewport_defers_geometry_reconciliation_until_restore() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    let primary_window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    let context_id = primary_context_id(&app);
    let (id, entity) = create_live_secondary_viewport(&mut app);
    let context_bridge = app
        .world()
        .non_send::<ImguiViewportBridge>()
        .context(context_id)
        .expect("the primary Context bridge should remain registered");
    let instance_id = context_bridge
        .instance_for_id(id)
        .expect("the viewport route should have a stable instance");
    let initial = ImguiViewportFeedback {
        pos: [100.0, 200.0],
        size: [320.0, 180.0],
        framebuffer_scale: [1.0, 1.0],
        dpi_scale: 1.0,
        focused: true,
        minimized: false,
    };
    context_bridge.remove_viewport_feedback(instance_id);
    context_bridge.set_viewport_feedback(instance_id, initial);
    context_bridge.record_position_request(instance_id, [500.0, 600.0], 1.0);
    context_bridge.record_size_request(instance_id, [640.0, 360.0], 1.0);

    let minimized = ImguiViewportFeedback {
        pos: [0.0, 0.0],
        size: [1.0, 1.0],
        focused: false,
        minimized: true,
        ..initial
    };
    prepare_secondary_viewport_frame(
        &mut app,
        &context_bridge,
        primary_window,
        entity,
        id,
        minimized,
    );
    assert_eq!(
        context_bridge.viewport_feedback(id),
        Some(ImguiViewportFeedback {
            focused: false,
            minimized: true,
            ..initial
        }),
        "minimized geometry must not replace the last authoritative client rectangle"
    );
    assert_eq!(platform_geometry_requests(&mut app, id), (false, false));
    {
        let state = context_bridge.inner.state.borrow();
        let geometry = state
            .record(
                context_bridge
                    .instance_for_id(id)
                    .expect("the viewport route should have a stable instance"),
            )
            .map(|record| &record.geometry)
            .expect("minimized frames must preserve unresolved geometry intent");
        assert!(geometry.has_requested_position());
        assert!(geometry.has_requested_size());
    }

    let restored = ImguiViewportFeedback {
        pos: [420.0, 630.0],
        size: [600.0, 340.0],
        ..initial
    };
    prepare_secondary_viewport_frame(
        &mut app,
        &context_bridge,
        primary_window,
        entity,
        id,
        restored,
    );
    assert_eq!(context_bridge.viewport_feedback(id), Some(restored));
    assert_eq!(
        platform_geometry_requests(&mut app, id),
        (true, true),
        "the first restored frame must reconcile both pending fields against native geometry"
    );

    destroy_live_secondary_viewport(&mut app, id);
}

#[test]
fn decorated_viewport_waits_for_client_geometry_before_platform_sync() {
    let _guard = imgui_context_guard();
    let mut app = app_with_multi_viewport_bridge();
    let primary_window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    let context_id = primary_context_id(&app);
    let (id, entity) = create_live_secondary_viewport(&mut app);
    let context_bridge = app
        .world()
        .non_send::<ImguiViewportBridge>()
        .context(context_id)
        .expect("the primary Context bridge should remain registered");
    let instance_id = context_bridge
        .instance_for_id(id)
        .expect("the viewport route should have a stable instance");
    let target = ImguiViewportFeedback {
        pos: [320.0, 240.0],
        size: [480.0, 270.0],
        framebuffer_scale: [1.0, 1.0],
        dpi_scale: 1.0,
        focused: true,
        minimized: false,
    };

    context_bridge.remove_viewport_feedback(instance_id);
    context_bridge.set_viewport_feedback(instance_id, target);
    context_bridge.record_position_request(instance_id, [128.0, 96.0], 1.0);
    context_bridge
        .inner
        .state
        .borrow_mut()
        .record_mut(instance_id)
        .expect("the viewport record should exist")
        .pending_client_placement = Some(crate::viewport::PendingClientPlacement {
        pos: [0.0, 0.0],
        dpi_scale: 0.5,
        show_requested: false,
        focus_requested: false,
    });
    context_bridge.record_position_request(instance_id, target.pos, target.dpi_scale);
    {
        let state = context_bridge.inner.state.borrow();
        let record = state
            .record(instance_id)
            .expect("the viewport record should exist");
        let placement = record
            .pending_client_placement
            .as_ref()
            .expect("decorated client placement should own the deferred request");
        assert_eq!(placement.pos, target.pos);
        assert_eq!(placement.dpi_scale, target.dpi_scale);
        assert!(
            record.geometry.is_empty(),
            "deferred position intent must not be duplicated in the geometry reconciler"
        );
    }

    let outer_origin_observation = ImguiViewportFeedback {
        pos: [target.pos[0], target.pos[1] + 15.0],
        ..target
    };
    prepare_secondary_viewport_frame(
        &mut app,
        &context_bridge,
        primary_window,
        entity,
        id,
        outer_origin_observation,
    );
    assert_eq!(
        context_bridge.viewport_feedback(id),
        Some(target),
        "a transient outer-window origin must not replace the requested client origin"
    );
    assert_eq!(
        platform_geometry_requests(&mut app, id),
        (false, false),
        "deferring client placement must not create geometry synchronization requests"
    );

    let mut settle_schedule = Schedule::default();
    settle_schedule.add_systems(settle_test_decorated_client_placements);
    settle_schedule.run(app.world_mut());
    {
        let state = context_bridge.inner.state.borrow();
        let record = state
            .record(instance_id)
            .expect("the viewport record should exist");
        assert!(record.pending_client_placement.is_none());
        assert!(
            record.geometry.has_requested_position(),
            "settlement must transfer the client-origin request to the geometry reconciler"
        );
    }
    prepare_secondary_viewport_frame(
        &mut app,
        &context_bridge,
        primary_window,
        entity,
        id,
        target,
    );
    assert_eq!(context_bridge.viewport_feedback(id), Some(target));
    assert_eq!(
        platform_geometry_requests(&mut app, id),
        (false, false),
        "the settled client origin must not leave a persistent docking offset"
    );

    destroy_live_secondary_viewport(&mut app, id);
}

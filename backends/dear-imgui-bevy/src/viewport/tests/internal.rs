use super::*;
#[cfg(feature = "render")]
use bevy_camera::{CameraOutputMode, ClearColorConfig};
use bevy_math::IVec2;
use bevy_window::WindowLevel;

#[derive(Resource)]
struct EcsReleaseBeforeDeferredProbe {
    entity: Entity,
    entity_was_live: bool,
    release_was_pending: bool,
}

fn observe_ecs_release_before_deferred(
    bridge: NonSend<ImguiViewportBridge>,
    entities: Query<Entity>,
    mut probe: ResMut<EcsReleaseBeforeDeferredProbe>,
) {
    probe.entity_was_live = entities.get(probe.entity).is_ok();
    probe.release_was_pending = bridge.inner.has_tracked_ecs_entities();
}

static FOREIGN_DESTROY_CALLS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static FOREIGN_RENDERER_DESTROY_CALLS: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0);

#[test]
fn removing_viewport_feedback_discards_geometry_before_id_reuse() {
    let bridge = ImguiViewportBridge::default();
    let context_bridge = ImguiViewportBridgeContext {
        context_id: test_context_id(),
        inner: bridge.keepalive(),
    };
    let viewport_id = ImguiViewportId::from(0xC1A0_u32);
    let instance_id = register_test_viewport(
        &context_bridge.inner,
        context_bridge.context_id,
        viewport_id,
    );
    let first = ImguiViewportFeedback {
        pos: [32.0, 48.0],
        size: [320.0, 180.0],
        framebuffer_scale: [1.0, 1.0],
        dpi_scale: 1.0,
        focused: true,
        minimized: false,
    };
    let replacement = ImguiViewportFeedback {
        pos: [640.0, 480.0],
        ..first
    };

    context_bridge.set_viewport_feedback(instance_id, first);
    context_bridge.record_position_request(instance_id, [-96.0, -64.0], 1.0);
    context_bridge.remove_viewport_feedback(instance_id);
    assert!(
        context_bridge
            .inner
            .state
            .borrow()
            .record(instance_id)
            .is_some_and(|record| record.geometry.is_empty()),
        "destroying a viewport must remove unresolved geometry before Dear ImGui reuses its id"
    );

    context_bridge.set_viewport_feedback(instance_id, replacement);
    let reconciliation = context_bridge.observe_viewport_feedback(instance_id, replacement);
    assert!(
        !reconciliation.request_move && !reconciliation.request_resize,
        "a reused viewport id must not inherit an old request as if it belonged to the new window"
    );
    assert_eq!(
        context_bridge.viewport_feedback(viewport_id),
        Some(replacement)
    );
    assert!(
        context_bridge
            .inner
            .state
            .borrow()
            .record(instance_id)
            .is_some_and(|record| record.geometry.is_empty()),
        "acknowledged geometry intent must not leave an empty per-frame map entry"
    );
}

#[test]
fn mixed_dpi_client_and_desktop_positions_round_trip() {
    let entity = Entity::from_raw_u32(1).expect("test entity index should be valid");
    let window_position = WindowPosition::At(IVec2::new(1920, -200));
    let client_position = [160.25, 48.0];
    let cached_origin = window_position_desktop(&window_position, 2.0);
    let desktop_position =
        window_client_logical_to_desktop(entity, 2.0, cached_origin, client_position)
            .expect("finite client geometry should map into desktop space");

    #[cfg(not(target_os = "macos"))]
    assert_eq!(desktop_position, [2240.5, -104.0]);
    #[cfg(target_os = "macos")]
    assert_eq!(desktop_position, [1120.25, -52.0]);

    assert_eq!(
        desktop_to_window_client_logical(entity, &window_position, 2.0, desktop_position,),
        Some(client_position)
    );
}

#[test]
fn mixed_dpi_window_geometry_round_trips_through_platform_feedback() {
    let entity = Entity::from_raw_u32(1).expect("test entity index should be valid");
    let snapshot = ImguiViewportSnapshot {
        id: imgui::Id::from(0x430),
        pos: [1920.0, -200.0],
        size: [800.0, 600.0],
        dpi_scale: 2.0,
        flags: imgui::ViewportFlags::IS_PLATFORM_WINDOW,
    };
    let window = window_from_snapshot(&snapshot);
    let feedback = feedback_from_window_for_entity(entity, &window, None, None);

    assert_eq!(feedback.pos, snapshot.pos);
    assert_eq!(feedback.size, snapshot.size);
    assert_eq!(feedback.dpi_scale, snapshot.dpi_scale);
    #[cfg(not(target_os = "macos"))]
    assert_eq!(feedback.framebuffer_scale, [1.0, 1.0]);
    #[cfg(target_os = "macos")]
    assert_eq!(feedback.framebuffer_scale, [2.0, 2.0]);
}

unsafe extern "C" fn foreign_platform_destroy_window(_viewport: *mut sys::ImGuiViewport) {
    FOREIGN_DESTROY_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

unsafe extern "C" fn foreign_renderer_destroy_window(_viewport: *mut sys::ImGuiViewport) {
    FOREIGN_RENDERER_DESTROY_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

unsafe extern "C" fn foreign_renderer_set_window_size(
    _viewport: *mut sys::ImGuiViewport,
    _size: sys::ImVec2,
) {
}

unsafe extern "C" fn foreign_platform_set_window_pos(
    _viewport: *mut sys::ImGuiViewport,
    _pos: *const sys::ImVec2,
) {
}

unsafe extern "C" fn foreign_platform_alpha(_viewport: *mut sys::ImGuiViewport, _alpha: f32) {}

unsafe extern "C" fn foreign_platform_render(
    _viewport: *mut sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
}

unsafe extern "C" fn foreign_platform_work_area(_viewport: *mut sys::ImGuiViewport) -> sys::ImVec4 {
    sys::ImVec4::default()
}

unsafe extern "C" fn foreign_platform_vk_surface(
    _viewport: *mut sys::ImGuiViewport,
    _instance: sys::ImU64,
    _allocators: *const c_void,
    _surface: *mut sys::ImU64,
) -> i32 {
    0
}

fn test_context_id() -> imgui::ContextId {
    imgui::Context::create().id()
}

fn register_test_viewport(
    keepalive: &ImguiViewportBridgeKeepalive,
    context_id: imgui::ContextId,
    viewport_id: ImguiViewportId,
) -> ImguiViewportInstanceId {
    static NEXT_NATIVE_ADDRESS: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(1);
    let identity = ImguiViewportIdentity {
        context_address: 0,
        address: NEXT_NATIVE_ADDRESS.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    };
    keepalive
        .state
        .borrow_mut()
        .register_viewport(context_id, identity, viewport_id)
        .expect("a synthetic test viewport should be registerable")
}

fn queue_test_viewport_command(
    keepalive: &ImguiViewportBridgeKeepalive,
    context_id: imgui::ContextId,
    command: ImguiViewportCommand,
) {
    keepalive
        .state
        .borrow_mut()
        .queue_for_test(context_id, command);
}

fn assert_despawn_remains_tracked_until_deferred_application(release: bool) {
    let viewport_id = imgui::Id::from(0x7A0);
    let main_viewport_id = imgui::Id::from(0x7A1);
    let context_id = test_context_id();
    let mut bridge = ImguiViewportBridge::default();
    let keepalive = bridge.keepalive();
    bridge.register_context(context_id, Rc::clone(&keepalive));
    let instance_id = register_test_viewport(&keepalive, context_id, viewport_id);
    let mut world = World::new();
    let entity = world
        .spawn((
            Window::default(),
            ImguiViewportWindow::new(instance_id, viewport_id),
            ImguiViewportOwner::window(instance_id),
        ))
        .id();
    bridge.set_viewport_window(viewport_id, entity);
    if release {
        keepalive.prepare_ecs_release(main_viewport_id);
    } else {
        bridge.queue(ImguiViewportCommand::Destroy { id: viewport_id });
    }
    world.insert_non_send(bridge);
    world.insert_resource(crate::context::ownership::ImguiBackendRuntime::new(
        crate::ImguiPluginConfig::default(),
        true,
    ));
    world.insert_resource(EcsReleaseBeforeDeferredProbe {
        entity,
        entity_was_live: false,
        release_was_pending: false,
    });

    let mut schedule = Schedule::default();
    schedule.add_systems(
        (
            apply_viewport_commands_system,
            observe_ecs_release_before_deferred,
            ApplyDeferred,
            acknowledge_viewport_ecs_despawns_system,
        )
            .chain_ignore_deferred(),
    );
    schedule.run(&mut world);

    let probe = world.resource::<EcsReleaseBeforeDeferredProbe>();
    assert!(
        probe.entity_was_live,
        "the probe must run before the deferred despawn is applied"
    );
    assert!(
        probe.release_was_pending,
        "despawn cannot be acknowledged while its ECS entity is still live"
    );
    assert!(world.get_entity(entity).is_err());
    assert!(
        !world
            .get_non_send::<ImguiViewportBridge>()
            .unwrap()
            .inner
            .has_tracked_ecs_entities(),
        "post-deferred acknowledgement must clear only entities proven absent"
    );
    if release {
        assert!(
            world
                .get_non_send::<ImguiViewportBridge>()
                .unwrap()
                .ecs_release_pending(),
            "ECS acknowledgement must leave final release ownership with the Context owner"
        );
        keepalive.finish_ecs_release();
        assert!(
            !world
                .get_non_send::<ImguiViewportBridge>()
                .unwrap()
                .ecs_release_pending(),
            "the Context owner must finish release after observing the drained ECS world"
        );
    }
}

#[test]
fn explicit_release_remains_pending_until_deferred_despawn_is_applied() {
    assert_despawn_remains_tracked_until_deferred_application(true);
}

#[test]
fn ordinary_destroy_remains_tracked_until_deferred_despawn_is_applied() {
    assert_despawn_remains_tracked_until_deferred_application(false);
}

#[test]
fn stale_owner_cannot_unregister_a_replacement_context_bridge() {
    let context_id = test_context_id();
    let bridge = ImguiViewportBridge::default();
    let registration = bridge.registration();
    let stale_owner = Rc::new(ImguiViewportBridgeShared::default());
    registration.register_context(context_id, Rc::clone(&stale_owner));
    registration.unregister_context(context_id, &stale_owner);

    let replacement = Rc::new(ImguiViewportBridgeShared::default());
    registration.register_context(context_id, Rc::clone(&replacement));
    registration.unregister_context(context_id, &stale_owner);

    let registered = bridge
        .context(context_id)
        .expect("the replacement viewport bridge must remain registered");
    assert!(Rc::ptr_eq(&registered.inner, &replacement));
}

fn test_viewport_snapshot(
    id: ImguiViewportId,
    flags: imgui::ViewportFlags,
) -> ImguiViewportSnapshot {
    ImguiViewportSnapshot {
        id,
        pos: [32.0, 48.0],
        size: [640.0, 360.0],
        dpi_scale: 1.0,
        flags,
    }
}

fn run_viewport_command_schedule(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(
        (
            apply_viewport_commands_system,
            ApplyDeferred,
            acknowledge_viewport_ecs_despawns_system,
        )
            .chain_ignore_deferred(),
    );
    schedule.run(world);
}

#[test]
fn pending_decorated_window_is_positioned_by_client_origin_before_show() {
    fn settle_with_test_decoration(
        mut windows: Query<&mut Window>,
        bridge: NonSend<ImguiViewportBridge>,
    ) {
        for context in bridge.contexts() {
            settle_pending_client_placements(&mut windows, &context, |_| Some([4.0, 15.0]));
        }
    }

    let context_id = test_context_id();
    let viewport_id = imgui::Id::from(0x7AF);
    let mut bridge = ImguiViewportBridge::default();
    let keepalive = bridge.keepalive();
    bridge.register_context(context_id, Rc::clone(&keepalive));
    let instance_id = register_test_viewport(&keepalive, context_id, viewport_id);

    let mut world = World::new();
    let entity = world
        .spawn(Window {
            visible: false,
            ..Default::default()
        })
        .id();
    {
        let mut state = keepalive.state.borrow_mut();
        let record = state
            .record_mut(instance_id)
            .expect("the synthetic viewport record should exist");
        record.window = Some(entity);
        record.feedback = Some(ImguiViewportFeedback {
            pos: [100.0, 200.0],
            size: [320.0, 180.0],
            framebuffer_scale: [1.0, 1.0],
            dpi_scale: 1.0,
            focused: false,
            minimized: false,
        });
        record.pending_client_placement = Some(PendingClientPlacement {
            pos: [100.0, 200.0],
            dpi_scale: 1.0,
            show_requested: true,
            focus_requested: true,
        });
    }
    world.insert_non_send(bridge);

    let mut schedule = Schedule::default();
    schedule.add_systems(settle_with_test_decoration);
    schedule.run(&mut world);

    let window = world
        .get::<Window>(entity)
        .expect("the pending viewport Window should remain live");
    assert_eq!(window.position, WindowPosition::At(IVec2::new(96, 185)));
    assert!(window.visible);
    let state = keepalive.state.borrow();
    let record = state
        .record(instance_id)
        .expect("the synthetic viewport record should remain live");
    assert!(record.pending_client_placement.is_none());
    assert!(record.focus_next_frame);
    assert_eq!(
        record
            .feedback
            .expect("settlement should preserve platform feedback")
            .pos,
        [100.0, 200.0]
    );
    assert!(
        record.geometry.has_requested_position(),
        "settlement must record the client-origin request for frame-boundary reconciliation"
    );
}

#[test]
fn command_application_scopes_equal_viewport_ids_to_their_contexts() {
    let context_a_id = test_context_id();
    let context_b_id = test_context_id();
    assert_ne!(context_a_id, context_b_id);

    let viewport_id = imgui::Id::from(0x7B0);
    let mut bridge = ImguiViewportBridge::default();
    let keepalive_a = bridge.keepalive();
    bridge.register_context(context_a_id, Rc::clone(&keepalive_a));
    let keepalive_b = Rc::new(ImguiViewportBridgeShared::default());
    bridge.register_context(context_b_id, Rc::clone(&keepalive_b));
    queue_test_viewport_command(
        &keepalive_a,
        context_a_id,
        ImguiViewportCommand::Create(test_viewport_snapshot(
            viewport_id,
            imgui::ViewportFlags::IS_PLATFORM_WINDOW | imgui::ViewportFlags::TOP_MOST,
        )),
    );
    queue_test_viewport_command(
        &keepalive_b,
        context_b_id,
        ImguiViewportCommand::Create(test_viewport_snapshot(
            viewport_id,
            imgui::ViewportFlags::IS_PLATFORM_WINDOW | imgui::ViewportFlags::NO_FOCUS_ON_APPEARING,
        )),
    );

    let mut world = World::new();
    world.insert_resource(crate::context::ownership::ImguiBackendRuntime::new(
        crate::ImguiPluginConfig::default(),
        true,
    ));
    world.insert_non_send(bridge);
    run_viewport_command_schedule(&mut world);

    let (window_a, window_b) = {
        let bridge = world.non_send::<ImguiViewportBridge>();
        (
            bridge
                .viewport_window(context_a_id, viewport_id)
                .expect("Context A should own its viewport window"),
            bridge
                .viewport_window(context_b_id, viewport_id)
                .expect("Context B should own its viewport window"),
        )
    };
    assert_ne!(window_a, window_b);
    assert_eq!(
        world
            .get::<ImguiViewportWindow>(window_a)
            .expect("Context A window should carry a viewport marker")
            .context_id(),
        context_a_id
    );
    assert_eq!(
        world
            .get::<ImguiViewportWindow>(window_b)
            .expect("Context B window should carry a viewport marker")
            .context_id(),
        context_b_id
    );
    assert_eq!(
        world
            .get::<Window>(window_a)
            .expect("Context A window should exist")
            .window_level,
        WindowLevel::AlwaysOnTop
    );

    queue_test_viewport_command(
        &keepalive_a,
        context_a_id,
        ImguiViewportCommand::SetPos {
            id: viewport_id,
            pos: [80.0, 96.0],
            dpi_scale: 1.0,
        },
    );
    queue_test_viewport_command(
        &keepalive_a,
        context_a_id,
        ImguiViewportCommand::SetSize {
            id: viewport_id,
            size: [320.0, 200.0],
            dpi_scale: 1.0,
        },
    );
    queue_test_viewport_command(
        &keepalive_a,
        context_a_id,
        ImguiViewportCommand::SetTitle {
            id: viewport_id,
            title: "Context A".to_owned(),
        },
    );
    queue_test_viewport_command(
        &keepalive_a,
        context_a_id,
        ImguiViewportCommand::Show { id: viewport_id },
    );
    queue_test_viewport_command(
        &keepalive_b,
        context_b_id,
        ImguiViewportCommand::Show { id: viewport_id },
    );
    run_viewport_command_schedule(&mut world);

    let window_a_state = world
        .get::<Window>(window_a)
        .expect("Context A window should remain live");
    assert_eq!(
        window_a_state.position,
        WindowPosition::At(IVec2::new(80, 96))
    );
    assert_eq!(window_a_state.resolution.width(), 320.0);
    assert_eq!(window_a_state.resolution.height(), 200.0);
    assert_eq!(window_a_state.title, "Context A");
    assert!(window_a_state.visible);
    assert!(!window_a_state.focused);
    assert!(
        !world
            .get::<Window>(window_b)
            .expect("Context B window should remain live")
            .focused,
        "NoFocusOnAppearing must remain local to Context B"
    );

    run_viewport_command_schedule(&mut world);
    assert!(
        world
            .get::<Window>(window_a)
            .expect("Context A window should remain live")
            .focused,
        "Context A show must request focus on the following ECS pass"
    );
    assert!(
        !world
            .get::<Window>(window_b)
            .expect("Context B window should remain live")
            .focused,
        "Context B must honor NoFocusOnAppearing"
    );

    queue_test_viewport_command(
        &keepalive_b,
        context_b_id,
        ImguiViewportCommand::SetFocus { id: viewport_id },
    );
    run_viewport_command_schedule(&mut world);
    run_viewport_command_schedule(&mut world);
    assert!(
        world
            .get::<Window>(window_b)
            .expect("Context B window should remain live")
            .focused,
        "an explicit Context B focus request must not be blocked by its show policy"
    );

    queue_test_viewport_command(
        &keepalive_b,
        context_b_id,
        ImguiViewportCommand::Destroy { id: viewport_id },
    );
    run_viewport_command_schedule(&mut world);
    let bridge = world.non_send::<ImguiViewportBridge>();
    assert!(
        bridge.viewport_window(context_b_id, viewport_id).is_none(),
        "destroying Context B must remove only Context B's mapping"
    );
    assert_eq!(
        bridge.viewport_window(context_a_id, viewport_id),
        Some(window_a),
        "Context A's equal numeric viewport id must remain live"
    );
    keepalive_b.record_callback_fault(ImguiViewportRuntimeError::CallbackReentered);
    assert_eq!(
        bridge.callback_error_for(context_b_id),
        Some(ImguiViewportRuntimeError::CallbackReentered)
    );
    assert_eq!(
        bridge.callback_error_for(context_a_id),
        None,
        "a deferred callback failure must remain local to its owning Context"
    );
}

#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
#[test]
fn transparent_viewport_camera_clears_to_transparent() {
    let camera = viewport_camera(true, imgui::ViewportFlags::empty());

    assert!(matches!(
        camera.output_mode,
        CameraOutputMode::Write {
            clear_color: ClearColorConfig::Custom(color),
            ..
        } if color == bevy_color::Color::NONE
    ));
    assert!(matches!(
        viewport_camera(false, imgui::ViewportFlags::empty()).output_mode,
        CameraOutputMode::Write {
            clear_color: ClearColorConfig::Default,
            ..
        }
    ));
}

struct PlatformViewportsGuard {
    platform_io: *mut sys::ImGuiPlatformIO,
    original_viewports: sys::ImVector_ImGuiViewportPtr,
    owned_viewport: *mut sys::ImGuiViewport,
}

impl PlatformViewportsGuard {
    unsafe fn replace(
        context: &mut imgui::Context,
        viewports: &mut [*mut sys::ImGuiViewport],
        owned_viewport: *mut sys::ImGuiViewport,
    ) -> Self {
        let platform_io = context.platform_io_mut().as_raw_mut();
        let original_viewports = unsafe { (*platform_io).Viewports };
        unsafe {
            (*platform_io).Viewports = sys::ImVector_ImGuiViewportPtr {
                Size: viewports
                    .len()
                    .try_into()
                    .expect("test viewport count should fit i32"),
                Capacity: viewports
                    .len()
                    .try_into()
                    .expect("test viewport count should fit i32"),
                Data: viewports.as_mut_ptr(),
            };
        }
        Self {
            platform_io,
            original_viewports,
            owned_viewport,
        }
    }
}

impl Drop for PlatformViewportsGuard {
    fn drop(&mut self) {
        unsafe {
            (*self.platform_io).Viewports = self.original_viewports;
            if !self.owned_viewport.is_null() {
                sys::ImGuiViewport_destroy(self.owned_viewport);
            }
        }
    }
}

fn feedback() -> ImguiViewportFeedback {
    ImguiViewportFeedback {
        pos: [0.0, 0.0],
        size: [64.0, 64.0],
        framebuffer_scale: [1.0, 1.0],
        dpi_scale: 1.0,
        focused: false,
        minimized: false,
    }
}

#[test]
fn platform_capabilities_follow_native_desktop_position_support() {
    let mut context = imgui::Context::create();
    let bridge = ImguiViewportBridge::default();
    let keepalive = bridge.keepalive();
    unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
    let context_bridge = ImguiViewportBridgeContext {
        context_id: context.id(),
        inner: Rc::clone(&keepalive),
    };
    let primary_window = Entity::from_raw_u32(1).expect("test entity index should be valid");
    let requested_flags = context.io().config_flags()
        | imgui::ConfigFlags::DOCKING_ENABLE
        | imgui::ConfigFlags::VIEWPORTS_ENABLE;
    context.io_mut().set_config_flags(requested_flags);

    for support in [
        native_window::DesktopPositionSupport::PendingWindow,
        native_window::DesktopPositionSupport::Unavailable,
    ] {
        prepare_platform_viewports_for_frame(
            &mut context,
            &context_bridge,
            primary_window,
            &Window::default(),
            &[],
            std::iter::empty(),
            NativeViewportFrameSupport::new(true, support),
        )
        .unwrap();
        assert!(
            !context
                .io()
                .config_flags()
                .contains(imgui::ConfigFlags::VIEWPORTS_ENABLE)
        );
        assert!(
            context
                .io()
                .config_flags()
                .contains(imgui::ConfigFlags::DOCKING_ENABLE),
            "native viewport fallback must preserve in-window docking",
        );
        assert!(!context.io().backend_flags().intersects(
            imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS
                | imgui::BackendFlags::RENDERER_HAS_VIEWPORTS
                | imgui::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT
        ));
    }

    prepare_platform_viewports_for_frame(
        &mut context,
        &context_bridge,
        primary_window,
        &Window::default(),
        &[],
        std::iter::empty(),
        NativeViewportFrameSupport::new(true, native_window::DesktopPositionSupport::Available),
    )
    .unwrap();
    assert!(
        context
            .io()
            .config_flags()
            .contains(imgui::ConfigFlags::VIEWPORTS_ENABLE)
    );
    assert!(context.io().backend_flags().contains(
        imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS | imgui::BackendFlags::RENDERER_HAS_VIEWPORTS
    ));
    assert_eq!(
        context
            .io()
            .backend_flags()
            .contains(imgui::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT),
        cfg!(target_os = "windows")
    );

    detach_owned_bridge(&mut context, &keepalive).unwrap();
}

#[test]
fn owned_command_snapshot_is_isolated_from_later_queue_changes() {
    let mut bridge = ImguiViewportBridge::default();
    let context_id = test_context_id();
    let keepalive = bridge.keepalive();
    bridge.register_context(context_id, keepalive);
    let viewport_id = imgui::Id::from(0x440);
    bridge.queue(ImguiViewportCommand::Show { id: viewport_id });

    let observed = bridge.commands();
    assert_eq!(
        bridge.drain_commands().unwrap(),
        [ImguiViewportCommand::Show { id: viewport_id }]
    );
    bridge.queue(ImguiViewportCommand::Destroy { id: viewport_id });

    assert_eq!(
        observed,
        [ImguiViewportCommand::Show { id: viewport_id }],
        "an observer must own an immutable snapshot of the queue"
    );
    assert_eq!(
        bridge.drain_commands().unwrap(),
        [ImguiViewportCommand::Destroy { id: viewport_id }]
    );
}

#[test]
fn callback_contention_latches_without_unwinding_through_c() {
    let mut context = imgui::Context::create();
    let mut bridge = ImguiViewportBridge::default();
    let keepalive = bridge.keepalive();
    unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
    let viewport = unsafe { sys::ImGuiViewport_ImGuiViewport() };
    assert!(!viewport.is_null());
    unsafe {
        (*viewport).ID = 0x441;
    }

    let state_borrow = bridge.inner.state.borrow_mut();
    let callback = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        platform_show_window(viewport.cast::<imgui::Viewport>());
    }));
    assert!(callback.is_ok(), "the C callback boundary must not unwind");
    drop(state_borrow);

    assert_eq!(
        bridge.callback_error(),
        Some(ImguiViewportRuntimeError::CallbackReentered)
    );
    assert_eq!(
        bridge.drain_commands(),
        Err(ImguiViewportRuntimeError::CallbackReentered)
    );
    assert_eq!(
        bridge.drain_commands(),
        Err(ImguiViewportRuntimeError::CallbackReentered),
        "the deferred callback fault must remain sticky"
    );

    bridge.clear_viewport_state();
    assert_eq!(bridge.callback_error(), None);
    assert!(bridge.drain_commands().unwrap().is_empty());

    unsafe {
        context
            .io_mut()
            .set_backend_platform_user_data(std::ptr::null_mut());
        sys::ImGuiViewport_destroy(viewport);
    }
}

#[test]
fn direct_callback_skips_foreign_userdata_without_dereferencing_it() {
    let mut context = imgui::Context::create();
    let bridge = ImguiViewportBridge::default();
    let keepalive = bridge.keepalive();
    unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
    let viewport = unsafe { sys::ImGuiViewport_ImGuiViewport() };
    assert!(!viewport.is_null());

    unsafe {
        context
            .io_mut()
            .set_backend_platform_user_data(std::ptr::dangling_mut::<u8>().cast());
        platform_show_window(viewport.cast::<imgui::Viewport>());
    }
    assert_eq!(
        bridge.callback_error(),
        Some(ImguiViewportRuntimeError::CallbackOwnership(
            ImguiViewportCallbackOwnershipError::BackendPlatformUserDataReplaced,
        )),
        "a callback must latch drift before casting foreign userdata"
    );
    assert!(bridge.commands().is_empty());

    unsafe {
        context
            .io_mut()
            .set_backend_platform_user_data(std::ptr::null_mut());
        sys::ImGuiViewport_destroy(viewport);
    }
}

#[test]
fn destroy_callback_never_clears_handles_when_dispatch_is_rejected() {
    let mut context = imgui::Context::create();
    let bridge = ImguiViewportBridge::default();
    let keepalive = bridge.keepalive();
    unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
    let viewport = unsafe { sys::ImGuiViewport_ImGuiViewport() };
    assert!(!viewport.is_null());
    unsafe {
        (*viewport).ID = 0x445;
        platform_create_window_raw_callback(viewport);
    }
    let owned_handle = unsafe { (*viewport).PlatformHandle };
    assert!(!owned_handle.is_null());

    let state_borrow = bridge.inner.state.borrow_mut();
    unsafe { platform_destroy_window_raw_callback(viewport) };
    assert_eq!(unsafe { (*viewport).PlatformUserData }, owned_handle);
    assert_eq!(unsafe { (*viewport).PlatformHandle }, owned_handle);
    assert_eq!(
        bridge.callback_error(),
        Some(ImguiViewportRuntimeError::CallbackReentered)
    );
    drop(state_borrow);

    bridge.inner.callback_fault.set(None);
    unsafe { platform_destroy_window_raw_callback(viewport) };
    assert!(unsafe { (*viewport).PlatformUserData.is_null() });
    assert!(unsafe { (*viewport).PlatformHandle.is_null() });
    detach_owned_bridge(&mut context, &keepalive).unwrap();
    unsafe { sys::ImGuiViewport_destroy(viewport) };
}

#[test]
fn live_numeric_id_collision_fails_without_replacing_the_existing_route() {
    let mut context = imgui::Context::create();
    let context_id = context.id();
    let main_id = context.main_viewport().id();
    let main_identity = ImguiViewportIdentity::capture(context.as_raw(), context.main_viewport());
    let mut state = ImguiViewportBridgeState::default();
    let main_instance = state
        .register_viewport(context_id, main_identity, main_id)
        .expect("the main viewport should be registerable");
    let collision = ImguiViewportIdentity {
        context_address: context.as_raw() as usize,
        address: main_identity.address.wrapping_add(1),
    };

    assert_eq!(
        state.register_viewport(context_id, collision, main_id),
        Err(ImguiViewportRuntimeError::ViewportIdCollision {
            viewport_id: main_id,
        })
    );
    assert_eq!(state.instance_for_id(main_id), Some(main_instance));
    assert_eq!(state.viewports.len(), 1);
}

#[test]
fn viewport_instance_generation_exhaustion_fails_closed() {
    let mut context = imgui::Context::create();
    let context_id = context.id();
    let main_id = context.main_viewport().id();
    let main_identity = ImguiViewportIdentity::capture(context.as_raw(), context.main_viewport());
    let mut state = ImguiViewportBridgeState {
        next_instance_generation: u64::MAX,
        ..Default::default()
    };

    assert_eq!(
        state.register_viewport(context_id, main_identity, main_id),
        Err(ImguiViewportRuntimeError::ViewportInstanceGenerationExhausted)
    );
    assert!(state.viewports.is_empty());
    assert!(state.instances_by_id.is_empty());
    assert!(state.instances_by_native.is_empty());
}

#[test]
fn destroy_callback_does_not_touch_viewport_for_the_wrong_current_context() {
    let mut context = imgui::Context::create();
    let raw_context = context.as_raw();
    let io = unsafe { sys::igGetIO_ContextPtr(raw_context) };
    let bridge = ImguiViewportBridge::default();
    let keepalive = bridge.keepalive();
    unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
    let viewport = unsafe { sys::ImGuiViewport_ImGuiViewport() };
    assert!(!viewport.is_null());
    unsafe {
        (*viewport).ID = 0x446;
        platform_create_window_raw_callback(viewport);
    }
    let owned_handle = unsafe { (*viewport).PlatformHandle };
    let suspended = context.suspend();
    let other_context = imgui::Context::create();

    unsafe { platform_destroy_window_raw_callback(viewport) };
    assert_eq!(unsafe { (*viewport).PlatformUserData }, owned_handle);
    assert_eq!(unsafe { (*viewport).PlatformHandle }, owned_handle);
    assert_eq!(bridge.callback_error(), None);

    unsafe {
        (*viewport).PlatformUserData = std::ptr::null_mut();
        (*viewport).PlatformHandle = std::ptr::null_mut();
        (*io).BackendPlatformUserData = std::ptr::null_mut();
        sys::ImGuiViewport_destroy(viewport);
    }
    VIEWPORT_BRIDGE_REGISTRY.with(|registry| {
        registry.borrow_mut().remove(&(raw_context as usize));
    });
    drop(other_context);
    drop(suspended);
}

#[test]
fn callback_install_rejects_foreign_ownership_without_partial_mutation() {
    let mut context = imgui::Context::create();
    let bridge = ImguiViewportBridge::default();
    unsafe {
        context
            .platform_io_mut()
            .set_platform_destroy_window_raw(Some(foreign_platform_destroy_window));
    }

    assert_eq!(
        unsafe { install_owned_platform_callbacks(&mut context, &bridge.keepalive()) },
        Err(ImguiViewportCallbackInstallError::CallbackSlot {
            slot: "Platform_DestroyWindow",
        })
    );
    assert!(context.io().backend_platform_user_data().is_null());
    let raw = unsafe { &*context.platform_io().as_raw() };
    assert!(raw.Platform_CreateWindow.is_none());
    assert!(raw.Platform_DestroyWindow.is_some_and(|callback| {
        std::ptr::fn_addr_eq(
            callback,
            foreign_platform_destroy_window as unsafe extern "C" fn(*mut sys::ImGuiViewport),
        )
    }));

    unsafe {
        context
            .platform_io_mut()
            .set_platform_destroy_window_raw(None);
        context
            .io_mut()
            .set_backend_platform_user_data(std::ptr::dangling_mut::<u8>().cast());
    }
    assert_eq!(
        unsafe { install_owned_platform_callbacks(&mut context, &bridge.keepalive()) },
        Err(ImguiViewportCallbackInstallError::BackendPlatformUserData)
    );
    assert!(
        unsafe { &*context.platform_io().as_raw() }
            .Platform_CreateWindow
            .is_none()
    );
    unsafe {
        context
            .io_mut()
            .set_backend_platform_user_data(std::ptr::null_mut());
    }
}

#[test]
fn callback_install_rejects_existing_platform_monitors_without_mutating_them() {
    let mut context = imgui::Context::create();
    let bridge = ImguiViewportBridge::default();
    let monitor = monitor_from_window(&Window::default());
    unsafe { context.platform_io_mut().set_monitors(&[monitor]) };
    let original = unsafe { (*context.platform_io().as_raw()).Monitors };

    assert_eq!(
        unsafe { install_owned_platform_callbacks(&mut context, &bridge.keepalive()) },
        Err(ImguiViewportCallbackInstallError::PlatformMonitors)
    );
    let actual = unsafe { (*context.platform_io().as_raw()).Monitors };
    assert_eq!(actual, original);
    assert_eq!(unsafe { *actual.Data }, monitor);
    assert!(context.io().backend_platform_user_data().is_null());
    assert!(
        unsafe { &*context.platform_io().as_raw() }
            .Platform_CreateWindow
            .is_none()
    );

    unsafe { context.platform_io_mut().set_monitors(&[]) };
}

#[test]
fn callback_install_rejects_foreign_name_and_main_viewport_handles_without_mutation() {
    let mut context = imgui::Context::create();
    let bridge = ImguiViewportBridge::default();
    context.set_platform_name(Some("foreign-platform")).unwrap();
    let foreign_name = context.io().backend_platform_name().unwrap().as_ptr();
    assert_eq!(
        unsafe { install_owned_platform_callbacks(&mut context, &bridge.keepalive()) },
        Err(ImguiViewportCallbackInstallError::BackendPlatformName)
    );
    assert_eq!(
        context.io().backend_platform_name().unwrap().as_ptr(),
        foreign_name
    );
    assert!(context.io().backend_platform_user_data().is_null());
    assert!(
        unsafe { &*context.platform_io().as_raw() }
            .Platform_CreateWindow
            .is_none()
    );
    context.set_platform_name::<String>(None).unwrap();
    drop(context);

    macro_rules! assert_main_field_rejected {
        ($field:ident) => {{
            let mut context = imgui::Context::create();
            let bridge = ImguiViewportBridge::default();
            let marker = std::ptr::dangling_mut::<u16>().cast::<c_void>();
            unsafe { (*context.main_viewport().as_raw_mut()).$field = marker };
            assert_eq!(
                unsafe { install_owned_platform_callbacks(&mut context, &bridge.keepalive()) },
                Err(ImguiViewportCallbackInstallError::MainViewportField {
                    field: stringify!($field),
                })
            );
            assert_eq!(
                unsafe { (*context.main_viewport().as_raw()).$field },
                marker
            );
            assert!(context.io().backend_platform_user_data().is_null());
            assert!(
                unsafe { &*context.platform_io().as_raw() }
                    .Platform_CreateWindow
                    .is_none()
            );
            unsafe { (*context.main_viewport().as_raw_mut()).$field = std::ptr::null_mut() };
        }};
    }

    assert_main_field_rejected!(PlatformUserData);
    assert_main_field_rejected!(PlatformHandle);
    assert_main_field_rejected!(PlatformHandleRaw);
}

#[test]
fn direct_callback_validates_every_platform_callback_slot_before_dispatch() {
    macro_rules! assert_removed_slot_drift {
        ($slot:ident) => {{
            let mut context = imgui::Context::create();
            let bridge = ImguiViewportBridge::default();
            let keepalive = bridge.keepalive();
            unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
            let flags = context.io().backend_flags() | imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS;
            context.io_mut().set_backend_flags(flags);
            keepalive.record_runtime_contract(&mut context);
            let viewport = unsafe { sys::ImGuiViewport_ImGuiViewport() };
            assert!(!viewport.is_null());
            unsafe {
                (*context.platform_io_mut().as_raw_mut()).$slot = None;
                platform_show_window_raw_callback(viewport);
            }
            assert_eq!(
                bridge.callback_error(),
                Some(ImguiViewportRuntimeError::CallbackOwnership(
                    ImguiViewportCallbackOwnershipError::PlatformCallbackReplaced {
                        slot: stringify!($slot),
                    },
                ))
            );
            assert!(
                !context
                    .io()
                    .backend_flags()
                    .contains(imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS)
            );
            let _ = detach_owned_bridge(&mut context, &keepalive);
            unsafe { sys::ImGuiViewport_destroy(viewport) };
        }};
    }

    assert_removed_slot_drift!(Platform_CreateWindow);
    assert_removed_slot_drift!(Platform_DestroyWindow);
    assert_removed_slot_drift!(Platform_ShowWindow);
    assert_removed_slot_drift!(Platform_SetWindowPos);
    assert_removed_slot_drift!(Platform_GetWindowPos);
    assert_removed_slot_drift!(Platform_SetWindowSize);
    assert_removed_slot_drift!(Platform_GetWindowSize);
    assert_removed_slot_drift!(Platform_GetWindowFramebufferScale);
    assert_removed_slot_drift!(Platform_SetWindowFocus);
    assert_removed_slot_drift!(Platform_GetWindowFocus);
    assert_removed_slot_drift!(Platform_GetWindowMinimized);
    assert_removed_slot_drift!(Platform_SetWindowTitle);
    assert_removed_slot_drift!(Platform_UpdateWindow);
    assert_removed_slot_drift!(Platform_GetWindowDpiScale);

    macro_rules! assert_installed_slot_drift {
        ($slot:ident, $callback:path) => {{
            let mut context = imgui::Context::create();
            let bridge = ImguiViewportBridge::default();
            let keepalive = bridge.keepalive();
            unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
            let flags = context.io().backend_flags() | imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS;
            context.io_mut().set_backend_flags(flags);
            keepalive.record_runtime_contract(&mut context);
            let viewport = unsafe { sys::ImGuiViewport_ImGuiViewport() };
            assert!(!viewport.is_null());
            unsafe {
                (*context.platform_io_mut().as_raw_mut()).$slot = Some($callback);
                platform_show_window_raw_callback(viewport);
            }
            assert_eq!(
                bridge.callback_error(),
                Some(ImguiViewportRuntimeError::CallbackOwnership(
                    ImguiViewportCallbackOwnershipError::PlatformCallbackInstalled {
                        slot: stringify!($slot),
                    },
                ))
            );
            assert!(
                !context
                    .io()
                    .backend_flags()
                    .contains(imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS)
            );
            let _ = detach_owned_bridge(&mut context, &keepalive);
            unsafe {
                (*context.platform_io_mut().as_raw_mut()).$slot = None;
                sys::ImGuiViewport_destroy(viewport);
            }
        }};
    }

    assert_installed_slot_drift!(Platform_SetWindowAlpha, foreign_platform_alpha);
    assert_installed_slot_drift!(Platform_RenderWindow, foreign_platform_render);
    assert_installed_slot_drift!(Platform_SwapBuffers, foreign_platform_render);
    assert_installed_slot_drift!(Platform_OnChangedViewport, foreign_platform_destroy_window);
    assert_installed_slot_drift!(Platform_GetWindowWorkAreaInsets, foreign_platform_work_area);
    assert_installed_slot_drift!(Platform_CreateVkSurface, foreign_platform_vk_surface);

    macro_rules! assert_renderer_slot_drift {
        ($slot:ident, $callback:path) => {{
            let mut context = imgui::Context::create();
            let bridge = ImguiViewportBridge::default();
            let keepalive = bridge.keepalive();
            unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
            let viewport = unsafe { sys::ImGuiViewport_ImGuiViewport() };
            assert!(!viewport.is_null());
            unsafe {
                (*context.platform_io_mut().as_raw_mut()).$slot = Some($callback);
                platform_show_window_raw_callback(viewport);
            }
            assert_eq!(
                bridge.callback_error(),
                Some(ImguiViewportRuntimeError::CallbackOwnership(
                    ImguiViewportCallbackOwnershipError::RendererCallbackInstalled {
                        slot: stringify!($slot),
                    },
                ))
            );
            let _ = detach_owned_bridge(&mut context, &keepalive);
            unsafe {
                (*context.platform_io_mut().as_raw_mut()).$slot = None;
                sys::ImGuiViewport_destroy(viewport);
            }
        }};
    }

    assert_renderer_slot_drift!(Renderer_CreateWindow, foreign_renderer_destroy_window);
    assert_renderer_slot_drift!(Renderer_DestroyWindow, foreign_renderer_destroy_window);
    assert_renderer_slot_drift!(Renderer_SetWindowSize, foreign_renderer_set_window_size);
    assert_renderer_slot_drift!(Renderer_RenderWindow, foreign_platform_render);
    assert_renderer_slot_drift!(Renderer_SwapBuffers, foreign_platform_render);
}

#[test]
fn direct_callback_validates_platform_name_flags_and_monitor_storage() {
    fn invoke_and_assert(
        context: &mut imgui::Context,
        bridge: &ImguiViewportBridge,
        expected: ImguiViewportCallbackOwnershipError,
    ) {
        let viewport = unsafe { sys::ImGuiViewport_ImGuiViewport() };
        assert!(!viewport.is_null());
        unsafe { platform_show_window_raw_callback(viewport) };
        assert_eq!(
            bridge.callback_error(),
            Some(ImguiViewportRuntimeError::CallbackOwnership(expected))
        );
        assert!(bridge.commands().is_empty());
        unsafe { sys::ImGuiViewport_destroy(viewport) };
        let _ = context;
    }

    let mut context = imgui::Context::create();
    let bridge = ImguiViewportBridge::default();
    let keepalive = bridge.keepalive();
    unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
    context.set_platform_name(Some("foreign-platform")).unwrap();
    invoke_and_assert(
        &mut context,
        &bridge,
        ImguiViewportCallbackOwnershipError::BackendPlatformNameReplaced,
    );
    let _ = detach_owned_bridge(&mut context, &keepalive);
    context.set_platform_name::<String>(None).unwrap();
    drop(context);

    let mut context = imgui::Context::create();
    let bridge = ImguiViewportBridge::default();
    let keepalive = bridge.keepalive();
    unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
    let flags = context.io().backend_flags() | imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS;
    context.io_mut().set_backend_flags(flags);
    invoke_and_assert(
        &mut context,
        &bridge,
        ImguiViewportCallbackOwnershipError::BackendFlagReplaced {
            flag: "PLATFORM_HAS_VIEWPORTS",
        },
    );
    assert!(
        !context
            .io()
            .backend_flags()
            .contains(imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS)
    );
    let _ = detach_owned_bridge(&mut context, &keepalive);
    drop(context);

    let mut context = imgui::Context::create();
    let bridge = ImguiViewportBridge::default();
    let keepalive = bridge.keepalive();
    unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
    let monitor = monitor_from_window(&Window::default());
    unsafe { context.platform_io_mut().set_monitors(&[monitor]) };
    let foreign = unsafe { (*context.platform_io().as_raw()).Monitors };
    invoke_and_assert(
        &mut context,
        &bridge,
        ImguiViewportCallbackOwnershipError::PlatformMonitorsReplaced,
    );
    let actual = unsafe { (*context.platform_io().as_raw()).Monitors };
    assert_eq!(actual, foreign);
    assert_eq!(unsafe { *actual.Data }, monitor);
    let _ = detach_owned_bridge(&mut context, &keepalive);
    unsafe { context.platform_io_mut().set_monitors(&[]) };
}

#[test]
fn owned_platform_name_rebase_preserves_every_other_runtime_contract_field() {
    let mut context = imgui::Context::create();
    let bridge = ImguiViewportBridge::default();
    let keepalive = bridge.keepalive();
    unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };

    let before = keepalive
        .runtime_contract
        .get()
        .expect("callback installation records a runtime contract");
    context.set_platform_name(Some("dear-imgui-bevy")).unwrap();
    keepalive.record_owned_platform_name(&mut context);
    let after = keepalive
        .runtime_contract
        .get()
        .expect("rebasing the owned name retains a runtime contract");

    assert_ne!(
        before.backend_platform_name, after.backend_platform_name,
        "the backend name write must replace only its own baseline"
    );
    assert_eq!(
        (
            before.backend_platform_user_data,
            before.owned_flags,
            before.main_viewport_platform_user_data,
            before.main_viewport_platform_handle,
            before.main_viewport_platform_handle_raw,
        ),
        (
            after.backend_platform_user_data,
            after.owned_flags,
            after.main_viewport_platform_user_data,
            after.main_viewport_platform_handle,
            after.main_viewport_platform_handle_raw,
        ),
        "rebasing the backend name must not accept unrelated platform drift"
    );
    assert_eq!(
        platform_callback_ownership(&mut context, &keepalive),
        Ok(())
    );

    let foreign_handle = std::ptr::dangling_mut::<u16>().cast::<c_void>();
    unsafe {
        (*context.main_viewport().as_raw_mut()).PlatformHandle = foreign_handle;
    }
    assert_eq!(
        platform_callback_ownership(&mut context, &keepalive),
        Err(ImguiViewportCallbackOwnershipError::ViewportFieldReplaced {
            field: "PlatformHandle",
        })
    );

    unsafe {
        (*context.main_viewport().as_raw_mut()).PlatformHandle = std::ptr::null_mut();
    }
    let _ = detach_owned_bridge(&mut context, &keepalive);
    context.set_platform_name::<String>(None).unwrap();
}

#[test]
fn callback_ownership_drift_detaches_owned_handles_without_calling_foreign_destroy() {
    let mut context = imgui::Context::create();
    let bridge = ImguiViewportBridge::default();
    let keepalive = bridge.keepalive();
    FOREIGN_DESTROY_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
    FOREIGN_RENDERER_DESTROY_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
    unsafe {
        install_owned_platform_callbacks(&mut context, &keepalive).unwrap();
    }
    assert_eq!(
        platform_callback_ownership(&mut context, &keepalive),
        Ok(())
    );

    let main_viewport = context.main_viewport().as_raw_mut();
    unsafe {
        platform_create_window_raw_callback(main_viewport);
        (*main_viewport).PlatformHandleRaw = (*main_viewport).PlatformHandle;
        (*main_viewport).PlatformWindowCreated = true;
    }
    keepalive.record_runtime_contract(&mut context);
    let owned_handle = unsafe { (*main_viewport).PlatformHandle };
    assert!(!owned_handle.is_null());
    let foreign_platform_handle = std::ptr::dangling_mut::<u16>().cast::<c_void>();

    unsafe {
        let platform_io = context.platform_io_mut();
        platform_io.set_renderer_destroy_window_raw(Some(foreign_renderer_destroy_window));
    }
    assert_eq!(
        platform_callback_ownership(&mut context, &keepalive),
        Err(
            ImguiViewportCallbackOwnershipError::RendererCallbackInstalled {
                slot: "Renderer_DestroyWindow",
            }
        )
    );
    unsafe {
        (*main_viewport).PlatformHandle = foreign_platform_handle;
    }
    unsafe {
        let platform_io = context.platform_io_mut();
        platform_io.set_platform_destroy_window_raw(Some(foreign_platform_destroy_window));
        platform_io.set_platform_set_window_pos_raw(Some(foreign_platform_set_window_pos));
    }

    assert_eq!(
        detach_owned_bridge(&mut context, &keepalive),
        Err(
            ImguiViewportCallbackOwnershipError::RendererCallbackInstalled {
                slot: "Renderer_DestroyWindow",
            }
        )
    );
    assert_eq!(
        FOREIGN_DESTROY_CALLS.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "a foreign destroy callback must not receive Bevy-owned viewport handles"
    );
    assert_eq!(
        FOREIGN_RENDERER_DESTROY_CALLS.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "a foreign renderer destroy callback must not receive Bevy-owned viewport handles"
    );
    unsafe {
        assert!((*main_viewport).PlatformUserData.is_null());
        assert_eq!(
            (*main_viewport).PlatformHandle,
            foreign_platform_handle,
            "direct detach must preserve a foreign viewport-field replacement"
        );
        assert!((*main_viewport).PlatformHandleRaw.is_null());
    }
    let state = bridge.inner.state.borrow();
    assert!(
        state
            .viewports
            .values()
            .all(|record| record.handle.is_none())
    );
    assert!(state.commands.is_empty());
    drop(state);
    assert!(context.io().backend_platform_user_data().is_null());

    let platform_io = context.platform_io_mut();
    let raw = unsafe { &*platform_io.as_raw() };
    assert!(raw.Platform_CreateWindow.is_none());
    assert!(raw.Platform_DestroyWindow.is_some_and(|callback| {
        std::ptr::fn_addr_eq(
            callback,
            foreign_platform_destroy_window as unsafe extern "C" fn(*mut sys::ImGuiViewport),
        )
    }));
    assert!(unsafe {
        platform_io
            .clear_platform_set_window_pos_if_pointer_callback(foreign_platform_set_window_pos)
    });
    assert!(raw.Renderer_DestroyWindow.is_some_and(|callback| {
        std::ptr::fn_addr_eq(
            callback,
            foreign_renderer_destroy_window as unsafe extern "C" fn(*mut sys::ImGuiViewport),
        )
    }));
    unsafe {
        platform_io.set_platform_destroy_window_raw(None);
        platform_io.set_renderer_destroy_window_raw(None);
        (*main_viewport).PlatformHandle = std::ptr::null_mut();
    }
}

#[test]
fn prepare_platform_viewports_rejects_each_replaced_main_viewport_field() {
    macro_rules! assert_main_viewport_field_drift {
        ($field:ident) => {{
            let mut context = imgui::Context::create();
            let bridge = ImguiViewportBridge::default();
            let keepalive = bridge.keepalive();
            unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
            let context_bridge = ImguiViewportBridgeContext {
                context_id: context.id(),
                inner: Rc::clone(&keepalive),
            };
            let primary_window =
                Entity::from_raw_u32(1).expect("test entity index should be valid");

            prepare_platform_viewports_for_frame(
                &mut context,
                &context_bridge,
                primary_window,
                &Window::default(),
                &[],
                std::iter::empty(),
                NativeViewportFrameSupport::new(
                    true,
                    native_window::DesktopPositionSupport::Available,
                ),
            )
            .unwrap();

            let foreign = std::ptr::dangling_mut::<u16>().cast::<c_void>();
            unsafe {
                (*context.main_viewport().as_raw_mut()).$field = foreign;
            }
            assert_eq!(
                prepare_platform_viewports_for_frame(
                    &mut context,
                    &context_bridge,
                    primary_window,
                    &Window::default(),
                    &[],
                    std::iter::empty(),
                    NativeViewportFrameSupport::new(
                        true,
                        native_window::DesktopPositionSupport::Available,
                    ),
                ),
                Err(ImguiViewportRuntimeError::CallbackOwnership(
                    ImguiViewportCallbackOwnershipError::ViewportFieldReplaced {
                        field: stringify!($field),
                    }
                ))
            );
            assert_eq!(
                unsafe { (*context.main_viewport().as_raw()).$field },
                foreign,
                "frame preparation must not overwrite a foreign main viewport field"
            );
            assert_eq!(
                bridge.callback_error(),
                Some(ImguiViewportRuntimeError::CallbackOwnership(
                    ImguiViewportCallbackOwnershipError::ViewportFieldReplaced {
                        field: stringify!($field),
                    }
                ))
            );
            assert!(
                (context.io().backend_flags().bits() & viewport_backend_flag_mask()) == 0,
                "a partial ownership drift must revoke the Bevy viewport capabilities"
            );
            assert!(
                !context
                    .io()
                    .config_flags()
                    .contains(imgui::ConfigFlags::VIEWPORTS_ENABLE),
                "a partial ownership drift must disable Bevy-managed viewport execution"
            );

            let _ = detach_owned_bridge(&mut context, &keepalive);
            unsafe {
                (*context.main_viewport().as_raw_mut()).$field = std::ptr::null_mut();
            }
        }};
    }

    assert_main_viewport_field_drift!(PlatformUserData);
    assert_main_viewport_field_drift!(PlatformHandle);
    assert_main_viewport_field_drift!(PlatformHandleRaw);
}

#[test]
fn direct_callback_preserves_complete_foreign_platform_takeover() {
    let mut context = imgui::Context::create();
    let bridge = ImguiViewportBridge::default();
    let keepalive = bridge.keepalive();
    unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
    let context_bridge = ImguiViewportBridgeContext {
        context_id: context.id(),
        inner: Rc::clone(&keepalive),
    };
    let primary_window = Entity::from_raw_u32(1).expect("test entity index should be valid");
    prepare_platform_viewports_for_frame(
        &mut context,
        &context_bridge,
        primary_window,
        &Window::default(),
        &[],
        std::iter::empty(),
        NativeViewportFrameSupport::new(true, native_window::DesktopPositionSupport::Available),
    )
    .unwrap();

    let foreign_user_data = std::ptr::dangling_mut::<u16>().cast::<c_void>();
    let foreign_main_user_data = std::ptr::dangling_mut::<u32>().cast::<c_void>();
    let foreign_main_handle = std::ptr::dangling_mut::<u64>().cast::<c_void>();
    let foreign_main_handle_raw = std::ptr::dangling_mut::<u8>().cast::<c_void>();
    let foreign_monitor = monitor_from_window(&Window::default());
    let foreign_flags = context.io().backend_flags()
        | imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS
        | imgui::BackendFlags::RENDERER_HAS_VIEWPORTS
        | imgui::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT;
    let foreign_config_flags = context.io().config_flags() | imgui::ConfigFlags::VIEWPORTS_ENABLE;
    unsafe {
        context
            .io_mut()
            .set_backend_platform_user_data(foreign_user_data);
        context.set_platform_name(Some("foreign-platform")).unwrap();
        let platform_io = context.platform_io_mut().as_raw_mut();
        sys::ImGuiPlatformIO_ClearPlatformHandlers(platform_io);
        (*platform_io).Platform_ShowWindow = Some(foreign_platform_destroy_window);
        (*context.main_viewport().as_raw_mut()).PlatformUserData = foreign_main_user_data;
        (*context.main_viewport().as_raw_mut()).PlatformHandle = foreign_main_handle;
        (*context.main_viewport().as_raw_mut()).PlatformHandleRaw = foreign_main_handle_raw;
    }
    unsafe { context.platform_io_mut().set_monitors(&[foreign_monitor]) };
    context.io_mut().set_backend_flags(foreign_flags);
    context.io_mut().set_config_flags(foreign_config_flags);

    let main_viewport = context.main_viewport().as_raw_mut();
    unsafe { platform_show_window_raw_callback(main_viewport) };

    assert_eq!(
        bridge.callback_error(),
        Some(ImguiViewportRuntimeError::CallbackOwnership(
            ImguiViewportCallbackOwnershipError::BackendPlatformUserDataReplaced,
        ))
    );
    assert_eq!(context.io().backend_flags(), foreign_flags);
    assert_eq!(context.io().config_flags(), foreign_config_flags);
    assert_eq!(context.io().backend_platform_user_data(), foreign_user_data);
    assert_eq!(
        context.io().backend_platform_name().unwrap().to_bytes(),
        b"foreign-platform"
    );
    unsafe {
        let main_viewport = context.main_viewport().as_raw();
        assert_eq!((*main_viewport).PlatformUserData, foreign_main_user_data);
        assert_eq!((*main_viewport).PlatformHandle, foreign_main_handle);
        assert_eq!((*main_viewport).PlatformHandleRaw, foreign_main_handle_raw);
        let platform_io = context.platform_io().as_raw();
        assert_eq!((*platform_io).Monitors.Size, 1);
        assert_eq!(*(*platform_io).Monitors.Data, foreign_monitor);
        assert!((*platform_io).Platform_ShowWindow.is_some_and(|callback| {
            std::ptr::fn_addr_eq(
                callback,
                foreign_platform_destroy_window as unsafe extern "C" fn(*mut sys::ImGuiViewport),
            )
        }));
    }

    let _ = detach_owned_bridge(&mut context, &keepalive);
    unsafe {
        let platform_io = context.platform_io_mut().as_raw_mut();
        sys::ImGuiPlatformIO_ClearPlatformHandlers(platform_io);
        context
            .io_mut()
            .set_backend_platform_user_data(std::ptr::null_mut());
        context.set_platform_name::<String>(None).unwrap();
        (*context.main_viewport().as_raw_mut()).PlatformUserData = std::ptr::null_mut();
        (*context.main_viewport().as_raw_mut()).PlatformHandle = std::ptr::null_mut();
        (*context.main_viewport().as_raw_mut()).PlatformHandleRaw = std::ptr::null_mut();
    }
    unsafe { context.platform_io_mut().set_monitors(&[]) };
    let mut flags = context.io().backend_flags();
    flags.remove(
        imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS
            | imgui::BackendFlags::RENDERER_HAS_VIEWPORTS
            | imgui::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT,
    );
    context.io_mut().set_backend_flags(flags);
    let mut config_flags = context.io().config_flags();
    config_flags.remove(imgui::ConfigFlags::VIEWPORTS_ENABLE);
    context.io_mut().set_config_flags(config_flags);
}

#[test]
fn prepare_platform_viewports_clears_ecs_state_for_missing_instances() {
    let mut context = imgui::Context::create();
    let bridge = ImguiViewportBridge::default();
    let keepalive = bridge.keepalive();
    unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
    let context_bridge = ImguiViewportBridgeContext {
        context_id: context.id(),
        inner: Rc::clone(&keepalive),
    };
    let primary_window = Entity::from_raw_u32(1).expect("test entity index should be valid");
    let secondary_window = Entity::from_raw_u32(2).expect("test entity index should be valid");
    let stale_viewport = imgui::Id::from(0x500);
    let live_viewport = imgui::Id::from(0x501);
    let stale_instance = register_test_viewport(&keepalive, context.id(), stale_viewport);
    let live_instance = register_test_viewport(&keepalive, context.id(), live_viewport);
    context_bridge.set_viewport_feedback(stale_instance, feedback());
    context_bridge.record_position_request(stale_instance, [-96.0, -64.0], 1.0);

    prepare_platform_viewports_for_frame(
        &mut context,
        &context_bridge,
        primary_window,
        &Window::default(),
        &[],
        std::iter::once((secondary_window, live_instance, feedback())),
        NativeViewportFrameSupport::new(true, native_window::DesktopPositionSupport::Available),
    )
    .unwrap();

    let main_viewport_id = context.main_viewport().id();
    let state = bridge.inner.state.borrow();
    let main_instance = state
        .instance_for_id(main_viewport_id)
        .expect("the main viewport should have a stable instance");
    assert!(
        state.record(main_instance).is_some_and(|record| matches!(
            record.handle.as_ref(),
            Some(ImguiViewportPlatformHandleState::Active(_))
        )),
        "the main viewport should retain its active native handle"
    );
    let live_record = state
        .record(live_instance)
        .expect("the live secondary instance should remain registered");
    assert_eq!(live_record.window, Some(secondary_window));
    assert_eq!(live_record.feedback, Some(feedback()));
    let stale_record = state
        .record(stale_instance)
        .expect("a stable native instance remains routable while its ECS state is absent");
    assert!(
        stale_record.feedback.is_none(),
        "native feedback must not outlive a viewport omitted from the current frame"
    );
    assert!(
        stale_record.geometry.is_empty(),
        "geometry intent must not outlive a viewport omitted from the current frame"
    );
    drop(state);

    let replacement = ImguiViewportFeedback {
        pos: [640.0, 480.0],
        ..feedback()
    };
    context_bridge.set_viewport_feedback(stale_instance, replacement);
    assert_eq!(
        context_bridge.observe_viewport_feedback(stale_instance, replacement),
        geometry::ViewportGeometryReconciliation::default(),
        "a reused viewport id must not inherit geometry pruned with its previous window"
    );

    detach_owned_bridge(&mut context, &keepalive).unwrap();
}

#[test]
fn cleanup_clears_handles_filtered_from_the_public_viewport_snapshot() {
    let mut context = imgui::Context::create();
    let bridge = ImguiViewportBridge::default();
    let keepalive = bridge.keepalive();
    unsafe { install_owned_platform_callbacks(&mut context, &keepalive).unwrap() };
    let context_bridge = ImguiViewportBridgeContext {
        context_id: context.id(),
        inner: Rc::clone(&keepalive),
    };
    let primary_window = Entity::from_raw_u32(1).expect("test entity index should be valid");

    prepare_platform_viewports_for_frame(
        &mut context,
        &context_bridge,
        primary_window,
        &Window::default(),
        &[],
        std::iter::empty(),
        NativeViewportFrameSupport::new(true, native_window::DesktopPositionSupport::Available),
    )
    .unwrap();

    let main_viewport = context.main_viewport().as_raw_mut();
    assert!(unsafe { !(*main_viewport).PlatformHandle.is_null() });
    let mut filtered_snapshot = [];
    {
        // Dear ImGui's internal list still includes the main viewport. This only models the
        // filtered public `PlatformIO.Viewports` snapshot that hides a live viewport.
        let _viewports_guard = unsafe {
            PlatformViewportsGuard::replace(
                &mut context,
                &mut filtered_snapshot,
                std::ptr::null_mut(),
            )
        };
        clear_imgui_viewport_platform_handles(&mut context, &context_bridge);
        assert!(
            unsafe { (*main_viewport).PlatformHandle.is_null() },
            "cleanup must clear a hidden backend-owned PlatformHandle before dropping it"
        );
        assert!(
            unsafe { (*main_viewport).PlatformUserData.is_null() },
            "cleanup must clear a hidden backend-owned PlatformUserData before dropping it"
        );
    }

    // The direct cleanup above intentionally changed the main viewport's owned fields. Rebase
    // the test's runtime contract before exercising the ordinary bridge detach path.
    keepalive.record_runtime_contract(&mut context);
    detach_owned_bridge(&mut context, &keepalive).unwrap();
}

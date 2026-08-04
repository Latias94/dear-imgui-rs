use super::frame::{
    PlatformViewportRequests, clear_imgui_viewport_platform_handles,
    mark_platform_viewport_requests,
};
use super::*;

fn with_window_mut(
    windows: &mut Query<&mut Window>,
    bridge: &ImguiViewportBridgeContext,
    instance_id: ImguiViewportInstanceId,
    f: impl FnOnce(&mut Window),
) -> Option<()> {
    let entity = bridge.viewport_window_for_instance(instance_id)?;
    let Ok(mut window) = windows.get_mut(entity) else {
        return None;
    };
    f(&mut window);
    Some(())
}

#[derive(SystemParam)]
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) struct OsViewportWindowEvents<'w, 's> {
    close_requests: MessageReader<'w, 's, WindowCloseRequested>,
    occluded: MessageReader<'w, 's, WindowOccluded>,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) fn sync_os_viewport_lifecycle_events(
    mut events: OsViewportWindowEvents,
    viewport_windows: Query<(&ImguiViewportWindow, &ImguiViewportOwner)>,
    contexts: Option<NonSendMut<crate::ImguiContexts>>,
    bridge: NonSend<ImguiViewportBridge>,
) {
    if events.close_requests.is_empty() && events.occluded.is_empty() {
        return;
    }
    let Some(mut contexts) = contexts else {
        events.close_requests.read().for_each(drop);
        events.occluded.read().for_each(drop);
        return;
    };
    let mapped_viewport = |window| {
        let (marker, owner) = viewport_windows.get(window).ok()?;
        let (context_id, instance_id) = owner.window_identity()?;
        let context_bridge = bridge.context(context_id)?;
        if !owner.matches_window(marker)
            || context_bridge.viewport_window_for_instance(instance_id) != Some(window)
        {
            return None;
        }
        Some((
            context_id,
            instance_id,
            context_bridge.viewport_id(instance_id)?,
        ))
    };
    let mut closed_viewports = HashMap::<imgui::ContextId, HashSet<ImguiViewportId>>::new();

    for event in events.close_requests.read() {
        if let Some((context_id, _, viewport_id)) = mapped_viewport(event.window) {
            closed_viewports
                .entry(context_id)
                .or_default()
                .insert(viewport_id);
        }
    }

    for event in events.occluded.read() {
        if let Some((context_id, instance_id, _)) = mapped_viewport(event.window) {
            let Some(context_bridge) = bridge.context(context_id) else {
                continue;
            };
            if let Some(mut feedback) = context_bridge.viewport_feedback_for_instance(instance_id) {
                feedback.minimized = event.occluded;
                context_bridge.set_viewport_feedback(instance_id, feedback);
            }
        }
    }

    let mut context_ids = closed_viewports.keys().copied().collect::<Vec<_>>();
    context_ids.sort_by_key(|context_id| context_id.get().get());
    for context_id in context_ids {
        let result = contexts.configure(context_id, |context| {
            mark_platform_viewport_requests(
                context,
                closed_viewports
                    .get(&context_id)
                    .into_iter()
                    .flat_map(|ids| ids.iter().copied())
                    .map(|viewport_id| (viewport_id, PlatformViewportRequests::close_requested())),
            );
        });
        match result {
            Ok(()) => {}
            Err(
                crate::ImguiContextError::TeardownInProgress { .. }
                | crate::ImguiContextError::UnknownContext { .. },
            ) => {}
            Err(error) => {
                panic!("cannot apply Dear ImGui viewport requests for {context_id:?}: {error}")
            }
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[cfg(feature = "render")]
type ViewportCameraComponentPresence = (
    Has<Camera2d>,
    Has<Camera>,
    Has<RenderTarget>,
    Has<CameraRenderGraph>,
    Has<RenderLayers>,
);

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(SystemParam)]
pub(super) struct ViewportCommandQueries<'w, 's> {
    windows: Query<'w, 's, &'static mut Window>,
    cursor_options: Query<'w, 's, &'static mut CursorOptions>,
    viewport_windows: Query<
        'w,
        's,
        (
            Entity,
            Option<&'static ImguiViewportWindow>,
            &'static ImguiViewportOwner,
        ),
    >,
    viewport_cameras: Query<
        'w,
        's,
        (
            Entity,
            Option<&'static ImguiViewportCamera>,
            &'static ImguiViewportOwner,
        ),
    >,
    #[cfg(feature = "render")]
    viewport_camera_components: Query<'w, 's, ViewportCameraComponentPresence>,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[allow(unused_variables)]
pub(super) fn apply_viewport_commands_system(
    mut ecs_commands: Commands,
    bridge: NonSend<ImguiViewportBridge>,
    backend_runtime: Res<crate::context::ownership::ImguiBackendRuntime>,
    winit_settings: Option<Res<WinitSettings>>,
    mut queries: ViewportCommandQueries,
) {
    let contexts = bridge.contexts();
    for context in contexts {
        apply_viewport_commands_for_context(
            &mut ecs_commands,
            &context,
            backend_runtime.config(),
            winit_settings.is_some(),
            &mut queries,
        );
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[allow(unused_variables)]
fn apply_viewport_commands_for_context(
    ecs_commands: &mut Commands,
    context: &ImguiViewportBridgeContext,
    config: &crate::ImguiPluginConfig,
    uses_winit_window_lifecycle: bool,
    queries: &mut ViewportCommandQueries,
) {
    let ViewportCommandQueries {
        windows,
        cursor_options,
        viewport_windows,
        viewport_cameras,
        #[cfg(feature = "render")]
        viewport_camera_components,
    } = queries;
    let Ok(queued) = context.drain_commands() else {
        return;
    };
    if context.ecs_release_pending() {
        for entity in context.take_all_ecs_entities_for_release() {
            native_window::release_pointer_capture_for(entity);
            ecs_commands.entity(entity).try_despawn();
        }
        return;
    }
    for entity in context.pending_ecs_despawns() {
        native_window::release_pointer_capture_for(entity);
        ecs_commands.entity(entity).try_despawn();
    }
    if uses_winit_window_lifecycle {
        settle_pending_client_placements(windows, context, winit_window_decoration_offset_desktop);
    }

    let viewport_window_config = config.viewport_window().validate().unwrap_or_else(|error| {
        panic!("invalid Dear ImGui viewport window configuration: {error}")
    });
    let mut feedback_candidates = HashSet::new();
    let mut pending_windows: HashMap<ImguiViewportInstanceId, Window> = HashMap::new();
    #[cfg(feature = "render")]
    let mut pending_cameras = HashSet::new();
    #[cfg(feature = "render")]
    let mut scheduled_camera_despawns = HashSet::new();
    #[cfg(feature = "render")]
    let mut owned_cameras = HashSet::new();
    #[cfg(feature = "render")]
    let mut recoverable_cameras = HashSet::new();
    for (entity, _, owner) in viewport_windows.iter() {
        let Some((context_id, instance_id)) = owner.window_identity() else {
            continue;
        };
        if context_id != context.context_id || windows.get_mut(entity).is_ok() {
            continue;
        }

        let owns_current_mapping =
            context.viewport_window_for_instance(instance_id) == Some(entity);
        if owns_current_mapping {
            context.remove_viewport_window(instance_id);
            context.remove_viewport_feedback(instance_id);
            context.remove_viewport_flags(instance_id);
            context.remove_pending_client_placement(instance_id);
            context.clear_focus_request(instance_id);
        }
        native_window::release_pointer_capture_for(entity);
        context.track_ecs_despawn(entity);
        ecs_commands.entity(entity).try_despawn();
        #[cfg(feature = "render")]
        if owns_current_mapping && let Some(camera) = context.remove_viewport_camera(instance_id) {
            scheduled_camera_despawns.insert(camera);
            context.track_ecs_despawn(camera);
            ecs_commands.entity(camera).try_despawn();
        }
    }
    #[cfg(feature = "render")]
    let live_cameras = viewport_cameras
        .iter()
        .filter_map(|(entity, marker, owner)| {
            let (context_id, instance_id) = owner.camera_identity()?;
            if context_id != context.context_id {
                return None;
            }
            owned_cameras.insert(entity);
            if context.viewport_camera_for_instance(instance_id) != Some(entity) {
                return None;
            }
            let viewport_id = context.viewport_id(instance_id)?;
            recoverable_cameras.insert((instance_id, entity));
            if marker.is_none_or(|marker| {
                !owner.matches_camera(marker) || marker.viewport_id() != viewport_id
            }) {
                ecs_commands
                    .entity(entity)
                    .insert(ImguiViewportCamera::new(instance_id, viewport_id));
            }
            viewport_camera_components
                .get(entity)
                .is_ok_and(
                    |(has_camera_2d, has_camera, has_target, has_graph, has_layers)| {
                        has_camera_2d && has_camera && has_target && has_graph && has_layers
                    },
                )
                .then_some((instance_id, entity))
        })
        .collect::<HashSet<_>>();
    for queued_command in queued {
        let instance_id = queued_command.instance_id;
        let Some(id) = context.viewport_id(instance_id) else {
            continue;
        };
        let command = queued_command.command;
        match command {
            ImguiViewportCommand::Create(mut snapshot) => {
                snapshot.id = id;
                {
                    let mut state = context.inner.state.borrow_mut();
                    let Some(record) = state.record_mut(instance_id) else {
                        continue;
                    };
                    record.flags = Some(snapshot.flags);
                    record.pending_client_placement = if uses_winit_window_lifecycle
                        && !snapshot.flags.contains(imgui::ViewportFlags::NO_DECORATION)
                    {
                        Some(PendingClientPlacement {
                            pos: finite_desktop_pos(snapshot.pos),
                            dpi_scale: positive_finite_or(snapshot.dpi_scale, 1.0),
                            show_requested: false,
                            focus_requested: false,
                        })
                    } else {
                        None
                    };
                }
                let entity = if let Some(entity) = context.viewport_window_for_instance(instance_id)
                {
                    entity
                } else {
                    let mut cursor_options = CursorOptions::default();
                    apply_viewport_flags_to_cursor_options(snapshot.flags, &mut cursor_options);
                    let entity = ecs_commands
                        .spawn((
                            window_from_snapshot_with_config(&snapshot, viewport_window_config)
                                .expect("the viewport window configuration was validated"),
                            cursor_options,
                            ImguiViewportWindow::new(instance_id, id),
                            ImguiViewportOwner::window(instance_id),
                        ))
                        .id();
                    context.set_viewport_window(instance_id, entity);
                    entity
                };
                context.set_viewport_feedback(instance_id, feedback_from_snapshot(&snapshot));
                context.record_position_request(instance_id, snapshot.pos, snapshot.dpi_scale);
                context.record_size_request(instance_id, snapshot.size, snapshot.dpi_scale);
                #[cfg(feature = "render")]
                ensure_viewport_camera(
                    ecs_commands,
                    context,
                    instance_id,
                    entity,
                    viewport_window_config.transparent,
                    snapshot.flags,
                    ViewportCameraReconciliation {
                        live: &live_cameras,
                        recoverable: &recoverable_cameras,
                        pending: &mut pending_cameras,
                    },
                );
                if let Ok(mut window) = windows.get_mut(entity) {
                    apply_snapshot_to_window(&snapshot, entity, &mut window);
                } else {
                    pending_windows.insert(
                        instance_id,
                        window_from_snapshot_with_config(&snapshot, viewport_window_config)
                            .expect("the viewport window configuration was validated"),
                    );
                }
                feedback_candidates.insert(instance_id);
            }
            ImguiViewportCommand::Destroy { .. } => {
                pending_windows.remove(&instance_id);
                if let Some(entity) = context.remove_viewport_window(instance_id) {
                    native_window::release_pointer_capture_for(entity);
                    context.track_ecs_despawn(entity);
                    ecs_commands.entity(entity).try_despawn();
                }
                context.remove_viewport_feedback(instance_id);
                context.remove_viewport_flags(instance_id);
                context.remove_pending_client_placement(instance_id);
                context.clear_focus_request(instance_id);
                #[cfg(feature = "render")]
                {
                    pending_cameras.remove(&instance_id);
                    if let Some(entity) = context.remove_viewport_camera(instance_id) {
                        scheduled_camera_despawns.insert(entity);
                        context.track_ecs_despawn(entity);
                        ecs_commands.entity(entity).try_despawn();
                    }
                }
                let mut state = context.inner.state.borrow_mut();
                let handle_is_live = state
                    .record(instance_id)
                    .is_some_and(|record| record.handle.is_some());
                if !handle_is_live {
                    state.remove_instance(instance_id);
                }
            }
            ImguiViewportCommand::Show { .. } => {
                let should_focus = context.show_should_focus(instance_id);
                let show_is_deferred = {
                    let mut state = context.inner.state.borrow_mut();
                    state
                        .record_mut(instance_id)
                        .and_then(|record| record.pending_client_placement.as_mut())
                        .is_some_and(|placement| {
                            placement.show_requested = true;
                            placement.focus_requested |= should_focus;
                            true
                        })
                };
                if !show_is_deferred {
                    if let Some(window) = pending_windows.get_mut(&instance_id) {
                        window.visible = true;
                        if should_focus {
                            window.focused = false;
                        }
                    } else {
                        with_window_mut(windows, context, instance_id, |window| {
                            window.visible = true;
                            if should_focus {
                                window.focused = false;
                            }
                        });
                    }
                    if should_focus {
                        context.request_focus_next_frame(instance_id);
                    }
                }
                feedback_candidates.insert(instance_id);
            }
            ImguiViewportCommand::Update {
                id: _,
                previous_flags,
                flags,
            } => {
                if let Some(record) = context.inner.state.borrow_mut().record_mut(instance_id) {
                    record.flags = Some(flags);
                }
                let decoration_changed = previous_flags.is_some_and(|previous| {
                    previous.contains(imgui::ViewportFlags::NO_DECORATION)
                        != flags.contains(imgui::ViewportFlags::NO_DECORATION)
                });
                #[cfg(feature = "render")]
                let renderer_clear_changed = previous_flags.is_some_and(|previous| {
                    previous.contains(imgui::ViewportFlags::NO_RENDERER_CLEAR)
                        != flags.contains(imgui::ViewportFlags::NO_RENDERER_CLEAR)
                });
                let mut was_visible = false;
                if let Some(window) = pending_windows.get_mut(&instance_id) {
                    was_visible = window.visible;
                    if decoration_changed && uses_winit_window_lifecycle {
                        window.visible = false;
                    }
                    apply_viewport_flags_to_window(flags, window);
                } else {
                    with_window_mut(windows, context, instance_id, |window| {
                        was_visible = window.visible;
                        if decoration_changed && uses_winit_window_lifecycle {
                            window.visible = false;
                        }
                        apply_viewport_flags_to_window(flags, window);
                    });
                }
                if decoration_changed
                    && uses_winit_window_lifecycle
                    && let Some(feedback) = context.viewport_feedback_for_instance(instance_id)
                {
                    let mut state = context.inner.state.borrow_mut();
                    let Some(record) = state.record_mut(instance_id) else {
                        continue;
                    };
                    let placement =
                        record
                            .pending_client_placement
                            .get_or_insert(PendingClientPlacement {
                                pos: feedback.pos,
                                dpi_scale: feedback.dpi_scale,
                                show_requested: false,
                                focus_requested: false,
                            });
                    placement.pos = feedback.pos;
                    placement.dpi_scale = feedback.dpi_scale;
                    placement.show_requested |= was_visible;
                }
                if let Some(entity) = context.viewport_window_for_instance(instance_id)
                    && let Ok(mut cursor_options) = cursor_options.get_mut(entity)
                {
                    apply_viewport_flags_to_cursor_options(flags, &mut cursor_options);
                } else if let Some(entity) = context.viewport_window_for_instance(instance_id) {
                    let mut cursor_options = CursorOptions::default();
                    apply_viewport_flags_to_cursor_options(flags, &mut cursor_options);
                    ecs_commands.entity(entity).insert(cursor_options);
                }
                #[cfg(feature = "render")]
                if renderer_clear_changed
                    && let Some(camera) = context.viewport_camera_for_instance(instance_id)
                    && (live_cameras.contains(&(instance_id, camera))
                        || recoverable_cameras.contains(&(instance_id, camera))
                        || pending_cameras.contains(&instance_id))
                {
                    ecs_commands
                        .entity(camera)
                        .insert(viewport_camera(viewport_window_config.transparent, flags));
                }
            }
            ImguiViewportCommand::SetPos {
                id: _,
                pos,
                dpi_scale,
            } => {
                let pos = finite_desktop_pos(pos);
                let dpi_scale = positive_finite_or(dpi_scale, 1.0);
                context.record_position_request(instance_id, pos, dpi_scale);
                if let Some(window) = pending_windows.get_mut(&instance_id) {
                    window.position = WindowPosition::At(physical_pos_from_desktop(pos, dpi_scale));
                } else if let Some(entity) = context.viewport_window_for_instance(instance_id)
                    && let Ok(mut window) = windows.get_mut(entity)
                {
                    window.position = WindowPosition::At(physical_outer_pos_for_client_pos(
                        entity, pos, dpi_scale,
                    ));
                }
            }
            ImguiViewportCommand::SetSize {
                id: _,
                size,
                dpi_scale,
            } => {
                let size = finite_desktop_size(size);
                let dpi_scale = positive_finite_or(dpi_scale, 1.0);
                context.record_size_request(instance_id, size, dpi_scale);
                if let Some(window) = pending_windows.get_mut(&instance_id) {
                    set_window_desktop_size(window, size, dpi_scale);
                } else {
                    with_window_mut(windows, context, instance_id, |window| {
                        set_window_desktop_size(window, size, dpi_scale);
                    });
                }
            }
            ImguiViewportCommand::SetFocus { .. } => {
                if let Some(window) = pending_windows.get_mut(&instance_id) {
                    window.focused = false;
                } else {
                    with_window_mut(windows, context, instance_id, |window| {
                        window.focused = false;
                    });
                }
                let focus_is_deferred = context
                    .inner
                    .state
                    .borrow_mut()
                    .record_mut(instance_id)
                    .and_then(|record| record.pending_client_placement.as_mut())
                    .is_some_and(|placement| {
                        placement.focus_requested = true;
                        true
                    });
                if !focus_is_deferred {
                    context.request_focus_next_frame(instance_id);
                }
                feedback_candidates.insert(instance_id);
            }
            ImguiViewportCommand::SetTitle { id: _, title } => {
                if let Some(window) = pending_windows.get_mut(&instance_id) {
                    window.title = title;
                } else {
                    with_window_mut(windows, context, instance_id, |window| {
                        window.title = title;
                    });
                }
                feedback_candidates.insert(instance_id);
            }
        }
    }

    let pending_instance_ids = pending_windows.keys().copied().collect::<HashSet<_>>();
    for (instance_id, window) in pending_windows {
        if let Some(entity) = context.viewport_window_for_instance(instance_id) {
            let previous = context.viewport_feedback_for_instance(instance_id);
            context.set_viewport_feedback(
                instance_id,
                feedback_from_window_for_entity(entity, &window, previous, None),
            );
            ecs_commands.entity(entity).insert(window);
        }
    }

    for instance_id in feedback_candidates {
        if pending_instance_ids.contains(&instance_id)
            || context
                .inner
                .state
                .borrow()
                .record(instance_id)
                .is_some_and(|record| record.pending_client_placement.is_some())
        {
            continue;
        }
        if let Some(entity) = context.viewport_window_for_instance(instance_id)
            && let Ok(window) = windows.get(entity)
        {
            let previous = context.viewport_feedback_for_instance(instance_id);
            context.set_viewport_feedback(
                instance_id,
                feedback_from_window_for_entity(entity, window, previous, None),
            );
        }
    }

    apply_pending_viewport_focus_requests(windows, context);

    for (window_entity, marker, owner) in viewport_windows.iter() {
        let Some((context_id, instance_id)) = owner.window_identity() else {
            continue;
        };
        if context_id != context.context_id
            || context.viewport_window_for_instance(instance_id) != Some(window_entity)
        {
            continue;
        }
        let Some(viewport_id) = context.viewport_id(instance_id) else {
            continue;
        };
        if marker.is_none_or(|marker| {
            !owner.matches_window(marker) || marker.viewport_id() != viewport_id
        }) {
            ecs_commands
                .entity(window_entity)
                .insert(ImguiViewportWindow::new(instance_id, viewport_id));
        }
        #[cfg(feature = "render")]
        {
            let flags = context
                .inner
                .state
                .borrow()
                .record(instance_id)
                .and_then(|record| record.flags)
                .unwrap_or_else(imgui::ViewportFlags::empty);
            ensure_viewport_camera(
                ecs_commands,
                context,
                instance_id,
                window_entity,
                viewport_window_config.transparent,
                flags,
                ViewportCameraReconciliation {
                    live: &live_cameras,
                    recoverable: &recoverable_cameras,
                    pending: &mut pending_cameras,
                },
            );
        }
    }

    #[cfg(feature = "render")]
    cleanup_orphaned_viewport_cameras(
        ecs_commands,
        context,
        owned_cameras.into_iter(),
        &scheduled_camera_despawns,
    );
    #[cfg(not(feature = "render"))]
    let _ = viewport_cameras;
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) fn acknowledge_viewport_ecs_despawns_system(
    bridge: NonSend<ImguiViewportBridge>,
    entities: Query<Entity>,
) {
    for context in bridge.contexts() {
        context.acknowledge_ecs_despawns(|entity| entities.get(entity).is_ok());
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
fn apply_pending_viewport_focus_requests(
    windows: &mut Query<&mut Window>,
    bridge: &ImguiViewportBridgeContext,
) {
    let ready = bridge
        .inner
        .state
        .borrow_mut()
        .viewports
        .iter_mut()
        .filter_map(|(&instance_id, record)| {
            let ready = record.focus_ready;
            record.focus_ready = false;
            ready.then_some(instance_id)
        })
        .collect::<Vec<_>>();
    for instance_id in ready {
        if let Some(entity) = bridge.viewport_window_for_instance(instance_id)
            && let Ok(mut window) = windows.get_mut(entity)
        {
            window.focused = true;
        }
    }
    let mut state = bridge.inner.state.borrow_mut();
    for record in state.viewports.values_mut() {
        record.focus_ready = record.focus_next_frame;
        record.focus_next_frame = false;
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(crate) fn settle_pending_client_placements(
    windows: &mut Query<&mut Window>,
    bridge: &ImguiViewportBridgeContext,
    mut decoration_offset: impl FnMut(Entity) -> Option<[f32; 2]>,
) {
    let pending = bridge
        .inner
        .state
        .borrow()
        .viewports
        .iter()
        .filter_map(|(&instance_id, record)| {
            record
                .pending_client_placement
                .map(|placement| (instance_id, placement))
        })
        .collect::<Vec<_>>();

    for (instance_id, placement) in pending {
        let Some(entity) = bridge.viewport_window_for_instance(instance_id) else {
            bridge.remove_pending_client_placement(instance_id);
            continue;
        };
        let Some(offset) = decoration_offset(entity) else {
            continue;
        };
        let Ok(mut window) = windows.get_mut(entity) else {
            continue;
        };
        window.position = WindowPosition::At(physical_pos_from_desktop(
            [placement.pos[0] - offset[0], placement.pos[1] - offset[1]],
            placement.dpi_scale,
        ));
        bridge.remove_pending_client_placement(instance_id);
        bridge.record_position_request(instance_id, placement.pos, placement.dpi_scale);
        if placement.show_requested {
            window.visible = true;
        }
        if placement.focus_requested {
            window.focused = false;
            bridge.request_focus_next_frame(instance_id);
        }
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
#[derive(SystemParam)]
pub(super) struct SecondaryViewportHostQueries<'w, 's> {
    primary: Query<'w, 's, Entity, With<PrimaryWindow>>,
    windows: Query<'w, 's, Entity, With<Window>>,
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) fn cleanup_secondary_viewports_when_host_is_unavailable(
    mut ecs_commands: Commands,
    mut close_requests: MessageReader<WindowCloseRequested>,
    host_queries: SecondaryViewportHostQueries,
    contexts: Option<NonSendMut<crate::ImguiContexts>>,
    bridge: NonSend<ImguiViewportBridge>,
    #[cfg(feature = "render")] input_metrics: Res<crate::input::ImguiContextInputMetrics>,
    #[cfg(feature = "render")] resolved_routes: Res<crate::route::ImguiResolvedRoutes>,
) {
    let Some(mut contexts) = contexts else {
        close_requests.read().for_each(drop);
        return;
    };
    let primary_window = host_queries.primary.single().ok();
    let close_requested = close_requests
        .read()
        .map(|event| event.window)
        .collect::<HashSet<_>>();
    #[cfg(feature = "render")]
    let primary_context = contexts.primary_id();

    for context_bridge in bridge.contexts() {
        let context_id = context_bridge.context_id;
        #[cfg(feature = "render")]
        let host_window = resolved_routes
            .render_route(context_id)
            .and_then(crate::route::ImguiResolvedRenderRoute::host_window)
            .or_else(|| {
                resolved_routes
                    .input_route(context_id)
                    .map(crate::route::ImguiResolvedInputRoute::host_window)
            })
            .or_else(|| {
                input_metrics
                    .get(context_id)
                    .map(|metrics| metrics.host_window)
            })
            .or_else(|| {
                (Some(context_id) == primary_context)
                    .then_some(primary_window)
                    .flatten()
            });
        #[cfg(not(feature = "render"))]
        let host_window = primary_window;
        let host_is_unavailable = host_window.is_none_or(|host_window| {
            host_queries.windows.get(host_window).is_err() || close_requested.contains(&host_window)
        });
        if !host_is_unavailable {
            continue;
        }

        let entities = context_bridge.mapped_ecs_entities();
        context_bridge.track_ecs_despawns(entities.iter().copied());
        for entity in entities {
            native_window::release_pointer_capture_for(entity);
            ecs_commands.entity(entity).try_despawn();
        }

        let result = contexts.configure(context_id, |context| {
            clear_imgui_viewport_platform_handles(context, &context_bridge);
        });
        let native_handles_cleared = match result {
            Ok(()) => true,
            Err(
                crate::ImguiContextError::TeardownInProgress { .. }
                | crate::ImguiContextError::UnknownContext { .. },
            ) => false,
            Err(error) => {
                panic!("cannot clear Dear ImGui viewport handles for {context_id:?}: {error}")
            }
        };
        if native_handles_cleared {
            context_bridge
                .inner
                .clear_viewport_state_preserving_pending_despawns();
        } else {
            // Teardown still owns the Context and must clear its raw fields before these boxes drop.
            context_bridge
                .inner
                .clear_viewport_state_preserving_native_handles();
        }
    }
}

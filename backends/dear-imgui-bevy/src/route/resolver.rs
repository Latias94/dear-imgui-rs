use super::*;

#[derive(Clone)]
struct CameraRecord {
    entity: Entity,
    camera: Camera,
    target: RenderTarget,
    schedule: Option<bevy_ecs::schedule::InternedScheduleLabel>,
}

#[derive(Clone)]
struct ValidatedCamera {
    entity: Entity,
    camera_order: isize,
    camera_schedule: InternedScheduleLabel,
    target: NormalizedRenderTarget,
    target_info: RenderTargetInfo,
    camera_viewport: Option<Viewport>,
}

impl ValidatedCamera {
    fn target_kind(&self) -> ImguiRenderTargetKind {
        render_target_kind(&self.target)
    }

    fn logical_window_region(&self) -> Option<(Entity, Rect)> {
        let NormalizedRenderTarget::Window(window) = &self.target else {
            return None;
        };
        let scale = self.target_info.scale_factor;
        if !scale.is_finite() || scale <= 0.0 {
            return None;
        }
        let (position, size) = self
            .camera_viewport
            .as_ref()
            .map_or((UVec2::ZERO, self.target_info.physical_size), |viewport| {
                (viewport.physical_position, viewport.physical_size)
            });
        let min = position.as_vec2() / scale;
        let max = min + size.as_vec2() / scale;
        Some((window.entity(), Rect { min, max }))
    }
}

struct ResolverWorld<'a> {
    registered_contexts: &'a HashSet<ContextId>,
    primary_context: Option<ContextId>,
    primary_windows: &'a [Entity],
    windows: &'a [(Entity, Window)],
    images: &'a Assets<Image>,
    manual_texture_views: &'a ManualTextureViews,
    cameras: &'a [CameraRecord],
}

#[derive(SystemParam)]
pub(super) struct RouteResolverParams<'w, 's> {
    contexts: Option<NonSend<'w, ImguiContexts>>,
    primary_windows: Query<'w, 's, Entity, With<PrimaryWindow>>,
    windows: Query<'w, 's, (Entity, &'static Window)>,
    images: Option<Res<'w, Assets<Image>>>,
    manual_texture_views: Option<Res<'w, ManualTextureViews>>,
    cameras: Query<
        'w,
        's,
        (
            Entity,
            &'static Camera,
            &'static RenderTarget,
            Option<&'static CameraRenderGraph>,
        ),
    >,
    render_routes: Query<'w, 's, (Entity, &'static ImguiRenderRoute)>,
    input_routes: Query<'w, 's, (Entity, &'static ImguiInputRoute)>,
}

/// Resolve all route declarations against the current main world.
///
/// This is an internal plugin system. Public consumers should inspect [`ImguiDiagnostics`] instead
/// of scheduling it directly.
///
pub(super) fn resolve_imgui_routes(
    mut resolved: ResMut<ImguiResolvedRoutes>,
    diagnostics: Res<ImguiDiagnostics>,
    params: RouteResolverParams,
) {
    let mut primary_windows = params.primary_windows.iter().collect::<Vec<_>>();
    primary_windows.sort();
    let windows = params
        .windows
        .iter()
        .map(|(entity, window)| (entity, window.clone()))
        .collect::<Vec<_>>();
    let empty_images = Assets::<Image>::default();
    let images = params.images.as_deref().unwrap_or(&empty_images);
    let empty_manual_texture_views = ManualTextureViews::default();
    let manual_texture_views = params
        .manual_texture_views
        .as_deref()
        .unwrap_or(&empty_manual_texture_views);
    let registered_contexts = params
        .contexts
        .as_deref()
        .and_then(|contexts| contexts.ids().ok())
        .map(|ids| ids.collect::<HashSet<_>>())
        .unwrap_or_default();
    let mut cameras = params
        .cameras
        .iter()
        .map(|(entity, camera, target, schedule)| CameraRecord {
            entity,
            camera: camera.clone(),
            target: target.clone(),
            schedule: schedule.map(|schedule| schedule.0),
        })
        .collect::<Vec<_>>();
    cameras.sort_by_key(|camera| camera.entity);
    let mut render_declarations = params
        .render_routes
        .iter()
        .map(|(entity, route)| (entity, *route))
        .collect::<Vec<_>>();
    render_declarations.sort_by(|(left_entity, left), (right_entity, right)| {
        context_key(left.context_id)
            .cmp(&context_key(right.context_id))
            .then_with(|| left_entity.cmp(right_entity))
    });
    let mut input_declarations = params
        .input_routes
        .iter()
        .map(|(entity, route)| (entity, *route))
        .collect::<Vec<_>>();
    input_declarations.sort_by(|(left_entity, left), (right_entity, right)| {
        context_key(left.context_id)
            .cmp(&context_key(right.context_id))
            .then_with(|| left_entity.cmp(right_entity))
    });

    let route_world = ResolverWorld {
        registered_contexts: &registered_contexts,
        primary_context: params
            .contexts
            .as_deref()
            .and_then(|contexts| contexts.primary_id().ok().flatten()),
        primary_windows: &primary_windows,
        windows: &windows,
        images,
        manual_texture_views,
        cameras: &cameras,
    };
    let (mut resolved_render, render_diagnostics) =
        resolve_render_routes(&route_world, &render_declarations);
    resolved_render.sort_by(compare_render_routes);
    let (mut resolved_input, input_diagnostics) =
        resolve_input_routes(&route_world, &input_declarations, &resolved_render);
    resolved_input.sort_by(compare_input_routes);

    let epoch = resolved.replace(resolved_render, resolved_input);
    diagnostics.replace(
        ImguiDiagnosticOrigin::RenderRouting,
        epoch,
        render_diagnostics,
    );
    diagnostics.replace(
        ImguiDiagnosticOrigin::InputRouting,
        epoch,
        input_diagnostics,
    );
}

fn resolve_render_routes(
    world: &ResolverWorld<'_>,
    declarations: &[(Entity, ImguiRenderRoute)],
) -> (Vec<ImguiResolvedRenderRoute>, Vec<ImguiDiagnostic>) {
    let mut resolved = Vec::new();
    let mut diagnostics = Vec::new();
    let explicit_primary = world.primary_context.is_some_and(|primary| {
        declarations
            .iter()
            .any(|(_, declaration)| declaration.context_id == primary)
    });

    let mut start = 0;
    while start < declarations.len() {
        let context_id = declarations[start].1.context_id;
        let mut end = start + 1;
        while end < declarations.len() && declarations[end].1.context_id == context_id {
            end += 1;
        }
        let group = &declarations[start..end];
        if group.len() > 1 {
            diagnostics.extend(group.iter().map(|(route_entity, declaration)| {
                ImguiDiagnostic::new(ImguiDiagnosticKind::DuplicateRenderRoute {
                    declarations: group.len(),
                })
                .with_context(context_id)
                .with_route(*route_entity)
                .with_camera(declaration.camera)
            }));
        } else if !world.registered_contexts.contains(&context_id) {
            let (route_entity, declaration) = group[0];
            diagnostics.push(
                ImguiDiagnostic::new(ImguiDiagnosticKind::UnknownContext)
                    .with_context(context_id)
                    .with_route(route_entity)
                    .with_camera(declaration.camera),
            );
        } else {
            let (route_entity, declaration) = group[0];
            match validate_camera(world, declaration.camera) {
                Ok(camera) => resolved.push(ImguiResolvedRenderRoute {
                    context_id,
                    route_entity: Some(route_entity),
                    camera: camera.entity,
                    order: declaration.order,
                    camera_order: camera.camera_order,
                    camera_schedule: camera.camera_schedule,
                    #[cfg(test)]
                    source: ImguiRenderRouteSource::Explicit,
                    target: camera.target,
                    target_info: camera.target_info,
                    camera_viewport: camera.camera_viewport,
                }),
                Err(kind) => diagnostics.push(
                    ImguiDiagnostic::new(kind)
                        .with_context(context_id)
                        .with_route(route_entity)
                        .with_camera(declaration.camera),
                ),
            }
        }
        start = end;
    }

    if !explicit_primary && let Some(primary_context) = world.primary_context {
        resolve_auto_primary(world, primary_context, &mut resolved, &mut diagnostics);
    }

    (resolved, diagnostics)
}

fn resolve_auto_primary(
    world: &ResolverWorld<'_>,
    primary_context: ContextId,
    resolved: &mut Vec<ImguiResolvedRenderRoute>,
    diagnostics: &mut Vec<ImguiDiagnostic>,
) {
    let primary_window = match world.primary_windows {
        [] => {
            diagnostics.push(
                ImguiDiagnostic::new(ImguiDiagnosticKind::MissingPrimaryWindow)
                    .with_context(primary_context),
            );
            return;
        }
        [primary_window] => *primary_window,
        primary_windows => {
            diagnostics.push(
                ImguiDiagnostic::new(ImguiDiagnosticKind::AmbiguousPrimaryWindow {
                    count: primary_windows.len(),
                })
                .with_context(primary_context),
            );
            return;
        }
    };

    let mut candidates = Vec::new();
    for camera in world.cameras {
        let Some(normalized) = camera.target.normalize(Some(primary_window)) else {
            continue;
        };
        let NormalizedRenderTarget::Window(window) = &normalized else {
            continue;
        };
        if window.entity() != primary_window {
            continue;
        }
        match validate_camera(world, camera.entity) {
            Ok(camera) => candidates.push(camera),
            Err(kind) => diagnostics.push(
                ImguiDiagnostic::new(kind)
                    .with_context(primary_context)
                    .with_camera(camera.entity),
            ),
        }
    }

    let Some(highest_order) = candidates.iter().map(|camera| camera.camera_order).max() else {
        diagnostics.push(
            ImguiDiagnostic::new(ImguiDiagnosticKind::NoEligibleAutoPrimaryCamera)
                .with_context(primary_context),
        );
        return;
    };
    let mut highest = candidates
        .into_iter()
        .filter(|camera| camera.camera_order == highest_order);
    let winner = highest
        .next()
        .expect("a maximum camera order must have at least one candidate");
    if let Some(second) = highest.next() {
        diagnostics.push(
            ImguiDiagnostic::new(ImguiDiagnosticKind::AmbiguousAutoPrimary {
                order: highest_order,
            })
            .with_context(primary_context)
            .with_camera(winner.entity),
        );
        diagnostics.push(
            ImguiDiagnostic::new(ImguiDiagnosticKind::AmbiguousAutoPrimary {
                order: highest_order,
            })
            .with_context(primary_context)
            .with_camera(second.entity),
        );
        diagnostics.extend(highest.map(|camera| {
            ImguiDiagnostic::new(ImguiDiagnosticKind::AmbiguousAutoPrimary {
                order: highest_order,
            })
            .with_context(primary_context)
            .with_camera(camera.entity)
        }));
        return;
    }

    resolved.push(ImguiResolvedRenderRoute {
        context_id: primary_context,
        route_entity: None,
        camera: winner.entity,
        order: 0,
        camera_order: winner.camera_order,
        camera_schedule: winner.camera_schedule,
        #[cfg(test)]
        source: ImguiRenderRouteSource::AutoPrimary,
        target: winner.target,
        target_info: winner.target_info,
        camera_viewport: winner.camera_viewport,
    });
}

fn resolve_input_routes(
    world: &ResolverWorld<'_>,
    declarations: &[(Entity, ImguiInputRoute)],
    render_routes: &[ImguiResolvedRenderRoute],
) -> (Vec<ImguiResolvedInputRoute>, Vec<ImguiDiagnostic>) {
    let explicitly_declared = declarations
        .iter()
        .map(|(_, declaration)| declaration.context_id)
        .collect::<HashSet<_>>();
    let mut resolved = Vec::new();
    let mut diagnostics = Vec::new();

    let mut start = 0;
    while start < declarations.len() {
        let context_id = declarations[start].1.context_id;
        let mut end = start + 1;
        while end < declarations.len() && declarations[end].1.context_id == context_id {
            end += 1;
        }
        let group = &declarations[start..end];
        if group.len() > 1 {
            diagnostics.extend(group.iter().map(|(route_entity, declaration)| {
                input_diagnostic(
                    ImguiDiagnosticKind::DuplicateInputRoute {
                        declarations: group.len(),
                    },
                    context_id,
                    *route_entity,
                    declaration.source,
                )
            }));
        } else if !world.registered_contexts.contains(&context_id) {
            let (route_entity, declaration) = group[0];
            diagnostics.push(input_diagnostic(
                ImguiDiagnosticKind::UnknownContext,
                context_id,
                route_entity,
                declaration.source,
            ));
        } else {
            let (route_entity, declaration) = group[0];
            if declaration.policy != ImguiInputPolicy::Disabled {
                match validate_input_source(world, declaration.source) {
                    Ok((host_window, logical_region)) => {
                        resolved.push(ImguiResolvedInputRoute {
                            context_id,
                            route_entity: Some(route_entity),
                            source: declaration.source,
                            policy: declaration.policy,
                            host_window,
                            logical_region,
                        });
                    }
                    Err(kind) => diagnostics.push(input_diagnostic(
                        kind,
                        context_id,
                        route_entity,
                        declaration.source,
                    )),
                }
            }
        }
        start = end;
    }

    for render_route in render_routes {
        if explicitly_declared.contains(&render_route.context_id) {
            continue;
        }
        let validated = ValidatedCamera {
            entity: render_route.camera,
            camera_order: render_route.camera_order,
            camera_schedule: render_route.camera_schedule,
            target: render_route.target.clone(),
            target_info: render_route.target_info.clone(),
            camera_viewport: render_route.camera_viewport.clone(),
        };
        let Some((host_window, logical_region)) = validated.logical_window_region() else {
            continue;
        };
        resolved.push(ImguiResolvedInputRoute {
            context_id: render_route.context_id,
            route_entity: None,
            source: ImguiInputSource::camera(render_route.camera),
            policy: ImguiInputPolicy::Exclusive { priority: 0 },
            host_window,
            logical_region,
        });
    }

    remove_ambiguous_exclusive_input(&mut resolved, &mut diagnostics);
    (resolved, diagnostics)
}

fn validate_input_source(
    world: &ResolverWorld<'_>,
    source: ImguiInputSource,
) -> Result<(Entity, Rect), ImguiDiagnosticKind> {
    match source {
        ImguiInputSource::Camera(source) => {
            let camera = validate_camera(world, source.camera)?;
            camera.logical_window_region().ok_or(
                ImguiDiagnosticKind::InputCameraRequiresWindowTarget {
                    target: camera.target_kind(),
                },
            )
        }
        ImguiInputSource::Logical(source) => {
            let Some((_, window)) = world
                .windows
                .iter()
                .find(|(entity, _)| *entity == source.window)
            else {
                return Err(ImguiDiagnosticKind::MissingLogicalInputWindow {
                    window: source.window,
                });
            };
            if window.physical_width() == 0 || window.physical_height() == 0 {
                return Err(ImguiDiagnosticKind::MissingLogicalInputWindow {
                    window: source.window,
                });
            }
            if !valid_rect(source.region) {
                return Err(ImguiDiagnosticKind::InvalidLogicalInputRegion);
            }
            Ok((source.window, source.region))
        }
    }
}

fn remove_ambiguous_exclusive_input(
    routes: &mut Vec<ImguiResolvedInputRoute>,
    diagnostics: &mut Vec<ImguiDiagnostic>,
) {
    let mut ambiguous = HashSet::new();
    for left in 0..routes.len() {
        let ImguiInputPolicy::Exclusive {
            priority: left_priority,
        } = routes[left].policy
        else {
            continue;
        };
        for right in (left + 1)..routes.len() {
            let ImguiInputPolicy::Exclusive {
                priority: right_priority,
            } = routes[right].policy
            else {
                continue;
            };
            if routes[left].context_id != routes[right].context_id
                && routes[left].host_window == routes[right].host_window
                && left_priority == right_priority
                && rects_overlap(routes[left].logical_region, routes[right].logical_region)
            {
                ambiguous.insert(left);
                ambiguous.insert(right);
            }
        }
    }

    for index in ambiguous.iter().copied() {
        let route = routes[index];
        let priority = route
            .policy
            .priority()
            .expect("only exclusive routes are marked ambiguous");
        let mut diagnostic =
            ImguiDiagnostic::new(ImguiDiagnosticKind::AmbiguousExclusiveInput { priority })
                .with_context(route.context_id);
        if let Some(route_entity) = route.route_entity {
            diagnostic = diagnostic.with_route(route_entity);
        }
        if let Some(camera) = route.source.as_camera() {
            diagnostic = diagnostic.with_camera(camera.camera);
        }
        diagnostics.push(diagnostic);
    }

    let mut index = 0;
    routes.retain(|_| {
        let keep = !ambiguous.contains(&index);
        index += 1;
        keep
    });
}

fn validate_camera(
    world: &ResolverWorld<'_>,
    camera_entity: Entity,
) -> Result<ValidatedCamera, ImguiDiagnosticKind> {
    let camera = world
        .cameras
        .iter()
        .find(|camera| camera.entity == camera_entity)
        .ok_or(ImguiDiagnosticKind::MissingCamera)?;
    if !camera.camera.is_active {
        return Err(ImguiDiagnosticKind::InactiveCamera);
    }
    if !matches!(camera.camera.output_mode, CameraOutputMode::Write { .. }) {
        return Err(ImguiDiagnosticKind::CameraDoesNotWrite);
    }
    let Some(camera_schedule) = camera
        .schedule
        .filter(|schedule| *schedule == Core2d.intern() || *schedule == Core3d.intern())
    else {
        return Err(ImguiDiagnosticKind::UnsupportedCameraSchedule);
    };

    let primary_window = match world.primary_windows {
        [primary_window] => Some(*primary_window),
        _ => None,
    };
    let Some(target) = camera.target.normalize(primary_window) else {
        return Err(ImguiDiagnosticKind::UnresolvedPrimaryWindowTarget {
            candidates: world.primary_windows.len(),
        });
    };
    if matches!(target, NormalizedRenderTarget::None { .. }) {
        return Err(ImguiDiagnosticKind::UnsupportedRenderTargetNone);
    }
    let target_info = target
        .get_render_target_info(
            world
                .windows
                .iter()
                .map(|(entity, window)| (*entity, window)),
            world.images,
            world.manual_texture_views,
        )
        .map_err(missing_target_diagnostic)?;
    let target_kind = render_target_kind(&target);
    if target_info.physical_size.x == 0 || target_info.physical_size.y == 0 {
        return Err(ImguiDiagnosticKind::ZeroSizedRenderTarget {
            target: target_kind,
        });
    }
    if !target_info.scale_factor.is_finite() || target_info.scale_factor <= 0.0 {
        return Err(ImguiDiagnosticKind::InvalidRenderTargetScaleFactor {
            target: target_kind,
        });
    }
    let mut camera_viewport = camera.camera.viewport.clone();
    if let Some(viewport) = &mut camera_viewport {
        viewport.clamp_to_size(target_info.physical_size);
    }
    if camera_viewport
        .as_ref()
        .is_some_and(|viewport| viewport.physical_size.x == 0 || viewport.physical_size.y == 0)
    {
        return Err(ImguiDiagnosticKind::ZeroSizedRenderTarget {
            target: target_kind,
        });
    }

    Ok(ValidatedCamera {
        entity: camera.entity,
        camera_order: camera.camera.order,
        camera_schedule,
        target,
        target_info,
        camera_viewport,
    })
}

fn missing_target_diagnostic(error: MissingRenderTargetInfoError) -> ImguiDiagnosticKind {
    match error {
        MissingRenderTargetInfoError::Window { window } => {
            ImguiDiagnosticKind::MissingWindowTarget { window }
        }
        MissingRenderTargetInfoError::Image { image } => {
            ImguiDiagnosticKind::MissingImageTarget { image }
        }
        MissingRenderTargetInfoError::TextureView { texture_view } => {
            ImguiDiagnosticKind::MissingManualTextureViewTarget { texture_view }
        }
    }
}

fn input_diagnostic(
    kind: ImguiDiagnosticKind,
    context_id: ContextId,
    route_entity: Entity,
    source: ImguiInputSource,
) -> ImguiDiagnostic {
    let mut diagnostic = ImguiDiagnostic::new(kind)
        .with_context(context_id)
        .with_route(route_entity);
    if let Some(source) = source.as_camera() {
        diagnostic = diagnostic.with_camera(source.camera);
    }
    diagnostic
}

fn render_target_kind(target: &NormalizedRenderTarget) -> ImguiRenderTargetKind {
    match target {
        NormalizedRenderTarget::Window(_) => ImguiRenderTargetKind::Window,
        NormalizedRenderTarget::Image(_) => ImguiRenderTargetKind::Image,
        NormalizedRenderTarget::TextureView(_) => ImguiRenderTargetKind::ManualTextureView,
        NormalizedRenderTarget::None { .. } => ImguiRenderTargetKind::None,
    }
}

fn compare_render_routes(
    left: &ImguiResolvedRenderRoute,
    right: &ImguiResolvedRenderRoute,
) -> Ordering {
    left.camera
        .cmp(&right.camera)
        .then_with(|| left.order.cmp(&right.order))
        .then_with(|| context_key(left.context_id).cmp(&context_key(right.context_id)))
        .then_with(|| left.route_entity.cmp(&right.route_entity))
}

fn compare_input_routes(
    left: &ImguiResolvedInputRoute,
    right: &ImguiResolvedInputRoute,
) -> Ordering {
    left.host_window
        .cmp(&right.host_window)
        .then_with(|| context_key(left.context_id).cmp(&context_key(right.context_id)))
        .then_with(|| left.route_entity.cmp(&right.route_entity))
}

fn context_key(context_id: ContextId) -> u64 {
    context_id.get().get()
}

pub(super) fn optional_context_key(context_id: Option<ContextId>) -> Option<u64> {
    context_id.map(context_key)
}

fn valid_rect(rect: Rect) -> bool {
    finite_vec2(rect.min)
        && finite_vec2(rect.max)
        && rect.max.x > rect.min.x
        && rect.max.y > rect.min.y
}

fn finite_vec2(value: Vec2) -> bool {
    value.x.is_finite() && value.y.is_finite()
}

fn rects_overlap(left: Rect, right: Rect) -> bool {
    left.min.x < right.max.x
        && left.max.x > right.min.x
        && left.min.y < right.max.y
        && left.max.y > right.min.y
}

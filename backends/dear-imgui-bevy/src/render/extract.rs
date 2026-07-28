//! Main-world route extraction and render-world view resolution.

use super::resources::ImguiRenderRouteSnapshot;
use super::*;

type ExtractedViewCandidate<'a> = (
    &'a ExtractedView,
    &'a ExtractedCamera,
    &'a CameraMainTextureUsages,
    &'a Msaa,
);

struct IndexedExtractedView<'a> {
    first: ExtractedViewCandidate<'a>,
    has_duplicates: bool,
}

pub(super) fn extract_imgui_bevy_textures(
    registry: Extract<Option<Res<crate::ImguiBevyTextures>>>,
    mut extracted: ResMut<ImguiExtractedBevyTextures>,
) {
    let extraction = registry
        .as_ref()
        .map(|registry| registry.extract_for_render())
        .unwrap_or_default();
    extracted.replace(extraction);
}

pub(super) fn extract_imgui_render_frame(
    mut extracted: ResMut<ImguiExtractedRenderFrame>,
    snapshot_mailbox: Res<crate::context::ImguiFrameMailbox>,
    renderer_releases: Res<ImguiRendererReleases>,
    resolved_routes: Extract<Res<crate::route::ImguiResolvedRoutes>>,
    primary_window: Extract<Query<Entity, With<PrimaryWindow>>>,
    viewport_cameras: Extract<ViewportCameraQuery<'_>>,
    viewport_windows: Extract<ViewportWindowQuery<'_>>,
) {
    let mut pending = snapshot_mailbox.take_all().into_iter().collect::<Vec<_>>();
    pending.sort_by_key(|(context_id, _)| context_id.get().get());
    let route_epoch = pending.first().map_or_else(
        || resolved_routes.epoch(),
        |(_, frame)| frame.render_routes.epoch(),
    );
    debug_assert!(
        pending
            .iter()
            .all(|(_, frame)| frame.render_routes.epoch() == route_epoch),
        "all Context snapshots from one driver pass must capture the same render-route epoch"
    );
    extracted.begin_extraction(route_epoch, snapshot_mailbox.completion_watermarks());

    for (context_id, frame) in pending {
        if renderer_releases.release_requested(context_id) {
            drop(frame);
            continue;
        }

        let frame_route_epoch = frame.render_routes.epoch();
        let mut route_snapshots = frame
            .render_routes
            .render_routes()
            .iter()
            .filter(|route| route.context_id() == context_id)
            .map(|route| ImguiRenderRouteSnapshot {
                context_id: route.context_id(),
                route_epoch: frame_route_epoch,
                route_entity: route.route_entity(),
                camera: route.camera(),
                order: route.order(),
                camera_order: route.camera_order(),
                camera_schedule: route.camera_schedule(),
                target: route.target().clone(),
                physical_target_size: [
                    route.target_info().physical_size.x,
                    route.target_info().physical_size.y,
                ],
                viewport_id: None,
                camera_viewport: route.camera_viewport().map(ImguiCameraViewport::from),
            })
            .collect::<Vec<_>>();

        if frame.include_platform_viewports {
            let primary_window = primary_window.single().ok();
            route_snapshots.extend(viewport_cameras.iter().filter_map(
                |(camera_entity, camera, target, camera_graph, viewport_camera, camera_owner)| {
                    if !camera_owner.matches_camera(viewport_camera) {
                        return None;
                    }
                    if !camera.is_active
                        || !matches!(camera.output_mode, CameraOutputMode::Write { .. })
                    {
                        return None;
                    }
                    let camera_schedule = camera_graph.0;
                    if camera_schedule != Core2d.intern() && camera_schedule != Core3d.intern() {
                        return None;
                    }
                    let target = target.normalize(primary_window)?;
                    let NormalizedRenderTarget::Window(window) = &target else {
                        return None;
                    };
                    let Ok((window, viewport_window, window_owner)) =
                        viewport_windows.get(window.entity())
                    else {
                        return None;
                    };
                    if !window_owner.matches_window(viewport_window)
                        || viewport_window.context_id() != context_id
                        || viewport_camera.context_id() != context_id
                        || viewport_window.viewport_id() != viewport_camera.viewport_id()
                    {
                        return None;
                    }
                    let physical_target_size = window.physical_size();
                    let mut camera_viewport = camera.viewport.clone();
                    if let Some(viewport) = &mut camera_viewport {
                        viewport.clamp_to_size(physical_target_size);
                    }
                    Some(ImguiRenderRouteSnapshot {
                        context_id,
                        route_epoch: frame_route_epoch,
                        route_entity: None,
                        camera: camera_entity,
                        order: 0,
                        camera_order: camera.order,
                        camera_schedule,
                        target,
                        physical_target_size: [physical_target_size.x, physical_target_size.y],
                        viewport_id: Some(viewport_camera.viewport_id()),
                        camera_viewport: camera_viewport.as_ref().map(ImguiCameraViewport::from),
                    })
                },
            ));
        }

        route_snapshots
            .sort_by_key(|route| (route.viewport_id.is_some(), route.order, route.camera));
        extracted.replace(context_id, frame, route_snapshots);
    }
}

pub(super) fn resolve_extracted_imgui_render_routes(
    mut extracted: ResMut<ImguiExtractedRenderFrame>,
    views: Query<(
        &ExtractedView,
        &ExtractedCamera,
        &CameraMainTextureUsages,
        &Msaa,
    )>,
    diagnostics: Res<crate::route::ImguiDiagnostics>,
) {
    let route_epoch = extracted.route_epoch();
    let mut context_ids = extracted.context_ids().collect::<Vec<_>>();
    context_ids.sort_by_key(|context_id| context_id.get().get());
    let mut route_diagnostics = Vec::new();
    let mut views_by_main_camera = HashMap::<Entity, IndexedExtractedView<'_>>::new();
    for candidate @ (view, _, _, _) in &views {
        views_by_main_camera
            .entry(view.retained_view_entity.main_entity.id())
            .and_modify(|indexed| indexed.has_duplicates = true)
            .or_insert(IndexedExtractedView {
                first: candidate,
                has_duplicates: false,
            });
    }

    for context_id in context_ids {
        let route_snapshots = extracted.route_snapshots(context_id).to_vec();
        let mut camera_targets = Vec::with_capacity(route_snapshots.len());
        for route in route_snapshots {
            let Some(indexed_view) = views_by_main_camera.get(&route.camera) else {
                route_diagnostics.push(extraction_diagnostic(
                    &route,
                    crate::route::ImguiDiagnosticKind::MissingExtractedView,
                ));
                continue;
            };
            let (view, camera, texture_usages, msaa) = indexed_view.first;
            let diagnostic_kind = if indexed_view.has_duplicates {
                Some(crate::route::ImguiDiagnosticKind::StaleExtractedView)
            } else if !matches!(camera.output_mode, CameraOutputMode::Write { .. }) {
                Some(crate::route::ImguiDiagnosticKind::CameraDoesNotWrite)
            } else if camera.schedule != route.camera_schedule
                || (camera.schedule != Core2d.intern() && camera.schedule != Core3d.intern())
            {
                Some(crate::route::ImguiDiagnosticKind::UnsupportedCameraSchedule)
            } else if !extracted_view_matches_route(&route, view, camera)
                || !texture_usages.0.contains(TextureUsages::RENDER_ATTACHMENT)
            {
                Some(crate::route::ImguiDiagnosticKind::StaleExtractedView)
            } else {
                None
            };
            if let Some(kind) = diagnostic_kind {
                route_diagnostics.push(extraction_diagnostic(&route, kind));
                continue;
            }

            camera_targets.push(ImguiCameraTarget {
                context_id: route.context_id,
                route_epoch,
                camera: route.camera,
                view: view.retained_view_entity,
                order: route.order,
                camera_order: route.camera_order,
                camera_schedule: route.camera_schedule,
                target: route.target,
                target_format: view.target_format,
                texture_usages: texture_usages.0,
                msaa: *msaa,
                physical_target_size: route.physical_target_size,
                viewport_id: route.viewport_id,
                camera_viewport: route.camera_viewport,
            });
        }

        camera_targets.sort_by_key(|target| {
            (
                target.camera,
                target.order,
                target.context_id.get().get(),
                target.viewport_id.is_some(),
            )
        });
        extracted.replace_camera_targets(context_id, camera_targets);
    }
    diagnostics.replace(
        crate::route::ImguiDiagnosticOrigin::RenderExtraction,
        route_epoch,
        route_diagnostics,
    );
}

fn extracted_view_matches_route(
    route: &ImguiRenderRouteSnapshot,
    view: &ExtractedView,
    camera: &ExtractedCamera,
) -> bool {
    if camera.target.as_ref() != Some(&route.target)
        || camera.order != route.camera_order
        || camera.physical_target_size.map(|size| [size.x, size.y])
            != Some(route.physical_target_size)
    {
        return false;
    }

    let mut camera_viewport = camera.viewport.clone();
    if let Some(viewport) = &mut camera_viewport {
        viewport.clamp_to_size(UVec2::from(route.physical_target_size));
    }
    let camera_viewport = camera_viewport.as_ref().map(ImguiCameraViewport::from);
    let (viewport_position, viewport_size) = route
        .camera_viewport
        .map_or(([0, 0], route.physical_target_size), |viewport| {
            (viewport.physical_position, viewport.physical_size)
        });

    camera_viewport == route.camera_viewport
        && camera.physical_viewport_size.map(|size| [size.x, size.y]) == Some(viewport_size)
        && view.viewport
            == UVec4::new(
                viewport_position[0],
                viewport_position[1],
                viewport_size[0],
                viewport_size[1],
            )
}

fn extraction_diagnostic(
    route: &ImguiRenderRouteSnapshot,
    kind: crate::route::ImguiDiagnosticKind,
) -> crate::route::ImguiDiagnostic {
    let mut diagnostic = crate::route::ImguiDiagnostic::new(kind)
        .with_context(route.context_id)
        .with_camera(route.camera);
    if let Some(route_entity) = route.route_entity {
        diagnostic = diagnostic.with_route(route_entity);
    }
    diagnostic
}

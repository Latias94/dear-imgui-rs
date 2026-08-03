#![cfg(feature = "render")]

use std::time::Duration;

use crate::test_util::imgui_context_guard as context_guard;
use bevy_app::{App, Update};
use bevy_asset::{Assets, RenderAssetUsages};
use bevy_camera::{Camera, CameraOutputMode, ManualTextureViewHandle, RenderTarget, Viewport};
use bevy_core_pipeline::{Core2d, Core3d};
use bevy_ecs::{prelude::*, schedule::ScheduleLabel};
use bevy_image::Image;
use bevy_math::{Rect, UVec2, Vec2};
use bevy_render::{
    camera::CameraRenderGraph,
    render_resource::{Extent3d, TextureDimension, TextureFormat},
    texture::ManualTextureViews,
};
use bevy_time::{Real, Time};
use bevy_window::{PrimaryWindow, Window, WindowRef, WindowResolution};
use dear_imgui_bevy::{
    ImguiContextConfig, ImguiContexts, ImguiPlugin,
    route::{
        ImguiDiagnosticKind, ImguiDiagnosticOrigin, ImguiDiagnostics, ImguiInputPolicy,
        ImguiInputRoute, ImguiInputSource, ImguiRenderRoute, ImguiRenderRouteSource,
        ImguiResolvedRoutes,
    },
};
use dear_imgui_rs::SuspendedContext;

#[derive(ScheduleLabel, Clone, Debug, Eq, Hash, PartialEq)]
struct SecondaryUi;

fn routing_app() -> App {
    let mut app = App::new();
    app.init_resource::<Assets<Image>>();
    app.init_resource::<ManualTextureViews>();
    app.add_plugins(ImguiPlugin::default());
    app
}

fn primary_context(app: &App) -> dear_imgui_bevy::ContextId {
    app.world()
        .non_send::<ImguiContexts>()
        .primary_id()
        .expect("test registry must retain its primary Context")
}

fn add_secondary_context(app: &mut App) -> dear_imgui_bevy::ContextId {
    app.init_schedule(crate::ImguiContextPass::new(SecondaryUi));
    app.world_mut()
        .non_send_mut::<ImguiContexts>()
        .create(ImguiContextConfig::new(crate::ImguiContextPass::new(
            SecondaryUi,
        )))
        .expect("secondary Context admission must succeed")
}

fn spawn_window(app: &mut App, primary: bool, size: UVec2) -> Entity {
    let window = Window {
        resolution: WindowResolution::new(size.x, size.y),
        ..Default::default()
    };
    if primary {
        app.world_mut().spawn((window, PrimaryWindow)).id()
    } else {
        app.world_mut().spawn(window).id()
    }
}

fn spawn_camera(app: &mut App, target: RenderTarget, order: isize, active: bool) -> Entity {
    app.world_mut()
        .spawn((
            Camera {
                order,
                is_active: active,
                ..Default::default()
            },
            target,
            CameraRenderGraph::new(Core2d),
        ))
        .id()
}

fn spawn_overlapping_input_routes(
    app: &mut App,
    primary_policy: ImguiInputPolicy,
    secondary_policy: ImguiInputPolicy,
) -> (
    dear_imgui_bevy::ContextId,
    dear_imgui_bevy::ContextId,
    Entity,
) {
    let primary = primary_context(app);
    let secondary = add_secondary_context(app);
    let window = spawn_window(app, true, UVec2::new(640, 480));
    app.world_mut().spawn(
        ImguiInputRoute::logical(
            primary,
            window,
            Rect::from_corners(Vec2::new(20.0, 20.0), Vec2::new(260.0, 220.0)),
        )
        .with_policy(primary_policy),
    );
    app.world_mut().spawn(
        ImguiInputRoute::logical(
            secondary,
            window,
            Rect::from_corners(Vec2::new(120.0, 80.0), Vec2::new(360.0, 300.0)),
        )
        .with_policy(secondary_policy),
    );
    (primary, secondary, window)
}

fn diagnostics(app: &App, origin: ImguiDiagnosticOrigin) -> Vec<ImguiDiagnosticKind> {
    app.world()
        .resource::<ImguiDiagnostics>()
        .entries_for(origin)
        .map(|diagnostic| diagnostic.kind().clone())
        .collect()
}

#[test]
fn auto_primary_selects_the_unique_highest_eligible_camera() {
    let _guard = context_guard();
    let mut app = routing_app();
    let primary = primary_context(&app);
    let primary_window = spawn_window(&mut app, true, UVec2::new(1280, 720));
    let lower = spawn_camera(&mut app, RenderTarget::Window(WindowRef::Primary), 2, true);
    let winner = spawn_camera(
        &mut app,
        RenderTarget::Window(WindowRef::Entity(primary_window)),
        7,
        true,
    );
    spawn_camera(
        &mut app,
        RenderTarget::Window(WindowRef::Entity(primary_window)),
        99,
        false,
    );

    app.update();

    let routes = app.world().resource::<ImguiResolvedRoutes>();
    let route = routes
        .render_route(primary)
        .expect("the lower valid camera must survive a higher invalid candidate");
    assert_eq!(route.camera(), winner);
    assert_ne!(route.camera(), lower);
    assert_eq!(route.source(), ImguiRenderRouteSource::AutoPrimary);
    assert_eq!(route.target_info().physical_size, UVec2::new(1280, 720));
    assert!(
        diagnostics(&app, ImguiDiagnosticOrigin::RenderRouting)
            .iter()
            .any(|kind| matches!(kind, ImguiDiagnosticKind::InactiveCamera))
    );
}

#[test]
fn auto_primary_fails_closed_for_zero_candidates_and_highest_order_ties() {
    let _guard = context_guard();
    let mut app = routing_app();
    let primary = primary_context(&app);
    spawn_window(&mut app, true, UVec2::new(800, 600));

    app.update();
    assert!(
        app.world()
            .resource::<ImguiResolvedRoutes>()
            .render_route(primary)
            .is_none()
    );
    assert!(
        diagnostics(&app, ImguiDiagnosticOrigin::RenderRouting)
            .iter()
            .any(|kind| matches!(kind, ImguiDiagnosticKind::NoEligibleAutoPrimaryCamera))
    );

    spawn_camera(&mut app, RenderTarget::Window(WindowRef::Primary), 4, true);
    spawn_camera(&mut app, RenderTarget::Window(WindowRef::Primary), 4, true);
    app.update();

    assert!(
        app.world()
            .resource::<ImguiResolvedRoutes>()
            .render_route(primary)
            .is_none()
    );
    assert_eq!(
        diagnostics(&app, ImguiDiagnosticOrigin::RenderRouting)
            .iter()
            .filter(|kind| matches!(kind, ImguiDiagnosticKind::AmbiguousAutoPrimary { order: 4 }))
            .count(),
        2
    );
}

#[test]
fn automatic_routing_never_selects_secondary_or_offscreen_targets() {
    let _guard = context_guard();
    let mut app = routing_app();
    let primary = primary_context(&app);
    let secondary_window = spawn_window(&mut app, false, UVec2::new(640, 480));
    spawn_window(&mut app, true, UVec2::new(1280, 720));
    spawn_camera(
        &mut app,
        RenderTarget::Window(WindowRef::Entity(secondary_window)),
        30,
        true,
    );

    let image = Image::new_fill(
        Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD,
    );
    let image = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    spawn_camera(&mut app, RenderTarget::Image(image.into()), 40, true);
    spawn_camera(
        &mut app,
        RenderTarget::TextureView(ManualTextureViewHandle(9)),
        50,
        true,
    );

    app.update();

    assert!(
        app.world()
            .resource::<ImguiResolvedRoutes>()
            .render_route(primary)
            .is_none()
    );
    assert!(
        diagnostics(&app, ImguiDiagnosticOrigin::RenderRouting)
            .iter()
            .any(|kind| matches!(kind, ImguiDiagnosticKind::NoEligibleAutoPrimaryCamera))
    );
}

#[test]
fn any_explicit_primary_declaration_suppresses_auto_primary_even_when_invalid() {
    let _guard = context_guard();
    let mut app = routing_app();
    let primary = primary_context(&app);
    spawn_window(&mut app, true, UVec2::new(1280, 720));
    let automatic = spawn_camera(&mut app, RenderTarget::Window(WindowRef::Primary), 1, true);
    let stale_camera = app.world_mut().spawn_empty().id();
    app.world_mut().despawn(stale_camera);
    app.world_mut()
        .spawn(ImguiRenderRoute::new(primary, stale_camera));

    app.update();

    let routes = app.world().resource::<ImguiResolvedRoutes>();
    assert!(routes.render_route(primary).is_none());
    assert!(
        routes
            .render_routes()
            .iter()
            .all(|route| route.camera() != automatic)
    );
    let diagnostics = diagnostics(&app, ImguiDiagnosticOrigin::RenderRouting);
    assert!(
        diagnostics
            .iter()
            .any(|kind| matches!(kind, ImguiDiagnosticKind::MissingCamera))
    );
    assert!(
        !diagnostics
            .iter()
            .any(|kind| matches!(kind, ImguiDiagnosticKind::NoEligibleAutoPrimaryCamera))
    );
}

#[test]
fn duplicate_context_declarations_are_all_invalid_and_same_camera_routes_are_stable() {
    let _guard = context_guard();
    let mut app = routing_app();
    let primary = primary_context(&app);
    let window = spawn_window(&mut app, true, UVec2::new(1280, 720));
    let camera = spawn_camera(
        &mut app,
        RenderTarget::Window(WindowRef::Entity(window)),
        0,
        true,
    );
    app.world_mut()
        .spawn(ImguiRenderRoute::new(primary, camera));
    app.world_mut()
        .spawn(ImguiRenderRoute::new(primary, camera).with_order(1));
    app.update();

    assert!(
        app.world()
            .resource::<ImguiResolvedRoutes>()
            .render_route(primary)
            .is_none()
    );
    assert_eq!(
        diagnostics(&app, ImguiDiagnosticOrigin::RenderRouting)
            .iter()
            .filter(|kind| matches!(
                kind,
                ImguiDiagnosticKind::DuplicateRenderRoute { declarations: 2 }
            ))
            .count(),
        2
    );

    let route_entities = app
        .world_mut()
        .query_filtered::<Entity, With<ImguiRenderRoute>>()
        .iter(app.world())
        .collect::<Vec<_>>();
    for entity in route_entities {
        app.world_mut().despawn(entity);
    }
    let secondary = add_secondary_context(&mut app);
    let primary_route = app
        .world_mut()
        .spawn(ImguiRenderRoute::new(primary, camera).with_order(3))
        .id();
    let secondary_route = app
        .world_mut()
        .spawn(ImguiRenderRoute::new(secondary, camera).with_order(3))
        .id();
    app.update();

    let routes = app.world().resource::<ImguiResolvedRoutes>();
    let same_camera = routes
        .render_routes()
        .iter()
        .filter(|route| route.camera() == camera)
        .collect::<Vec<_>>();
    assert_eq!(same_camera.len(), 2);
    let expected = if primary.get().get() < secondary.get().get() {
        [(primary, primary_route), (secondary, secondary_route)]
    } else {
        [(secondary, secondary_route), (primary, primary_route)]
    };
    assert_eq!(
        same_camera
            .iter()
            .map(|route| (route.context_id(), route.route_entity().unwrap()))
            .collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn same_camera_routes_follow_explicit_overlay_order() {
    let _guard = context_guard();
    let mut app = routing_app();
    let primary = primary_context(&app);
    let secondary = add_secondary_context(&mut app);
    let window = spawn_window(&mut app, true, UVec2::new(1280, 720));
    let camera = spawn_camera(
        &mut app,
        RenderTarget::Window(WindowRef::Entity(window)),
        0,
        true,
    );
    app.world_mut()
        .spawn(ImguiRenderRoute::new(primary, camera).with_order(12));
    app.world_mut()
        .spawn(ImguiRenderRoute::new(secondary, camera).with_order(-4));

    app.update();

    assert_eq!(
        app.world()
            .resource::<ImguiResolvedRoutes>()
            .render_routes()
            .iter()
            .map(|route| (route.context_id(), route.order()))
            .collect::<Vec<_>>(),
        vec![(secondary, -4), (primary, 12)],
    );
}

#[test]
fn explicit_targets_validate_window_image_manual_none_and_zero_size() {
    let _guard = context_guard();
    let mut app = routing_app();
    let primary = primary_context(&app);
    let window = spawn_window(&mut app, true, UVec2::new(320, 240));
    let camera = spawn_camera(
        &mut app,
        RenderTarget::Window(WindowRef::Entity(window)),
        0,
        true,
    );
    app.world_mut()
        .spawn(ImguiRenderRoute::new(primary, camera));
    app.update();
    assert!(matches!(
        app.world()
            .resource::<ImguiResolvedRoutes>()
            .render_route(primary)
            .unwrap()
            .target(),
        bevy_camera::NormalizedRenderTarget::Window(_)
    ));

    *app.world_mut()
        .entity_mut(camera)
        .get_mut::<RenderTarget>()
        .unwrap() = RenderTarget::None {
        size: UVec2::new(320, 240),
    };
    app.update();
    assert!(
        diagnostics(&app, ImguiDiagnosticOrigin::RenderRouting)
            .iter()
            .any(|kind| matches!(kind, ImguiDiagnosticKind::UnsupportedRenderTargetNone))
    );

    *app.world_mut()
        .entity_mut(camera)
        .get_mut::<RenderTarget>()
        .unwrap() = RenderTarget::TextureView(ManualTextureViewHandle(17));
    app.update();
    assert!(
        diagnostics(&app, ImguiDiagnosticOrigin::RenderRouting)
            .iter()
            .any(|kind| matches!(
                kind,
                ImguiDiagnosticKind::MissingManualTextureViewTarget { texture_view }
                    if *texture_view == ManualTextureViewHandle(17)
            ))
    );

    let image = Image::new_fill(
        Extent3d {
            width: 32,
            height: 16,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[255, 255, 255, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD,
    );
    let image = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    *app.world_mut()
        .entity_mut(camera)
        .get_mut::<RenderTarget>()
        .unwrap() = RenderTarget::Image(image.clone().into());
    app.update();
    assert_eq!(
        app.world()
            .resource::<ImguiResolvedRoutes>()
            .render_route(primary)
            .unwrap()
            .target_info()
            .physical_size,
        UVec2::new(32, 16)
    );

    app.world_mut()
        .resource_mut::<Assets<Image>>()
        .get_mut(&image)
        .unwrap()
        .texture_descriptor
        .size
        .width = 0;
    app.update();
    assert!(
        diagnostics(&app, ImguiDiagnosticOrigin::RenderRouting)
            .iter()
            .any(|kind| matches!(kind, ImguiDiagnosticKind::ZeroSizedRenderTarget { .. }))
    );
}

#[test]
fn stale_context_camera_and_unsupported_schedule_fail_closed() {
    let _guard = context_guard();
    let mut app = routing_app();
    let stale_context = {
        let context = SuspendedContext::create();
        context.id()
    };
    let window = spawn_window(&mut app, true, UVec2::new(800, 600));
    let stale_camera = app.world_mut().spawn_empty().id();
    app.world_mut().despawn(stale_camera);
    app.world_mut()
        .spawn(ImguiRenderRoute::new(stale_context, stale_camera));
    app.update();
    assert!(
        diagnostics(&app, ImguiDiagnosticOrigin::RenderRouting)
            .iter()
            .any(|kind| matches!(kind, ImguiDiagnosticKind::UnknownContext))
    );

    let primary = primary_context(&app);
    let custom_camera = app
        .world_mut()
        .spawn((
            Camera::default(),
            RenderTarget::Window(WindowRef::Entity(window)),
            CameraRenderGraph::new(Update),
        ))
        .id();
    app.world_mut()
        .spawn(ImguiRenderRoute::new(primary, custom_camera));
    app.update();
    assert!(
        diagnostics(&app, ImguiDiagnosticOrigin::RenderRouting)
            .iter()
            .any(|kind| matches!(kind, ImguiDiagnosticKind::UnsupportedCameraSchedule))
    );
}

#[test]
fn camera_viewport_is_clamped_and_mapped_through_hidpi_scale() {
    let _guard = context_guard();
    let mut app = routing_app();
    let primary = primary_context(&app);
    let window = app
        .world_mut()
        .spawn((
            Window {
                resolution: WindowResolution::new(1000, 800).with_scale_factor_override(2.0),
                ..Default::default()
            },
            PrimaryWindow,
        ))
        .id();
    let camera = app
        .world_mut()
        .spawn((
            Camera {
                viewport: Some(Viewport {
                    physical_position: UVec2::new(900, 700),
                    physical_size: UVec2::new(300, 300),
                    ..Default::default()
                }),
                ..Default::default()
            },
            RenderTarget::Window(WindowRef::Entity(window)),
            CameraRenderGraph::new(Core3d),
        ))
        .id();
    app.world_mut()
        .spawn(ImguiRenderRoute::new(primary, camera));
    app.update();

    let routes = app.world().resource::<ImguiResolvedRoutes>();
    assert_eq!(
        routes
            .render_route(primary)
            .unwrap()
            .camera_viewport()
            .unwrap()
            .physical_size,
        UVec2::new(100, 100)
    );
    assert_eq!(
        routes.render_route(primary).unwrap().physical_output_size(),
        UVec2::new(100, 100),
    );
    let input = routes
        .input_route(primary)
        .expect("a window render route should derive input");
    assert_eq!(input.host_window(), window);
    assert_eq!(
        input.logical_region(),
        Rect {
            min: Vec2::new(450.0, 350.0),
            max: Vec2::new(500.0, 400.0)
        }
    );

    let camera_source = ImguiInputSource::camera(camera);
    assert_eq!(camera_source.as_camera().unwrap().camera(), camera);
    let logical_source =
        ImguiInputSource::logical(window, Rect::from_corners(Vec2::ZERO, Vec2::splat(64.0)));
    assert_eq!(logical_source.as_logical().unwrap().window(), window);
    assert_eq!(ImguiInputPolicy::exclusive(7).priority(), Some(7));
}

#[test]
fn image_routes_do_not_derive_input_and_duplicate_input_declarations_fail_closed() {
    let _guard = context_guard();
    let mut app = routing_app();
    let primary = primary_context(&app);
    let host_window = spawn_window(&mut app, true, UVec2::new(800, 600));
    let image = Image::new_fill(
        Extent3d {
            width: 256,
            height: 128,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD,
    );
    let image = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    let camera = spawn_camera(&mut app, RenderTarget::Image(image.into()), 0, true);
    app.world_mut()
        .spawn(ImguiRenderRoute::new(primary, camera));
    app.update();
    assert!(
        app.world()
            .resource::<ImguiResolvedRoutes>()
            .input_route(primary)
            .is_none()
    );

    let region = Rect::from_corners(Vec2::new(10.0, 20.0), Vec2::new(266.0, 148.0));
    app.world_mut()
        .spawn(ImguiInputRoute::logical(primary, host_window, region));
    app.world_mut()
        .spawn(ImguiInputRoute::from_camera(primary, camera).with_policy(ImguiInputPolicy::Shared));
    app.update();

    assert!(
        app.world()
            .resource::<ImguiResolvedRoutes>()
            .input_route(primary)
            .is_none()
    );
    assert_eq!(
        diagnostics(&app, ImguiDiagnosticOrigin::InputRouting)
            .iter()
            .filter(|kind| matches!(
                kind,
                ImguiDiagnosticKind::DuplicateInputRoute { declarations: 2 }
            ))
            .count(),
        2
    );
}

#[test]
fn image_render_route_drives_secondary_context_metrics_without_an_input_route() {
    let _guard = context_guard();
    let mut app = routing_app();
    let mut real_time = Time::<Real>::default();
    real_time.advance_by(Duration::from_millis(37));
    app.insert_resource(real_time);
    let secondary = add_secondary_context(&mut app);
    let image = Image::new_fill(
        Extent3d {
            width: 256,
            height: 128,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD,
    );
    let image = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    let camera = spawn_camera(&mut app, RenderTarget::Image(image.into()), 0, true);
    app.world_mut()
        .spawn(ImguiRenderRoute::new(secondary, camera));

    // The first update resolves the route; the second drives the Context from that route epoch.
    for _ in 0..2 {
        app.update();
    }

    assert!(
        app.world()
            .resource::<ImguiResolvedRoutes>()
            .input_route(secondary)
            .is_none()
    );
    let metrics = app
        .world_mut()
        .non_send_mut::<ImguiContexts>()
        .configure(secondary, |context| {
            (
                context.io().display_size(),
                context.io().display_framebuffer_scale(),
                context.io().delta_time(),
            )
        })
        .expect("secondary Context metrics must remain inspectable");
    assert_eq!(metrics.0, [256.0, 128.0]);
    assert_eq!(metrics.1, [1.0, 1.0]);
    assert!((metrics.2 - 0.037).abs() < f32::EPSILON);
}

#[test]
fn resolved_routes_and_diagnostics_replace_the_previous_epoch() {
    let _guard = context_guard();
    let mut app = routing_app();
    let primary = primary_context(&app);
    let window = spawn_window(&mut app, true, UVec2::new(800, 600));
    let camera = spawn_camera(
        &mut app,
        RenderTarget::Window(WindowRef::Entity(window)),
        0,
        true,
    );
    app.world_mut()
        .spawn(ImguiRenderRoute::new(primary, camera));

    app.update();
    let valid_epoch = app.world().resource::<ImguiResolvedRoutes>().epoch();
    assert!(
        app.world()
            .resource::<ImguiResolvedRoutes>()
            .render_route(primary)
            .is_some()
    );
    assert_eq!(
        app.world()
            .resource::<ImguiDiagnostics>()
            .epoch(ImguiDiagnosticOrigin::RenderRouting),
        Some(valid_epoch),
    );
    assert!(diagnostics(&app, ImguiDiagnosticOrigin::RenderRouting).is_empty());

    app.world_mut()
        .entity_mut(camera)
        .get_mut::<Camera>()
        .unwrap()
        .is_active = false;
    app.update();

    let invalid_epoch = app.world().resource::<ImguiResolvedRoutes>().epoch();
    assert!(invalid_epoch > valid_epoch);
    assert!(
        app.world()
            .resource::<ImguiResolvedRoutes>()
            .render_route(primary)
            .is_none()
    );
    let invalid_diagnostics = diagnostics(&app, ImguiDiagnosticOrigin::RenderRouting);
    assert_eq!(invalid_diagnostics.len(), 1);
    assert!(matches!(
        &invalid_diagnostics[0],
        ImguiDiagnosticKind::InactiveCamera
    ));
    assert!(
        app.world()
            .resource::<ImguiDiagnostics>()
            .entries()
            .iter()
            .filter(|entry| entry.origin() == ImguiDiagnosticOrigin::RenderRouting)
            .all(|entry| entry.epoch() == invalid_epoch)
    );

    app.world_mut()
        .entity_mut(camera)
        .get_mut::<Camera>()
        .unwrap()
        .is_active = true;
    app.update();

    let restored_epoch = app.world().resource::<ImguiResolvedRoutes>().epoch();
    assert!(restored_epoch > invalid_epoch);
    assert!(
        app.world()
            .resource::<ImguiResolvedRoutes>()
            .render_route(primary)
            .is_some()
    );
    assert!(diagnostics(&app, ImguiDiagnosticOrigin::RenderRouting).is_empty());
    assert_eq!(
        app.world()
            .resource::<ImguiDiagnostics>()
            .epoch(ImguiDiagnosticOrigin::RenderRouting),
        Some(restored_epoch),
    );
}

#[test]
fn equal_priority_overlapping_exclusive_input_fails_closed() {
    let _guard = context_guard();
    let mut app = routing_app();
    let (primary, secondary, _) = spawn_overlapping_input_routes(
        &mut app,
        ImguiInputPolicy::exclusive(4),
        ImguiInputPolicy::exclusive(4),
    );

    app.update();

    let routes = app.world().resource::<ImguiResolvedRoutes>();
    assert!(routes.input_route(primary).is_none());
    assert!(routes.input_route(secondary).is_none());
    assert_eq!(
        diagnostics(&app, ImguiDiagnosticOrigin::InputRouting)
            .iter()
            .filter(|kind| matches!(
                kind,
                ImguiDiagnosticKind::AmbiguousExclusiveInput { priority: 4 }
            ))
            .count(),
        2,
    );
}

#[test]
fn different_priority_overlapping_exclusive_input_remains_routable() {
    let _guard = context_guard();
    let mut app = routing_app();
    let (primary, secondary, _) = spawn_overlapping_input_routes(
        &mut app,
        ImguiInputPolicy::exclusive(-2),
        ImguiInputPolicy::exclusive(9),
    );

    app.update();

    let routes = app.world().resource::<ImguiResolvedRoutes>();
    assert_eq!(
        routes.input_route(primary).unwrap().policy().priority(),
        Some(-2),
    );
    assert_eq!(
        routes.input_route(secondary).unwrap().policy().priority(),
        Some(9),
    );
    assert!(
        diagnostics(&app, ImguiDiagnosticOrigin::InputRouting)
            .iter()
            .all(|kind| !matches!(kind, ImguiDiagnosticKind::AmbiguousExclusiveInput { .. }))
    );
}

#[test]
fn shared_overlapping_input_routes_are_both_preserved() {
    let _guard = context_guard();
    let mut app = routing_app();
    let (primary, secondary, _) = spawn_overlapping_input_routes(
        &mut app,
        ImguiInputPolicy::Shared,
        ImguiInputPolicy::Shared,
    );

    app.update();

    let routes = app.world().resource::<ImguiResolvedRoutes>();
    assert_eq!(
        routes.input_route(primary).unwrap().policy(),
        ImguiInputPolicy::Shared,
    );
    assert_eq!(
        routes.input_route(secondary).unwrap().policy(),
        ImguiInputPolicy::Shared,
    );
    assert!(diagnostics(&app, ImguiDiagnosticOrigin::InputRouting).is_empty());
}

#[test]
fn disabled_input_declaration_suppresses_derived_window_input() {
    let _guard = context_guard();
    let mut app = routing_app();
    let primary = primary_context(&app);
    let window = spawn_window(&mut app, true, UVec2::new(800, 600));
    let camera = spawn_camera(
        &mut app,
        RenderTarget::Window(WindowRef::Entity(window)),
        0,
        true,
    );
    app.world_mut()
        .spawn(ImguiRenderRoute::new(primary, camera));
    app.world_mut().spawn(
        ImguiInputRoute::from_camera(primary, camera).with_policy(ImguiInputPolicy::Disabled),
    );

    app.update();

    let routes = app.world().resource::<ImguiResolvedRoutes>();
    assert!(routes.render_route(primary).is_some());
    assert!(routes.input_route(primary).is_none());
    assert!(diagnostics(&app, ImguiDiagnosticOrigin::InputRouting).is_empty());
}

#[test]
fn non_writing_camera_is_not_an_automatic_candidate() {
    let _guard = context_guard();
    let mut app = routing_app();
    let primary = primary_context(&app);
    spawn_window(&mut app, true, UVec2::new(800, 600));
    let camera = spawn_camera(&mut app, RenderTarget::Window(WindowRef::Primary), 10, true);
    app.world_mut()
        .entity_mut(camera)
        .get_mut::<Camera>()
        .unwrap()
        .output_mode = CameraOutputMode::Skip;
    app.update();

    assert!(
        app.world()
            .resource::<ImguiResolvedRoutes>()
            .render_route(primary)
            .is_none()
    );
    assert!(
        diagnostics(&app, ImguiDiagnosticOrigin::RenderRouting)
            .iter()
            .any(|kind| matches!(kind, ImguiDiagnosticKind::CameraDoesNotWrite))
    );
}

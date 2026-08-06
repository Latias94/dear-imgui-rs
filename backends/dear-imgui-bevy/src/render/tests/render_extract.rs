#![cfg(feature = "render")]

use crate::test_util::imgui_context_guard;
use bevy::prelude::GlobalTransform;
use bevy_app::{App, Update};
use bevy_asset::Assets;
use bevy_camera::{
    Camera, CameraMainTextureUsages, CameraOutputMode, ClearColorConfig, NormalizedRenderTarget,
    RenderTarget, Viewport,
};
use bevy_core_pipeline::Core2d;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;
use bevy_math::{Mat4, UVec2, UVec4};
use bevy_render::{
    ExtractSchedule, Render, RenderApp,
    camera::{CameraRenderGraph, ExtractedCamera},
    extract_plugin::ExtractPlugin,
    render_resource::{
        BindGroupLayoutEntry, BindingType, BlendState, SamplerBindingType,
        SpecializedRenderPipeline, TextureFormat, TextureUsages,
    },
    view::{ColorGrading, ExtractedView, Msaa, RetainedViewEntity},
};
use bevy_shader::Shader;
use bevy_window::{PrimaryWindow, Window, WindowRef, WindowResolution};
#[cfg(feature = "multi-viewport")]
use dear_imgui_bevy::ImguiPluginConfig;
use dear_imgui_bevy::{
    ImguiAppExt, ImguiContextConfig, ImguiContexts, ImguiFrame, ImguiPlugin,
    render::{
        IMGUI_FRAGMENT_ENTRY_POINT, IMGUI_SHADER_HANDLE, IMGUI_SHADER_SOURCE,
        IMGUI_VERTEX_ENTRY_POINT, ImguiExtractedRenderFrame, ImguiPipelineKey,
        ImguiPreparedRenderFrame, ImguiQueuedPipelines, ImguiRenderPipeline,
        ImguiTextureBindGroups, imgui_vertex_buffer_layout,
    },
    route::{
        ImguiDiagnosticKind, ImguiDiagnosticOrigin, ImguiDiagnostics, ImguiRenderRoute,
        ImguiResolvedRoutes,
    },
};
use dear_imgui_rs::{self as imgui, render::TextureBinding};

struct ManagedTexture(imgui::ManagedTextureId);

const LEGACY_RENDER_TEXTURE_ID: imgui::TextureId = imgui::TextureId::new(0xD1A6);
const SECONDARY_RENDER_TEXTURE_ID: imgui::TextureId = imgui::TextureId::new(0x5EC0);

struct SecondaryContextPass;

fn primary_context_id(app: &App) -> imgui::ContextId {
    app.world()
        .non_send::<ImguiContexts>()
        .primary_id()
        .expect("ImguiPlugin should install a primary Context")
}

fn configure_primary<T>(app: &mut App, configure: impl FnOnce(&mut imgui::Context) -> T) -> T {
    let mut contexts = app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .expect("ImguiPlugin should install the Context registry");
    let primary_id = contexts
        .primary_id()
        .expect("ImguiPlugin should install a primary Context");
    contexts
        .configure(primary_id, configure)
        .unwrap_or_else(|error| panic!("primary Context should be configurable: {error}"))
}

fn add_secondary_context(app: &mut App) -> imgui::ContextId {
    let secondary_pass = app.declare_imgui_pass::<SecondaryContextPass>();
    app.add_imgui_systems(
        &secondary_pass,
        secondary_pass.system(draw_secondary_legacy_texture),
    );
    let secondary_id = app
        .world_mut()
        .non_send_mut::<ImguiContexts>()
        .create(ImguiContextConfig::new(&secondary_pass))
        .expect("the secondary Context should be admitted");
    app.world_mut()
        .non_send_mut::<ImguiContexts>()
        .configure(secondary_id, |context| {
            context.io_mut().set_config_input_trickle_event_queue(false);
            let _ = context.font_atlas().build();
            let _ = context.set_ini_filename::<std::path::PathBuf>(None);
        })
        .expect("the secondary Context should be configurable");
    secondary_id
}

#[derive(Resource, Default)]
#[cfg(feature = "multi-viewport")]
struct SecondaryViewportRouteState {
    viewport_id: Option<imgui::Id>,
}

fn app_with_primary_window() -> (App, Entity, Entity, imgui::ManagedTextureId) {
    let mut app = App::new();
    app.add_plugins(ExtractPlugin::default());
    app.add_plugins(ImguiPlugin::default());
    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());

    let mut window = Window {
        resolution: WindowResolution::new(1280, 720),
        ..Default::default()
    };
    window.resolution.set_scale_factor(2.0);
    let primary_window = app.world_mut().spawn((window, PrimaryWindow)).id();

    let camera = app
        .world_mut()
        .spawn((
            Camera {
                order: 3,
                ..Default::default()
            },
            RenderTarget::Window(WindowRef::Primary),
            CameraRenderGraph::new(Core2d),
        ))
        .id();

    app.world_mut().spawn((
        Camera {
            is_active: false,
            order: 99,
            ..Default::default()
        },
        RenderTarget::Window(WindowRef::Primary),
        CameraRenderGraph::new(Core2d),
    ));

    app.world_mut().spawn((
        Camera {
            order: -10,
            ..Default::default()
        },
        RenderTarget::Window(WindowRef::Primary),
        CameraRenderGraph::new(Core2d),
    ));

    install_render_view(
        &mut app,
        camera,
        NormalizedRenderTarget::Window(
            WindowRef::Entity(primary_window)
                .normalize(None)
                .expect("entity window target should normalize"),
        ),
        3,
        [1280, 720],
        None,
        TextureFormat::Rgba8UnormSrgb,
        CameraMainTextureUsages::default().0,
        Msaa::Off,
    );

    let texture = imgui::texture::OwnedTextureData::from_pixels(
        imgui::texture::TextureFormat::RGBA32,
        1,
        1,
        &[255, 0, 255, 255],
    )
    .unwrap();
    let texture_id = configure_primary(&mut app, |context| {
        context.io_mut().set_config_input_trickle_event_queue(false);
        let _ = context.font_atlas().build();
        let _ = context.set_ini_filename::<std::path::PathBuf>(None);
        context.register_texture(texture)
    });
    app.insert_non_send(ManagedTexture(texture_id));

    (app, primary_window, camera, texture_id)
}

#[allow(clippy::too_many_arguments)]
fn install_render_view(
    app: &mut App,
    camera: Entity,
    target: NormalizedRenderTarget,
    camera_order: isize,
    physical_target_size: [u32; 2],
    viewport: Option<Viewport>,
    target_format: TextureFormat,
    texture_usages: TextureUsages,
    msaa: Msaa,
) -> (Entity, RetainedViewEntity) {
    let physical_target_size = UVec2::from(physical_target_size);
    let physical_viewport_size = viewport
        .as_ref()
        .map_or(physical_target_size, |viewport| viewport.physical_size);
    let physical_viewport_position = viewport
        .as_ref()
        .map_or(UVec2::ZERO, |viewport| viewport.physical_position);
    let hdr = target_format == TextureFormat::Rgba16Float;
    let view = RetainedViewEntity::new(camera.into(), None, 0);

    let render_entity = app
        .sub_app_mut(RenderApp)
        .world_mut()
        .spawn((
            ExtractedView {
                retained_view_entity: view,
                clip_from_view: Mat4::IDENTITY,
                world_from_view: GlobalTransform::IDENTITY,
                clip_from_world: None,
                target_format,
                viewport: UVec4::new(
                    physical_viewport_position.x,
                    physical_viewport_position.y,
                    physical_viewport_size.x,
                    physical_viewport_size.y,
                ),
                color_grading: ColorGrading::default(),
                invert_culling: false,
            },
            ExtractedCamera {
                target: Some(target),
                physical_viewport_size: Some(physical_viewport_size),
                physical_target_size: Some(physical_target_size),
                viewport,
                schedule: Core2d.intern(),
                order: camera_order,
                output_mode: CameraOutputMode::default(),
                msaa_writeback: Default::default(),
                clear_color: ClearColorConfig::Default,
                sorted_camera_index_for_target: 0,
                exposure: 1.0,
                hdr,
                compositing_space: None,
            },
            CameraMainTextureUsages(texture_usages),
            msaa,
        ))
        .id();

    (render_entity, view)
}

fn render_entity_for_camera(app: &mut App, camera: Entity) -> Entity {
    let render_world = app.sub_app_mut(RenderApp).world_mut();
    let mut views = render_world.query::<(Entity, &ExtractedView)>();
    views
        .iter(render_world)
        .find_map(|(entity, view)| {
            (view.retained_view_entity.main_entity.id() == camera).then_some(entity)
        })
        .expect("the camera should have an installed render view")
}

fn draw_managed_texture(frame: ImguiFrame<'_>, texture: NonSend<ManagedTexture>) {
    let ui = frame.ui();
    ui.image(texture.0, [16.0, 16.0]);
}

fn draw_legacy_texture(frame: ImguiFrame<'_>) {
    let ui = frame.ui();
    ui.get_foreground_draw_list().add_image(
        LEGACY_RENDER_TEXTURE_ID,
        [0.0, 0.0],
        [16.0, 16.0],
        [0.0, 0.0],
        [1.0, 1.0],
        [1.0, 1.0, 1.0, 1.0],
    );
}

fn draw_secondary_legacy_texture(frame: ImguiFrame<'_, SecondaryContextPass>) {
    let ui = frame.ui();
    ui.get_foreground_draw_list().add_image(
        SECONDARY_RENDER_TEXTURE_ID,
        [24.0, 24.0],
        [48.0, 48.0],
        [0.0, 0.0],
        [1.0, 1.0],
        [1.0, 1.0, 1.0, 1.0],
    );
}

fn add_primary_legacy_draw_system(app: &mut App) {
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(&primary_pass, primary_pass.system(draw_legacy_texture));
}

#[test]
fn render_extract_moves_context_owned_managed_frame_and_commits_once() {
    let _guard = imgui_context_guard();
    let (mut app, primary_window, camera, texture_id) = app_with_primary_window();
    let context_id = primary_context_id(&app);
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(&primary_pass, primary_pass.system(draw_managed_texture));

    app.update();

    assert_eq!(
        app.world()
            .get_non_send::<ImguiContexts>()
            .expect("the plugin must retain its Context registry")
            .frame_index(context_id)
            .expect("the primary Context must remain registered"),
        1
    );

    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedRenderFrame>();
    assert_eq!(extracted.frame_index(context_id), Some(1));
    assert!(
        extracted.snapshot(context_id).is_none(),
        "the full render schedule must consume the move-only snapshot"
    );
    assert_eq!(extracted.camera_targets(context_id).len(), 1);

    let prepared = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiPreparedRenderFrame>();
    assert_eq!(prepared.frame_index(context_id), Some(1));
    assert!(!prepared.draws().is_empty());
    assert!(prepared.texture_request_count(context_id) >= 1);
    assert!(prepared.draws().iter().any(|draw| {
        draw.texture == TextureBinding::Managed(imgui::render::SnapshotTextureId::User(texture_id))
    }));

    let progress = configure_primary(&mut app, |context| context.poll_snapshot_completions())
        .expect("request-bound retry feedback should complete the snapshot epoch");
    assert_eq!(progress.committed(), 1);
    assert_eq!(progress.feedback_applied(), 0);
    let _ = (primary_window, camera);
}

#[test]
fn render_extract_batches_independent_contexts_without_aliasing_shared_resources() {
    let _guard = imgui_context_guard();
    let (mut app, _primary_window, camera, _texture_id) = app_with_primary_window();
    let primary_id = primary_context_id(&app);
    let secondary_id = add_secondary_context(&mut app);
    app.world_mut()
        .spawn(ImguiRenderRoute::new(secondary_id, camera));
    add_primary_legacy_draw_system(&mut app);

    app.update();

    let contexts = app
        .world()
        .get_non_send::<ImguiContexts>()
        .expect("the plugin must retain its Context registry");
    for context_id in [primary_id, secondary_id] {
        assert_eq!(
            contexts
                .frame_index(context_id)
                .expect("each Context must remain registered"),
            1
        );
    }

    let render_world = app.sub_app(RenderApp).world();
    let extracted = render_world.resource::<ImguiExtractedRenderFrame>();
    let mut extracted_contexts = extracted.context_ids().collect::<Vec<_>>();
    extracted_contexts.sort_by_key(|context_id| context_id.get().get());
    let mut expected_contexts = vec![primary_id, secondary_id];
    expected_contexts.sort_by_key(|context_id| context_id.get().get());
    assert_eq!(extracted_contexts, expected_contexts);
    for context_id in [primary_id, secondary_id] {
        assert_eq!(extracted.frame_index(context_id), Some(1));
        assert!(
            extracted.snapshot(context_id).is_none(),
            "the render schedule must terminally consume each move-only snapshot"
        );
        assert_eq!(extracted.camera_targets(context_id).len(), 1);
    }
    let primary_view = extracted.camera_targets(primary_id)[0].view;
    let secondary_view = extracted.camera_targets(secondary_id)[0].view;
    assert_eq!(
        primary_view, secondary_view,
        "this fixture deliberately routes two Contexts through one Bevy view"
    );

    let prepared = render_world.resource::<ImguiPreparedRenderFrame>();
    assert_eq!(prepared.frame_index(primary_id), Some(1));
    assert_eq!(prepared.frame_index(secondary_id), Some(1));
    assert!(
        prepared
            .uniforms_for_view(primary_id, primary_view)
            .is_some()
    );
    assert!(
        prepared
            .uniforms_for_view(secondary_id, secondary_view)
            .is_some(),
        "uniform ownership must be keyed by both Context and view"
    );
    assert!(prepared.texture_request_count(primary_id) >= 1);
    assert!(prepared.texture_request_count(secondary_id) >= 1);

    let primary_draw = prepared
        .draws()
        .iter()
        .find(|draw| {
            draw.context_id == primary_id
                && draw.texture == TextureBinding::Legacy(LEGACY_RENDER_TEXTURE_ID)
        })
        .expect("the primary Context draw should be part of the shared batch");
    let secondary_draw = prepared
        .draws()
        .iter()
        .find(|draw| {
            draw.context_id == secondary_id
                && draw.texture == TextureBinding::Legacy(SECONDARY_RENDER_TEXTURE_ID)
        })
        .expect("the secondary Context draw should be part of the shared batch");
    assert!(
        primary_draw.index_range.end <= secondary_draw.index_range.start
            || secondary_draw.index_range.end <= primary_draw.index_range.start,
        "Context-local indices must be rebased into non-overlapping shared-buffer ranges"
    );
    assert_ne!(
        primary_draw.vertex_offset, secondary_draw.vertex_offset,
        "Context-local vertices must be rebased into the shared vertex buffer"
    );
    assert!(
        primary_draw.index_range.end as usize <= prepared.indices().len()
            && secondary_draw.index_range.end as usize <= prepared.indices().len()
    );

    let (primary_progress, secondary_progress) = {
        let mut contexts = app.world_mut().non_send_mut::<ImguiContexts>();
        let primary_progress = contexts
            .configure(primary_id, |context| context.poll_snapshot_completions())
            .expect("the primary Context should remain configurable")
            .expect("the primary snapshot completion should be valid");
        let secondary_progress = contexts
            .configure(secondary_id, |context| context.poll_snapshot_completions())
            .expect("the secondary Context should remain configurable")
            .expect("the secondary snapshot completion should be valid");
        (primary_progress, secondary_progress)
    };
    assert_eq!(primary_progress.committed(), 1);
    assert_eq!(secondary_progress.committed(), 1);
}

#[test]
fn render_extract_clears_stale_snapshot_after_primary_window_is_removed() {
    let _guard = imgui_context_guard();
    let (mut app, primary_window, _camera, _texture_id) = app_with_primary_window();
    let context_id = primary_context_id(&app);
    add_primary_legacy_draw_system(&mut app);

    app.update();
    assert!(
        app.sub_app(RenderApp)
            .world()
            .resource::<ImguiPreparedRenderFrame>()
            .draws()
            .iter()
            .any(|draw| draw.texture == TextureBinding::Legacy(LEGACY_RENDER_TEXTURE_ID)),
        "first update should prepare the extracted snapshot"
    );

    app.world_mut().despawn(primary_window);
    app.update();

    assert_eq!(
        app.world()
            .get_non_send::<ImguiContexts>()
            .expect("the plugin must retain its Context registry")
            .frame_index(context_id)
            .expect("the primary Context must remain registered"),
        1
    );

    let render_world = app.sub_app(RenderApp).world();
    let extracted = render_world.resource::<ImguiExtractedRenderFrame>();
    assert_eq!(extracted.frame_index(context_id), None);
    assert!(
        extracted.snapshot(context_id).is_none(),
        "render extraction must not keep drawing the last frame after the primary window disappears"
    );

    let prepared = render_world.resource::<ImguiPreparedRenderFrame>();
    assert_eq!(prepared.frame_index(context_id), None);
    assert!(prepared.draws().is_empty());
    assert!(prepared.vertices().is_empty());
    assert!(prepared.indices().is_empty());
}

#[test]
fn render_extract_materializes_the_unique_auto_primary_camera() {
    let _guard = imgui_context_guard();
    let (mut app, primary_window, _camera, _texture_id) = app_with_primary_window();
    let context_id = primary_context_id(&app);
    let primary_target =
        NormalizedRenderTarget::Window(WindowRef::Entity(primary_window).normalize(None).unwrap());
    let auto_primary_camera = app
        .world_mut()
        .spawn((
            Camera {
                order: 12,
                ..Default::default()
            },
            RenderTarget::Window(WindowRef::Primary),
            CameraRenderGraph::new(Core2d),
        ))
        .id();
    install_render_view(
        &mut app,
        auto_primary_camera,
        primary_target.clone(),
        12,
        [1280, 720],
        None,
        TextureFormat::Rgba8UnormSrgb,
        CameraMainTextureUsages::default().0,
        Msaa::Off,
    );
    add_primary_legacy_draw_system(&mut app);

    app.update();

    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedRenderFrame>();
    let targets = extracted.camera_targets(context_id);
    assert_eq!(
        targets.len(),
        1,
        "AutoPrimary must resolve to one unique highest-order primary-window camera"
    );
    assert_eq!(targets[0].camera, auto_primary_camera);
    assert_eq!(targets[0].order, 0);
    assert_eq!(targets[0].camera_order, 12);
    assert_eq!(targets[0].target, primary_target);
}

#[test]
fn render_route_epoch_rejects_same_frame_camera_changes_until_the_next_snapshot() {
    let _guard = imgui_context_guard();
    let (mut app, _primary_window, camera, _texture_id) = app_with_primary_window();
    let context_id = primary_context_id(&app);
    let render_entity = render_entity_for_camera(&mut app, camera);
    add_primary_legacy_draw_system(&mut app);

    // Resolve the initial camera topology. The next frame must keep using this epoch even when
    // Update and extraction publish a newer camera configuration later in that same frame.
    app.update();

    app.add_systems(Update, move |mut cameras: Query<&mut Camera>| {
        cameras
            .get_mut(camera)
            .expect("the primary camera should remain live")
            .order = 13;
    });
    app.sub_app_mut(RenderApp).add_systems(
        ExtractSchedule,
        move |mut cameras: Query<&mut ExtractedCamera>| {
            cameras
                .get_mut(render_entity)
                .expect("the primary render view should remain live")
                .order = 13;
        },
    );

    app.update();

    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedRenderFrame>();
    assert!(
        extracted.camera_targets(context_id).is_empty(),
        "a snapshot must fail closed instead of borrowing a camera configuration from a newer route epoch"
    );
    assert!(
        app.world()
            .resource::<ImguiDiagnostics>()
            .entries_for(ImguiDiagnosticOrigin::RenderExtraction)
            .any(|diagnostic| diagnostic.kind() == &ImguiDiagnosticKind::StaleExtractedView)
    );

    app.update();

    let routes = app.world().resource::<ImguiResolvedRoutes>();
    assert_eq!(
        routes
            .render_route(context_id)
            .expect("the updated camera should remain routable")
            .camera_order(),
        13
    );
    let targets = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedRenderFrame>()
        .camera_targets(context_id);
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].camera, camera);
    assert_eq!(targets[0].camera_order, 13);
    assert!(
        app.world()
            .resource::<ImguiDiagnostics>()
            .entries_for(ImguiDiagnosticOrigin::RenderExtraction)
            .all(|diagnostic| diagnostic.kind() != &ImguiDiagnosticKind::StaleExtractedView)
    );
}

#[test]
fn render_route_epoch_keeps_same_frame_context_route_swaps_with_their_snapshots() {
    let _guard = imgui_context_guard();
    let (mut app, primary_window, primary_camera, _texture_id) = app_with_primary_window();
    let primary_context = primary_context_id(&app);
    let secondary_context = add_secondary_context(&mut app);
    let target =
        NormalizedRenderTarget::Window(WindowRef::Entity(primary_window).normalize(None).unwrap());
    let secondary_camera = app
        .world_mut()
        .spawn((
            Camera {
                order: 4,
                ..Default::default()
            },
            RenderTarget::Window(WindowRef::Entity(primary_window)),
            CameraRenderGraph::new(Core2d),
        ))
        .id();
    install_render_view(
        &mut app,
        secondary_camera,
        target,
        4,
        [1280, 720],
        None,
        TextureFormat::Rgba8UnormSrgb,
        CameraMainTextureUsages::default().0,
        Msaa::Off,
    );
    let primary_route = app
        .world_mut()
        .spawn(ImguiRenderRoute::new(primary_context, primary_camera))
        .id();
    let secondary_route = app
        .world_mut()
        .spawn(ImguiRenderRoute::new(secondary_context, secondary_camera))
        .id();
    add_primary_legacy_draw_system(&mut app);

    app.update();

    app.add_systems(
        Update,
        move |mut swapped: Local<bool>, mut routes: Query<&mut ImguiRenderRoute>| {
            if *swapped {
                return;
            }
            *swapped = true;
            *routes
                .get_mut(primary_route)
                .expect("the primary route must remain live") =
                ImguiRenderRoute::new(secondary_context, primary_camera);
            *routes
                .get_mut(secondary_route)
                .expect("the secondary route must remain live") =
                ImguiRenderRoute::new(primary_context, secondary_camera);
        },
    );

    app.update();

    {
        let extracted = app
            .sub_app(RenderApp)
            .world()
            .resource::<ImguiExtractedRenderFrame>();
        assert_eq!(
            extracted.camera_targets(primary_context)[0].camera,
            primary_camera
        );
        assert_eq!(
            extracted.camera_targets(secondary_context)[0].camera,
            secondary_camera
        );
        assert!(
            extracted.route_epoch() < app.world().resource::<ImguiResolvedRoutes>().epoch(),
            "the extracted snapshots must retain the route epoch captured before Update"
        );
    }

    app.update();

    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedRenderFrame>();
    assert_eq!(
        extracted.camera_targets(primary_context)[0].camera,
        secondary_camera
    );
    assert_eq!(
        extracted.camera_targets(secondary_context)[0].camera,
        primary_camera
    );
}

#[test]
fn render_extract_prefers_an_explicit_route_over_auto_primary() {
    let _guard = imgui_context_guard();
    let (mut app, primary_window, _camera, _texture_id) = app_with_primary_window();
    let context_id = app
        .world()
        .non_send::<ImguiContexts>()
        .primary_id()
        .expect("the primary Context should exist");
    let primary_target =
        NormalizedRenderTarget::Window(WindowRef::Entity(primary_window).normalize(None).unwrap());
    let explicit_camera = app
        .world_mut()
        .spawn((
            Camera {
                order: -100,
                ..Default::default()
            },
            RenderTarget::Window(WindowRef::Primary),
            CameraRenderGraph::new(Core2d),
        ))
        .id();
    app.world_mut().spawn((
        Camera {
            order: 500,
            ..Default::default()
        },
        RenderTarget::Window(WindowRef::Primary),
        CameraRenderGraph::new(Core2d),
    ));
    app.world_mut()
        .spawn(ImguiRenderRoute::new(context_id, explicit_camera));
    install_render_view(
        &mut app,
        explicit_camera,
        primary_target.clone(),
        -100,
        [1280, 720],
        None,
        TextureFormat::Rgba8UnormSrgb,
        CameraMainTextureUsages::default().0,
        Msaa::Off,
    );
    add_primary_legacy_draw_system(&mut app);

    app.update();

    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedRenderFrame>();
    let targets = extracted.camera_targets(context_id);
    assert_eq!(
        targets.len(),
        1,
        "an explicit ordinary route must suppress AutoPrimary for that Context"
    );
    assert_eq!(targets[0].camera, explicit_camera);
    assert_eq!(targets[0].context_id, context_id);
    assert_eq!(targets[0].order, 0);
    assert_eq!(targets[0].camera_order, -100);
    assert_eq!(targets[0].target, primary_target);
    assert_eq!(targets[0].route_epoch, extracted.route_epoch());
    assert!(
        targets[0].route_epoch < app.world().resource::<ImguiResolvedRoutes>().epoch(),
        "PostUpdate may publish the next route epoch only after the frame captured its topology"
    );
}

#[test]
fn render_extract_preserves_explicit_route_viewport_for_render_pass() {
    let _guard = imgui_context_guard();
    let (mut app, primary_window, _camera, _texture_id) = app_with_primary_window();
    let context_id = app
        .world()
        .non_send::<ImguiContexts>()
        .primary_id()
        .expect("the primary Context should exist");
    let primary_target =
        NormalizedRenderTarget::Window(WindowRef::Entity(primary_window).normalize(None).unwrap());
    let viewport = Viewport {
        physical_position: UVec2::new(320, 0),
        physical_size: UVec2::new(640, 360),
        ..Default::default()
    };
    let routed_camera = app
        .world_mut()
        .spawn((
            Camera {
                order: 50,
                viewport: Some(viewport.clone()),
                ..Default::default()
            },
            RenderTarget::Window(WindowRef::Primary),
            CameraRenderGraph::new(Core2d),
        ))
        .id();
    app.world_mut()
        .spawn(ImguiRenderRoute::new(context_id, routed_camera));
    install_render_view(
        &mut app,
        routed_camera,
        primary_target,
        50,
        [1280, 720],
        Some(viewport),
        TextureFormat::Rgba8UnormSrgb,
        CameraMainTextureUsages::default().0,
        Msaa::Off,
    );
    add_primary_legacy_draw_system(&mut app);

    app.update();

    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedRenderFrame>();
    let target = extracted
        .camera_targets(context_id)
        .iter()
        .find(|target| target.camera == routed_camera)
        .expect("the explicitly routed camera should be extracted");
    assert_eq!(
        target
            .camera_viewport
            .map(|viewport| { (viewport.physical_position, viewport.physical_size) }),
        Some(([320, 0], [640, 360]))
    );

    let prepared = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiPreparedRenderFrame>();
    let draw = prepared
        .draws()
        .iter()
        .find(|draw| draw.camera == routed_camera)
        .expect("the explicitly routed camera should receive prepared draw commands");
    assert_eq!(
        draw.camera_viewport
            .map(|viewport| { (viewport.physical_position, viewport.physical_size) }),
        Some(([320, 0], [640, 360]))
    );
    assert_eq!(
        draw.framebuffer_size,
        [640, 360],
        "Context metrics must match the routed camera viewport instead of the full target"
    );
}

#[test]
fn render_extract_does_not_broadcast_one_context_to_unrouted_windows() {
    let _guard = imgui_context_guard();
    let (mut app, primary_window, primary_camera, _texture_id) = app_with_primary_window();
    let context_id = primary_context_id(&app);
    let secondary_window = app.world_mut().spawn(Window::default()).id();
    let secondary_camera = app
        .world_mut()
        .spawn((
            Camera {
                order: 7,
                ..Default::default()
            },
            RenderTarget::Window(WindowRef::Entity(secondary_window)),
            CameraRenderGraph::new(Core2d),
        ))
        .id();
    add_primary_legacy_draw_system(&mut app);

    app.update();

    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedRenderFrame>();
    let targets = extracted.camera_targets(context_id);
    let expected_primary_target =
        NormalizedRenderTarget::Window(WindowRef::Entity(primary_window).normalize(None).unwrap());
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].camera, primary_camera);
    assert_eq!(targets[0].target, expected_primary_target);

    let prepared = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiPreparedRenderFrame>();
    let primary_draws = prepared
        .draws()
        .iter()
        .filter(|draw| draw.camera == primary_camera)
        .collect::<Vec<_>>();
    let secondary_draws = prepared
        .draws()
        .iter()
        .filter(|draw| draw.camera == secondary_camera)
        .collect::<Vec<_>>();
    assert!(!primary_draws.is_empty());
    assert!(
        secondary_draws.is_empty(),
        "ordinary Context draw data must not be broadcast to an unrelated window"
    );
    assert!(
        primary_draws
            .iter()
            .all(|draw| draw.target == expected_primary_target)
    );
}

#[test]
fn render_extract_keeps_same_target_context_views_with_different_render_identity_distinct() {
    let _guard = imgui_context_guard();
    let (mut app, primary_window, primary_camera, _texture_id) = app_with_primary_window();
    let primary_id = primary_context_id(&app);
    let secondary_id = add_secondary_context(&mut app);
    let primary_target =
        NormalizedRenderTarget::Window(WindowRef::Entity(primary_window).normalize(None).unwrap());
    app.world_mut()
        .spawn(ImguiRenderRoute::new(primary_id, primary_camera));
    let secondary_camera = app
        .world_mut()
        .spawn((
            Camera {
                order: 11,
                ..Default::default()
            },
            RenderTarget::Window(WindowRef::Primary),
            CameraRenderGraph::new(Core2d),
        ))
        .id();
    app.world_mut()
        .spawn(ImguiRenderRoute::new(secondary_id, secondary_camera));
    let secondary_usages = TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC;
    install_render_view(
        &mut app,
        secondary_camera,
        primary_target.clone(),
        11,
        [1280, 720],
        None,
        TextureFormat::Rgba16Float,
        secondary_usages,
        Msaa::Sample4,
    );
    add_primary_legacy_draw_system(&mut app);

    app.update();

    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedRenderFrame>();
    let primary_targets = extracted.camera_targets(primary_id);
    let secondary_targets = extracted.camera_targets(secondary_id);
    assert_eq!(primary_targets.len(), 1);
    assert_eq!(secondary_targets.len(), 1);
    let primary = &primary_targets[0];
    let secondary = &secondary_targets[0];

    assert_eq!(primary.camera, primary_camera);
    assert_eq!(secondary.camera, secondary_camera);
    assert_eq!(primary.target, primary_target);
    assert_eq!(secondary.target, primary_target);
    assert_ne!(primary.view, secondary.view);
    assert_eq!(primary.target_format, TextureFormat::Rgba8UnormSrgb);
    assert_eq!(secondary.target_format, TextureFormat::Rgba16Float);
    assert_eq!(primary.texture_usages, CameraMainTextureUsages::default().0);
    assert_eq!(secondary.texture_usages, secondary_usages);
    assert_eq!(primary.msaa, Msaa::Off);
    assert_eq!(secondary.msaa, Msaa::Sample4);
}

#[test]
fn render_extract_reports_a_stale_render_view_without_retargeting() {
    let _guard = imgui_context_guard();
    let (mut app, primary_window, _primary_camera, _texture_id) = app_with_primary_window();
    let context_id = app
        .world()
        .non_send::<ImguiContexts>()
        .primary_id()
        .expect("the primary Context should exist");
    let primary_target =
        NormalizedRenderTarget::Window(WindowRef::Entity(primary_window).normalize(None).unwrap());
    let routed_camera = app
        .world_mut()
        .spawn((
            Camera {
                order: 7,
                ..Default::default()
            },
            RenderTarget::Window(WindowRef::Primary),
            CameraRenderGraph::new(Core2d),
        ))
        .id();
    app.world_mut()
        .spawn(ImguiRenderRoute::new(context_id, routed_camera));
    install_render_view(
        &mut app,
        routed_camera,
        primary_target,
        8,
        [1280, 720],
        None,
        TextureFormat::Rgba8UnormSrgb,
        CameraMainTextureUsages::default().0,
        Msaa::Off,
    );
    add_primary_legacy_draw_system(&mut app);

    app.update();

    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedRenderFrame>();
    assert!(
        extracted.camera_targets(context_id).is_empty(),
        "a stale view must fail closed rather than draw through a changed camera"
    );
    let prepared = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiPreparedRenderFrame>();
    assert!(prepared.draws().is_empty());
    assert!(
        app.world()
            .resource::<ImguiDiagnostics>()
            .entries_for(ImguiDiagnosticOrigin::RenderExtraction)
            .any(|diagnostic| {
                diagnostic.kind() == &ImguiDiagnosticKind::StaleExtractedView
                    && diagnostic.context_id() == Some(context_id)
                    && diagnostic.camera() == Some(routed_camera)
            }),
        "render-world rejection must be observable through the main-world diagnostics resource"
    );
}

#[test]
fn render_extract_rejects_output_mode_and_schedule_drift_after_route_resolution() {
    let _guard = imgui_context_guard();
    let (mut app, primary_window, _primary_camera, _texture_id) = app_with_primary_window();
    let context_id = app
        .world()
        .non_send::<ImguiContexts>()
        .primary_id()
        .expect("the primary Context should exist");
    let primary_target =
        NormalizedRenderTarget::Window(WindowRef::Entity(primary_window).normalize(None).unwrap());
    let routed_camera = app
        .world_mut()
        .spawn((
            Camera {
                order: 23,
                ..Default::default()
            },
            RenderTarget::Window(WindowRef::Primary),
            CameraRenderGraph::new(Core2d),
        ))
        .id();
    app.world_mut()
        .spawn(ImguiRenderRoute::new(context_id, routed_camera));
    let (render_entity, _) = install_render_view(
        &mut app,
        routed_camera,
        primary_target,
        23,
        [1280, 720],
        None,
        TextureFormat::Rgba8UnormSrgb,
        CameraMainTextureUsages::default().0,
        Msaa::Off,
    );
    add_primary_legacy_draw_system(&mut app);

    app.sub_app_mut(RenderApp)
        .world_mut()
        .entity_mut(render_entity)
        .get_mut::<ExtractedCamera>()
        .expect("the installed render view should have camera metadata")
        .output_mode = CameraOutputMode::Skip;
    app.update();
    assert!(
        app.world()
            .resource::<ImguiDiagnostics>()
            .entries_for(ImguiDiagnosticOrigin::RenderExtraction)
            .any(|diagnostic| {
                diagnostic.kind() == &ImguiDiagnosticKind::CameraDoesNotWrite
                    && diagnostic.camera() == Some(routed_camera)
            })
    );

    {
        let render_world = app.sub_app_mut(RenderApp).world_mut();
        let mut render_entity = render_world.entity_mut(render_entity);
        let mut render_camera = render_entity
            .get_mut::<ExtractedCamera>()
            .expect("the installed render view should have camera metadata");
        render_camera.output_mode = CameraOutputMode::default();
        render_camera.schedule = Update.intern();
    }
    app.update();
    assert!(
        app.world()
            .resource::<ImguiDiagnostics>()
            .entries_for(ImguiDiagnosticOrigin::RenderExtraction)
            .any(|diagnostic| {
                diagnostic.kind() == &ImguiDiagnosticKind::UnsupportedCameraSchedule
                    && diagnostic.camera() == Some(routed_camera)
            })
    );
}

#[test]
fn render_extract_reports_a_missing_render_view_without_replaying_an_old_target() {
    let _guard = imgui_context_guard();
    let (mut app, _primary_window, _primary_camera, _texture_id) = app_with_primary_window();
    let context_id = app
        .world()
        .non_send::<ImguiContexts>()
        .primary_id()
        .expect("the primary Context should exist");
    let missing_camera = app
        .world_mut()
        .spawn((
            Camera {
                order: 17,
                ..Default::default()
            },
            RenderTarget::Window(WindowRef::Primary),
            CameraRenderGraph::new(Core2d),
        ))
        .id();
    app.world_mut()
        .spawn(ImguiRenderRoute::new(context_id, missing_camera));
    add_primary_legacy_draw_system(&mut app);

    app.update();

    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedRenderFrame>();
    assert!(extracted.camera_targets(context_id).is_empty());
    assert!(
        app.world()
            .resource::<ImguiDiagnostics>()
            .entries_for(ImguiDiagnosticOrigin::RenderExtraction)
            .any(|diagnostic| {
                diagnostic.kind() == &ImguiDiagnosticKind::MissingExtractedView
                    && diagnostic.context_id() == Some(context_id)
                    && diagnostic.camera() == Some(missing_camera)
            })
    );
}

#[test]
#[cfg(feature = "multi-viewport")]
fn renderer_prepare_routes_secondary_viewport_and_rejects_relocated_camera_marker() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    app.insert_resource(
        crate::viewport::native_window::DesktopPositionSupportOverride(
            crate::viewport::native_window::DesktopPositionSupport::Available,
        ),
    );
    app.add_plugins(ExtractPlugin::default());
    app.add_plugins(ImguiPlugin::new(
        ImguiPluginConfig::default().with_multi_viewport(true),
    ));
    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    let context_id = primary_context_id(&app);

    let mut window = Window {
        resolution: WindowResolution::new(1280, 720),
        ..Default::default()
    };
    window.resolution.set_scale_factor(2.0);
    let primary_window = app.world_mut().spawn((window, PrimaryWindow)).id();

    let primary_camera = app
        .world_mut()
        .spawn((
            Camera {
                order: 3,
                ..Default::default()
            },
            RenderTarget::Window(WindowRef::Primary),
            CameraRenderGraph::new(Core2d),
        ))
        .id();
    let primary_target =
        NormalizedRenderTarget::Window(WindowRef::Entity(primary_window).normalize(None).unwrap());
    let (_, primary_view) = install_render_view(
        &mut app,
        primary_camera,
        primary_target.clone(),
        3,
        [1280, 720],
        None,
        TextureFormat::Rgba8UnormSrgb,
        CameraMainTextureUsages::default().0,
        Msaa::Off,
    );

    configure_primary(&mut app, |context| {
        context.io_mut().set_config_input_trickle_event_queue(false);
        let _ = context.font_atlas().build();
        let _ = context.set_ini_filename::<std::path::PathBuf>(None);
        context.io_mut().set_config_viewports_no_auto_merge(true);
    });

    app.init_resource::<SecondaryViewportRouteState>();
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(
        &primary_pass,
        primary_pass.system(
            move |frame: ImguiFrame<'_>, mut route: ResMut<SecondaryViewportRouteState>| {
                let ui = frame.ui();
                let main_viewport_id = ui.main_viewport().id();
                ui.set_next_window_viewport(main_viewport_id);
                ui.window("Primary route proof")
                    .position([8.0, 8.0], imgui::Condition::Always)
                    .size([160.0, 80.0], imgui::Condition::Always)
                    .flags(imgui::WindowFlags::NO_DOCKING)
                    .build(|| {
                        ui.text("primary viewport");
                    });
                ui.window("Secondary route proof")
                    .position([32.0, 32.0], imgui::Condition::Always)
                    .size([160.0, 80.0], imgui::Condition::Always)
                    .flags(imgui::WindowFlags::NO_DOCKING)
                    .build(|| {
                        ui.text("secondary viewport");
                        route.viewport_id = Some(ui.window_viewport().id());
                    });
            },
        ),
    );

    app.update();
    let secondary_viewport_id = app
        .world()
        .resource::<SecondaryViewportRouteState>()
        .viewport_id
        .expect("first frame should create a Dear ImGui viewport for the secondary window");
    let (secondary_window, secondary_camera, secondary_camera_order, secondary_target_size) = {
        let world = app.world_mut();
        let (secondary_window, secondary_target_size) = {
            let mut windows =
                world.query::<(Entity, &Window, &dear_imgui_bevy::ImguiViewportWindow)>();
            let matching = windows
                .iter(world)
                .filter_map(|(entity, window, viewport)| {
                    (viewport.viewport_id() == secondary_viewport_id)
                        .then_some((entity, window.physical_size()))
                })
                .collect::<Vec<_>>();
            assert_eq!(
                matching.len(),
                1,
                "one ImGui viewport must own exactly one Bevy window"
            );
            matching[0]
        };
        let (secondary_camera, secondary_camera_order) = {
            let mut cameras =
                world.query::<(Entity, &Camera, &dear_imgui_bevy::ImguiViewportCamera)>();
            let matching = cameras
                .iter(world)
                .filter_map(|(entity, camera, viewport)| {
                    (viewport.viewport_id() == secondary_viewport_id)
                        .then_some((entity, camera.order))
                })
                .collect::<Vec<_>>();
            assert_eq!(
                matching.len(),
                1,
                "one ImGui viewport must own exactly one Bevy camera"
            );
            matching[0]
        };
        (
            secondary_window,
            secondary_camera,
            secondary_camera_order,
            [secondary_target_size.x, secondary_target_size.y],
        )
    };
    let secondary_target = NormalizedRenderTarget::Window(
        WindowRef::Entity(secondary_window).normalize(None).unwrap(),
    );
    let (_, secondary_view) = install_render_view(
        &mut app,
        secondary_camera,
        secondary_target.clone(),
        secondary_camera_order,
        secondary_target_size,
        None,
        TextureFormat::Rgba8UnormSrgb,
        CameraMainTextureUsages::default().0,
        Msaa::Off,
    );
    app.update();

    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedRenderFrame>();
    assert!(
        extracted.snapshot(context_id).is_none(),
        "secondary viewport snapshots must also be committed after preparation"
    );

    let prepared = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiPreparedRenderFrame>();
    let expected_primary_target = primary_target;
    let expected_secondary_target = secondary_target;
    let primary_draws = prepared
        .draws()
        .iter()
        .filter(|draw| draw.camera == primary_camera)
        .collect::<Vec<_>>();
    let secondary_draws = prepared
        .draws()
        .iter()
        .filter(|draw| draw.camera == secondary_camera)
        .collect::<Vec<_>>();

    assert!(!primary_draws.is_empty());
    assert!(
        !secondary_draws.is_empty(),
        "secondary target was not prepared; targets={:?}; diagnostics={:?}",
        extracted.camera_targets(context_id),
        app.world()
            .resource::<ImguiDiagnostics>()
            .entries_for(ImguiDiagnosticOrigin::RenderExtraction)
            .collect::<Vec<_>>(),
    );
    assert!(primary_draws.iter().all(|draw| {
        draw.target == expected_primary_target && draw.viewport_id != Some(secondary_viewport_id)
    }));
    assert!(secondary_draws.iter().all(|draw| {
        draw.target == expected_secondary_target && draw.viewport_id == Some(secondary_viewport_id)
    }));
    assert_ne!(
        prepared.uniforms_for_view(context_id, primary_view),
        prepared.uniforms_for_view(context_id, secondary_view),
        "secondary viewport rendering needs a viewport-specific projection"
    );

    let first_poll = configure_primary(&mut app, |context| context.poll_snapshot_completions())
        .expect("the snapshot fanned out across main and platform views should complete");
    let second_poll = configure_primary(&mut app, |context| context.poll_snapshot_completions())
        .expect("re-polling the Context should remain valid");
    assert_eq!(first_poll.committed(), 1);
    assert_eq!(
        second_poll.committed(),
        0,
        "render fanout must produce exactly one terminal completion for its source snapshot"
    );

    let public_marker = app
        .world_mut()
        .entity_mut(secondary_camera)
        .take::<dear_imgui_bevy::ImguiViewportCamera>()
        .expect("the callback-created camera should expose a read-only identity marker");
    let spoof_camera = app
        .world_mut()
        .spawn((
            Camera {
                order: secondary_camera_order + 1,
                ..Default::default()
            },
            RenderTarget::Window(WindowRef::Entity(secondary_window)),
            CameraRenderGraph::new(Core2d),
            public_marker,
        ))
        .id();
    let _ = install_render_view(
        &mut app,
        spoof_camera,
        expected_secondary_target.clone(),
        secondary_camera_order + 1,
        secondary_target_size,
        None,
        TextureFormat::Rgba8UnormSrgb,
        CameraMainTextureUsages::default().0,
        Msaa::Off,
    );

    app.update();

    let prepared = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiPreparedRenderFrame>();
    assert!(
        prepared
            .draws()
            .iter()
            .all(|draw| draw.camera != spoof_camera),
        "moving the public marker must not transfer the backend's private render capability"
    );
    assert!(
        prepared
            .draws()
            .iter()
            .any(|draw| draw.camera == secondary_camera),
        "the backend should restore and continue routing its genuinely owned camera"
    );
}

#[test]
fn renderer_prepare_flattens_extracted_snapshot_for_pipeline_consumption() {
    let _guard = imgui_context_guard();
    let (mut app, primary_window, _camera, _texture_id) = app_with_primary_window();
    let context_id = primary_context_id(&app);
    add_primary_legacy_draw_system(&mut app);

    app.update();

    let prepared = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiPreparedRenderFrame>();
    assert_eq!(prepared.frame_index(context_id), Some(1));
    assert!(!prepared.vertices().is_empty());
    assert!(!prepared.indices().is_empty());
    assert!(
        prepared.texture_request_count(context_id) >= 1,
        "Context-owned capture should carry the font-atlas request alongside legacy draws"
    );

    let draw = prepared
        .draws()
        .iter()
        .find(|draw| draw.texture == TextureBinding::Legacy(LEGACY_RENDER_TEXTURE_ID))
        .expect("the legacy texture draw should be prepared");
    assert_eq!(
        draw.target,
        NormalizedRenderTarget::Window(WindowRef::Entity(primary_window).normalize(None).unwrap())
    );
    assert!(draw.index_range.end > draw.index_range.start);
    assert!(draw.scissor.width > 0);
    assert!(draw.scissor.height > 0);

    let layout = imgui_vertex_buffer_layout();
    assert_eq!(
        layout.array_stride,
        std::mem::size_of::<dear_imgui_bevy::render::ImguiGpuVertex>() as u64
    );
    assert_eq!(layout.attributes.len(), 3);
    assert!(IMGUI_SHADER_SOURCE.contains(IMGUI_VERTEX_ENTRY_POINT));
    assert!(IMGUI_SHADER_SOURCE.contains(IMGUI_FRAGMENT_ENTRY_POINT));
}

#[test]
fn renderer_pipeline_resources_and_descriptors_are_installed() {
    let _guard = imgui_context_guard();
    let (app, _, _, _) = app_with_primary_window();

    let shaders = app.world().resource::<Assets<Shader>>();
    assert!(
        shaders.get(IMGUI_SHADER_HANDLE.id()).is_some(),
        "ImguiPlugin should register the embedded ImGui shader asset"
    );

    let render_world = app.sub_app(RenderApp).world();
    assert!(render_world.contains_resource::<ImguiRenderPipeline>());
    assert!(render_world.contains_resource::<ImguiTextureBindGroups>());
    assert!(render_world.contains_resource::<ImguiQueuedPipelines>());

    let pipeline = render_world.resource::<ImguiRenderPipeline>();
    let descriptor = pipeline.specialize(ImguiPipelineKey {
        target_format: TextureFormat::Rgba8UnormSrgb,
    });
    let hdr_descriptor = pipeline.specialize(ImguiPipelineKey {
        target_format: TextureFormat::Rgba16Float,
    });

    assert_eq!(descriptor.layout.len(), 2);
    assert_eq!(pipeline.common_layout().entries.len(), 1);
    assert_eq!(pipeline.texture_layout().entries.len(), 2);
    assert!(
        pipeline
            .texture_layout()
            .entries
            .iter()
            .any(is_filtering_sampler_binding),
        "texture bind group layout must carry a sampler so registered Bevy images keep their GpuImage sampler"
    );
    assert_eq!(descriptor.vertex.shader, IMGUI_SHADER_HANDLE);
    assert_eq!(
        descriptor.vertex.entry_point.as_deref(),
        Some(IMGUI_VERTEX_ENTRY_POINT)
    );
    assert_eq!(descriptor.vertex.buffers.len(), 1);
    assert_eq!(descriptor.multisample.count, 1);
    assert_eq!(
        hdr_descriptor.fragment.as_ref().unwrap().targets[0]
            .as_ref()
            .unwrap()
            .format,
        TextureFormat::Rgba16Float,
        "target format must remain part of pipeline specialization"
    );

    let fragment = descriptor
        .fragment
        .expect("Imgui pipeline should have a fragment stage");
    assert_eq!(fragment.shader, IMGUI_SHADER_HANDLE);
    assert_eq!(
        fragment.entry_point.as_deref(),
        Some(IMGUI_FRAGMENT_ENTRY_POINT)
    );
    let target = fragment.targets[0]
        .as_ref()
        .expect("Imgui pipeline should write one color target");
    assert_eq!(target.format, TextureFormat::Rgba8UnormSrgb);
    assert_eq!(target.blend, Some(BlendState::ALPHA_BLENDING));
}

fn is_filtering_sampler_binding(entry: &BindGroupLayoutEntry) -> bool {
    matches!(
        entry.ty,
        BindingType::Sampler(SamplerBindingType::Filtering)
    )
}

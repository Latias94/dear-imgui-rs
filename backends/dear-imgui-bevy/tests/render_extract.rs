#![cfg(feature = "render")]

use bevy_app::App;
use bevy_asset::Assets;
use bevy_camera::{Camera, NormalizedRenderTarget, RenderTarget};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;
use bevy_render::{
    Render, RenderApp,
    extract_plugin::ExtractPlugin,
    render_resource::{BlendState, SpecializedRenderPipeline, TextureFormat},
};
use bevy_shader::Shader;
use bevy_window::{PrimaryWindow, Window, WindowRef, WindowResolution};
#[cfg(feature = "multi-viewport")]
use dear_imgui_bevy::ImguiBackendConfig;
use dear_imgui_bevy::{
    ImguiContext, ImguiContexts, ImguiPlugin, ImguiPrimaryContextPass, ImguiViewportWindow,
    render::{
        IMGUI_FRAGMENT_ENTRY_POINT, IMGUI_SHADER_HANDLE, IMGUI_SHADER_SOURCE,
        IMGUI_VERTEX_ENTRY_POINT, ImguiExtractedRenderFrame, ImguiOverlayDisabled,
        ImguiPipelineKey, ImguiPreparedRenderFrame, ImguiQueuedPipelines, ImguiRenderPipeline,
        ImguiTextureBindGroups, imgui_vertex_buffer_layout,
    },
};
use dear_imgui_rs::{self as imgui, render::TextureBinding};
use std::sync::{Mutex, OnceLock};

fn imgui_context_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

struct ManagedTexture(imgui::texture::OwnedTextureData);

#[derive(Resource, Default)]
#[cfg(feature = "multi-viewport")]
struct SecondaryViewportRouteState {
    viewport_id: Option<imgui::Id>,
    window: Option<Entity>,
    camera: Option<Entity>,
}

fn app_with_primary_window() -> (
    App,
    Entity,
    Entity,
    imgui::render::snapshot::ManagedTextureId,
) {
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
        ))
        .id();

    app.world_mut().spawn((
        Camera {
            is_active: false,
            order: 99,
            ..Default::default()
        },
        RenderTarget::Window(WindowRef::Primary),
    ));

    app.world_mut().spawn((
        Camera {
            order: -10,
            ..Default::default()
        },
        RenderTarget::Window(WindowRef::Primary),
        ImguiOverlayDisabled,
    ));

    let mut texture = imgui::texture::TextureData::new();
    texture.create(imgui::texture::TextureFormat::RGBA32, 1, 1);
    texture.set_data(&[255, 0, 255, 255]);
    texture.set_status(imgui::texture::TextureStatus::WantCreate);
    let texture_id = texture.unique_id();

    {
        let mut context = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
        let ctx = context.context_mut();
        ctx.io_mut().set_config_input_trickle_event_queue(false);
        let _ = ctx.font_atlas_mut().build();
        let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
        ctx.register_user_texture(&mut texture);
    }

    app.insert_non_send(ManagedTexture(texture));

    (app, primary_window, camera, texture_id)
}

fn draw_managed_texture(mut contexts: ImguiContexts, mut texture: NonSendMut<ManagedTexture>) {
    let ui = contexts
        .primary_ui_mut()
        .expect("render extraction test should run inside an open ImGui frame");
    ui.image(&mut *texture.0, [16.0, 16.0]);
}

#[test]
fn render_extract_clones_snapshot_texture_requests_and_camera_targets() {
    let _guard = imgui_context_guard();
    let (mut app, primary_window, camera, texture_id) = app_with_primary_window();
    app.add_systems(ImguiPrimaryContextPass, draw_managed_texture);

    app.update();

    assert!(
        !app.world().contains_resource::<ImguiExtractedRenderFrame>(),
        "extracted frame should live in Bevy's render sub-app, not the main world"
    );
    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedRenderFrame>();
    assert_eq!(extracted.frame_index(), Some(1));
    let snapshot = extracted.snapshot().expect("snapshot should be extracted");
    assert_eq!(snapshot.draw.display_size, [640.0, 360.0]);
    assert!(
        snapshot
            .texture_requests
            .iter()
            .any(|request| request.id == texture_id),
        "managed texture requests must cross the extraction boundary"
    );

    let targets = extracted.camera_targets();
    assert_eq!(targets.len(), 1, "inactive cameras should not be extracted");
    assert_eq!(targets[0].camera, camera);
    assert_eq!(targets[0].order, 3);
    assert_eq!(
        targets[0].target,
        NormalizedRenderTarget::Window(WindowRef::Entity(primary_window).normalize(None).unwrap())
    );
}

#[test]
fn render_extract_clears_stale_snapshot_after_primary_window_is_removed() {
    let _guard = imgui_context_guard();
    let (mut app, primary_window, _camera, _texture_id) = app_with_primary_window();
    app.add_systems(ImguiPrimaryContextPass, |mut contexts: ImguiContexts| {
        let Some(ui) = contexts.primary_ui_mut() else {
            return;
        };
        ui.text("render extract guard");
    });

    app.update();
    assert!(
        app.sub_app(RenderApp)
            .world()
            .resource::<ImguiExtractedRenderFrame>()
            .snapshot()
            .is_some(),
        "first update should extract a snapshot"
    );

    app.world_mut().despawn(primary_window);
    app.update();

    let render_world = app.sub_app(RenderApp).world();
    let extracted = render_world.resource::<ImguiExtractedRenderFrame>();
    assert_eq!(extracted.frame_index(), Some(1));
    assert!(
        extracted.snapshot().is_none(),
        "render extraction must not keep drawing the last frame after the primary window disappears"
    );

    let prepared = render_world.resource::<ImguiPreparedRenderFrame>();
    assert_eq!(prepared.frame_index(), Some(1));
    assert!(prepared.draws().is_empty());
    assert!(prepared.vertices().is_empty());
    assert!(prepared.indices().is_empty());
}

#[test]
fn render_extract_uses_one_overlay_camera_per_render_target() {
    let _guard = imgui_context_guard();
    let (mut app, primary_window, _camera, _texture_id) = app_with_primary_window();
    let overlay_camera = app
        .world_mut()
        .spawn((
            Camera {
                order: 12,
                ..Default::default()
            },
            RenderTarget::Window(WindowRef::Primary),
        ))
        .id();
    app.add_systems(ImguiPrimaryContextPass, draw_managed_texture);

    app.update();

    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedRenderFrame>();
    let targets = extracted.camera_targets();
    assert_eq!(
        targets.len(),
        1,
        "the same ImGui overlay must not be drawn once for every active Bevy camera on a target"
    );
    assert_eq!(targets[0].camera, overlay_camera);
    assert_eq!(targets[0].order, 12);
    assert_eq!(
        targets[0].target,
        NormalizedRenderTarget::Window(WindowRef::Entity(primary_window).normalize(None).unwrap())
    );
}

#[test]
fn render_extract_routes_overlay_to_primary_and_secondary_windows() {
    let _guard = imgui_context_guard();
    let (mut app, primary_window, primary_camera, _texture_id) = app_with_primary_window();
    let secondary_window = app.world_mut().spawn(Window::default()).id();
    let secondary_camera = app
        .world_mut()
        .spawn((
            Camera {
                order: 7,
                ..Default::default()
            },
            RenderTarget::Window(WindowRef::Entity(secondary_window)),
        ))
        .id();
    app.add_systems(ImguiPrimaryContextPass, draw_managed_texture);

    app.update();

    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedRenderFrame>();
    let targets = extracted.camera_targets();
    let expected_primary_target =
        NormalizedRenderTarget::Window(WindowRef::Entity(primary_window).normalize(None).unwrap());
    let expected_secondary_target = NormalizedRenderTarget::Window(
        WindowRef::Entity(secondary_window).normalize(None).unwrap(),
    );
    assert_eq!(targets.len(), 2);
    assert_eq!(targets[0].camera, primary_camera);
    assert_eq!(targets[0].target, expected_primary_target);
    assert_eq!(targets[1].camera, secondary_camera);
    assert_eq!(targets[1].target, expected_secondary_target);

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
    assert!(!secondary_draws.is_empty());
    assert!(
        primary_draws
            .iter()
            .all(|draw| draw.target == expected_primary_target)
    );
    assert!(
        secondary_draws
            .iter()
            .all(|draw| draw.target == expected_secondary_target)
    );
}

#[test]
#[cfg(not(feature = "multi-viewport"))]
fn render_extract_ignores_viewport_window_mapping_until_multi_viewport_is_supported() {
    let _guard = imgui_context_guard();
    let (mut app, primary_window, primary_camera, _texture_id) = app_with_primary_window();
    let secondary_viewport_id = imgui::Id::from(0xC0FFEE);
    let secondary_window = app
        .world_mut()
        .spawn((
            Window::default(),
            ImguiViewportWindow {
                viewport_id: secondary_viewport_id,
            },
        ))
        .id();
    let secondary_camera = app
        .world_mut()
        .spawn((
            Camera {
                order: 7,
                ..Default::default()
            },
            RenderTarget::Window(WindowRef::Entity(secondary_window)),
        ))
        .id();
    app.add_systems(ImguiPrimaryContextPass, draw_managed_texture);

    app.update();

    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedRenderFrame>();
    let targets = extracted.camera_targets();
    let expected_primary_target =
        NormalizedRenderTarget::Window(WindowRef::Entity(primary_window).normalize(None).unwrap());
    let expected_secondary_target = NormalizedRenderTarget::Window(
        WindowRef::Entity(secondary_window).normalize(None).unwrap(),
    );

    assert_eq!(targets.len(), 2);
    assert_eq!(targets[0].camera, primary_camera);
    assert_eq!(targets[0].viewport_id, None);
    assert_eq!(targets[0].target, expected_primary_target);
    assert_eq!(targets[1].camera, secondary_camera);
    assert_eq!(targets[1].viewport_id, None);
    assert_eq!(targets[1].target, expected_secondary_target);

    let prepared = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiPreparedRenderFrame>();
    assert!(
        prepared
            .draws()
            .iter()
            .all(|draw| draw.viewport_id.is_none())
    );
}

#[test]
#[cfg(feature = "multi-viewport")]
fn renderer_prepare_routes_secondary_viewport_draws_only_to_matching_window() {
    let _guard = imgui_context_guard();
    let mut app = App::new();
    app.add_plugins(ExtractPlugin::default());
    app.add_plugins(ImguiPlugin::new(ImguiBackendConfig {
        name: "render-routing".to_owned(),
        docking: true,
        multi_viewport: true,
    }));
    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());

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
        ))
        .id();

    {
        let mut context = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
        let ctx = context.context_mut();
        ctx.io_mut().set_config_input_trickle_event_queue(false);
        let _ = ctx.font_atlas_mut().build();
        let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
        ctx.io_mut().set_config_viewports_no_auto_merge(true);
    }

    app.init_resource::<SecondaryViewportRouteState>();
    app.add_systems(
        ImguiPrimaryContextPass,
        move |mut contexts: ImguiContexts, mut route: ResMut<SecondaryViewportRouteState>| {
            let ui = contexts
                .primary_ui_mut()
                .expect("render extraction test should run inside an open ImGui frame");
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
    );

    app.update();
    let secondary_viewport_id = app
        .world()
        .resource::<SecondaryViewportRouteState>()
        .viewport_id
        .expect("first frame should create a Dear ImGui viewport for the secondary window");
    let secondary_window = app
        .world_mut()
        .spawn((
            Window::default(),
            ImguiViewportWindow {
                viewport_id: secondary_viewport_id,
            },
        ))
        .id();
    let secondary_camera = app
        .world_mut()
        .spawn((
            Camera {
                order: 7,
                ..Default::default()
            },
            RenderTarget::Window(WindowRef::Entity(secondary_window)),
        ))
        .id();
    {
        let mut route = app
            .world_mut()
            .resource_mut::<SecondaryViewportRouteState>();
        route.window = Some(secondary_window);
        route.camera = Some(secondary_camera);
    }
    app.update();

    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedRenderFrame>();
    let snapshot = extracted.snapshot().expect("snapshot should be extracted");
    assert!(
        snapshot.viewport_draw(secondary_viewport_id).is_some(),
        "core snapshot should carry secondary Dear ImGui viewport draw data"
    );

    let prepared = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiPreparedRenderFrame>();
    let expected_primary_target =
        NormalizedRenderTarget::Window(WindowRef::Entity(primary_window).normalize(None).unwrap());
    let expected_secondary_target = NormalizedRenderTarget::Window(
        WindowRef::Entity(secondary_window).normalize(None).unwrap(),
    );
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
    assert!(!secondary_draws.is_empty());
    assert!(primary_draws.iter().all(|draw| {
        draw.target == expected_primary_target && draw.viewport_id != Some(secondary_viewport_id)
    }));
    assert!(secondary_draws.iter().all(|draw| {
        draw.target == expected_secondary_target && draw.viewport_id == Some(secondary_viewport_id)
    }));
    assert_ne!(
        prepared.uniforms_for_camera(primary_camera),
        prepared.uniforms_for_camera(secondary_camera),
        "secondary viewport rendering needs a viewport-specific projection"
    );
}

#[test]
fn renderer_prepare_flattens_extracted_snapshot_for_pipeline_consumption() {
    let _guard = imgui_context_guard();
    let (mut app, primary_window, _camera, texture_id) = app_with_primary_window();
    app.add_systems(ImguiPrimaryContextPass, draw_managed_texture);

    app.update();

    let prepared = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiPreparedRenderFrame>();
    assert_eq!(prepared.frame_index(), Some(1));
    assert!(!prepared.vertices().is_empty());
    assert!(!prepared.indices().is_empty());
    assert!(
        prepared.texture_request_count() >= 1,
        "managed texture requests stay associated with prepared renderer data"
    );

    let draw = prepared
        .draws()
        .iter()
        .find(|draw| matches!(draw.texture, TextureBinding::Managed(id) if id == texture_id))
        .expect("the managed texture draw should be prepared");
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
        sample_count: 4,
    });

    assert_eq!(descriptor.layout.len(), 2);
    assert_eq!(descriptor.vertex.shader, IMGUI_SHADER_HANDLE);
    assert_eq!(
        descriptor.vertex.entry_point.as_deref(),
        Some(IMGUI_VERTEX_ENTRY_POINT)
    );
    assert_eq!(descriptor.vertex.buffers.len(), 1);
    assert_eq!(descriptor.multisample.count, 4);

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

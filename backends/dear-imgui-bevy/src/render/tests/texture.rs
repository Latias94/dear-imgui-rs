#![cfg(feature = "render")]

use crate::test_util::imgui_context_guard;
use bevy::prelude::GlobalTransform;
use bevy_app::App;
use bevy_asset::{AssetId, Assets};
use bevy_camera::{
    Camera, CameraMainTextureUsages, CameraOutputMode, ClearColorConfig, NormalizedRenderTarget,
    RenderTarget,
};
use bevy_core_pipeline::Core2d;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;
use bevy_image::Image;
use bevy_math::{Mat4, UVec2, UVec4};
use bevy_render::{
    Render, RenderApp,
    camera::CameraRenderGraph,
    camera::ExtractedCamera,
    extract_plugin::ExtractPlugin,
    render_resource::{TextureFormat, TextureUsages},
    view::{ColorGrading, ExtractedView, Msaa, RetainedViewEntity},
};
use bevy_window::{PrimaryWindow, Window, WindowRef, WindowResolution};
use dear_imgui_bevy::{
    ImguiAppExt, ImguiBevyTextures, ImguiContextConfig, ImguiContexts, ImguiFrame, ImguiPlugin,
    ImguiPrimaryPass, ImguiTexture,
    render::{ImguiExtractedBevyTextures, ImguiTextureBindGroups},
    route::{ImguiDiagnosticKind, ImguiDiagnosticOrigin, ImguiDiagnostics, ImguiRenderRoute},
};
use dear_imgui_rs::{self as imgui, render::TextureBinding};
use std::collections::HashMap;

struct SecondaryUi;

struct ManagedTexture(imgui::ManagedTextureId);

#[derive(Resource)]
struct ContextManagedTextures(HashMap<imgui::ContextId, imgui::ManagedTextureId>);

#[derive(Resource)]
struct BevyImageTexture {
    texture: ImguiTexture,
}

#[derive(Resource)]
struct OneShotBevyImageTexture(Option<ImguiTexture>);

fn app_with_render_world() -> App {
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
                order: 1,
                ..Default::default()
            },
            RenderTarget::Window(WindowRef::Primary),
            CameraRenderGraph::new(Core2d),
        ))
        .id();
    install_render_view(
        &mut app,
        camera,
        NormalizedRenderTarget::Window(
            WindowRef::Entity(primary_window)
                .normalize(None)
                .expect("entity window target should normalize"),
        ),
    );

    let primary_id = app
        .world()
        .get_non_send::<ImguiContexts>()
        .unwrap()
        .primary_id()
        .unwrap();
    app.world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .configure(primary_id, |context| {
            context.io_mut().set_config_input_trickle_event_queue(false);
            let _ = context.font_atlas().build();
            let _ = context.set_ini_filename::<std::path::PathBuf>(None);
        })
        .unwrap();

    app
}

fn install_render_view(app: &mut App, camera: Entity, target: NormalizedRenderTarget) {
    let physical_target_size = UVec2::new(1280, 720);
    app.sub_app_mut(RenderApp).world_mut().spawn((
        ExtractedView {
            retained_view_entity: RetainedViewEntity::new(camera.into(), None, 0),
            clip_from_view: Mat4::IDENTITY,
            world_from_view: GlobalTransform::IDENTITY,
            clip_from_world: None,
            target_format: TextureFormat::Rgba8UnormSrgb,
            viewport: UVec4::new(0, 0, physical_target_size.x, physical_target_size.y),
            color_grading: ColorGrading::default(),
            invert_culling: false,
        },
        ExtractedCamera {
            target: Some(target),
            physical_viewport_size: Some(physical_target_size),
            physical_target_size: Some(physical_target_size),
            viewport: None,
            schedule: Core2d.intern(),
            order: 1,
            output_mode: CameraOutputMode::default(),
            msaa_writeback: Default::default(),
            clear_color: ClearColorConfig::Default,
            sorted_camera_index_for_target: 0,
            exposure: 1.0,
            hdr: false,
            compositing_space: None,
        },
        CameraMainTextureUsages(TextureUsages::RENDER_ATTACHMENT),
        Msaa::Off,
    ));
}

fn register_managed_texture(app: &mut App) -> imgui::ManagedTextureId {
    let mut texture = imgui::texture::OwnedTextureData::new();
    texture.create(imgui::texture::TextureFormat::RGBA32, 1, 1);
    texture.set_data(&[255, 0, 255, 255]);
    let primary_id = app
        .world()
        .get_non_send::<ImguiContexts>()
        .unwrap()
        .primary_id()
        .unwrap();
    let texture_id = app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .configure(primary_id, |context| context.register_texture(texture))
        .unwrap();

    app.insert_non_send(ManagedTexture(texture_id));
    texture_id
}

fn draw_managed_texture(frame: ImguiFrame<'_>, texture: NonSend<ManagedTexture>) {
    frame.ui().image(texture.0, [16.0, 16.0]);
}

fn draw_bevy_image<P: 'static>(frame: ImguiFrame<'_, P>, texture: Res<BevyImageTexture>) {
    frame.ui().get_foreground_draw_list().add_image(
        &texture.texture,
        [0.0, 0.0],
        [32.0, 24.0],
        [0.0, 0.0],
        [1.0, 1.0],
        [1.0, 1.0, 1.0, 1.0],
    );
}

fn draw_and_release_bevy_image(
    frame: ImguiFrame<'_>,
    mut texture: ResMut<OneShotBevyImageTexture>,
) {
    if let Some(texture) = texture.0.as_ref() {
        frame.ui().get_foreground_draw_list().add_image(
            texture,
            [0.0, 0.0],
            [32.0, 24.0],
            [0.0, 0.0],
            [1.0, 1.0],
            [1.0, 1.0, 1.0, 1.0],
        );
    }
    texture.0.take();
}

fn draw_context_managed_texture<P: 'static>(
    frame: ImguiFrame<'_, P>,
    textures: Res<ContextManagedTextures>,
) {
    let context_id = frame.context_id();
    let Some(texture) = textures.0.get(&context_id).copied() else {
        return;
    };
    frame.ui().image(texture, [16.0, 16.0]);
}

#[test]
fn managed_texture_create_request_repeats_after_gpu_retry() {
    let _guard = imgui_context_guard();
    let mut app = app_with_render_world();
    let texture_id = register_managed_texture(&mut app);
    let primary_id = texture_id.context_id();
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(&primary_pass, primary_pass.system(draw_managed_texture));

    app.update();

    assert_eq!(
        app.world()
            .get_non_send::<ImguiContexts>()
            .expect("the plugin must retain its Context registry")
            .frame_index(primary_id)
            .expect("the primary Context must remain registered"),
        1
    );

    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<dear_imgui_bevy::render::ImguiExtractedRenderFrame>();
    assert_eq!(extracted.frame_index(primary_id), Some(1));
    assert!(
        extracted.snapshot(primary_id).is_none(),
        "render preparation should commit the one-shot snapshot with explicit retry feedback"
    );

    let texture = app.world().get_non_send::<ManagedTexture>().unwrap().0;
    app.world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .configure(primary_id, |context| {
            context
                .with_texture(texture, |texture| {
                    assert_eq!(texture.status(), imgui::texture::TextureStatus::WantCreate);
                    assert!(texture.texture_id().is_null());
                })
                .expect("managed texture should remain active");
        })
        .unwrap();

    let prepared = app
        .sub_app(RenderApp)
        .world()
        .resource::<dear_imgui_bevy::render::ImguiPreparedRenderFrame>();
    assert!(!prepared.draws().is_empty());
    assert!(prepared.texture_request_count(primary_id) >= 1);
    let texture = app.world().get_non_send::<ManagedTexture>().unwrap();
    assert_eq!(texture_id, texture.0);

    app.update();
    assert_eq!(
        app.world()
            .get_non_send::<ImguiContexts>()
            .expect("the plugin must retain its Context registry")
            .frame_index(primary_id)
            .expect("the primary Context must remain registered"),
        2
    );
    let prepared = app
        .sub_app(RenderApp)
        .world()
        .resource::<dear_imgui_bevy::render::ImguiPreparedRenderFrame>();
    assert!(
        prepared.texture_request_count(primary_id) >= 1,
        "retried creates must repeat in the next snapshot"
    );
}

#[test]
fn managed_texture_requests_and_lifecycles_are_isolated_by_context() {
    let _guard = imgui_context_guard();
    let mut app = app_with_render_world();
    let primary_pass = app.imgui_primary_pass();
    let secondary_pass = app.declare_imgui_pass::<SecondaryUi>();

    let primary_id = app
        .world()
        .get_non_send::<ImguiContexts>()
        .unwrap()
        .primary_id()
        .unwrap();
    let secondary_id = app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .create(ImguiContextConfig::new(&secondary_pass))
        .expect("secondary Context admission should succeed");

    let register = |app: &mut App, context_id: imgui::ContextId, pixel: [u8; 4]| {
        let mut texture = imgui::texture::OwnedTextureData::new();
        texture.create(imgui::texture::TextureFormat::RGBA32, 1, 1);
        texture.set_data(&pixel);
        app.world_mut()
            .get_non_send_mut::<ImguiContexts>()
            .unwrap()
            .configure(context_id, |context| {
                context.io_mut().set_config_input_trickle_event_queue(false);
                context.io_mut().set_display_size([1280.0, 720.0]);
                let _ = context.font_atlas().build();
                let _ = context.set_ini_filename::<std::path::PathBuf>(None);
                context.register_texture(texture)
            })
            .unwrap()
    };
    let primary_texture = register(&mut app, primary_id, [255, 0, 0, 255]);
    let secondary_texture = register(&mut app, secondary_id, [0, 255, 0, 255]);
    assert_eq!(primary_texture.context_id(), primary_id);
    assert_eq!(secondary_texture.context_id(), secondary_id);
    assert_ne!(
        primary_texture, secondary_texture,
        "the first managed texture slot in each Context must retain Context identity"
    );

    app.insert_resource(ContextManagedTextures(HashMap::from([
        (primary_id, primary_texture),
        (secondary_id, secondary_texture),
    ])));
    app.add_imgui_systems(
        &primary_pass,
        primary_pass.system(draw_context_managed_texture::<ImguiPrimaryPass>),
    );
    app.add_imgui_systems(
        &secondary_pass,
        secondary_pass.system(draw_context_managed_texture::<SecondaryUi>),
    );

    let camera = {
        let world = app.world_mut();
        let mut cameras = world.query_filtered::<Entity, With<Camera>>();
        cameras
            .single(world)
            .expect("texture test should have one render camera")
    };
    app.world_mut()
        .spawn(ImguiRenderRoute::new(primary_id, camera).with_order(0));
    app.world_mut()
        .spawn(ImguiRenderRoute::new(secondary_id, camera).with_order(1));

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
    let extracted = render_world.resource::<dear_imgui_bevy::render::ImguiExtractedRenderFrame>();
    for context_id in [primary_id, secondary_id] {
        assert_eq!(extracted.frame_index(context_id), Some(1));
        assert!(
            extracted.snapshot(context_id).is_none(),
            "each move-only Context snapshot should commit exactly once"
        );
    }

    let prepared = render_world.resource::<dear_imgui_bevy::render::ImguiPreparedRenderFrame>();
    for (context_id, texture) in [
        (primary_id, primary_texture),
        (secondary_id, secondary_texture),
    ] {
        assert!(
            prepared.texture_request_count(context_id) >= 1,
            "each Context should retain its own retried create request"
        );
        assert!(prepared.draws().iter().any(|draw| {
            draw.context_id == context_id
                && draw.texture
                    == TextureBinding::Managed(imgui::render::SnapshotTextureId::User(texture))
        }));
    }
    assert!(!prepared.draws().iter().any(|draw| {
        (draw.context_id == primary_id
            && draw.texture
                == TextureBinding::Managed(imgui::render::SnapshotTextureId::User(
                    secondary_texture,
                )))
            || (draw.context_id == secondary_id
                && draw.texture
                    == TextureBinding::Managed(imgui::render::SnapshotTextureId::User(
                        primary_texture,
                    )))
    }));

    app.update();
    app.world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .configure(primary_id, |context| {
            context
                .remove_texture(primary_texture)
                .expect("primary texture should begin Context-local retirement");
        })
        .unwrap();
    let primary_texture_state = app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .configure(primary_id, |context| {
            context.with_texture(primary_texture, |_| ())
        })
        .unwrap();
    assert!(matches!(
        primary_texture_state,
        Err(imgui::ManagedTextureError::Retiring(texture)) if texture == primary_texture
    ));
    app.world_mut()
        .resource_mut::<ContextManagedTextures>()
        .0
        .remove(&primary_id);

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
            3
        );
    }

    let prepared = app
        .sub_app(RenderApp)
        .world()
        .resource::<dear_imgui_bevy::render::ImguiPreparedRenderFrame>();
    assert!(
        prepared.texture_request_count(primary_id) >= 1,
        "the primary Context should retain its retried destroy request"
    );
    assert!(
        prepared.texture_request_count(secondary_id) >= 1,
        "the secondary Context create request must survive another Context's retirement"
    );
    assert!(prepared.draws().iter().any(|draw| {
        draw.context_id == secondary_id
            && draw.texture
                == TextureBinding::Managed(imgui::render::SnapshotTextureId::User(
                    secondary_texture,
                ))
    }));
    assert!(!prepared.draws().iter().any(|draw| {
        draw.texture
            == TextureBinding::Managed(imgui::render::SnapshotTextureId::User(primary_texture))
    }));

    app.world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .unwrap()
        .configure(secondary_id, |context| {
            context
                .with_texture(secondary_texture, |texture| {
                    assert_eq!(texture.status(), imgui::texture::TextureStatus::WantCreate);
                    assert!(texture.texture_id().is_null());
                })
                .expect("secondary texture should remain active");
        })
        .unwrap();
}

#[test]
fn bevy_image_leases_register_as_stable_imgui_texture_ids_and_extract() {
    let _guard = imgui_context_guard();
    let mut app = app_with_render_world();
    app.init_resource::<Assets<Image>>();

    let handle = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::default());
    let (texture, registered_again) = {
        let mut textures = app.world_mut().resource_mut::<ImguiBevyTextures>();
        (
            textures
                .register_strong(handle.clone())
                .expect("Assets::add should return a retaining Bevy handle"),
            textures
                .register_strong(handle.clone())
                .expect("Assets::add should return a retaining Bevy handle"),
        )
    };
    let texture_id = texture.id();
    assert_eq!(texture_id, registered_again.id());
    assert!(!texture_id.is_null());
    assert!(texture.is_strong());
    drop(registered_again);
    app.insert_resource(BevyImageTexture { texture });
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(
        &primary_pass,
        primary_pass.system(draw_bevy_image::<ImguiPrimaryPass>),
    );

    app.update();

    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedBevyTextures>();
    assert_eq!(extracted.len(), 1);
    assert_eq!(extracted.textures()[0], (texture_id, handle.id()));

    let prepared = app
        .sub_app(RenderApp)
        .world()
        .resource::<dear_imgui_bevy::render::ImguiPreparedRenderFrame>();
    let draw = prepared
        .draws()
        .iter()
        .find(|draw| draw.texture == TextureBinding::Legacy(texture_id))
        .expect("Bevy image draw should preserve the registered legacy texture id");
    assert!(matches!(draw.target, NormalizedRenderTarget::Window(_)));
}

#[test]
fn one_bevy_image_lease_can_draw_from_two_contexts() {
    let _guard = imgui_context_guard();
    let mut app = app_with_render_world();
    let primary_pass = app.imgui_primary_pass();
    let secondary_pass = app.declare_imgui_pass::<SecondaryUi>();

    let primary_id = app
        .world()
        .get_non_send::<ImguiContexts>()
        .expect("ImguiPlugin should install the primary Context")
        .primary_id()
        .expect("ImguiPlugin should create the primary Context");
    let secondary_id = app
        .world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .expect("ImguiPlugin should retain its Context registry")
        .create(ImguiContextConfig::new(&secondary_pass))
        .expect("secondary Context admission should succeed");
    app.world_mut()
        .get_non_send_mut::<ImguiContexts>()
        .expect("ImguiPlugin should retain its Context registry")
        .configure(secondary_id, |context| {
            context.io_mut().set_config_input_trickle_event_queue(false);
            context.io_mut().set_display_size([1280.0, 720.0]);
            let _ = context.font_atlas().build();
            let _ = context.set_ini_filename::<std::path::PathBuf>(None);
        })
        .expect("secondary Context configuration should succeed");

    let texture = app
        .world_mut()
        .resource_mut::<ImguiBevyTextures>()
        .register_weak(AssetId::<Image>::invalid());
    let texture_id = texture.id();
    app.insert_resource(BevyImageTexture { texture });
    app.add_imgui_systems(
        &primary_pass,
        primary_pass.system(draw_bevy_image::<ImguiPrimaryPass>),
    );
    app.add_imgui_systems(
        &secondary_pass,
        secondary_pass.system(draw_bevy_image::<SecondaryUi>),
    );

    let camera = {
        let world = app.world_mut();
        let mut cameras = world.query_filtered::<Entity, With<Camera>>();
        cameras
            .single(world)
            .expect("texture test should have one render camera")
    };
    app.world_mut()
        .spawn(ImguiRenderRoute::new(primary_id, camera).with_order(0));
    app.world_mut()
        .spawn(ImguiRenderRoute::new(secondary_id, camera).with_order(1));

    app.update();

    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedBevyTextures>();
    assert_eq!(
        extracted.len(),
        1,
        "one renderer-global registration should serve both Contexts"
    );
    let prepared = app
        .sub_app(RenderApp)
        .world()
        .resource::<dear_imgui_bevy::render::ImguiPreparedRenderFrame>();
    for context_id in [primary_id, secondary_id] {
        assert!(
            prepared.draws().iter().any(|draw| {
                draw.context_id == context_id && draw.texture == TextureBinding::Legacy(texture_id)
            }),
            "each Context should retain the shared lease's texture binding"
        );
    }
}

#[test]
fn bevy_image_texture_leases_wait_for_render_acknowledgement_before_slot_reuse() {
    let _guard = imgui_context_guard();
    let mut app = app_with_render_world();
    app.init_resource::<Assets<Image>>();

    let (first, second) = {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        (images.add(Image::default()), images.add(Image::default()))
    };
    let first_asset_id = first.id();
    let (first_lease, first_clone, second_lease) = {
        let mut textures = app.world_mut().resource_mut::<ImguiBevyTextures>();
        let first_lease = textures
            .register_strong(first.clone())
            .expect("Assets::add should return a retaining Bevy handle");
        let first_clone = first_lease.clone();
        let second_lease = textures
            .register_strong(second.clone())
            .expect("Assets::add should return a retaining Bevy handle");
        (first_lease, first_clone, second_lease)
    };
    let first_texture_id = first_lease.id();
    let second_texture_id = second_lease.id();

    assert_ne!(first_texture_id, second_texture_id);
    assert!(first_lease.is_strong());
    assert!(second_lease.is_strong());
    drop(first);
    drop(first_lease);
    drop(first_clone);

    app.update();
    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedBevyTextures>();
    assert!(
        extracted
            .textures()
            .iter()
            .any(|(id, asset_id)| *id == first_texture_id && *asset_id == first_asset_id),
        "the last lease release must publish one extraction that still contains the mapping"
    );

    app.update();
    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedBevyTextures>();
    assert!(
        !extracted
            .textures()
            .iter()
            .any(|(id, _)| *id == first_texture_id),
        "the published mapping should withdraw before the renderer acknowledges its cleanup"
    );

    app.update();
    assert_eq!(
        app.world().resource::<ImguiBevyTextures>().len(),
        1,
        "the slot should recycle only after the render-world acknowledgement returns"
    );

    let replacement = app
        .world_mut()
        .resource_mut::<ImguiBevyTextures>()
        .register_weak(first_asset_id);
    assert_eq!(replacement.id(), first_texture_id);
    assert!(replacement.is_weak());
}

#[test]
fn weak_bevy_image_leases_use_the_fallback_and_publish_a_recoverable_diagnostic() {
    let _guard = imgui_context_guard();
    let mut app = app_with_render_world();
    let asset_id = AssetId::<Image>::invalid();
    let texture = app
        .world_mut()
        .resource_mut::<ImguiBevyTextures>()
        .register_weak(asset_id);
    let texture_id = texture.id();
    assert!(texture.is_weak());
    app.insert_resource(BevyImageTexture { texture });
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(
        &primary_pass,
        primary_pass.system(draw_bevy_image::<ImguiPrimaryPass>),
    );

    app.update();

    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedBevyTextures>();
    assert!(
        extracted
            .textures()
            .iter()
            .any(|(id, image)| *id == texture_id && *image == asset_id)
    );
    let prepared = app
        .sub_app(RenderApp)
        .world()
        .resource::<dear_imgui_bevy::render::ImguiPreparedRenderFrame>();
    assert!(
        prepared
            .draws()
            .iter()
            .any(|draw| draw.texture == TextureBinding::Legacy(texture_id)),
        "an unavailable weak image must retain its ImGui draw for fallback rendering"
    );
    assert!(
        app.sub_app(RenderApp)
            .world()
            .resource::<ImguiTextureBindGroups>()
            .is_empty(),
        "an unavailable image must not retain a stale bind group"
    );
    assert!(
        app.world()
            .resource::<ImguiDiagnostics>()
            .entries_for(ImguiDiagnosticOrigin::Texture)
            .any(|diagnostic| {
                matches!(
                    diagnostic.kind(),
                    ImguiDiagnosticKind::UnavailableBevyImageTexture { image }
                        if *image == asset_id
                )
            }),
        "unavailable weak images should publish a recoverable texture diagnostic"
    );
}

#[test]
fn a_lease_dropped_during_ui_submission_survives_the_in_flight_snapshot() {
    let _guard = imgui_context_guard();
    let mut app = app_with_render_world();
    let texture = app
        .world_mut()
        .resource_mut::<ImguiBevyTextures>()
        .register_weak(AssetId::<Image>::invalid());
    let texture_id = texture.id();
    app.insert_resource(OneShotBevyImageTexture(Some(texture)));
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(
        &primary_pass,
        primary_pass.system(draw_and_release_bevy_image),
    );

    app.update();
    assert!(
        app.world()
            .resource::<OneShotBevyImageTexture>()
            .0
            .is_none(),
        "the UI system should release the lease after submitting its image"
    );
    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedBevyTextures>();
    assert!(
        extracted.textures().iter().any(|(id, _)| *id == texture_id),
        "the frame that captured the image must retain its mapping"
    );
    let prepared = app
        .sub_app(RenderApp)
        .world()
        .resource::<dear_imgui_bevy::render::ImguiPreparedRenderFrame>();
    assert!(
        prepared
            .draws()
            .iter()
            .any(|draw| draw.texture == TextureBinding::Legacy(texture_id))
    );

    app.update();
    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedBevyTextures>();
    assert!(
        extracted.textures().iter().any(|(id, _)| *id == texture_id),
        "the retirement publication frame must still retain the mapping"
    );

    app.update();
    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedBevyTextures>();
    assert!(
        !extracted.textures().iter().any(|(id, _)| *id == texture_id),
        "the mapping may withdraw only after the publication frame"
    );
    assert_eq!(app.world().resource::<ImguiBevyTextures>().len(), 1);

    app.update();
    assert!(
        app.world().resource::<ImguiBevyTextures>().is_empty(),
        "the slot should release after prepared draws and stale bind groups are gone"
    );
}

#[test]
fn render_target_texture_images_can_be_registered_and_drawn_as_imgui_viewports() {
    let _guard = imgui_context_guard();
    let mut app = app_with_render_world();
    app.init_resource::<Assets<Image>>();

    let handle = {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        images.add(Image::new_target_texture(
            320,
            180,
            bevy_render::render_resource::TextureFormat::Rgba8UnormSrgb,
            None,
        ))
    };
    let texture = app
        .world_mut()
        .resource_mut::<ImguiBevyTextures>()
        .register_strong(handle.clone())
        .expect("Assets::add should return a retaining Bevy handle");
    let texture_id = texture.id();
    app.insert_resource(BevyImageTexture { texture });
    let primary_pass = app.imgui_primary_pass();
    app.add_imgui_systems(
        &primary_pass,
        primary_pass.system(draw_bevy_image::<ImguiPrimaryPass>),
    );

    app.update();

    let images = app.world().resource::<Assets<Image>>();
    let image = images
        .get(&handle)
        .expect("render target image should exist");
    assert!(
        image
            .texture_descriptor
            .usage
            .contains(bevy_render::render_resource::TextureUsages::RENDER_ATTACHMENT)
    );
    assert!(
        image
            .texture_descriptor
            .usage
            .contains(bevy_render::render_resource::TextureUsages::TEXTURE_BINDING)
    );

    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<ImguiExtractedBevyTextures>();
    assert!(
        extracted
            .textures()
            .iter()
            .any(|(id, asset_id)| *id == texture_id && *asset_id == handle.id()),
        "render target Handle<Image> should extract for ImGui bind-group resolution"
    );

    let prepared = app
        .sub_app(RenderApp)
        .world()
        .resource::<dear_imgui_bevy::render::ImguiPreparedRenderFrame>();
    assert!(
        prepared
            .draws()
            .iter()
            .any(|draw| draw.texture == TextureBinding::Legacy(texture_id)),
        "render target TextureId should be drawable as an ImGui image"
    );
}

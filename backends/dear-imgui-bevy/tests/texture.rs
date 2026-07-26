#![cfg(feature = "render")]

use bevy::prelude::GlobalTransform;
use bevy_app::App;
use bevy_asset::{Assets, Handle};
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
    ImguiBevyTextures, ImguiContextConfig, ImguiContexts, ImguiFrameOutput, ImguiPlugin,
    ImguiPrimaryContextPass, ImguiUi, render::ImguiExtractedBevyTextures, route::ImguiRenderRoute,
};
use dear_imgui_rs::{self as imgui, render::TextureBinding};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

#[derive(ScheduleLabel, Clone, Debug, Eq, Hash, PartialEq)]
struct SecondaryUi;

fn imgui_context_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

struct ManagedTexture(imgui::ManagedTextureId);

#[derive(Resource)]
struct ContextManagedTextures(HashMap<imgui::ContextId, imgui::ManagedTextureId>);

#[derive(Resource, Clone)]
struct BevyImageTexture {
    texture_id: imgui::TextureId,
}

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

fn draw_managed_texture(imgui: ImguiUi, texture: NonSend<ManagedTexture>) {
    let ui = imgui
        .ui()
        .expect("texture test should run inside an open ImGui frame");
    ui.image(texture.0, [16.0, 16.0]);
}

fn draw_bevy_image(imgui: ImguiUi, texture: Res<BevyImageTexture>) {
    let ui = imgui
        .ui()
        .expect("texture test should run inside an open ImGui frame");
    ui.get_foreground_draw_list().add_image(
        texture.texture_id,
        [0.0, 0.0],
        [32.0, 24.0],
        [0.0, 0.0],
        [1.0, 1.0],
        [1.0, 1.0, 1.0, 1.0],
    );
}

fn draw_context_managed_texture(imgui: ImguiUi, textures: Res<ContextManagedTextures>) {
    let context_id = imgui
        .context_id()
        .expect("texture test should run inside an open ImGui frame");
    let Some(texture) = textures.0.get(&context_id).copied() else {
        return;
    };
    let ui = imgui
        .ui()
        .expect("texture test should run inside an open ImGui frame");
    ui.image(texture, [16.0, 16.0]);
}

#[test]
fn managed_texture_create_request_repeats_without_gpu_feedback() {
    let _guard = imgui_context_guard();
    let mut app = app_with_render_world();
    let texture_id = register_managed_texture(&mut app);
    let primary_id = texture_id.context_id();
    app.add_systems(ImguiPrimaryContextPass, draw_managed_texture);

    app.update();

    let output = app.world().resource::<ImguiFrameOutput>();
    let context_output = output
        .get(primary_id)
        .expect("primary Context should publish frame output");
    assert_eq!(context_output.frame_index(), 1);
    assert_eq!(context_output.snapshot_epoch().unwrap().sequence(), 1);
    assert!(context_output.snapshot_error().is_none());

    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<dear_imgui_bevy::render::ImguiExtractedRenderFrame>();
    assert_eq!(extracted.frame_index(primary_id), Some(1));
    assert!(
        extracted.snapshot(primary_id).is_none(),
        "render preparation should commit the one-shot snapshot with empty feedback"
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
    let output = app.world().resource::<ImguiFrameOutput>();
    assert_eq!(
        output
            .get(primary_id)
            .expect("primary Context should publish its second frame")
            .snapshot_epoch()
            .unwrap()
            .sequence(),
        2
    );
    let prepared = app
        .sub_app(RenderApp)
        .world()
        .resource::<dear_imgui_bevy::render::ImguiPreparedRenderFrame>();
    assert!(
        prepared.texture_request_count(primary_id) >= 1,
        "unacknowledged creates must repeat in the next snapshot"
    );
}

#[test]
fn managed_texture_requests_and_lifecycles_are_isolated_by_context() {
    let _guard = imgui_context_guard();
    let mut app = app_with_render_world();
    app.init_schedule(SecondaryUi);

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
        .create(ImguiContextConfig::new(SecondaryUi))
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
    app.add_systems(ImguiPrimaryContextPass, draw_context_managed_texture);
    app.add_systems(SecondaryUi, draw_context_managed_texture);

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

    let output = app.world().resource::<ImguiFrameOutput>();
    for context_id in [primary_id, secondary_id] {
        let context_output = output
            .get(context_id)
            .expect("each Context should publish independent frame output");
        assert_eq!(context_output.frame_index(), 1);
        assert_eq!(context_output.snapshot_epoch().unwrap().sequence(), 1);
        assert!(context_output.snapshot_error().is_none());
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
            "each Context should retain its own unacknowledged create request"
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

    let output = app.world().resource::<ImguiFrameOutput>();
    for context_id in [primary_id, secondary_id] {
        assert_eq!(
            output
                .get(context_id)
                .expect("each Context should continue publishing independent output")
                .snapshot_epoch()
                .unwrap()
                .sequence(),
            3
        );
    }

    let prepared = app
        .sub_app(RenderApp)
        .world()
        .resource::<dear_imgui_bevy::render::ImguiPreparedRenderFrame>();
    assert!(
        prepared.texture_request_count(primary_id) >= 1,
        "the primary Context should retain its unacknowledged destroy request"
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
fn bevy_image_handles_register_as_stable_imgui_texture_ids_and_extract() {
    let _guard = imgui_context_guard();
    let mut app = app_with_render_world();
    app.init_resource::<ImguiBevyTextures>();

    let handle = Handle::<Image>::default();
    let texture_id = app
        .world_mut()
        .resource_mut::<ImguiBevyTextures>()
        .register(&handle);
    let registered_again = app
        .world_mut()
        .resource_mut::<ImguiBevyTextures>()
        .register(&handle);
    assert_eq!(texture_id, registered_again);
    assert!(!texture_id.is_null());
    app.insert_resource(BevyImageTexture { texture_id });
    app.add_systems(ImguiPrimaryContextPass, draw_bevy_image);

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
fn bevy_image_texture_registry_allocates_distinct_reversible_ids() {
    let mut images = Assets::<Image>::default();
    let first = images.add(Image::default());
    let second = images.add(Image::default());

    let mut textures = ImguiBevyTextures::default();
    let first_texture = textures.register(&first);
    let second_texture = textures.register(&second);

    assert_eq!(textures.register(&first), first_texture);
    assert_ne!(first_texture, second_texture);
    assert!(!first_texture.is_null());
    assert!(!second_texture.is_null());
    assert_eq!(textures.asset_id(first_texture), Some(first.id()));
    assert_eq!(textures.asset_id(second_texture), Some(second.id()));
    assert_eq!(textures.unregister(&first), Some(first_texture));
    assert_eq!(textures.asset_id(first_texture), None);
    assert_eq!(textures.asset_id(second_texture), Some(second.id()));
}

#[test]
fn render_target_texture_images_can_be_registered_and_drawn_as_imgui_viewports() {
    let _guard = imgui_context_guard();
    let mut app = app_with_render_world();
    app.init_resource::<Assets<Image>>();
    app.init_resource::<ImguiBevyTextures>();

    let handle = {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        images.add(Image::new_target_texture(
            320,
            180,
            bevy_render::render_resource::TextureFormat::Rgba8UnormSrgb,
            None,
        ))
    };
    let texture_id = app
        .world_mut()
        .resource_mut::<ImguiBevyTextures>()
        .register(&handle);
    app.insert_resource(BevyImageTexture { texture_id });
    app.add_systems(ImguiPrimaryContextPass, draw_bevy_image);

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

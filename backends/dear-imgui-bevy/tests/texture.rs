#![cfg(feature = "render")]

use bevy_app::App;
use bevy_asset::{Assets, Handle};
use bevy_camera::{Camera, NormalizedRenderTarget, RenderTarget};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;
use bevy_image::Image;
use bevy_render::{Render, RenderApp, extract_plugin::ExtractPlugin};
use bevy_window::{PrimaryWindow, Window, WindowRef, WindowResolution};
use dear_imgui_bevy::{
    ImguiBevyTextures, ImguiContext, ImguiContexts, ImguiFrameOutput, ImguiPlugin,
    ImguiPrimaryContextPass, render::ImguiExtractedBevyTextures,
};
use dear_imgui_rs::{self as imgui, render::TextureBinding};
use std::sync::{Mutex, OnceLock};

fn imgui_context_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

struct ManagedTexture(imgui::ManagedTextureId);

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
    app.world_mut().spawn((window, PrimaryWindow));

    app.world_mut().spawn((
        Camera {
            order: 1,
            ..Default::default()
        },
        RenderTarget::Window(WindowRef::Primary),
    ));

    let mut context = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
    let ctx = context.context_mut();
    ctx.io_mut().set_config_input_trickle_event_queue(false);
    let _ = ctx.font_atlas().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

    app
}

fn register_managed_texture(app: &mut App) -> imgui::ManagedTextureId {
    let mut texture = imgui::texture::OwnedTextureData::new();
    texture.create(imgui::texture::TextureFormat::RGBA32, 1, 1);
    texture.set_data(&[255, 0, 255, 255]);
    let mut context = app.world_mut().get_non_send_mut::<ImguiContext>().unwrap();
    let texture_id = context.context_mut().register_texture(texture);
    drop(context);

    app.insert_non_send(ManagedTexture(texture_id));
    texture_id
}

fn draw_managed_texture(mut contexts: ImguiContexts, texture: NonSend<ManagedTexture>) {
    let ui = contexts
        .primary_ui_mut()
        .expect("texture test should run inside an open ImGui frame");
    ui.image(texture.0, [16.0, 16.0]);
}

fn draw_bevy_image(mut contexts: ImguiContexts, texture: Res<BevyImageTexture>) {
    let ui = contexts
        .primary_ui_mut()
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

#[test]
fn managed_texture_create_request_repeats_without_gpu_feedback() {
    let _guard = imgui_context_guard();
    let mut app = app_with_render_world();
    let texture_id = register_managed_texture(&mut app);
    app.add_systems(ImguiPrimaryContextPass, draw_managed_texture);

    app.update();

    let output = app.world().resource::<ImguiFrameOutput>();
    assert_eq!(output.frame_index(), 1);
    assert_eq!(output.snapshot_epoch().unwrap().sequence(), 1);
    assert!(output.snapshot_error().is_none());

    let extracted = app
        .sub_app(RenderApp)
        .world()
        .resource::<dear_imgui_bevy::render::ImguiExtractedRenderFrame>();
    assert_eq!(extracted.frame_index(), Some(1));
    assert!(
        extracted.snapshot().is_none(),
        "render preparation should commit the one-shot snapshot with empty feedback"
    );

    let texture = app.world().get_non_send::<ManagedTexture>().unwrap().0;
    let context = app.world().get_non_send::<ImguiContext>().unwrap();
    context
        .context()
        .with_texture(texture, |texture| {
            assert_eq!(texture.status(), imgui::texture::TextureStatus::WantCreate);
            assert!(texture.texture_id().is_null());
        })
        .expect("managed texture should remain active");

    let prepared = app
        .sub_app(RenderApp)
        .world()
        .resource::<dear_imgui_bevy::render::ImguiPreparedRenderFrame>();
    assert!(!prepared.draws().is_empty());
    assert!(prepared.texture_request_count() >= 1);
    let texture = app.world().get_non_send::<ManagedTexture>().unwrap();
    assert_eq!(texture_id, texture.0);

    app.update();
    let output = app.world().resource::<ImguiFrameOutput>();
    assert_eq!(output.snapshot_epoch().unwrap().sequence(), 2);
    let prepared = app
        .sub_app(RenderApp)
        .world()
        .resource::<dear_imgui_bevy::render::ImguiPreparedRenderFrame>();
    assert!(
        prepared.texture_request_count() >= 1,
        "unacknowledged creates must repeat in the next snapshot"
    );
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

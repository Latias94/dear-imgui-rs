#![cfg(feature = "render")]

use bevy_app::App;
use bevy_camera::{Camera, NormalizedRenderTarget, RenderTarget};
use bevy_ecs::prelude::*;
use bevy_render::{RenderApp, extract_plugin::ExtractPlugin};
use bevy_window::{PrimaryWindow, Window, WindowRef, WindowResolution};
use dear_imgui_bevy::{
    ImguiContext, ImguiContexts, ImguiPlugin, ImguiPrimaryContextPass,
    render::ImguiExtractedRenderFrame,
};
use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn imgui_context_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

struct ManagedTexture(imgui::texture::OwnedTextureData);

fn app_with_primary_window() -> (
    App,
    Entity,
    Entity,
    imgui::render::snapshot::ManagedTextureId,
) {
    let mut app = App::new();
    app.add_plugins(ExtractPlugin::default());
    app.add_plugins(ImguiPlugin::default());

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

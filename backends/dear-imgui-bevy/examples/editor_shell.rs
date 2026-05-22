//! Editor-oriented Dear ImGui shell with a Bevy render target shown as an ImGui image.
//!
//! Run:
//! `cargo run -p dear-imgui-bevy --features render --example editor_shell`

use bevy_app::{App, ScheduleRunnerPlugin, Startup};
use bevy_asset::{Assets, Handle};
use bevy_camera::{Camera, Camera2d, RenderTarget};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;
use bevy_image::Image;
use bevy_render::{
    Render, RenderApp,
    extract_plugin::ExtractPlugin,
    render_resource::{TextureFormat, TextureUsages},
};
use bevy_window::{PrimaryWindow, Window, WindowResolution};
use dear_imgui_bevy::{
    ImguiBevyTextures, ImguiContext, ImguiContexts, ImguiPlugin, ImguiPrimaryContextPass,
};
use dear_imgui_rs::{
    Condition, ConfigFlags, DockNodeFlags, TextureId, WindowFlags, render::snapshot::FrameSnapshot,
};

const SCENE_WIDTH: u32 = 960;
const SCENE_HEIGHT: u32 = 540;

#[derive(Resource, Clone)]
struct SceneViewport {
    image: Handle<Image>,
    texture_id: TextureId,
    size: [u32; 2],
}

#[derive(Resource, Debug)]
struct EditorState {
    show_demo_inspector: bool,
    route_shortcuts_to_imgui: bool,
    scene_hovered: bool,
    last_frame_index: u64,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            show_demo_inspector: true,
            route_shortcuts_to_imgui: true,
            scene_hovered: false,
            last_frame_index: 0,
        }
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins(ScheduleRunnerPlugin::run_once())
        .add_plugins(ExtractPlugin::default())
        .add_plugins(ImguiPlugin::default())
        .init_resource::<Assets<Image>>()
        .init_resource::<ImguiBevyTextures>()
        .init_resource::<EditorState>()
        .add_systems(Startup, setup)
        .add_systems(ImguiPrimaryContextPass, editor_ui);
    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    app.run();
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut textures: ResMut<ImguiBevyTextures>,
    mut imgui: NonSendMut<ImguiContext>,
) {
    commands.spawn((
        Window {
            title: "dear-imgui-bevy editor shell".to_owned(),
            resolution: WindowResolution::new(1440, 900),
            ..Default::default()
        },
        PrimaryWindow,
    ));

    let mut image = Image::new_target_texture(
        SCENE_WIDTH,
        SCENE_HEIGHT,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    image.texture_descriptor.label = Some("dear_imgui_bevy_editor_scene");
    image.texture_descriptor.usage |=
        TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT;
    let image = images.add(image);
    let texture_id = textures.register(&image);

    commands.spawn((
        Camera2d,
        Camera {
            order: -1,
            ..Default::default()
        },
        RenderTarget::Image(image.clone().into()),
    ));
    commands.insert_resource(SceneViewport {
        image,
        texture_id,
        size: [SCENE_WIDTH, SCENE_HEIGHT],
    });

    let context = imgui.context_mut();
    context.io_mut().set_config_input_trickle_event_queue(false);
    let config_flags = context.io().config_flags() | ConfigFlags::DOCKING_ENABLE;
    context.io_mut().set_config_flags(config_flags);
    let _ = context.font_atlas_mut().build();
    let _ = context.set_ini_filename::<std::path::PathBuf>(None);
}

fn editor_ui(
    mut contexts: ImguiContexts,
    viewport: Res<SceneViewport>,
    mut state: ResMut<EditorState>,
    output: Res<dear_imgui_bevy::ImguiFrameOutput>,
) {
    let frame_index = contexts.frame_index().unwrap_or_default();
    let Some(ui) = contexts.primary_ui_mut() else {
        return;
    };

    state.last_frame_index = frame_index;

    let dockspace_id = ui.dockspace_over_main_viewport_with_flags(
        ui.get_id("DearImguiBevyEditorDockspace"),
        DockNodeFlags::PASSTHRU_CENTRAL_NODE,
    );

    render_menu_bar(ui, &mut state);

    ui.set_next_window_dock_id_with_cond(dockspace_id, Condition::FirstUseEver);
    ui.window("Scene")
        .size([720.0, 480.0], Condition::FirstUseEver)
        .build(|| {
            render_scene_view(ui, &viewport, &mut state);
        });

    ui.set_next_window_dock_id_with_cond(dockspace_id, Condition::FirstUseEver);
    ui.window("Inspector")
        .size([320.0, 480.0], Condition::FirstUseEver)
        .build(|| {
            render_inspector(ui, &viewport, &state, output.snapshot());
        });

    ui.set_next_window_dock_id_with_cond(dockspace_id, Condition::FirstUseEver);
    ui.window("Input Policy")
        .size([420.0, 240.0], Condition::FirstUseEver)
        .flags(WindowFlags::NO_COLLAPSE)
        .build(|| {
            render_input_policy(ui, &mut state);
        });
}

fn render_menu_bar(ui: &dear_imgui_rs::Ui, state: &mut EditorState) {
    if let Some(_bar) = ui.begin_main_menu_bar()
        && let Some(_menu) = ui.begin_menu("Window")
    {
        let _ = ui.menu_item_toggle_no_shortcut("Inspector", &mut state.show_demo_inspector, true);
        let _ = ui.menu_item_toggle_no_shortcut(
            "Route shortcuts to ImGui",
            &mut state.route_shortcuts_to_imgui,
            true,
        );
    }
}

fn render_scene_view(ui: &dear_imgui_rs::Ui, viewport: &SceneViewport, state: &mut EditorState) {
    let available = ui.content_region_avail();
    let image_size = fit_aspect(
        [available[0].max(64.0), available[1].max(64.0)],
        viewport.size,
    );
    ui.text("Bevy camera render target");
    ui.separator();
    ui.image_config(viewport.texture_id, image_size)
        .uv0([0.0, 1.0])
        .uv1([1.0, 0.0])
        .build();
    state.scene_hovered = ui.is_item_hovered();
}

fn render_inspector(
    ui: &dear_imgui_rs::Ui,
    viewport: &SceneViewport,
    state: &EditorState,
    snapshot: Option<&FrameSnapshot>,
) {
    if state.show_demo_inspector {
        ui.text("Scene viewport");
        ui.separator();
        ui.text(format!("Image handle: {:?}", viewport.image.id()));
        ui.text(format!("TextureId: {:?}", viewport.texture_id));
        ui.text(format!(
            "Target size: {} x {}",
            viewport.size[0], viewport.size[1]
        ));
        ui.text(format!("Frame: {}", state.last_frame_index));
        ui.text(format!("Scene hovered: {}", state.scene_hovered));
        if let Some(snapshot) = snapshot {
            ui.text(format!("Draw lists: {}", snapshot.draw.draw_lists.len()));
            ui.text(format!(
                "Texture requests: {}",
                snapshot.texture_requests.len()
            ));
        }
    } else {
        ui.text("Inspector hidden from Window menu.");
    }
}

fn render_input_policy(ui: &dear_imgui_rs::Ui, state: &mut EditorState) {
    ui.text("Editor policy");
    ui.separator();
    let io = ui.io();
    ui.text(format!("want_capture_mouse: {}", io.want_capture_mouse()));
    ui.text(format!(
        "want_capture_keyboard: {}",
        io.want_capture_keyboard()
    ));
    ui.checkbox(
        "Route global shortcuts while ImGui wants keyboard",
        &mut state.route_shortcuts_to_imgui,
    );
    ui.separator();
    ui.bullet_text("Keep Bevy messages readable by game/editor systems.");
    ui.bullet_text("Use ImGui capture flags as routing policy, not message deletion.");
    ui.bullet_text("Only forward viewport camera controls when the Scene image is hovered.");
}

fn fit_aspect(available: [f32; 2], target: [u32; 2]) -> [f32; 2] {
    let target_aspect = target[0] as f32 / target[1] as f32;
    let available_aspect = available[0] / available[1].max(1.0);
    if available_aspect > target_aspect {
        [available[1] * target_aspect, available[1]]
    } else {
        [available[0], available[0] / target_aspect]
    }
}

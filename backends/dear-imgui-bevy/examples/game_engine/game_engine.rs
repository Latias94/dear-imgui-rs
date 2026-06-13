//! Compact game-engine style editor integration.
//!
//! Run:
//! `cargo run -p dear-imgui-bevy --features render --example game_engine`
//!
//! Native multi-viewport run:
//! `cargo run -p dear-imgui-bevy --features render,multi-viewport --example game_engine`

use bevy::{
    app::AppExit,
    camera::{ClearColorConfig, RenderTarget, visibility::RenderLayers},
    diagnostic::FrameCount,
    prelude::*,
    window::{PresentMode, WindowPlugin, WindowTheme},
};
use dear_imgui_bevy::{
    ImguiBackendConfig, ImguiBackendStatus, ImguiBevyTextures, ImguiContext, ImguiContexts,
    ImguiPlugin, ImguiPrimaryContextPass, configure_example_context,
    render::{ImguiOverlayCamera, ImguiOverlayDisabled},
};
use dear_imgui_rs::{
    Condition, DockBuilder, DockNodeFlags, SplitDirection, TextureId, WindowFlags,
};

const SCENE_SIZE: [u32; 2] = [960, 540];

#[derive(Component)]
struct SceneObject {
    base: Vec3,
    orbit: f32,
    speed: f32,
}

#[derive(Resource)]
struct ScenePreview {
    texture_id: TextureId,
    size: [u32; 2],
}

#[derive(Resource, Debug)]
struct EditorState {
    dockspace_seeded: bool,
    selected: Option<Entity>,
    playing: bool,
    scene_hovered: bool,
    viewport_zoom: f32,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            dockspace_seeded: false,
            selected: None,
            playing: true,
            scene_hovered: false,
            viewport_zoom: 1.0,
        }
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "dear-imgui-bevy game engine".to_owned(),
                resolution: (1440, 900).into(),
                present_mode: PresentMode::AutoVsync,
                window_theme: Some(WindowTheme::Dark),
                ..Default::default()
            }),
            ..Default::default()
        }))
        .add_plugins(ImguiPlugin::new(ImguiBackendConfig {
            multi_viewport: cfg!(feature = "multi-viewport"),
            ..Default::default()
        }))
        .init_resource::<ImguiBevyTextures>()
        .init_resource::<EditorState>()
        .add_systems(Startup, setup)
        .add_systems(Update, (close_on_escape, animate_scene))
        .add_systems(ImguiPrimaryContextPass, editor_ui)
        .run();
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut textures: ResMut<ImguiBevyTextures>,
    mut imgui: NonSendMut<ImguiContext>,
    mut editor: ResMut<EditorState>,
) {
    // Keep the primary target as an ImGui-only editor surface. The scene is rendered by the
    // offscreen camera below and shown through the docked Scene window.
    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.045, 0.052, 0.062)),
            ..Default::default()
        },
        RenderLayers::none(),
        ImguiOverlayCamera,
    ));

    let mut scene_image = Image::new_target_texture(
        SCENE_SIZE[0],
        SCENE_SIZE[1],
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        None,
    );
    scene_image.texture_descriptor.label = Some("dear_imgui_bevy_game_engine_scene");
    scene_image.texture_descriptor.usage |=
        bevy::render::render_resource::TextureUsages::TEXTURE_BINDING
            | bevy::render::render_resource::TextureUsages::RENDER_ATTACHMENT;
    let scene_image = images.add(scene_image);
    let texture_id = textures.register(&scene_image);

    commands.spawn((
        Camera2d,
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.045, 0.052, 0.062)),
            ..Default::default()
        },
        RenderTarget::Image(scene_image.into()),
        ImguiOverlayDisabled,
    ));
    commands.insert_resource(ScenePreview {
        texture_id,
        size: SCENE_SIZE,
    });

    let light_panel = spawn_rect(
        &mut commands,
        "Light Panel",
        Color::srgb(0.77, 0.80, 0.95),
        Vec2::new(180.0, 120.0),
        Vec3::new(-220.0, 90.0, 0.0),
        16.0,
        0.8,
    );
    spawn_rect(
        &mut commands,
        "Blue Panel",
        Color::srgb(0.18, 0.48, 0.82),
        Vec2::new(220.0, 140.0),
        Vec3::new(40.0, 20.0, 0.2),
        28.0,
        1.2,
    );
    commands.spawn((
        Name::new("Green Probe"),
        Mesh2d(meshes.add(Circle::new(72.0))),
        MeshMaterial2d(materials.add(Color::srgb(0.24, 0.78, 0.54))),
        Transform::from_xyz(210.0, -90.0, 0.4),
        SceneObject {
            base: Vec3::new(210.0, -90.0, 0.4),
            orbit: 24.0,
            speed: -1.0,
        },
    ));

    editor.selected = Some(light_panel);
    configure_example_context(&mut imgui, true);
}

fn spawn_rect(
    commands: &mut Commands,
    name: &'static str,
    color: Color,
    size: Vec2,
    translation: Vec3,
    orbit: f32,
    speed: f32,
) -> Entity {
    commands
        .spawn((
            Name::new(name),
            Sprite::from_color(color, size),
            Transform::from_translation(translation),
            SceneObject {
                base: translation,
                orbit,
                speed,
            },
        ))
        .id()
}

fn close_on_escape(input: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if input.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

fn animate_scene(
    time: Res<Time>,
    editor: Res<EditorState>,
    mut objects: Query<(&mut Transform, &SceneObject)>,
) {
    if !editor.playing {
        return;
    }

    let elapsed = time.elapsed_secs();
    for (mut transform, object) in &mut objects {
        let phase = elapsed * object.speed;
        transform.translation.x = object.base.x + phase.cos() * object.orbit;
        transform.translation.y = object.base.y + phase.sin() * object.orbit;
        transform.rotation = Quat::from_rotation_z(phase * 0.25);
    }
}

fn editor_ui(
    mut contexts: ImguiContexts,
    preview: Res<ScenePreview>,
    mut editor: ResMut<EditorState>,
    objects: Query<(Entity, &Name, &Transform), With<SceneObject>>,
    frame_count: Res<FrameCount>,
    backend: Res<ImguiBackendStatus>,
) {
    let frame_index = contexts.frame_index().unwrap_or_default();
    let Some(ui) = contexts.primary_ui_mut() else {
        return;
    };

    let dockspace_id = ui.dockspace_over_main_viewport_with_flags(
        ui.get_id("DearImguiBevyGameEngineDockspace"),
        DockNodeFlags::NONE,
    );
    seed_dockspace(ui, dockspace_id, &mut editor);

    ui.set_next_window_dock_id_with_cond(dockspace_id, Condition::FirstUseEver);
    ui.window("Scene")
        .size([780.0, 520.0], Condition::FirstUseEver)
        .build(|| render_scene_window(ui, &preview, &mut editor));

    ui.set_next_window_dock_id_with_cond(dockspace_id, Condition::FirstUseEver);
    ui.window("Hierarchy")
        .size([260.0, 420.0], Condition::FirstUseEver)
        .build(|| render_hierarchy(ui, &objects, &mut editor));

    ui.set_next_window_dock_id_with_cond(dockspace_id, Condition::FirstUseEver);
    ui.window("Inspector")
        .size([330.0, 420.0], Condition::FirstUseEver)
        .build(|| render_inspector(ui, &objects, &editor));

    ui.set_next_window_dock_id_with_cond(dockspace_id, Condition::FirstUseEver);
    ui.window("Diagnostics")
        .size([360.0, 180.0], Condition::FirstUseEver)
        .build(|| {
            ui.text(format!("Bevy frame: {}", frame_count.0));
            ui.text(format!("ImGui frame: {frame_index}"));
            ui.text(format!("Scene hovered: {}", editor.scene_hovered));
            ui.text(format!(
                "Multi-viewport requested: {}",
                backend.multi_viewport_requested
            ));
            ui.text(format!(
                "Multi-viewport supported: {}",
                backend.multi_viewport_supported
            ));
        });

    if backend.multi_viewport_supported {
        render_detached_viewport_window(ui, frame_index);
    }
}

fn seed_dockspace(
    ui: &dear_imgui_rs::Ui,
    dockspace_id: dear_imgui_rs::Id,
    editor: &mut EditorState,
) {
    if editor.dockspace_seeded {
        return;
    }

    let viewport = ui.main_viewport();
    DockBuilder::remove_node(ui, dockspace_id);
    let root = DockBuilder::add_node(ui, dockspace_id, DockNodeFlags::NONE);
    DockBuilder::set_node_pos(ui, root, viewport.pos());
    DockBuilder::set_node_size(ui, root, viewport.size());
    let (hierarchy_id, center_id) = DockBuilder::split_node(ui, root, SplitDirection::Left, 0.20);
    let (inspector_id, scene_id) =
        DockBuilder::split_node(ui, center_id, SplitDirection::Right, 0.25);
    let (diagnostics_id, scene_id) =
        DockBuilder::split_node(ui, scene_id, SplitDirection::Down, 0.24);
    DockBuilder::dock_window(ui, "Hierarchy", hierarchy_id);
    DockBuilder::dock_window(ui, "Scene", scene_id);
    DockBuilder::dock_window(ui, "Inspector", inspector_id);
    DockBuilder::dock_window(ui, "Diagnostics", diagnostics_id);
    DockBuilder::finish(ui, root);

    editor.dockspace_seeded = true;
}

fn render_scene_window(ui: &dear_imgui_rs::Ui, preview: &ScenePreview, editor: &mut EditorState) {
    if ui.button(if editor.playing { "Pause" } else { "Play" }) {
        editor.playing = !editor.playing;
    }
    ui.same_line();
    ui.slider_f32("Zoom", &mut editor.viewport_zoom, 0.5, 1.5);

    let available = ui.content_region_avail();
    let fit = fit_aspect(
        [available[0].max(160.0), (available[1] - 36.0).max(120.0)],
        preview.size,
    );
    let image_size = [
        (fit[0] * editor.viewport_zoom).max(96.0),
        (fit[1] * editor.viewport_zoom).max(96.0),
    ];
    ui.image_config(preview.texture_id, image_size)
        .uv0([0.0, 1.0])
        .uv1([1.0, 0.0])
        .build();
    editor.scene_hovered = ui.is_item_hovered();
}

fn render_hierarchy(
    ui: &dear_imgui_rs::Ui,
    objects: &Query<(Entity, &Name, &Transform), With<SceneObject>>,
    editor: &mut EditorState,
) {
    for (entity, name, _transform) in objects.iter() {
        if ui
            .selectable_config(name.as_str())
            .selected(editor.selected == Some(entity))
            .build()
        {
            editor.selected = Some(entity);
        }
    }
}

fn render_inspector(
    ui: &dear_imgui_rs::Ui,
    objects: &Query<(Entity, &Name, &Transform), With<SceneObject>>,
    editor: &EditorState,
) {
    let Some(selected) = editor.selected else {
        ui.text("No entity selected.");
        return;
    };

    let Ok((entity, name, transform)) = objects.get(selected) else {
        ui.text("Selected entity is no longer available.");
        return;
    };

    ui.text(name.as_str());
    ui.separator();
    ui.text(format!("Entity: {entity}"));
    ui.text(format!(
        "Translation: {:.1}, {:.1}, {:.1}",
        transform.translation.x, transform.translation.y, transform.translation.z
    ));
    ui.text(format!("Scale: {:.1}", transform.scale.x));
}

fn render_detached_viewport_window(ui: &dear_imgui_rs::Ui, frame_index: u64) {
    let main_viewport = ui.main_viewport();
    let main_pos = main_viewport.pos();
    let main_size = main_viewport.size();
    let detached_pos = [main_pos[0] + main_size[0] + 24.0, main_pos[1] + 96.0];

    ui.window("Detached Viewport")
        .position(detached_pos, Condition::FirstUseEver)
        .size([380.0, 260.0], Condition::FirstUseEver)
        .flags(WindowFlags::NO_SAVED_SETTINGS)
        .build(|| {
            ui.text("This window is owned by a Dear ImGui platform viewport.");
            ui.separator();
            ui.text("The Bevy backend creates and renders the OS window.");
            ui.text(format!("Frame: {frame_index}"));
            ui.separator();
            ui.text("Drag the title bar back into the dockspace to dock it again.");
        });
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

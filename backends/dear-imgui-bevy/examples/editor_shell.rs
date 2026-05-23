//! Persistent editor-oriented Dear ImGui shell with a seeded split dock layout and a Bevy scene
//! render target shown as an ImGui image.
//!
//! Run:
//! `cargo run -p dear-imgui-bevy --features render --example editor_shell`

use bevy::{
    app::AppExit,
    camera::{ClearColorConfig, RenderTarget},
    prelude::*,
    window::{PresentMode, WindowPlugin, WindowTheme},
};
use dear_imgui_bevy::{
    ImguiBevyTextures, ImguiContext, ImguiContexts, ImguiFrameOutput, ImguiPlugin,
    ImguiPrimaryContextPass, configure_example_context, render::ImguiOverlayDisabled,
};
use dear_imgui_rs::{
    Condition, DockBuilder, DockNodeFlags, SplitDirection, TextureId, WindowFlags,
};

const SCENE_WIDTH: u32 = 960;
const SCENE_HEIGHT: u32 = 540;

#[derive(Component)]
struct EditorSceneObject {
    base_position: Vec3,
    orbit_radius: f32,
    orbit_speed: f32,
}

#[derive(Resource, Clone)]
struct SceneViewport {
    image: Handle<Image>,
    texture_id: TextureId,
    size: [u32; 2],
}

#[derive(Resource, Debug)]
struct EditorState {
    show_inspector: bool,
    show_hierarchy: bool,
    show_input_policy: bool,
    show_diagnostics: bool,
    dock_layout_seeded: bool,
    route_shortcuts_to_imgui: bool,
    route_scene_camera_when_hovered: bool,
    scene_hovered: bool,
    viewport_zoom: f32,
    playback_running: bool,
    selected_entity_name: String,
    last_frame_index: u64,
    editor_events: u32,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            show_inspector: true,
            show_hierarchy: true,
            show_input_policy: true,
            show_diagnostics: true,
            dock_layout_seeded: false,
            route_shortcuts_to_imgui: true,
            route_scene_camera_when_hovered: true,
            scene_hovered: false,
            viewport_zoom: 1.0,
            playback_running: true,
            selected_entity_name: "Camera Preview".to_owned(),
            last_frame_index: 0,
            editor_events: 0,
        }
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "dear-imgui-bevy editor shell".to_owned(),
                resolution: (1440, 900).into(),
                present_mode: PresentMode::AutoVsync,
                window_theme: Some(WindowTheme::Dark),
                ..Default::default()
            }),
            ..Default::default()
        }))
        .add_plugins(ImguiPlugin::default())
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
) {
    // Render the Dear ImGui overlay into the primary window, while the offscreen scene keeps its
    // own image target for the editor viewport.
    commands.spawn(Camera2d);

    let mut image = Image::new_target_texture(
        SCENE_WIDTH,
        SCENE_HEIGHT,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        None,
    );
    image.texture_descriptor.label = Some("dear_imgui_bevy_editor_scene");
    image.texture_descriptor.usage |= bevy::render::render_resource::TextureUsages::TEXTURE_BINDING
        | bevy::render::render_resource::TextureUsages::RENDER_ATTACHMENT;
    let image = images.add(image);
    let texture_id = textures.register(&image);

    commands.spawn((
        Camera2d,
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.045, 0.052, 0.062)),
            ..Default::default()
        },
        RenderTarget::Image(image.clone().into()),
        ImguiOverlayDisabled,
    ));
    commands.insert_resource(SceneViewport {
        image,
        texture_id,
        size: [SCENE_WIDTH, SCENE_HEIGHT],
    });

    commands.spawn((
        Sprite::from_color(Color::srgb(0.12, 0.16, 0.22), Vec2::new(740.0, 420.0)),
        Transform::from_xyz(0.0, 0.0, -2.0),
    ));
    commands.spawn((
        Sprite::from_color(Color::srgb(0.18, 0.48, 0.82), Vec2::new(220.0, 140.0)),
        Transform::from_xyz(-220.0, 40.0, 0.0),
        EditorSceneObject {
            base_position: Vec3::new(-220.0, 40.0, 0.0),
            orbit_radius: 18.0,
            orbit_speed: 1.4,
        },
    ));
    commands.spawn((
        Sprite::from_color(Color::srgb(0.90, 0.62, 0.22), Vec2::new(150.0, 220.0)),
        Transform::from_xyz(170.0, -20.0, 0.5),
        EditorSceneObject {
            base_position: Vec3::new(170.0, -20.0, 0.5),
            orbit_radius: 28.0,
            orbit_speed: -0.9,
        },
    ));
    commands.spawn((
        Mesh2d(meshes.add(Circle::new(76.0))),
        MeshMaterial2d(materials.add(Color::srgb(0.24, 0.78, 0.54))),
        Transform::from_xyz(60.0, 120.0, 1.0),
        EditorSceneObject {
            base_position: Vec3::new(60.0, 120.0, 1.0),
            orbit_radius: 22.0,
            orbit_speed: 1.0,
        },
    ));

    configure_example_context(&mut imgui, true);
}

fn close_on_escape(input: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if input.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

fn animate_scene(
    time: Res<Time>,
    state: Res<EditorState>,
    mut objects: Query<(&mut Transform, &EditorSceneObject)>,
) {
    if !state.playback_running {
        return;
    }

    let elapsed = time.elapsed_secs();
    for (mut transform, object) in &mut objects {
        let phase = elapsed * object.orbit_speed;
        transform.translation.x = object.base_position.x + phase.cos() * object.orbit_radius;
        transform.translation.y = object.base_position.y + phase.sin() * object.orbit_radius;
        transform.rotation = Quat::from_rotation_z(phase * 0.35);
    }
}

fn editor_ui(
    mut contexts: ImguiContexts,
    viewport: Res<SceneViewport>,
    mut state: ResMut<EditorState>,
    output: Res<ImguiFrameOutput>,
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
    seed_editor_dock_layout(ui, dockspace_id, &mut state);

    render_menu_bar(ui, &mut state);

    ui.window("Scene")
        .size([820.0, 560.0], Condition::FirstUseEver)
        .build(|| {
            render_scene_view(ui, &viewport, &mut state);
        });

    if state.show_hierarchy {
        ui.window("Hierarchy")
            .size([260.0, 420.0], Condition::FirstUseEver)
            .build(|| {
                render_hierarchy(ui, &mut state);
            });
    }

    if state.show_inspector {
        ui.window("Inspector")
            .size([340.0, 520.0], Condition::FirstUseEver)
            .build(|| {
                render_inspector(ui, &viewport, &mut state, &output);
            });
    }

    if state.show_input_policy {
        ui.window("Input Policy")
            .size([420.0, 260.0], Condition::FirstUseEver)
            .flags(WindowFlags::NO_COLLAPSE)
            .build(|| {
                render_input_policy(ui, &mut state);
            });
    }

    if state.show_diagnostics {
        ui.window("Diagnostics")
            .size([340.0, 220.0], Condition::FirstUseEver)
            .build(|| {
                render_diagnostics(ui, &state, &output);
            });
    }
}

fn render_menu_bar(ui: &dear_imgui_rs::Ui, state: &mut EditorState) {
    if let Some(_bar) = ui.begin_main_menu_bar()
        && let Some(_menu) = ui.begin_menu("Window")
    {
        let _ = ui.menu_item_toggle_no_shortcut("Hierarchy", &mut state.show_hierarchy, true);
        let _ = ui.menu_item_toggle_no_shortcut("Inspector", &mut state.show_inspector, true);
        let _ = ui.menu_item_toggle_no_shortcut("Input Policy", &mut state.show_input_policy, true);
        let _ = ui.menu_item_toggle_no_shortcut("Diagnostics", &mut state.show_diagnostics, true);
    }
}

fn seed_editor_dock_layout(
    ui: &dear_imgui_rs::Ui,
    dockspace_id: dear_imgui_rs::Id,
    state: &mut EditorState,
) {
    if state.dock_layout_seeded {
        return;
    }

    let viewport = ui.main_viewport();
    let viewport_pos = viewport.pos();
    let viewport_size = viewport.size();

    DockBuilder::remove_node(dockspace_id);
    let root = DockBuilder::add_node(dockspace_id, DockNodeFlags::PASSTHRU_CENTRAL_NODE);
    DockBuilder::set_node_pos(root, viewport_pos);
    DockBuilder::set_node_size(root, viewport_size);

    let (hierarchy_id, center_stack) = DockBuilder::split_node(root, SplitDirection::Left, 0.20);
    let (inspector_id, scene_stack) =
        DockBuilder::split_node(center_stack, SplitDirection::Right, 0.24);
    let (bottom_id, scene_id) = DockBuilder::split_node(scene_stack, SplitDirection::Down, 0.28);

    DockBuilder::dock_window("Hierarchy", hierarchy_id);
    DockBuilder::dock_window("Scene", scene_id);
    DockBuilder::dock_window("Inspector", inspector_id);
    DockBuilder::dock_window("Input Policy", bottom_id);
    DockBuilder::dock_window("Diagnostics", bottom_id);
    DockBuilder::finish(root);

    state.dock_layout_seeded = true;
}

fn render_scene_view(ui: &dear_imgui_rs::Ui, viewport: &SceneViewport, state: &mut EditorState) {
    let available = ui.content_region_avail();
    let fit = fit_aspect(
        [available[0].max(96.0), (available[1] - 44.0).max(96.0)],
        viewport.size,
    );
    let image_size = [
        (fit[0] * state.viewport_zoom).max(64.0),
        (fit[1] * state.viewport_zoom).max(64.0),
    ];

    if ui.button(if state.playback_running {
        "Pause"
    } else {
        "Play"
    }) {
        state.playback_running = !state.playback_running;
        state.editor_events = state.editor_events.saturating_add(1);
    }
    ui.same_line();
    if ui.button("Frame") {
        state.editor_events = state.editor_events.saturating_add(1);
    }
    ui.same_line();
    ui.text(format!("zoom {:.2}x", state.viewport_zoom));
    ui.slider_f32("Viewport zoom", &mut state.viewport_zoom, 0.50, 2.00);
    ui.separator();
    ui.image_config(viewport.texture_id, image_size)
        .uv0([0.0, 1.0])
        .uv1([1.0, 0.0])
        .build();
    state.scene_hovered = ui.is_item_hovered();
}

fn render_hierarchy(ui: &dear_imgui_rs::Ui, state: &mut EditorState) {
    render_selectable_entity(ui, state, "Camera Preview");
    render_selectable_entity(ui, state, "Blue Panel");
    render_selectable_entity(ui, state, "Amber Tool");
    render_selectable_entity(ui, state, "Green Probe");
}

fn render_selectable_entity(ui: &dear_imgui_rs::Ui, state: &mut EditorState, label: &str) {
    let selected = state.selected_entity_name == label;
    let item_label = if selected {
        format!("> {label}")
    } else {
        format!("  {label}")
    };
    if ui.button(item_label) {
        state.selected_entity_name = label.to_owned();
        state.editor_events = state.editor_events.saturating_add(1);
    }
}

fn render_inspector(
    ui: &dear_imgui_rs::Ui,
    viewport: &SceneViewport,
    state: &mut EditorState,
    output: &ImguiFrameOutput,
) {
    ui.text(format!("Selected: {}", state.selected_entity_name));
    ui.separator();
    ui.text(format!("Image handle: {:?}", viewport.image.id()));
    ui.text(format!("TextureId: {:?}", viewport.texture_id));
    ui.text(format!(
        "Target size: {} x {}",
        viewport.size[0], viewport.size[1]
    ));
    ui.text(format!("Backend frame: {}", output.frame_index()));
    ui.text(format!("UI frame: {}", state.last_frame_index));
    ui.text(format!("Scene hovered: {}", state.scene_hovered));
    ui.checkbox("Playback running", &mut state.playback_running);
    ui.checkbox(
        "Route scene camera when hovered",
        &mut state.route_scene_camera_when_hovered,
    );
}

fn render_input_policy(ui: &dear_imgui_rs::Ui, state: &mut EditorState) {
    let io = ui.io();
    ui.text(format!("want_capture_mouse: {}", io.want_capture_mouse()));
    ui.text(format!(
        "want_capture_keyboard: {}",
        io.want_capture_keyboard()
    ));
    ui.text(format!("scene_hovered: {}", state.scene_hovered));
    ui.separator();
    ui.checkbox(
        "Route global shortcuts while ImGui wants keyboard",
        &mut state.route_shortcuts_to_imgui,
    );
    ui.checkbox(
        "Route scene camera only when viewport is hovered",
        &mut state.route_scene_camera_when_hovered,
    );
    ui.separator();
    ui.text_wrapped(
        "Bevy messages stay readable by game and editor systems; Dear ImGui capture flags are policy inputs.",
    );
}

fn render_diagnostics(ui: &dear_imgui_rs::Ui, state: &EditorState, output: &ImguiFrameOutput) {
    ui.text(format!("Editor events: {}", state.editor_events));
    ui.text(format!("Frame output: {}", output.frame_index()));
    if let Some(snapshot) = output.snapshot() {
        ui.text(format!("Draw lists: {}", snapshot.draw.draw_lists.len()));
        ui.text(format!(
            "Texture requests: {}",
            snapshot.texture_requests.len()
        ));
        ui.text(format!(
            "Display size: {:.0} x {:.0}",
            snapshot.draw.display_size[0], snapshot.draw.display_size[1]
        ));
    } else if let Some(error) = output.snapshot_error() {
        ui.text(format!("Snapshot error: {error}"));
    }
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

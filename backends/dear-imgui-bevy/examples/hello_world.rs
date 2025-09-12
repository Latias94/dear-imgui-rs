//! Hello world example with 3D scene and ImGui overlay

use bevy::prelude::*;
use dear_imgui_bevy::prelude::*;

#[derive(Resource)]
struct UiState {
    demo_window_open: bool,
    show_metrics: bool,
    cube_rotation_speed: f32,
}

#[derive(Component)]
struct RotatingCube;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(ImguiPlugin {
            ini_filename: Some("hello_world.ini".into()),
            font_size: 16.0,
            font_oversample_h: 2,
            font_oversample_v: 2,
            ..default()
        })
        .insert_resource(UiState {
            demo_window_open: false,
            show_metrics: false,
            cube_rotation_speed: 1.0,
        })
        .add_systems(Startup, setup_scene)
        .add_systems(Update, (ui_system, rotate_cube_system))
        .run();
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Plane
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(5.0, 5.0))),
        MeshMaterial3d(materials.add(bevy::color::Color::srgb(0.3, 0.5, 0.3))),
    ));

    // Rotating cube
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::default().mesh())),
        MeshMaterial3d(materials.add(bevy::color::Color::srgb(0.8, 0.7, 0.6))),
        Transform::from_xyz(0.0, 0.5, 0.0),
        RotatingCube,
    ));

    // Light
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            intensity: 1500.0,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));

    // Camera
    commands.spawn((
        Transform::from_xyz(1.7, 1.7, 2.0).looking_at(Vec3::new(0.0, 0.3, 0.0), Vec3::Y),
        Camera3d::default(),
    ));
}

fn ui_system(context: NonSendMut<ImguiContext>, mut state: ResMut<UiState>) {
    context.with_ui(|ui| {
        // Main menu bar
        if let Some(_token) = ui.begin_main_menu_bar() {
            ui.menu("Windows", || {
                ui.checkbox("Demo Window", &mut state.demo_window_open);
                ui.checkbox("Metrics", &mut state.show_metrics);
            });
        }

        // Demo window
        if state.demo_window_open {
            ui.show_demo_window(&mut state.demo_window_open);
        }

        // Metrics window
        if state.show_metrics {
            ui.show_metrics_window(&mut state.show_metrics);
        }

        // Control panel
        ui.window("Control Panel")
            .size([300.0, 200.0], Condition::FirstUseEver)
            .position([10.0, 50.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("Dear ImGui + Bevy Integration");
                ui.separator();

                ui.text("Cube Controls:");
                ui.slider("Rotation Speed", 0.0, 5.0, &mut state.cube_rotation_speed);

                ui.separator();

                let io = ui.io();
                ui.text(format!("FPS: {:.1}", io.framerate()));
                ui.text(format!("Frame Time: {:.3}ms", 1000.0 / io.framerate()));

                let mouse_pos = io.mouse_pos();
                ui.text(format!("Mouse: ({:.1}, {:.1})", mouse_pos[0], mouse_pos[1]));
            });
    });
}

fn rotate_cube_system(
    time: Res<Time>,
    state: Res<UiState>,
    mut query: Query<&mut Transform, With<RotatingCube>>,
) {
    for mut transform in query.iter_mut() {
        transform.rotate_y(state.cube_rotation_speed * time.delta_secs());
    }
}

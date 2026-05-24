//! Small Bevy runtime demo with an ImPlot profiler and motion controls.
//!
//! Run:
//! `cargo run -p dear-imgui-bevy --features render --example bevy_plot_controls`

use bevy::{
    app::AppExit,
    prelude::*,
    window::{PresentMode, WindowPlugin, WindowTheme},
};
use dear_imgui_bevy::{
    ImguiContext, ImguiContexts, ImguiPlugin, ImguiPrimaryContextPass, configure_example_context,
};
use dear_imgui_rs::Condition;
use dear_implot::{ImPlotExt, PlotCond};

const HISTORY_CAPACITY: usize = 120;
const BACKGROUND_SIZE: Vec2 = Vec2::new(920.0, 460.0);
const MARKER_SIZE: Vec2 = Vec2::new(110.0, 110.0);
const MOTION_LIMIT: f32 = 320.0;
const MOTION_SPEED_SCALE: f32 = 220.0;

#[derive(Component)]
struct MotionMarker;

#[derive(Resource, Debug)]
struct PlotDemoState {
    paused: bool,
    reverse_direction: bool,
    speed: f32,
    phase_ms: f64,
    current_frame_time_ms: f64,
    current_fps: f64,
    current_x: f64,
    frame_time_ms: Vec<f64>,
    fps_history: Vec<f64>,
    x_history: Vec<f64>,
    sample_x: Vec<f64>,
}

impl Default for PlotDemoState {
    fn default() -> Self {
        Self {
            paused: false,
            reverse_direction: false,
            speed: 1.0,
            phase_ms: 0.0,
            current_frame_time_ms: 0.0,
            current_fps: 0.0,
            current_x: -MOTION_LIMIT as f64,
            frame_time_ms: vec![0.0; HISTORY_CAPACITY],
            fps_history: vec![0.0; HISTORY_CAPACITY],
            x_history: vec![0.0; HISTORY_CAPACITY],
            sample_x: (0..HISTORY_CAPACITY).map(|i| i as f64).collect(),
        }
    }
}

struct PlotDemoContexts {
    plot: dear_implot::PlotContext,
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "dear-imgui-bevy plot controls".to_owned(),
            resolution: (1280, 720).into(),
            present_mode: PresentMode::AutoVsync,
            window_theme: Some(WindowTheme::Dark),
            ..Default::default()
        }),
        ..Default::default()
    }))
    .add_plugins(ImguiPlugin::default())
    .init_resource::<PlotDemoState>()
    .add_systems(Startup, setup_scene)
    .add_systems(Update, (close_on_escape, animate_marker))
    .add_systems(ImguiPrimaryContextPass, plot_demo_ui);

    install_plot_context(&mut app);
    app.run();
}

fn setup_scene(mut commands: Commands, mut imgui: NonSendMut<ImguiContext>) {
    commands.spawn(Camera2d);
    commands.spawn((
        Sprite::from_color(Color::srgb(0.12, 0.16, 0.22), BACKGROUND_SIZE),
        Transform::from_xyz(0.0, 0.0, -1.0),
    ));
    commands.spawn((
        Sprite::from_color(Color::srgb(0.24, 0.72, 0.96), MARKER_SIZE),
        Transform::from_xyz(-MOTION_LIMIT, 0.0, 0.0),
        MotionMarker,
    ));

    configure_example_context(&mut imgui, false);
}

fn install_plot_context(app: &mut App) {
    let contexts = {
        let mut imgui = app
            .world_mut()
            .get_non_send_mut::<ImguiContext>()
            .expect("ImguiPlugin should install ImguiContext before examples create extensions");
        let plot = dear_implot::PlotContext::create(imgui.context_mut());
        PlotDemoContexts { plot }
    };
    app.world_mut().insert_non_send(contexts);
}

fn close_on_escape(input: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if input.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

fn animate_marker(
    time: Res<Time>,
    mut state: ResMut<PlotDemoState>,
    mut marker: Query<&mut Transform, With<MotionMarker>>,
) {
    let Some(mut transform) = marker.iter_mut().next() else {
        return;
    };

    state.phase_ms += time.delta().as_secs_f64() * 1000.0;
    state.current_frame_time_ms = time.delta().as_secs_f64() * 1000.0;
    state.current_fps = if state.current_frame_time_ms > 0.0 {
        1000.0 / state.current_frame_time_ms
    } else {
        0.0
    };

    if state.paused {
        state.current_x = transform.translation.x as f64;
        return;
    }

    let direction = if state.reverse_direction { -1.0 } else { 1.0 };
    transform.translation.x +=
        direction * state.speed * time.delta().as_secs_f32() * MOTION_SPEED_SCALE;

    if transform.translation.x >= MOTION_LIMIT {
        transform.translation.x = MOTION_LIMIT;
        state.reverse_direction = true;
    } else if transform.translation.x <= -MOTION_LIMIT {
        transform.translation.x = -MOTION_LIMIT;
        state.reverse_direction = false;
    }

    transform.rotation = Quat::from_rotation_z(transform.translation.x * 0.01);
    state.current_x = transform.translation.x as f64;
}

fn plot_demo_ui(
    mut contexts: ImguiContexts,
    plot_contexts: NonSend<PlotDemoContexts>,
    mut state: ResMut<PlotDemoState>,
    frame_count: Res<bevy::diagnostic::FrameCount>,
) {
    let imgui_frame_index = contexts.frame_index().unwrap_or_default();
    let Some(ui) = contexts.primary_ui_mut() else {
        return;
    };

    let frame_time_ms = state.current_frame_time_ms;
    let fps = state.current_fps;
    let marker_x = state.current_x;
    push_sample(&mut state.frame_time_ms, frame_time_ms);
    push_sample(&mut state.fps_history, fps.min(240.0));
    push_sample(&mut state.x_history, marker_x);

    ui.window("Controls")
        .size([380.0, 280.0], Condition::FirstUseEver)
        .build(|| {
            ui.text("Bevy drives the motion; ImPlot shows the live state.");
            ui.separator();
            ui.checkbox("Pause motion", &mut state.paused);
            ui.checkbox("Reverse direction", &mut state.reverse_direction);
            ui.slider_f32("Speed", &mut state.speed, 0.25, 4.0);
            ui.separator();
            ui.text(format!("Bevy frame: {}", frame_count.0));
            ui.text(format!("ImGui frame: {}", imgui_frame_index));
            ui.text(format!("Frame time: {:.2} ms", state.current_frame_time_ms));
            ui.text(format!("Approx FPS: {:.1}", state.current_fps));
            ui.text(format!("Marker X: {:.1}", state.current_x));
            ui.text(format!("Phase: {:.0} ms", state.phase_ms));
        });

    ui.window("Profiler")
        .size([560.0, 420.0], Condition::FirstUseEver)
        .build(|| {
            ui.text("The plot history is kept in memory only.");
            ui.text(format!("History points: {}", state.frame_time_ms.len()));
            ui.separator();

            let plot_ui = ui.implot(&plot_contexts.plot);
            let x_max = (HISTORY_CAPACITY - 1) as f64;

            plot_ui.set_next_axes_limits(0.0, x_max, 0.0, 40.0, PlotCond::Always);
            if let Some(plot) = plot_ui.begin_plot_with_size("Frame time (ms)", [-1.0, 115.0]) {
                plot_ui.plot_line("frame time ms", &state.sample_x, &state.frame_time_ms);
                plot.end();
            }

            plot_ui.set_next_axes_limits(0.0, x_max, 0.0, 240.0, PlotCond::Always);
            if let Some(plot) = plot_ui.begin_plot_with_size("FPS", [-1.0, 115.0]) {
                plot_ui.plot_line("fps capped at 240", &state.sample_x, &state.fps_history);
                plot.end();
            }

            plot_ui.set_next_axes_limits(
                0.0,
                x_max,
                -(MOTION_LIMIT as f64) - 40.0,
                MOTION_LIMIT as f64 + 40.0,
                PlotCond::Always,
            );
            if let Some(plot) = plot_ui.begin_plot_with_size("Marker position", [-1.0, 115.0]) {
                plot_ui.plot_line("x position", &state.sample_x, &state.x_history);
                plot.end();
            }
        });
}

fn push_sample(history: &mut Vec<f64>, value: f64) {
    if history.len() == HISTORY_CAPACITY {
        history.remove(0);
    }
    history.push(value);
}

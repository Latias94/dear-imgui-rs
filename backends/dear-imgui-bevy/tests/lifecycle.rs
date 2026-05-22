use bevy_app::App;
use bevy_ecs::prelude::*;
use bevy_window::{PrimaryWindow, Window, WindowResolution};
use dear_imgui_bevy::{
    ImguiContext, ImguiContexts, ImguiFrameOutput, ImguiFrameState, ImguiPlugin,
    ImguiPrimaryContextPass,
};
use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn imgui_context_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[derive(Resource, Default)]
struct LifecycleTrace {
    entries: Vec<&'static str>,
    frame_indices: Vec<u64>,
}

fn app_with_primary_window() -> App {
    let mut app = App::new();
    app.add_plugins(ImguiPlugin::default());

    let mut window = Window {
        resolution: WindowResolution::new(1280, 720),
        ..Default::default()
    };
    window.resolution.set_scale_factor(2.0);
    app.world_mut().spawn((window, PrimaryWindow));

    let mut context = app
        .world_mut()
        .get_non_send_mut::<ImguiContext>()
        .expect("ImguiPlugin should install an ImGui context");
    let ctx = context.context_mut();
    ctx.io_mut().set_config_input_trickle_event_queue(false);
    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

    app
}

fn first_ui_system(mut contexts: ImguiContexts, mut trace: ResMut<LifecycleTrace>) {
    let frame_index = contexts.frame_index().expect("frame should be open");
    let ui = contexts
        .primary_ui_mut()
        .expect("ImguiPrimaryContextPass should run inside an open frame");
    assert_eq!(ui.io().display_size(), [640.0, 360.0]);
    assert_eq!(ui.io().display_framebuffer_scale(), [2.0, 2.0]);
    ui.text("first system");
    trace.entries.push("first");
    trace.frame_indices.push(frame_index);
}

fn second_ui_system(mut contexts: ImguiContexts, mut trace: ResMut<LifecycleTrace>) {
    let frame_index = contexts.frame_index().expect("frame should be open");
    let ui = contexts
        .primary_ui_mut()
        .expect("ImguiPrimaryContextPass should expose the same open frame");
    ui.text("second system");
    trace.entries.push("second");
    trace.frame_indices.push(frame_index);
}

#[test]
fn lifecycle_primary_context_pass_opens_shared_frame_and_snapshots_once() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    app.init_resource::<LifecycleTrace>();
    app.add_systems(
        ImguiPrimaryContextPass,
        (first_ui_system, second_ui_system).chain(),
    );

    app.update();
    app.update();

    let trace = app.world().resource::<LifecycleTrace>();
    assert_eq!(trace.entries, ["first", "second", "first", "second"]);
    assert_eq!(trace.frame_indices, [1, 1, 2, 2]);

    let output = app.world().resource::<ImguiFrameOutput>();
    let snapshot = output
        .snapshot()
        .expect("end-frame system should store a snapshot");
    assert_eq!(output.frame_index(), 2);
    assert_eq!(snapshot.draw.display_size, [640.0, 360.0]);
    assert_eq!(snapshot.draw.framebuffer_scale, [2.0, 2.0]);

    let state = app
        .world()
        .get_non_send::<ImguiFrameState>()
        .expect("frame state should be installed");
    assert!(!state.is_frame_open());

    let context = app
        .world()
        .get_non_send::<ImguiContext>()
        .expect("ImguiContext should still exist");
    assert_eq!(
        context.context().frame_lifecycle_state(),
        imgui::FrameLifecycleState::Rendered
    );
}

#[test]
fn lifecycle_ui_access_is_unavailable_outside_primary_context_pass() {
    let _guard = imgui_context_guard();
    let app = app_with_primary_window();

    let state = app
        .world()
        .get_non_send::<ImguiFrameState>()
        .expect("frame state should be installed");
    assert!(!state.is_frame_open());
    assert!(state.ui().is_none());
}

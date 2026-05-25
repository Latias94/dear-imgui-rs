use bevy_app::App;
use bevy_ecs::prelude::*;
use bevy_time::{Real, Time};
use bevy_window::{PrimaryWindow, Window, WindowResolution};
use dear_imgui_bevy::{
    ImguiBackendConfig, ImguiContext, ImguiContexts, ImguiFrameOutput, ImguiFrameState,
    ImguiPlugin, ImguiPrimaryContextPass,
};
use dear_imgui_rs::{self as imgui, ConfigFlags};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

fn imgui_context_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[derive(Resource, Default)]
struct LifecycleTrace {
    entries: Vec<&'static str>,
    frame_indices: Vec<u64>,
    delta_times: Vec<f32>,
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

fn app_with_primary_window_and_config(config: ImguiBackendConfig) -> App {
    let mut app = App::new();
    app.add_plugins(ImguiPlugin::new(config));

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

fn capture_delta_time(mut contexts: ImguiContexts, mut trace: ResMut<LifecycleTrace>) {
    let ui = contexts
        .primary_ui_mut()
        .expect("ImguiPrimaryContextPass should run inside an open frame");
    trace.delta_times.push(ui.io().delta_time());
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
fn lifecycle_clears_last_snapshot_when_primary_window_is_missing() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();

    app.update();
    assert!(
        app.world()
            .resource::<ImguiFrameOutput>()
            .snapshot()
            .is_some(),
        "first update should produce a render snapshot"
    );

    let mut primary_query = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>();
    let primary = primary_query
        .single(app.world())
        .expect("test app should have one primary window");
    app.world_mut().despawn(primary);

    app.update();

    let output = app.world().resource::<ImguiFrameOutput>();
    assert_eq!(output.frame_index(), 1);
    assert!(
        output.snapshot().is_none(),
        "removing the primary window must not leave stale draw data for render extraction"
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

#[test]
fn lifecycle_uses_bevy_real_delta_time_when_available() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    let mut real_time = Time::<Real>::default();
    real_time.advance_by(Duration::from_millis(42));
    app.insert_resource(real_time);
    app.init_resource::<LifecycleTrace>();
    app.add_systems(ImguiPrimaryContextPass, capture_delta_time);

    app.update();

    let trace = app.world().resource::<LifecycleTrace>();
    assert_eq!(trace.delta_times.len(), 1);
    assert!(
        (trace.delta_times[0] - 0.042).abs() < f32::EPSILON,
        "Dear ImGui delta time should come from Bevy Time<Real>"
    );
}

#[test]
fn lifecycle_invalid_window_scale_factor_falls_back_before_begin_frame() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window();
    let mut primary_query = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>();
    let primary = primary_query
        .single(app.world())
        .expect("test app should have one primary window");
    {
        let mut window = app
            .world_mut()
            .get_mut::<Window>(primary)
            .expect("primary window should exist");
        window.resolution.set(f32::NAN, -10.0);
        window.resolution.set_scale_factor(f32::NAN);
    }
    app.init_resource::<LifecycleTrace>();
    app.add_systems(ImguiPrimaryContextPass, capture_delta_time);

    app.update();

    let context = app
        .world()
        .get_non_send::<ImguiContext>()
        .expect("ImguiContext should still exist");
    assert_eq!(context.context().io().display_size(), [0.0, 0.0]);
    assert_eq!(
        context.context().io().display_framebuffer_scale(),
        [1.0, 1.0]
    );
}

#[test]
fn lifecycle_multi_viewport_request_does_not_advertise_viewports_without_render_app() {
    let _guard = imgui_context_guard();
    let mut app = app_with_primary_window_and_config(ImguiBackendConfig {
        name: "viewport-request".to_owned(),
        docking: true,
        multi_viewport: true,
    });

    app.update();

    let context = app
        .world()
        .get_non_send::<ImguiContext>()
        .expect("ImguiContext should still exist");
    let io = context.context().io();
    let flags = io.config_flags();
    assert!(
        !flags.contains(ConfigFlags::VIEWPORTS_ENABLE),
        "Bevy backend should not advertise Dear ImGui OS-level viewports until render routing is actually installed"
    );
    #[cfg(feature = "multi-viewport")]
    {
        assert!(
            !io.backend_flags()
                .contains(imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS),
            "Platform viewport backend capability should not be advertised before full routing support"
        );
        assert!(
            !io.backend_flags()
                .contains(imgui::BackendFlags::RENDERER_HAS_VIEWPORTS),
            "Renderer viewport backend capability should not be advertised before full routing support"
        );
        assert!(
            !io.backend_flags()
                .contains(imgui::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT),
            "Hovered viewport feedback should not be advertised while viewports are disabled"
        );
    }
}

#[cfg(all(
    feature = "render",
    feature = "multi-viewport",
    not(target_arch = "wasm32")
))]
#[test]
fn lifecycle_multi_viewport_request_advertises_viewports_after_render_app_installation() {
    use bevy_ecs::schedule::ScheduleLabel;
    use bevy_render::{Render, RenderApp, extract_plugin::ExtractPlugin};

    let _guard = imgui_context_guard();
    let mut app = App::new();
    app.add_plugins(ExtractPlugin::default());
    app.sub_app_mut(RenderApp).update_schedule = Some(Render.intern());
    app.add_plugins(ImguiPlugin::new(ImguiBackendConfig {
        name: "viewport-supported".to_owned(),
        docking: true,
        multi_viewport: true,
    }));

    let mut window = Window {
        resolution: WindowResolution::new(1280, 720),
        ..Default::default()
    };
    window.resolution.set_scale_factor(2.0);
    app.world_mut().spawn((window, PrimaryWindow));

    {
        let mut context = app
            .world_mut()
            .get_non_send_mut::<ImguiContext>()
            .expect("ImguiPlugin should install an ImGui context");
        let ctx = context.context_mut();
        ctx.io_mut().set_config_input_trickle_event_queue(false);
        let _ = ctx.font_atlas_mut().build();
        let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
    }

    app.update();

    let context = app
        .world()
        .get_non_send::<ImguiContext>()
        .expect("ImguiContext should still exist");
    let io = context.context().io();
    assert!(io.config_flags().contains(ConfigFlags::VIEWPORTS_ENABLE));
    assert!(
        io.backend_flags()
            .contains(imgui::BackendFlags::PLATFORM_HAS_VIEWPORTS)
    );
    assert!(
        io.backend_flags()
            .contains(imgui::BackendFlags::RENDERER_HAS_VIEWPORTS)
    );
    assert!(
        io.backend_flags()
            .contains(imgui::BackendFlags::HAS_MOUSE_HOVERED_VIEWPORT)
    );
}

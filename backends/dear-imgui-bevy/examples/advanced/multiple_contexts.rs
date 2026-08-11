//! Two Dear ImGui Contexts with independent private passes routed to two Bevy windows.
//!
//! The second camera uses an inset viewport, so its automatically derived input route covers only
//! that region of the second window. Remove the second Context from the primary UI to exercise
//! Context-local renderer teardown while the primary Context keeps running.
//!
//! Run:
//! `cargo run -p dear-imgui-bevy --example multiple_contexts`

use bevy::{
    app::AppExit,
    camera::{RenderTarget, Viewport},
    prelude::*,
    window::{PresentMode, WindowPlugin, WindowRef, WindowTheme},
};
use dear_imgui_bevy::prelude::*;
use dear_imgui_rs::Condition;

struct SecondaryContextPass;

#[derive(Resource)]
struct SecondaryIntegration {
    context_id: Option<ContextId>,
    retirement: Option<ImguiContextRetirementId>,
    window: Entity,
    camera: Entity,
    route: Entity,
}

#[derive(Resource, Default)]
struct MultipleContextState {
    primary_text: String,
    secondary_text: String,
    remove_secondary: bool,
    retirement_status: String,
}

fn main() -> Result {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "dear-imgui-bevy primary Context".to_owned(),
            resolution: (960, 640).into(),
            present_mode: PresentMode::AutoVsync,
            window_theme: Some(WindowTheme::Dark),
            ..Default::default()
        }),
        ..Default::default()
    }));
    app.try_install_imgui(ImguiPlugin::default())?;
    app.init_resource::<MultipleContextState>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                close_on_escape,
                resize_secondary_camera_viewport,
                (retire_secondary_context, finish_secondary_retirement).chain(),
            ),
        );
    let primary_pass = app.imgui_primary_pass()?;
    let secondary_pass = app.declare_imgui_pass::<SecondaryContextPass>()?;
    app.insert_resource(secondary_pass.clone());
    app.add_imgui_systems(&primary_pass, primary_pass.system(primary_ui))?;
    app.add_imgui_systems(&secondary_pass, secondary_pass.system(secondary_ui))?;
    app.run();
    Ok(())
}

fn setup(
    mut commands: Commands,
    mut contexts: NonSendMut<ImguiContexts>,
    secondary_pass: Res<ImguiPass<SecondaryContextPass>>,
) -> Result {
    let primary_context = contexts
        .primary_id()?
        .ok_or("ImguiPlugin should install a primary Context before Startup")?;
    let secondary_context =
        contexts.create(ImguiContextConfig::new(&secondary_pass).with_docking(false))?;

    let primary_camera = commands
        .spawn((
            Camera2d,
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgb(0.07, 0.09, 0.13)),
                ..Default::default()
            },
        ))
        .id();
    commands.spawn(ImguiRenderRoute::new(primary_context, primary_camera));

    let secondary_window = commands
        .spawn(Window {
            title: "dear-imgui-bevy secondary Context".to_owned(),
            resolution: (800, 600).into(),
            present_mode: PresentMode::AutoVsync,
            window_theme: Some(WindowTheme::Dark),
            ..Default::default()
        })
        .id();
    let secondary_camera = commands
        .spawn((
            Camera2d,
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgb(0.12, 0.07, 0.10)),
                viewport: Some(inset_viewport(UVec2::new(800, 600))),
                ..Default::default()
            },
            RenderTarget::Window(WindowRef::Entity(secondary_window)),
        ))
        .id();
    let secondary_route = commands
        .spawn(ImguiRenderRoute::new(secondary_context, secondary_camera))
        .id();

    commands.insert_resource(SecondaryIntegration {
        context_id: Some(secondary_context),
        retirement: None,
        window: secondary_window,
        camera: secondary_camera,
        route: secondary_route,
    });

    Ok(())
}

fn inset_viewport(window_size: UVec2) -> Viewport {
    const MARGIN: u32 = 40;
    Viewport {
        physical_position: UVec2::splat(MARGIN),
        physical_size: UVec2::new(
            window_size.x.saturating_sub(MARGIN * 2).max(1),
            window_size.y.saturating_sub(MARGIN * 2).max(1),
        ),
        ..Default::default()
    }
}

fn resize_secondary_camera_viewport(
    integration: Res<SecondaryIntegration>,
    windows: Query<&Window>,
    mut cameras: Query<&mut Camera>,
) {
    if integration.context_id.is_none() || integration.retirement.is_some() {
        return;
    }
    let (Ok(window), Ok(mut camera)) = (
        windows.get(integration.window),
        cameras.get_mut(integration.camera),
    ) else {
        return;
    };
    let viewport = inset_viewport(window.physical_size());
    let changed = camera.viewport.as_ref().is_none_or(|current| {
        current.physical_position != viewport.physical_position
            || current.physical_size != viewport.physical_size
    });
    if changed {
        camera.viewport = Some(viewport);
    }
}

fn retire_secondary_context(
    mut contexts: NonSendMut<ImguiContexts>,
    mut integration: ResMut<SecondaryIntegration>,
    mut state: ResMut<MultipleContextState>,
) {
    if !state.remove_secondary {
        return;
    }
    let Some(context_id) = integration.context_id else {
        state.remove_secondary = false;
        return;
    };

    match contexts.remove(context_id) {
        Ok(retirement) => {
            integration.retirement = Some(retirement);
            state.remove_secondary = false;
            state.retirement_status =
                "Waiting for the secondary renderer and viewport work to drain...".to_owned();
        }
        Err(error) => {
            state.remove_secondary = false;
            state.retirement_status = format!("Secondary Context removal failed: {error}");
        }
    }
}

fn finish_secondary_retirement(
    mut commands: Commands,
    mut completions: MessageReader<ImguiContextRetired>,
    mut integration: ResMut<SecondaryIntegration>,
    mut state: ResMut<MultipleContextState>,
) {
    let Some(expected) = integration.retirement else {
        return;
    };
    if !completions
        .read()
        .any(|completed| completed.retirement() == expected)
    {
        return;
    }
    commands.entity(integration.route).despawn();
    commands.entity(integration.camera).despawn();
    commands.entity(integration.window).despawn();
    integration.context_id = None;
    integration.retirement = None;
    state.retirement_status =
        "Secondary Context retired; the primary Context is still rendering.".to_owned();
}

fn close_on_escape(input: Res<ButtonInput<KeyCode>>, mut exit: MessageWriter<AppExit>) {
    if input.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

fn primary_ui(
    frame: ImguiFrame<'_>,
    capture: Res<ImguiInputCapture>,
    integration: Res<SecondaryIntegration>,
    mut state: ResMut<MultipleContextState>,
) -> Result {
    let context_id = frame.context_id();
    let frame_index = frame.frame_index();
    let ui = frame.ui();

    ui.window("Primary Context")
        .position([32.0, 32.0], Condition::FirstUseEver)
        .size([520.0, 330.0], Condition::FirstUseEver)
        .build(|| {
            ui.text(format!("Context: {context_id:?}"));
            ui.text(format!("Frame: {frame_index}"));
            ui.input_text("Primary text", &mut state.primary_text)
                .build();
            ui.separator();

            let primary_capture = capture.context(context_id);
            ui.text(format!(
                "Primary capture: mouse={} keyboard={} text={}",
                primary_capture.wants_pointer_input(),
                primary_capture.wants_keyboard_input(),
                primary_capture.wants_text_input()
            ));

            if let Some(secondary_context) = integration.context_id
                && integration.retirement.is_none()
            {
                let secondary_capture = capture.context(secondary_context);
                ui.text(format!(
                    "Secondary capture: mouse={} keyboard={} text={}",
                    secondary_capture.wants_pointer_input(),
                    secondary_capture.wants_keyboard_input(),
                    secondary_capture.wants_text_input()
                ));
                if ui.button("Remove secondary Context") {
                    state.remove_secondary = true;
                }
            } else if integration.retirement.is_some() {
                ui.text("The secondary Context is retiring.");
            } else {
                ui.text("The secondary Context has been removed.");
            }

            if !state.retirement_status.is_empty() {
                ui.separator();
                ui.text_wrapped(&state.retirement_status);
            }
        });

    Ok(())
}

fn secondary_ui(
    frame: ImguiFrame<'_, SecondaryContextPass>,
    mut state: ResMut<MultipleContextState>,
) -> Result {
    let context_id = frame.context_id();
    let frame_index = frame.frame_index();
    let ui = frame.ui();

    ui.window("Secondary Context")
        .position([24.0, 24.0], Condition::FirstUseEver)
        .size([500.0, 280.0], Condition::FirstUseEver)
        .build(|| {
            ui.text(format!("Context: {context_id:?}"));
            ui.text(format!("Frame: {frame_index}"));
            ui.input_text("Secondary text", &mut state.secondary_text)
                .build();
            ui.separator();
            ui.text("Only the inset camera viewport accepts pointer input.");
            ui.text("Focus and capture are tracked independently from the primary window.");
        });

    Ok(())
}

use std::panic::{self, AssertUnwindSafe};

use bevy_app::App;
use bevy_ecs::{
    prelude::{With, World},
    schedule::{InternedScheduleLabel, Schedules},
};
use bevy_time::{Real, Time};
use bevy_window::{PrimaryWindow, Window};
use dear_imgui_rs as imgui;

#[cfg(feature = "render")]
use super::ImguiFrameMailbox;
use super::platform;
use super::{
    ActiveUiCapability, ImguiActiveRendererContextError, ImguiActiveUi, ImguiContextError,
    ImguiContexts, ImguiFrameOutput, ImguiFrameState,
};

pub(crate) fn install_context_lifecycle(app: &mut App) {
    app.insert_non_send(ImguiActiveUi::default());
    app.insert_non_send(ImguiFrameState::default());
    app.init_resource::<ImguiFrameOutput>();
    #[cfg(feature = "render")]
    app.init_resource::<ImguiFrameMailbox>();
}

struct PrimaryFrameMetrics {
    display_size: [f32; 2],
    framebuffer_scale: [f32; 2],
    delta_time: f32,
}

enum PendingFrameOutput {
    #[cfg(feature = "render")]
    Snapshot(
        Result<imgui::render::snapshot::FrameSnapshot, imgui::render::snapshot::SnapshotError>,
    ),
    Rendered,
}

/// Serially activate, frame, schedule, render, and suspend every registered Context.
pub(crate) fn drive_imgui_contexts(world: &mut World) {
    world
        .get_non_send_mut::<ImguiFrameState>()
        .expect("primary frame state must be installed")
        .end();
    world
        .get_non_send::<ImguiActiveUi>()
        .expect("ImguiPlugin must install the active UI capability")
        .capability()
        .revoke();
    let order = world
        .get_non_send::<ImguiContexts>()
        .map(ImguiContexts::drive_order)
        .unwrap_or_default();
    let primary_id = world
        .get_non_send::<ImguiContexts>()
        .and_then(ImguiContexts::primary_id);
    let primary_metrics = primary_frame_metrics(world);
    let active = world
        .get_non_send::<ImguiActiveUi>()
        .expect("ImguiPlugin must install the active UI capability")
        .capability();

    for context_id in order {
        let is_primary = Some(context_id) == primary_id;
        #[cfg(feature = "render")]
        if is_primary
            && world
                .resource::<crate::render::ImguiRendererRelease>()
                .release_requested()
        {
            poll_primary_completions_fail_closed(world, context_id);
            clear_primary_output(world);
            continue;
        }
        if is_primary && primary_metrics.is_none() {
            #[cfg(feature = "render")]
            poll_primary_completions_fail_closed(world, context_id);
            clear_primary_output(world);
            continue;
        }
        let taken = world
            .get_non_send_mut::<ImguiContexts>()
            .expect("ImguiPlugin must install the Context registry")
            .take_for_drive(context_id);
        let (mut owner, config, frame_index) = match taken {
            Ok(taken) => taken,
            Err(ImguiContextError::TeardownInProgress { .. }) => continue,
            Err(error) => panic!("{error}"),
        };

        let metrics = (Some(context_id) == primary_id)
            .then_some(primary_metrics.as_ref())
            .flatten();
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            owner.try_with_active_renderer_context_checked(
                config.multi_viewport(),
                |context, renderer_consumer| {
                    if is_primary {
                        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                        platform::prepare_primary_platform_frame(world, context);
                    }

                    let (display_size, framebuffer_scale, delta_time) = metrics.map_or_else(
                        || {
                            (
                                finite_display_size(context.io().display_size()),
                                finite_framebuffer_scale(context.io().display_framebuffer_scale()),
                                context.io().delta_time().max(f32::EPSILON),
                            )
                        },
                        |metrics| {
                            (
                                metrics.display_size,
                                metrics.framebuffer_scale,
                                metrics.delta_time,
                            )
                        },
                    );

                    let mut prepare = imgui::FramePrepareOptions::new(display_size, delta_time)
                        .framebuffer_scale(framebuffer_scale);
                    if renderer_consumer.is_some() {
                        prepare = prepare.renderer_has_textures();
                    } else if !context.font_atlas().is_built() && !context.font_atlas().build() {
                        return Err(ImguiContextError::FontAtlasBuildFailed { context_id });
                    }
                    context.prepare_frame(prepare);
                    let context_raw = context.as_raw();
                    let ui = context.frame();

                    let schedule_capability =
                        active_for_frame(&active, context_id, config.schedule(), frame_index, ui);
                    if is_primary {
                        world
                            .get_non_send_mut::<ImguiFrameState>()
                            .expect("primary frame state must be installed")
                            .begin(frame_index);
                    }

                    let schedule_found = try_run_context_schedule(world, config.schedule());
                    drop(schedule_capability);
                    if is_primary {
                        platform::sync_primary_window_platform_feedback(world, ui, context_raw);
                        world
                            .get_non_send_mut::<ImguiFrameState>()
                            .expect("primary frame state must be installed")
                            .end();
                    }
                    if !schedule_found {
                        let _ = context.end_frame();
                        return Err(ImguiContextError::MissingSchedule {
                            context_id,
                            schedule: config.schedule(),
                        });
                    }

                    #[cfg(feature = "render")]
                    {
                        if let Some(consumer) = renderer_consumer {
                            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                            let snapshot = if config.multi_viewport() {
                                context.render_platform_viewport_snapshot(consumer)
                            } else {
                                context.render_snapshot(consumer)
                            };
                            #[cfg(not(all(
                                feature = "multi-viewport",
                                not(target_arch = "wasm32")
                            )))]
                            let snapshot = context.render_snapshot(consumer);
                            return Ok(PendingFrameOutput::Snapshot(snapshot));
                        }
                    }

                    let _ = context.render();
                    Ok(PendingFrameOutput::Rendered)
                },
            )
        }));

        if Some(context_id) == primary_id {
            world
                .get_non_send_mut::<ImguiFrameState>()
                .expect("primary frame state must be installed")
                .end();
        }
        active.revoke();

        let (completed_frame, context_error, panic_payload) = match result {
            Ok(Ok(output)) => {
                let finalized = panic::catch_unwind(AssertUnwindSafe(|| {
                    finish_frame_output(world, is_primary, frame_index, output);
                }));
                match finalized {
                    Ok(()) => (Some(frame_index), None, None),
                    Err(payload) => (None, None, Some(payload)),
                }
            }
            Ok(Err(ImguiActiveRendererContextError::Operation(error))) => {
                if is_primary {
                    clear_primary_output(world);
                }
                (None, Some(error), None)
            }
            #[cfg(feature = "render")]
            Ok(Err(ImguiActiveRendererContextError::RendererOwnership(source))) => {
                if is_primary {
                    clear_primary_output(world);
                }
                (
                    None,
                    Some(ImguiContextError::RendererOwnership { context_id, source }),
                    None,
                )
            }
            Err(payload) => {
                if is_primary {
                    clear_primary_output(world);
                }
                (None, None, Some(payload))
            }
        };
        world
            .get_non_send_mut::<ImguiContexts>()
            .expect("ImguiPlugin must retain the Context registry")
            .finish_drive(context_id, owner, completed_frame, context_error);

        if let Some(payload) = panic_payload {
            panic::resume_unwind(payload);
        }
    }
}

fn try_run_context_schedule(world: &mut World, label: InternedScheduleLabel) -> bool {
    let Some(mut schedule) = world
        .get_resource_mut::<Schedules>()
        .and_then(|mut schedules| schedules.remove_temporarily(label))
    else {
        return false;
    };

    // Bevy's schedule scope only reinserts after a normal return, so preserve this nested
    // Context schedule before propagating a user-system panic.
    let result = panic::catch_unwind(AssertUnwindSafe(|| schedule.run(world)));
    let displaced = world.resource_mut::<Schedules>().reinsert(schedule);
    debug_assert!(
        displaced.is_none(),
        "a Context UI schedule was replaced while it was running"
    );
    if let Err(payload) = result {
        panic::resume_unwind(payload);
    }
    true
}

#[cfg(feature = "render")]
fn poll_primary_completions_fail_closed(world: &mut World, context_id: imgui::ContextId) {
    let result = world
        .get_non_send_mut::<ImguiContexts>()
        .expect("ImguiPlugin must retain the Context registry")
        .configure(context_id, |context| context.poll_snapshot_completions());
    match result {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => {
            panic!(
                "primary Dear ImGui snapshot completion failed while frames were paused: {error}"
            )
        }
        Err(
            ImguiContextError::UnknownContext { .. } | ImguiContextError::TeardownInProgress { .. },
        ) => {}
        Err(error) => {
            panic!("primary Dear ImGui completion polling was rejected: {error}")
        }
    }
}

fn clear_primary_output(world: &mut World) {
    #[cfg(feature = "render")]
    world.resource::<ImguiFrameMailbox>().clear();
    world.resource_mut::<ImguiFrameOutput>().clear_snapshot();
}

fn finish_frame_output(
    world: &mut World,
    is_primary: bool,
    frame_index: u64,
    output: PendingFrameOutput,
) {
    match output {
        #[cfg(feature = "render")]
        PendingFrameOutput::Snapshot(snapshot) => {
            if is_primary {
                let mailbox = world.resource::<ImguiFrameMailbox>().clone();
                let release = world
                    .resource::<crate::render::ImguiRendererRelease>()
                    .clone();
                world.resource_mut::<ImguiFrameOutput>().set_snapshot(
                    &mailbox,
                    &release,
                    frame_index,
                    snapshot,
                );
            } else {
                drop(snapshot);
            }
        }
        PendingFrameOutput::Rendered => {
            if is_primary {
                world
                    .resource_mut::<ImguiFrameOutput>()
                    .complete_without_snapshot(frame_index);
            }
        }
    }
}

fn active_for_frame(
    active: &ActiveUiCapability,
    context_id: imgui::ContextId,
    schedule: bevy_ecs::schedule::InternedScheduleLabel,
    frame_index: u64,
    ui: &imgui::Ui,
) -> ActiveUiCapability {
    let scoped = active.clone();
    scoped.install(context_id, schedule, frame_index, ui);
    scoped
}

fn primary_frame_metrics(world: &mut World) -> Option<PrimaryFrameMetrics> {
    let delta_time = world
        .get_resource::<Time<Real>>()
        .map(Time::delta_secs)
        .unwrap_or(1.0 / 60.0)
        .max(f32::EPSILON);
    let mut query = world.query_filtered::<&Window, With<PrimaryWindow>>();
    let window = query.single(world).ok()?;
    Some(PrimaryFrameMetrics {
        display_size: finite_display_size([window.width(), window.height()]),
        framebuffer_scale: finite_framebuffer_scale([
            window.scale_factor() as f32,
            window.scale_factor() as f32,
        ]),
        delta_time,
    })
}

fn finite_display_size(size: [f32; 2]) -> [f32; 2] {
    size.map(|value| {
        if value.is_finite() && value > 0.0 {
            value
        } else {
            1.0
        }
    })
}

fn finite_framebuffer_scale(scale: [f32; 2]) -> [f32; 2] {
    scale.map(|value| {
        if value.is_finite() && value > 0.0 {
            value
        } else {
            1.0
        }
    })
}

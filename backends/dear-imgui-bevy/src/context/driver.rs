use std::{
    collections::HashSet,
    panic::{self, AssertUnwindSafe},
};

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
#[cfg(feature = "render")]
use crate::input::ImguiContextInputMetrics;

pub(crate) fn install_context_lifecycle(app: &mut App) {
    app.insert_non_send(ImguiActiveUi::default());
    app.insert_non_send(ImguiFrameState::default());
    app.init_resource::<ImguiFrameOutput>();
    #[cfg(feature = "render")]
    app.init_resource::<ImguiFrameMailbox>()
        .init_resource::<platform::ImguiPlatformImeFeedback>();
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
    let live_contexts = order.iter().copied().collect::<HashSet<_>>();
    world
        .resource_mut::<ImguiFrameOutput>()
        .retain_contexts(|context_id| live_contexts.contains(&context_id));
    let primary_id = world
        .get_non_send::<ImguiContexts>()
        .and_then(ImguiContexts::primary_id);
    let primary_metrics = primary_frame_metrics(world);
    #[cfg(feature = "render")]
    let routed_metrics = world
        .get_resource::<ImguiContextInputMetrics>()
        .cloned()
        .unwrap_or_default();
    let active = world
        .get_non_send::<ImguiActiveUi>()
        .expect("ImguiPlugin must install the active UI capability")
        .capability();
    #[cfg(feature = "render")]
    platform::begin_platform_ime_feedback(world);

    for context_id in order {
        let is_primary = Some(context_id) == primary_id;
        #[cfg(feature = "render")]
        if world
            .resource::<crate::render::ImguiRendererReleases>()
            .recovery_requested(context_id)
        {
            world
                .get_non_send_mut::<ImguiContexts>()
                .expect("ImguiPlugin must retain the Context registry")
                .recover_renderer(context_id)
                .unwrap_or_else(|error| {
                    panic!(
                        "Dear ImGui renderer recovery failed for Context {context_id:?}: {error}"
                    )
                });
        }
        #[cfg(feature = "render")]
        if world
            .resource::<crate::render::ImguiRendererReleases>()
            .release_requested(context_id)
        {
            poll_context_completions_fail_closed(world, context_id);
            clear_context_output(world, context_id);
            continue;
        }
        #[cfg(feature = "render")]
        let has_routed_metrics = routed_metrics.get(context_id).is_some();
        #[cfg(not(feature = "render"))]
        let has_routed_metrics = false;
        if is_primary && primary_metrics.is_none() && !has_routed_metrics {
            #[cfg(feature = "render")]
            poll_context_completions_fail_closed(world, context_id);
            clear_context_output(world, context_id);
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

        let primary_metrics_for_context = (Some(context_id) == primary_id)
            .then_some(primary_metrics.as_ref())
            .flatten();
        #[cfg(feature = "render")]
        let routed_metrics_for_context = routed_metrics.get(context_id);
        #[cfg(feature = "render")]
        let snapshot_mailbox = world.resource::<ImguiFrameMailbox>().clone();
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            owner.try_with_active_renderer_context_checked(
                config.multi_viewport(),
                |context, renderer_consumer| {
                    if is_primary {
                        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                        platform::prepare_primary_platform_frame(world, context);
                    }

                    let (display_size, framebuffer_scale, delta_time) = {
                        #[cfg(feature = "render")]
                        if let Some(metrics) = routed_metrics_for_context {
                            (
                                metrics.display_size,
                                metrics.framebuffer_scale,
                                primary_metrics_for_context.map_or_else(
                                    || context.io().delta_time().max(f32::EPSILON),
                                    |metrics| metrics.delta_time,
                                ),
                            )
                        } else {
                            primary_metrics_for_context.map_or_else(
                                || {
                                    (
                                        finite_display_size(context.io().display_size()),
                                        finite_framebuffer_scale(
                                            context.io().display_framebuffer_scale(),
                                        ),
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
                            )
                        }
                        #[cfg(not(feature = "render"))]
                        primary_metrics_for_context.map_or_else(
                            || {
                                (
                                    finite_display_size(context.io().display_size()),
                                    finite_framebuffer_scale(
                                        context.io().display_framebuffer_scale(),
                                    ),
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
                        )
                    };

                    #[cfg(feature = "render")]
                    if renderer_consumer.is_some() {
                        let progress = context.poll_snapshot_completions().unwrap_or_else(|error| {
                            panic!(
                                "Context {context_id:?} rejected Bevy renderer completion: {error}"
                            )
                        });
                        snapshot_mailbox
                            .update_completion_watermark(context_id, progress.watermark());
                    }

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
                    world
                        .get_non_send_mut::<ImguiFrameState>()
                        .expect("active frame state must be installed")
                        .begin(context_id, frame_index);

                    let schedule_found = try_run_context_schedule(world, config.schedule());
                    drop(schedule_capability);
                    #[cfg(feature = "render")]
                    platform::record_context_platform_ime_feedback(
                        world,
                        context_id,
                        is_primary,
                        context_raw,
                    );
                    if is_primary {
                        platform::sync_primary_window_platform_feedback(world, ui);
                        #[cfg(not(feature = "render"))]
                        platform::sync_primary_window_ime_feedback(world, context_raw);
                    }
                    world
                        .get_non_send_mut::<ImguiFrameState>()
                        .expect("active frame state must be installed")
                        .end();
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

        world
            .get_non_send_mut::<ImguiFrameState>()
            .expect("active frame state must be installed")
            .end();
        active.revoke();

        let (completed_frame, context_error, panic_payload) = match result {
            Ok(Ok(output)) => {
                let finalized = panic::catch_unwind(AssertUnwindSafe(|| {
                    finish_frame_output(
                        world,
                        context_id,
                        frame_index,
                        config.multi_viewport(),
                        output,
                    );
                }));
                match finalized {
                    Ok(()) => (Some(frame_index), None, None),
                    Err(payload) => (None, None, Some(payload)),
                }
            }
            Ok(Err(ImguiActiveRendererContextError::Operation(error))) => {
                clear_context_output(world, context_id);
                (None, Some(error), None)
            }
            #[cfg(feature = "render")]
            Ok(Err(ImguiActiveRendererContextError::RendererOwnership(source))) => {
                clear_context_output(world, context_id);
                (
                    None,
                    Some(ImguiContextError::RendererOwnership { context_id, source }),
                    None,
                )
            }
            Err(payload) => {
                clear_context_output(world, context_id);
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
    #[cfg(feature = "render")]
    platform::finish_platform_ime_feedback(world);
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
fn poll_context_completions_fail_closed(world: &mut World, context_id: imgui::ContextId) {
    let result = world
        .get_non_send_mut::<ImguiContexts>()
        .expect("ImguiPlugin must retain the Context registry")
        .configure(context_id, |context| context.poll_snapshot_completions());
    match result {
        Ok(Ok(progress)) => world
            .resource::<ImguiFrameMailbox>()
            .update_completion_watermark(context_id, progress.watermark()),
        Ok(Err(error)) => {
            panic!(
                "Dear ImGui snapshot completion for Context {context_id:?} failed while frames were paused: {error}"
            )
        }
        Err(
            ImguiContextError::UnknownContext { .. } | ImguiContextError::TeardownInProgress { .. },
        ) => {}
        Err(error) => {
            panic!("Dear ImGui completion polling for Context {context_id:?} was rejected: {error}")
        }
    }
}

fn clear_context_output(world: &mut World, context_id: imgui::ContextId) {
    #[cfg(feature = "render")]
    world.resource::<ImguiFrameMailbox>().clear(context_id);
    world
        .resource_mut::<ImguiFrameOutput>()
        .clear_snapshot(context_id);
}

fn finish_frame_output(
    world: &mut World,
    context_id: imgui::ContextId,
    frame_index: u64,
    _include_platform_viewports: bool,
    output: PendingFrameOutput,
) {
    match output {
        #[cfg(feature = "render")]
        PendingFrameOutput::Snapshot(snapshot) => {
            let mailbox = world.resource::<ImguiFrameMailbox>().clone();
            let releases = world
                .resource::<crate::render::ImguiRendererReleases>()
                .clone();
            world.resource_mut::<ImguiFrameOutput>().set_snapshot(
                context_id,
                &mailbox,
                &releases,
                frame_index,
                _include_platform_viewports,
                snapshot,
            );
        }
        PendingFrameOutput::Rendered => {
            world
                .resource_mut::<ImguiFrameOutput>()
                .complete_without_snapshot(context_id, frame_index);
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
        framebuffer_scale: finite_framebuffer_scale([window.scale_factor(), window.scale_factor()]),
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

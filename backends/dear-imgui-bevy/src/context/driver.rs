#[cfg(feature = "render")]
use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe};

use bevy_app::App;
use bevy_ecs::prelude::{Entity, With, World};
use bevy_time::{Real, Time};
use bevy_window::{PrimaryWindow, Window};
use dear_imgui_rs as imgui;

#[cfg(feature = "render")]
use super::ImguiFrameMailbox;
use super::platform;
use super::{ImguiActiveRendererContextError, ImguiContextError, ImguiContexts};
#[cfg(feature = "render")]
use crate::input::{ImguiContextInputMetrics, ImguiInputFrameMetrics};

pub(crate) fn install_context_lifecycle(app: &mut App) {
    #[cfg(feature = "render")]
    app.init_resource::<ImguiFrameMailbox>()
        .init_resource::<platform::ImguiPlatformFeedback>();
    super::ownership::install_context_retirements(app);
}

struct PrimaryFrameMetrics {
    host_window: Entity,
    display_size: [f32; 2],
    framebuffer_scale: [f32; 2],
}

#[cfg(feature = "render")]
#[derive(Clone, Copy)]
struct RoutedFrameMetrics {
    display_size: [f32; 2],
    framebuffer_scale: [f32; 2],
}

#[cfg(feature = "render")]
impl From<ImguiInputFrameMetrics> for RoutedFrameMetrics {
    fn from(metrics: ImguiInputFrameMetrics) -> Self {
        Self {
            display_size: metrics.display_size,
            framebuffer_scale: metrics.framebuffer_scale,
        }
    }
}

enum PendingFrameOutput {
    #[cfg(feature = "render")]
    Snapshot(imgui::render::snapshot::FrameSnapshot),
    Rendered,
}

/// Serially activate, frame, run, render, and suspend every registered Context.
pub(crate) fn drive_imgui_contexts(world: &mut World) {
    #[cfg(feature = "render")]
    drain_snapshot_commit_errors(world);
    let order = world
        .get_non_send::<ImguiContexts>()
        .map(ImguiContexts::drive_order)
        .unwrap_or_default();
    #[cfg(feature = "render")]
    let routed_input_metrics = world
        .get_resource::<ImguiContextInputMetrics>()
        .cloned()
        .unwrap_or_default();
    #[cfg(feature = "render")]
    let render_route_epoch = world
        .get_resource::<crate::route::ImguiResolvedRoutes>()
        .map(crate::route::ImguiResolvedRoutes::render_epoch)
        .unwrap_or_default();
    #[cfg(feature = "render")]
    if routed_input_metrics.epoch() != render_route_epoch.epoch() {
        return;
    }
    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
    {
        let native_viewport_contexts = world
            .get_non_send::<ImguiContexts>()
            .map(ImguiContexts::native_viewport_context_ids)
            .unwrap_or_default();
        world
            .resource_mut::<crate::ImguiNativeViewportSupport>()
            .begin_frame(native_viewport_contexts);
    }
    let primary_id = world
        .get_non_send::<ImguiContexts>()
        .and_then(ImguiContexts::primary_id);
    let delta_time = frame_delta_time(world);
    let primary_metrics = primary_frame_metrics(world);
    #[cfg(feature = "render")]
    let (routed_render_metrics, routed_platform_hosts) = {
        let metrics = render_route_epoch
            .render_routes()
            .iter()
            .filter(|route| {
                route
                    .host_window()
                    .is_none_or(|host_window| world.get::<Window>(host_window).is_some())
            })
            .map(|route| {
                let framebuffer_scale = finite_framebuffer_scale([
                    route.target_info().scale_factor,
                    route.target_info().scale_factor,
                ]);
                let physical_size = route.physical_output_size();
                (
                    route.context_id(),
                    RoutedFrameMetrics {
                        display_size: finite_display_size([
                            physical_size.x as f32 / framebuffer_scale[0],
                            physical_size.y as f32 / framebuffer_scale[1],
                        ]),
                        framebuffer_scale,
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        let hosts = render_route_epoch
            .render_routes()
            .iter()
            .filter(|route| {
                route
                    .host_window()
                    .is_none_or(|host_window| world.get::<Window>(host_window).is_some())
            })
            .filter_map(|route| {
                route
                    .host_window()
                    .map(|host_window| (route.context_id(), host_window))
            })
            .collect::<HashMap<_, _>>();
        (metrics, hosts)
    };
    #[cfg(feature = "render")]
    platform::begin_platform_feedback(world);

    for context_id in order {
        let is_primary = Some(context_id) == primary_id;
        #[cfg(feature = "render")]
        let context_tearing_down = world
            .get_non_send::<ImguiContexts>()
            .is_some_and(|contexts| contexts.is_tearing_down(context_id));
        #[cfg(feature = "render")]
        // Teardown owns any recovery-detached renderer state and converts it into release.
        if !context_tearing_down
            && world
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
            poll_context_completions_or_quarantine(world, context_id);
            clear_context_output(world, context_id);
            continue;
        }
        #[cfg(feature = "render")]
        let has_routed_metrics = routed_render_metrics.contains_key(&context_id)
            || routed_input_metrics
                .get(context_id)
                .is_some_and(|metrics| world.get::<Window>(metrics.host_window).is_some());
        #[cfg(not(feature = "render"))]
        let has_routed_metrics = false;
        if is_primary && primary_metrics.is_none() && !has_routed_metrics {
            #[cfg(feature = "render")]
            poll_context_completions_or_quarantine(world, context_id);
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
        let routed_input_metrics_for_context = routed_input_metrics
            .get(context_id)
            .filter(|metrics| world.get::<Window>(metrics.host_window).is_some());
        #[cfg(feature = "render")]
        let routed_metrics_for_context = routed_render_metrics
            .get(&context_id)
            .copied()
            .or_else(|| routed_input_metrics_for_context.map(RoutedFrameMetrics::from));
        #[cfg(feature = "render")]
        let platform_host = routed_platform_hosts
            .get(&context_id)
            .copied()
            .or_else(|| routed_input_metrics_for_context.map(|metrics| metrics.host_window))
            .or_else(|| primary_metrics_for_context.map(|metrics| metrics.host_window));
        #[cfg(not(feature = "render"))]
        let platform_host = primary_metrics_for_context.map(|metrics| metrics.host_window);
        #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
        let native_platform_metrics = config
            .multi_viewport()
            .then(|| {
                platform_host
                    .and_then(|host_window| world.get::<Window>(host_window))
                    .map(crate::viewport::desktop_metrics_for_window)
            })
            .flatten();
        #[cfg(not(all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
        let native_platform_metrics: Option<([f32; 2], [f32; 2])> = None;
        #[cfg(feature = "render")]
        let snapshot_mailbox = world.resource::<ImguiFrameMailbox>().clone();
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            owner.try_with_active_renderer_context_checked(
                config.multi_viewport(),
                |context, renderer_consumer| {
                    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                    if config.multi_viewport() {
                        let host_window = platform_host
                            .ok_or(ImguiContextError::PlatformHostUnavailable { context_id })?;
                        let prepared = platform::prepare_context_platform_frame(
                            world,
                            context_id,
                            context,
                            host_window,
                        )
                        .map_err(|source| {
                            ImguiContextError::ViewportBridge { context_id, source }
                        })?;
                        if !prepared {
                            return Err(ImguiContextError::PlatformHostUnavailable { context_id });
                        }
                    }

                    let (display_size, framebuffer_scale, delta_time) = {
                        if let Some((display_size, framebuffer_scale)) = native_platform_metrics {
                            (
                                finite_display_size(display_size),
                                finite_framebuffer_scale(framebuffer_scale),
                                delta_time,
                            )
                        } else {
                            #[cfg(feature = "render")]
                            if let Some(metrics) = routed_metrics_for_context {
                                (metrics.display_size, metrics.framebuffer_scale, delta_time)
                            } else {
                                primary_metrics_for_context.map_or_else(
                                    || {
                                        (
                                            finite_display_size(context.io().display_size()),
                                            finite_framebuffer_scale(
                                                context.io().display_framebuffer_scale(),
                                            ),
                                            delta_time,
                                        )
                                    },
                                    |metrics| {
                                        (
                                            metrics.display_size,
                                            metrics.framebuffer_scale,
                                            delta_time,
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
                                        delta_time,
                                    )
                                },
                                |metrics| {
                                    (metrics.display_size, metrics.framebuffer_scale, delta_time)
                                },
                            )
                        }
                    };

                    #[cfg(feature = "render")]
                    if renderer_consumer.is_some() {
                        let progress = context.poll_snapshot_completions().map_err(|source| {
                            ImguiContextError::RendererCompletion { context_id, source }
                        })?;
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

                    super::run_pass(world, config.pass(), context_id, frame_index, ui);
                    #[cfg(feature = "render")]
                    platform::record_context_platform_ime_feedback(
                        world,
                        context_id,
                        platform_host,
                        context_raw,
                    );
                    #[cfg(feature = "render")]
                    platform::sync_context_platform_feedback(world, context_id, platform_host, ui);
                    #[cfg(not(feature = "render"))]
                    if is_primary {
                        platform::sync_context_platform_feedback(
                            world,
                            context_id,
                            platform_host,
                            ui,
                        );
                        platform::sync_primary_window_ime_feedback(world, context_id, context_raw);
                    }
                    #[cfg(feature = "render")]
                    {
                        if let Some(consumer) = renderer_consumer {
                            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                            let snapshot = if config.multi_viewport() {
                                let snapshot = context.render_platform_viewport_snapshot(consumer);
                                context.update_platform_windows();
                                snapshot
                            } else {
                                context.render_snapshot(consumer)
                            };
                            #[cfg(not(all(
                                feature = "multi-viewport",
                                not(target_arch = "wasm32")
                            )))]
                            let snapshot = context.render_snapshot(consumer);
                            let snapshot = snapshot.map_err(|source| {
                                ImguiContextError::SnapshotCapture { context_id, source }
                            })?;
                            return Ok(PendingFrameOutput::Snapshot(snapshot));
                        }
                    }

                    drop(context.render_legacy());
                    #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
                    if config.multi_viewport() {
                        context.update_platform_windows();
                    }
                    Ok(PendingFrameOutput::Rendered)
                },
            )
        }));

        let (completed_frame, context_error, panic_payload) = match result {
            Ok(Ok(output)) => {
                let finalized = panic::catch_unwind(AssertUnwindSafe(|| {
                    finish_frame_output(
                        world,
                        context_id,
                        frame_index,
                        config.multi_viewport(),
                        #[cfg(feature = "render")]
                        render_route_epoch.clone(),
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
            #[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
            Ok(Err(ImguiActiveRendererContextError::ViewportBridge(source))) => {
                clear_context_output(world, context_id);
                (
                    None,
                    Some(ImguiContextError::ViewportBridge { context_id, source }),
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
    platform::finish_platform_feedback(world);
}

#[cfg(feature = "render")]
fn drain_snapshot_commit_errors(world: &mut World) {
    let errors = world
        .resource::<ImguiFrameMailbox>()
        .take_snapshot_commit_errors();
    if errors.is_empty() {
        return;
    }
    let Some(mut contexts) = world.get_non_send_mut::<ImguiContexts>() else {
        return;
    };
    for (context_id, source) in errors {
        contexts.record_snapshot_commit_error(context_id, source);
    }
}

#[cfg(feature = "render")]
fn poll_context_completions_or_quarantine(world: &mut World, context_id: imgui::ContextId) {
    let result = world
        .get_non_send_mut::<ImguiContexts>()
        .expect("ImguiPlugin must retain the Context registry")
        .configure(context_id, |context| context.poll_snapshot_completions());
    match result {
        Ok(Ok(progress)) => world
            .resource::<ImguiFrameMailbox>()
            .update_completion_watermark(context_id, progress.watermark()),
        Ok(Err(source)) => world
            .get_non_send_mut::<ImguiContexts>()
            .expect("ImguiPlugin must retain the Context registry")
            .record_renderer_completion_error(context_id, source),
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
    #[cfg(not(feature = "render"))]
    let _ = (world, context_id);
}

fn finish_frame_output(
    world: &mut World,
    context_id: imgui::ContextId,
    frame_index: u64,
    include_platform_viewports: bool,
    #[cfg(feature = "render")] render_routes: crate::route::ImguiRenderRouteEpoch,
    output: PendingFrameOutput,
) {
    #[cfg(not(feature = "render"))]
    let _ = (world, context_id, frame_index, include_platform_viewports);
    #[cfg(all(feature = "render", not(test)))]
    let _ = frame_index;
    match output {
        #[cfg(feature = "render")]
        PendingFrameOutput::Snapshot(snapshot) => {
            let mailbox = world.resource::<ImguiFrameMailbox>().clone();
            if world
                .resource::<crate::render::ImguiRendererReleases>()
                .release_requested(context_id)
            {
                mailbox.clear(context_id);
                return;
            }
            debug_assert_eq!(snapshot.epoch().context_id(), context_id);
            mailbox.publish(
                context_id,
                super::PendingFrame {
                    #[cfg(test)]
                    frame_index,
                    include_platform_viewports,
                    render_routes,
                    snapshot,
                },
            );
        }
        PendingFrameOutput::Rendered => {}
    }
}

fn primary_frame_metrics(world: &mut World) -> Option<PrimaryFrameMetrics> {
    let mut query = world.query_filtered::<(Entity, &Window), With<PrimaryWindow>>();
    let (host_window, window) = query.single(world).ok()?;
    Some(PrimaryFrameMetrics {
        host_window,
        display_size: finite_display_size([window.width(), window.height()]),
        framebuffer_scale: finite_framebuffer_scale([window.scale_factor(), window.scale_factor()]),
    })
}

fn frame_delta_time(world: &World) -> f32 {
    world
        .get_resource::<Time<Real>>()
        .map(Time::delta_secs)
        .unwrap_or(1.0 / 60.0)
        .max(f32::EPSILON)
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

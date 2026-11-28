//! Native secondary targets share the platform and renderer's existing owning transaction.

use dear_imgui_rs::FrameToken;
use dear_imgui_wgpu::multi_viewport::{
    WgpuPreparedMainFrame, WgpuViewportFrameReport, WinitViewportRoute,
};
use dear_imgui_wgpu::{FramebufferExtent, WgpuFrameStage, WgpuViewportObservation};
use winit::event_loop::ActiveEventLoop;

use super::{
    admission::{SurfaceAdmission, SurfaceDispatch, SurfaceSkipReason, next_frame_index},
    ownership::RuntimeOwnership,
    renderer::AppRenderer,
    surface::{
        FrameDriver, FrameRecordingOptions, MainFrameOutput, MainFrameTarget,
        RecordedApplicationFrame, RuntimeSurfaceAdmission, drive_frame, record_application_frame,
    },
};
use crate::{AppConfig, Application, ApplicationStage, RunError, ViewportFrameContext};

struct MultiFrameDriver<'a> {
    route: &'a WinitViewportRoute,
    event_loop: &'a ActiveEventLoop,
    main: RuntimeSurfaceAdmission<'a>,
    output: MainFrameOutput<'a>,
    producer: Option<wgpu::CommandBuffer>,
    secondary: WgpuViewportFrameReport,
    main_observation: Option<WgpuViewportObservation>,
    skip: Option<SurfaceSkipReason>,
}

impl FrameDriver for MultiFrameDriver<'_> {
    type Prepared<'frame> = WgpuPreparedMainFrame<'frame>;

    #[cfg(feature = "test-engine")]
    const MAIN_CAN_SKIP: bool = true;

    fn prepare<'frame>(
        &mut self,
        frame: FrameToken<'frame>,
    ) -> Result<WgpuPreparedMainFrame<'frame>, RunError> {
        let prepared = self
            .route
            .prepare(self.event_loop, frame)
            .map_err(|error| {
                self.secondary = error.report().clone();
                RunError::ViewportPreparation(error)
            })?;
        let main = self
            .route
            .complete_secondary(prepared, self.producer.take())
            .map_err(|error| {
                self.secondary = error.report().clone();
                RunError::ViewportWork(error)
            })?;
        self.secondary = main.secondary_report().clone();
        Ok(main)
    }

    #[cfg(feature = "test-engine")]
    fn prepared_context_id(frame: &Self::Prepared<'_>) -> dear_imgui_rs::ContextId {
        frame.context_id()
    }

    fn render_main(&mut self, prepared: WgpuPreparedMainFrame<'_>) -> Result<bool, RunError> {
        let admitted = match self.main.admit()? {
            SurfaceAdmission::Skipped(reason) => {
                self.skip = Some(reason);
                drop(prepared);
                return Ok(false);
            }
            SurfaceAdmission::Admitted(admitted) => admitted,
        };
        let extent = FramebufferExtent::from_texture(&admitted.frame.texture);
        let encoder = self
            .output
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Dear App main UI consumer"),
            });
        let mut observation = self.output.record_and_submit(
            MainFrameTarget::Surface(admitted),
            encoder,
            "Dear App main viewport",
            |pass| {
                self.route
                    .record_main(prepared, pass, extent)
                    .map_err(|error| {
                        self.main_observation = error
                            .report()
                            .observations()
                            .iter()
                            .find(|entry| entry.target.is_main)
                            .cloned();
                        RunError::ViewportWork(error)
                    })
            },
        )?;
        observation.stage = observation.stage.max(WgpuFrameStage::Submitted);
        self.main_observation = Some(observation);
        Ok(true)
    }

    fn present(&mut self) -> Result<(), RunError> {
        self.output.present()?;
        if let Some(observation) = &mut self.main_observation {
            observation.stage = observation.stage.max(WgpuFrameStage::PresentationRequested);
        }
        Ok(())
    }
}

pub(super) fn render_frame<A: Application>(
    ownership: &mut RuntimeOwnership,
    clear_color: wgpu::Color,
    frame_count: &mut u64,
    application: &mut A,
    config: &AppConfig,
    wake: &crate::AppWakeHandle,
    event_loop: &ActiveEventLoop,
) -> Result<SurfaceDispatch<bool>, RunError> {
    let RuntimeOwnership {
        window,
        ui,
        generations,
    } = ownership;
    let (generation, lifecycle) = generations.render_parts();
    let generation = generation.ok_or_else(|| RunError::Recovery {
        message: "native frame requested without an active GPU generation".to_owned(),
    })?;
    let frame_index = next_frame_index(frame_count, "UI frame counter exhausted")?;
    let RecordedApplicationFrame {
        frame,
        exit_requested,
        encoder,
        ..
    } = record_application_frame(
        ui,
        generation,
        window,
        application,
        config,
        wake,
        FrameRecordingOptions {
            frame_index,
            target: None,
        },
    )?;
    let gpu = &generation.gpu;
    let AppRenderer::Multi(route) = &gpu.renderer else {
        return Err(RunError::Recovery {
            message: "native frame used a single-window renderer".to_owned(),
        });
    };
    let mut driver = MultiFrameDriver {
        route,
        event_loop,
        main: RuntimeSurfaceAdmission::new(window, lifecycle, Some(gpu), config),
        output: MainFrameOutput::new(&gpu.queue, &gpu.device, clear_color),
        producer: Some(encoder.finish()),
        secondary: WgpuViewportFrameReport::default(),
        main_observation: None,
        skip: None,
    };
    let result = drive_frame(application, frame, frame_index, &mut driver);
    let result = driver.output.after_render(driver.main.window(), result);
    // Always publish earlier secondary observations, including when main admission/drawing failed.
    let observed = application
        .viewport_frame(&ViewportFrameContext {
            secondary: &driver.secondary,
            main: driver.main_observation.as_ref(),
            generation: generation.id,
            frame_index,
        })
        .map_err(|error| error.during_application_stage(ApplicationStage::ViewportFrame));
    if let Err(primary) = result {
        if let Err(secondary) = observed {
            tracing::warn!(%primary, %secondary, "viewport observation hook failed after rendering failed");
        }
        return Err(primary);
    }
    observed?;
    Ok(settle_frame_dispatch(
        driver.skip,
        exit_requested,
        driver.secondary.observations(),
    ))
}

fn settle_frame_dispatch(
    main_skip: Option<SurfaceSkipReason>,
    exit_requested: bool,
    secondary: &[WgpuViewportObservation],
) -> SurfaceDispatch<bool> {
    let mut dispatch = match main_skip {
        Some(reason) if !exit_requested => SurfaceDispatch::skipped(reason),
        _ => SurfaceDispatch::presented(exit_requested),
    };
    dispatch.retry_needed = dispatch.retry_needed
        || secondary.iter().any(|observation| {
            matches!(
                observation.outcome,
                dear_imgui_wgpu::WgpuFrameOutcome::Skipped(
                    dear_imgui_wgpu::WgpuFrameSkipReason::Timeout
                        | dear_imgui_wgpu::WgpuFrameSkipReason::Lost
                        | dear_imgui_wgpu::WgpuFrameSkipReason::Outdated
                )
            )
        });
    dispatch
}

#[cfg(test)]
mod tests {
    use super::*;
    use dear_imgui_rs::{Context, NativeWindowInstanceId, NativeWindowTarget};
    use dear_imgui_wgpu::{WgpuFrameOutcome, WgpuFrameScope, WgpuFrameSkipReason};
    use std::time::{Duration, Instant};

    use crate::runtime::admission::{SurfacePresentation, SurfaceRedrawRetry};
    use crate::runtime::runner::earliest_deadline;

    #[test]
    fn wait_retries_secondary_transient_skips_after_main_presentation() {
        let _guard = crate::runtime::imgui_test_guard();
        let mut context = Context::create();
        let target = NativeWindowTarget {
            instance_id: NativeWindowInstanceId::allocate(context.id()).unwrap(),
            viewport_id: context.main_viewport().id(),
            is_main: false,
        };
        for reason in [
            WgpuFrameSkipReason::Timeout,
            WgpuFrameSkipReason::Lost,
            WgpuFrameSkipReason::Outdated,
        ] {
            let secondary = [WgpuViewportObservation {
                target,
                scope: WgpuFrameScope {
                    context_id: context.id(),
                    epoch: None,
                    frame_count: 1,
                },
                dependencies: Default::default(),
                stage: WgpuFrameStage::Unrecorded,
                outcome: WgpuFrameOutcome::Skipped(reason),
            }];
            let dispatch = settle_frame_dispatch(None, false, &secondary);
            assert_eq!(dispatch.main, SurfacePresentation::Presented(false));
            assert!(
                dispatch.retry_needed,
                "secondary {reason:?} needs another frame"
            );

            let now = Instant::now();
            let mut retry = SurfaceRedrawRetry::default();
            retry.settle(dispatch.retry_needed, now);
            let deadline = earliest_deadline(retry.deadline(), None)
                .expect("Wait must retain a wake without application deadlines or new input");
            assert_eq!(deadline, now + Duration::from_millis(16));
            assert!(!retry.take_due(now));
            assert!(retry.take_due(deadline));

            retry.settle(dispatch.retry_needed, deadline);
            assert_eq!(retry.deadline(), Some(deadline + Duration::from_millis(32)));

            let main_occluded =
                settle_frame_dispatch(Some(SurfaceSkipReason::Occluded), false, &secondary);
            assert_eq!(
                main_occluded.main,
                SurfacePresentation::Skipped(SurfaceSkipReason::Occluded)
            );
            assert!(main_occluded.retry_needed);

            let recovered = settle_frame_dispatch(None, false, &[]);
            assert_eq!(recovered.main, SurfacePresentation::Presented(false));
            retry.settle(recovered.retry_needed, deadline);
            assert!(retry.deadline().is_none());
        }

        for reason in [
            WgpuFrameSkipReason::Occluded,
            WgpuFrameSkipReason::ZeroSize,
            WgpuFrameSkipReason::RetiredTarget,
        ] {
            let secondary = [WgpuViewportObservation {
                target,
                scope: WgpuFrameScope {
                    context_id: context.id(),
                    epoch: None,
                    frame_count: 1,
                },
                dependencies: Default::default(),
                stage: WgpuFrameStage::Unrecorded,
                outcome: WgpuFrameOutcome::Skipped(reason),
            }];
            let dispatch = settle_frame_dispatch(None, false, &secondary);
            assert_eq!(dispatch.main, SurfacePresentation::Presented(false));
            assert!(
                !dispatch.retry_needed,
                "secondary {reason:?} waits for events or cleanup"
            );
        }
    }
}

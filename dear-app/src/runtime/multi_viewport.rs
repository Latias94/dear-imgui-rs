//! Native secondary targets share the platform and renderer's existing owning transaction.

use dear_imgui_rs::FrameToken;
use dear_imgui_wgpu::multi_viewport::{
    WgpuPreparedMainFrame, WgpuViewportFrameReport, WinitViewportRoute,
};
use dear_imgui_wgpu::{FramebufferExtent, WgpuFrameStage, WgpuViewportObservation};
use winit::event_loop::ActiveEventLoop;

use super::{
    admission::{
        SurfaceAcquisition, SurfaceAdmission, SurfaceAdmissionBackend, SurfaceDispatch,
        SurfaceSkipReason, admit_surface_frame,
    },
    lifecycle::{LifecycleAction, LifecycleMachine, SurfaceEvent},
    ownership::RuntimeOwnership,
    renderer::AppRenderer,
    state::WindowState,
    surface::build_application_frame,
};
use crate::{AppConfig, Application, ApplicationStage, RunError, ViewportFrameContext};

struct MainAdmission<'a> {
    window: &'a mut WindowState,
    lifecycle: &'a mut LifecycleMachine,
    adapter: &'a wgpu::Adapter,
    device: &'a wgpu::Device,
    config: &'a AppConfig,
}

impl SurfaceAdmissionBackend for MainAdmission<'_> {
    type Frame = wgpu::SurfaceTexture;
    fn acquire(&mut self) -> SurfaceAcquisition<Self::Frame> {
        let Some(surface) = self.window.surface.as_ref() else {
            return SurfaceAcquisition::Validation;
        };
        match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => SurfaceAcquisition::Success(frame),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => SurfaceAcquisition::Suboptimal(frame),
            wgpu::CurrentSurfaceTexture::Lost => SurfaceAcquisition::Lost,
            wgpu::CurrentSurfaceTexture::Outdated => SurfaceAcquisition::Outdated,
            wgpu::CurrentSurfaceTexture::Timeout => SurfaceAcquisition::Timeout,
            wgpu::CurrentSurfaceTexture::Occluded => SurfaceAcquisition::Occluded,
            wgpu::CurrentSurfaceTexture::Validation => SurfaceAcquisition::Validation,
        }
    }
    fn record_event(&mut self, event: SurfaceEvent) -> LifecycleAction {
        self.lifecycle.surface_event(event)
    }
    fn recover(&mut self, action: LifecycleAction) -> Result<(), RunError> {
        match action {
            LifecycleAction::RecreateSurface => {
                self.window
                    .recreate_surface(self.adapter, self.device, self.config)
            }
            LifecycleAction::ReconfigureSurface => {
                self.window.reconfigure(self.device);
                Ok(())
            }
            _ => Err(RunError::Recovery {
                message: format!("invalid native main-surface recovery action: {action:?}"),
            }),
        }
    }
}

struct MultiFrameDriver<'a> {
    route: &'a WinitViewportRoute,
    event_loop: &'a ActiveEventLoop,
    main: MainAdmission<'a>,
    queue: &'a wgpu::Queue,
    producer: Option<wgpu::CommandBuffer>,
    secondary: WgpuViewportFrameReport,
    main_observation: Option<WgpuViewportObservation>,
    surface_frame: Option<wgpu::SurfaceTexture>,
    reconfigure_after_present: bool,
    skip: Option<SurfaceSkipReason>,
    clear_color: wgpu::Color,
}

impl MultiFrameDriver<'_> {
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

    fn render_main(&mut self, prepared: WgpuPreparedMainFrame<'_>) -> Result<bool, RunError> {
        let size = self.main.window.window.inner_size();
        let admission = if size.width == 0 || size.height == 0 {
            SurfaceAdmission::Skipped(SurfaceSkipReason::Occluded)
        } else {
            admit_surface_frame(&mut self.main)?
        };
        let admitted = match admission {
            SurfaceAdmission::Skipped(reason) => {
                self.skip = Some(reason);
                drop(prepared);
                return Ok(false);
            }
            SurfaceAdmission::Admitted(admitted) => admitted,
        };
        let view = admitted.frame.texture.create_view(&Default::default());
        let extent = FramebufferExtent::from_texture(&admitted.frame.texture);
        let mut encoder =
            self.main
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Dear App main UI consumer"),
                });
        let mut observation = {
            let attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(self.clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Dear App main viewport"),
                color_attachments: &attachments,
                ..Default::default()
            });
            self.route
                .record_main(prepared, &mut pass, extent)
                .map_err(|error| {
                    self.main_observation = error
                        .report()
                        .observations()
                        .iter()
                        .find(|entry| entry.target.is_main)
                        .cloned();
                    RunError::ViewportWork(error)
                })?
        };
        self.queue.submit([encoder.finish()]);
        observation.stage = observation.stage.max(WgpuFrameStage::Submitted);
        self.main_observation = Some(observation);
        self.reconfigure_after_present = admitted.reconfigure_after_present;
        self.surface_frame = Some(admitted.frame);
        Ok(true)
    }

    fn present(&mut self) -> Result<(), RunError> {
        let frame = self
            .surface_frame
            .take()
            .ok_or_else(|| RunError::Recovery {
                message: "native main frame was not recorded or was already presented".to_owned(),
            })?;
        self.queue.present(frame);
        if let Some(observation) = &mut self.main_observation {
            observation.stage = observation.stage.max(WgpuFrameStage::PresentationRequested);
        }
        if self.reconfigure_after_present {
            self.main.window.reconfigure(self.main.device);
        }
        Ok(())
    }
}

#[cfg(feature = "test-engine")]
impl dear_imgui_test_engine::TestFrameDriver for MultiFrameDriver<'_> {
    type PreparedFrame<'frame> = WgpuPreparedMainFrame<'frame>;
    type PrepareError = RunError;
    type RenderError = RunError;
    type PresentError = RunError;
    fn prepare<'frame>(
        &mut self,
        frame: FrameToken<'frame>,
        _: u64,
    ) -> Result<Self::PreparedFrame<'frame>, RunError> {
        self.prepare(frame)
    }
    fn prepared_context_id(frame: &Self::PreparedFrame<'_>) -> dear_imgui_rs::ContextId {
        frame.context_id()
    }
    fn render_main(
        &mut self,
        frame: Self::PreparedFrame<'_>,
        _: u64,
    ) -> Result<dear_imgui_test_engine::MainRenderOutcome, RunError> {
        Ok(if self.render_main(frame)? {
            dear_imgui_test_engine::MainRenderOutcome::ReadyToPresent
        } else {
            dear_imgui_test_engine::MainRenderOutcome::Skipped
        })
    }
    fn present(&mut self, _: u64) -> Result<(), RunError> {
        self.present()
    }
}

fn drive_frame<A: Application>(
    application: &mut A,
    frame: FrameToken<'_>,
    frame_index: u64,
    driver: &mut MultiFrameDriver<'_>,
) -> Result<(), RunError> {
    #[cfg(feature = "test-engine")]
    if let Some(engine) = application.test_engine() {
        engine
            .drive_frame(frame, frame_index, driver)
            .map_err(|source| RunError::TestEngineFrame {
                frame: frame_index,
                source: Box::new(source),
            })?;
        return Ok(());
    }
    #[cfg(not(feature = "test-engine"))]
    let _ = (application, frame_index);
    let prepared = driver.prepare(frame)?;
    if driver.render_main(prepared)? {
        driver.present()?;
    }
    Ok(())
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
    let (generation, lifecycle) = generations
        .render_parts()
        .ok_or_else(|| RunError::Recovery {
            message: "native frame requested without an active GPU generation".to_owned(),
        })?;
    let frame_index = frame_count
        .checked_add(1)
        .ok_or_else(|| RunError::Recovery {
            message: "UI frame counter exhausted".to_owned(),
        })?;
    *frame_count = frame_index;
    let (frame, exit_requested) =
        build_application_frame(ui, generation, window, application, config, wake)?;
    let gpu = &mut generation.gpu;
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Dear App engine producer"),
        });
    let size = window.window.inner_size();
    application
        .record_render(&mut crate::RenderContext {
            device: &gpu.device,
            queue: &gpu.queue,
            encoder: &mut encoder,
            window: &window.window,
            generation: generation.id,
            frame_index,
            framebuffer_extent: FramebufferExtent::new(size.width, size.height),
            surface_format: window.surface_config()?.format,
            wake,
        })
        .map_err(|error| error.during_application_stage(ApplicationStage::RecordRender))?;
    let AppRenderer::Multi(route) = &gpu.renderer else {
        return Err(RunError::Recovery {
            message: "native frame used a single-window renderer".to_owned(),
        });
    };
    let mut driver = MultiFrameDriver {
        route,
        event_loop,
        main: MainAdmission {
            window,
            lifecycle,
            adapter: &gpu.adapter,
            device: &gpu.device,
            config,
        },
        queue: &gpu.queue,
        producer: Some(encoder.finish()),
        secondary: WgpuViewportFrameReport::default(),
        main_observation: None,
        surface_frame: None,
        reconfigure_after_present: false,
        skip: None,
        clear_color,
    };
    let result = drive_frame(application, frame, frame_index, &mut driver);
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

//! WGPU surface admission, rendering, presentation, and retry settlement.

use dear_imgui_rs::render::ReconciledFrame;
use dear_imgui_rs::{DockNodeFlags, FrameToken, WindowFlags};
#[cfg(feature = "test-engine")]
use dear_imgui_test_engine::{FrameDriveOutcome, MainRenderOutcome, TestFrameDriver};
use dear_imgui_wgpu::FramebufferExtent;

use super::admission::{
    AdmittedSurfaceFrame, SurfaceAcquisition, SurfaceAdmission, SurfaceAdmissionBackend,
    SurfaceDispatch, admit_surface_frame, dispatch_surface_frame, settle_surface_presentation,
};
use super::lifecycle::{LifecycleAction, LifecycleMachine, SurfaceEvent};
use super::ownership::RuntimeOwnership;
use super::state::{GpuState, RuntimeGeneration, UiState, WindowState};
use crate::{
    AddOns, AppConfig, AppWakeHandle, Application, ApplicationStage, DockingApi, FrameContext,
    PrepareFrameContext, RunError,
};

pub(super) struct RuntimeSurfaceAdmission<'a> {
    window: &'a mut WindowState,
    lifecycle: &'a mut LifecycleMachine,
    gpu: Option<&'a GpuState>,
    config: &'a AppConfig,
}

impl<'a> RuntimeSurfaceAdmission<'a> {
    pub(super) fn new(
        window: &'a mut WindowState,
        lifecycle: &'a mut LifecycleMachine,
        gpu: Option<&'a GpuState>,
        config: &'a AppConfig,
    ) -> Self {
        Self {
            window,
            lifecycle,
            gpu,
            config,
        }
    }

    pub(super) fn admit(&mut self) -> Result<SurfaceAdmission<wgpu::SurfaceTexture>, RunError> {
        admit_surface_frame(self)
    }

    #[cfg(feature = "multi-viewport")]
    pub(super) fn window(&self) -> &WindowState {
        self.window
    }
}

impl SurfaceAdmissionBackend for RuntimeSurfaceAdmission<'_> {
    type Frame = wgpu::SurfaceTexture;

    fn window_size(&self) -> winit::dpi::PhysicalSize<u32> {
        self.window.window.inner_size()
    }

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
        let Some(gpu) = self.gpu else {
            return Err(RunError::Recovery {
                message: "surface recovery requested without an active GPU generation".to_owned(),
            });
        };
        gpu.check_surface_recovery()?;
        match action {
            LifecycleAction::RecreateSurface => {
                self.window
                    .recreate_surface(&gpu.adapter, &gpu.device, self.config)
            }
            LifecycleAction::ReconfigureSurface => {
                self.window.reconfigure(&gpu.device);
                Ok(())
            }
            _ => Err(RunError::Recovery {
                message: format!("surface admission requested invalid recovery action {action:?}"),
            }),
        }?;
        gpu.check_surface_recovery()
    }
}

pub(super) enum MainFrameTarget {
    Surface(AdmittedSurfaceFrame<wgpu::SurfaceTexture>),
    #[cfg(test)]
    Offscreen(wgpu::Texture),
}

impl MainFrameTarget {
    fn texture(&self) -> &wgpu::Texture {
        match self {
            Self::Surface(admitted) => &admitted.frame.texture,
            #[cfg(test)]
            Self::Offscreen(texture) => texture,
        }
    }

    /// Returns whether this presentation request requires a surface reconfiguration.
    fn present(self, queue: &wgpu::Queue) -> bool {
        match self {
            Self::Surface(admitted) => {
                queue.present(admitted.frame);
                admitted.reconfigure_after_present
            }
            #[cfg(test)]
            Self::Offscreen(_) => false,
        }
    }
}

/// Holds the submitted main target through the presentation protocol.
pub(super) struct MainFrameOutput<'a> {
    queue: &'a wgpu::Queue,
    pub(super) device: &'a wgpu::Device,
    clear_color: wgpu::Color,
    submitted: Option<MainFrameTarget>,
    reconfigure_after_present: bool,
}

impl<'a> MainFrameOutput<'a> {
    pub(super) fn new(
        queue: &'a wgpu::Queue,
        device: &'a wgpu::Device,
        clear_color: wgpu::Color,
    ) -> Self {
        Self {
            queue,
            device,
            clear_color,
            submitted: None,
            reconfigure_after_present: false,
        }
    }

    pub(super) fn record_and_submit<Output>(
        &mut self,
        target: MainFrameTarget,
        mut encoder: wgpu::CommandEncoder,
        pass_label: &'static str,
        render: impl FnOnce(&mut wgpu::RenderPass<'_>) -> Result<Output, RunError>,
    ) -> Result<Output, RunError> {
        let view = target
            .texture()
            .create_view(&wgpu::TextureViewDescriptor::default());
        let output = {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(pass_label),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                ..Default::default()
            });
            render(&mut pass)?
        };
        self.queue.submit(Some(encoder.finish()));
        self.submitted = Some(target);
        Ok(output)
    }

    pub(super) fn present(&mut self) -> Result<(), RunError> {
        let target = self.submitted.take().ok_or_else(|| RunError::Recovery {
            message: "main frame was not submitted or was already presented".to_owned(),
        })?;
        self.reconfigure_after_present = target.present(self.queue);
        Ok(())
    }

    pub(super) fn after_render(
        self,
        window: &WindowState,
        result: Result<(), RunError>,
    ) -> Result<(), RunError> {
        // A failed pre-swap leaves a submitted target to discard once the engine returns.
        drop(self.submitted);
        settle_surface_presentation(result, self.reconfigure_after_present, || {
            window.reconfigure(self.device);
        })
    }
}

struct AdmittedWgpuFrameDriver<'a> {
    renderer: &'a mut dear_imgui_wgpu::WgpuRenderer,
    output: MainFrameOutput<'a>,
    recording: Option<(wgpu::CommandEncoder, MainFrameTarget)>,
    extent: FramebufferExtent,
}

/// Drives the same frame phases with each renderer's own prepared-frame capability.
pub(super) trait FrameDriver {
    type Prepared<'frame>;

    #[cfg(feature = "test-engine")]
    const MAIN_CAN_SKIP: bool;

    fn prepare<'frame>(
        &mut self,
        frame: FrameToken<'frame>,
    ) -> Result<Self::Prepared<'frame>, RunError>;

    #[cfg(feature = "test-engine")]
    fn prepared_context_id(frame: &Self::Prepared<'_>) -> dear_imgui_rs::ContextId;

    /// Returns whether the main target is ready for presentation.
    fn render_main(&mut self, prepared: Self::Prepared<'_>) -> Result<bool, RunError>;

    fn present(&mut self) -> Result<(), RunError>;
}

impl FrameDriver for AdmittedWgpuFrameDriver<'_> {
    type Prepared<'frame> = ReconciledFrame<'frame>;

    #[cfg(feature = "test-engine")]
    const MAIN_CAN_SKIP: bool = false;

    fn prepare<'frame>(
        &mut self,
        frame: FrameToken<'frame>,
    ) -> Result<ReconciledFrame<'frame>, RunError> {
        let pending = frame.render(
            self.renderer
                .renderer_consumer()
                .map_err(RunError::Render)?,
        );
        self.renderer
            .reconcile_frame(pending)
            .map_err(RunError::Render)
    }

    #[cfg(feature = "test-engine")]
    fn prepared_context_id(frame: &Self::Prepared<'_>) -> dear_imgui_rs::ContextId {
        frame.context_id()
    }

    fn render_main(&mut self, frame: ReconciledFrame<'_>) -> Result<bool, RunError> {
        let (encoder, target) = self.recording.take().ok_or_else(|| RunError::Recovery {
            message: "admitted surface frame was already recorded".to_owned(),
        })?;
        self.output
            .record_and_submit(target, encoder, "Dear App render pass", |pass| {
                self.renderer
                    .render_reconciled(frame, pass, self.extent)
                    .map_err(RunError::Render)
            })?;
        Ok(true)
    }

    fn present(&mut self) -> Result<(), RunError> {
        self.output.present()
    }
}

#[cfg(feature = "test-engine")]
struct TestEngineDriver<'a, D>(&'a mut D);

#[cfg(feature = "test-engine")]
impl<D: FrameDriver> TestFrameDriver for TestEngineDriver<'_, D> {
    type PreparedFrame<'frame> = D::Prepared<'frame>;
    type PrepareError = RunError;
    type RenderError = RunError;
    type PresentError = RunError;

    fn prepare<'frame>(
        &mut self,
        frame: FrameToken<'frame>,
        _frame_index: u64,
    ) -> Result<Self::PreparedFrame<'frame>, Self::PrepareError> {
        self.0.prepare(frame)
    }

    fn prepared_context_id(frame: &Self::PreparedFrame<'_>) -> dear_imgui_rs::ContextId {
        D::prepared_context_id(frame)
    }

    fn render_main(
        &mut self,
        frame: Self::PreparedFrame<'_>,
        _frame_index: u64,
    ) -> Result<MainRenderOutcome, Self::RenderError> {
        Ok(if self.0.render_main(frame)? {
            MainRenderOutcome::ReadyToPresent
        } else {
            MainRenderOutcome::Skipped
        })
    }

    fn present(&mut self, _frame_index: u64) -> Result<(), Self::PresentError> {
        self.0.present()
    }
}

pub(super) fn drive_frame<A: Application, D: FrameDriver>(
    application: &mut A,
    frame: FrameToken<'_>,
    frame_index: u64,
    driver: &mut D,
) -> Result<(), RunError> {
    #[cfg(feature = "test-engine")]
    if let Some(engine) = application.test_engine() {
        return match engine
            .drive_frame(frame, frame_index, &mut TestEngineDriver(driver))
            .map_err(|source| RunError::TestEngineFrame {
                frame: frame_index,
                source: Box::new(source),
            })? {
            FrameDriveOutcome::Presented => Ok(()),
            _ if D::MAIN_CAN_SKIP => Ok(()),
            FrameDriveOutcome::Skipped => Err(RunError::Recovery {
                message: "an admitted surface frame was unexpectedly skipped".to_owned(),
            }),
            _ => Err(RunError::Recovery {
                message: "Test Engine returned an unsupported frame-drive outcome".to_owned(),
            }),
        };
    }

    #[cfg(not(feature = "test-engine"))]
    let _ = (application, frame_index);
    let prepared = driver.prepare(frame)?;
    if driver.render_main(prepared)? {
        driver.present()?;
    }
    Ok(())
}

pub(super) fn render_surface_frame<A: Application>(
    ownership: &mut RuntimeOwnership,
    clear_color: wgpu::Color,
    frame_count: &mut u64,
    application: &mut A,
    config: &AppConfig,
    wake: &crate::AppWakeHandle,
    event_loop: &winit::event_loop::ActiveEventLoop,
) -> Result<SurfaceDispatch<bool>, RunError> {
    #[cfg(feature = "multi-viewport")]
    if config.multi_viewport {
        return super::multi_viewport::render_frame(
            ownership,
            clear_color,
            frame_count,
            application,
            config,
            wake,
            event_loop,
        );
    }
    #[cfg(not(feature = "multi-viewport"))]
    let _ = event_loop;
    let admitted = {
        let RuntimeOwnership {
            window,
            generations,
            ..
        } = ownership;
        let (generation, lifecycle) = generations.render_parts();
        let gpu = generation.map(|generation| &generation.gpu);
        RuntimeSurfaceAdmission::new(window, lifecycle, gpu, config).admit()?
    };
    dispatch_surface_frame(admitted, frame_count, |admitted, frame_index| {
        render_application_frame(
            ownership,
            MainFrameTarget::Surface(admitted),
            clear_color,
            frame_index,
            application,
            config,
            wake,
        )
    })
}

pub(super) fn render_application_frame<A: Application>(
    ownership: &mut RuntimeOwnership,
    target: MainFrameTarget,
    clear_color: wgpu::Color,
    frame_index: u64,
    application: &mut A,
    config: &AppConfig,
    wake: &crate::AppWakeHandle,
) -> Result<bool, RunError> {
    let RuntimeOwnership {
        window,
        ui,
        generations,
    } = ownership;
    let generation = generations
        .current_mut()
        .ok_or_else(|| RunError::Recovery {
            message: "render requested without an active GPU generation".to_owned(),
        })?;
    let RecordedApplicationFrame {
        frame,
        exit_requested,
        encoder,
        framebuffer_extent,
    } = record_application_frame(
        ui,
        generation,
        window,
        application,
        config,
        wake,
        frame_index,
        Some(target.texture()),
    )?;
    let gpu = &mut generation.gpu;
    let mut driver = AdmittedWgpuFrameDriver {
        renderer: gpu.renderer.single_mut()?,
        output: MainFrameOutput::new(&gpu.queue, &gpu.device, clear_color),
        recording: Some((encoder, target)),
        extent: framebuffer_extent,
    };
    let result = drive_frame(application, frame, frame_index, &mut driver);
    driver.output.after_render(window, result)?;
    Ok(exit_requested)
}

/// An open UI frame and the application commands recorded before its UI consumers.
pub(super) struct RecordedApplicationFrame<'ctx> {
    pub(super) frame: FrameToken<'ctx>,
    pub(super) exit_requested: bool,
    pub(super) encoder: wgpu::CommandEncoder,
    framebuffer_extent: FramebufferExtent,
}

pub(super) fn record_application_frame<'ctx, A: Application>(
    ui: &'ctx mut UiState,
    generation: &mut RuntimeGeneration,
    window: &WindowState,
    application: &mut A,
    config: &AppConfig,
    wake: &AppWakeHandle,
    frame_index: u64,
    target: Option<&wgpu::Texture>,
) -> Result<RecordedApplicationFrame<'ctx>, RunError> {
    let (frame, exit_requested) =
        build_application_frame(ui, generation, window, application, config, wake)?;
    let gpu = &generation.gpu;
    let label = if target.is_some() {
        "Dear App engine and UI encoder"
    } else {
        "Dear App engine producer"
    };
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
    let (framebuffer_extent, surface_format) = match target {
        Some(texture) => (FramebufferExtent::from_texture(texture), texture.format()),
        None => {
            let size = window.window.inner_size();
            (
                FramebufferExtent::new(size.width, size.height),
                window.surface_config()?.format,
            )
        }
    };
    application
        .record_render(&mut crate::RenderContext {
            device: &gpu.device,
            queue: &gpu.queue,
            encoder: &mut encoder,
            window: &window.window,
            generation: generation.id,
            frame_index,
            framebuffer_extent,
            surface_format,
            wake,
        })
        .map_err(|error| error.during_application_stage(ApplicationStage::RecordRender))?;
    Ok(RecordedApplicationFrame {
        frame,
        exit_requested,
        encoder,
        framebuffer_extent,
    })
}

fn build_application_frame<'ctx, A: Application>(
    ui: &'ctx mut UiState,
    generation: &mut RuntimeGeneration,
    window: &WindowState,
    application: &mut A,
    config: &AppConfig,
    wake: &crate::AppWakeHandle,
) -> Result<(FrameToken<'ctx>, bool), RunError> {
    let UiState {
        context,
        platform,
        #[cfg(feature = "implot")]
        implot,
        #[cfg(feature = "imnodes")]
        imnodes,
        #[cfg(feature = "implot3d")]
        implot3d,
        docking,
        multi_viewport,
    } = ui;

    super::state::run_application_frame_boundary(
        context,
        ApplicationStage::PrepareFrame,
        |context| {
            let mut prepare_frame = PrepareFrameContext {
                imgui: context,
                window: &window.window,
            };
            application.prepare_frame(&mut prepare_frame)
        },
    )?;
    super::state::validate_supported_imgui_config(context, *multi_viewport)?;
    platform
        .prepare_frame(context, &window.window)
        .map_err(|error| super::state::platform_error("Winit frame preparation", error))?;
    let mut exit_requested = false;
    let frame = build_frame(context, |ui| {
        draw_dockspace(ui, docking.flags, config)?;
        let addons = AddOns {
            #[cfg(feature = "implot")]
            implot: implot.as_ref(),
            #[cfg(feature = "imnodes")]
            imnodes: imnodes.as_ref(),
            #[cfg(feature = "implot3d")]
            implot3d: implot3d.as_ref(),
            docking: DockingApi {
                controller: docking,
            },
        };
        let mut frame = FrameContext {
            ui,
            addons,
            gpu: generation.api(),
            exit_requested: &mut exit_requested,
            wake,
        };
        application
            .frame(&mut frame)
            .map_err(|error| error.during_application_stage(ApplicationStage::Frame))?;
        platform
            .prepare_render(ui, &window.window)
            .map_err(|error| super::state::platform_error("Winit render preparation", error))?;
        Ok(())
    })?;
    Ok((frame, exit_requested))
}

pub(super) fn build_frame<'ctx>(
    context: &'ctx mut dear_imgui_rs::Context,
    build: impl FnOnce(&dear_imgui_rs::Ui) -> Result<(), RunError>,
) -> Result<FrameToken<'ctx>, RunError> {
    let frame = context.try_begin_frame().map_err(RunError::ImGuiFrame)?;
    build(frame.ui())?;
    Ok(frame)
}

fn draw_dockspace(
    ui: &dear_imgui_rs::Ui,
    flags: DockNodeFlags,
    config: &AppConfig,
) -> Result<(), RunError> {
    let Some((host_window_name, mut window_flags)) = config.docking.full_viewport_host() else {
        return Ok(());
    };

    let viewport = ui.main_viewport();
    ui.set_next_window_viewport(viewport.id());
    if flags.contains(DockNodeFlags::PASSTHRU_CENTRAL_NODE) {
        window_flags |= WindowFlags::NO_BACKGROUND;
    }
    let mut dockspace_result = Ok(());
    ui.window(host_window_name)
        .flags(window_flags)
        .position(viewport.pos(), dear_imgui_rs::Condition::Always)
        .size(viewport.size(), dear_imgui_rs::Condition::Always)
        .build(|| {
            dockspace_result = ui
                .dockspace()
                .flags(flags)
                .build()
                .map(|_| ())
                .map_err(RunError::Dockspace);
        });
    dockspace_result
}

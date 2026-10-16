//! WGPU surface admission, rendering, presentation, and retry settlement.

use dear_imgui_rs::render::ReconciledFrame;
use dear_imgui_rs::{DockNodeFlags, FrameToken, WindowFlags};
#[cfg(feature = "test-engine")]
use dear_imgui_test_engine::{FrameDriveOutcome, MainRenderOutcome, TestFrameDriver};

use super::admission::{
    AdmittedSurfaceFrame, SurfaceAcquisition, SurfaceAdmissionBackend, SurfaceDispatch,
    admit_surface_frame, dispatch_surface_frame, settle_surface_presentation,
};
use super::lifecycle::{LifecycleAction, SurfaceEvent};
use super::ownership::RuntimeOwnership;
use super::recovery::RuntimeGenerations;
use super::state::{RuntimeGeneration, UiState, WindowState};
use crate::{
    AddOns, AppConfig, Application, ApplicationStage, DockingApi, FrameContext,
    PrepareFrameContext, RunError,
};

struct RuntimeSurfaceAdmission<'a> {
    window: &'a mut WindowState,
    generations: &'a mut RuntimeGenerations<RuntimeGeneration>,
    config: &'a AppConfig,
}

impl SurfaceAdmissionBackend for RuntimeSurfaceAdmission<'_> {
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
        self.generations.surface_event(event)
    }

    fn recover(&mut self, action: LifecycleAction) -> Result<(), RunError> {
        let generation = self
            .generations
            .current()
            .ok_or_else(|| RunError::Recovery {
                message: "surface recovery requested without an active GPU generation".to_owned(),
            })?;
        match action {
            LifecycleAction::RecreateSurface => self.window.recreate_surface(
                &generation.gpu.adapter,
                &generation.gpu.device,
                self.config,
            ),
            LifecycleAction::ReconfigureSurface => {
                self.window.reconfigure(&generation.gpu.device);
                Ok(())
            }
            _ => Err(RunError::Recovery {
                message: format!("surface admission requested invalid recovery action {action:?}"),
            }),
        }
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

    fn present(self, queue: &wgpu::Queue) {
        match self {
            Self::Surface(admitted) => queue.present(admitted.frame),
            #[cfg(test)]
            Self::Offscreen(_) => {}
        }
    }

    fn reconfigure_after_present(&self) -> bool {
        match self {
            Self::Surface(admitted) => admitted.reconfigure_after_present,
            #[cfg(test)]
            Self::Offscreen(_) => false,
        }
    }
}

struct AdmittedWgpuFrameDriver<'a> {
    renderer: &'a mut dear_imgui_wgpu::WgpuRenderer,
    queue: &'a wgpu::Queue,
    target: Option<MainFrameTarget>,
    clear_color: wgpu::Color,
    encoder: Option<wgpu::CommandEncoder>,
    rendered: bool,
    presented: bool,
}

impl<'a> AdmittedWgpuFrameDriver<'a> {
    fn new(
        renderer: &'a mut dear_imgui_wgpu::WgpuRenderer,
        queue: &'a wgpu::Queue,
        target: MainFrameTarget,
        clear_color: wgpu::Color,
        encoder: wgpu::CommandEncoder,
    ) -> Self {
        Self {
            renderer,
            queue,
            target: Some(target),
            clear_color,
            encoder: Some(encoder),
            rendered: false,
            presented: false,
        }
    }

    fn prepare_frame<'frame>(
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

    fn render_main_frame(&mut self, frame: ReconciledFrame<'_>) -> Result<(), RunError> {
        if self.rendered {
            return Err(RunError::Recovery {
                message: "admitted surface frame was rendered more than once".to_owned(),
            });
        }
        let target = self.target.as_ref().ok_or_else(|| RunError::Recovery {
            message: "admitted surface frame was consumed before rendering".to_owned(),
        })?;
        let framebuffer_extent = dear_imgui_wgpu::FramebufferExtent::from_texture(target.texture());
        let view = target
            .texture()
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.encoder.take().ok_or_else(|| RunError::Recovery {
            message: "admitted frame encoder was already consumed".to_owned(),
        })?;
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Dear App render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            self.renderer
                .render_reconciled(frame, &mut render_pass, framebuffer_extent)
                .map_err(RunError::Render)?;
        }
        self.queue.submit(Some(encoder.finish()));
        self.rendered = true;
        Ok(())
    }

    fn present_frame(&mut self) -> Result<(), RunError> {
        if !self.rendered {
            return Err(RunError::Recovery {
                message: "admitted surface frame was presented before rendering".to_owned(),
            });
        }
        let target = self.target.take().ok_or_else(|| RunError::Recovery {
            message: "admitted surface frame was presented more than once".to_owned(),
        })?;
        target.present(self.queue);
        self.presented = true;
        Ok(())
    }

    const fn was_presented(&self) -> bool {
        self.presented
    }
}

#[cfg(feature = "test-engine")]
impl TestFrameDriver for AdmittedWgpuFrameDriver<'_> {
    type PreparedFrame<'frame> = ReconciledFrame<'frame>;
    type PrepareError = RunError;
    type RenderError = RunError;
    type PresentError = RunError;

    fn prepare<'frame>(
        &mut self,
        frame: FrameToken<'frame>,
        _frame_index: u64,
    ) -> Result<Self::PreparedFrame<'frame>, Self::PrepareError> {
        self.prepare_frame(frame)
    }

    fn prepared_context_id(frame: &Self::PreparedFrame<'_>) -> dear_imgui_rs::ContextId {
        frame.context_id()
    }

    fn render_main(
        &mut self,
        frame: Self::PreparedFrame<'_>,
        _frame_index: u64,
    ) -> Result<MainRenderOutcome, Self::RenderError> {
        self.render_main_frame(frame)?;
        Ok(MainRenderOutcome::ReadyToPresent)
    }

    fn present(&mut self, _frame_index: u64) -> Result<(), Self::PresentError> {
        self.present_frame()
    }
}

fn drive_admitted_frame<A: Application>(
    application: &mut A,
    frame: FrameToken<'_>,
    frame_index: u64,
    driver: &mut AdmittedWgpuFrameDriver<'_>,
) -> Result<(), RunError> {
    #[cfg(feature = "test-engine")]
    if let Some(engine) = application.test_engine() {
        return match engine
            .drive_frame(frame, frame_index, driver)
            .map_err(|source| RunError::TestEngineFrame {
                frame: frame_index,
                source: Box::new(source),
            })? {
            FrameDriveOutcome::Presented => Ok(()),
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
    let reconciled = driver.prepare_frame(frame)?;
    driver.render_main_frame(reconciled)?;
    driver.present_frame()
}

pub(super) fn render_surface_frame<A: Application>(
    ownership: &mut RuntimeOwnership,
    clear_color: wgpu::Color,
    admitted_frame_count: &mut u64,
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
            admitted_frame_count,
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
        let mut backend = RuntimeSurfaceAdmission {
            window,
            generations,
            config,
        };
        admit_surface_frame(&mut backend)?
    };
    dispatch_surface_frame(admitted, admitted_frame_count, |admitted, frame_index| {
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
    let (frame, exit_requested) =
        build_application_frame(ui, generation, window, application, config, wake)?;
    let gpu = &mut generation.gpu;
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Dear App engine and UI encoder"),
        });
    let mut render_context = crate::RenderContext {
        device: &gpu.device,
        queue: &gpu.queue,
        encoder: &mut encoder,
        window: &window.window,
        generation: generation.id,
        frame_index,
        framebuffer_extent: dear_imgui_wgpu::FramebufferExtent::from_texture(target.texture()),
        surface_format: target.texture().format(),
        wake,
    };
    application
        .record_render(&mut render_context)
        .map_err(|error| error.during_application_stage(ApplicationStage::RecordRender))?;
    let renderer = gpu.renderer.single_mut()?;
    let reconfigure_after_present = target.reconfigure_after_present();
    let mut driver =
        AdmittedWgpuFrameDriver::new(renderer, &gpu.queue, target, clear_color, encoder);
    let result = drive_admitted_frame(application, frame, frame_index, &mut driver);
    let was_presented = driver.was_presented();
    drop(driver);
    settle_surface_presentation(result, was_presented, reconfigure_after_present, || {
        window.reconfigure(&generation.gpu.device)
    })?;
    Ok(exit_requested)
}

pub(super) fn build_application_frame<'ctx, A: Application>(
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

//! WGPU surface admission, rendering, presentation, and retry settlement.

use dear_imgui_rs::render::{ReconciledFrame, RenderedFrame};
use dear_imgui_rs::{DockNodeFlags, Id, WindowFlags};
#[cfg(feature = "test-engine")]
use dear_imgui_test_engine::TestFrameDriver;

use super::admission::{
    SurfaceAcquisition, SurfaceAdmissionBackend, SurfaceDispatch, admit_surface_frame,
    dispatch_surface_frame, settle_surface_presentation,
};
use super::lifecycle::{LifecycleAction, SurfaceEvent};
use super::ownership::RuntimeOwnership;
use super::recovery::RuntimeGenerations;
use super::state::{RuntimeGeneration, UiState, WindowState};
use crate::{
    AddOns, AppConfig, Application, DockingApi, FrameContext, PrepareFrameContext, RunError,
};

struct RuntimeSurfaceAdmission<'a> {
    window: &'a mut WindowState,
    generations: &'a mut RuntimeGenerations<RuntimeGeneration>,
    config: &'a AppConfig,
}

impl SurfaceAdmissionBackend for RuntimeSurfaceAdmission<'_> {
    type Frame = wgpu::SurfaceTexture;

    fn acquire(&mut self) -> SurfaceAcquisition<Self::Frame> {
        match self.window.surface.get_current_texture() {
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

struct AdmittedWgpuFrameDriver<'a> {
    renderer: &'a mut dear_imgui_wgpu::WgpuRenderer,
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    surface_frame: Option<wgpu::SurfaceTexture>,
    clear_color: wgpu::Color,
    rendered: bool,
    presented: bool,
}

impl<'a> AdmittedWgpuFrameDriver<'a> {
    fn new(
        renderer: &'a mut dear_imgui_wgpu::WgpuRenderer,
        device: &'a wgpu::Device,
        queue: &'a wgpu::Queue,
        surface_frame: wgpu::SurfaceTexture,
        clear_color: wgpu::Color,
    ) -> Self {
        Self {
            renderer,
            device,
            queue,
            surface_frame: Some(surface_frame),
            clear_color,
            rendered: false,
            presented: false,
        }
    }

    fn render_frame<'frame>(
        &mut self,
        frame: RenderedFrame<'frame>,
    ) -> Result<ReconciledFrame<'frame>, RunError> {
        if self.rendered {
            return Err(RunError::Recovery {
                message: "admitted surface frame was rendered more than once".to_owned(),
            });
        }
        let surface_frame = self
            .surface_frame
            .as_ref()
            .ok_or_else(|| RunError::Recovery {
                message: "admitted surface frame was consumed before rendering".to_owned(),
            })?;
        let view = surface_frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Dear App render encoder"),
            });
        let reconciled = {
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
                .render_reconciled(frame, &mut render_pass)
                .map_err(RunError::Render)?
        };
        self.queue.submit(Some(encoder.finish()));
        self.rendered = true;
        Ok(reconciled)
    }

    fn present_frame(&mut self) -> Result<(), RunError> {
        if !self.rendered {
            return Err(RunError::Recovery {
                message: "admitted surface frame was presented before rendering".to_owned(),
            });
        }
        let surface_frame = self
            .surface_frame
            .take()
            .ok_or_else(|| RunError::Recovery {
                message: "admitted surface frame was presented more than once".to_owned(),
            })?;
        self.queue.present(surface_frame);
        self.presented = true;
        Ok(())
    }

    const fn was_presented(&self) -> bool {
        self.presented
    }
}

#[cfg(feature = "test-engine")]
impl TestFrameDriver for AdmittedWgpuFrameDriver<'_> {
    type RenderError = RunError;
    type PresentError = RunError;

    fn render<'frame>(
        &mut self,
        frame: RenderedFrame<'frame>,
        _frame_index: u64,
    ) -> Result<ReconciledFrame<'frame>, Self::RenderError> {
        self.render_frame(frame)
    }

    fn present(&mut self, _frame_index: u64) -> Result<(), Self::PresentError> {
        self.present_frame()
    }
}

fn drive_admitted_frame<A: Application>(
    application: &mut A,
    rendered: RenderedFrame<'_>,
    frame_index: u64,
    driver: &mut AdmittedWgpuFrameDriver<'_>,
) -> Result<(), RunError> {
    #[cfg(feature = "test-engine")]
    if let Some(engine) = application.test_engine() {
        return engine
            .drive_frame(rendered, frame_index, driver)
            .map_err(|source| RunError::TestEngineFrame {
                frame: frame_index,
                source: Box::new(source),
            });
    }

    #[cfg(not(feature = "test-engine"))]
    let _ = (application, frame_index);
    let reconciled = driver.render_frame(rendered)?;
    drop(reconciled);
    driver.present_frame()
}

pub(super) fn render_surface_frame<A: Application>(
    ownership: &mut RuntimeOwnership,
    clear_color: wgpu::Color,
    admitted_frame_count: &mut u64,
    application: &mut A,
    config: &AppConfig,
) -> Result<SurfaceDispatch<bool>, RunError> {
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
    let dispatch =
        dispatch_surface_frame(admitted, admitted_frame_count, |admitted, frame_index| {
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
            } = ui;

            let mut prepare_frame = PrepareFrameContext {
                imgui: context,
                window: &window.window,
            };
            application.prepare_frame(&mut prepare_frame)?;
            super::state::validate_supported_imgui_config(context)?;
            platform
                .prepare_frame(context, &window.window)
                .map_err(|error| super::state::platform_error("Winit frame preparation", error))?;
            let mut exit_requested = false;
            let draw_data = build_and_render_frame(context, |ui| {
                draw_dockspace(ui, docking.flags, config);
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
                };
                application.frame(&mut frame)?;
                platform
                    .prepare_render(ui, &window.window)
                    .map_err(|error| {
                        super::state::platform_error("Winit render preparation", error)
                    })?;
                Ok(())
            })?;

            let generation = generations
                .current_mut()
                .ok_or_else(|| RunError::Recovery {
                    message: "render submission requested without an active GPU generation"
                        .to_owned(),
                })?;
            let reconfigure_after_present = admitted.reconfigure_after_present;
            let gpu = &mut generation.gpu;
            let mut driver = AdmittedWgpuFrameDriver::new(
                &mut gpu.renderer,
                &gpu.device,
                &gpu.queue,
                admitted.frame,
                clear_color,
            );
            let result = drive_admitted_frame(application, draw_data, frame_index, &mut driver);
            let was_presented = driver.was_presented();
            drop(driver);
            settle_surface_presentation(result, was_presented, reconfigure_after_present, || {
                window.reconfigure(&generation.gpu.device)
            })?;
            Ok(exit_requested)
        })?;
    Ok(dispatch)
}

pub(super) fn build_and_render_frame<'ctx>(
    context: &'ctx mut dear_imgui_rs::Context,
    build: impl FnOnce(&dear_imgui_rs::Ui) -> Result<(), RunError>,
) -> Result<dear_imgui_rs::render::RenderedFrame<'ctx>, RunError> {
    let frame = context.begin_frame();
    build(frame.ui())?;
    Ok(frame.render())
}

fn draw_dockspace(ui: &dear_imgui_rs::Ui, flags: DockNodeFlags, config: &AppConfig) {
    let Some((host_window_name, mut window_flags)) = config.docking.full_viewport_host() else {
        return;
    };

    let viewport = ui.main_viewport();
    ui.set_next_window_viewport(viewport.id());
    if flags.contains(DockNodeFlags::PASSTHRU_CENTRAL_NODE) {
        window_flags |= WindowFlags::NO_BACKGROUND;
    }
    ui.window(host_window_name)
        .flags(window_flags)
        .position(viewport.pos(), dear_imgui_rs::Condition::Always)
        .size(viewport.size(), dear_imgui_rs::Condition::Always)
        .build(|| {
            let _ = ui.dockspace_over_main_viewport_with_flags(Id::from(0_u32), flags);
        });
}

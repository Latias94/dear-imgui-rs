use std::time::{Duration, Instant};

use dear_imgui_rs::render::{ReconciledFrame, RenderedFrame};
use dear_imgui_rs::{DockNodeFlags, Id, WindowFlags};
#[cfg(feature = "test-engine")]
use dear_imgui_test_engine::TestFrameDriver;
use tracing::{error, info, warn};
use winit::{
    application::ApplicationHandler,
    event::{Event, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::WindowId,
};

use super::{
    admission::{
        SurfaceAcquisition, SurfaceAdmissionBackend, admit_surface_frame, dispatch_surface_frame,
        settle_surface_presentation,
    },
    lifecycle::{LifecycleAction, SurfaceEvent},
    recovery::{
        GenerationRelease, RecoveryEffects, RecoveryOutcome, RuntimeFactory, RuntimeGenerations,
    },
    state::{
        GpuFaultKind, RuntimeEvent, RuntimeGeneration, UiState, WgpuRuntimeFactory, WindowState,
    },
};
use crate::{
    AddOns, AppConfig, Application, DockingApi, FrameContext, GpuGeneration, InitContext,
    PrepareFrameContext, RedrawMode, RunError, ShutdownContext,
};

pub(crate) fn run<A: Application + 'static>(
    config: AppConfig,
    application: A,
) -> Result<(), RunError> {
    validate_config(&config)?;
    let event_loop = EventLoop::<RuntimeEvent>::with_user_event().build()?;
    set_initial_control_flow(&event_loop, config.redraw);
    let event_proxy = event_loop.create_proxy();
    let mut runner = Runner::new(config, application, event_proxy);

    info!("Starting Dear App event loop");
    let event_loop_result = event_loop.run_app(&mut runner);
    runner.shutdown_once();
    let terminal_before_shutdown = runner.take_terminal_error();
    let shutdown_error = runner.take_shutdown_error();
    resolve_run_result(terminal_before_shutdown, event_loop_result, shutdown_error)
}

fn validate_config(config: &AppConfig) -> Result<(), RunError> {
    if config
        .io_config_flags
        .is_some_and(|flags| flags.contains(dear_imgui_rs::ConfigFlags::VIEWPORTS_ENABLE))
    {
        return Err(RunError::MultiViewportUnsupported);
    }
    Ok(())
}

fn set_initial_control_flow(event_loop: &EventLoop<RuntimeEvent>, redraw: RedrawMode) {
    match redraw {
        RedrawMode::Poll => event_loop.set_control_flow(ControlFlow::Poll),
        RedrawMode::Wait => event_loop.set_control_flow(ControlFlow::Wait),
        RedrawMode::WaitUntil { fps } => {
            event_loop
                .set_control_flow(ControlFlow::WaitUntil(Instant::now() + frame_duration(fps)));
        }
    }
}

fn frame_duration(fps: f32) -> Duration {
    Duration::from_secs_f32(1.0 / fps.max(1.0))
}

struct Runtime {
    ownership: OrderedRuntimeOwner<RuntimeOwnership>,
    clear_color: wgpu::Color,
    admitted_frame_count: u64,
}

struct RuntimeOwnership {
    window: WindowState,
    ui: UiState,
    generations: RuntimeGenerations<RuntimeGeneration>,
}

trait RuntimeOwnershipLifecycle: Sized {
    fn release_renderer(&mut self) -> Result<(), RunError>;
    fn release_platform(&mut self) -> Result<(), RunError>;
    fn teardown_after_backend_release(self);
}

struct OrderedRuntimeOwner<T: RuntimeOwnershipLifecycle> {
    ownership: Option<T>,
}

/// Quarantines the ownership graph unless every Context-bound backend release reaches its commit
/// point.
struct BackendReleaseTransaction<T> {
    ownership: Option<T>,
}

impl<T> BackendReleaseTransaction<T> {
    fn new(ownership: T) -> Self {
        Self {
            ownership: Some(ownership),
        }
    }

    fn ownership_mut(&mut self) -> &mut T {
        self.ownership
            .as_mut()
            .expect("renderer release transaction owns the runtime graph")
    }

    fn commit(mut self) -> T {
        self.ownership
            .take()
            .expect("renderer release transaction can commit only once")
    }
}

impl<T> Drop for BackendReleaseTransaction<T> {
    fn drop(&mut self) {
        if let Some(ownership) = self.ownership.take() {
            std::mem::forget(ownership);
        }
    }
}

impl<T: RuntimeOwnershipLifecycle> OrderedRuntimeOwner<T> {
    fn new(ownership: T) -> Self {
        Self {
            ownership: Some(ownership),
        }
    }

    fn get(&self) -> &T {
        self.ownership
            .as_ref()
            .expect("runtime ownership is available until teardown starts")
    }

    fn get_mut(&mut self) -> &mut T {
        self.ownership
            .as_mut()
            .expect("runtime ownership is available until teardown starts")
    }

    fn teardown(mut self) -> Result<(), RunError> {
        let ownership = self
            .ownership
            .take()
            .expect("runtime ownership can be consumed only once");
        release_then_teardown_or_quarantine(ownership)
    }
}

impl<T: RuntimeOwnershipLifecycle> Drop for OrderedRuntimeOwner<T> {
    fn drop(&mut self) {
        let Some(ownership) = self.ownership.take() else {
            return;
        };
        if let Err(error) = release_then_teardown_or_quarantine(ownership) {
            error!("Dear App quarantined runtime ownership after backend release failed: {error}");
        }
    }
}

fn release_then_teardown_or_quarantine<T: RuntimeOwnershipLifecycle>(
    ownership: T,
) -> Result<(), RunError> {
    let mut transaction = BackendReleaseTransaction::new(ownership);
    // The transaction quarantines the complete graph if renderer resources still borrow the
    // Context, window, or GPU generation.
    transaction.ownership_mut().release_renderer()?;
    // A platform ownership conflict leaves Context attachment state uncertain. Context drop must
    // not run its fallback teardown after an explicit release failure.
    transaction.ownership_mut().release_platform()?;
    let ownership = transaction.commit();
    ownership.teardown_after_backend_release();
    Ok(())
}

impl RuntimeOwnershipLifecycle for RuntimeOwnership {
    fn release_renderer(&mut self) -> Result<(), RunError> {
        let mut release = RuntimeRelease { ui: &mut self.ui };
        self.generations.shutdown(&mut release)
    }

    fn release_platform(&mut self) -> Result<(), RunError> {
        self.ui.release_platform()
    }

    fn teardown_after_backend_release(self) {
        let Self {
            window,
            ui,
            generations,
        } = self;
        drop(generations);
        ui.teardown_after_platform_release();
        drop(window);
    }
}

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

impl Runtime {
    fn window(&self) -> &WindowState {
        &self.ownership.get().window
    }

    fn new<A: Application>(
        event_loop: &ActiveEventLoop,
        event_proxy: EventLoopProxy<RuntimeEvent>,
        config: &AppConfig,
        application: &mut A,
    ) -> Result<Self, RunError> {
        let mut window = WindowState::new(event_loop, config)?;
        let mut ui = UiState::new(&window, config, application)?;
        let generation = match WgpuRuntimeFactory::create(
            &mut window,
            &mut ui,
            config,
            event_proxy,
            GpuGeneration::INITIAL,
        ) {
            Ok(generation) => generation,
            Err(error) => {
                return Err(abort_runtime_initialization(application, ui, window, error));
            }
        };
        let generations = match RuntimeGenerations::new(generation) {
            Ok(generations) => generations,
            Err(error) => {
                return Err(abort_runtime_initialization(application, ui, window, error));
            }
        };
        let mut runtime = Self {
            ownership: OrderedRuntimeOwner::new(RuntimeOwnership {
                window,
                ui,
                generations,
            }),
            clear_color: wgpu::Color {
                r: config.clear_color[0] as f64,
                g: config.clear_color[1] as f64,
                b: config.clear_color[2] as f64,
                a: config.clear_color[3] as f64,
            },
            admitted_frame_count: 0,
        };

        if let Err(error) = runtime.notify_initialized(application, config) {
            return Err(super::state::preserve_initialization_error(error, || {
                let application_error = runtime.shutdown_application(application);
                let teardown_error = runtime.teardown();
                application_error.or(teardown_error).map_or(Ok(()), Err)
            }));
        }
        Ok(runtime)
    }

    fn notify_initialized<A: Application>(
        &mut self,
        application: &mut A,
        config: &AppConfig,
    ) -> Result<(), RunError> {
        let RuntimeOwnership {
            window,
            ui,
            generations,
        } = self.ownership.get_mut();
        let generation = generations
            .current_mut()
            .ok_or_else(|| RunError::Recovery {
                message: "initialized callback requested without a GPU generation".to_owned(),
            })?;
        let mut init = InitContext {
            imgui: &mut ui.context,
            window: &window.window,
            config,
        };
        let mut gpu = generation.context(window)?;
        application.initialized(&mut init, &mut gpu)?;
        super::state::validate_supported_imgui_config(&ui.context)
    }

    fn handle_event<A: Application>(
        &mut self,
        application: &mut A,
        window_id: WindowId,
        event: &WindowEvent,
    ) -> Result<bool, RunError> {
        let RuntimeOwnership {
            window,
            ui,
            generations,
        } = self.ownership.get_mut();
        let full_event: Event<RuntimeEvent> = Event::WindowEvent {
            window_id,
            event: event.clone(),
        };
        ui.platform
            .handle_event(&mut ui.context, &window.window, &full_event)
            .map_err(|error| super::state::platform_error("Winit event handling", error))?;

        let mut exit_requested = matches!(event, WindowEvent::CloseRequested);
        {
            let mut context = crate::EventContext {
                event,
                imgui: &mut ui.context,
                window: &window.window,
                exit_requested: &mut exit_requested,
            };
            application.event(&mut context)?;
        }
        super::state::validate_supported_imgui_config(&ui.context)?;

        let Some(generation) = generations.current() else {
            return Ok(exit_requested);
        };
        match event {
            WindowEvent::Resized(size) => {
                window.resize(*size, &generation.gpu.device);
                window.window.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                let size = window.window.inner_size();
                window.resize(size, &generation.gpu.device);
                window.window.request_redraw();
            }
            _ => {}
        }
        Ok(exit_requested)
    }

    fn render<A: Application>(
        &mut self,
        application: &mut A,
        config: &AppConfig,
    ) -> Result<bool, RunError> {
        let clear_color = self.clear_color;
        let admitted = {
            let RuntimeOwnership {
                window,
                generations,
                ..
            } = self.ownership.get_mut();
            let mut backend = RuntimeSurfaceAdmission {
                window,
                generations,
                config,
            };
            admit_surface_frame(&mut backend)?
        };
        let ownership = self.ownership.get_mut();
        let dispatch = dispatch_surface_frame(
            admitted,
            &mut self.admitted_frame_count,
            |admitted, frame_index| {
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
                    .map_err(|error| {
                        super::state::platform_error("Winit frame preparation", error)
                    })?;
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
                settle_surface_presentation(
                    result,
                    was_presented,
                    reconfigure_after_present,
                    || window.reconfigure(&generation.gpu.device),
                )?;
                Ok(exit_requested)
            },
        )?;
        Ok(dispatch.unwrap_or(false))
    }

    fn recover<A: Application>(
        &mut self,
        application: &mut A,
        config: &AppConfig,
        event_proxy: EventLoopProxy<RuntimeEvent>,
        signal_generation: GpuGeneration,
    ) -> RecoveryOutcome {
        let RuntimeOwnership {
            window,
            ui,
            generations,
        } = self.ownership.get_mut();
        let mut environment = RuntimeRecovery {
            window,
            ui,
            application,
            config,
            event_proxy,
        };
        let mut factory = WgpuRuntimeFactory;
        generations.recover(signal_generation, &mut environment, &mut factory)
    }

    fn recovery_error_message(&self) -> String {
        self.ownership
            .get()
            .generations
            .terminal_error()
            .map(ToString::to_string)
            .unwrap_or_else(|| "GPU recovery failed without a terminal error".to_owned())
    }

    fn current_generation(&self) -> Option<GpuGeneration> {
        self.ownership.get().generations.current_generation()
    }

    fn shutdown_application<A: Application>(&mut self, application: &mut A) -> Option<RunError> {
        let RuntimeOwnership {
            window,
            ui,
            generations,
        } = self.ownership.get_mut();
        let generation = generations.current_generation();
        let mut context = ShutdownContext {
            imgui: &mut ui.context,
            window: &window.window,
            generation,
        };
        application.shutdown(&mut context).err()
    }

    fn fail(&mut self, error: RunError) {
        self.ownership.get_mut().generations.fail(error);
    }

    fn teardown(mut self) -> Option<RunError> {
        let terminal_error = self.ownership.get_mut().generations.take_terminal_error();
        let release_result = self.ownership.teardown();
        terminal_error.or_else(|| release_result.err())
    }

    fn shutdown<A: Application>(mut self, application: &mut A) -> RuntimeShutdownErrors {
        let terminal_error = self.ownership.get_mut().generations.take_terminal_error();
        let shutdown_error = self.shutdown_application(application);
        let ownership = self.ownership;
        finish_runtime_shutdown(terminal_error, || shutdown_error, || ownership.teardown())
    }
}

fn abort_runtime_initialization<A: Application>(
    application: &mut A,
    mut ui: UiState,
    window: WindowState,
    primary_error: RunError,
) -> RunError {
    super::state::preserve_initialization_error(primary_error, move || {
        let application_result = {
            let mut shutdown = ShutdownContext {
                imgui: &mut ui.context,
                window: &window.window,
                generation: None,
            };
            application.shutdown(&mut shutdown)
        };
        let platform_result = ui.release_platform_then_teardown_or_quarantine();
        drop(window);
        application_result.and(platform_result)
    })
}

fn build_and_render_frame<'ctx>(
    context: &'ctx mut dear_imgui_rs::Context,
    build: impl FnOnce(&dear_imgui_rs::Ui) -> Result<(), RunError>,
) -> Result<dear_imgui_rs::render::RenderedFrame<'ctx>, RunError> {
    let frame = context.begin_frame();
    build(frame.ui())?;
    Ok(frame.render())
}

struct RuntimeRecovery<'a, A> {
    window: &'a mut WindowState,
    ui: &'a mut UiState,
    application: &'a mut A,
    config: &'a AppConfig,
    event_proxy: EventLoopProxy<RuntimeEvent>,
}

struct RuntimeRelease<'a> {
    ui: &'a mut UiState,
}

impl GenerationRelease<RuntimeGeneration> for RuntimeRelease<'_> {
    fn release_generation(&mut self, generation: &mut RuntimeGeneration) -> Result<(), RunError> {
        generation.gpu.release_renderer(&mut self.ui.context)
    }
}

impl<A: Application> GenerationRelease<RuntimeGeneration> for RuntimeRecovery<'_, A> {
    fn release_generation(&mut self, generation: &mut RuntimeGeneration) -> Result<(), RunError> {
        generation.gpu.release_renderer(&mut self.ui.context)
    }
}

impl<A: Application> RecoveryEffects<RuntimeGeneration> for RuntimeRecovery<'_, A> {
    fn gpu_lost(&mut self, generation: &mut RuntimeGeneration) -> Result<(), RunError> {
        let mut context = generation.context(self.window)?;
        self.application.gpu_lost(&mut context)
    }

    fn invalidate_resources(&mut self, generation: &mut RuntimeGeneration) -> Result<(), RunError> {
        generation
            .gpu
            .renderer
            .invalidate_device_objects(&mut self.ui.context)
            .map_err(RunError::GpuInvalidation)
    }

    fn gpu_recreated(&mut self, generation: &mut RuntimeGeneration) -> Result<(), RunError> {
        let mut context = generation.context(self.window)?;
        self.application.gpu_recreated(&mut context)
    }
}

impl<A: Application> RuntimeFactory<RuntimeRecovery<'_, A>> for WgpuRuntimeFactory {
    type Candidate = RuntimeGeneration;

    fn create(
        &mut self,
        environment: &mut RuntimeRecovery<'_, A>,
        generation: GpuGeneration,
    ) -> Result<Self::Candidate, RunError> {
        WgpuRuntimeFactory::create(
            environment.window,
            environment.ui,
            environment.config,
            environment.event_proxy.clone(),
            generation,
        )
    }
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

struct Runner<A> {
    config: AppConfig,
    ownership: RunnerOwnership<Runtime, A>,
    shutdown: ShutdownCoordinator,
    event_proxy: EventLoopProxy<RuntimeEvent>,
    last_wake: Instant,
}

/// Keeps renderer-side registrations alive no longer than their application-owned resources.
struct RunnerOwnership<R, A> {
    runtime: Option<R>,
    application: A,
}

impl<A: Application> Runner<A> {
    fn new(config: AppConfig, application: A, event_proxy: EventLoopProxy<RuntimeEvent>) -> Self {
        Self {
            config,
            ownership: RunnerOwnership {
                runtime: None,
                application,
            },
            shutdown: ShutdownCoordinator::default(),
            event_proxy,
            last_wake: Instant::now(),
        }
    }

    fn terminate(&mut self, event_loop: &ActiveEventLoop, error: RunError) {
        error!("Dear App terminated: {error}");
        if let Some(runtime) = self.ownership.runtime.as_mut() {
            runtime.fail(error);
        } else {
            self.shutdown.remember_error(error);
        }
        self.shutdown_once();
        event_loop.exit();
    }

    fn exit_normally(&mut self, event_loop: &ActiveEventLoop) {
        self.shutdown_once();
        event_loop.exit();
    }

    fn shutdown_once(&mut self) {
        self.shutdown.shutdown_once(
            &mut self.ownership.runtime,
            &mut self.ownership.application,
            Runtime::shutdown,
        );
    }

    fn take_terminal_error(&mut self) -> Option<RunError> {
        self.shutdown.take_terminal_error()
    }

    fn take_shutdown_error(&mut self) -> Option<RunError> {
        self.shutdown.take_shutdown_error()
    }
}

#[derive(Default)]
struct RuntimeShutdownErrors {
    terminal_error: Option<RunError>,
    shutdown_error: Option<RunError>,
}

fn finish_runtime_shutdown(
    terminal_error: Option<RunError>,
    application_shutdown: impl FnOnce() -> Option<RunError>,
    release_backends: impl FnOnce() -> Result<(), RunError>,
) -> RuntimeShutdownErrors {
    // User-owned resources may still need the Context, but renderer and platform teardown must
    // proceed even when the hook reports an error. Backend release owns the Context fail-stop
    // decision and quarantines the complete graph if it cannot commit.
    let shutdown_error = application_shutdown();
    let release_result = release_backends();
    RuntimeShutdownErrors {
        terminal_error: terminal_error.or_else(|| release_result.err()),
        shutdown_error,
    }
}

#[derive(Default)]
struct ShutdownCoordinator {
    started: bool,
    terminal_error: Option<RunError>,
    shutdown_error: Option<RunError>,
}

impl ShutdownCoordinator {
    const fn started(&self) -> bool {
        self.started
    }

    fn remember_error(&mut self, error: RunError) {
        if self.terminal_error.is_none() {
            self.terminal_error = Some(error);
        }
    }

    fn shutdown_once<R, A>(
        &mut self,
        runtime: &mut Option<R>,
        application: &mut A,
        shutdown: impl FnOnce(R, &mut A) -> RuntimeShutdownErrors,
    ) {
        if self.started {
            return;
        }
        self.started = true;
        let errors = runtime
            .take()
            .map(|runtime| shutdown(runtime, application))
            .unwrap_or_default();
        if let Some(error) = errors.terminal_error {
            self.remember_error(error);
        }
        if self.shutdown_error.is_none() {
            self.shutdown_error = errors.shutdown_error;
        }
    }

    fn take_terminal_error(&mut self) -> Option<RunError> {
        self.terminal_error.take()
    }

    fn take_shutdown_error(&mut self) -> Option<RunError> {
        self.shutdown_error.take()
    }
}

fn should_process_runtime_event(shutdown_called: bool, event_loop_exiting: bool) -> bool {
    !shutdown_called && !event_loop_exiting
}

fn uncaptured_gpu_fault(kind: GpuFaultKind, message: String) -> RunError {
    match kind {
        GpuFaultKind::OutOfMemory => RunError::GpuOutOfMemory { message },
        GpuFaultKind::Validation => RunError::GpuValidation { message },
        GpuFaultKind::Internal => RunError::GpuInternal { message },
    }
}

enum GpuFaultDisposition {
    IgnoreStale,
    Terminate(RunError),
}

fn classify_uncaptured_gpu_fault(
    current_generation: Option<GpuGeneration>,
    signal_generation: GpuGeneration,
    kind: GpuFaultKind,
    message: String,
) -> GpuFaultDisposition {
    match current_generation {
        None => GpuFaultDisposition::Terminate(RunError::Recovery {
            message: "GPU fault received before runtime initialization".to_owned(),
        }),
        Some(current) if current != signal_generation => GpuFaultDisposition::IgnoreStale,
        Some(_) => GpuFaultDisposition::Terminate(uncaptured_gpu_fault(kind, message)),
    }
}

fn resolve_run_result(
    terminal_before_shutdown: Option<RunError>,
    event_loop_result: Result<(), winit::error::EventLoopError>,
    shutdown_error: Option<RunError>,
) -> Result<(), RunError> {
    if let Some(error) = terminal_before_shutdown {
        return Err(error);
    }
    if let Err(error) = event_loop_result {
        return Err(error.into());
    }
    match shutdown_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn initialize_runtime_once<R, E>(
    runtime: &mut Option<R>,
    shutdown_started: bool,
    initialize: impl FnOnce() -> Result<R, E>,
) -> Option<Result<(), E>> {
    if runtime.is_some() || shutdown_started {
        return None;
    }

    Some(initialize().map(|initialized| {
        *runtime = Some(initialized);
    }))
}

fn dispatch_live_window_event<T>(
    live_window_id: WindowId,
    event_window_id: WindowId,
    dispatch: impl FnOnce() -> T,
) -> Option<T> {
    (live_window_id == event_window_id).then(dispatch)
}

impl<A: Application + 'static> ApplicationHandler<RuntimeEvent> for Runner<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let shutdown_started = self.shutdown.started();
        let event_proxy = self.event_proxy.clone();
        match initialize_runtime_once(&mut self.ownership.runtime, shutdown_started, || {
            Runtime::new(
                event_loop,
                event_proxy,
                &self.config,
                &mut self.ownership.application,
            )
        }) {
            Some(Ok(())) => {
                info!("Dear App window and initial GPU generation are ready");
                self.ownership
                    .runtime
                    .as_ref()
                    .expect("successful initialization stores the runtime")
                    .window()
                    .window
                    .request_redraw();
            }
            Some(Err(error)) => self.terminate(event_loop, error),
            None => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: RuntimeEvent) {
        let generation = match &event {
            RuntimeEvent::DeviceLost { generation, .. }
            | RuntimeEvent::GpuFault { generation, .. } => *generation,
        };

        if !should_process_runtime_event(self.shutdown.started(), event_loop.exiting()) {
            warn!(
                generation = generation.get(),
                "Ignoring GPU signal after runtime shutdown"
            );
            return;
        }

        let (generation, message) = match event {
            RuntimeEvent::DeviceLost {
                generation,
                message,
            } => (generation, message),
            RuntimeEvent::GpuFault {
                generation,
                kind,
                message,
            } => {
                let current_generation = self
                    .ownership
                    .runtime
                    .as_ref()
                    .and_then(Runtime::current_generation);
                match classify_uncaptured_gpu_fault(current_generation, generation, kind, message) {
                    GpuFaultDisposition::IgnoreStale => {
                        warn!(
                            generation = generation.get(),
                            "Ignoring stale uncaptured GPU fault"
                        );
                    }
                    GpuFaultDisposition::Terminate(error) => self.terminate(event_loop, error),
                }
                return;
            }
        };

        let Some(runtime) = self.ownership.runtime.as_mut() else {
            self.terminate(
                event_loop,
                RunError::Recovery {
                    message: "device loss received before runtime initialization".to_owned(),
                },
            );
            return;
        };
        match runtime.recover(
            &mut self.ownership.application,
            &self.config,
            self.event_proxy.clone(),
            generation,
        ) {
            RecoveryOutcome::Ignored => {
                warn!(
                    generation = generation.get(),
                    "Ignoring stale or duplicate device-loss signal"
                );
            }
            RecoveryOutcome::Recovered(replacement) => {
                warn!(
                    generation = generation.get(),
                    replacement = replacement.get(),
                    %message,
                    "Recovered lost WGPU device"
                );
                runtime.window().window.request_redraw();
            }
            RecoveryOutcome::Failed => {
                let message = runtime.recovery_error_message();
                error!(%message, "WGPU device recovery failed");
                self.shutdown_once();
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(runtime) = self.ownership.runtime.as_mut() else {
            return;
        };
        let Some(handle_result) =
            dispatch_live_window_event(runtime.window().window.id(), window_id, || {
                runtime.handle_event(&mut self.ownership.application, window_id, &event)
            })
        else {
            return;
        };

        let exit_requested = match handle_result {
            Ok(exit_requested) => exit_requested,
            Err(error) => {
                self.terminate(event_loop, error);
                return;
            }
        };
        if exit_requested {
            self.exit_normally(event_loop);
            return;
        }

        if matches!(event, WindowEvent::RedrawRequested) {
            let render_result = runtime.render(&mut self.ownership.application, &self.config);
            match render_result {
                Ok(true) => self.exit_normally(event_loop),
                Ok(false) => {
                    if matches!(self.config.redraw, RedrawMode::Poll)
                        && let Some(runtime) = self.ownership.runtime.as_ref()
                    {
                        runtime.window().window.request_redraw();
                    }
                }
                Err(error) => self.terminate(event_loop, error),
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        match self.config.redraw {
            RedrawMode::Poll => {
                event_loop.set_control_flow(ControlFlow::Poll);
                if let Some(runtime) = self.ownership.runtime.as_ref() {
                    runtime.window().window.request_redraw();
                }
            }
            RedrawMode::Wait => event_loop.set_control_flow(ControlFlow::Wait),
            RedrawMode::WaitUntil { fps } => {
                let frame = frame_duration(fps);
                let now = Instant::now();
                let mut next_wake = self.last_wake + frame;
                if now >= next_wake {
                    self.last_wake = now;
                    next_wake = now + frame;
                    if let Some(runtime) = self.ownership.runtime.as_ref() {
                        runtime.window().window.request_redraw();
                    }
                }
                event_loop.set_control_flow(ControlFlow::WaitUntil(next_wake));
            }
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.shutdown_once();
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };

    use dear_imgui_rs::FrameLifecycleState;
    use winit::error::EventLoopError;
    use winit::window::WindowId;

    use super::{
        GpuFaultDisposition, OrderedRuntimeOwner, RunnerOwnership, RuntimeOwnershipLifecycle,
        RuntimeShutdownErrors, ShutdownCoordinator, build_and_render_frame,
        classify_uncaptured_gpu_fault, dispatch_live_window_event, finish_runtime_shutdown,
        initialize_runtime_once, resolve_run_result, should_process_runtime_event,
        uncaptured_gpu_fault, validate_config,
    };
    use crate::runtime::state::GpuFaultKind;
    use crate::{AppConfig, GpuGeneration, RunError};
    use dear_imgui_rs::ConfigFlags;

    struct DropProbe {
        event: &'static str,
        events: Rc<RefCell<Vec<&'static str>>>,
    }

    #[test]
    fn uncaptured_gpu_faults_preserve_their_terminal_classification() {
        assert!(matches!(
            uncaptured_gpu_fault(GpuFaultKind::OutOfMemory, "oom".to_owned()),
            RunError::GpuOutOfMemory { message } if message == "oom"
        ));
        assert!(matches!(
            uncaptured_gpu_fault(GpuFaultKind::Validation, "invalid".to_owned()),
            RunError::GpuValidation { message } if message == "invalid"
        ));
        assert!(matches!(
            uncaptured_gpu_fault(GpuFaultKind::Internal, "driver".to_owned()),
            RunError::GpuInternal { message } if message == "driver"
        ));
    }

    #[test]
    fn uncaptured_gpu_fault_dispatch_is_generation_bound() {
        let current = GpuGeneration(8);
        assert!(matches!(
            classify_uncaptured_gpu_fault(
                Some(current),
                GpuGeneration(7),
                GpuFaultKind::Validation,
                "stale".to_owned(),
            ),
            GpuFaultDisposition::IgnoreStale
        ));
        assert!(matches!(
            classify_uncaptured_gpu_fault(
                Some(current),
                current,
                GpuFaultKind::OutOfMemory,
                "live oom".to_owned(),
            ),
            GpuFaultDisposition::Terminate(RunError::GpuOutOfMemory { message })
                if message == "live oom"
        ));
        assert!(matches!(
            classify_uncaptured_gpu_fault(
                None,
                current,
                GpuFaultKind::Internal,
                "early".to_owned(),
            ),
            GpuFaultDisposition::Terminate(RunError::Recovery { message })
                if message.contains("before runtime initialization")
        ));
    }

    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.events.borrow_mut().push(self.event);
        }
    }

    #[test]
    fn runner_ownership_drops_runtime_before_application_during_unwind() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let unwind = catch_unwind(AssertUnwindSafe({
            let events = Rc::clone(&events);
            move || {
                let _ownership = RunnerOwnership {
                    runtime: Some(DropProbe {
                        event: "drop_runtime",
                        events: Rc::clone(&events),
                    }),
                    application: DropProbe {
                        event: "drop_application",
                        events,
                    },
                };
                panic!("injected runner callback panic");
            }
        }));

        assert!(unwind.is_err());
        assert_eq!(*events.borrow(), ["drop_runtime", "drop_application"]);
    }

    struct ProbeRuntimeOwnership {
        events: Rc<RefCell<Vec<&'static str>>>,
        renderer_release: ProbeRelease,
        platform_release: ProbeRelease,
        renderer: DropProbe,
        platform: DropProbe,
        context: DropProbe,
        window: DropProbe,
    }

    #[derive(Clone, Copy)]
    enum ProbeRelease {
        Succeeds,
        Fails,
        Panics,
    }

    impl ProbeRuntimeOwnership {
        fn new(events: Rc<RefCell<Vec<&'static str>>>, renderer_release: ProbeRelease) -> Self {
            Self::with_platform_release(events, renderer_release, ProbeRelease::Succeeds)
        }

        fn with_platform_release(
            events: Rc<RefCell<Vec<&'static str>>>,
            renderer_release: ProbeRelease,
            platform_release: ProbeRelease,
        ) -> Self {
            Self {
                events: Rc::clone(&events),
                renderer_release,
                platform_release,
                renderer: DropProbe {
                    event: "drop_renderer",
                    events: Rc::clone(&events),
                },
                platform: DropProbe {
                    event: "drop_platform",
                    events: Rc::clone(&events),
                },
                context: DropProbe {
                    event: "drop_context",
                    events: Rc::clone(&events),
                },
                window: DropProbe {
                    event: "drop_window",
                    events,
                },
            }
        }
    }

    impl RuntimeOwnershipLifecycle for ProbeRuntimeOwnership {
        fn release_renderer(&mut self) -> Result<(), RunError> {
            self.events.borrow_mut().push("release_renderer");
            match self.renderer_release {
                ProbeRelease::Succeeds => Ok(()),
                ProbeRelease::Fails => Err(RunError::application(
                    "renderer release",
                    "injected release failure",
                )),
                ProbeRelease::Panics => panic!("injected renderer release panic"),
            }
        }

        fn release_platform(&mut self) -> Result<(), RunError> {
            self.events.borrow_mut().push("release_platform");
            match self.platform_release {
                ProbeRelease::Succeeds => Ok(()),
                ProbeRelease::Fails => Err(RunError::application(
                    "platform release",
                    "injected release failure",
                )),
                ProbeRelease::Panics => panic!("injected platform release panic"),
            }
        }

        fn teardown_after_backend_release(self) {
            let Self {
                events: _,
                renderer_release: _,
                platform_release: _,
                renderer,
                platform,
                context,
                window,
            } = self;
            drop(renderer);
            drop(platform);
            drop(context);
            drop(window);
        }
    }

    #[test]
    fn runtime_owner_drop_releases_renderer_before_context_and_window() {
        let events = Rc::new(RefCell::new(Vec::new()));
        drop(OrderedRuntimeOwner::new(ProbeRuntimeOwnership::new(
            Rc::clone(&events),
            ProbeRelease::Succeeds,
        )));

        assert_eq!(
            *events.borrow(),
            [
                "release_renderer",
                "release_platform",
                "drop_renderer",
                "drop_platform",
                "drop_context",
                "drop_window",
            ]
        );
    }

    #[test]
    fn explicit_runtime_owner_teardown_uses_the_same_order_once() {
        let events = Rc::new(RefCell::new(Vec::new()));
        OrderedRuntimeOwner::new(ProbeRuntimeOwnership::new(
            Rc::clone(&events),
            ProbeRelease::Succeeds,
        ))
        .teardown()
        .expect("explicit renderer release should succeed");

        assert_eq!(
            *events.borrow(),
            [
                "release_renderer",
                "release_platform",
                "drop_renderer",
                "drop_platform",
                "drop_context",
                "drop_window",
            ]
        );
    }

    #[test]
    fn runtime_shutdown_reports_application_failure_after_ordered_backend_release() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let owner = OrderedRuntimeOwner::new(ProbeRuntimeOwnership::new(
            Rc::clone(&events),
            ProbeRelease::Succeeds,
        ));

        let errors = finish_runtime_shutdown(
            None,
            || {
                events.borrow_mut().push("application_shutdown");
                Some(RunError::application(
                    "shutdown",
                    "injected application failure",
                ))
            },
            || owner.teardown(),
        );

        assert_eq!(
            *events.borrow(),
            [
                "application_shutdown",
                "release_renderer",
                "release_platform",
                "drop_renderer",
                "drop_platform",
                "drop_context",
                "drop_window",
            ]
        );
        assert!(errors.terminal_error.is_none());
        assert_eq!(
            errors
                .shutdown_error
                .expect("application shutdown failure must remain reportable")
                .to_string(),
            "application callback failed during shutdown: injected application failure"
        );
    }

    #[test]
    fn runtime_shutdown_quarantines_context_after_platform_release_failure() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let owner = OrderedRuntimeOwner::new(ProbeRuntimeOwnership::with_platform_release(
            Rc::clone(&events),
            ProbeRelease::Succeeds,
            ProbeRelease::Fails,
        ));

        let errors = finish_runtime_shutdown(
            None,
            || {
                events.borrow_mut().push("application_shutdown");
                None
            },
            || owner.teardown(),
        );

        assert_eq!(
            *events.borrow(),
            [
                "application_shutdown",
                "release_renderer",
                "release_platform"
            ]
        );
        assert!(errors.shutdown_error.is_none());
        assert_eq!(
            errors
                .terminal_error
                .expect("platform release failure must be reportable")
                .to_string(),
            "application callback failed during platform release: injected release failure"
        );
    }

    #[test]
    fn runtime_owner_uses_ordered_teardown_during_application_panic_unwind() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let unwind = catch_unwind(AssertUnwindSafe({
            let events = Rc::clone(&events);
            move || {
                let _owner = OrderedRuntimeOwner::new(ProbeRuntimeOwnership::new(
                    events,
                    ProbeRelease::Succeeds,
                ));
                panic!("injected application callback panic");
            }
        }));

        assert!(unwind.is_err());
        assert_eq!(
            *events.borrow(),
            [
                "release_renderer",
                "release_platform",
                "drop_renderer",
                "drop_platform",
                "drop_context",
                "drop_window",
            ]
        );
    }

    #[test]
    fn runtime_owner_quarantines_the_complete_graph_when_renderer_release_fails() {
        let events = Rc::new(RefCell::new(Vec::new()));
        drop(OrderedRuntimeOwner::new(ProbeRuntimeOwnership::new(
            Rc::clone(&events),
            ProbeRelease::Fails,
        )));

        assert_eq!(*events.borrow(), ["release_renderer"]);
    }

    #[test]
    fn runtime_owner_quarantines_the_complete_graph_when_platform_release_fails() {
        let events = Rc::new(RefCell::new(Vec::new()));
        drop(OrderedRuntimeOwner::new(
            ProbeRuntimeOwnership::with_platform_release(
                Rc::clone(&events),
                ProbeRelease::Succeeds,
                ProbeRelease::Fails,
            ),
        ));

        assert_eq!(*events.borrow(), ["release_renderer", "release_platform"]);
    }

    #[test]
    fn runtime_owner_quarantines_the_complete_graph_when_renderer_release_panics() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let unwind = catch_unwind(AssertUnwindSafe({
            let events = Rc::clone(&events);
            move || {
                drop(OrderedRuntimeOwner::new(ProbeRuntimeOwnership::new(
                    events,
                    ProbeRelease::Panics,
                )));
            }
        }));

        assert!(unwind.is_err());
        assert_eq!(*events.borrow(), ["release_renderer"]);
    }

    #[test]
    fn config_rejects_multi_viewport_before_runtime_initialization() {
        let mut config = AppConfig::default();
        config.io_config_flags = Some(ConfigFlags::VIEWPORTS_ENABLE);

        assert!(matches!(
            validate_config(&config),
            Err(RunError::MultiViewportUnsupported)
        ));
    }

    #[test]
    fn live_context_rejects_multi_viewport_enabled_by_application_callbacks() {
        let _guard = super::super::imgui_test_guard();
        let mut context = dear_imgui_rs::Context::create();
        let mut flags = context.io().config_flags();
        flags.insert(ConfigFlags::VIEWPORTS_ENABLE);
        context.io_mut().set_config_flags(flags);

        assert!(matches!(
            super::super::state::validate_supported_imgui_config(&context),
            Err(RunError::MultiViewportUnsupported)
        ));
    }

    #[test]
    fn application_frame_error_closes_the_active_frame() {
        let _guard = super::super::imgui_test_guard();
        let mut context = dear_imgui_rs::Context::create();
        context.prepare_frame(
            dear_imgui_rs::FramePrepareOptions::new([640.0, 480.0], 1.0 / 60.0)
                .renderer_has_textures(),
        );
        let _ = context.font_atlas().build();

        {
            let result = build_and_render_frame(&mut context, |_ui| {
                Err(RunError::application("frame", "injected frame failure"))
            });
            assert!(result.is_err());
        }
        assert_eq!(context.frame_lifecycle_state(), FrameLifecycleState::Idle);
    }

    #[test]
    fn delayed_device_loss_is_ignored_after_shutdown_or_event_loop_exit() {
        assert!(should_process_runtime_event(false, false));
        assert!(!should_process_runtime_event(true, false));
        assert!(!should_process_runtime_event(false, true));
        assert!(!should_process_runtime_event(true, true));
    }

    #[test]
    fn resumed_initializes_once_and_never_after_shutdown_starts() {
        let calls = Cell::new(0);
        let mut runtime = None;
        let initialize = || {
            calls.set(calls.get() + 1);
            Ok::<_, ()>(())
        };

        assert_eq!(
            initialize_runtime_once(&mut runtime, false, initialize),
            Some(Ok(()))
        );
        assert_eq!(calls.get(), 1);
        assert!(runtime.is_some());

        assert_eq!(
            initialize_runtime_once(&mut runtime, false, initialize),
            None
        );
        assert_eq!(calls.get(), 1);

        runtime = None;
        assert_eq!(
            initialize_runtime_once(&mut runtime, true, initialize),
            None
        );
        assert_eq!(calls.get(), 1);
        assert!(runtime.is_none());
    }

    #[test]
    fn only_the_live_window_id_dispatches_an_event() {
        let live = WindowId::from(41_u64);
        let foreign = WindowId::from(42_u64);
        let calls = Cell::new(0);

        assert_eq!(
            dispatch_live_window_event(live, foreign, || {
                calls.set(calls.get() + 1);
                "foreign"
            }),
            None
        );
        assert_eq!(calls.get(), 0);

        assert_eq!(
            dispatch_live_window_event(live, live, || {
                calls.set(calls.get() + 1);
                "live"
            }),
            Some("live")
        );
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn shutdown_coordinator_hands_off_the_first_runtime_error_exactly_once() {
        struct ProbeRuntime {
            teardown_calls: Rc<Cell<usize>>,
            terminal_error: Option<RunError>,
        }

        impl Drop for ProbeRuntime {
            fn drop(&mut self) {
                self.teardown_calls.set(self.teardown_calls.get() + 1);
            }
        }

        #[derive(Default)]
        struct ProbeApplication {
            shutdown_calls: usize,
        }

        let mut shutdown = ShutdownCoordinator::default();
        let teardown_calls = Rc::new(Cell::new(0));
        let mut runtime = Some(ProbeRuntime {
            teardown_calls: Rc::clone(&teardown_calls),
            terminal_error: Some(RunError::application("runtime", "primary failure")),
        });
        let mut application = ProbeApplication::default();
        for _ in 0..2 {
            shutdown.shutdown_once(
                &mut runtime,
                &mut application,
                |mut runtime, application| {
                    application.shutdown_calls += 1;
                    RuntimeShutdownErrors {
                        terminal_error: runtime.terminal_error.take(),
                        shutdown_error: Some(RunError::application(
                            "shutdown",
                            "secondary failure",
                        )),
                    }
                },
            );
        }

        assert!(shutdown.started());
        assert!(runtime.is_none());
        assert_eq!(application.shutdown_calls, 1);
        assert_eq!(teardown_calls.get(), 1);
        let error = shutdown
            .take_terminal_error()
            .expect("the runtime error must reach the runner owner");
        assert_eq!(
            error.to_string(),
            "application callback failed during runtime: primary failure"
        );
        assert!(shutdown.take_terminal_error().is_none());
        let shutdown_error = shutdown
            .take_shutdown_error()
            .expect("the shutdown error must remain separately observable");
        assert_eq!(
            shutdown_error.to_string(),
            "application callback failed during shutdown: secondary failure"
        );
        assert!(shutdown.take_shutdown_error().is_none());
    }

    #[test]
    fn shutdown_coordinator_does_not_replace_an_earlier_runner_error() {
        let mut shutdown = ShutdownCoordinator::default();
        shutdown.remember_error(RunError::application("runner", "primary failure"));
        let mut runtime = Some(());
        let mut shutdown_calls = 0;

        shutdown.shutdown_once(&mut runtime, &mut shutdown_calls, |_runtime, calls| {
            *calls += 1;
            RuntimeShutdownErrors {
                terminal_error: Some(RunError::application("runtime", "secondary failure")),
                shutdown_error: Some(RunError::application("shutdown", "shutdown failure")),
            }
        });

        assert_eq!(shutdown_calls, 1);
        let error = shutdown
            .take_terminal_error()
            .expect("the first runner error must survive shutdown");
        assert_eq!(
            error.to_string(),
            "application callback failed during runner: primary failure"
        );
        assert_eq!(
            shutdown
                .take_shutdown_error()
                .expect("the separate shutdown error must be retained")
                .to_string(),
            "application callback failed during shutdown: shutdown failure"
        );
    }

    #[test]
    fn run_result_resolution_covers_every_error_combination_in_observed_order() {
        for mask in 0_u8..8 {
            let has_terminal_before_shutdown = mask & 0b001 != 0;
            let event_loop_failed = mask & 0b010 != 0;
            let shutdown_failed = mask & 0b100 != 0;

            let terminal_before_shutdown = has_terminal_before_shutdown
                .then(|| RunError::application("runtime", "runtime failure"));
            let event_loop_result = if event_loop_failed {
                Err(EventLoopError::ExitFailure(73))
            } else {
                Ok(())
            };
            let shutdown_error =
                shutdown_failed.then(|| RunError::application("shutdown", "shutdown failure"));

            let result =
                resolve_run_result(terminal_before_shutdown, event_loop_result, shutdown_error);
            let actual = match result {
                Ok(()) => "ok",
                Err(RunError::Application { stage, .. }) => stage,
                Err(RunError::EventLoop(EventLoopError::ExitFailure(73))) => "event-loop",
                Err(error) => panic!("unexpected run result for mask {mask:#05b}: {error}"),
            };
            let expected = if has_terminal_before_shutdown {
                "runtime"
            } else if event_loop_failed {
                "event-loop"
            } else if shutdown_failed {
                "shutdown"
            } else {
                "ok"
            };
            assert_eq!(actual, expected, "result mask: {mask:#05b}");
        }
    }
}

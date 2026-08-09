use std::time::{Duration, Instant};

use tracing::{error, info, warn};
use winit::{
    application::ApplicationHandler,
    event::{Event, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::WindowId,
};

use super::{
    admission::{SurfaceDispatch, SurfaceRedrawRetry},
    ownership::{OrderedRuntimeOwner, RuntimeOwnership},
    recovery::{
        GenerationRelease, RecoveryEffects, RecoveryOutcome, RuntimeFactory, RuntimeGenerations,
    },
    shutdown::{
        RuntimeShutdownErrors, ShutdownCoordinator, abort_runtime_initialization,
        finish_runtime_shutdown, resolve_run_result,
    },
    state::{
        GpuFaultKind, RuntimeEvent, RuntimeGeneration, UiState, WgpuRuntimeFactory, WindowState,
    },
    surface::render_surface_frame,
};
use crate::{
    AppConfig, Application, ApplicationStage, GpuGeneration, InitContext, RedrawMode, RunError,
    ShutdownContext,
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

pub(super) fn validate_config(config: &AppConfig) -> Result<(), RunError> {
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
        application
            .initialized(&mut init, &mut gpu)
            .map_err(|error| error.during_application_stage(ApplicationStage::Initialized))?;
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
            application
                .event(&mut context)
                .map_err(|error| error.during_application_stage(ApplicationStage::Event))?;
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
    ) -> Result<SurfaceDispatch<bool>, RunError> {
        render_surface_frame(
            self.ownership.get_mut(),
            self.clear_color,
            &mut self.admitted_frame_count,
            application,
            config,
        )
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
        application
            .shutdown(&mut context)
            .map_err(|error| error.during_application_stage(ApplicationStage::Shutdown))
            .err()
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

struct RuntimeRecovery<'a, A> {
    window: &'a mut WindowState,
    ui: &'a mut UiState,
    application: &'a mut A,
    config: &'a AppConfig,
    event_proxy: EventLoopProxy<RuntimeEvent>,
}

impl<A: Application> GenerationRelease<RuntimeGeneration> for RuntimeRecovery<'_, A> {
    fn release_generation(&mut self, generation: &mut RuntimeGeneration) -> Result<(), RunError> {
        generation.gpu.release_renderer(&mut self.ui.context)
    }
}

impl<A: Application> RecoveryEffects<RuntimeGeneration> for RuntimeRecovery<'_, A> {
    fn gpu_lost(&mut self, generation: &mut RuntimeGeneration) -> Result<(), RunError> {
        let mut context = generation.context(self.window)?;
        self.application
            .gpu_lost(&mut context)
            .map_err(|error| error.during_application_stage(ApplicationStage::GpuLost))
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
        self.application
            .gpu_recreated(&mut context)
            .map_err(|error| error.during_application_stage(ApplicationStage::GpuRecreated))
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

struct Runner<A> {
    config: AppConfig,
    ownership: RunnerOwnership<Runtime, A>,
    shutdown: ShutdownCoordinator,
    event_proxy: EventLoopProxy<RuntimeEvent>,
    last_wake: Instant,
    surface_redraw_retry: SurfaceRedrawRetry,
}

/// Keeps renderer-side registrations alive no longer than their application-owned resources.
pub(super) struct RunnerOwnership<R, A> {
    pub(super) runtime: Option<R>,
    pub(super) application: A,
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
            surface_redraw_retry: SurfaceRedrawRetry::default(),
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

pub(super) fn should_process_runtime_event(
    shutdown_called: bool,
    event_loop_exiting: bool,
) -> bool {
    !shutdown_called && !event_loop_exiting
}

pub(super) fn uncaptured_gpu_fault(kind: GpuFaultKind, message: String) -> RunError {
    match kind {
        GpuFaultKind::OutOfMemory => RunError::GpuOutOfMemory { message },
        GpuFaultKind::Validation => RunError::GpuValidation { message },
        GpuFaultKind::Internal => RunError::GpuInternal { message },
    }
}

pub(super) enum GpuFaultDisposition {
    IgnoreStale,
    Terminate(RunError),
}

pub(super) fn classify_uncaptured_gpu_fault(
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

pub(super) fn initialize_runtime_once<R, E>(
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

pub(super) fn dispatch_live_window_event<T>(
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
                Ok(SurfaceDispatch::Presented(true)) => self.exit_normally(event_loop),
                Ok(SurfaceDispatch::Presented(false)) => {
                    self.surface_redraw_retry.reset();
                    if matches!(self.config.redraw, RedrawMode::Poll)
                        && let Some(runtime) = self.ownership.runtime.as_ref()
                    {
                        runtime.window().window.request_redraw();
                    }
                }
                Ok(SurfaceDispatch::Skipped(reason)) => {
                    if reason.should_retry() {
                        self.surface_redraw_retry.schedule(Instant::now());
                    } else {
                        self.surface_redraw_retry.reset();
                    }
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
        let now = Instant::now();
        let retry_due = self.surface_redraw_retry.take_due(now);
        match self.config.redraw {
            RedrawMode::Poll => {
                event_loop.set_control_flow(ControlFlow::Poll);
                if let Some(runtime) = self.ownership.runtime.as_ref() {
                    runtime.window().window.request_redraw();
                }
            }
            RedrawMode::Wait => {
                if retry_due && let Some(runtime) = self.ownership.runtime.as_ref() {
                    runtime.window().window.request_redraw();
                }
                match self.surface_redraw_retry.deadline() {
                    Some(deadline) => event_loop.set_control_flow(ControlFlow::WaitUntil(deadline)),
                    None => event_loop.set_control_flow(ControlFlow::Wait),
                }
            }
            RedrawMode::WaitUntil { fps } => {
                let frame = frame_duration(fps);
                let mut next_wake = self.last_wake + frame;
                let periodic_due = now >= next_wake;
                if periodic_due {
                    self.last_wake = now;
                    next_wake = now + frame;
                }
                if (periodic_due || retry_due)
                    && let Some(runtime) = self.ownership.runtime.as_ref()
                {
                    runtime.window().window.request_redraw();
                }
                if let Some(retry_deadline) = self.surface_redraw_retry.deadline() {
                    next_wake = next_wake.min(retry_deadline);
                }
                event_loop.set_control_flow(ControlFlow::WaitUntil(next_wake));
            }
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.shutdown_once();
    }
}

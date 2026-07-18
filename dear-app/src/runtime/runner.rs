use std::time::{Duration, Instant};

use dear_imgui_rs::{DockFlags, Id, WindowFlags};
use tracing::{error, info, warn};
use winit::{
    application::ApplicationHandler,
    event::{Event, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::WindowId,
};

use super::{
    lifecycle::{LifecycleAction, SurfaceEvent},
    recovery::{RecoveryEffects, RecoveryOutcome, RuntimeFactory, RuntimeGenerations},
    state::{RuntimeEvent, RuntimeGeneration, UiState, WgpuRuntimeFactory, WindowState},
};
use crate::{
    AddOns, AppConfig, Application, DockingApi, FrameContext, GpuGeneration, InitContext,
    PrepareFrameContext, RedrawMode, RunError, ShutdownContext,
};

pub(crate) fn run<A: Application + 'static>(
    config: AppConfig,
    application: A,
) -> Result<(), RunError> {
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
    window: WindowState,
    ui: UiState,
    generations: RuntimeGenerations<RuntimeGeneration>,
    clear_color: wgpu::Color,
}

impl Runtime {
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
                let mut shutdown = ShutdownContext {
                    imgui: &mut ui.context,
                    window: &window.window,
                    generation: None,
                };
                let _ = application.shutdown(&mut shutdown);
                ui.teardown();
                drop(window);
                return Err(error);
            }
        };
        let generations = match RuntimeGenerations::new(generation) {
            Ok(generations) => generations,
            Err(error) => {
                let mut shutdown = ShutdownContext {
                    imgui: &mut ui.context,
                    window: &window.window,
                    generation: None,
                };
                let _ = application.shutdown(&mut shutdown);
                ui.teardown();
                drop(window);
                return Err(error);
            }
        };
        let mut runtime = Self {
            window,
            ui,
            generations,
            clear_color: wgpu::Color {
                r: config.clear_color[0] as f64,
                g: config.clear_color[1] as f64,
                b: config.clear_color[2] as f64,
                a: config.clear_color[3] as f64,
            },
        };

        if let Err(error) = runtime.notify_initialized(application, config) {
            runtime.shutdown_application(application);
            let _ = runtime.teardown();
            return Err(error);
        }
        Ok(runtime)
    }

    fn notify_initialized<A: Application>(
        &mut self,
        application: &mut A,
        config: &AppConfig,
    ) -> Result<(), RunError> {
        let generation = self
            .generations
            .current_mut()
            .ok_or_else(|| RunError::Recovery {
                message: "initialized callback requested without a GPU generation".to_owned(),
            })?;
        let mut init = InitContext {
            imgui: &mut self.ui.context,
            window: &self.window.window,
            config,
        };
        let mut gpu = generation.context(&self.window)?;
        application.initialized(&mut init, &mut gpu)
    }

    fn handle_event<A: Application>(
        &mut self,
        application: &mut A,
        window_id: WindowId,
        event: &WindowEvent,
    ) -> Result<bool, RunError> {
        let full_event: Event<RuntimeEvent> = Event::WindowEvent {
            window_id,
            event: event.clone(),
        };
        self.ui
            .platform
            .handle_event(&mut self.ui.context, &self.window.window, &full_event);

        let mut exit_requested = matches!(event, WindowEvent::CloseRequested);
        {
            let mut context = crate::EventContext {
                event,
                imgui: &mut self.ui.context,
                window: &self.window.window,
                exit_requested: &mut exit_requested,
            };
            application.event(&mut context)?;
        }

        let Some(generation) = self.generations.current() else {
            return Ok(exit_requested);
        };
        match event {
            WindowEvent::Resized(size) => {
                self.window.resize(*size, &generation.gpu.device);
                self.window.window.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                let size = self.window.window.inner_size();
                self.window.resize(size, &generation.gpu.device);
                self.window.window.request_redraw();
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
        let generation = self
            .generations
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
        } = &mut self.ui;

        let mut prepare_frame = PrepareFrameContext {
            imgui: context,
            window: &self.window.window,
        };
        application.prepare_frame(&mut prepare_frame)?;
        platform.prepare_frame(&self.window.window, context);
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
                marker: std::marker::PhantomData,
            };
            let mut frame = FrameContext {
                ui,
                addons,
                gpu: generation.api(),
                exit_requested: &mut exit_requested,
            };
            application.frame(&mut frame)?;
            platform.prepare_render_with_ui(ui, &self.window.window);
            Ok(())
        })?;

        let (frame, reconfigure_after_present) = match self.window.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => {
                let action = self.generations.surface_event(SurfaceEvent::Success);
                debug_assert_eq!(action, LifecycleAction::Render);
                (frame, false)
            }
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                let action = self.generations.surface_event(SurfaceEvent::Suboptimal);
                debug_assert_eq!(action, LifecycleAction::RenderAndReconfigure);
                (frame, true)
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                let action = self.generations.surface_event(SurfaceEvent::Lost);
                debug_assert_eq!(action, LifecycleAction::RecreateSurface);
                let generation = self
                    .generations
                    .current()
                    .ok_or_else(|| RunError::Recovery {
                        message: "surface recreation requested without an active GPU generation"
                            .to_owned(),
                    })?;
                self.window.recreate_surface(
                    &generation.gpu.adapter,
                    &generation.gpu.device,
                    config,
                )?;
                return Ok(exit_requested);
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                let action = self.generations.surface_event(SurfaceEvent::Outdated);
                debug_assert_eq!(action, LifecycleAction::ReconfigureSurface);
                let generation = self
                    .generations
                    .current()
                    .ok_or_else(|| RunError::Recovery {
                        message:
                            "surface reconfiguration requested without an active GPU generation"
                                .to_owned(),
                    })?;
                self.window.reconfigure(&generation.gpu.device);
                return Ok(exit_requested);
            }
            wgpu::CurrentSurfaceTexture::Timeout => {
                let action = self.generations.surface_event(SurfaceEvent::Timeout);
                debug_assert_eq!(action, LifecycleAction::SkipFrame);
                return Ok(exit_requested);
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                let action = self.generations.surface_event(SurfaceEvent::Occluded);
                debug_assert_eq!(action, LifecycleAction::SkipFrame);
                return Ok(exit_requested);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                let action = self.generations.surface_event(SurfaceEvent::Validation);
                debug_assert_eq!(action, LifecycleAction::Exit);
                return Err(RunError::SurfaceValidation);
            }
        };

        let generation = self
            .generations
            .current_mut()
            .ok_or_else(|| RunError::Recovery {
                message: "render submission requested without an active GPU generation".to_owned(),
            })?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            generation
                .gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Dear App render encoder"),
                });
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
            generation
                .gpu
                .renderer
                .new_frame()
                .map_err(RunError::FramePrepare)?;
            generation
                .gpu
                .renderer
                .render(draw_data, &mut render_pass)
                .map_err(RunError::Render)?;
        }
        generation.gpu.queue.submit(Some(encoder.finish()));
        generation.gpu.queue.present(frame);
        if reconfigure_after_present {
            self.window.reconfigure(&generation.gpu.device);
        }
        Ok(exit_requested)
    }

    fn recover<A: Application>(
        &mut self,
        application: &mut A,
        config: &AppConfig,
        event_proxy: EventLoopProxy<RuntimeEvent>,
        signal_generation: GpuGeneration,
    ) -> RecoveryOutcome {
        let mut environment = RuntimeRecovery {
            window: &mut self.window,
            ui: &mut self.ui,
            application,
            config,
            event_proxy,
        };
        let mut factory = WgpuRuntimeFactory;
        self.generations
            .recover(signal_generation, &mut environment, &mut factory)
    }

    fn recovery_error_message(&self) -> String {
        self.generations
            .terminal_error()
            .map(ToString::to_string)
            .unwrap_or_else(|| "GPU recovery failed without a terminal error".to_owned())
    }

    fn shutdown_application<A: Application>(&mut self, application: &mut A) -> Option<RunError> {
        let generation = self.generations.current_generation();
        let mut context = ShutdownContext {
            imgui: &mut self.ui.context,
            window: &self.window.window,
            generation,
        };
        application.shutdown(&mut context).err()
    }

    fn fail(&mut self, error: RunError) {
        self.generations.fail(error);
    }

    fn teardown(mut self) -> Option<RunError> {
        self.generations.shutdown();
        let terminal_error = self.generations.take_terminal_error();
        self.ui.teardown();
        drop(self.window);
        terminal_error
    }

    fn shutdown<A: Application>(mut self, application: &mut A) -> RuntimeShutdownErrors {
        let terminal_error = self.generations.take_terminal_error();
        let shutdown_error = self.shutdown_application(application);
        self.generations.shutdown();
        self.ui.teardown();
        drop(self.window);
        RuntimeShutdownErrors {
            terminal_error,
            shutdown_error,
        }
    }
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

fn draw_dockspace(ui: &dear_imgui_rs::Ui, flags: DockFlags, config: &AppConfig) {
    if !config.docking.enable || !config.docking.auto_dockspace {
        return;
    }

    let viewport = ui.main_viewport();
    ui.set_next_window_viewport(viewport.id());
    let mut window_flags = config.docking.host_window_flags;
    if flags.contains(DockFlags::PASSTHRU_CENTRAL_NODE) {
        window_flags |= WindowFlags::NO_BACKGROUND;
    }
    ui.window(config.docking.host_window_name)
        .flags(window_flags)
        .position(viewport.pos(), dear_imgui_rs::Condition::Always)
        .size(viewport.size(), dear_imgui_rs::Condition::Always)
        .build(|| {
            let _ = ui.dockspace_over_main_viewport_with_flags(Id::from(0_u32), flags);
        });
}

struct Runner<A> {
    config: AppConfig,
    application: A,
    runtime: Option<Runtime>,
    shutdown: ShutdownCoordinator,
    event_proxy: EventLoopProxy<RuntimeEvent>,
    last_wake: Instant,
}

impl<A: Application> Runner<A> {
    fn new(config: AppConfig, application: A, event_proxy: EventLoopProxy<RuntimeEvent>) -> Self {
        Self {
            config,
            application,
            runtime: None,
            shutdown: ShutdownCoordinator::default(),
            event_proxy,
            last_wake: Instant::now(),
        }
    }

    fn terminate(&mut self, event_loop: &ActiveEventLoop, error: RunError) {
        error!("Dear App terminated: {error}");
        if let Some(runtime) = self.runtime.as_mut() {
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
        self.shutdown
            .shutdown_once(&mut self.runtime, &mut self.application, Runtime::shutdown);
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
        match initialize_runtime_once(&mut self.runtime, shutdown_started, || {
            Runtime::new(event_loop, event_proxy, &self.config, &mut self.application)
        }) {
            Some(Ok(())) => {
                info!("Dear App window and initial GPU generation are ready");
                self.runtime
                    .as_ref()
                    .expect("successful initialization stores the runtime")
                    .window
                    .window
                    .request_redraw();
            }
            Some(Err(error)) => self.terminate(event_loop, error),
            None => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: RuntimeEvent) {
        let RuntimeEvent::DeviceLost {
            generation,
            message,
        } = event;

        if !should_process_runtime_event(self.shutdown.started(), event_loop.exiting()) {
            warn!(
                generation = generation.get(),
                "Ignoring device-loss signal after runtime shutdown"
            );
            return;
        }

        let Some(runtime) = self.runtime.as_mut() else {
            self.terminate(
                event_loop,
                RunError::Recovery {
                    message: "device loss received before runtime initialization".to_owned(),
                },
            );
            return;
        };
        match runtime.recover(
            &mut self.application,
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
                runtime.window.window.request_redraw();
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
        let Some(runtime) = self.runtime.as_mut() else {
            return;
        };
        let Some(handle_result) =
            dispatch_live_window_event(runtime.window.window.id(), window_id, || {
                runtime.handle_event(&mut self.application, window_id, &event)
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
            let render_result = runtime.render(&mut self.application, &self.config);
            match render_result {
                Ok(true) => self.exit_normally(event_loop),
                Ok(false) => {
                    if matches!(self.config.redraw, RedrawMode::Poll)
                        && let Some(runtime) = self.runtime.as_ref()
                    {
                        runtime.window.window.request_redraw();
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
                if let Some(runtime) = self.runtime.as_ref() {
                    runtime.window.window.request_redraw();
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
                    if let Some(runtime) = self.runtime.as_ref() {
                        runtime.window.window.request_redraw();
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
    use std::{cell::Cell, rc::Rc};

    use dear_imgui_rs::FrameLifecycleState;
    use winit::error::EventLoopError;
    use winit::window::WindowId;

    use super::{
        RuntimeShutdownErrors, ShutdownCoordinator, build_and_render_frame,
        dispatch_live_window_event, initialize_runtime_once, resolve_run_result,
        should_process_runtime_event,
    };
    use crate::RunError;

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

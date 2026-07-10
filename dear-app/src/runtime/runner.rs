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
    lifecycle::{LifecycleAction, LifecycleMachine, SurfaceEvent},
    managed_textures::reset_for_new_gpu_generation,
    recovery::{RecoveryHooks, execute_recovery},
    state::{
        RuntimeEvent, RuntimeFactory, RuntimeGeneration, UiState, WgpuRuntimeFactory, WindowState,
    },
};
use crate::{
    AddOns, AppConfig, Application, DockingApi, FrameContext, GpuGeneration, InitContext,
    RedrawMode, RunError, ShutdownContext,
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
    runner.finish()?;
    event_loop_result?;
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
    window: WindowState,
    ui: UiState,
    generation: Option<RuntimeGeneration>,
    clear_color: wgpu::Color,
}

impl Runtime {
    fn new<A: Application>(
        event_loop: &ActiveEventLoop,
        event_proxy: EventLoopProxy<RuntimeEvent>,
        config: &AppConfig,
        application: &mut A,
    ) -> Result<Self, RunError> {
        let window = WindowState::new(event_loop, config)?;
        let ui = UiState::new(&window, config, application)?;
        let mut runtime = Self {
            window,
            ui,
            generation: None,
            clear_color: wgpu::Color {
                r: config.clear_color[0] as f64,
                g: config.clear_color[1] as f64,
                b: config.clear_color[2] as f64,
                a: config.clear_color[3] as f64,
            },
        };

        let initial_generation = {
            let mut factory = WgpuRuntimeFactory {
                window: &mut runtime.window,
                ui: &mut runtime.ui,
                config,
                event_proxy,
            };
            factory.create(GpuGeneration::INITIAL)
        };
        let generation = match initial_generation {
            Ok(generation) => generation,
            Err(error) => {
                runtime.shutdown_application(application);
                runtime.teardown();
                return Err(error);
            }
        };
        runtime.generation = Some(generation);

        if let Err(error) = runtime.notify_initialized(application, config) {
            runtime.shutdown_application(application);
            runtime.teardown();
            return Err(error);
        }
        Ok(runtime)
    }

    fn notify_initialized<A: Application>(
        &mut self,
        application: &mut A,
        config: &AppConfig,
    ) -> Result<(), RunError> {
        let generation = self.generation.as_mut().ok_or_else(|| RunError::Recovery {
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

        let Some(generation) = self.generation.as_ref() else {
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
        lifecycle: &mut LifecycleMachine,
    ) -> Result<bool, RunError> {
        let generation = self.generation.as_mut().ok_or_else(|| RunError::Recovery {
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
                let action = lifecycle.surface_event(SurfaceEvent::Success);
                debug_assert_eq!(action, LifecycleAction::Render);
                (frame, false)
            }
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                let action = lifecycle.surface_event(SurfaceEvent::Suboptimal);
                debug_assert_eq!(action, LifecycleAction::RenderAndReconfigure);
                (frame, true)
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                let action = lifecycle.surface_event(SurfaceEvent::Lost);
                debug_assert_eq!(action, LifecycleAction::RecreateSurface);
                self.window.recreate_surface(
                    &generation.gpu.adapter,
                    &generation.gpu.device,
                    config,
                )?;
                return Ok(exit_requested);
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                let action = lifecycle.surface_event(SurfaceEvent::Outdated);
                debug_assert_eq!(action, LifecycleAction::ReconfigureSurface);
                self.window.reconfigure(&generation.gpu.device);
                return Ok(exit_requested);
            }
            wgpu::CurrentSurfaceTexture::Timeout => {
                let action = lifecycle.surface_event(SurfaceEvent::Timeout);
                debug_assert_eq!(action, LifecycleAction::SkipFrame);
                return Ok(exit_requested);
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                let action = lifecycle.surface_event(SurfaceEvent::Occluded);
                debug_assert_eq!(action, LifecycleAction::SkipFrame);
                return Ok(exit_requested);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                let action = lifecycle.surface_event(SurfaceEvent::Validation);
                debug_assert_eq!(action, LifecycleAction::Exit);
                return Err(RunError::SurfaceValidation);
            }
        };

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
                .render_draw_data(draw_data, &mut render_pass)
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
        lifecycle: &mut LifecycleMachine,
    ) -> Result<(), RunError> {
        let mut hooks = RuntimeRecovery {
            runtime: self,
            application,
            config,
            event_proxy,
            lifecycle,
        };
        execute_recovery(&mut hooks)
    }

    fn shutdown_application<A: Application>(&mut self, application: &mut A) -> Option<RunError> {
        let generation = self.generation.as_ref().map(|generation| generation.id);
        let mut context = ShutdownContext {
            imgui: &mut self.ui.context,
            window: &self.window.window,
            generation,
        };
        application.shutdown(&mut context).err()
    }

    fn teardown(mut self) {
        if let Some(generation) = self.generation.take() {
            generation.teardown();
        }
        self.ui.teardown();
        drop(self.window);
    }
}

fn build_and_render_frame(
    context: &mut dear_imgui_rs::Context,
    build: impl FnOnce(&dear_imgui_rs::Ui) -> Result<(), RunError>,
) -> Result<&mut dear_imgui_rs::render::DrawData, RunError> {
    let frame = context.begin_frame();
    build(frame.ui())?;
    Ok(frame.render())
}

struct RuntimeRecovery<'a, A> {
    runtime: &'a mut Runtime,
    application: &'a mut A,
    config: &'a AppConfig,
    event_proxy: EventLoopProxy<RuntimeEvent>,
    lifecycle: &'a mut LifecycleMachine,
}

impl<A: Application> RecoveryHooks for RuntimeRecovery<'_, A> {
    type Candidate = RuntimeGeneration;

    fn pending_generation(&self) -> Result<GpuGeneration, RunError> {
        self.lifecycle
            .pending_generation()
            .map_err(|error| RunError::Recovery {
                message: format!("cannot allocate the next GPU generation: {error:?}"),
            })
    }

    fn gpu_lost(&mut self) -> Result<(), RunError> {
        let generation = self
            .runtime
            .generation
            .as_mut()
            .ok_or_else(|| RunError::Recovery {
                message: "device loss received without an active GPU generation".to_owned(),
            })?;
        let mut context = generation.context(&self.runtime.window)?;
        self.application.gpu_lost(&mut context)
    }

    fn invalidate_resources(&mut self) -> Result<(), RunError> {
        reset_for_new_gpu_generation(&mut self.runtime.ui.context);
        self.runtime
            .generation
            .as_mut()
            .ok_or_else(|| RunError::Recovery {
                message: "GPU generation disappeared before resource invalidation".to_owned(),
            })?
            .gpu
            .renderer
            .invalidate_device_objects()
            .map_err(RunError::GpuInvalidation)
    }

    fn teardown_old_gpu(&mut self) {
        if let Some(generation) = self.runtime.generation.take() {
            generation.teardown();
        }
    }

    fn build_candidate(&mut self, generation: GpuGeneration) -> Result<Self::Candidate, RunError> {
        let mut factory = WgpuRuntimeFactory {
            window: &mut self.runtime.window,
            ui: &mut self.runtime.ui,
            config: self.config,
            event_proxy: self.event_proxy.clone(),
        };
        factory.create(generation)
    }

    fn commit_candidate(&mut self, candidate: Self::Candidate) {
        self.runtime.generation = Some(candidate);
    }

    fn advance_generation(&mut self) -> Result<GpuGeneration, RunError> {
        let committed =
            self.lifecycle
                .recovery_succeeded()
                .map_err(|error| RunError::Recovery {
                    message: format!("cannot commit GPU generation: {error:?}"),
                })?;
        if self
            .runtime
            .generation
            .as_ref()
            .map(|generation| generation.id)
            != Some(committed)
        {
            return Err(RunError::Recovery {
                message: "candidate generation does not match lifecycle generation".to_owned(),
            });
        }
        Ok(committed)
    }

    fn gpu_recreated(&mut self, generation: GpuGeneration) -> Result<(), RunError> {
        let candidate = self
            .runtime
            .generation
            .as_mut()
            .filter(|candidate| candidate.id == generation)
            .ok_or_else(|| RunError::Recovery {
                message: "ready callback requested for a missing GPU generation".to_owned(),
            })?;
        let mut context = candidate.context(&self.runtime.window)?;
        self.application.gpu_recreated(&mut context)
    }

    fn recovery_failed(&mut self) {
        let _ = self.lifecycle.recovery_failed();
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
    lifecycle: LifecycleMachine,
    event_proxy: EventLoopProxy<RuntimeEvent>,
    shutdown_called: bool,
    last_wake: Instant,
}

impl<A: Application> Runner<A> {
    fn new(config: AppConfig, application: A, event_proxy: EventLoopProxy<RuntimeEvent>) -> Self {
        Self {
            config,
            application,
            runtime: None,
            lifecycle: LifecycleMachine::new(),
            event_proxy,
            shutdown_called: false,
            last_wake: Instant::now(),
        }
    }

    fn terminate(&mut self, event_loop: &ActiveEventLoop, error: RunError) {
        error!("Dear App terminated: {error}");
        self.lifecycle.fail(error);
        self.shutdown_once();
        event_loop.exit();
    }

    fn exit_normally(&mut self, event_loop: &ActiveEventLoop) {
        self.shutdown_once();
        event_loop.exit();
    }

    fn shutdown_once(&mut self) {
        if !begin_once(&mut self.shutdown_called) {
            return;
        }

        if let Some(runtime) = self.runtime.as_mut()
            && let Some(error) = runtime.shutdown_application(&mut self.application)
        {
            self.lifecycle.fail(error);
        }
        if let Some(runtime) = self.runtime.take() {
            runtime.teardown();
        }
        self.lifecycle.shutdown();
    }

    fn finish(mut self) -> Result<(), RunError> {
        match self.lifecycle.take_terminal_error() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

fn begin_once(started: &mut bool) -> bool {
    if *started {
        false
    } else {
        *started = true;
        true
    }
}

impl<A: Application + 'static> ApplicationHandler<RuntimeEvent> for Runner<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.runtime.is_some() || self.shutdown_called {
            return;
        }

        match Runtime::new(
            event_loop,
            self.event_proxy.clone(),
            &self.config,
            &mut self.application,
        ) {
            Ok(runtime) => {
                info!("Dear App window and initial GPU generation are ready");
                runtime.window.window.request_redraw();
                self.runtime = Some(runtime);
            }
            Err(error) => self.terminate(event_loop, error),
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: RuntimeEvent) {
        let RuntimeEvent::DeviceLost {
            generation,
            message,
        } = event;
        if self.lifecycle.device_lost(generation) != LifecycleAction::RecoverGpu {
            warn!(
                generation = generation.get(),
                "Ignoring stale or duplicate device-loss signal"
            );
            return;
        }
        warn!(generation = generation.get(), %message, "Recovering lost WGPU device");

        let Some(runtime) = self.runtime.as_mut() else {
            self.terminate(
                event_loop,
                RunError::Recovery {
                    message: "device loss received before runtime initialization".to_owned(),
                },
            );
            return;
        };
        if let Err(error) = runtime.recover(
            &mut self.application,
            &self.config,
            self.event_proxy.clone(),
            &mut self.lifecycle,
        ) {
            self.terminate(event_loop, error);
            return;
        }
        runtime.window.window.request_redraw();
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
        if runtime.window.window.id() != window_id {
            return;
        }

        let exit_requested = match runtime.handle_event(&mut self.application, window_id, &event) {
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
            let render_result =
                runtime.render(&mut self.application, &self.config, &mut self.lifecycle);
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
    use dear_imgui_rs::FrameLifecycleState;

    use super::{begin_once, build_and_render_frame};
    use crate::RunError;

    #[test]
    fn application_frame_error_closes_frame_before_shutdown_once() {
        let mut context = dear_imgui_rs::Context::create();
        context.prepare_frame(
            dear_imgui_rs::FramePrepareOptions::new([640.0, 480.0], 1.0 / 60.0)
                .renderer_has_textures(),
        );
        let _ = context.font_atlas_mut().build();

        let result = build_and_render_frame(&mut context, |_ui| {
            Err(RunError::application("frame", "injected frame failure"))
        });
        assert!(result.is_err());
        assert_eq!(context.frame_lifecycle_state(), FrameLifecycleState::Idle);

        let mut shutdown_started = false;
        let mut shutdown_calls = 0;
        for _ in 0..2 {
            if begin_once(&mut shutdown_started) {
                assert_eq!(context.frame_lifecycle_state(), FrameLifecycleState::Idle);
                shutdown_calls += 1;
            }
        }
        assert_eq!(shutdown_calls, 1);
    }
}

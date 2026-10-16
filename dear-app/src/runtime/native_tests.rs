use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use winit::platform::pump_events::EventLoopExtPumpEvents;

use super::*;
use crate::{ExternalTextureHandle, FrameContext, GpuContext, RenderContext};

struct NativeApplication {
    frames: usize,
    recordings: usize,
    inputs: usize,
    background_ready: Arc<AtomicBool>,
    deadline: Instant,
    texture: Option<wgpu::Texture>,
    external: Option<ExternalTextureHandle>,
    readback: Option<wgpu::Buffer>,
    verified: bool,
}

impl Application for NativeApplication {
    fn initialized(
        &mut self,
        context: &mut InitializedContext<'_>,
        gpu: &mut GpuContext<'_>,
    ) -> Result<(), RunError> {
        self.deadline = Instant::now() + Duration::from_millis(300);
        let device = gpu.gpu().device();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("native dear-app producer"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        self.readback = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("native dear-app producer readback"),
            size: 256,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        self.external = Some(
            gpu.gpu()
                .register_external_texture(&texture.create_view(&Default::default()))
                .unwrap(),
        );
        self.texture = Some(texture);
        let ready = Arc::clone(&self.background_ready);
        let wake = context.wake_handle();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(40));
            ready.store(true, Ordering::Release);
            let _ = wake.request_redraw();
        });
        Ok(())
    }

    fn device_event(
        &mut self,
        context: &mut crate::DeviceEventContext<'_>,
    ) -> Result<(), RunError> {
        assert!(matches!(
            context.event(),
            DeviceEvent::MouseMotion { delta: (1.0, 2.0) }
        ));
        self.inputs += 1;
        context.wake_handle().request_redraw().unwrap();
        Ok(())
    }

    fn frame(&mut self, context: &mut FrameContext<'_>) -> Result<(), RunError> {
        self.frames += 1;
        if self.recordings > 0 && !self.verified {
            let buffer = self.readback.as_ref().unwrap();
            let (sender, receiver) = std::sync::mpsc::sync_channel(1);
            buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = sender.send(result);
                });
            context
                .gpu()
                .device()
                .poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: Some(Duration::from_secs(3)),
                })
                .unwrap();
            receiver
                .recv_timeout(Duration::from_secs(3))
                .unwrap()
                .unwrap();
            {
                let bytes = buffer.slice(..).get_mapped_range().unwrap();
                assert_eq!(
                    &bytes[..4],
                    &[255, 0, 0, 255],
                    "engine commands must reach the submitted encoder"
                );
            }
            buffer.unmap();
            self.verified = true;
        }
        let texture = context
            .gpu()
            .resolve_external_texture(self.external.unwrap())
            .unwrap();
        context.ui().get_foreground_draw_list().add_image(
            texture,
            [0.0, 0.0],
            [32.0, 32.0],
            [0.0, 0.0],
            [1.0, 1.0],
            dear_imgui_rs::Color::WHITE,
        );
        if self.verified
            && self.background_ready.load(Ordering::Acquire)
            && Instant::now() >= self.deadline
        {
            context.request_exit();
        } else {
            context
                .wake_handle()
                .request_redraw_at(self.deadline)
                .unwrap();
        }
        Ok(())
    }

    fn record_render(&mut self, context: &mut RenderContext<'_>) -> Result<(), RunError> {
        if self.recordings == 0 {
            let texture = self.texture.as_ref().unwrap();
            let view = texture.create_view(&Default::default());
            {
                let _pass = context
                    .encoder()
                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("native dear-app scene producer"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::RED),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        ..Default::default()
                    });
            }
            context.encoder().copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: self.readback.as_ref().unwrap(),
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(1),
                    },
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
        }
        self.recordings += 1;
        Ok(())
    }
}

struct NativeProbe(Runner<NativeApplication>);

impl ApplicationHandler<RuntimeEvent> for NativeProbe {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.0.resumed(event_loop);
        self.0.device_event(
            event_loop,
            DeviceId::dummy(),
            DeviceEvent::MouseMotion { delta: (1.0, 2.0) },
        );
    }
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: RuntimeEvent) {
        self.0.user_event(event_loop, event);
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        self.0.window_event(event_loop, id, event);
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.0.about_to_wait(event_loop);
    }
    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        self.0.exiting(event_loop);
    }
}

#[test]
#[ignore = "requires a native desktop and GPU surface"]
fn native_wait_wakes_raw_input_and_engine_recording() {
    let _guard = crate::runtime::imgui_test_guard();
    let mut builder = EventLoop::<RuntimeEvent>::with_user_event();
    #[cfg(target_os = "windows")]
    {
        use winit::platform::windows::EventLoopBuilderExtWindows;
        builder.with_any_thread(true);
    }
    #[cfg(target_os = "linux")]
    {
        use winit::platform::x11::EventLoopBuilderExtX11;
        builder.with_x11().with_any_thread(true);
    }
    let mut event_loop = builder.build().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    let config = AppConfig {
        window_title: "dear-app bounded native contract".to_owned(),
        window_size: (320.0, 240.0),
        redraw: RedrawMode::Wait,
        restore_previous_geometry: false,
        ..Default::default()
    };
    let application = NativeApplication {
        frames: 0,
        recordings: 0,
        inputs: 0,
        background_ready: Arc::new(AtomicBool::new(false)),
        deadline: Instant::now() + Duration::from_millis(300),
        texture: None,
        external: None,
        readback: None,
        verified: false,
    };
    let mut probe = NativeProbe(Runner::new(config, application, event_loop.create_proxy()));
    let timeout = Instant::now() + Duration::from_secs(10);
    while !probe.0.shutdown.started() && Instant::now() < timeout {
        event_loop.pump_app_events(Some(Duration::from_millis(50)), &mut probe);
    }
    let exited_normally = probe.0.shutdown.started();
    probe.0.shutdown_once();
    assert!(
        exited_normally,
        "Wait-mode invalidation must finish before the native smoke timeout"
    );
    assert!(probe.0.take_terminal_error().is_none());
    assert!(probe.0.take_shutdown_error().is_none());
    let application = &probe.0.ownership.application;
    assert!(application.verified);
    assert!(application.frames >= 2);
    assert_eq!(application.frames, application.recordings);
    assert_eq!(application.inputs, 1);
    assert!(application.background_ready.load(Ordering::Acquire));
}

#[cfg(feature = "multi-viewport")]
mod multi_viewport {
    use super::*;
    use dear_imgui_rs::{Condition, NativeWindowInstanceId};
    use dear_imgui_wgpu::{WgpuFrameStage, WgpuViewportObservation};

    #[derive(Default)]
    struct Editor {
        frames: usize,
        recordings: usize,
        merged: bool,
        open: bool,
        secondary_close: bool,
        lost: usize,
        recreated: usize,
        shutdowns: usize,
        history: Vec<WgpuViewportObservation>,
        last_main_presentation: Option<GpuGeneration>,
    }

    impl Application for Editor {
        fn configure_imgui(
            &mut self,
            context: &mut crate::InitContext<'_>,
        ) -> Result<(), RunError> {
            context
                .imgui()
                .io_mut()
                .set_config_viewports_no_auto_merge(true);
            Ok(())
        }
        fn frame(&mut self, context: &mut FrameContext<'_>) -> Result<(), RunError> {
            self.frames += 1;
            // Like ImGui::Begin, opened() reports a close request; the application decides whether
            // to submit the window again. Continuing to call Begin keeps the viewport active.
            if !self.open {
                return Ok(());
            }
            let ui = context.ui();
            let main = ui.main_viewport();
            let pos = main.pos();
            let size = main.size();
            let position = if self.merged {
                ui.set_next_window_viewport(main.id());
                [pos[0] + 30.0, pos[1] + 30.0]
            } else {
                [pos[0] + size[0] + 40.0, pos[1] + 30.0]
            };
            ui.window("Dear App native editor regression")
                .opened(&mut self.open)
                .position(position, Condition::Always)
                .size([200.0, 120.0], Condition::Always)
                .build(|| ui.text(format!("Persistent frame state: {}", self.frames)));
            Ok(())
        }
        fn event(&mut self, context: &mut crate::EventContext<'_>) -> Result<(), RunError> {
            if matches!(context.event(), WindowEvent::CloseRequested) {
                let target = context.outcome().target.unwrap();
                assert!(!target.native.is_main);
                assert_eq!(context.window().id(), target.window_id);
                assert_ne!(context.window().id(), context.main_window().id());
                self.secondary_close = matches!(
                    context.outcome().platform_action,
                    dear_imgui_winit::WinitPlatformAction::ViewportCloseRequested
                );
            }
            Ok(())
        }
        fn record_render(&mut self, _: &mut RenderContext<'_>) -> Result<(), RunError> {
            self.recordings += 1;
            Ok(())
        }
        fn viewport_frame(
            &mut self,
            context: &crate::ViewportFrameContext<'_>,
        ) -> Result<(), RunError> {
            self.history
                .extend(context.secondary().observations().iter().cloned());
            if let Some(main) = context.main() {
                self.history.push(main.clone());
                if main.stage == WgpuFrameStage::PresentationRequested {
                    self.last_main_presentation = Some(context.generation());
                }
            }
            Ok(())
        }
        fn gpu_lost(&mut self, _: &mut GpuContext<'_>) -> Result<(), RunError> {
            self.lost += 1;
            Ok(())
        }
        fn gpu_recreated(&mut self, _: &mut GpuContext<'_>) -> Result<(), RunError> {
            self.recreated += 1;
            Ok(())
        }
        fn shutdown(&mut self, _: &mut crate::ShutdownContext<'_>) -> Result<(), RunError> {
            self.shutdowns += 1;
            Ok(())
        }
    }

    struct Probe {
        runner: Runner<Editor>,
        phase: u8,
        first: Option<NativeWindowInstanceId>,
        before_recovery: Option<NativeWindowInstanceId>,
        main: Option<NativeWindowInstanceId>,
        context: Option<dear_imgui_rs::ContextId>,
        frames_before_recovery: usize,
    }

    impl Probe {
        fn progress(&mut self, event_loop: &ActiveEventLoop) {
            let Some(runtime) = self.runner.ownership.runtime.as_ref() else {
                return;
            };
            let owned = runtime.ownership.get();
            let snapshots = owned.ui.platform.window_snapshots().unwrap();
            let main = snapshots
                .iter()
                .find(|item| item.target.is_main)
                .unwrap()
                .target
                .instance_id;
            let secondary = snapshots.iter().find(|item| !item.target.is_main).copied();
            let presented = |instance| {
                self.runner
                    .ownership
                    .application
                    .history
                    .iter()
                    .any(|item| {
                        item.target.instance_id == instance
                            && item.stage == WgpuFrameStage::PresentationRequested
                    })
            };
            if let Some(expected) = self.main {
                assert_eq!(main, expected);
            }
            if let Some(expected) = self.context {
                assert_eq!(owned.ui.context.id(), expected);
            }
            match self.phase {
                0 => {
                    if let Some(target) =
                        secondary.filter(|target| presented(target.target.instance_id))
                    {
                        self.first = Some(target.target.instance_id);
                        self.main = Some(main);
                        self.context = Some(owned.ui.context.id());
                        self.runner.ownership.application.merged = true;
                        self.phase = 1;
                    }
                }
                1 if secondary.is_none() => {
                    self.runner.ownership.application.merged = false;
                    self.phase = 2;
                }
                2 => {
                    if let Some(target) =
                        secondary.filter(|target| presented(target.target.instance_id))
                    {
                        assert_ne!(Some(target.target.instance_id), self.first);
                        self.phase = 3;
                        self.runner.window_event(
                            event_loop,
                            target.window_id,
                            WindowEvent::CloseRequested,
                        );
                        assert!(
                            !self.runner.shutdown.started(),
                            "secondary close must not exit the host"
                        );
                    }
                }
                3 if secondary.is_none() => {
                    assert!(self.runner.ownership.application.secondary_close);
                    assert!(!self.runner.ownership.application.open);
                    self.runner.ownership.application.open = true;
                    self.phase = 4;
                }
                4 => {
                    if let Some(target) =
                        secondary.filter(|target| presented(target.target.instance_id))
                    {
                        self.before_recovery = Some(target.target.instance_id);
                        self.frames_before_recovery = self.runner.ownership.application.frames;
                        self.phase = 5;
                        owned.generations.current().unwrap().gpu.device.destroy();
                        // Exercise the production entry before Winit can dispatch the queued
                        // DeviceLost user event. The runtime must observe callback evidence itself.
                        let main_id = runtime.window().window.id();
                        self.runner
                            .window_event(event_loop, main_id, WindowEvent::RedrawRequested);
                    }
                }
                5 if runtime.current_generation() == Some(GpuGeneration(1)) => {
                    if let Some(target) =
                        secondary.filter(|target| presented(target.target.instance_id))
                    {
                        assert_ne!(Some(target.target.instance_id), self.before_recovery);
                        assert!(
                            owned
                                .ui
                                .platform
                                .window_snapshot_for_instance(self.before_recovery.unwrap())
                                .unwrap()
                                .is_none()
                        );
                        assert!(
                            self.runner.ownership.application.frames > self.frames_before_recovery
                        );
                        self.phase = 6;
                        self.runner.ownership.application.open = false;
                    }
                }
                6 if secondary.is_none() => {
                    // A second real loss has no secondary acquisition to reject first: the main
                    // Surface validation must leave recovery pending until callbacks are checked.
                    self.frames_before_recovery = self.runner.ownership.application.frames;
                    self.phase = 7;
                    owned.generations.current().unwrap().gpu.device.destroy();
                    let main_id = runtime.window().window.id();
                    self.runner
                        .window_event(event_loop, main_id, WindowEvent::RedrawRequested);
                }
                7 if runtime.current_generation() == Some(GpuGeneration(2))
                    && self.runner.ownership.application.last_main_presentation
                        == Some(GpuGeneration(2)) =>
                {
                    assert!(self.runner.ownership.application.frames > self.frames_before_recovery);
                    assert!(secondary.is_none());
                    self.phase = 8;
                    self.runner.exit_normally(event_loop);
                }
                _ => {}
            }
        }
    }

    impl ApplicationHandler<RuntimeEvent> for Probe {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            self.runner.resumed(event_loop);
        }
        fn user_event(&mut self, event_loop: &ActiveEventLoop, event: RuntimeEvent) {
            self.runner.user_event(event_loop, event);
        }
        fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
            self.runner.window_event(event_loop, id, event);
        }
        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            self.progress(event_loop);
            self.runner.about_to_wait(event_loop);
        }
        fn exiting(&mut self, event_loop: &ActiveEventLoop) {
            self.runner.exiting(event_loop);
        }
    }

    #[test]
    #[ignore = "requires a native desktop and GPU; undock/merge/close and real device destruction"]
    fn native_multi_viewport_undock_merge_close_and_gpu_recovery() {
        let _guard = crate::runtime::imgui_test_guard();
        let mut builder = EventLoop::<RuntimeEvent>::with_user_event();
        #[cfg(target_os = "windows")]
        {
            use winit::platform::windows::EventLoopBuilderExtWindows;
            builder.with_any_thread(true);
        }
        #[cfg(target_os = "linux")]
        {
            use winit::platform::x11::EventLoopBuilderExtX11;
            builder.with_x11().with_any_thread(true);
        }
        let mut event_loop = builder.build().unwrap();
        event_loop.set_control_flow(ControlFlow::Poll);
        let config = AppConfig {
            multi_viewport: true,
            window_size: (320.0, 240.0),
            restore_previous_geometry: false,
            ..Default::default()
        };
        let editor = Editor {
            open: true,
            ..Default::default()
        };
        let mut probe = Probe {
            runner: Runner::new(config, editor, event_loop.create_proxy()),
            phase: 0,
            first: None,
            before_recovery: None,
            main: None,
            context: None,
            frames_before_recovery: 0,
        };
        let timeout = Instant::now() + Duration::from_secs(25);
        while !probe.runner.shutdown.started() && Instant::now() < timeout {
            event_loop.pump_app_events(Some(Duration::from_millis(10)), &mut probe);
        }
        probe.runner.shutdown_once();
        let terminal = probe.runner.take_terminal_error();
        let shutdown = probe.runner.take_shutdown_error();
        assert!(
            terminal.is_none(),
            "native multi-viewport failed: {terminal:?}"
        );
        assert!(
            shutdown.is_none(),
            "native multi-viewport shutdown failed: {shutdown:?}"
        );
        assert_eq!(probe.phase, 8, "native multi-viewport phase timed out");
        let editor = &probe.runner.ownership.application;
        assert_eq!(editor.lost, 2);
        assert_eq!(editor.recreated, 2);
        assert_eq!(editor.shutdowns, 1);
        assert_eq!(editor.recordings, editor.frames);
        assert!(editor.history.iter().any(|item| !item.target.is_main));
    }
}

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

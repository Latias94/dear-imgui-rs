//! Explicit ownership transfer between recording, queue submission, and presentation.

use std::{
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use super::{
    platform_adapter::NativeWindowLease,
    runtime::WgpuViewportError,
    trace::{ObservationKey, WgpuViewportFrameReport},
};
use crate::WgpuFrameStage;

/// Failure of an explicit viewport work operation, retaining every earlier observation.
#[derive(Debug)]
pub struct WgpuViewportWorkError {
    report: WgpuViewportFrameReport,
    faults: Vec<WgpuViewportError>,
}

impl WgpuViewportWorkError {
    pub(super) fn new(report: WgpuViewportFrameReport, faults: Vec<WgpuViewportError>) -> Self {
        Self { report, faults }
    }
    /// Partial observations; no unobserved stage is inferred from this error.
    pub fn report(&self) -> &WgpuViewportFrameReport {
        &self.report
    }
    /// Failures in observation order.
    pub fn faults(&self) -> &[WgpuViewportError] {
        &self.faults
    }
    /// Retains report and failures without requiring any live Context or native window.
    pub fn into_parts(self) -> (WgpuViewportFrameReport, Vec<WgpuViewportError>) {
        (self.report, self.faults)
    }
}

impl fmt::Display for WgpuViewportWorkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.faults.first() {
            Some(error) => write!(f, "WGPU viewport work failed: {error}"),
            None => f.write_str("WGPU viewport work failed"),
        }
    }
}
impl std::error::Error for WgpuViewportWorkError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.faults.first().map(|fault| fault as _)
    }
}

/// The acquired frame is released before the Surface, then the exact window and its owner chain.
pub(super) struct AcquiredTarget {
    pub(super) frame: Option<wgpu::SurfaceTexture>,
    pub(super) surface: Option<Rc<wgpu::Surface<'static>>>,
    pub(super) native_window: Option<NativeWindowLease>,
}

pub(super) fn release_acquired_parts<Frame, Surface, Window>(
    frame: &mut Option<Frame>,
    surface: &mut Option<Surface>,
    window: &mut Option<Window>,
) {
    drop(frame.take());
    drop(surface.take());
    drop(window.take());
}

impl Drop for AcquiredTarget {
    fn drop(&mut self) {
        release_acquired_parts(&mut self.frame, &mut self.surface, &mut self.native_window);
    }
}

pub(super) struct RecordedTarget {
    pub(super) command_buffer: Option<wgpu::CommandBuffer>,
    pub(super) acquired: AcquiredTarget,
    pub(super) observation: Option<ObservationKey>,
}

/// Move-only secondary command buffers and target leases that have not been submitted.
///
/// Drop abandons the commands and discards acquired frames. It never submits or presents. Resource
/// uploads remain isolated by the renderer epoch; later frames cannot overwrite these commands.
#[must_use = "submit or explicitly discard the recorded secondary work"]
pub struct WgpuRecordedViewportWork {
    queue: wgpu::Queue,
    targets: Vec<RecordedTarget>,
    report: WgpuViewportFrameReport,
}

impl WgpuRecordedViewportWork {
    pub(super) fn new(
        queue: wgpu::Queue,
        targets: Vec<RecordedTarget>,
        report: WgpuViewportFrameReport,
    ) -> Self {
        Self {
            queue,
            targets,
            report,
        }
    }
    /// Observations after recording, before any queue submission.
    pub fn report(&self) -> &WgpuViewportFrameReport {
        &self.report
    }

    fn validate_targets(&mut self) -> Vec<WgpuViewportError> {
        let mut faults = Vec::new();
        for target in &self.targets {
            if target
                .acquired
                .native_window
                .as_ref()
                .is_none_or(NativeWindowLease::is_retired)
            {
                let error = WgpuViewportError::SurfaceOperationFailed {
                    operation: "submit work for a retired native target",
                };
                self.report
                    .fail(target.observation, "submit viewport commands", &error);
                faults.push(error);
            }
        }
        faults
    }

    /// Submits producer command buffers before every UI consumer in one queue operation.
    ///
    /// Producer buffers must belong to this renderer's Device. Retired targets reject the whole
    /// operation before producer iteration or submission. After successful submission, acquired
    /// frames, Surfaces, and window leases transfer to pending presentation instead of being freed.
    pub fn submit(
        mut self,
        producer_commands: impl IntoIterator<Item = wgpu::CommandBuffer>,
    ) -> Result<WgpuPendingPresentations, WgpuViewportWorkError> {
        let faults = self.validate_targets();
        if !faults.is_empty() {
            return Err(WgpuViewportWorkError::new(self.report, faults));
        }
        let collected = catch_unwind(AssertUnwindSafe(|| {
            producer_commands.into_iter().collect::<Vec<_>>()
        }));
        let commands = match collected {
            Ok(commands) => commands,
            Err(_) => {
                return Err(WgpuViewportWorkError::new(
                    self.report,
                    vec![WgpuViewportError::CallbackPanicked {
                        callback: "collect producer command buffers",
                    }],
                ));
            }
        };
        // Producer iteration is application code and may retire a platform owner. Revalidate
        // immediately before submission as well as before invoking that code.
        let faults = self.validate_targets();
        if !faults.is_empty() {
            return Err(WgpuViewportWorkError::new(self.report, faults));
        }
        let ui_commands = self
            .targets
            .iter_mut()
            .filter_map(|target| target.command_buffer.take())
            .collect::<Vec<_>>();
        let submission = catch_unwind(AssertUnwindSafe(|| {
            submit_after_producers(&self.queue, commands, ui_commands)
        }));
        let submission = match submission {
            Ok(submission) => submission,
            Err(_) => {
                let error = WgpuViewportError::CallbackPanicked {
                    callback: "Queue::submit viewport work",
                };
                for target in &self.targets {
                    self.report
                        .fail(target.observation, "submit viewport commands", &error);
                }
                return Err(WgpuViewportWorkError::new(self.report, vec![error]));
            }
        };
        for target in &self.targets {
            self.report
                .stage(target.observation, WgpuFrameStage::Submitted);
        }
        Ok(WgpuPendingPresentations {
            #[cfg(feature = "wgpu-30")]
            queue: self.queue,
            targets: self.targets,
            report: self.report,
            submission,
        })
    }

    /// Abandons recorded commands without submitting or presenting them.
    pub fn discard(mut self) -> WgpuViewportFrameReport {
        for target in &self.targets {
            self.report.discard(target.observation);
        }
        self.targets.clear();
        self.report
    }
}

/// One queue boundary shared by native work and headless ordering verification.
pub(super) fn submit_after_producers(
    queue: &wgpu::Queue,
    producers: Vec<wgpu::CommandBuffer>,
    ui: Vec<wgpu::CommandBuffer>,
) -> wgpu::SubmissionIndex {
    queue.submit(producers.into_iter().chain(ui))
}

/// Submitted viewport work retaining its acquired frames and exact native targets.
///
/// This UI-thread value may outlive platform registration retirement and Context teardown. Native
/// target lookup is intentionally not repeated after submission: the held leases authorize only
/// presentation/discard of these exact acquired frames. Drop discards them exactly once.
#[must_use = "present or explicitly discard the submitted viewport frames"]
pub struct WgpuPendingPresentations {
    #[cfg(feature = "wgpu-30")]
    queue: wgpu::Queue,
    targets: Vec<RecordedTarget>,
    report: WgpuViewportFrameReport,
    submission: wgpu::SubmissionIndex,
}

impl WgpuPendingPresentations {
    /// Observations after queue submission; GPU completion remains unobserved.
    pub fn report(&self) -> &WgpuViewportFrameReport {
        &self.report
    }
    /// The queue's returned submission index, for application-owned completion tracking.
    pub fn submission_index(&self) -> wgpu::SubmissionIndex {
        self.submission.clone()
    }

    /// Requests presentation once for each acquired target, then releases all target leases.
    ///
    /// This calls WGPU's presentation API; it does not wait for GPU completion or visible pixels.
    pub fn present(mut self) -> Result<WgpuViewportFrameReport, WgpuViewportWorkError> {
        let mut faults = Vec::new();
        for target in &mut self.targets {
            let Some(frame) = target.acquired.frame.take() else {
                continue;
            };
            let result = catch_unwind(AssertUnwindSafe(|| {
                #[cfg(feature = "wgpu-30")]
                self.queue.present(frame);
                #[cfg(not(feature = "wgpu-30"))]
                frame.present();
            }));
            if result.is_ok() {
                self.report
                    .stage(target.observation, WgpuFrameStage::PresentationRequested);
            } else {
                let error = WgpuViewportError::CallbackPanicked {
                    callback: "present viewport frame",
                };
                self.report
                    .fail(target.observation, "request viewport presentation", &error);
                faults.push(error);
            }
        }
        self.targets.clear();
        if faults.is_empty() {
            Ok(self.report)
        } else {
            Err(WgpuViewportWorkError::new(self.report, faults))
        }
    }

    /// Discards submitted surface frames without requesting presentation. Submission evidence is
    /// preserved, and frame/Surface/window release order is identical to the Drop path.
    pub fn discard(mut self) -> WgpuViewportFrameReport {
        for target in &self.targets {
            self.report.discard(target.observation);
        }
        self.targets.clear();
        self.report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn producer_commands_execute_before_ui_consumers_on_the_real_queue() -> crate::RendererResult<()>
    {
        use crate::{FramebufferExtent, WgpuInitInfo, WgpuRenderer};
        use wgpu::*;
        let Some((device, queue)) = crate::texture::request_test_device() else {
            return Ok(());
        };
        let mut context = dear_imgui_rs::Context::create();
        let mut renderer = WgpuRenderer::new(
            WgpuInitInfo::new(device.clone(), queue.clone(), TextureFormat::Rgba8Unorm),
            &mut context,
        )?;
        renderer.set_gamma_mode(crate::GammaMode::Linear);
        let texture = |label, size, usage| {
            device.create_texture(&TextureDescriptor {
                label: Some(label),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm,
                usage,
                view_formats: &[],
            })
        };
        let scene = texture(
            "producer scene color",
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            TextureUsages::TEXTURE_BINDING
                | TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::COPY_DST,
        );
        let scene_view = scene.create_view(&TextureViewDescriptor::default());
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &scene,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &[0, 0, 255, 255],
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let registered = renderer.register_external_texture(&scene_view)?;
        context.io_mut().set_display_size([16.0, 16.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        {
            let ui = context.frame();
            ui.get_foreground_draw_list().add_image(
                registered.texture_id(),
                [0.0, 0.0],
                [16.0, 16.0],
                [0.0, 0.0],
                [1.0, 1.0],
                dear_imgui_rs::Color::WHITE,
            );
        }
        let target = texture(
            "producer-order UI target",
            Extent3d {
                width: 16,
                height: 16,
                depth_or_array_layers: 1,
            },
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
        );
        let target_view = target.create_view(&TextureViewDescriptor::default());
        let readback = device.create_buffer(&BufferDescriptor {
            label: None,
            size: 256 * 16,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        // Record the UI first. Execution order must depend on the explicit submit boundary,
        // not CPU recording order or stale contents left in the engine-owned scene texture.
        let mut ui_encoder = device.create_command_encoder(&CommandEncoderDescriptor::default());
        let recording = {
            let attachments = [Some(RenderPassColorAttachment {
                view: &target_view,
                resolve_target: None,
                depth_slice: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::BLACK),
                    store: StoreOp::Store,
                },
            })];
            let mut pass = ui_encoder.begin_render_pass(&RenderPassDescriptor {
                color_attachments: &attachments,
                #[cfg(any(feature = "wgpu-28", feature = "wgpu-29", feature = "wgpu-30"))]
                multiview_mask: None,
                ..Default::default()
            });
            let pending = context.try_render(renderer.renderer_consumer()?)?;
            renderer.render(pending, &mut pass, FramebufferExtent::new(16, 16))?
        };
        assert_eq!(
            recording.dependencies.external_bindings,
            [renderer.external_texture_binding(registered)?]
        );
        ui_encoder.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            TexelCopyBufferInfo {
                buffer: &readback,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(16),
                },
            },
            Extent3d {
                width: 16,
                height: 16,
                depth_or_array_layers: 1,
            },
        );
        let mut producer = device.create_command_encoder(&CommandEncoderDescriptor::default());
        {
            let attachments = [Some(RenderPassColorAttachment {
                view: &scene_view,
                resolve_target: None,
                depth_slice: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::RED),
                    store: StoreOp::Store,
                },
            })];
            let _pass = producer.begin_render_pass(&RenderPassDescriptor {
                color_attachments: &attachments,
                #[cfg(any(feature = "wgpu-28", feature = "wgpu-29", feature = "wgpu-30"))]
                multiview_mask: None,
                ..Default::default()
            });
        }
        let submission =
            submit_after_producers(&queue, vec![producer.finish()], vec![ui_encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        readback.slice(..).map_async(MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        device
            .poll(PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(std::time::Duration::from_secs(10)),
            })
            .expect("producer-order GPU readback must complete");
        receiver
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("map callback must run")
            .expect("map must succeed");
        {
            #[cfg(feature = "wgpu-30")]
            let bytes = readback
                .slice(..)
                .get_mapped_range()
                .expect("readback range must remain mapped");
            #[cfg(not(feature = "wgpu-30"))]
            let bytes = readback.slice(..).get_mapped_range();
            assert_eq!(
                &bytes[8 * 256 + 8 * 4..8 * 256 + 8 * 4 + 4],
                &[255, 0, 0, 255],
                "UI must consume this submission's produced SceneColor rather than the previous blue contents"
            );
        }
        readback.unmap();
        renderer.shutdown(&mut context)
    }

    #[test]
    fn abandoned_recording_does_not_claim_submission_or_presentation() {
        let Some((_device, queue)) = crate::texture::request_test_device() else {
            return;
        };
        let context = dear_imgui_rs::Context::create();
        let mut trace = super::super::trace::FrameTraceState::default();
        trace.begin(crate::WgpuFrameScope {
            context_id: context.id(),
            epoch: None,
            frame_count: 1,
        });
        let target = dear_imgui_rs::NativeWindowTarget {
            instance_id: dear_imgui_rs::NativeWindowInstanceId::allocate(context.id()).unwrap(),
            viewport_id: dear_imgui_rs::Id::from(7),
            is_main: false,
        };
        let key = trace.start(target);
        trace.stage(key, WgpuFrameStage::Recorded);
        let report = trace.finish();
        let recorded = RecordedTarget {
            command_buffer: None,
            acquired: AcquiredTarget {
                frame: None,
                surface: None,
                native_window: None,
            },
            observation: key,
        };
        let work = WgpuRecordedViewportWork::new(queue, vec![recorded], report);
        let report = work.discard();
        assert_eq!(report.observations()[0].stage, WgpuFrameStage::Recorded);
        assert_eq!(
            report.observations()[0].outcome,
            crate::WgpuFrameOutcome::Discarded
        );
    }

    #[test]
    fn retired_work_rejects_submission_before_producer_iteration() {
        let Some((_device, queue)) = crate::texture::request_test_device() else {
            return;
        };
        let context = dear_imgui_rs::Context::create();
        let mut trace = super::super::trace::FrameTraceState::default();
        trace.begin(crate::WgpuFrameScope {
            context_id: context.id(),
            epoch: None,
            frame_count: 1,
        });
        let target = dear_imgui_rs::NativeWindowTarget {
            instance_id: dear_imgui_rs::NativeWindowInstanceId::allocate(context.id()).unwrap(),
            viewport_id: dear_imgui_rs::Id::from(7),
            is_main: false,
        };
        let key = trace.start(target);
        trace.stage(key, WgpuFrameStage::Recorded);
        let work = WgpuRecordedViewportWork::new(
            queue,
            vec![RecordedTarget {
                command_buffer: None,
                acquired: AcquiredTarget {
                    frame: None,
                    surface: None,
                    native_window: None,
                },
                observation: key,
            }],
            trace.finish(),
        );
        let called = std::cell::Cell::new(false);
        let producer = std::iter::from_fn(|| {
            called.set(true);
            None
        });
        let error = work.submit(producer).unwrap_err();
        assert!(!called.get());
        assert_eq!(
            error.report().observations()[0].stage,
            WgpuFrameStage::Recorded
        );
        assert!(matches!(
            error.report().observations()[0].outcome,
            crate::WgpuFrameOutcome::Failed { .. }
        ));
    }

    #[cfg(all(
        feature = "multi-viewport-winit",
        any(target_os = "windows", target_os = "linux")
    ))]
    fn native_test_instance(allow_primary_reproducer: bool) -> wgpu::Instance {
        let selected = std::env::var("DEAR_IMGUI_TEST_WGPU_BACKEND").expect(
            "set DEAR_IMGUI_TEST_WGPU_BACKEND=vulkan or dx12 explicitly; the pure WGPU diagnostic also accepts primary to reproduce multi-backend teardown failures",
        );
        let backends = match selected.as_str() {
            "dx12" => wgpu::Backends::DX12,
            "vulkan" => wgpu::Backends::VULKAN,
            "primary" if allow_primary_reproducer => wgpu::Backends::PRIMARY,
            value => panic!("unsupported DEAR_IMGUI_TEST_WGPU_BACKEND for this fixture: {value}"),
        };
        eprintln!("[native-wgpu] requested backends {backends:?}");
        #[cfg(any(feature = "wgpu-27", feature = "wgpu-28"))]
        {
            wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends,
                ..Default::default()
            })
        }
        #[cfg(any(feature = "wgpu-29", feature = "wgpu-30"))]
        {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            descriptor.backends = backends;
            wgpu::Instance::new(descriptor)
        }
    }

    #[test]
    #[cfg(all(
        feature = "multi-viewport-winit",
        any(target_os = "windows", target_os = "linux")
    ))]
    #[ignore = "requires a native desktop and GPU; run the named pending-presentation lease regression"]
    fn native_pending_present_survives_context_retirement() {
        native_pending_presentation_fixture("present");
    }

    #[test]
    #[cfg(all(
        feature = "multi-viewport-winit",
        any(target_os = "windows", target_os = "linux")
    ))]
    #[ignore = "requires a native desktop and GPU; run the named pending-presentation lease regression"]
    fn native_pending_discard_survives_context_retirement() {
        native_pending_presentation_fixture("discard");
    }

    #[test]
    #[cfg(all(
        feature = "multi-viewport-winit",
        any(target_os = "windows", target_os = "linux")
    ))]
    #[ignore = "requires a native desktop and GPU; run the named pending-presentation lease regression"]
    fn native_pending_drop_survives_context_retirement() {
        native_pending_presentation_fixture("drop");
    }

    #[cfg(all(
        feature = "multi-viewport-winit",
        any(target_os = "windows", target_os = "linux")
    ))]
    #[allow(
        deprecated,
        reason = "The bounded native fixture creates windows without an app event pump"
    )]
    fn native_pending_presentation_fixture(release: &str) {
        use wgpu::*;
        let mut builder = winit::event_loop::EventLoop::builder();
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
        let event_loop = builder.build().expect("native desktop must be available");
        {
            let window = std::sync::Arc::new(
                event_loop
                    .create_window(
                        winit::window::Window::default_attributes()
                            .with_title("WGPU pending lease regression")
                            .with_inner_size(winit::dpi::PhysicalSize::new(64, 64)),
                    )
                    .unwrap(),
            );
            let weak_window = std::sync::Arc::downgrade(&window);
            let mut context = dear_imgui_rs::Context::create();
            context.enable_multi_viewport();
            let mut platform = dear_imgui_winit::WinitPlatform::new(&mut context).unwrap();
            platform
                .attach_window(
                    std::sync::Arc::clone(&window),
                    dear_imgui_winit::HiDpiMode::Default,
                    &mut context,
                )
                .unwrap();
            platform.enable_viewports(&mut context).unwrap();
            let snapshot = platform.window_snapshot(window.id()).unwrap().unwrap();
            let adapter_owner = platform.viewport_renderer_adapter(&context).unwrap();
            let lease = adapter_owner
                .acquire_native_window_lease(snapshot.target.instance_id)
                .unwrap()
                .unwrap();
            let instance = native_test_instance(false);
            let surface = Rc::new(instance.create_surface(lease.window_arc()).unwrap());
            let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            }))
            .expect("native fixture requires a surface-compatible adapter");
            eprintln!(
                "[pending-lease] {release}: adapter {:?}",
                adapter.get_info()
            );
            let (device, queue) =
                pollster::block_on(adapter.request_device(&DeviceDescriptor::default())).unwrap();
            let config = surface
                .get_default_config(&adapter, 64, 64)
                .expect("native surface must support a default configuration");
            surface.configure(&device, &config);
            #[cfg(any(feature = "wgpu-27", feature = "wgpu-28"))]
            let frame = surface
                .get_current_texture()
                .expect("native fixture must acquire its frame");
            #[cfg(any(feature = "wgpu-29", feature = "wgpu-30"))]
            let frame = match surface.get_current_texture() {
                CurrentSurfaceTexture::Success(frame)
                | CurrentSurfaceTexture::Suboptimal(frame) => frame,
                event => panic!("native fixture must acquire its frame: {event:?}"),
            };
            let view = frame.texture.create_view(&TextureViewDescriptor::default());
            let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor::default());
            {
                let attachments = [Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::GREEN),
                        store: StoreOp::Store,
                    },
                })];
                let _pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    color_attachments: &attachments,
                    #[cfg(any(feature = "wgpu-28", feature = "wgpu-29", feature = "wgpu-30"))]
                    multiview_mask: None,
                    ..Default::default()
                });
            }
            let mut trace = super::super::trace::FrameTraceState::default();
            trace.begin(crate::WgpuFrameScope {
                context_id: context.id(),
                epoch: None,
                frame_count: 1,
            });
            let key = trace.start(snapshot.target);
            trace.stage(key, WgpuFrameStage::Recorded);
            let work = WgpuRecordedViewportWork::new(
                queue,
                vec![RecordedTarget {
                    command_buffer: Some(encoder.finish()),
                    acquired: AcquiredTarget {
                        frame: Some(frame),
                        surface: Some(Rc::clone(&surface)),
                        native_window: Some(lease),
                    },
                    observation: key,
                }],
                trace.finish(),
            );
            drop(view);
            let pending = work.submit(std::iter::empty()).unwrap();
            let submission = pending.submission_index();
            assert_eq!(
                pending.report().observations()[0].stage,
                WgpuFrameStage::Submitted
            );
            drop(context);
            drop(adapter_owner);
            drop(platform);
            drop(window);
            drop(surface);
            assert!(
                weak_window.upgrade().is_some(),
                "pending work must retain the actual window after Context teardown"
            );
            assert!(
                pending.targets[0]
                    .acquired
                    .native_window
                    .as_ref()
                    .unwrap()
                    .is_retired()
            );
            match release {
                "present" => {
                    let report = pending.present().unwrap();
                    assert_eq!(
                        report.observations()[0].stage,
                        WgpuFrameStage::PresentationRequested
                    );
                }
                "discard" => {
                    let report = pending.discard();
                    assert_eq!(report.observations()[0].stage, WgpuFrameStage::Submitted);
                    assert_eq!(
                        report.observations()[0].outcome,
                        crate::WgpuFrameOutcome::Discarded
                    );
                }
                _ => drop(pending),
            }
            assert!(
                weak_window.upgrade().is_none(),
                "the final frame/Surface/window lease must release the native allocation exactly once"
            );
            // The test tears down its entire Device after each case. Drain the completed
            // submission only at fixture shutdown; production work never inserts this wait or
            // upgrades its observation to GPU-completed based on target-lease release.
            device
                .poll(PollType::Wait {
                    submission_index: Some(submission),
                    timeout: Some(std::time::Duration::from_secs(10)),
                })
                .expect("fixture GPU submission must complete before Device teardown");
            drop(device);
            drop(adapter);
            drop(instance);
        }
    }

    #[test]
    #[cfg(all(
        feature = "multi-viewport-winit",
        any(target_os = "windows", target_os = "linux")
    ))]
    #[ignore = "pure WGPU native discard diagnostic; requires a desktop and selected GPU backend"]
    #[allow(
        deprecated,
        reason = "The bounded diagnostic deliberately excludes all Dear ImGui platform ownership"
    )]
    fn native_pure_wgpu_submit_discard_teardown_without_imgui_context() {
        use wgpu::*;
        let mut builder = winit::event_loop::EventLoop::builder();
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
        let event_loop = builder.build().expect("native desktop must be available");
        let window = std::sync::Arc::new(
            event_loop
                .create_window(
                    winit::window::Window::default_attributes()
                        .with_title("Pure WGPU discard diagnostic")
                        .with_inner_size(winit::dpi::PhysicalSize::new(64, 64)),
                )
                .unwrap(),
        );
        let instance = native_test_instance(true);
        let surface = instance
            .create_surface(std::sync::Arc::clone(&window))
            .unwrap();
        let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .unwrap();
        eprintln!("[pure-wgpu-discard] adapter {:?}", adapter.get_info());
        let (device, queue) =
            pollster::block_on(adapter.request_device(&DeviceDescriptor::default())).unwrap();
        let config = surface.get_default_config(&adapter, 64, 64).unwrap();
        surface.configure(&device, &config);
        #[cfg(any(feature = "wgpu-27", feature = "wgpu-28"))]
        let frame = surface.get_current_texture().unwrap();
        #[cfg(any(feature = "wgpu-29", feature = "wgpu-30"))]
        let frame = match surface.get_current_texture() {
            CurrentSurfaceTexture::Success(frame) | CurrentSurfaceTexture::Suboptimal(frame) => {
                frame
            }
            event => panic!("pure WGPU diagnostic must acquire a frame: {event:?}"),
        };
        let view = frame.texture.create_view(&TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor::default());
        {
            let attachments = [Some(RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::GREEN),
                    store: StoreOp::Store,
                },
            })];
            let _pass = encoder.begin_render_pass(&RenderPassDescriptor {
                color_attachments: &attachments,
                #[cfg(any(feature = "wgpu-28", feature = "wgpu-29", feature = "wgpu-30"))]
                multiview_mask: None,
                ..Default::default()
            });
        }
        let submission = queue.submit([encoder.finish()]);
        drop(view);
        device
            .poll(PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(std::time::Duration::from_secs(10)),
            })
            .unwrap();
        drop(frame);
        drop(surface);
        drop(window);
        drop(queue);
        drop(device);
        drop(adapter);
        drop(instance);
    }

    #[test]
    fn acquired_target_release_keeps_surface_and_window_alive_until_frame_is_gone() {
        struct Probe(&'static str, Rc<RefCell<Vec<&'static str>>>);
        impl Drop for Probe {
            fn drop(&mut self) {
                self.1.borrow_mut().push(self.0);
            }
        }
        let order = Rc::new(RefCell::new(Vec::new()));
        let mut frame = Some(Probe("frame", Rc::clone(&order)));
        let mut surface = Some(Probe("surface", Rc::clone(&order)));
        let mut window = Some(Probe("window", Rc::clone(&order)));
        release_acquired_parts(&mut frame, &mut surface, &mut window);
        release_acquired_parts(&mut frame, &mut surface, &mut window);
        assert_eq!(order.borrow().as_slice(), ["frame", "surface", "window"]);
    }
}

impl fmt::Debug for WgpuRecordedViewportWork {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WgpuRecordedViewportWork")
            .field("target_count", &self.targets.len())
            .field("report", &self.report)
            .finish_non_exhaustive()
    }
}
impl fmt::Debug for WgpuPendingPresentations {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WgpuPendingPresentations")
            .field("target_count", &self.targets.len())
            .field("report", &self.report)
            .finish_non_exhaustive()
    }
}

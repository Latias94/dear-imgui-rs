//! Shared WGPU multi-viewport application runtime.
//!
//! The public teaching example and the private runtime contract use this module
//! for the actual Winit, WGPU, Dear ImGui, and viewport-rendering lifecycle.
//! Contract-specific policy (Test Engine, environment variables, and JSON
//! evidence) lives in the private CI binary instead of in the teaching sample.

use dear_imgui_rs::render::ReconciledFrame;
use dear_imgui_rs::{Condition, Context, FrameToken, TextureId, Ui};
use dear_imgui_wgpu::{
    FramebufferExtent, GammaMode, WgpuInitInfo, WgpuRenderer, multi_viewport as wgpu_mvp,
};
use dear_imgui_winit::{HiDpiMode, WinitPlatform};
use pollster::block_on;
use std::{fmt, sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

/// Policy hooks for the interactive sample and private runtime contract.
///
/// This is deliberately specific to this WGPU example. It keeps the renderer
/// transaction and teardown order in one place without introducing a generic
/// probe framework into the examples crate.
pub(crate) trait ViewportScenario {
    type Output;

    fn initialize(
        &mut self,
        _context: &mut Context,
        _window: &Window,
        _adapter: &wgpu::AdapterInfo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn show_example_ui(&self) -> bool {
        true
    }

    fn before_ui(&mut self, _viewport_count: i32, _secondary_window: Option<&Window>) {}

    fn extend_main_window(&mut self, _ui: &Ui, _viewport_count: &mut i32) {}

    fn extend_game_window(&mut self, _ui: &Ui) {}

    fn after_ui(&mut self, _ui: &Ui, _viewport_count: i32) {}

    fn trace_secondary_submissions(&self) -> bool {
        false
    }

    fn drive_frame<'frame>(
        &mut self,
        frame: FrameToken<'frame>,
        frame_index: u64,
        driver: &mut MainSurfaceFrameDriver<'_>,
    ) -> Result<(), Box<dyn std::error::Error>>;

    fn after_frame(
        &mut self,
        _presented: bool,
        _secondary_submission_evidence: Option<SecondarySubmissionEvidence>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn complete(&self) -> bool {
        false
    }

    fn redraw_continuously(&self) -> bool {
        true
    }

    fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn take_output(&mut self) -> Option<Self::Output> {
        None
    }
}

enum AppRenderer {
    Single(Box<WgpuRenderer>),
    Multi(wgpu_mvp::WinitViewportRoute),
}

pub(crate) enum AppPreparedFrame<'frame> {
    Single(ReconciledFrame<'frame>),
    Multi(wgpu_mvp::WgpuPreparedViewportFrame<'frame>),
}

#[derive(Debug)]
pub(crate) struct MainSurfaceFrameError {
    source: Box<dyn std::error::Error>,
}

impl MainSurfaceFrameError {
    fn message(message: &'static str) -> Self {
        Self {
            source: Box::new(std::io::Error::other(message)),
        }
    }
}

impl fmt::Display for MainSurfaceFrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.source.fmt(formatter)
    }
}

impl std::error::Error for MainSurfaceFrameError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

impl From<Box<dyn std::error::Error>> for MainSurfaceFrameError {
    fn from(source: Box<dyn std::error::Error>) -> Self {
        Self { source }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SecondarySubmissionEvidence {
    pub(crate) render_submitted_viewport_ids: Vec<u32>,
    pub(crate) present_submitted_viewport_ids: Vec<u32>,
}

impl SecondarySubmissionEvidence {
    fn from_report(report: &wgpu_mvp::WgpuViewportFrameReport) -> Option<Self> {
        let render_submitted_viewport_ids = report
            .render_submitted_viewport_ids()
            .iter()
            .map(|id| id.raw())
            .collect::<Vec<_>>();
        let present_submitted_viewport_ids = report
            .present_submitted_viewport_ids()
            .iter()
            .map(|id| id.raw())
            .collect::<Vec<_>>();

        render_submitted_viewport_ids
            .iter()
            .any(|id| present_submitted_viewport_ids.contains(id))
            .then_some(Self {
                render_submitted_viewport_ids,
                present_submitted_viewport_ids,
            })
    }
}

impl AppRenderer {
    fn prepare_frame<'frame>(
        &mut self,
        event_loop: &ActiveEventLoop,
        frame: FrameToken<'frame>,
    ) -> Result<AppPreparedFrame<'frame>, Box<dyn std::error::Error>> {
        match self {
            Self::Single(renderer) => {
                let pending_frame = frame.try_render(renderer.renderer_consumer()?)?;
                Ok(AppPreparedFrame::Single(
                    renderer.reconcile_frame(pending_frame)?,
                ))
            }
            Self::Multi(route) => Ok(AppPreparedFrame::Multi(route.prepare(event_loop, frame)?)),
        }
    }

    fn render_main(
        &mut self,
        frame: AppPreparedFrame<'_>,
        render_pass: &mut wgpu::RenderPass<'_>,
        framebuffer_extent: FramebufferExtent,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match (self, frame) {
            (Self::Single(renderer), AppPreparedFrame::Single(frame)) => {
                renderer.render_reconciled(frame, render_pass, framebuffer_extent)?
            }
            (Self::Multi(route), AppPreparedFrame::Multi(frame)) => {
                route.render_main(frame, render_pass, framebuffer_extent)?
            }
            _ => return Err("prepared frame does not belong to the active renderer route".into()),
        }
        Ok(())
    }

    fn shutdown(&mut self, context: &mut Context) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::Single(renderer) => renderer.shutdown(context)?,
            Self::Multi(route) => route.shutdown(context)?,
        }
        Ok(())
    }
}

pub(crate) struct MainSurfaceFrameDriver<'a> {
    renderer: &'a mut AppRenderer,
    event_loop: &'a ActiveEventLoop,
    surface: &'a wgpu::Surface<'static>,
    surface_config: &'a wgpu::SurfaceConfiguration,
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    surface_frame: Option<wgpu::SurfaceTexture>,
    reconfigure_after_present: bool,
    trace_secondary_submissions: bool,
    secondary_submission_evidence: Option<SecondarySubmissionEvidence>,
    rendered: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MainSurfaceRenderOutcome {
    ReadyToPresent,
    Skipped,
}

impl<'a> MainSurfaceFrameDriver<'a> {
    fn new(
        renderer: &'a mut AppRenderer,
        event_loop: &'a ActiveEventLoop,
        surface: &'a wgpu::Surface<'static>,
        surface_config: &'a wgpu::SurfaceConfiguration,
        device: &'a wgpu::Device,
        queue: &'a wgpu::Queue,
        trace_secondary_submissions: bool,
    ) -> Self {
        Self {
            renderer,
            event_loop,
            surface,
            surface_config,
            device,
            queue,
            surface_frame: None,
            reconfigure_after_present: false,
            trace_secondary_submissions,
            secondary_submission_evidence: None,
            rendered: false,
        }
    }

    pub(crate) fn prepare_frame<'frame>(
        &mut self,
        frame: FrameToken<'frame>,
    ) -> Result<AppPreparedFrame<'frame>, MainSurfaceFrameError> {
        let prepared = self
            .renderer
            .prepare_frame(self.event_loop, frame)
            .map_err(MainSurfaceFrameError::from)?;
        if self.trace_secondary_submissions
            && let AppPreparedFrame::Multi(frame) = &prepared
        {
            self.secondary_submission_evidence =
                SecondarySubmissionEvidence::from_report(frame.secondary_report());
        }
        Ok(prepared)
    }

    pub(crate) fn render_main_frame(
        &mut self,
        frame: AppPreparedFrame<'_>,
    ) -> Result<MainSurfaceRenderOutcome, MainSurfaceFrameError> {
        if self.rendered {
            return Err(MainSurfaceFrameError::message(
                "main surface frame was rendered more than once",
            ));
        }
        if self.surface_frame.is_some() {
            return Err(MainSurfaceFrameError::message(
                "main surface frame was acquired more than once",
            ));
        }
        let surface_frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                self.reconfigure_after_present = true;
                frame
            }
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(self.device, self.surface_config);
                return Ok(MainSurfaceRenderOutcome::Skipped);
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(MainSurfaceRenderOutcome::Skipped);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(MainSurfaceFrameError::message(
                    "surface acquisition failed with a WGPU validation error",
                ));
            }
        };
        let framebuffer_extent = FramebufferExtent::from_texture(&surface_frame.texture);
        let view = surface_frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("imgui-main-encoder"),
            });
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("imgui-main-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.12,
                            b: 0.15,
                            a: 1.0,
                        }),
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
                .render_main(frame, &mut render_pass, framebuffer_extent)
                .map_err(MainSurfaceFrameError::from)?;
        }
        self.queue.submit(Some(encoder.finish()));
        self.surface_frame = Some(surface_frame);
        self.rendered = true;
        Ok(MainSurfaceRenderOutcome::ReadyToPresent)
    }

    pub(crate) fn present_frame(&mut self) -> Result<(), MainSurfaceFrameError> {
        if !self.rendered {
            return Err(MainSurfaceFrameError::message(
                "main surface frame was presented before rendering",
            ));
        }
        let surface_frame = self.surface_frame.take().ok_or_else(|| {
            MainSurfaceFrameError::message("main surface frame was presented more than once")
        })?;
        self.queue.present(surface_frame);
        if self.reconfigure_after_present {
            self.surface.configure(self.device, self.surface_config);
        }
        Ok(())
    }

    pub(crate) fn was_presented(&self) -> bool {
        self.rendered && self.surface_frame.is_none()
    }

    pub(crate) fn take_secondary_submission_evidence(
        &mut self,
    ) -> Option<SecondarySubmissionEvidence> {
        self.secondary_submission_evidence.take()
    }
}

pub(crate) struct AppWindow<S: ViewportScenario> {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: AppRenderer,
    platform: WinitPlatform,
    start_time: Instant,
    enable_viewports: bool,
    _game_tex: wgpu::Texture,
    game_tex_view: wgpu::TextureView,
    game_tex_id: TextureId,
    scenario: S,
    scenario_shutdown_complete: bool,
    renderer_shutdown_complete: bool,
    platform_shutdown_complete: bool,
    frame_index: u64,
    imgui: Context,
}

impl<S: ViewportScenario> Drop for AppWindow<S> {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("WGPU example fallback shutdown failed: {error}");
        }
    }
}

impl<S: ViewportScenario> AppWindow<S> {
    fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.imgui.end_frame();
        let mut errors = Vec::new();

        if !self.scenario_shutdown_complete {
            match self.scenario.shutdown() {
                Ok(()) => self.scenario_shutdown_complete = true,
                Err(error) => errors.push(format!("scenario shutdown failed: {error}")),
            }
        }

        if !self.renderer_shutdown_complete {
            match self.renderer.shutdown(&mut self.imgui) {
                Ok(()) => self.renderer_shutdown_complete = true,
                Err(error) => errors.push(format!("WGPU renderer shutdown failed: {error}")),
            }
        }

        if self.platform.viewports_enabled()
            && let Err(error) = self.platform.disable_viewports(&mut self.imgui)
        {
            errors.push(format!("Winit multi-viewport shutdown failed: {error}"));
        }

        if !self.platform_shutdown_complete && !self.platform.viewports_enabled() {
            match self.platform.shutdown(&mut self.imgui) {
                Ok(()) => self.platform_shutdown_complete = true,
                Err(error) => errors.push(format!("Winit platform shutdown failed: {error}")),
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; ").into())
        }
    }

    fn new(event_loop: &ActiveEventLoop, scenario: S) -> Result<Self, Box<dyn std::error::Error>> {
        let enable_viewports = cfg!(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        ));
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let window: Arc<Window> = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Dear ImGui Multi-Viewport (wgpu)")
                    .with_inner_size(LogicalSize::new(1200.0, 720.0)),
            )?,
        );
        let surface = instance.create_surface(window.clone())?;
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            apply_limit_buckets: false,
            force_fallback_adapter: false,
        }))?;
        let adapter_info = adapter.get_info();
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = [
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ]
        .into_iter()
        .find(|format| caps.formats.contains(format))
        .unwrap_or(caps.formats[0]);
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);
        let game_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mvw_game_view_texture"),
            size: wgpu::Extent3d {
                width: 512,
                height: 512,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let game_tex_view = game_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let mut imgui = Context::create();
        if enable_viewports {
            imgui.enable_multi_viewport();
        }
        {
            let io = imgui.io_mut();
            let mut flags = io.config_flags();
            flags.insert(dear_imgui_rs::ConfigFlags::DOCKING_ENABLE);
            io.set_config_flags(flags);
        }
        let mut platform = WinitPlatform::new(&mut imgui)?;
        platform.attach_window(Arc::clone(&window), HiDpiMode::Default, &mut imgui)?;
        if enable_viewports {
            platform.enable_viewports(&mut imgui)?;
        }
        let init_info = WgpuInitInfo::new(device.clone(), queue.clone(), surface_config.format)
            .with_instance(instance.clone())
            .with_adapter(adapter.clone())
            .with_viewport_surface_config((&surface_config).into());
        let mut renderer = WgpuRenderer::new(init_info, &mut imgui)?;
        renderer.set_gamma_mode(GammaMode::Auto);
        let game_tex_id = renderer
            .register_external_texture(&game_tex_view)?
            .texture_id();
        let renderer = if enable_viewports {
            match wgpu_mvp::WinitViewportRoute::attach(&mut imgui, &platform, renderer) {
                Ok(route) => AppRenderer::Multi(route),
                Err(failure) => {
                    let (attach_error, mut renderer) = failure.into_parts();
                    if let Err(shutdown_error) = renderer.shutdown(&mut imgui) {
                        return Err(format!(
                            "WGPU multi-viewport attachment failed: {attach_error}; renderer cleanup failed: {shutdown_error}"
                        )
                        .into());
                    }
                    return Err(attach_error.into());
                }
            }
        } else {
            AppRenderer::Single(Box::new(renderer))
        };
        let mut app = Self {
            window,
            surface,
            surface_config,
            device,
            queue,
            renderer,
            platform,
            start_time: Instant::now(),
            enable_viewports,
            _game_tex: game_tex,
            game_tex_view,
            game_tex_id,
            scenario,
            scenario_shutdown_complete: false,
            renderer_shutdown_complete: false,
            platform_shutdown_complete: false,
            frame_index: 0,
            imgui,
        };
        if let Err(initialize_error) =
            app.scenario
                .initialize(&mut app.imgui, app.window.as_ref(), &adapter_info)
        {
            return match app.shutdown() {
                Ok(()) => Err(initialize_error),
                Err(shutdown_error) => Err(format!(
                    "scenario initialization failed: {initialize_error}; runtime cleanup failed: {shutdown_error}"
                )
                .into()),
            };
        }
        Ok(app)
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) -> Result<(), Box<dyn std::error::Error>> {
        {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("mvw_game_view_encoder"),
                });
            let rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mvw_game_view_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.game_tex_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: (self.start_time.elapsed().as_secs_f32().sin() * 0.5 + 0.5) as f64,
                            g: 0.2,
                            b: 0.4,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            drop(rpass);
            self.queue.submit(Some(encoder.finish()));
        }

        self.platform.prepare_frame(&mut self.imgui, &self.window)?;
        let mut viewport_count =
            i32::try_from(self.imgui.platform_io().viewports_iter().count()).unwrap_or(i32::MAX);
        let secondary_window = self
            .imgui
            .platform_io()
            .viewports_iter()
            .find(|viewport| viewport.is_platform_window() && viewport.platform_window_created())
            .and_then(|viewport| {
                let handle = viewport.platform_handle().cast::<Window>();
                // SAFETY: Dear ImGui's Winit platform backend stores a live `winit::Window`
                // pointer in PlatformHandle for the lifetime of the platform viewport. The
                // reference remains scoped to this frame and is not retained by the scenario.
                unsafe { handle.as_ref() }
            });
        self.scenario.before_ui(viewport_count, secondary_window);
        let frame = self.imgui.begin_frame();
        let ui = frame.ui();
        if self.scenario.show_example_ui() {
            ui.dockspace().build()?;
            let enable_viewports = self.enable_viewports;
            let game_tex_id = self.game_tex_id;
            let scenario = &mut self.scenario;
            ui.window("Main")
                .size([420.0, 260.0], Condition::FirstUseEver)
                .build(|| {
                    if enable_viewports {
                        ui.text("Drag this window outside to create a new OS window.");
                        ui.separator();
                        ui.text("Multi-viewport is enabled (experimental).");
                    } else {
                        ui.text("Multi-viewport is disabled on this platform (winit + WGPU).");
                        ui.separator();
                        ui.text("Use a native platform example for multi-viewport support.");
                    }
                    scenario.extend_main_window(ui, &mut viewport_count);
                });
            ui.window("Game View")
                .size([520.0, 540.0], Condition::FirstUseEver)
                .build(|| {
                    scenario.extend_game_window(ui);
                    let avail = ui.content_region_avail();
                    let side = avail[0].min(avail[1]).max(64.0);
                    ui.text("Offscreen WGPU texture rendered each frame:");
                    ui.image(game_tex_id, [side, side]);
                });
        }
        self.scenario.after_ui(ui, viewport_count);
        self.platform.prepare_render(ui, &self.window)?;
        self.frame_index = self
            .frame_index
            .checked_add(1)
            .ok_or("WGPU example frame index exhausted")?;
        let trace_secondary_submissions = self.scenario.trace_secondary_submissions();
        let mut driver = MainSurfaceFrameDriver::new(
            &mut self.renderer,
            event_loop,
            &self.surface,
            &self.surface_config,
            &self.device,
            &self.queue,
            trace_secondary_submissions,
        );
        self.scenario
            .drive_frame(frame, self.frame_index, &mut driver)?;
        let presented = driver.was_presented();
        let evidence = driver.take_secondary_submission_evidence();
        drop(driver);
        self.scenario.after_frame(presented, evidence)?;
        Ok(())
    }

    fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        if size.width > 0 && size.height > 0 {
            self.surface_config.width = size.width;
            self.surface_config.height = size.height;
            self.surface.configure(&self.device, &self.surface_config);
        }
    }

    fn take_output(&mut self) -> Option<S::Output> {
        self.scenario.take_output()
    }
}

#[derive(Default)]
struct App<S: ViewportScenario> {
    scenario: Option<S>,
    window: Option<AppWindow<S>>,
    error: Option<String>,
}

impl<S: ViewportScenario> App<S> {
    fn new(scenario: S) -> Self {
        Self {
            scenario: Some(scenario),
            window: None,
            error: None,
        }
    }

    fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.window.as_mut().map_or(Ok(()), AppWindow::shutdown)
    }

    fn take_output(&mut self) -> Option<S::Output> {
        self.window.as_mut().and_then(AppWindow::take_output)
    }
}

impl<S: ViewportScenario> ApplicationHandler for App<S> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let Some(scenario) = self.scenario.take() else {
            self.error = Some("WGPU viewport scenario was initialized more than once".into());
            event_loop.exit();
            return;
        };
        match AppWindow::new(event_loop, scenario) {
            Ok(window) => {
                window.window.request_redraw();
                self.window = Some(window);
            }
            Err(error) => {
                self.error = Some(error.to_string());
                event_loop.exit();
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(app) = &self.window {
            let redraw_continuously = app.scenario.redraw_continuously();
            event_loop.set_control_flow(if redraw_continuously {
                ControlFlow::Poll
            } else {
                ControlFlow::Wait
            });
            if redraw_continuously {
                app.window.request_redraw();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(app) = self.window.as_mut() else {
            return;
        };
        let is_main_window = window_id == app.window.id();
        let full: Event<()> = Event::WindowEvent {
            window_id,
            event: event.clone(),
        };
        if let Err(error) = app
            .platform
            .handle_event(&mut app.imgui, &app.window, &full)
        {
            self.error = Some(error.to_string());
            event_loop.exit();
            return;
        }
        match event {
            WindowEvent::CloseRequested if is_main_window => event_loop.exit(),
            WindowEvent::Resized(size) if is_main_window => app.resize(size),
            WindowEvent::ScaleFactorChanged { .. } if is_main_window => {
                app.resize(app.window.inner_size());
            }
            WindowEvent::RedrawRequested if is_main_window => match app.redraw(event_loop) {
                Ok(()) if app.scenario.complete() => event_loop.exit(),
                Ok(()) if app.scenario.redraw_continuously() => app.window.request_redraw(),
                Ok(()) => {}
                Err(error) => {
                    self.error = Some(error.to_string());
                    event_loop.exit();
                }
            },
            _ => {}
        }
    }
}

pub(crate) fn run<S: ViewportScenario>(
    scenario: S,
) -> Result<Option<S::Output>, Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(if scenario.redraw_continuously() {
        ControlFlow::Poll
    } else {
        ControlFlow::Wait
    });
    let mut app = App::new(scenario);
    let event_loop_result = event_loop.run_app(&mut app);
    let app_error = app.error.take();
    let shutdown_result = app.shutdown();
    let output = app.take_output();
    drop(app);
    let mut errors = Vec::new();
    if let Err(error) = event_loop_result {
        errors.push(format!("event loop failed: {error}"));
    }
    if let Some(error) = app_error {
        errors.push(error);
    }
    if let Err(error) = shutdown_result {
        errors.push(error.to_string());
    }
    if errors.is_empty() {
        Ok(output)
    } else {
        Err(errors.join("; ").into())
    }
}

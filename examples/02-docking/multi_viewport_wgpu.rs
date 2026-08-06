//! Minimal multi-viewport sample using winit + wgpu backends
//!
//! ⚠️ **EXPERIMENTAL TEST EXAMPLE ONLY** ⚠️
//!
//! Multi-viewport support is currently **NOT PRODUCTION-READY**.
//! This example is for testing and development purposes only.
//!
//! Run with:
//! ```bash
//! cargo run --bin multi_viewport_wgpu --features multi-viewport
//! ```
//!
//! Automated Linux viewport smoke with Xvfb and Mesa Lavapipe:
//! ```text
//! python3 tools/ci/run_contract.py multi-viewport-smoke
//! ```
//!
//! What this example demonstrates:
//! - Creates a main window with WGPU rendering
//! - Enables Dear ImGui multi-viewport (experimental)
//! - Routes input events for secondary windows
//! - Lets Dear ImGui create/update/destroy platform windows and renders them
//!
//! Known limitations:
//! - Multi-viewport functionality may have bugs or incomplete features
//! - Not recommended for production use
//! - Secondary OS windows are enabled only on desktop native targets
//!   (Windows/macOS/Linux); Linux is exercised with Xvfb and Mesa Lavapipe in CI.

#[cfg(feature = "test-engine")]
use dear_imgui_rs::MouseButton;
use dear_imgui_rs::render::ReconciledFrame;
use dear_imgui_rs::{Condition, Context, FrameToken, TextureId};
#[cfg(feature = "test-engine")]
use dear_imgui_test_engine::{
    BuiltInTestSuite, FrameDriveOutcome, MainRenderOutcome, RegisteredTestSuite, ResultSummary,
    RunFlags, RunSpeed, ScriptCount, TestEngine, TestFrameDriver, TestGroup, VerboseLevel,
};
use dear_imgui_wgpu::{GammaMode, WgpuInitInfo, WgpuRenderer, multi_viewport as wgpu_mvp};
use dear_imgui_winit::{HiDpiMode, WinitPlatform, multi_viewport as winit_mvp};
use pollster::block_on;
use std::{fmt, sync::Arc, time::Instant};
#[cfg(feature = "test-engine")]
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

enum AppRenderer {
    Single(Box<WgpuRenderer>),
    Multi(wgpu_mvp::WinitViewportRuntime),
}

#[derive(Debug)]
struct MainSurfaceFrameError {
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

#[cfg(feature = "test-engine")]
#[derive(Clone, Debug)]
struct SecondarySubmissionEvidence {
    render_submitted_viewport_ids: Vec<u32>,
    present_submitted_viewport_ids: Vec<u32>,
}

#[cfg(feature = "test-engine")]
impl SecondarySubmissionEvidence {
    fn from_report(report: &wgpu_mvp::WgpuViewportFrameTraceReport) -> Option<Self> {
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

#[cfg(feature = "test-engine")]
struct ViewportSmokeState {
    result_path: Option<PathBuf>,
    adapter: wgpu::AdapterInfo,
    mode: ViewportSmokeMode,
    complete: bool,
}

#[cfg(feature = "test-engine")]
enum ViewportSmokeMode {
    Lifecycle(LifecycleSmokeState),
    UpstreamSuite {
        suite: RegisteredTestSuite,
        terminal_summary: Option<ResultSummary>,
    },
}

#[cfg(feature = "test-engine")]
struct LifecycleSmokeState {
    require_secondary_while_held: bool,
    held_probe_armed: bool,
    held_probe_pressed: bool,
    held_probe_complete: bool,
    saw_secondary_viewport: bool,
    saw_secondary_while_held: bool,
    saw_merged_viewport: bool,
    secondary_submission_before_main_acquire: Option<SecondarySubmissionEvidence>,
    main_present_bracketed_by_test_engine: bool,
}

#[cfg(feature = "test-engine")]
enum CompletedViewportSmoke {
    Lifecycle {
        result_path: Option<PathBuf>,
        adapter: wgpu::AdapterInfo,
        saw_secondary_while_held: bool,
        saw_merged_viewport: bool,
        secondary_submission_before_main_acquire: SecondarySubmissionEvidence,
        main_present_bracketed_by_test_engine: bool,
    },
    UpstreamSuite {
        result_path: Option<PathBuf>,
        adapter: wgpu::AdapterInfo,
        suite: RegisteredTestSuite,
        summary: ResultSummary,
    },
}

#[cfg(feature = "test-engine")]
impl ViewportSmokeState {
    fn completed_result(&self) -> Option<CompletedViewportSmoke> {
        if !self.complete {
            return None;
        }
        match &self.mode {
            ViewportSmokeMode::UpstreamSuite {
                suite,
                terminal_summary,
            } => Some(CompletedViewportSmoke::UpstreamSuite {
                result_path: self.result_path.clone(),
                adapter: self.adapter.clone(),
                suite: suite.clone(),
                summary: (*terminal_summary)?,
            }),
            ViewportSmokeMode::Lifecycle(lifecycle) => {
                let secondary_submission_before_main_acquire = lifecycle
                    .secondary_submission_before_main_acquire
                    .as_ref()?
                    .clone();
                Some(CompletedViewportSmoke::Lifecycle {
                    result_path: self.result_path.clone(),
                    adapter: self.adapter.clone(),
                    saw_secondary_while_held: lifecycle.saw_secondary_while_held,
                    saw_merged_viewport: lifecycle.saw_merged_viewport,
                    secondary_submission_before_main_acquire,
                    main_present_bracketed_by_test_engine: lifecycle
                        .main_present_bracketed_by_test_engine,
                })
            }
        }
    }
}

#[cfg(feature = "test-engine")]
impl CompletedViewportSmoke {
    fn write_after_teardown(self) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::Lifecycle {
                result_path,
                adapter,
                saw_secondary_while_held,
                saw_merged_viewport,
                secondary_submission_before_main_acquire,
                main_present_bracketed_by_test_engine,
            } => {
                let Some(path) = result_path else {
                    return Ok(());
                };
                let render_submitted_viewport_ids = json_u32_array(
                    &secondary_submission_before_main_acquire.render_submitted_viewport_ids,
                );
                let present_submitted_viewport_ids = json_u32_array(
                    &secondary_submission_before_main_acquire.present_submitted_viewport_ids,
                );
                let json = format!(
                    "{{\"schema_version\":3,\"adapter\":{{\"name\":\"{}\",\"backend\":\"{:?}\",\"device_type\":\"{:?}\",\"driver\":\"{}\",\"driver_info\":\"{}\",\"vendor\":{},\"device\":{}}},\"secondary_viewport_while_held_observed\":{},\"merge_observed\":{},\"secondary_render_submitted_before_main_acquire_viewport_ids\":{},\"secondary_present_submitted_before_main_acquire_viewport_ids\":{},\"main_present_bracketed_by_test_engine\":{}}}",
                    json_escape(&adapter.name),
                    adapter.backend,
                    adapter.device_type,
                    json_escape(&adapter.driver),
                    json_escape(&adapter.driver_info),
                    adapter.vendor,
                    adapter.device,
                    saw_secondary_while_held,
                    saw_merged_viewport,
                    render_submitted_viewport_ids,
                    present_submitted_viewport_ids,
                    main_present_bracketed_by_test_engine,
                );
                write_json_atomic(&path, &json)
            }
            Self::UpstreamSuite {
                result_path,
                adapter,
                suite,
                summary,
            } => {
                let Some(path) = result_path else {
                    return Ok(());
                };
                let registered_tests = json_string_array(suite.test_names());
                let json = format!(
                    "{{\"schema_version\":1,\"suite\":\"upstream-viewports\",\"category\":\"{}\",\"platform_backend\":\"Winit\",\"renderer_backend\":\"WGPU\",\"real_platform_backend\":true,\"runtime_teardown_complete\":true,\"registered_count\":{},\"registered_tests\":{},\"tested\":{},\"success\":{},\"in_queue\":{},\"adapter\":{{\"name\":\"{}\",\"backend\":\"{:?}\",\"device_type\":\"{:?}\",\"driver\":\"{}\",\"driver_info\":\"{}\",\"vendor\":{},\"device\":{}}}}}",
                    suite.suite().category(),
                    suite.test_count(),
                    registered_tests,
                    summary.count_tested,
                    summary.count_success,
                    summary.count_in_queue,
                    json_escape(&adapter.name),
                    adapter.backend,
                    adapter.device_type,
                    json_escape(&adapter.driver),
                    json_escape(&adapter.driver_info),
                    adapter.vendor,
                    adapter.device,
                );
                write_json_atomic(&path, &json)
            }
        }
    }
}

#[cfg(feature = "test-engine")]
fn validate_software_vulkan_adapter(info: &wgpu::AdapterInfo) -> Result<(), String> {
    let identity = format!("{} {} {}", info.name, info.driver, info.driver_info).to_lowercase();
    if info.backend != wgpu::Backend::Vulkan {
        return Err(format!(
            "viewport smoke requires Vulkan, selected {:?}",
            info.backend
        ));
    }
    if info.device_type != wgpu::DeviceType::Cpu {
        return Err(format!(
            "viewport smoke requires a CPU software adapter, selected {:?}",
            info.device_type
        ));
    }
    if !identity.contains("lavapipe") && !identity.contains("llvmpipe") {
        return Err(format!(
            "viewport smoke requires Lavapipe/llvmpipe, selected '{}' ({})",
            info.name, info.driver
        ));
    }
    Ok(())
}

#[cfg(feature = "test-engine")]
fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                use fmt::Write as _;
                let _ = write!(escaped, "\\u{:04x}", character as u32);
            }
            character => escaped.push(character),
        }
    }
    escaped
}

#[cfg(feature = "test-engine")]
fn json_u32_array(values: &[u32]) -> String {
    let values = values
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    format!("[{values}]")
}

#[cfg(feature = "test-engine")]
fn json_string_array(values: &[String]) -> String {
    let values = values
        .iter()
        .map(|value| format!("\"{}\"", json_escape(value)))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{values}]")
}

#[cfg(feature = "test-engine")]
fn write_json_atomic(path: &Path, contents: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("DEAR_IMGUI_VIEWPORT_SMOKE_JSON must name a file")?;
    let temporary = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(contents.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok::<_, Box<dyn std::error::Error>>(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

impl AppRenderer {
    fn reconcile_frame<'frame>(
        &mut self,
        frame: FrameToken<'frame>,
    ) -> Result<ReconciledFrame<'frame>, Box<dyn std::error::Error>> {
        match self {
            Self::Single(renderer) => {
                let pending_frame = frame.try_render(renderer.renderer_consumer()?)?;
                Ok(renderer.reconcile_frame(pending_frame)?)
            }
            Self::Multi(runtime) => Ok(runtime.reconcile_frame(frame)?),
        }
    }

    fn render_with_fb_size_reconciled<'frame>(
        &mut self,
        frame: ReconciledFrame<'frame>,
        render_pass: &mut wgpu::RenderPass<'_>,
        width: u32,
        height: u32,
    ) -> Result<ReconciledFrame<'frame>, Box<dyn std::error::Error>> {
        match self {
            Self::Single(renderer) => {
                Ok(renderer.render_with_fb_size_reconciled(frame, render_pass, width, height)?)
            }
            Self::Multi(runtime) => {
                Ok(runtime.render_with_fb_size_reconciled(frame, render_pass, width, height)?)
            }
        }
    }

    #[cfg(feature = "test-engine")]
    fn begin_frame_trace(
        &self,
    ) -> Result<Option<wgpu_mvp::WgpuViewportFrameTraceGuard<'_>>, Box<dyn std::error::Error>> {
        Ok(match self {
            Self::Single(_) => None,
            Self::Multi(runtime) => Some(runtime.begin_frame_trace()?),
        })
    }

    fn poll_fault(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Self::Multi(runtime) = self {
            runtime.poll_fault()?;
        }
        Ok(())
    }

    fn shutdown(&mut self, context: &mut Context) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::Single(renderer) => renderer.shutdown(context)?,
            Self::Multi(runtime) => runtime.shutdown(context)?,
        }
        Ok(())
    }
}

struct MainSurfaceFrameDriver<'a> {
    renderer: &'a mut AppRenderer,
    surface: &'a wgpu::Surface<'static>,
    surface_config: &'a wgpu::SurfaceConfiguration,
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    surface_frame: Option<wgpu::SurfaceTexture>,
    enable_viewports: bool,
    reconfigure_after_present: bool,
    #[cfg(feature = "test-engine")]
    trace_secondary_submissions: bool,
    #[cfg(feature = "test-engine")]
    secondary_submission_evidence: Option<SecondarySubmissionEvidence>,
    rendered: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MainSurfaceRenderOutcome {
    ReadyToPresent,
    Skipped,
}

impl MainSurfaceFrameDriver<'_> {
    fn prepare_frame<'frame>(
        &mut self,
        frame: FrameToken<'frame>,
    ) -> Result<ReconciledFrame<'frame>, MainSurfaceFrameError> {
        let mut reconciled = self
            .renderer
            .reconcile_frame(frame)
            .map_err(MainSurfaceFrameError::from)?;
        #[cfg(feature = "test-engine")]
        let secondary_trace = if self.trace_secondary_submissions {
            self.renderer
                .begin_frame_trace()
                .map_err(MainSurfaceFrameError::from)?
        } else {
            None
        };
        if self.enable_viewports {
            reconciled.update_and_render_platform_windows_default();
        }
        #[cfg(feature = "test-engine")]
        if let Some(trace) = secondary_trace {
            self.secondary_submission_evidence =
                SecondarySubmissionEvidence::from_report(&trace.finish());
        }
        self.renderer
            .poll_fault()
            .map_err(MainSurfaceFrameError::from)?;
        Ok(reconciled)
    }

    fn render_main_frame(
        &mut self,
        frame: ReconciledFrame<'_>,
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
        let view = surface_frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("imgui-main-encoder"),
            });
        let frame = {
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
                .render_with_fb_size_reconciled(
                    frame,
                    &mut render_pass,
                    self.surface_config.width,
                    self.surface_config.height,
                )
                .map_err(MainSurfaceFrameError::from)?
        };
        drop(frame);
        self.queue.submit(Some(encoder.finish()));
        self.surface_frame = Some(surface_frame);
        self.rendered = true;
        Ok(MainSurfaceRenderOutcome::ReadyToPresent)
    }

    fn present_frame(&mut self) -> Result<(), MainSurfaceFrameError> {
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
}

#[cfg(feature = "test-engine")]
impl TestFrameDriver for MainSurfaceFrameDriver<'_> {
    type PrepareError = MainSurfaceFrameError;
    type RenderError = MainSurfaceFrameError;
    type PresentError = MainSurfaceFrameError;

    fn prepare<'frame>(
        &mut self,
        frame: FrameToken<'frame>,
        _frame_index: u64,
    ) -> Result<ReconciledFrame<'frame>, Self::PrepareError> {
        self.prepare_frame(frame)
    }

    fn render_main(
        &mut self,
        frame: ReconciledFrame<'_>,
        _frame_index: u64,
    ) -> Result<MainRenderOutcome, Self::RenderError> {
        self.render_main_frame(frame).map(|outcome| match outcome {
            MainSurfaceRenderOutcome::ReadyToPresent => MainRenderOutcome::ReadyToPresent,
            MainSurfaceRenderOutcome::Skipped => MainRenderOutcome::Skipped,
        })
    }

    fn present(&mut self, _frame_index: u64) -> Result<(), Self::PresentError> {
        self.present_frame()
    }
}

struct AppWindow {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: AppRenderer,
    viewport_runtime: Option<winit_mvp::WinitPlatformRuntime>,
    platform: WinitPlatform,
    start_time: Instant,
    enable_viewports: bool,
    // Offscreen "game view" texture and view
    // Keep the texture alive; the view alone doesn't own the resource.
    _game_tex: wgpu::Texture,
    game_tex_view: wgpu::TextureView,
    game_tex_id: TextureId,
    #[cfg(feature = "test-engine")]
    test_engine: Option<TestEngine>,
    #[cfg(feature = "test-engine")]
    viewport_smoke: Option<ViewportSmokeState>,
    #[cfg(feature = "test-engine")]
    test_engine_shutdown_complete: bool,
    #[cfg(feature = "test-engine")]
    test_engine_frame_index: u64,
    renderer_shutdown_complete: bool,
    viewport_runtime_shutdown_complete: bool,
    platform_shutdown_complete: bool,
    // Every backend and extension that may retain Context-bound state is dropped first.
    imgui: Context,
}

impl Drop for AppWindow {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("WGPU example fallback shutdown failed: {error}");
        }
    }
}

impl AppWindow {
    fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.imgui.end_frame();
        let mut errors = Vec::new();

        #[cfg(feature = "test-engine")]
        if !self.test_engine_shutdown_complete {
            match self.test_engine.as_mut().map(TestEngine::shutdown) {
                Some(Err(error)) => errors.push(format!("test engine shutdown failed: {error}")),
                Some(Ok(())) | None => self.test_engine_shutdown_complete = true,
            }
        }

        if !self.renderer_shutdown_complete {
            match self.renderer.shutdown(&mut self.imgui) {
                Ok(()) => self.renderer_shutdown_complete = true,
                Err(error) => errors.push(format!("WGPU renderer shutdown failed: {error}")),
            }
        }

        if !self.viewport_runtime_shutdown_complete {
            let (viewport_runtime, imgui) = (&mut self.viewport_runtime, &mut self.imgui);
            match viewport_runtime
                .as_mut()
                .map(|runtime| runtime.shutdown(imgui))
            {
                Some(Err(error)) => {
                    errors.push(format!("Winit multi-viewport shutdown failed: {error}"));
                }
                Some(Ok(())) | None => self.viewport_runtime_shutdown_complete = true,
            }
        }

        if !self.platform_shutdown_complete {
            let (platform, imgui) = (&mut self.platform, &mut self.imgui);
            match platform.shutdown(imgui) {
                Ok(()) => {
                    // `WinitPlatform::shutdown` is the authoritative final release for both its
                    // base attachment and a runtime that completed native cleanup with a fault.
                    self.viewport_runtime_shutdown_complete = true;
                    self.platform_shutdown_complete = true;
                }
                Err(error) => errors.push(format!("Winit platform shutdown failed: {error}")),
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; ").into())
        }
    }

    fn new(event_loop: &ActiveEventLoop) -> Result<Self, Box<dyn std::error::Error>> {
        // Winit + WGPU multi-viewport is experimental.
        // Enabled by default on desktop native targets. The Linux path is exercised
        // with Xvfb and Mesa Lavapipe in native runtime CI.
        let enable_viewports = cfg!(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        ));
        #[cfg(feature = "test-engine")]
        let run_viewport_drag_smoke =
            std::env::var("DEAR_IMGUI_VIEWPORT_DRAG_SMOKE").is_ok_and(|value| value == "1");
        #[cfg(feature = "test-engine")]
        let run_upstream_viewport_suite =
            std::env::var("DEAR_IMGUI_UPSTREAM_VIEWPORT_SUITE").is_ok_and(|value| value == "1");
        #[cfg(feature = "test-engine")]
        if run_viewport_drag_smoke && run_upstream_viewport_suite {
            return Err(
                "DEAR_IMGUI_VIEWPORT_DRAG_SMOKE and DEAR_IMGUI_UPSTREAM_VIEWPORT_SUITE are mutually exclusive"
                    .into(),
            );
        }
        #[cfg(feature = "test-engine")]
        let run_viewport_smoke = run_viewport_drag_smoke
            || run_upstream_viewport_suite
            || std::env::var("DEAR_IMGUI_VIEWPORT_SMOKE").is_ok_and(|value| value == "1");

        // Create WGPU instance first (also used by renderer for per-viewport surfaces)
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

        #[cfg(feature = "test-engine")]
        let adapter_info = adapter.get_info();
        #[cfg(feature = "test-engine")]
        if run_viewport_smoke {
            println!(
                "WGPU adapter: name='{}', backend={:?}, device_type={:?}, driver='{}', info='{}'",
                adapter_info.name,
                adapter_info.backend,
                adapter_info.device_type,
                adapter_info.driver,
                adapter_info.driver_info,
            );
            if std::env::var("DEAR_IMGUI_REQUIRE_SOFTWARE_VULKAN").is_ok_and(|value| value == "1") {
                validate_software_vulkan_adapter(&adapter_info)?;
            }
        }

        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = [
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ]
        .into_iter()
        .find(|f| caps.formats.contains(f))
        .unwrap_or(caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width.max(1),
            height: size.height.max(1),
            // Secondary viewports inherit this policy. AutoNoVsync prefers low-latency present
            // modes and falls back portably when a surface cannot provide one.
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Create a simple offscreen texture for a "game view" (rendered every frame).
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

        // Dear ImGui context + platform
        let mut imgui = Context::create();
        #[cfg(feature = "test-engine")]
        if run_viewport_smoke {
            imgui.set_ini_filename(None::<String>)?;
        }

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
        let viewport_runtime = enable_viewports
            .then(|| winit_mvp::WinitPlatformRuntime::new(&mut imgui, &platform))
            .transpose()?;

        // WGPU renderer
        let init_info = WgpuInitInfo::new(device.clone(), queue.clone(), surface_config.format)
            .with_instance(instance.clone())
            .with_adapter(adapter.clone())
            .with_viewport_surface_config((&surface_config).into());
        let mut renderer = WgpuRenderer::new(init_info, &mut imgui)?;
        renderer.set_gamma_mode(GammaMode::Auto);

        // Register the offscreen texture as an external ImGui texture.
        let game_tex_id = renderer
            .register_external_texture(&game_tex_view)?
            .texture_id();

        let renderer = if enable_viewports {
            match wgpu_mvp::WinitViewportRuntime::attach(
                &mut imgui,
                viewport_runtime
                    .as_ref()
                    .expect("Winit viewport runtime must exist when viewports are enabled"),
                renderer,
            ) {
                Ok(runtime) => AppRenderer::Multi(runtime),
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

        let app = Self {
            window,
            surface,
            surface_config,
            device,
            queue,
            renderer,
            viewport_runtime,
            platform,
            start_time: Instant::now(),
            enable_viewports,
            _game_tex: game_tex,
            game_tex_view,
            game_tex_id,
            #[cfg(feature = "test-engine")]
            test_engine: None,
            #[cfg(feature = "test-engine")]
            viewport_smoke: None,
            #[cfg(feature = "test-engine")]
            test_engine_shutdown_complete: false,
            #[cfg(feature = "test-engine")]
            test_engine_frame_index: 0,
            renderer_shutdown_complete: false,
            viewport_runtime_shutdown_complete: false,
            platform_shutdown_complete: false,
            imgui,
        };

        #[cfg(feature = "test-engine")]
        let app = {
            let mut app = app;
            if run_viewport_smoke {
                app.configure_viewport_smoke(
                    adapter_info,
                    run_viewport_drag_smoke,
                    run_upstream_viewport_suite,
                )?;
            }
            app
        };

        Ok(app)
    }

    #[cfg(feature = "test-engine")]
    fn configure_viewport_smoke(
        &mut self,
        adapter: wgpu::AdapterInfo,
        drag_while_held: bool,
        upstream_suite: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let main_pos = self
            .window
            .inner_position()
            .unwrap_or_else(|_| winit::dpi::PhysicalPosition::new(0, 0));
        let main_size = self.window.inner_size();
        #[cfg(target_os = "macos")]
        let (main_pos, main_size) = {
            let scale = self.window.scale_factor();
            (
                main_pos.to_logical::<f32>(scale),
                main_size.to_logical::<f32>(scale),
            )
        };
        #[cfg(not(target_os = "macos"))]
        let (main_pos, main_size) = (
            main_pos.cast::<f32>(),
            winit::dpi::PhysicalSize::new(main_size.width as f32, main_size.height as f32),
        );
        let external_pos = [main_pos.x + main_size.width + 100.0, main_pos.y + 100.0];
        let redock_pos = [
            main_pos.x + main_size.width * 0.5,
            main_pos.y + main_size.height * 0.5,
        ];
        let mut engine = TestEngine::create()?;
        engine.start(&mut self.imgui)?;
        engine.set_run_speed(if drag_while_held {
            RunSpeed::Normal
        } else {
            RunSpeed::Fast
        })?;
        engine.set_verbose_level(VerboseLevel::Info)?;
        engine.set_verbose_level_on_error(VerboseLevel::Debug)?;
        engine.set_log_to_tty(true)?;
        let mode = if upstream_suite {
            let suite = engine.register_builtin_test_suite(BuiltInTestSuite::UpstreamViewports)?;
            engine.queue_tests(
                TestGroup::Tests,
                Some(suite.suite().category()),
                RunFlags::RUN_FROM_COMMAND_LINE,
            )?;
            ViewportSmokeMode::UpstreamSuite {
                suite,
                terminal_summary: None,
            }
        } else {
            let test_name = if drag_while_held {
                "multi_viewport_held_undock_smoke"
            } else {
                "multi_viewport_surface_smoke"
            };
            engine.add_script_test("wgpu", test_name, move |test| {
                test.wait_for_item("Main/Viewport Count", ScriptCount::new(240)?)?;
                if drag_while_held {
                    test.dock_into("Game View", "Main")?;
                    test.yield_frames(ScriptCount::new(10)?)?;
                    test.item_click("Main/Begin Held Drag Probe")?;
                    test.mouse_move("Game View/#TAB")?;
                    test.mouse_down(MouseButton::Left)?;
                    test.mouse_lift_drag_threshold(MouseButton::Left)?;
                    test.mouse_move_to_pos(external_pos[0], external_pos[1])?;
                    test.yield_frames(ScriptCount::new(120)?)?;
                    test.mouse_move_to_pos(redock_pos[0], redock_pos[1])?;
                    test.yield_frames(ScriptCount::new(60)?)?;
                    test.mouse_up(MouseButton::Left)?;
                    test.yield_frames(ScriptCount::new(30)?)?;
                    test.assert_item_read_int_eq("Main/Viewport Count", 1)?;
                } else {
                    test.window_move("Game View", external_pos[0], external_pos[1])?;
                    test.yield_frames(ScriptCount::new(30)?)?;
                    test.assert_item_read_int_eq("Main/Viewport Count", 2)?;
                    test.dock_into("Game View", "Main")?;
                    test.yield_frames(ScriptCount::new(30)?)?;
                }
                Ok(())
            })?;
            engine.queue_tests(
                TestGroup::Tests,
                Some(test_name),
                RunFlags::RUN_FROM_COMMAND_LINE,
            )?;
            ViewportSmokeMode::Lifecycle(LifecycleSmokeState {
                require_secondary_while_held: drag_while_held,
                held_probe_armed: false,
                held_probe_pressed: false,
                held_probe_complete: false,
                saw_secondary_viewport: false,
                saw_secondary_while_held: false,
                saw_merged_viewport: false,
                secondary_submission_before_main_acquire: None,
                main_present_bracketed_by_test_engine: false,
            })
        };

        self.test_engine = Some(engine);
        self.viewport_smoke = Some(ViewportSmokeState {
            result_path: std::env::var_os("DEAR_IMGUI_VIEWPORT_SMOKE_JSON").map(PathBuf::from),
            adapter,
            mode,
            complete: false,
        });
        Ok(())
    }

    fn redraw_with_event_loop(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let viewport_runtime = self.viewport_runtime.take();
        let result = match viewport_runtime.as_ref() {
            Some(runtime) => match runtime.with_event_loop(event_loop, |_| self.redraw()) {
                Ok(result) => result,
                Err(error) => Err(Box::new(error) as Box<dyn std::error::Error>),
            },
            None => self.redraw(),
        };
        self.viewport_runtime = viewport_runtime;
        result
    }

    fn redraw(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Delta time is set by the platform backend in `prepare_frame()`.

        // First render a simple "game view" into the offscreen texture.
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
                        // Simple animated clear: color changes over time.
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
        #[cfg(feature = "test-engine")]
        let mut viewport_count =
            i32::try_from(self.imgui.platform_io().viewports_iter().count()).unwrap_or(i32::MAX);
        let frame = self.imgui.begin_frame();
        let ui = frame.ui();
        #[cfg(feature = "test-engine")]
        let running_upstream_suite = self
            .viewport_smoke
            .as_ref()
            .is_some_and(|smoke| matches!(&smoke.mode, ViewportSmokeMode::UpstreamSuite { .. }));
        #[cfg(not(feature = "test-engine"))]
        let running_upstream_suite = false;
        #[cfg(feature = "test-engine")]
        let show_held_drag_probe =
            self.viewport_smoke
                .as_ref()
                .is_some_and(|smoke| match &smoke.mode {
                    ViewportSmokeMode::Lifecycle(lifecycle) => {
                        lifecycle.require_secondary_while_held
                    }
                    ViewportSmokeMode::UpstreamSuite { .. } => false,
                });
        #[cfg(feature = "test-engine")]
        let mut arm_held_drag_probe = false;
        #[cfg(feature = "test-engine")]
        if let Some(smoke) = self.viewport_smoke.as_mut() {
            if let ViewportSmokeMode::Lifecycle(lifecycle) = &mut smoke.mode {
                if viewport_count > 1 {
                    lifecycle.saw_secondary_viewport = true;
                } else if lifecycle.saw_secondary_viewport {
                    lifecycle.saw_merged_viewport = true;
                }
            }
        }

        if !running_upstream_suite {
            // Keep a dockspace in the main viewport so it always has content
            ui.dockspace_over_main_viewport();

            // Simple UI that can be torn out into another viewport (when enabled)
            ui.window("Main")
                .size([420.0, 260.0], Condition::FirstUseEver)
                .build(|| {
                    if self.enable_viewports {
                        ui.text("Drag this window outside to create a new OS window.");
                        ui.separator();
                        ui.text("Multi-viewport is enabled (experimental).");
                    } else {
                        ui.text("Multi-viewport is disabled on this platform (winit + WGPU).");
                        ui.separator();
                        ui.text("Use the SDL3 + OpenGL example for a stable multi-viewport demo:");
                        ui.text("  cargo run -p dear-imgui-examples --bin sdl3_opengl_multi_viewport --features \"multi-viewport sdl3-opengl3\"");
                    }
                    #[cfg(feature = "test-engine")]
                    if self.test_engine.is_some() {
                        ui.input_int_config("Viewport Count")
                            .flags(dear_imgui_rs::InputScalarFlags::READ_ONLY)
                            .build(&mut viewport_count);
                        if show_held_drag_probe && ui.button("Begin Held Drag Probe") {
                            arm_held_drag_probe = true;
                        }
                    }
                });

            // "Game View" window showing the offscreen texture; you can drag this window
            // to any viewport (including secondary OS windows) and the texture will render
            // via the WGPU backend automatically.
            ui.window("Game View")
                .size([520.0, 540.0], Condition::FirstUseEver)
                .build(|| {
                    // Fit the game view into the available region while keeping it square.
                    let avail = ui.content_region_avail();
                    let side = avail[0].min(avail[1]).max(64.0);
                    let size = [side, side];
                    ui.text("Offscreen WGPU texture rendered each frame:");
                    ui.image(self.game_tex_id, size);
                });
        }

        #[cfg(feature = "test-engine")]
        if let Some(smoke) = self.viewport_smoke.as_mut()
            && let ViewportSmokeMode::Lifecycle(lifecycle) = &mut smoke.mode
            && lifecycle.require_secondary_while_held
        {
            if arm_held_drag_probe {
                lifecycle.held_probe_armed = true;
            }
            if lifecycle.held_probe_armed && !lifecycle.held_probe_complete {
                if ui.is_mouse_down(MouseButton::Left) {
                    lifecycle.held_probe_pressed = true;
                    if viewport_count > 1 {
                        lifecycle.saw_secondary_while_held = true;
                    }
                } else if lifecycle.held_probe_pressed {
                    lifecycle.held_probe_complete = true;
                }
            }
        }

        // Optionally show demo to validate interaction
        // let mut show_demo = true;
        // ui.show_demo_window(&mut show_demo);

        self.platform.prepare_render(ui, &self.window)?;

        #[cfg(feature = "test-engine")]
        let trace_secondary_submissions = self.viewport_smoke.as_ref().is_some_and(|smoke| {
            !smoke.complete && matches!(&smoke.mode, ViewportSmokeMode::Lifecycle(_))
        });

        #[cfg(feature = "test-engine")]
        let frame_index = {
            self.test_engine_frame_index = self
                .test_engine_frame_index
                .checked_add(1)
                .ok_or("Test Engine frame index exhausted")?;
            if self.test_engine_frame_index > 20_000
                && self.viewport_smoke.as_ref().is_some_and(|smoke| {
                    matches!(&smoke.mode, ViewportSmokeMode::UpstreamSuite { .. })
                })
            {
                return Err("upstream viewport suite exceeded its 20,000-frame budget".into());
            }
            self.test_engine_frame_index
        };

        let mut driver = MainSurfaceFrameDriver {
            renderer: &mut self.renderer,
            surface: &self.surface,
            surface_config: &self.surface_config,
            device: &self.device,
            queue: &self.queue,
            surface_frame: None,
            enable_viewports: self.enable_viewports,
            reconfigure_after_present: false,
            #[cfg(feature = "test-engine")]
            trace_secondary_submissions,
            #[cfg(feature = "test-engine")]
            secondary_submission_evidence: None,
            rendered: false,
        };

        #[cfg(feature = "test-engine")]
        let drive_outcome = if let Some(engine) = self.test_engine.as_mut() {
            engine
                .drive_frame(frame, frame_index, &mut driver)
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?
        } else {
            let reconciled = driver.prepare_frame(frame)?;
            match driver.render_main_frame(reconciled)? {
                MainSurfaceRenderOutcome::ReadyToPresent => {
                    driver.present_frame()?;
                    FrameDriveOutcome::Presented
                }
                MainSurfaceRenderOutcome::Skipped => FrameDriveOutcome::Skipped,
            }
        };
        #[cfg(not(feature = "test-engine"))]
        let was_presented = {
            let reconciled = driver.prepare_frame(frame)?;
            match driver.render_main_frame(reconciled)? {
                MainSurfaceRenderOutcome::ReadyToPresent => {
                    driver.present_frame()?;
                    true
                }
                MainSurfaceRenderOutcome::Skipped => false,
            }
        };

        #[cfg(feature = "test-engine")]
        let secondary_submission_evidence = driver.secondary_submission_evidence.take();
        #[cfg(feature = "test-engine")]
        let was_presented = matches!(drive_outcome, FrameDriveOutcome::Presented);
        drop(driver);

        #[cfg(feature = "test-engine")]
        if let Some(evidence) = secondary_submission_evidence
            && let Some(smoke) = self.viewport_smoke.as_mut()
            && let ViewportSmokeMode::Lifecycle(lifecycle) = &mut smoke.mode
            && lifecycle.secondary_submission_before_main_acquire.is_none()
        {
            lifecycle.secondary_submission_before_main_acquire = Some(evidence);
        }

        #[cfg(feature = "test-engine")]
        if let Some(engine) = self.test_engine.as_mut() {
            if let Some(smoke) = self.viewport_smoke.as_mut() {
                if let ViewportSmokeMode::Lifecycle(lifecycle) = &mut smoke.mode {
                    lifecycle.main_present_bracketed_by_test_engine = was_presented;
                }
            }
            let smoke_pending = self
                .viewport_smoke
                .as_ref()
                .is_some_and(|smoke| !smoke.complete);
            let terminal_summary = if smoke_pending {
                let smoke = self
                    .viewport_smoke
                    .as_ref()
                    .expect("a pending viewport smoke state must exist");
                match &smoke.mode {
                    ViewportSmokeMode::UpstreamSuite { suite, .. } => {
                        engine.take_terminal_test_suite_result(suite)?
                    }
                    ViewportSmokeMode::Lifecycle(_) => engine.take_terminal_summary()?,
                }
            } else {
                None
            };
            if let Some(summary) = terminal_summary {
                let smoke = self
                    .viewport_smoke
                    .as_mut()
                    .expect("a pending viewport smoke state must exist");
                match &mut smoke.mode {
                    ViewportSmokeMode::UpstreamSuite {
                        terminal_summary, ..
                    } => {
                        *terminal_summary = Some(summary);
                        println!("official upstream viewport Test Engine suite passed");
                    }
                    ViewportSmokeMode::Lifecycle(lifecycle) => {
                        if summary.count_tested != 1 || summary.count_success != 1 {
                            return Err(format!(
                                "viewport smoke failed: tested={}, success={}",
                                summary.count_tested, summary.count_success
                            )
                            .into());
                        }
                        if !lifecycle.saw_secondary_viewport
                            || lifecycle.require_secondary_while_held
                                && (!lifecycle.held_probe_complete
                                    || !lifecycle.saw_secondary_while_held)
                            || !lifecycle.saw_merged_viewport
                            || lifecycle.secondary_submission_before_main_acquire.is_none()
                            || !lifecycle.main_present_bracketed_by_test_engine
                        {
                            return Err(format!(
                                "viewport smoke did not observe the complete lifecycle and presentation order: secondary={}, secondary_while_held={}, held_probe_complete={}, merged={}, secondary_submission_before_main_acquire={:?}, main_present_bracketed={}",
                                lifecycle.saw_secondary_viewport,
                                lifecycle.saw_secondary_while_held,
                                lifecycle.held_probe_complete,
                                lifecycle.saw_merged_viewport,
                                lifecycle.secondary_submission_before_main_acquire,
                                lifecycle.main_present_bracketed_by_test_engine,
                            )
                            .into());
                        }
                        println!("WGPU multi-viewport Test Engine smoke passed");
                    }
                }
                smoke.complete = true;
            }
        }
        Ok(())
    }

    fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        if size.width > 0 && size.height > 0 {
            self.surface_config.width = size.width;
            self.surface_config.height = size.height;
            self.surface.configure(&self.device, &self.surface_config);
        }
    }
}

#[derive(Default)]
struct App {
    window: Option<AppWindow>,
    error: Option<String>,
}

impl App {
    fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.window.as_mut().map_or(Ok(()), AppWindow::shutdown)
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        match AppWindow::new(event_loop) {
            Ok(win) => {
                win.window.request_redraw();
                self.window = Some(win);
            }
            Err(error) => {
                self.error = Some(error.to_string());
                event_loop.exit();
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Continuously request redraw in Poll mode
        if let Some(app) = &self.window {
            app.window.request_redraw();
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
        if let Some(runtime) = app.viewport_runtime.as_ref() {
            if let Err(error) = runtime.handle_event(&mut app.platform, &mut app.imgui, &full) {
                self.error = Some(error.to_string());
                event_loop.exit();
                return;
            }
        } else {
            if let Err(error) = app
                .platform
                .handle_event(&mut app.imgui, &app.window, &full)
            {
                self.error = Some(error.to_string());
                event_loop.exit();
                return;
            }
        }

        match event {
            // Only exit when the main application window is closed.
            WindowEvent::CloseRequested if is_main_window => {
                event_loop.exit();
            }
            // Only reconfigure the main WGPU surface for the main window.
            WindowEvent::Resized(size) if is_main_window => {
                app.resize(size);
            }
            WindowEvent::ScaleFactorChanged { .. } if is_main_window => {
                app.resize(app.window.inner_size());
            }
            WindowEvent::RedrawRequested if is_main_window => {
                // We drive rendering from the main window. Secondary viewport windows are
                // rendered via ImGui's platform callbacks during `app.redraw()`.
                match app.redraw_with_event_loop(event_loop) {
                    Ok(()) => {
                        #[cfg(feature = "test-engine")]
                        if app
                            .viewport_smoke
                            .as_ref()
                            .is_some_and(|smoke| smoke.complete)
                        {
                            event_loop.exit();
                            return;
                        }
                        app.window.request_redraw();
                    }
                    Err(error) => {
                        self.error = Some(error.to_string());
                        event_loop.exit();
                    }
                }
            }
            _ => {}
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    let event_loop_result = event_loop.run_app(&mut app);
    let app_error = app.error.take();
    #[cfg(feature = "test-engine")]
    let smoke_result = app
        .window
        .as_ref()
        .and_then(|window| window.viewport_smoke.as_ref())
        .and_then(ViewportSmokeState::completed_result);
    let shutdown_result = app.shutdown();
    // A success artifact is evidence that renderer, platform, and Context teardown completed.
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
    if !errors.is_empty() {
        return Err(errors.join("; ").into());
    }

    #[cfg(feature = "test-engine")]
    if let Some(result) = smoke_result {
        result.write_after_teardown()?;
    }
    Ok(())
}

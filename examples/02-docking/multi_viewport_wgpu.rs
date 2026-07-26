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
use dear_imgui_rs::{Condition, Context, TextureId};
#[cfg(feature = "test-engine")]
use dear_imgui_test_engine::{
    RunFlags, RunSpeed, ScriptCount, TestEngine, TestGroup, VerboseLevel,
};
use dear_imgui_wgpu::{GammaMode, WgpuInitInfo, WgpuRenderer, multi_viewport as wgpu_mvp};
use dear_imgui_winit::{HiDpiMode, WinitPlatform, multi_viewport as winit_mvp};
use pollster::block_on;
#[cfg(feature = "test-engine")]
use std::{
    fmt,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use std::{sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

enum AppRenderer {
    Single(WgpuRenderer),
    Multi(wgpu_mvp::WinitViewportRuntime),
}

#[cfg(feature = "test-engine")]
struct ViewportSmokeState {
    result_path: Option<PathBuf>,
    adapter: wgpu::AdapterInfo,
    require_secondary_while_held: bool,
    held_probe_armed: bool,
    held_probe_pressed: bool,
    held_probe_complete: bool,
    saw_secondary_viewport: bool,
    saw_secondary_while_held: bool,
    saw_merged_viewport: bool,
    complete: bool,
}

#[cfg(feature = "test-engine")]
struct CompletedViewportSmoke {
    result_path: Option<PathBuf>,
    adapter: wgpu::AdapterInfo,
    saw_secondary_viewport: bool,
    saw_secondary_while_held: bool,
    saw_merged_viewport: bool,
}

#[cfg(feature = "test-engine")]
impl ViewportSmokeState {
    fn completed_result(&self) -> Option<CompletedViewportSmoke> {
        self.complete.then(|| CompletedViewportSmoke {
            result_path: self.result_path.clone(),
            adapter: self.adapter.clone(),
            saw_secondary_viewport: self.saw_secondary_viewport,
            saw_secondary_while_held: self.saw_secondary_while_held,
            saw_merged_viewport: self.saw_merged_viewport,
        })
    }
}

#[cfg(feature = "test-engine")]
impl CompletedViewportSmoke {
    fn write_after_teardown(self) -> Result<(), Box<dyn std::error::Error>> {
        let Some(path) = self.result_path else {
            return Ok(());
        };
        let json = format!(
            "{{\"schema_version\":1,\"outcome\":\"Passed\",\"adapter\":{{\"name\":\"{}\",\"backend\":\"{:?}\",\"device_type\":\"{:?}\",\"driver\":\"{}\",\"driver_info\":\"{}\",\"vendor\":{},\"device\":{}}},\"secondary_viewport_observed\":{},\"secondary_viewport_while_held_observed\":{},\"merge_observed\":{},\"teardown_complete\":true}}",
            json_escape(&self.adapter.name),
            self.adapter.backend,
            self.adapter.device_type,
            json_escape(&self.adapter.driver),
            json_escape(&self.adapter.driver_info),
            self.adapter.vendor,
            self.adapter.device,
            self.saw_secondary_viewport,
            self.saw_secondary_while_held,
            self.saw_merged_viewport,
        );
        write_json_atomic(&path, &json)
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
    fn new_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::Single(renderer) => renderer.new_frame()?,
            Self::Multi(runtime) => runtime.new_frame()?,
        }
        Ok(())
    }

    fn render_context_with_fb_size(
        &mut self,
        context: &mut Context,
        render_pass: &mut wgpu::RenderPass<'_>,
        width: u32,
        height: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::Single(renderer) => {
                renderer.render_context_with_fb_size(context, render_pass, width, height)?
            }
            Self::Multi(runtime) => {
                runtime.render_context_with_fb_size(context, render_pass, width, height)?
            }
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
        let run_viewport_smoke = run_viewport_drag_smoke
            || std::env::var("DEAR_IMGUI_VIEWPORT_SMOKE").is_ok_and(|value| value == "1");

        // Create WGPU instance first (also used by renderer for per-viewport surfaces)
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());

        let window: Arc<Window> = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Dear ImGui Multi-Viewport (wgpu)")
                        .with_inner_size(LogicalSize::new(1200.0, 720.0)),
                )?
                .into(),
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
        let game_tex_id = renderer.register_external_texture(&game_tex, &game_tex_view);

        let renderer = if enable_viewports {
            match wgpu_mvp::WinitViewportRuntime::attach(&mut imgui, renderer) {
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
            AppRenderer::Single(renderer)
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
            renderer_shutdown_complete: false,
            viewport_runtime_shutdown_complete: false,
            platform_shutdown_complete: false,
            imgui,
        };

        #[cfg(feature = "test-engine")]
        let app = {
            let mut app = app;
            if run_viewport_smoke {
                app.configure_viewport_smoke(adapter_info, run_viewport_drag_smoke)?;
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
        let test_name = if drag_while_held {
            "multi_viewport_held_undock_smoke"
        } else {
            "multi_viewport_surface_smoke"
        };

        let mut engine = TestEngine::create()?;
        engine.start(&mut self.imgui)?;
        engine.set_capture_enabled(false)?;
        engine.set_run_speed(if drag_while_held {
            RunSpeed::Normal
        } else {
            RunSpeed::Fast
        })?;
        engine.set_verbose_level(VerboseLevel::Info)?;
        engine.set_verbose_level_on_error(VerboseLevel::Debug)?;
        engine.set_log_to_tty(true)?;
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

        self.test_engine = Some(engine);
        self.viewport_smoke = Some(ViewportSmokeState {
            result_path: std::env::var_os("DEAR_IMGUI_VIEWPORT_SMOKE_JSON").map(PathBuf::from),
            adapter,
            require_secondary_while_held: drag_while_held,
            held_probe_armed: false,
            held_probe_pressed: false,
            held_probe_complete: false,
            saw_secondary_viewport: false,
            saw_secondary_while_held: false,
            saw_merged_viewport: false,
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

        let (frame, reconfigure_after_present) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.surface_config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("surface acquisition failed with a WGPU validation error".into());
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

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

        self.platform.prepare_frame(&self.window, &mut self.imgui)?;
        #[cfg(feature = "test-engine")]
        let mut viewport_count =
            i32::try_from(self.imgui.platform_io().viewports_iter().count()).unwrap_or(i32::MAX);
        let ui = self.imgui.frame();
        #[cfg(feature = "test-engine")]
        let show_held_drag_probe = self
            .viewport_smoke
            .as_ref()
            .is_some_and(|smoke| smoke.require_secondary_while_held);
        #[cfg(feature = "test-engine")]
        let mut arm_held_drag_probe = false;
        #[cfg(feature = "test-engine")]
        if let Some(smoke) = self.viewport_smoke.as_mut() {
            if viewport_count > 1 {
                smoke.saw_secondary_viewport = true;
            } else if smoke.saw_secondary_viewport {
                smoke.saw_merged_viewport = true;
            }
        }

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

        #[cfg(feature = "test-engine")]
        if let Some(smoke) = self.viewport_smoke.as_mut()
            && smoke.require_secondary_while_held
        {
            if arm_held_drag_probe {
                smoke.held_probe_armed = true;
            }
            if smoke.held_probe_armed && !smoke.held_probe_complete {
                if ui.is_mouse_down(MouseButton::Left) {
                    smoke.held_probe_pressed = true;
                    if viewport_count > 1 {
                        smoke.saw_secondary_while_held = true;
                    }
                } else if smoke.held_probe_pressed {
                    smoke.held_probe_complete = true;
                }
            }
        }

        // Optionally show demo to validate interaction
        // let mut show_demo = true;
        // ui.show_demo_window(&mut show_demo);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("imgui-main-encoder"),
            });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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

            self.renderer.new_frame()?;
            self.renderer.render_context_with_fb_size(
                &mut self.imgui,
                &mut rpass,
                self.surface_config.width,
                self.surface_config.height,
            )?;
        }

        // Submit and present main frame first to avoid cross-surface validation hazards
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
        if reconfigure_after_present {
            self.surface.configure(&self.device, &self.surface_config);
        }

        // Update + render all platform windows (secondary viewports)
        if self.enable_viewports {
            self.imgui.update_platform_windows();
            self.imgui.render_platform_windows_default();
        }

        #[cfg(feature = "test-engine")]
        if let Some(engine) = self.test_engine.as_mut() {
            engine.post_swap()?;
            let smoke_pending = self
                .viewport_smoke
                .as_ref()
                .is_some_and(|smoke| !smoke.complete);
            if smoke_pending && let Some(summary) = engine.take_terminal_summary()? {
                if summary.count_tested != 1 || summary.count_success != 1 {
                    return Err(format!(
                        "viewport smoke failed: tested={}, success={}",
                        summary.count_tested, summary.count_success
                    )
                    .into());
                }
                let smoke = self
                    .viewport_smoke
                    .as_mut()
                    .expect("a pending viewport smoke state must exist");
                if !smoke.saw_secondary_viewport
                    || smoke.require_secondary_while_held
                        && (!smoke.held_probe_complete || !smoke.saw_secondary_while_held)
                    || !smoke.saw_merged_viewport
                {
                    return Err(format!(
                        "viewport smoke did not observe the complete lifecycle: secondary={}, secondary_while_held={}, held_probe_complete={}, merged={}",
                        smoke.saw_secondary_viewport,
                        smoke.saw_secondary_while_held,
                        smoke.held_probe_complete,
                        smoke.saw_merged_viewport
                    )
                    .into());
                }
                println!("WGPU multi-viewport Test Engine smoke passed");
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
            WindowEvent::CloseRequested => {
                // Only exit when the main application window is closed.
                if is_main_window {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                // Only reconfigure the main WGPU surface for the main window.
                if is_main_window {
                    app.resize(size);
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if is_main_window {
                    app.resize(app.window.inner_size());
                }
            }
            WindowEvent::RedrawRequested => {
                // We drive rendering from the main window. Secondary viewport windows are
                // rendered via ImGui's platform callbacks during `app.redraw()`.
                if is_main_window {
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

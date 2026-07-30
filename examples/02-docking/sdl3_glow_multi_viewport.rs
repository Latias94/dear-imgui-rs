//! SDL3 + Glow multi-viewport example.
//!
//! This example drives Dear ImGui using:
//! - SDL3 for the main window, input, and secondary platform windows;
//! - the Rust Glow renderer backend (`dear-imgui-glow`);
//! - the high-level `dear-imgui-rs` API.
//!
//! It does not use the official OpenGL3 renderer from `dear-imgui-sdl3`.
//!
//! Run with:
//! ```text
//! cargo run -p dear-imgui-examples --bin sdl3_glow_multi_viewport \
//!     --features sdl3-glow-multi-viewport
//! ```
//!
//! Automated Linux secondary-window lifecycle smoke:
//! ```text
//! python3 tools/ci/run_contract.py sdl3-glow-multi-viewport-smoke
//! ```

use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;
use std::time::Instant;

use dear_imgui_examples::sdl3_callbacks::{
    Sdl3CallbackEventHandoff, configure_main_callback_rate, requests_exit,
};
use dear_imgui_glow::{GlowRenderer, SimpleTextureMap, multi_viewport::GlowViewportRuntime};
#[cfg(feature = "test-engine")]
use dear_imgui_rs::Id;
use dear_imgui_rs::{
    Condition, ConfigFlags, Context,
    render::{ReconciledFrame, RenderedFrame},
};
use dear_imgui_sdl3::{self as imgui_sdl3_backend, Sdl3PlatformBackend};
#[cfg(feature = "test-engine")]
use dear_imgui_test_engine::{
    RunFlags, RunSpeed, ScriptCount, TestEngine, TestFrameDriver, TestGroup, VerboseLevel,
};
use glow::HasContext;
use sdl3::video::{GLProfile, SwapInterval, WindowPos};
use sdl3_main::{AppResult, AppResultWithState, MainThreadData, app_impl};
use std::fmt;
#[cfg(feature = "test-engine")]
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

#[cfg(feature = "test-engine")]
#[derive(Clone)]
struct OpenGlRendererInfo {
    vendor: String,
    renderer: String,
    version: String,
}

#[cfg(feature = "test-engine")]
struct ViewportSmokeState {
    result_path: Option<PathBuf>,
    renderer: OpenGlRendererInfo,
    saw_secondary_viewport: bool,
    completed_frame_evidence: Option<SecondaryViewportFrameEvidence>,
    saw_merged_viewport: bool,
    main_present_bracketed_by_test_engine: bool,
    complete: bool,
}

#[cfg(feature = "test-engine")]
struct CompletedViewportSmoke {
    result_path: Option<PathBuf>,
    renderer: OpenGlRendererInfo,
    context_ready_viewports: Vec<Id>,
    glow_draw_issued_viewports: Vec<Id>,
    swap_succeeded_viewports: Vec<Id>,
    saw_merged_viewport: bool,
    main_present_bracketed_by_test_engine: bool,
}

#[cfg(feature = "test-engine")]
impl ViewportSmokeState {
    fn completed_result(&self) -> Option<CompletedViewportSmoke> {
        if !self.complete {
            return None;
        }
        let evidence = self.completed_frame_evidence.as_ref()?;
        Some(CompletedViewportSmoke {
            result_path: self.result_path.clone(),
            renderer: self.renderer.clone(),
            context_ready_viewports: evidence.context_activated_viewports.clone(),
            glow_draw_issued_viewports: evidence.glow_rendered_viewports.clone(),
            swap_succeeded_viewports: evidence.swapped_viewports.clone(),
            saw_merged_viewport: self.saw_merged_viewport,
            main_present_bracketed_by_test_engine: self.main_present_bracketed_by_test_engine,
        })
    }
}

#[cfg(feature = "test-engine")]
impl CompletedViewportSmoke {
    fn write_after_teardown(self) -> Result<(), Box<dyn Error>> {
        let Some(path) = self.result_path else {
            return Ok(());
        };
        let json = format!(
            "{{\"schema_version\":3,\"renderer\":{{\"backend\":\"OpenGL\",\"vendor\":\"{}\",\"name\":\"{}\",\"version\":\"{}\"}},\"secondary_context_ready_before_main_present_viewport_ids\":{},\"secondary_draw_issued_before_main_present_viewport_ids\":{},\"secondary_swap_succeeded_before_main_present_viewport_ids\":{},\"merge_observed\":{},\"main_present_bracketed_by_test_engine\":{}}}",
            json_escape(&self.renderer.vendor),
            json_escape(&self.renderer.renderer),
            json_escape(&self.renderer.version),
            viewport_ids_json(&self.context_ready_viewports),
            viewport_ids_json(&self.glow_draw_issued_viewports),
            viewport_ids_json(&self.swap_succeeded_viewports),
            self.saw_merged_viewport,
            self.main_present_bracketed_by_test_engine,
        );
        write_json_atomic(&path, &json)
    }
}

#[cfg(feature = "test-engine")]
fn viewport_ids_json(ids: &[Id]) -> String {
    let ids = ids
        .iter()
        .map(|id| id.raw().to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!("[{ids}]")
}

#[cfg(feature = "test-engine")]
fn query_opengl_renderer(gl: &glow::Context) -> OpenGlRendererInfo {
    unsafe {
        OpenGlRendererInfo {
            vendor: gl.get_parameter_string(glow::VENDOR),
            renderer: gl.get_parameter_string(glow::RENDERER),
            version: gl.get_parameter_string(glow::VERSION),
        }
    }
}

#[cfg(feature = "test-engine")]
fn validate_software_opengl_renderer(info: &OpenGlRendererInfo) -> Result<(), String> {
    let identity = format!("{} {} {}", info.vendor, info.renderer, info.version).to_lowercase();
    if !identity.contains("llvmpipe") && !identity.contains("lavapipe") {
        return Err(format!(
            "viewport smoke requires Mesa llvmpipe, selected '{}' ({}, {})",
            info.renderer, info.vendor, info.version
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
fn write_json_atomic(path: &Path, contents: &str) -> Result<(), Box<dyn Error>> {
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
        Ok::<_, Box<dyn Error>>(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(feature = "test-engine")]
type RunResult = Option<CompletedViewportSmoke>;
#[cfg(not(feature = "test-engine"))]
type RunResult = ();

struct GlowApp {
    main: MainThreadData<RefCell<Option<MainData>>>,
    events: Sdl3CallbackEventHandoff,
}

struct MainData {
    sdl3_backend: Sdl3PlatformBackend,
    imgui: Context,
    renderer: GlowViewportRuntime,
    gl: Rc<glow::Context>,
    gl_context: sdl3::video::GLContext,
    window: sdl3::video::Window,
    _video: sdl3::VideoSubsystem,
    _sdl: sdl3::Sdl,
    last_frame: Instant,
    #[cfg(feature = "test-engine")]
    test_engine: Option<TestEngine>,
    #[cfg(feature = "test-engine")]
    viewport_smoke: Option<ViewportSmokeState>,
    #[cfg(feature = "test-engine")]
    test_engine_frame_index: u64,
}

#[derive(Debug)]
struct SdlGlowFrameError {
    source: Box<dyn Error>,
}

impl SdlGlowFrameError {
    fn new(source: impl Error + 'static) -> Self {
        Self {
            source: Box::new(source),
        }
    }

    fn message(message: impl Into<String>) -> Self {
        Self::new(std::io::Error::other(message.into()))
    }
}

impl fmt::Display for SdlGlowFrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.source.fmt(formatter)
    }
}

impl Error for SdlGlowFrameError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

#[cfg(feature = "test-engine")]
#[derive(Debug, Default)]
struct SecondaryViewportFrameEvidence {
    context_activated_viewports: Vec<Id>,
    glow_rendered_viewports: Vec<Id>,
    swapped_viewports: Vec<Id>,
    completed_viewports: Vec<Id>,
}

#[cfg(feature = "test-engine")]
impl SecondaryViewportFrameEvidence {
    fn from_reports(
        glow: &dear_imgui_glow::multi_viewport::GlowViewportFrameReport,
        sdl3: &dear_imgui_sdl3::Sdl3OpenGlViewportFrameReport,
    ) -> Self {
        let context_activated_viewports = sdl3.context_activated_viewports().to_vec();
        let glow_rendered_viewports = glow.rendered_viewports().to_vec();
        let swapped_viewports = sdl3.swapped_viewports().to_vec();
        let completed_viewports = glow_rendered_viewports
            .iter()
            .copied()
            .filter(|id| context_activated_viewports.contains(id) && swapped_viewports.contains(id))
            .collect();
        Self {
            context_activated_viewports,
            glow_rendered_viewports,
            swapped_viewports,
            completed_viewports,
        }
    }
}

struct SdlGlowFrameDriver<'a> {
    sdl3_backend: &'a Sdl3PlatformBackend,
    renderer: &'a GlowViewportRuntime,
    gl: &'a glow::Context,
    window: &'a sdl3::video::Window,
    gl_context: &'a sdl3::video::GLContext,
    rendered: bool,
    #[cfg(feature = "test-engine")]
    secondary_viewport_evidence: Option<SecondaryViewportFrameEvidence>,
    presented: bool,
}

impl SdlGlowFrameDriver<'_> {
    fn render_frame<'frame>(
        &mut self,
        frame: RenderedFrame<'frame>,
    ) -> Result<ReconciledFrame<'frame>, SdlGlowFrameError> {
        if self.rendered {
            return Err(SdlGlowFrameError::message(
                "main OpenGL frame was rendered more than once",
            ));
        }
        unsafe {
            let (width, height) = self.window.size_in_pixels();
            self.gl.viewport(0, 0, width as i32, height as i32);
            self.gl.clear_color(0.1, 0.12, 0.15, 1.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
        }

        self.renderer.new_frame().map_err(SdlGlowFrameError::new)?;
        #[cfg(feature = "test-engine")]
        let sdl3_trace = self
            .sdl3_backend
            .begin_opengl_viewport_frame_trace()
            .map_err(SdlGlowFrameError::new)?;
        #[cfg(feature = "test-engine")]
        let glow_trace = self
            .renderer
            .begin_frame_trace()
            .map_err(SdlGlowFrameError::new)?;
        let render_result = self
            .renderer
            .render_with_platform_windows_reconciled(frame)
            .map_err(SdlGlowFrameError::new);
        let restore_result = self
            .window
            .gl_make_current(self.gl_context)
            .map_err(|error| {
                SdlGlowFrameError::message(format!(
                    "failed to restore the main OpenGL context: {error}"
                ))
            });
        #[cfg(feature = "test-engine")]
        let glow_report = glow_trace.finish();
        #[cfg(feature = "test-engine")]
        let sdl3_report = sdl3_trace.finish();
        let glow_fault = self.renderer.poll_fault().map_err(SdlGlowFrameError::new);
        let sdl3_fault = self
            .sdl3_backend
            .poll_fault()
            .map_err(SdlGlowFrameError::new);

        restore_result?;
        glow_fault?;
        sdl3_fault?;
        let reconciled = render_result?;
        #[cfg(feature = "test-engine")]
        {
            self.secondary_viewport_evidence = Some(SecondaryViewportFrameEvidence::from_reports(
                &glow_report,
                &sdl3_report,
            ));
        }
        self.rendered = true;
        Ok(reconciled)
    }

    fn present_frame(&mut self) -> Result<(), SdlGlowFrameError> {
        if !self.rendered {
            return Err(SdlGlowFrameError::message(
                "main OpenGL window was presented before frame and platform rendering completed",
            ));
        }
        #[cfg(feature = "test-engine")]
        if self.secondary_viewport_evidence.is_none() {
            return Err(SdlGlowFrameError::message(
                "main OpenGL window was presented before viewport evidence was collected",
            ));
        }
        if self.presented {
            return Err(SdlGlowFrameError::message(
                "main OpenGL window was presented more than once",
            ));
        }
        self.window.gl_swap_window();
        self.presented = true;
        Ok(())
    }
}

#[cfg(feature = "test-engine")]
impl TestFrameDriver for SdlGlowFrameDriver<'_> {
    type RenderError = SdlGlowFrameError;
    type PresentError = SdlGlowFrameError;

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

impl GlowApp {
    fn new() -> Result<Self, Box<dyn Error>> {
        imgui_sdl3_backend::enable_native_ime_ui();
        configure_main_callback_rate();
        Ok(Self {
            main: MainThreadData::assert_new(RefCell::new(Some(MainData::new()?))),
            events: Sdl3CallbackEventHandoff::default(),
        })
    }

    fn process_events(&self) -> Result<AppResult, Box<dyn Error>> {
        let mut events = self.events.drain();
        let mut main = self.main.assert_get().borrow_mut();
        let main = main
            .as_mut()
            .expect("SDL3 Glow state must be active while callbacks run");
        while let Some(event) = events.pop() {
            event.with_imgui_event(|raw| -> Result<(), Box<dyn Error>> {
                if let Some(raw) = raw {
                    let _ = main.sdl3_backend.process_event(&mut main.imgui, raw)?;
                }
                Ok(())
            })?;
            if requests_exit(&event, main.window.id()) {
                return Ok(AppResult::Success);
            }
        }
        Ok(AppResult::Continue)
    }

    fn render(&self) -> Result<bool, Box<dyn Error>> {
        self.main
            .assert_get()
            .borrow_mut()
            .as_mut()
            .expect("SDL3 Glow state must be active while callbacks run")
            .render()
    }

    fn shutdown(&self) {
        let main = self.main.assert_get().borrow_mut().take();
        let Some(main) = main else {
            return;
        };
        match main.shutdown() {
            #[cfg(feature = "test-engine")]
            Ok(Some(result)) => {
                // `MainData::shutdown` consumes and drops every SDL, GL, renderer, backend, and
                // Context owner before returning the smoke summary.
                if let Err(error) = result.write_after_teardown() {
                    eprintln!("failed to write SDL3/Glow viewport smoke result: {error}");
                }
            }
            Ok(_) => {}
            Err(error) => eprintln!("SDL3/Glow shutdown failed: {error}"),
        }
    }
}

impl MainData {
    fn new() -> Result<Self, Box<dyn Error>> {
        let sdl = sdl3::init()?;
        let video = sdl.video()?;
        #[cfg(feature = "test-engine")]
        let run_viewport_smoke =
            std::env::var("DEAR_IMGUI_VIEWPORT_SMOKE").is_ok_and(|value| value == "1");

        let gl_attr = video.gl_attr();
        gl_attr.set_context_version(3, 2);
        gl_attr.set_context_profile(GLProfile::Core);
        gl_attr.set_depth_size(0);

        let main_scale = video
            .get_primary_display()?
            .get_content_scale()
            .unwrap_or(1.0);

        let mut window = video
            .window(
                "Dear ImGui + SDL3 + Glow (multi-viewport)",
                (800.0 * main_scale) as u32,
                (600.0 * main_scale) as u32,
            )
            .opengl()
            .resizable()
            .hidden()
            .high_pixel_density()
            .build()
            .map_err(|error| format!("failed to create SDL3 window: {error}"))?;

        let gl_context = window
            .gl_create_context()
            .map_err(|error| format!("SDL_GL_CreateContext failed: {error}"))?;
        window
            .gl_make_current(&gl_context)
            .map_err(|error| format!("SDL_GL_MakeCurrent failed: {error}"))?;
        #[cfg(feature = "test-engine")]
        let main_swap_interval = if run_viewport_smoke {
            SwapInterval::Immediate
        } else {
            SwapInterval::VSync
        };
        #[cfg(not(feature = "test-engine"))]
        let main_swap_interval = SwapInterval::VSync;
        let _ = video.gl_set_swap_interval(main_swap_interval);
        window.set_position(WindowPos::Centered, WindowPos::Centered);
        window.show();

        let gl = Rc::new(unsafe { create_glow_context(&video) });

        #[cfg(feature = "test-engine")]
        let renderer_info = query_opengl_renderer(&gl);
        #[cfg(feature = "test-engine")]
        if run_viewport_smoke {
            println!(
                "OpenGL renderer: vendor='{}', renderer='{}', version='{}'",
                renderer_info.vendor, renderer_info.renderer, renderer_info.version
            );
            if std::env::var("DEAR_IMGUI_REQUIRE_SOFTWARE_OPENGL").is_ok_and(|value| value == "1") {
                validate_software_opengl_renderer(&renderer_info)?;
            }
        }

        let mut imgui = Context::create();
        #[cfg(feature = "test-engine")]
        if run_viewport_smoke {
            imgui.set_ini_filename(None::<String>)?;
        }
        {
            let io = imgui.io_mut();
            let mut flags = io.config_flags();
            flags.insert(ConfigFlags::DOCKING_ENABLE | ConfigFlags::VIEWPORTS_ENABLE);
            io.set_config_flags(flags);
        }

        // SAFETY: `window` and `gl_context` outlive renderer/platform shutdown and Context teardown.
        let sdl3_backend = unsafe {
            Sdl3PlatformBackend::init_platform_for_opengl(&mut imgui, &window, &gl_context)?
        };

        let window_scale = window.display_scale();
        imgui.style_mut().set_font_scale_dpi(window_scale);

        let renderer = GlowRenderer::with_shared_context(
            Rc::clone(&gl),
            &mut imgui,
            Box::new(SimpleTextureMap::default()),
        )?;
        // SAFETY: SDL3's OpenGL viewport backend creates every secondary context in the main
        // context's share group and makes the matching context current for renderer callbacks. The
        // frame driver explicitly restores `gl_context` after the platform-window pump.
        let renderer = unsafe { GlowViewportRuntime::attach(&mut imgui, renderer)? };

        #[cfg(feature = "test-engine")]
        let test_engine = if run_viewport_smoke {
            let (main_x, main_y) = window.position();
            let (main_width, _) = window.size();
            let external_pos = [
                main_x as f32 + main_width as f32 + 100.0,
                main_y as f32 + 100.0,
            ];
            let merged_pos = [main_x as f32 + 100.0, main_y as f32 + 100.0];

            let mut engine = TestEngine::create()?;
            engine.start(&mut imgui)?;
            engine.set_run_speed(RunSpeed::Fast)?;
            engine.set_verbose_level(VerboseLevel::Info)?;
            engine.add_script_test("sdl3-glow", "multi_viewport_surface_smoke", move |test| {
                test.wait_for_item("Main/Viewport Count", ScriptCount::new(240)?)?;
                test.window_move("Main", external_pos[0], external_pos[1])?;
                test.yield_frames(ScriptCount::new(30)?)?;
                test.assert_item_read_int_eq("Main/Viewport Count", 2)?;
                test.window_move("Main", merged_pos[0], merged_pos[1])?;
                test.yield_frames(ScriptCount::new(30)?)?;
                test.assert_item_read_int_eq("Main/Viewport Count", 1)
            })?;
            engine.queue_tests(
                TestGroup::Tests,
                Some("multi_viewport_surface_smoke"),
                RunFlags::RUN_FROM_COMMAND_LINE,
            )?;
            Some(engine)
        } else {
            None
        };

        #[cfg(feature = "test-engine")]
        let viewport_smoke = run_viewport_smoke.then(|| ViewportSmokeState {
            result_path: std::env::var_os("DEAR_IMGUI_VIEWPORT_SMOKE_JSON").map(PathBuf::from),
            renderer: renderer_info,
            saw_secondary_viewport: false,
            completed_frame_evidence: None,
            saw_merged_viewport: false,
            main_present_bracketed_by_test_engine: false,
            complete: false,
        });

        Ok(Self {
            sdl3_backend,
            imgui,
            renderer,
            gl,
            gl_context,
            window,
            _video: video,
            _sdl: sdl,
            last_frame: Instant::now(),
            #[cfg(feature = "test-engine")]
            test_engine,
            #[cfg(feature = "test-engine")]
            viewport_smoke,
            #[cfg(feature = "test-engine")]
            test_engine_frame_index: 0,
        })
    }

    fn render(&mut self) -> Result<bool, Box<dyn Error>> {
        let now = Instant::now();
        self.imgui
            .io_mut()
            .set_delta_time((now - self.last_frame).as_secs_f32());
        self.last_frame = now;

        self.sdl3_backend.new_frame(&mut self.imgui)?;
        #[cfg(feature = "test-engine")]
        let mut viewport_count =
            i32::try_from(self.imgui.platform_io().viewports_iter().count()).unwrap_or(i32::MAX);
        #[cfg(feature = "test-engine")]
        if let Some(smoke) = self.viewport_smoke.as_mut() {
            if viewport_count > 1 {
                smoke.saw_secondary_viewport = true;
            } else if smoke.saw_secondary_viewport {
                smoke.saw_merged_viewport = true;
            }
        }

        let ui = self.imgui.frame();
        ui.dockspace_over_main_viewport();
        ui.window("Main")
            .size([420.0, 260.0], Condition::FirstUseEver)
            .build(|| {
                ui.text("SDL3 + Glow + Dear ImGui multi-viewport");
                ui.separator();
                ui.text("Drag this window outside the main viewport to spawn an OS window.");
                #[cfg(feature = "test-engine")]
                if self.test_engine.is_some() {
                    ui.input_int_config("Viewport Count")
                        .flags(dear_imgui_rs::InputScalarFlags::READ_ONLY)
                        .build(&mut viewport_count);
                }
            });

        let frame = self.imgui.render();
        #[cfg(feature = "test-engine")]
        let frame_index = {
            self.test_engine_frame_index = self
                .test_engine_frame_index
                .checked_add(1)
                .ok_or("Test Engine frame index exhausted")?;
            self.test_engine_frame_index
        };
        #[cfg(feature = "test-engine")]
        let used_test_engine = self.test_engine.is_some();

        let mut driver = SdlGlowFrameDriver {
            sdl3_backend: &self.sdl3_backend,
            renderer: &self.renderer,
            gl: self.gl.as_ref(),
            window: &self.window,
            gl_context: &self.gl_context,
            rendered: false,
            #[cfg(feature = "test-engine")]
            secondary_viewport_evidence: None,
            presented: false,
        };
        #[cfg(feature = "test-engine")]
        let presentation_result: Result<(), Box<dyn Error>> =
            if let Some(engine) = self.test_engine.as_mut() {
                engine
                    .drive_frame(frame, frame_index, &mut driver)
                    .map_err(|error| Box::new(error) as Box<dyn Error>)
            } else {
                let reconciled = driver.render_frame(frame)?;
                drop(reconciled);
                driver
                    .present_frame()
                    .map_err(|error| Box::new(error) as Box<dyn Error>)
            };
        #[cfg(not(feature = "test-engine"))]
        let presentation_result: Result<(), Box<dyn Error>> = {
            let reconciled = driver.render_frame(frame)?;
            drop(reconciled);
            driver
                .present_frame()
                .map_err(|error| Box::new(error) as Box<dyn Error>)
        };

        #[cfg(feature = "test-engine")]
        let secondary_viewport_evidence = driver
            .secondary_viewport_evidence
            .take()
            .unwrap_or_default();
        #[cfg(feature = "test-engine")]
        let was_presented = driver.presented;
        drop(driver);
        presentation_result?;

        #[cfg(feature = "test-engine")]
        if let Some(smoke) = self.viewport_smoke.as_mut()
            && smoke.completed_frame_evidence.is_none()
            && !secondary_viewport_evidence.completed_viewports.is_empty()
        {
            smoke.completed_frame_evidence = Some(secondary_viewport_evidence);
            smoke.main_present_bracketed_by_test_engine = used_test_engine && was_presented;
        }

        #[cfg(feature = "test-engine")]
        if let Some(engine) = self.test_engine.as_mut() {
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
                    || smoke.completed_frame_evidence.is_none()
                    || !smoke.saw_merged_viewport
                    || !smoke.main_present_bracketed_by_test_engine
                {
                    let evidence = smoke.completed_frame_evidence.as_ref();
                    return Err(format!(
                        "viewport smoke did not observe one secondary viewport completing the native-context, Glow-draw, and native-swap stages in the same frame before the main present: secondary={}, context_ready={:?}, glow_drawn={:?}, swapped={:?}, completed={:?}, merged={}, main_present_bracketed={}",
                        smoke.saw_secondary_viewport,
                        evidence.map(|evidence| &evidence.context_activated_viewports),
                        evidence.map(|evidence| &evidence.glow_rendered_viewports),
                        evidence.map(|evidence| &evidence.swapped_viewports),
                        evidence.map(|evidence| &evidence.completed_viewports),
                        smoke.saw_merged_viewport,
                        smoke.main_present_bracketed_by_test_engine,
                    )
                    .into());
                }
                println!("SDL3/Glow multi-viewport Test Engine smoke passed");
                smoke.complete = true;
            }
        }
        #[cfg(feature = "test-engine")]
        return Ok(self
            .viewport_smoke
            .as_ref()
            .is_some_and(|smoke| smoke.complete));

        #[cfg(not(feature = "test-engine"))]
        Ok(false)
    }

    fn shutdown(mut self) -> Result<RunResult, Box<dyn Error>> {
        #[cfg(feature = "test-engine")]
        let completed_result = self
            .viewport_smoke
            .as_ref()
            .and_then(ViewportSmokeState::completed_result);
        #[cfg(feature = "test-engine")]
        if let Some(engine) = self.test_engine.as_mut() {
            engine.shutdown()?;
        }
        self.renderer.shutdown(&mut self.imgui)?;
        self.sdl3_backend.shutdown(&mut self.imgui)?;

        #[cfg(feature = "test-engine")]
        return Ok(completed_result);
        #[cfg(not(feature = "test-engine"))]
        Ok(())
    }
}

#[app_impl]
impl GlowApp {
    fn app_init() -> AppResultWithState<Box<Self>> {
        match Self::new() {
            Ok(app) => AppResultWithState::Continue(Box::new(app)),
            Err(error) => {
                eprintln!("failed to initialize SDL3 Glow example: {error}");
                AppResultWithState::Failure(None)
            }
        }
    }

    fn app_iterate(&self) -> AppResult {
        match self.process_events() {
            Ok(AppResult::Continue) => {}
            Ok(result) => return result,
            Err(error) => {
                eprintln!("SDL3 Glow event processing failed: {error}");
                return AppResult::Failure;
            }
        }
        match self.render() {
            Ok(true) => AppResult::Success,
            Ok(false) => AppResult::Continue,
            Err(error) => {
                eprintln!("SDL3 Glow frame failed: {error}");
                AppResult::Failure
            }
        }
    }

    fn app_event(&self, raw: &sdl3::sys::events::SDL_Event) -> AppResult {
        self.events.push(raw);
        AppResult::Continue
    }

    fn app_quit(state: Option<&Self>) {
        if let Some(app) = state {
            app.shutdown();
        }
    }
}

/// Create a Glow context from an SDL3 `VideoSubsystem`.
///
/// # Safety
///
/// Call this only after there is a current OpenGL context for the thread.
unsafe fn create_glow_context(video: &sdl3::VideoSubsystem) -> glow::Context {
    use std::ffi::c_void;

    unsafe {
        glow::Context::from_loader_function(|name| {
            video
                .gl_get_proc_address(name)
                .map(|function| function as *const c_void)
                .unwrap_or(std::ptr::null())
        })
    }
}

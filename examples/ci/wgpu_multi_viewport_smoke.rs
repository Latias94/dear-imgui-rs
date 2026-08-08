//! Private Winit + WGPU multi-viewport runtime contract for CI.

#[path = "../support/wgpu_multi_viewport_runtime.rs"]
mod wgpu_multi_viewport_runtime;

use dear_imgui_rs::{Context, FrameToken, MouseButton, Ui};
use dear_imgui_test_engine::{
    BuiltInTestSuite, FrameDriveOutcome, MainRenderOutcome, RegisteredTestSuite, ResultSummary,
    RunFlags, RunSpeed, ScriptCount, TestEngine, TestFrameDriver, TestGroup, VerboseLevel,
};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use wgpu_multi_viewport_runtime::{
    AppPreparedFrame, MainSurfaceFrameDriver, MainSurfaceFrameError, MainSurfaceRenderOutcome,
    SecondarySubmissionEvidence, ViewportScenario, run,
};
use winit::window::Window;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SmokeSelection {
    Lifecycle { drag_while_held: bool },
    UpstreamSuite,
}

struct ViewportSmokeScenario {
    result_path: Option<PathBuf>,
    require_software_vulkan: bool,
    selection: SmokeSelection,
    adapter: Option<wgpu::AdapterInfo>,
    engine: Option<TestEngine>,
    mode: Option<ViewportSmokeMode>,
    last_drive_presented: Option<bool>,
    complete: bool,
}

enum ViewportSmokeMode {
    Lifecycle(LifecycleSmokeState),
    UpstreamSuite {
        suite: RegisteredTestSuite,
        terminal_summary: Option<ResultSummary>,
    },
}

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

impl ViewportSmokeScenario {
    fn from_environment() -> Result<Self, Box<dyn std::error::Error>> {
        let drag_while_held =
            std::env::var("DEAR_IMGUI_VIEWPORT_DRAG_SMOKE").is_ok_and(|value| value == "1");
        let upstream_suite =
            std::env::var("DEAR_IMGUI_UPSTREAM_VIEWPORT_SUITE").is_ok_and(|value| value == "1");
        let lifecycle = std::env::var("DEAR_IMGUI_VIEWPORT_SMOKE").is_ok_and(|value| value == "1");
        if drag_while_held && upstream_suite {
            return Err(
                "DEAR_IMGUI_VIEWPORT_DRAG_SMOKE and DEAR_IMGUI_UPSTREAM_VIEWPORT_SUITE are mutually exclusive"
                    .into(),
            );
        }
        if !lifecycle && !drag_while_held && !upstream_suite {
            return Err(
                "private WGPU viewport runtime requires DEAR_IMGUI_VIEWPORT_SMOKE=1, DEAR_IMGUI_VIEWPORT_DRAG_SMOKE=1, or DEAR_IMGUI_UPSTREAM_VIEWPORT_SUITE=1"
                    .into(),
            );
        }
        let selection = if upstream_suite {
            SmokeSelection::UpstreamSuite
        } else {
            SmokeSelection::Lifecycle { drag_while_held }
        };
        Ok(Self {
            result_path: std::env::var_os("DEAR_IMGUI_VIEWPORT_SMOKE_JSON").map(PathBuf::from),
            require_software_vulkan: std::env::var("DEAR_IMGUI_REQUIRE_SOFTWARE_VULKAN")
                .is_ok_and(|value| value == "1"),
            selection,
            adapter: None,
            engine: None,
            mode: None,
            last_drive_presented: None,
            complete: false,
        })
    }

    fn completed_result(&self) -> Option<CompletedViewportSmoke> {
        if !self.complete {
            return None;
        }
        let adapter = self.adapter.as_ref()?.clone();
        match self.mode.as_ref()? {
            ViewportSmokeMode::UpstreamSuite {
                suite,
                terminal_summary,
            } => Some(CompletedViewportSmoke::UpstreamSuite {
                result_path: self.result_path.clone(),
                adapter,
                suite: suite.clone(),
                summary: (*terminal_summary)?,
            }),
            ViewportSmokeMode::Lifecycle(lifecycle) => Some(CompletedViewportSmoke::Lifecycle {
                result_path: self.result_path.clone(),
                adapter,
                saw_secondary_while_held: lifecycle.saw_secondary_while_held,
                saw_merged_viewport: lifecycle.saw_merged_viewport,
                secondary_submission_before_main_acquire: lifecycle
                    .secondary_submission_before_main_acquire
                    .as_ref()?
                    .clone(),
                main_present_bracketed_by_test_engine: lifecycle
                    .main_present_bracketed_by_test_engine,
            }),
        }
    }
}

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

fn json_u32_array(values: &[u32]) -> String {
    let values = values
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    format!("[{values}]")
}

fn json_string_array(values: &[String]) -> String {
    let values = values
        .iter()
        .map(|value| format!("\"{}\"", json_escape(value)))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{values}]")
}

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

impl TestFrameDriver for MainSurfaceFrameDriver<'_> {
    type PreparedFrame<'frame> = AppPreparedFrame<'frame>;
    type PrepareError = MainSurfaceFrameError;
    type RenderError = MainSurfaceFrameError;
    type PresentError = MainSurfaceFrameError;

    fn prepare<'frame>(
        &mut self,
        frame: FrameToken<'frame>,
        _frame_index: u64,
    ) -> Result<Self::PreparedFrame<'frame>, Self::PrepareError> {
        self.prepare_frame(frame)
    }

    fn prepared_context_id(frame: &Self::PreparedFrame<'_>) -> dear_imgui_rs::ContextId {
        match frame {
            AppPreparedFrame::Single(frame) => frame.context_id(),
            AppPreparedFrame::Multi(frame) => frame.context_id(),
        }
    }

    fn render_main(
        &mut self,
        frame: Self::PreparedFrame<'_>,
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

impl ViewportScenario for ViewportSmokeScenario {
    type Output = CompletedViewportSmoke;

    fn initialize(
        &mut self,
        context: &mut Context,
        window: &Window,
        adapter: &wgpu::AdapterInfo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!(
            "WGPU adapter: name='{}', backend={:?}, device_type={:?}, driver='{}', info='{}'",
            adapter.name, adapter.backend, adapter.device_type, adapter.driver, adapter.driver_info,
        );
        if self.require_software_vulkan {
            validate_software_vulkan_adapter(adapter)?;
        }
        context.set_ini_filename(None::<String>)?;
        let main_pos = window
            .inner_position()
            .unwrap_or_else(|_| winit::dpi::PhysicalPosition::new(0, 0));
        let main_size = window.inner_size();
        #[cfg(target_os = "macos")]
        let (main_pos, main_size) = {
            let scale = window.scale_factor();
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
        self.adapter = Some(adapter.clone());
        self.engine = Some(TestEngine::create()?);
        self.engine
            .as_mut()
            .ok_or_else(|| std::io::Error::other("Test Engine initialization was lost"))?
            .start(context)?;
        let engine = self
            .engine
            .as_mut()
            .ok_or_else(|| std::io::Error::other("Test Engine initialization was lost"))?;
        let mode = match self.selection {
            SmokeSelection::UpstreamSuite => {
                engine.set_run_speed(RunSpeed::Fast)?;
                engine.set_verbose_level(VerboseLevel::Info)?;
                engine.set_verbose_level_on_error(VerboseLevel::Debug)?;
                engine.set_log_to_tty(true)?;
                let suite =
                    engine.register_builtin_test_suite(BuiltInTestSuite::UpstreamViewports)?;
                engine.queue_tests(
                    TestGroup::Tests,
                    Some(suite.suite().category()),
                    RunFlags::RUN_FROM_COMMAND_LINE,
                )?;
                ViewportSmokeMode::UpstreamSuite {
                    suite,
                    terminal_summary: None,
                }
            }
            SmokeSelection::Lifecycle { drag_while_held } => {
                engine.set_run_speed(if drag_while_held {
                    RunSpeed::Normal
                } else {
                    RunSpeed::Fast
                })?;
                engine.set_verbose_level(VerboseLevel::Info)?;
                engine.set_verbose_level_on_error(VerboseLevel::Debug)?;
                engine.set_log_to_tty(true)?;
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
            }
        };
        self.mode = Some(mode);
        Ok(())
    }

    fn show_example_ui(&self) -> bool {
        !matches!(
            self.mode.as_ref(),
            Some(ViewportSmokeMode::UpstreamSuite { .. })
        )
    }

    fn before_ui(&mut self, viewport_count: i32) {
        let Some(ViewportSmokeMode::Lifecycle(lifecycle)) = self.mode.as_mut() else {
            return;
        };
        if viewport_count > 1 {
            lifecycle.saw_secondary_viewport = true;
        } else if lifecycle.saw_secondary_viewport {
            lifecycle.saw_merged_viewport = true;
        }
    }

    fn extend_main_window(&mut self, ui: &Ui, viewport_count: &mut i32) {
        ui.input_int_config("Viewport Count")
            .flags(dear_imgui_rs::InputScalarFlags::READ_ONLY)
            .build(viewport_count);
        let Some(ViewportSmokeMode::Lifecycle(lifecycle)) = self.mode.as_mut() else {
            return;
        };
        if lifecycle.require_secondary_while_held && ui.button("Begin Held Drag Probe") {
            lifecycle.held_probe_armed = true;
        }
    }

    fn after_ui(&mut self, ui: &Ui, viewport_count: i32) {
        let Some(ViewportSmokeMode::Lifecycle(lifecycle)) = self.mode.as_mut() else {
            return;
        };
        if !lifecycle.require_secondary_while_held
            || !lifecycle.held_probe_armed
            || lifecycle.held_probe_complete
        {
            return;
        }
        if ui.is_mouse_down(MouseButton::Left) {
            lifecycle.held_probe_pressed = true;
            if viewport_count > 1 {
                lifecycle.saw_secondary_while_held = true;
            }
        } else if lifecycle.held_probe_pressed {
            lifecycle.held_probe_complete = true;
        }
    }

    fn trace_secondary_submissions(&self) -> bool {
        !self.complete && matches!(self.mode.as_ref(), Some(ViewportSmokeMode::Lifecycle(_)))
    }

    fn drive_frame<'frame>(
        &mut self,
        frame: FrameToken<'frame>,
        frame_index: u64,
        driver: &mut MainSurfaceFrameDriver<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if frame_index > 20_000
            && matches!(
                self.mode.as_ref(),
                Some(ViewportSmokeMode::UpstreamSuite { .. })
            )
        {
            return Err("upstream viewport suite exceeded its 20,000-frame budget".into());
        }
        let outcome = self
            .engine
            .as_mut()
            .ok_or_else(|| std::io::Error::other("Test Engine is unavailable"))?
            .drive_frame(frame, frame_index, driver)
            .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?;
        self.last_drive_presented = Some(matches!(outcome, FrameDriveOutcome::Presented));
        Ok(())
    }

    fn after_frame(
        &mut self,
        presented: bool,
        secondary_submission_evidence: Option<SecondarySubmissionEvidence>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let bracketed_present = self
            .last_drive_presented
            .take()
            .ok_or("Test Engine did not report the frame drive outcome")?;
        if bracketed_present != presented {
            return Err(format!(
                "Test Engine frame outcome disagreed with the main surface transaction: outcome_presented={bracketed_present}, surface_presented={presented}"
            )
            .into());
        }
        if let Some(evidence) = secondary_submission_evidence
            && let Some(ViewportSmokeMode::Lifecycle(lifecycle)) = self.mode.as_mut()
            && lifecycle.secondary_submission_before_main_acquire.is_none()
        {
            lifecycle.secondary_submission_before_main_acquire = Some(evidence);
        }
        if let Some(ViewportSmokeMode::Lifecycle(lifecycle)) = self.mode.as_mut() {
            lifecycle.main_present_bracketed_by_test_engine = bracketed_present;
        }
        if self.complete {
            return Ok(());
        }
        let engine = self
            .engine
            .as_mut()
            .ok_or_else(|| std::io::Error::other("Test Engine is unavailable"))?;
        let terminal_summary = match self.mode.as_ref() {
            Some(ViewportSmokeMode::UpstreamSuite { suite, .. }) => {
                engine.take_terminal_test_suite_result(suite)?
            }
            Some(ViewportSmokeMode::Lifecycle(_)) => engine.take_terminal_summary()?,
            None => return Err("viewport smoke mode is unavailable".into()),
        };
        let Some(summary) = terminal_summary else {
            return Ok(());
        };
        match self.mode.as_mut() {
            Some(ViewportSmokeMode::UpstreamSuite {
                terminal_summary, ..
            }) => {
                *terminal_summary = Some(summary);
                println!("official upstream viewport Test Engine suite passed");
            }
            Some(ViewportSmokeMode::Lifecycle(lifecycle)) => {
                if summary.count_tested != 1 || summary.count_success != 1 {
                    return Err(format!(
                        "viewport smoke failed: tested={}, success={}",
                        summary.count_tested, summary.count_success
                    )
                    .into());
                }
                if !lifecycle.saw_secondary_viewport
                    || lifecycle.require_secondary_while_held
                        && (!lifecycle.held_probe_complete || !lifecycle.saw_secondary_while_held)
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
            None => return Err("viewport smoke mode is unavailable".into()),
        }
        self.complete = true;
        Ok(())
    }

    fn complete(&self) -> bool {
        self.complete
    }

    fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let Some(engine) = self.engine.as_mut() else {
            return Ok(());
        };
        engine.shutdown()?;
        self.engine = None;
        Ok(())
    }

    fn take_output(&mut self) -> Option<Self::Output> {
        self.completed_result()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let scenario = ViewportSmokeScenario::from_environment()?;
    let result = run(scenario)?.ok_or("viewport runtime completed without a result")?;
    result.write_after_teardown()
}

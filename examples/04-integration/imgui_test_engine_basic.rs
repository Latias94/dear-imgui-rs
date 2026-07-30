use std::cell::RefCell;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use dear_app::{
    AppConfig, Application, FrameContext, InitContext, RunError, ShutdownContext, Theme,
    WgpuPreset, run,
};
use dear_imgui_test_engine::{
    AttachmentState, HeadlessRunnerError, ResultSummary, RunFlags, RunOutcome, RunReport, RunSpeed,
    RunnerControl, ScriptCount, TestEngine, TestGroup, TestRunner, VerboseLevel, raw,
};

#[derive(Debug, Default)]
struct ScriptTargetState {
    checkbox: bool,
    slider: i32,
    input: String,
    my_int: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupSel {
    Tests,
    Perfs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scenario {
    Pass,
    Failure,
    NoMatch,
    Timeout,
    Abort,
    FfiFailure,
    CallbackError,
}

impl Scenario {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "pass" => Ok(Self::Pass),
            "failure" => Ok(Self::Failure),
            "no-match" => Ok(Self::NoMatch),
            "timeout" => Ok(Self::Timeout),
            "abort" => Ok(Self::Abort),
            "ffi-failure" => Ok(Self::FfiFailure),
            "callback-error" => Ok(Self::CallbackError),
            _ => Err(format!(
                "Unknown scenario '{value}' (expected: pass|failure|no-match|timeout|abort|ffi-failure|callback-error)"
            )),
        }
    }
}

#[derive(Debug)]
struct Cli {
    scenario: Option<Scenario>,
    dear_app_smoke: bool,
    json_output: Option<PathBuf>,
    max_frames: Option<NonZeroU64>,
    filter: Option<String>,
    group: GroupSel,
    speed: Option<RunSpeed>,
    verbose: Option<VerboseLevel>,
}

impl Default for Cli {
    fn default() -> Self {
        Self {
            scenario: None,
            dear_app_smoke: false,
            json_output: None,
            max_frames: None,
            filter: None,
            group: GroupSel::Tests,
            speed: None,
            verbose: None,
        }
    }
}

fn parse_cli() -> Result<Cli, String> {
    fn take_value(
        args: &mut std::iter::Peekable<std::env::Args>,
        flag: &str,
    ) -> Result<String, String> {
        args.next();
        args.next()
            .ok_or_else(|| format!("Expected value after {flag}"))
    }

    fn parse_group(s: &str) -> Result<GroupSel, String> {
        match s {
            "tests" => Ok(GroupSel::Tests),
            "perfs" => Ok(GroupSel::Perfs),
            _ => Err(format!("Unknown group '{s}' (expected: tests|perfs)")),
        }
    }

    fn parse_speed(s: &str) -> Result<RunSpeed, String> {
        match s {
            "fast" => Ok(RunSpeed::Fast),
            "normal" => Ok(RunSpeed::Normal),
            "cinematic" => Ok(RunSpeed::Cinematic),
            _ => Err(format!(
                "Unknown speed '{s}' (expected: fast|normal|cinematic)"
            )),
        }
    }

    fn parse_verbose(s: &str) -> Result<VerboseLevel, String> {
        match s {
            "silent" => Ok(VerboseLevel::Silent),
            "error" => Ok(VerboseLevel::Error),
            "warning" => Ok(VerboseLevel::Warning),
            "info" => Ok(VerboseLevel::Info),
            "debug" => Ok(VerboseLevel::Debug),
            "trace" => Ok(VerboseLevel::Trace),
            _ => Err(format!(
                "Unknown verbose level '{s}' (expected: silent|error|warning|info|debug|trace)"
            )),
        }
    }

    let mut cli = Cli {
        group: GroupSel::Tests,
        ..Default::default()
    };

    let mut args = std::env::args().peekable();
    let _exe = args.next();
    while let Some(arg) = args.peek().cloned() {
        match arg.as_str() {
            "-h" | "--help" => {
                return Err(
                    "Usage: imgui_test_engine_basic [options]\n\n\
Options:\n\
  --scenario <NAME>      Run headlessly: pass|failure|no-match|timeout|abort|ffi-failure|callback-error.\n\
  --dear-app-smoke       Run one bounded graphical test through dear_app::run.\n\
  --json-output <PATH>   Atomically write the machine-readable automated result.\n\
  --max-frames <N>       Set the non-zero primary frame budget for automated runs.\n\
  --run                  Alias for --scenario pass.\n\
  --exit-when-done       Alias for --scenario pass.\n\
  --group <tests|perfs>\n\
  --filter <SUBSTR>      Filter string passed to the test engine.\n\
  --speed <fast|normal|cinematic>\n\
  --verbose <silent|error|warning|info|debug|trace>\n"
                        .to_string(),
                );
            }
            "--run" => {
                args.next();
                cli.scenario.get_or_insert(Scenario::Pass);
            }
            "--exit-when-done" => {
                args.next();
                cli.scenario.get_or_insert(Scenario::Pass);
            }
            "--dear-app-smoke" => {
                args.next();
                cli.dear_app_smoke = true;
            }
            "--scenario" => {
                let value = take_value(&mut args, "--scenario")?;
                cli.scenario = Some(Scenario::parse(&value)?);
            }
            "--json-output" => {
                cli.json_output = Some(PathBuf::from(take_value(&mut args, "--json-output")?));
            }
            "--max-frames" => {
                let v = take_value(&mut args, "--max-frames")?;
                let parsed = v
                    .parse::<u64>()
                    .map_err(|_| format!("Invalid --max-frames '{v}'"))?;
                cli.max_frames = Some(
                    NonZeroU64::new(parsed)
                        .ok_or_else(|| "--max-frames must be greater than zero".to_owned())?,
                );
            }
            "--group" => {
                let v = take_value(&mut args, "--group")?;
                cli.group = parse_group(&v)?;
            }
            "--filter" => {
                let v = take_value(&mut args, "--filter")?;
                cli.filter = Some(v);
            }
            "--speed" => {
                let v = take_value(&mut args, "--speed")?;
                cli.speed = Some(parse_speed(&v)?);
            }
            "--verbose" => {
                let v = take_value(&mut args, "--verbose")?;
                cli.verbose = Some(parse_verbose(&v)?);
            }
            _ => return Err(format!("Unknown argument '{arg}' (use --help)")),
        }
    }

    if cli.dear_app_smoke && cli.scenario.is_some() {
        return Err("--dear-app-smoke cannot be combined with a headless scenario".to_owned());
    }

    Ok(cli)
}

#[derive(Debug)]
struct AutomatedCallbackError(&'static str);

impl fmt::Display for AutomatedCallbackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for AutomatedCallbackError {}

enum AutomatedResult {
    Report(RunReport),
    Infrastructure(String),
}

impl AutomatedResult {
    fn exit_code(&self) -> i32 {
        match self {
            Self::Report(report) if report.outcome().is_passed() => 0,
            Self::Report(_) => 2,
            Self::Infrastructure(_) => 3,
        }
    }

    fn to_json(&self) -> String {
        match self {
            Self::Report(report) => {
                let summary = report.summary();
                format!(
                    "{{\"schema_version\":1,\"outcome\":\"{}\",\"infrastructure\":false,\"tested\":{},\"success\":{},\"in_queue\":{},\"frames\":{},\"cleanup_frames\":{},\"error\":null}}",
                    outcome_name(report.outcome()),
                    summary.count_tested,
                    summary.count_success,
                    summary.count_in_queue,
                    report.frames(),
                    report.cleanup_frames(),
                )
            }
            Self::Infrastructure(error) => format!(
                "{{\"schema_version\":1,\"outcome\":\"InfrastructureError\",\"infrastructure\":true,\"tested\":0,\"success\":0,\"in_queue\":0,\"frames\":0,\"cleanup_frames\":0,\"error\":\"{}\"}}",
                json_escape(error),
            ),
        }
    }
}

fn outcome_name(outcome: RunOutcome) -> &'static str {
    match outcome {
        RunOutcome::Passed => "Passed",
        RunOutcome::Failed => "Failed",
        RunOutcome::NoMatch => "NoMatch",
        RunOutcome::TimedOut => "TimedOut",
        RunOutcome::Aborted => "Aborted",
        _ => "Unknown",
    }
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

fn write_json_atomic(path: &Path, contents: &str) -> Result<(), Box<dyn std::error::Error>> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    if let Some(parent) = parent {
        fs::create_dir_all(parent)?;
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("--json-output must name a file")?;
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

fn register_automated_scenario(
    engine: &mut TestEngine,
    scenario: Scenario,
) -> Result<&'static str, Box<dyn std::error::Error>> {
    let filter = match scenario {
        Scenario::Pass | Scenario::NoMatch | Scenario::FfiFailure => {
            engine.add_script_test("runtime", "pass", |script| {
                script.yield_frames(ScriptCount::new(2)?)
            })?;
            if scenario == Scenario::NoMatch {
                "does-not-match-any-test"
            } else {
                "pass"
            }
        }
        Scenario::Failure => {
            engine.add_script_test("runtime", "failure", |script| {
                script.set_ref("Failure Host")?;
                script.table_set_column_enabled_by_label("Missing Table", "Column", true)
            })?;
            "failure"
        }
        Scenario::Timeout | Scenario::Abort | Scenario::CallbackError => {
            engine.add_script_test("runtime", "long-running", |script| {
                script.yield_frames(ScriptCount::new(10_000)?)
            })?;
            "long-running"
        }
    };
    Ok(filter)
}

fn run_automated(cli: &Cli, scenario: Scenario) -> AutomatedResult {
    match try_run_automated(cli, scenario) {
        Ok(report) => AutomatedResult::Report(report),
        Err(error) => AutomatedResult::Infrastructure(error),
    }
}

fn try_run_automated(cli: &Cli, scenario: Scenario) -> Result<RunReport, String> {
    let mut context = dear_imgui_rs::Context::create();
    if !context.font_atlas().build() {
        return Err("failed to build the default font atlas".to_owned());
    }
    context.io_mut().set_display_size([1280.0, 720.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);

    let mut engine = TestEngine::create().map_err(|error| error.to_string())?;
    engine
        .start(&mut context)
        .map_err(|error| error.to_string())?;
    engine
        .set_verbose_level(cli.verbose.unwrap_or(VerboseLevel::Info))
        .map_err(|error| error.to_string())?;
    engine
        .set_run_speed(cli.speed.unwrap_or(RunSpeed::Fast))
        .map_err(|error| error.to_string())?;
    let default_filter =
        register_automated_scenario(&mut engine, scenario).map_err(|error| error.to_string())?;

    if scenario == Scenario::FfiFailure {
        let status = unsafe {
            raw::imgui_test_engine_test_set_exception_injection(
                raw::ImGuiTestEngineExceptionPoint_UpstreamCall,
            )
        };
        if status != raw::ImGuiTestEngineStatus_Success {
            return Err(format!(
                "failed to arm FFI exception injection: status {status}"
            ));
        }
    }

    let default_budget = if scenario == Scenario::Timeout {
        NonZeroU64::new(2).expect("two is non-zero")
    } else {
        NonZeroU64::new(512).expect("512 is non-zero")
    };
    let group = match cli.group {
        GroupSel::Tests => TestGroup::Tests,
        GroupSel::Perfs => TestGroup::Perfs,
    };
    let filter = cli.filter.as_deref().unwrap_or(default_filter);
    let runner = TestRunner::new(&mut engine)
        .group(group)
        .filter(filter)
        .run_flags(RunFlags::RUN_FROM_COMMAND_LINE)
        .frame_budget(cli.max_frames.unwrap_or(default_budget));

    let result: Result<RunReport, HeadlessRunnerError<AutomatedCallbackError>> = runner
        .run_headless(&mut context, |ui, frame| {
            if scenario == Scenario::CallbackError && frame == 1 {
                return Err(AutomatedCallbackError(
                    "injected application callback failure",
                ));
            }
            if scenario == Scenario::Failure {
                ui.window("Failure Host")
                    .build(|| ui.text("No table is intentionally created"));
            }
            Ok(if scenario == Scenario::Abort && frame == 1 {
                RunnerControl::Abort
            } else {
                RunnerControl::Continue
            })
        });
    let result = result.map_err(|error| error.to_string());
    let shutdown = engine.shutdown().map_err(|error| error.to_string());
    drop(context);
    match (result, shutdown) {
        (Ok(report), Ok(())) => Ok(report),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(format!("Test Engine shutdown failed: {error}")),
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct DearAppSmokeState {
    engine_started: bool,
    test_registered: bool,
    test_queued: bool,
    admitted_frames: u64,
    test_engine_calls: u64,
    terminal_summary: Option<ResultSummary>,
    exit_requested: bool,
    budget_exhausted: bool,
    application_shutdown: bool,
    engine_shutdown: bool,
}

struct DearAppSmokeResult {
    state: DearAppSmokeState,
    frame_budget: u64,
    runtime_teardown_complete: bool,
    error: Option<String>,
}

impl DearAppSmokeResult {
    fn contract_violation(&self) -> Option<String> {
        let state = self.state;
        let summary = state.terminal_summary.unwrap_or_default();
        if !state.engine_started {
            return Some("Test Engine did not start".to_owned());
        }
        if !state.test_registered || !state.test_queued {
            return Some("deterministic Test Engine test was not registered and queued".to_owned());
        }
        if state.admitted_frames == 0 || state.admitted_frames > self.frame_budget {
            return Some(format!(
                "admitted frame count {} exceeded budget {}",
                state.admitted_frames, self.frame_budget
            ));
        }
        if state.test_engine_calls != state.admitted_frames {
            return Some(format!(
                "Application::test_engine was called {} times for {} admitted frames",
                state.test_engine_calls, state.admitted_frames
            ));
        }
        if state.budget_exhausted {
            return Some(
                "Test Engine did not reach terminal state within the frame budget".to_owned(),
            );
        }
        if state.terminal_summary.is_none() {
            return Some("Test Engine terminal state was not observed".to_owned());
        }
        if summary.count_tested != 1 || summary.count_success != 1 || summary.count_in_queue != 0 {
            return Some(format!(
                "unexpected terminal summary: tested={}, success={}, in_queue={}",
                summary.count_tested, summary.count_success, summary.count_in_queue
            ));
        }
        if !state.exit_requested {
            return Some("terminal Test Engine state did not request runtime exit".to_owned());
        }
        if !state.application_shutdown || !state.engine_shutdown {
            return Some("application or Test Engine shutdown did not complete".to_owned());
        }
        if !self.runtime_teardown_complete {
            return Some("dear_app::run did not return after runtime teardown".to_owned());
        }
        None
    }

    fn is_passed(&self) -> bool {
        self.error.is_none() && self.contract_violation().is_none()
    }

    fn exit_code(&self) -> i32 {
        if self.is_passed() { 0 } else { 2 }
    }

    fn to_json(&self) -> String {
        let state = self.state;
        let summary = state.terminal_summary.unwrap_or_default();
        let outcome = if self.is_passed() { "Passed" } else { "Failed" };
        let error = self
            .error
            .clone()
            .or_else(|| self.contract_violation())
            .map_or_else(
                || "null".to_owned(),
                |error| format!("\"{}\"", json_escape(&error)),
            );
        format!(
            "{{\"schema_version\":1,\"mode\":\"DearAppGraphical\",\"outcome\":\"{outcome}\",\"engine_started\":{},\"test_registered\":{},\"test_queued\":{},\"admitted_frames\":{},\"frame_budget\":{},\"test_engine_calls\":{},\"terminal_observed\":{},\"tested\":{},\"success\":{},\"in_queue\":{},\"exit_requested\":{},\"budget_exhausted\":{},\"application_shutdown\":{},\"engine_shutdown\":{},\"runtime_teardown_complete\":{},\"error\":{error}}}",
            state.engine_started,
            state.test_registered,
            state.test_queued,
            state.admitted_frames,
            self.frame_budget,
            state.test_engine_calls,
            state.terminal_summary.is_some(),
            summary.count_tested,
            summary.count_success,
            summary.count_in_queue,
            state.exit_requested,
            state.budget_exhausted,
            state.application_shutdown,
            state.engine_shutdown,
            self.runtime_teardown_complete,
        )
    }
}

struct DearAppSmoke {
    engine: Option<TestEngine>,
    state: Rc<RefCell<DearAppSmokeState>>,
    frame_budget: NonZeroU64,
    speed: RunSpeed,
    verbose: VerboseLevel,
}

impl Application for DearAppSmoke {
    fn configure_imgui(&mut self, context: &mut InitContext<'_>) -> Result<(), RunError> {
        let mut engine = TestEngine::create()
            .map_err(|error| RunError::application("configure_imgui", error.to_string()))?;
        engine
            .start(context.imgui())
            .map_err(|error| RunError::application("configure_imgui", error.to_string()))?;
        self.state.borrow_mut().engine_started = true;
        engine
            .set_verbose_level(self.verbose)
            .map_err(|error| RunError::application("configure_imgui", error.to_string()))?;
        engine
            .set_run_speed(self.speed)
            .map_err(|error| RunError::application("configure_imgui", error.to_string()))?;
        engine
            .add_script_test("dear_app", "graphical_presentation", |script| {
                script.yield_frames(ScriptCount::new(2)?)
            })
            .map_err(|error| RunError::application("configure_imgui", error.to_string()))?;
        self.state.borrow_mut().test_registered = true;
        engine
            .queue_tests(TestGroup::Tests, None, RunFlags::RUN_FROM_COMMAND_LINE)
            .map_err(|error| RunError::application("configure_imgui", error.to_string()))?;
        self.state.borrow_mut().test_queued = true;
        self.engine = Some(engine);
        Ok(())
    }

    fn frame(&mut self, context: &mut FrameContext<'_>) -> Result<(), RunError> {
        let admitted_frames = {
            let mut state = self.state.borrow_mut();
            state.admitted_frames = state.admitted_frames.checked_add(1).ok_or_else(|| {
                RunError::application("frame", "admitted frame counter exhausted")
            })?;
            state.admitted_frames
        };
        let terminal_summary = self
            .engine
            .as_mut()
            .ok_or_else(|| RunError::application("frame", "Test Engine is unavailable"))?
            .take_terminal_summary()
            .map_err(|error| RunError::application("frame", error.to_string()))?;

        let should_exit = if let Some(summary) = terminal_summary {
            let mut state = self.state.borrow_mut();
            state.terminal_summary = Some(summary);
            state.exit_requested = true;
            true
        } else if admitted_frames >= self.frame_budget.get() {
            let mut state = self.state.borrow_mut();
            state.budget_exhausted = true;
            state.exit_requested = true;
            true
        } else {
            false
        };
        if should_exit {
            context.request_exit();
        }

        let ui = context.ui();
        ui.window("Dear App Test Engine Smoke").build(|| {
            ui.text("One deterministic test is running.");
            ui.text(format!(
                "Admitted frame {admitted_frames} / {}",
                self.frame_budget.get()
            ));
        });
        Ok(())
    }

    fn test_engine(&mut self) -> Option<&mut TestEngine> {
        let mut state = self.state.borrow_mut();
        state.test_engine_calls = state.test_engine_calls.saturating_add(1);
        drop(state);
        self.engine.as_mut()
    }

    fn shutdown(&mut self, _context: &mut ShutdownContext<'_>) -> Result<(), RunError> {
        self.state.borrow_mut().application_shutdown = true;
        let Some(engine) = self.engine.as_mut() else {
            return Ok(());
        };
        engine
            .shutdown()
            .map_err(|error| RunError::application("shutdown", error.to_string()))?;
        let destroyed = matches!(engine.attachment_state(), AttachmentState::Destroyed);
        self.state.borrow_mut().engine_shutdown = destroyed;
        if !destroyed {
            return Err(RunError::application(
                "shutdown",
                "Test Engine did not reach the destroyed attachment state",
            ));
        }
        self.engine = None;
        Ok(())
    }
}

fn run_dear_app_smoke(cli: &Cli) -> DearAppSmokeResult {
    let frame_budget = cli
        .max_frames
        .unwrap_or_else(|| NonZeroU64::new(256).expect("256 is non-zero"));
    let state = Rc::new(RefCell::new(DearAppSmokeState::default()));
    let application = DearAppSmoke {
        engine: None,
        state: Rc::clone(&state),
        frame_budget,
        speed: cli.speed.unwrap_or(RunSpeed::Fast),
        verbose: cli.verbose.unwrap_or(VerboseLevel::Info),
    };
    let config = AppConfig {
        window_title: "dear-app Test Engine graphical smoke".to_owned(),
        window_size: (960.0, 540.0),
        wgpu: dear_app::WgpuConfig::from_preset(WgpuPreset::SoftwareFallback),
        restore_previous_geometry: false,
        theme: Some(Theme::Dark),
        ..Default::default()
    };
    let runtime_error = run(config, application)
        .err()
        .map(|error| error.to_string());
    let mut result = DearAppSmokeResult {
        state: *state.borrow(),
        frame_budget: frame_budget.get(),
        runtime_teardown_complete: true,
        error: runtime_error,
    };
    if result.error.is_none() {
        result.error = result.contract_violation();
    }
    result
}

struct TestEngineApp {
    cli: Cli,
    engine: Option<TestEngine>,
    script_target_state: ScriptTargetState,
}

impl Application for TestEngineApp {
    fn configure_imgui(&mut self, context: &mut InitContext<'_>) -> Result<(), RunError> {
        let mut engine = TestEngine::create()
            .map_err(|error| RunError::application("configure_imgui", error.to_string()))?;
        engine
            .start(context.imgui())
            .map_err(|error| RunError::application("configure_imgui", error.to_string()))?;
        engine
            .set_verbose_level(self.cli.verbose.unwrap_or(VerboseLevel::Info))
            .map_err(|error| RunError::application("configure_imgui", error.to_string()))?;
        engine
            .set_run_speed(self.cli.speed.unwrap_or(RunSpeed::Normal))
            .map_err(|error| RunError::application("configure_imgui", error.to_string()))?;
        engine
            .register_default_tests()
            .map_err(|error| RunError::application("configure_imgui", error.to_string()))?;
        engine
            .add_script_test("rust_tests", "script_smoke", |test| {
                test.set_ref("Script Target###RustScriptTarget")?;
                test.wait_for_item("Click Me", ScriptCount::new(120)?)?;
                test.assert_item_visible("Click Me")?;
                test.item_click("Click Me")?;
                test.wait_for_item_visible("Input", ScriptCount::new(120)?)?;
                test.input_text_replace("Input", "hello from script", false)?;
                test.wait_for_item_visible("MyInt", ScriptCount::new(120)?)?;
                test.item_input_int("MyInt", 123)?;
                test.assert_item_read_int_eq("MyInt", 123)?;
                test.item_check("Node/Checkbox")?;
                test.item_uncheck("Node/Checkbox")?;
                test.yield_frames(ScriptCount::new(2)?)
            })
            .map_err(|error| RunError::application("configure_imgui", error.to_string()))?;

        engine
            .install_default_crash_handler()
            .map_err(|error| RunError::application("configure_imgui", error.to_string()))?;
        self.engine = Some(engine);
        Ok(())
    }

    fn frame(&mut self, context: &mut FrameContext<'_>) -> Result<(), RunError> {
        let ui = context.ui();
        let Some(engine) = self.engine.as_mut() else {
            return Ok(());
        };
        let summary = engine
            .result_summary()
            .map_err(|error| RunError::application("frame", error.to_string()))?;
        let running = engine
            .is_running_tests()
            .map_err(|error| RunError::application("frame", error.to_string()))?;
        let requesting_max_speed = engine
            .is_requesting_max_app_speed()
            .map_err(|error| RunError::application("frame", error.to_string()))?;
        let state = &mut self.script_target_state;
        ui.window("Script Target###RustScriptTarget")
            .size([420.0, 160.0], dear_imgui_rs::Condition::FirstUseEver)
            .build(|| {
                ui.text("This window is owned by the application.");
                ui.button("Click Me");
                ui.input_text("Input", &mut state.input).build();
                ui.input_int("MyInt", &mut state.my_int);
                if let Some(_node) = ui.tree_node("Node") {
                    ui.checkbox("Checkbox", &mut state.checkbox);
                }
                ui.slider_i32("Slider", &mut state.slider, 0, 1000);
            });

        let mut command_error = None;
        ui.window("ImGui Test Engine")
            .size([420.0, 220.0], dear_imgui_rs::Condition::FirstUseEver)
            .build(|| {
                if ui.button("Queue all Tests") {
                    command_error = engine
                        .queue_tests(TestGroup::Tests, None, RunFlags::RUN_FROM_COMMAND_LINE)
                        .err();
                }
                ui.same_line();
                if ui.button("Queue all Perfs") {
                    command_error = engine
                        .queue_tests(TestGroup::Perfs, None, RunFlags::RUN_FROM_COMMAND_LINE)
                        .err();
                }
                ui.same_line();
                if ui.button("Abort") {
                    command_error = engine.abort_current_test().err();
                }

                ui.separator();
                ui.text(format!(
                    "Tested: {}  Success: {}  In queue: {}",
                    summary.count_tested, summary.count_success, summary.count_in_queue
                ));
                ui.text(format!("Running tests: {running}"));
                ui.text(format!("Request max app speed: {requesting_max_speed}"));
            });
        if let Some(error) = command_error {
            return Err(RunError::application("frame", error.to_string()));
        }
        engine
            .show_windows(ui, None)
            .map_err(|error| RunError::application("frame", error.to_string()))?;
        let _ = engine
            .take_terminal_summary()
            .map_err(|error| RunError::application("frame", error.to_string()))?;
        Ok(())
    }

    fn test_engine(&mut self) -> Option<&mut TestEngine> {
        self.engine.as_mut()
    }

    fn shutdown(&mut self, _context: &mut ShutdownContext<'_>) -> Result<(), RunError> {
        if let Some(engine) = self.engine.as_mut() {
            engine
                .shutdown()
                .map_err(|error| RunError::application("shutdown", error.to_string()))?;
        }
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let cli = match parse_cli() {
        Ok(cli) => cli,
        Err(message) if message.starts_with("Usage:") => {
            eprintln!("{message}");
            return Ok(());
        }
        Err(message) => return Err(message.into()),
    };
    if cli.dear_app_smoke {
        let result = run_dear_app_smoke(&cli);
        let json = result.to_json();
        println!("{json}");
        if let Some(path) = cli.json_output.as_deref()
            && let Err(error) = write_json_atomic(path, &json)
        {
            eprintln!("failed to write dear-app smoke JSON result: {error}");
            std::process::exit(3);
        }
        let exit_code = result.exit_code();
        if exit_code != 0 {
            std::process::exit(exit_code);
        }
        return Ok(());
    }
    if let Some(scenario) = cli.scenario {
        let result = run_automated(&cli, scenario);
        let json = result.to_json();
        println!("{json}");
        if let Some(path) = cli.json_output.as_deref()
            && let Err(error) = write_json_atomic(path, &json)
        {
            eprintln!("failed to write automated JSON result: {error}");
            std::process::exit(3);
        }
        let exit_code = result.exit_code();
        if exit_code != 0 {
            std::process::exit(exit_code);
        }
        return Ok(());
    }
    let application = TestEngineApp {
        cli,
        engine: None,
        script_target_state: ScriptTargetState {
            checkbox: false,
            slider: 42,
            input: String::new(),
            my_int: 42,
        },
    };
    let config = AppConfig {
        theme: Some(Theme::Dark),
        ..Default::default()
    };
    run(config, application)?;
    Ok(())
}

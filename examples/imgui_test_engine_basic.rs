use dear_app::{
    AppConfig, Application, FrameContext, InitContext, RunError, ShutdownContext, Theme, run,
};
use dear_imgui_test_engine::{
    RunFlags, RunSpeed, ScriptCount, TestEngine, TestGroup, VerboseLevel,
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
    All,
}

#[derive(Debug)]
struct Cli {
    run: bool,
    exit_when_done: bool,
    max_frames: Option<u64>,
    filter: Option<String>,
    group: GroupSel,
    speed: Option<RunSpeed>,
    verbose: Option<VerboseLevel>,
}

impl Default for Cli {
    fn default() -> Self {
        Self {
            run: false,
            exit_when_done: false,
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
            "all" => Ok(GroupSel::All),
            _ => Err(format!("Unknown group '{s}' (expected: tests|perfs|all)")),
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
  --run                 Queue tests automatically at startup.\n\
  --exit-when-done       Exit the process when the queue is drained (implies --run).\n\
  --max-frames <N>       Fail if the queue does not drain within N frames (implies --exit-when-done).\n\
  --group <tests|perfs|all>\n\
  --filter <SUBSTR>      Filter string passed to the test engine.\n\
  --speed <fast|normal|cinematic>\n\
  --verbose <silent|error|warning|info|debug|trace>\n"
                        .to_string(),
                );
            }
            "--run" => {
                args.next();
                cli.run = true;
            }
            "--exit-when-done" => {
                args.next();
                cli.run = true;
                cli.exit_when_done = true;
            }
            "--max-frames" => {
                let v = take_value(&mut args, "--max-frames")?;
                cli.run = true;
                cli.exit_when_done = true;
                cli.max_frames = Some(
                    v.parse::<u64>()
                        .map_err(|_| format!("Invalid --max-frames '{v}'"))?,
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

    Ok(cli)
}

struct TestEngineApp {
    cli: Cli,
    engine: Option<TestEngine>,
    script_target_state: ScriptTargetState,
    auto_run_started: bool,
    frame_counter: u64,
}

impl Application for TestEngineApp {
    fn configure_imgui(&mut self, context: &mut InitContext<'_>) -> Result<(), RunError> {
        let mut engine = TestEngine::create();
        engine.set_verbose_level(self.cli.verbose.unwrap_or(VerboseLevel::Info));
        engine.set_run_speed(self.cli.speed.unwrap_or(RunSpeed::Normal));
        engine.register_default_tests();
        engine
            .add_script_test("rust_tests", "script_smoke", |test| {
                test.set_ref("Script Target###RustScriptTarget")?;
                test.wait_for_item("Click Me", ScriptCount::new(120))?;
                test.assert_item_visible("Click Me")?;
                test.item_click("Click Me")?;
                test.wait_for_item_visible("Input", ScriptCount::new(120))?;
                test.input_text_replace("Input", "hello from script", false)?;
                test.wait_for_item_visible("MyInt", ScriptCount::new(120))?;
                test.item_input_int("MyInt", 123)?;
                test.assert_item_read_int_eq("MyInt", 123)?;
                test.item_check("Node/Checkbox")?;
                test.item_uncheck("Node/Checkbox")?;
                test.yield_frames(ScriptCount::new(2));
                Ok(())
            })
            .map_err(|error| RunError::application("configure_imgui", error.to_string()))?;
        engine
            .try_start(context.imgui())
            .map_err(|error| RunError::application("configure_imgui", error.to_string()))?;

        if self.cli.run {
            let filter = self.cli.filter.as_deref();
            let flags = RunFlags::RUN_FROM_COMMAND_LINE;
            match self.cli.group {
                GroupSel::Tests => {
                    let _ = engine.queue_tests(TestGroup::Tests, filter, flags);
                }
                GroupSel::Perfs => {
                    let _ = engine.queue_tests(TestGroup::Perfs, filter, flags);
                }
                GroupSel::All => {
                    let _ = engine.queue_tests(TestGroup::Tests, filter, flags);
                    let _ = engine.queue_tests(TestGroup::Perfs, filter, flags);
                }
            }
            self.auto_run_started = true;
        }
        TestEngine::install_default_crash_handler();
        self.engine = Some(engine);
        Ok(())
    }

    fn frame(&mut self, context: &mut FrameContext<'_, '_>) -> Result<(), RunError> {
        let ui = context.ui();
        let Some(engine) = self.engine.as_mut() else {
            return Ok(());
        };
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

        ui.window("ImGui Test Engine")
            .size([420.0, 220.0], dear_imgui_rs::Condition::FirstUseEver)
            .build(|| {
                if ui.button("Queue all Tests") {
                    let _ =
                        engine.queue_tests(TestGroup::Tests, None, RunFlags::RUN_FROM_COMMAND_LINE);
                }
                ui.same_line();
                if ui.button("Queue all Perfs") {
                    let _ =
                        engine.queue_tests(TestGroup::Perfs, None, RunFlags::RUN_FROM_COMMAND_LINE);
                }
                ui.same_line();
                if ui.button("Abort") {
                    engine.abort_current_test();
                }

                let summary = engine.result_summary();
                ui.separator();
                ui.text(format!(
                    "Tested: {}  Success: {}  In queue: {}",
                    summary.count_tested, summary.count_success, summary.count_in_queue
                ));
                ui.text(format!("Running tests: {}", engine.is_running_tests()));
                ui.text(format!(
                    "Request max app speed: {}",
                    engine.is_requesting_max_app_speed()
                ));
            });
        engine.show_windows(ui, None);

        if self.cli.exit_when_done && self.auto_run_started {
            self.frame_counter += 1;
            let done = engine.is_test_queue_empty() && !engine.is_running_tests();
            let timed_out = self
                .cli
                .max_frames
                .is_some_and(|max| self.frame_counter >= max);
            if done || timed_out {
                engine.stop();
                let summary = engine.result_summary();
                let failures = summary.count_tested.saturating_sub(summary.count_success);
                if timed_out {
                    return Err(RunError::application(
                        "frame",
                        format!("test queue timed out after {} frames", self.frame_counter),
                    ));
                }
                if failures != 0 {
                    return Err(RunError::application(
                        "frame",
                        format!("{failures} test-engine tests failed"),
                    ));
                }
                println!(
                    "Tests passed (tested={}, success={})",
                    summary.count_tested, summary.count_success
                );
                context.request_exit();
            }
        }
        Ok(())
    }

    fn shutdown(&mut self, _context: &mut ShutdownContext<'_>) -> Result<(), RunError> {
        if let Some(engine) = self.engine.as_mut() {
            engine.shutdown();
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
    let application = TestEngineApp {
        cli,
        engine: None,
        script_target_state: ScriptTargetState {
            checkbox: false,
            slider: 42,
            input: String::new(),
            my_int: 42,
        },
        auto_run_started: false,
        frame_counter: 0,
    };
    let config = AppConfig {
        theme: Some(Theme::Dark),
        ..Default::default()
    };
    run(config, application)?;
    Ok(())
}

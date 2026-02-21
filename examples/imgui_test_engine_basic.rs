use dear_app::{AppBuilder, Theme};
use dear_imgui_test_engine::{RunFlags, RunSpeed, TestEngine, TestGroup, VerboseLevel};
use std::{cell::RefCell, rc::Rc};

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let cli = match parse_cli() {
        Ok(cli) => cli,
        Err(msg) => {
            if msg.starts_with("Usage:") {
                eprintln!("{msg}");
                return Ok(());
            }
            return Err(msg.into());
        }
    };

    let cli = Rc::new(cli);
    let engine = Rc::new(RefCell::new(None::<TestEngine>));
    let script_target_state = Rc::new(RefCell::new(ScriptTargetState {
        checkbox: false,
        slider: 42,
        input: String::new(),
        my_int: 42,
    }));

    let engine_setup = Rc::clone(&engine);
    let engine_frame = Rc::clone(&engine);
    let engine_exit = Rc::clone(&engine);
    let script_target_state_frame = Rc::clone(&script_target_state);
    let cli_setup = Rc::clone(&cli);
    let cli_frame = Rc::clone(&cli);
    let auto_run_started = Rc::new(RefCell::new(false));
    let auto_run_started_setup = Rc::clone(&auto_run_started);
    let auto_run_started_frame = Rc::clone(&auto_run_started);
    let frame_counter = Rc::new(RefCell::new(0u64));
    let frame_counter_frame = Rc::clone(&frame_counter);

    AppBuilder::new()
        .with_theme(Theme::Dark)
        .on_setup(move |ctx| {
            let mut test_engine = TestEngine::create();
            test_engine.set_verbose_level(cli_setup.verbose.unwrap_or(VerboseLevel::Info));
            test_engine.set_run_speed(cli_setup.speed.unwrap_or(RunSpeed::Normal));
            test_engine.register_default_tests();

            // Minimal Rust-authored test (scripted): keeps everything on the C++ side at runtime,
            // while letting you define the intent in Rust without FFI callbacks.
            test_engine
                .add_script_test("rust_tests", "script_smoke", |t| {
                    t.set_ref("Script Target###RustScriptTarget")?;
                    t.wait_for_item("Click Me", 120)?;
                    t.assert_item_visible("Click Me")?;
                    t.item_click("Click Me")?;
                    t.wait_for_item_visible("Input", 120)?;
                    t.input_text_replace("Input", "hello from script", false)?;
                    t.wait_for_item_visible("MyInt", 120)?;
                    t.item_input_int("MyInt", 123)?;
                    t.assert_item_read_int_eq("MyInt", 123)?;
                    t.item_check("Node/Checkbox")?;
                    t.item_uncheck("Node/Checkbox")?;
                    t.yield_frames(2);
                    Ok(())
                })
                .expect("Failed to register script_smoke test");

            test_engine
                .try_start(ctx)
                .expect("Failed to start Dear ImGui Test Engine");

            if cli_setup.run {
                let filter = cli_setup.filter.as_deref();
                let flags = RunFlags::RUN_FROM_COMMAND_LINE;
                match cli_setup.group {
                    GroupSel::Tests => {
                        let _ = test_engine.queue_tests(TestGroup::Tests, filter, flags);
                    }
                    GroupSel::Perfs => {
                        let _ = test_engine.queue_tests(TestGroup::Perfs, filter, flags);
                    }
                    GroupSel::All => {
                        let _ = test_engine.queue_tests(TestGroup::Tests, filter, flags);
                        let _ = test_engine.queue_tests(TestGroup::Perfs, filter, flags);
                    }
                }
                *auto_run_started_setup.borrow_mut() = true;
            }

            *engine_setup.borrow_mut() = Some(test_engine);
            TestEngine::install_default_crash_handler();
        })
        .on_frame(move |ui, _addons| {
            let mut guard = engine_frame.borrow_mut();
            let Some(engine) = guard.as_mut() else {
                return;
            };

            // A small window rendered by the application every frame, meant to be driven by script tests.
            let mut state = script_target_state_frame.borrow_mut();
            ui.window("Script Target###RustScriptTarget")
                .size([420.0, 160.0], dear_imgui_rs::Condition::FirstUseEver)
                .build(|| {
                    ui.text("This window is owned by the app (not a test GuiFunc).");
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
                        let _ = engine.queue_tests(
                            TestGroup::Tests,
                            None,
                            RunFlags::RUN_FROM_COMMAND_LINE,
                        );
                    }
                    ui.same_line();
                    if ui.button("Queue all Perfs") {
                        let _ = engine.queue_tests(
                            TestGroup::Perfs,
                            None,
                            RunFlags::RUN_FROM_COMMAND_LINE,
                        );
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
                    ui.text("Note: call post_swap() in custom loops when capture is needed.");
                });

            engine.show_windows(ui, None);

            if cli_frame.exit_when_done && *auto_run_started_frame.borrow() {
                *frame_counter_frame.borrow_mut() += 1;

                let done = engine.is_test_queue_empty() && !engine.is_running_tests();
                let too_many_frames = cli_frame
                    .max_frames
                    .is_some_and(|max| *frame_counter_frame.borrow() >= max);

                if done || too_many_frames {
                    // Try to shut down cleanly before exiting the process.
                    engine.stop();
                    let summary = engine.result_summary();
                    let failures = summary.count_tested - summary.count_success;
                    engine.shutdown();

                    if too_many_frames {
                        eprintln!(
                            "Timed out after {} frames (tested={}, success={}, in_queue={})",
                            frame_counter_frame.borrow(),
                            summary.count_tested,
                            summary.count_success,
                            summary.count_in_queue
                        );
                        std::process::exit(2);
                    }

                    if failures != 0 {
                        eprintln!(
                            "Tests failed (tested={}, success={}, in_queue={})",
                            summary.count_tested, summary.count_success, summary.count_in_queue
                        );
                        std::process::exit(1);
                    }

                    println!(
                        "Tests passed (tested={}, success={})",
                        summary.count_tested, summary.count_success
                    );
                    std::process::exit(0);
                }
            }
        })
        .on_exit(move |_ctx| {
            if let Some(engine) = engine_exit.borrow_mut().as_mut() {
                engine.shutdown();
            }
        })
        .run()?;

    Ok(())
}

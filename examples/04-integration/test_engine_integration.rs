//! Interactive Dear ImGui Test Engine integration.
//!
//! This example keeps the application-facing path small: start one engine, register tests,
//! expose it through `Application::test_engine`, and render the Test Engine windows. Automated
//! scenarios and machine-readable evidence live in the private `test_engine_runtime` CI binary.

use dear_app::{
    AppConfig, Application, ApplicationStage, FrameContext, InitContext, RunError, ShutdownContext,
    Theme, run,
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

struct TestEngineApp {
    engine: Option<TestEngine>,
    script_target_state: ScriptTargetState,
}

impl Application for TestEngineApp {
    fn configure_imgui(&mut self, context: &mut InitContext<'_>) -> Result<(), RunError> {
        let stage = ApplicationStage::ConfigureImgui;
        let mut engine =
            TestEngine::create().map_err(|error| RunError::application(stage, error))?;
        engine
            .start(context.imgui())
            .map_err(|error| RunError::application(stage, error))?;
        engine
            .set_verbose_level(VerboseLevel::Info)
            .map_err(|error| RunError::application(stage, error))?;
        engine
            .set_run_speed(RunSpeed::Normal)
            .map_err(|error| RunError::application(stage, error))?;
        engine
            .register_default_tests()
            .map_err(|error| RunError::application(stage, error))?;
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
            .map_err(|error| RunError::application(stage, error))?;
        engine
            .install_default_crash_handler()
            .map_err(|error| RunError::application(stage, error))?;
        self.engine = Some(engine);
        Ok(())
    }

    fn frame(&mut self, context: &mut FrameContext<'_>) -> Result<(), RunError> {
        let Some(engine) = self.engine.as_mut() else {
            return Ok(());
        };
        let stage = ApplicationStage::Frame;
        let summary = engine
            .result_summary()
            .map_err(|error| RunError::application(stage, error))?;
        let running = engine
            .is_running_tests()
            .map_err(|error| RunError::application(stage, error))?;
        let requesting_max_speed = engine
            .is_requesting_max_app_speed()
            .map_err(|error| RunError::application(stage, error))?;
        let state = &mut self.script_target_state;
        let ui = context.ui();

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
            return Err(RunError::application(stage, error));
        }
        engine
            .show_windows(ui, None)
            .map_err(|error| RunError::application(stage, error))?;
        let _ = engine
            .take_terminal_summary()
            .map_err(|error| RunError::application(stage, error))?;
        Ok(())
    }

    fn test_engine(&mut self) -> Option<&mut TestEngine> {
        self.engine.as_mut()
    }

    fn shutdown(&mut self, _context: &mut ShutdownContext<'_>) -> Result<(), RunError> {
        if let Some(engine) = self.engine.as_mut() {
            engine
                .shutdown()
                .map_err(|error| RunError::application(ApplicationStage::Shutdown, error))?;
        }
        self.engine = None;
        Ok(())
    }
}

fn main() -> Result<(), RunError> {
    env_logger::init();
    let application = TestEngineApp {
        engine: None,
        script_target_state: ScriptTargetState {
            checkbox: false,
            slider: 42,
            input: String::new(),
            my_int: 42,
        },
    };
    let config = AppConfig {
        window_title: "Dear ImGui Test Engine".to_owned(),
        theme: Some(Theme::Dark),
        ..Default::default()
    };
    run(config, application)
}

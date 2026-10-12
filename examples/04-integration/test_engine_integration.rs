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
    ResultSummary, RunFlags, RunSpeed, RunState, ScriptCount, TestEngine, TestGroup, VerboseLevel,
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
    result_summary: ResultSummary,
    gui_needs_layout: bool,
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
        self.engine = Some(engine);
        Ok(())
    }

    fn frame(&mut self, context: &mut FrameContext<'_>) -> Result<(), RunError> {
        let Some(engine) = self.engine.as_mut() else {
            return Ok(());
        };
        let stage = ApplicationStage::Frame;
        let running = engine
            .is_running_tests()
            .map_err(|error| RunError::application(stage, error))?;
        let queue_empty = engine
            .is_test_queue_empty()
            .map_err(|error| RunError::application(stage, error))?;
        if let Some(summary) = engine
            .take_terminal_summary()
            .map_err(|error| RunError::application(stage, error))?
        {
            self.result_summary = summary;
        } else if matches!(engine.run_state(), RunState::Queued | RunState::Running) {
            self.result_summary = engine
                .result_summary()
                .map_err(|error| RunError::application(stage, error))?;
        }
        let summary = self.result_summary;
        let accepts_queue = engine.run_state().accepts_queue() && queue_empty && !running;
        let accepts_abort = matches!(engine.run_state(), RunState::Queued | RunState::Running);
        let requesting_max_speed = engine
            .is_requesting_max_app_speed()
            .map_err(|error| RunError::application(stage, error))?;
        let state = &mut self.script_target_state;
        let ui = context.ui();
        let viewport = ui.main_viewport();
        let work_pos = viewport.work_pos();
        let work_size = viewport.work_size();
        let em = ui.current_font_size();
        let margin = [
            em.min((work_size[0] - 1.0).max(0.0) * 0.5),
            em.min((work_size[1] - 1.0).max(0.0) * 0.5),
        ];
        let origin = [work_pos[0] + margin[0], work_pos[1] + margin[1]];
        let available = [
            (work_size[0] - margin[0] * 2.0).max(1.0),
            (work_size[1] - margin[1] * 2.0).max(1.0),
        ];
        let gap = em.min(available[1] * 0.1);
        let side_by_side = available[0] >= 32.0 * em + gap + 28.0 * em;
        let panel_width = if side_by_side {
            32.0 * em
        } else {
            available[0]
        };
        // Wide clients keep the application controls beside the native GUI. Narrow clients
        // share the height between all three windows so every pane has scrollable content.
        let stacked_height = (available[1] - gap * 2.0).max(3.0);
        let controls_height = if side_by_side {
            available[1].min(18.0 * em)
        } else {
            (stacked_height * 0.25).min(18.0 * em)
        };
        let target_height = if side_by_side {
            (available[1] - controls_height - gap).clamp(1.0, 17.0 * em)
        } else {
            (stacked_height * 0.25).min(17.0 * em)
        };
        let target_pos = [origin[0], origin[1] + controls_height + gap];
        let gui_pos = if side_by_side {
            [origin[0] + panel_width + gap, origin[1]]
        } else {
            [origin[0], target_pos[1] + target_height + gap]
        };
        let gui_size = [
            (work_pos[0] + work_size[0] - margin[0] - gui_pos[0]).max(1.0),
            (work_pos[1] + work_size[1] - margin[1] - gui_pos[1]).max(1.0),
        ];

        ui.window("Script Target###RustScriptTarget")
            .size(
                [panel_width, target_height],
                dear_imgui_rs::Condition::FirstUseEver,
            )
            .position(target_pos, dear_imgui_rs::Condition::FirstUseEver)
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
            .size(
                [panel_width, controls_height],
                dear_imgui_rs::Condition::FirstUseEver,
            )
            .position(origin, dear_imgui_rs::Condition::FirstUseEver)
            .build(|| {
                {
                    let _disabled = ui.begin_disabled_with_cond(!accepts_queue);
                    if ui.button("Queue all Tests") {
                        command_error = engine
                            .queue_tests(TestGroup::Tests, None, RunFlags::NONE)
                            .err();
                    }
                    ui.same_line();
                    if ui.button("Queue all Perfs") {
                        command_error = engine
                            .queue_tests(TestGroup::Perfs, None, RunFlags::NONE)
                            .err();
                    }
                }
                ui.same_line();
                {
                    let _disabled = ui.begin_disabled_with_cond(!accepts_abort);
                    if ui.button("Abort") {
                        command_error = engine.abort_current_test().err();
                    }
                }

                ui.separator();
                ui.text("Results for tests queued from this panel:");
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
        if self.gui_needs_layout {
            // The upstream GUI supplies its own first-use size. Place it after creation;
            // the next frame uses this geometry, and later user movement stays untouched.
            ui.set_window_pos_by_name("Dear ImGui Test Engine", gui_pos);
            ui.set_window_size_by_name("Dear ImGui Test Engine", gui_size);
            self.gui_needs_layout = false;
        }
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
    dear_imgui_examples::init_tracing_with_filter(
        "dear_app=info,dear_imgui_test_engine=info,wgpu=warn",
    );
    let application = TestEngineApp {
        engine: None,
        result_summary: ResultSummary::default(),
        gui_needs_layout: true,
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

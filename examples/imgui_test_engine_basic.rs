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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

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

    AppBuilder::new()
        .with_theme(Theme::Dark)
        .on_setup(move |ctx| {
            let mut test_engine = TestEngine::create();
            test_engine.set_verbose_level(VerboseLevel::Info);
            test_engine.set_run_speed(RunSpeed::Normal);
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
        })
        .on_exit(move |_ctx| {
            if let Some(engine) = engine_exit.borrow_mut().as_mut() {
                engine.shutdown();
            }
        })
        .run()?;

    Ok(())
}

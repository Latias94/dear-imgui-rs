use dear_app::{AppBuilder, Theme};
use dear_imgui_test_engine::{RunFlags, RunSpeed, TestEngine, TestGroup, VerboseLevel};
use std::{cell::RefCell, rc::Rc};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let engine = Rc::new(RefCell::new(None::<TestEngine>));

    let engine_setup = Rc::clone(&engine);
    let engine_frame = Rc::clone(&engine);
    let engine_exit = Rc::clone(&engine);

    AppBuilder::new()
        .with_theme(Theme::Dark)
        .on_setup(move |ctx| {
            let mut test_engine = TestEngine::create();
            test_engine.set_verbose_level(VerboseLevel::Info);
            test_engine.set_run_speed(RunSpeed::Normal);
            test_engine.start(ctx);
            *engine_setup.borrow_mut() = Some(test_engine);
            TestEngine::install_default_crash_handler();
        })
        .on_frame(move |ui, _addons| {
            let mut guard = engine_frame.borrow_mut();
            let Some(engine) = guard.as_mut() else {
                return;
            };

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
            if let Some(mut engine) = engine_exit.borrow_mut().take() {
                engine.stop();
            }
        })
        .run()?;

    Ok(())
}

#![cfg(feature = "stack-layout")]

use dear_imgui_rs as imgui;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;

static CONTEXT: Mutex<()> = Mutex::new(());

fn prepared_context() -> imgui::Context {
    let mut context = imgui::Context::create();
    context.io_mut().set_display_size([320.0, 240.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("legacy renderer font atlas should be available")
        .build();
    context
}

#[test]
fn context_drop_releases_native_stack_layout_state() {
    let _guard = CONTEXT.lock().unwrap();
    let baseline = unsafe { imgui::sys::ImGuiStack_StateCount() };
    let mut context = prepared_context();

    let ui = context.frame();
    let _ = ui.window("stack-layout context state").build(|| {
        let layout = ui.begin_horizontal("row", [0.0, 0.0], -1.0);
        ui.text("item");
        layout.end();
    });
    let _ = context.render_legacy();

    assert_eq!(unsafe { imgui::sys::ImGuiStack_StateCount() }, baseline + 1);

    drop(context);

    assert_eq!(unsafe { imgui::sys::ImGuiStack_StateCount() }, baseline);
}

#[test]
fn spring_requires_an_unsuspended_layout_in_the_current_window() {
    let _guard = CONTEXT.lock().unwrap();
    let mut context = prepared_context();
    let frame = context.begin_frame();
    let ui = frame.ui();
    let spring = || ui.spring(1.0, -1.0);
    ui.window("spring state").build(|| {
        assert!(catch_unwind(AssertUnwindSafe(spring)).is_err());
        let layout = ui.begin_horizontal("row", [0.0, 0.0], -1.0);
        {
            let _suspended = ui.suspend_stack_layout();
            assert!(catch_unwind(AssertUnwindSafe(spring)).is_err());
            // A new layout inside a suspension can use springs of its own.
            ui.vertical("nested", [0.0, 0.0], -1.0, spring);
        }
        spring();
        layout.end();
    });
    drop(frame.render_legacy());
}

#[test]
fn stack_layout_ids_are_unique_per_window_and_frame() {
    let _guard = CONTEXT.lock().unwrap();
    let mut context = prepared_context();
    let raw = imgui::Id::from(42u32);

    let frame = context.begin_frame();
    let ui = frame.ui();
    ui.window("duplicate layout IDs").build(|| {
        let outer = ui.begin_horizontal(raw, [0.0, 0.0], -1.0);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _duplicate = ui.begin_vertical(raw, [0.0, 0.0], -1.0);
            }))
            .is_err()
        );
        outer.end();

        ui.vertical("col", [0.0, 0.0], -1.0, || {});
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _duplicate = ui.begin_vertical("col", [0.0, 0.0], -1.0);
            }))
            .is_err()
        );
    });
    ui.window("another window").build(|| {
        ui.horizontal(raw, [0.0, 0.0], -1.0, || {});
    });
    drop(frame.render_legacy());

    let frame = context.begin_frame();
    let ui = frame.ui();
    ui.window("duplicate layout IDs").build(|| {
        ui.vertical("col", [0.0, 0.0], -1.0, || {});
    });
    drop(frame.render_legacy());
}

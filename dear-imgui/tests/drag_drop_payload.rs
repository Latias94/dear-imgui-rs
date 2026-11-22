use dear_imgui_rs as imgui;
use std::mem::size_of_val;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn setup_context() -> imgui::Context {
    let mut ctx = imgui::Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    ctx.font_atlas()
        .try_claim_legacy_renderer()
        .expect("legacy renderer font atlas should be available")
        .build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
    ctx
}

#[test]
fn drag_drop_payload_query_no_panic() {
    let _guard = test_guard();
    let mut ctx = setup_context();
    let ui = ctx.frame();

    // No drag operation in this headless test, but the query should be safe.
    assert!(ui.drag_drop_payload().is_none());

    struct PayloadName<'a>(&'a std::cell::Cell<usize>);
    impl AsRef<str> for PayloadName<'_> {
        fn as_ref(&self) -> &str {
            self.0.set(self.0.get() + 1);
            "APP_ITEM"
        }
    }

    let conversions = std::cell::Cell::new(0);
    let source = ui
        .drag_drop_source_config(PayloadName(&conversions))
        .flags(imgui::DragDropSourceFlags::EXTERN | imgui::DragDropSourceFlags::NO_PREVIEW_TOOLTIP)
        .begin_payload(42_u32)
        .expect("external source");
    assert_eq!(conversions.get(), 1);
    assert!(ui.drag_drop_payload().is_some());
    assert!(
        ui.drag_drop_source_config("nested")
            .flags(imgui::DragDropSourceFlags::EXTERN)
            .begin()
            .is_none()
    );
    source.end();
}

#[test]
fn typed_and_native_targets_can_use_the_same_application_name() {
    let _guard = test_guard();
    let mut ctx = setup_context();
    let name = "DND_DEMO_CELL";
    let source_flags =
        imgui::DragDropSourceFlags::EXTERN | imgui::DragDropSourceFlags::NO_PREVIEW_TOOLTIP;
    let peek = imgui::DragDropTargetFlags::PEEK_ONLY;
    let mut hover = [0.0, 0.0];

    for frame in 0..3 {
        ctx.io_mut().add_mouse_pos_event(hover);
        let ui = ctx.frame();
        ui.window("payload routes")
            .position([20.0, 20.0], imgui::Condition::Always)
            .size([220.0, 140.0], imgui::Condition::Always)
            .build(|| {
                let source = |name| ui.drag_drop_source_config(name).flags(source_flags);
                match frame {
                    0 => source(name).begin_payload(17_u32).unwrap().end(),
                    // Once retains both the name and value originally copied by ImGui.
                    1 => source("next")
                        .condition(imgui::DragDropPayloadCond::Once)
                        .begin_payload(23_u32)
                        .unwrap()
                        .end(),
                    _ => {
                        let native_cell = 2_i32;
                        unsafe {
                            source(name).begin_payload_unchecked(
                                (&raw const native_cell).cast(),
                                size_of_val(&native_cell),
                            )
                        }
                        .unwrap()
                        .end();
                    }
                }

                let [x, y] = ui.cursor_screen_pos();
                hover = [x + 4.0, y + 4.0];
                ui.button_with_size("target", [120.0, 50.0]);
                if frame == 0 {
                    return;
                }
                let target = ui.drag_drop_target().expect("hovered target");
                let typed = target.accept_payload::<u32, _>(name, peek);
                let native = unsafe { target.accept_payload_unchecked(name, peek) };
                if frame == 1 {
                    assert_eq!(typed.unwrap().unwrap().data, 17);
                    assert!(native.is_none());
                    assert!(target.accept_payload::<u32, _>("next", peek).is_none());
                    assert!(
                        target
                            .accept_payload::<i32, _>(name, peek)
                            .unwrap()
                            .is_err()
                    );
                } else {
                    assert!(typed.is_none());
                    assert_eq!(native.unwrap().size, size_of::<i32>());
                }
            })
            .expect("visible window");
        let _ = ctx.render_legacy();
    }
}

use dear_imgui_rs as imgui;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn prepare_context(ctx: &mut imgui::Context) {
    let io = ctx.io_mut();
    io.set_display_size([800.0, 600.0]);
    io.set_delta_time(1.0 / 60.0);

    let _ = ctx.font_atlas_mut().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
}

#[test]
fn window_flags_reject_internal_bits_before_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let dock_node_host =
        imgui::WindowFlags::from_bits_retain(imgui::sys::ImGuiWindowFlags_DockNodeHost);
    let child_window =
        imgui::WindowFlags::from_bits_retain(imgui::sys::ImGuiWindowFlags_ChildWindow);

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = ui
                .window("dock node host")
                .flags(dock_node_host)
                .build(|| {});
        }))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = ui
                .window("child window bit")
                .flags(child_window)
                .build(|| {});
        }))
        .is_err()
    );

    let _ = ui
        .window("public flags")
        .flags(imgui::WindowFlags::NO_RESIZE | imgui::WindowFlags::NO_DOCKING)
        .build(|| {});
}

#[test]
fn popup_window_flags_reject_internal_bits_before_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let popup_bit = imgui::WindowFlags::from_bits_retain(imgui::sys::ImGuiWindowFlags_Popup);
    let modal_bit = imgui::WindowFlags::from_bits_retain(imgui::sys::ImGuiWindowFlags_Modal);

    let _ = ui.window("Popup window flags").build(|| {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.begin_popup_with_flags("popup bit", popup_bit);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui
                    .begin_modal_popup_config("modal bit")
                    .flags(modal_bit)
                    .begin();
            }))
            .is_err()
        );

        let _ = ui.begin_popup_with_flags("public popup flags", imgui::WindowFlags::NO_MOVE);
    });
}

#[test]
fn child_windows_reject_invalid_flags_before_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let unsupported_child_bits =
        imgui::ChildFlags::from_bits_retain(imgui::sys::ImGuiWindowFlags_NoMouseInputs as u32);

    let _ = ui.window("Child flags").build(|| {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui
                    .child_window("unsupported child bits")
                    .child_flags(unsupported_child_bits)
                    .build(&ui, || {});
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui
                    .child_window("window always auto resize")
                    .flags(imgui::WindowFlags::ALWAYS_AUTO_RESIZE)
                    .build(&ui, || {});
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui
                    .child_window("always auto resize alone")
                    .child_flags(imgui::ChildFlags::ALWAYS_AUTO_RESIZE)
                    .build(&ui, || {});
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui
                    .child_window("always plus resize")
                    .child_flags(
                        imgui::ChildFlags::ALWAYS_AUTO_RESIZE
                            | imgui::ChildFlags::AUTO_RESIZE_X
                            | imgui::ChildFlags::RESIZE_X,
                    )
                    .build(&ui, || {});
            }))
            .is_err()
        );

        let _ = ui
            .child_window("typed always auto resize")
            .child_flags(imgui::ChildFlags::ALWAYS_AUTO_RESIZE | imgui::ChildFlags::AUTO_RESIZE_X)
            .build(&ui, || {});
    });
}

#[test]
fn window_and_child_geometry_reject_non_finite_values_before_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = ui
                .window("nan size")
                .size([f32::NAN, 1.0], imgui::Condition::Always)
                .build(|| {});
        }))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = ui
                .window("infinite position")
                .position([1.0, f32::INFINITY], imgui::Condition::Always)
                .build(|| {});
        }))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = ui.window("nan alpha").bg_alpha(f32::NAN).build(|| {});
        }))
        .is_err()
    );

    let _ = ui.window("Child geometry").build(|| {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui
                    .child_window("infinite child size")
                    .size([f32::INFINITY, 0.0])
                    .build(&ui, || {});
            }))
            .is_err()
        );

        let _ = ui
            .child_window("finite child size")
            .size([100.0, 32.0])
            .build(&ui, || {});
    });
}

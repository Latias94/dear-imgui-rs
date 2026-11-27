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

    ctx.font_atlas()
        .try_claim_legacy_renderer()
        .expect("legacy renderer font atlas should be available")
        .build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
}

#[test]
fn no_inputs_excludes_mouse_navigation_and_navigation_focus() {
    let flags = imgui::WindowFlags::NO_INPUTS;
    assert_eq!(flags.bits(), imgui::sys::ImGuiWindowFlags_NoInputs);
    assert_eq!(
        flags,
        imgui::WindowFlags::NO_MOUSE_INPUTS
            | imgui::WindowFlags::NO_NAV_INPUTS
            | imgui::WindowFlags::NO_NAV_FOCUS
    );

    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let ui = ctx.frame();
    ui.window("non-interactive overlay").flags(flags).build(|| {
        let window = unsafe { imgui::sys::igGetCurrentWindow() };
        assert!(!window.is_null());
        assert_ne!(
            unsafe { (*window).Flags } & imgui::sys::ImGuiWindowFlags_NoNavFocus,
            0
        );
    });
}

#[test]
fn window_body_binds_its_owner_and_restores_the_previous_context() {
    let _guard = test_guard();
    let mut owner = imgui::Context::create();
    let mut previous = imgui::Context::create();
    prepare_context(&mut owner);
    prepare_context(&mut previous);
    let owner_raw = owner.as_raw();
    let previous_raw = previous.as_raw();
    let current = || unsafe { imgui::sys::igGetCurrentContext() };

    let ui = owner.frame();
    let previous_ui = previous.frame();
    assert_eq!(current(), previous_raw);
    let result = ui.window("Owner").build(|| {
        assert_eq!(current(), owner_raw);
        assert_eq!(
            previous_ui.window("Nested context").build(|| {
                assert_eq!(current(), previous_raw);
                previous_ui.text("nested");
                17
            }),
            Some(17)
        );
        assert_eq!(current(), owner_raw);
        ui.text("owner");
        42
    });
    assert_eq!(result, Some(42));
    assert_eq!(current(), previous_raw);

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ui.window("Unwinding owner").build(|| {
            assert_eq!(current(), owner_raw);
            std::panic::panic_any("window body panicked");
        });
    }))
    .expect_err("the window callback should unwind");
    assert_eq!(panic.downcast_ref::<&str>(), Some(&"window body panicked"));
    assert_eq!(current(), previous_raw);
    drop(owner.render_legacy());
    drop(previous.render_legacy());
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
                    .build(|| {});
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui
                    .child_window("window always auto resize")
                    .flags(imgui::WindowFlags::ALWAYS_AUTO_RESIZE)
                    .build(|| {});
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui
                    .child_window("always auto resize alone")
                    .child_flags(imgui::ChildFlags::ALWAYS_AUTO_RESIZE)
                    .build(|| {});
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
                    .build(|| {});
            }))
            .is_err()
        );

        let _ = ui
            .child_window("typed always auto resize")
            .child_flags(imgui::ChildFlags::ALWAYS_AUTO_RESIZE | imgui::ChildFlags::AUTO_RESIZE_X)
            .build(|| {});
    });
}

#[test]
fn invalid_window_build_does_not_stage_next_window_options() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let raw = ctx.as_raw();
    let ui = ctx.frame();

    for title in ["", "invalid alpha"] {
        let original_flags = unsafe { (*raw).NextWindowData.HasFlags };
        let mut window = ui
            .window(title)
            .size([320.0, 240.0], imgui::Condition::Always);
        if !title.is_empty() {
            window = window.bg_alpha(f32::NAN);
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| window.build(|| ())));
        assert!(result.is_err());
        assert_eq!(unsafe { (*raw).NextWindowData.HasFlags }, original_flags);
    }
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
                    .build(|| {});
            }))
            .is_err()
        );

        let _ = ui
            .child_window("finite child size")
            .size([100.0, 32.0])
            .build(|| {});
    });
}

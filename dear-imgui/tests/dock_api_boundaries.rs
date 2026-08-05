use dear_imgui_rs as imgui;
use std::ffi::CString;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn prepare_context(ctx: &mut imgui::Context) {
    let io = ctx.io_mut();
    io.set_display_size([800.0, 600.0]);
    io.set_delta_time(1.0 / 60.0);
    io.set_config_flags(io.config_flags() | imgui::ConfigFlags::DOCKING_ENABLE);

    let _ = ctx.font_atlas().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
}

fn prepare_context_without_docking(ctx: &mut imgui::Context) {
    let io = ctx.io_mut();
    io.set_display_size([800.0, 600.0]);
    io.set_delta_time(1.0 / 60.0);
    io.set_config_flags(io.config_flags() & !imgui::ConfigFlags::DOCKING_ENABLE);

    let _ = ctx.font_atlas().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
}

fn window_dock_id(name: &str) -> imgui::Id {
    let name = CString::new(name).unwrap();
    let window = unsafe { imgui::sys::igFindWindowByName(name.as_ptr()) };
    assert!(!window.is_null(), "test window must exist");
    imgui::Id::from(unsafe { (*window).DockId })
}

#[test]
fn dockspace_rejects_private_flags_and_invalid_id_or_size_before_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let private_dockspace =
        imgui::DockNodeFlags::from_bits_retain(imgui::sys::ImGuiDockNodeFlags_DockSpace);
    let dockspace_id = ui.get_id("Dockspace boundaries");

    let _ = ui.window("Dockspace boundaries").build(|| {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ =
                    ui.dock_space_with_class(dockspace_id, [100.0, 100.0], private_dockspace, None);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.dock_space(0.into(), [100.0, 100.0]);
            }))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.dock_space(dockspace_id, [f32::NAN, 100.0]);
            }))
            .is_err()
        );
        for invalid in [f32::MAX, -f32::MAX, 2_147_483_648.0] {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _ = ui.dock_space(dockspace_id, [invalid, 100.0]);
                }))
                .is_err()
            );
        }

        let _ = ui.dock_space(dockspace_id, [0.0, 0.0]);
    });
}

#[test]
fn dockspace_rejects_host_names_that_would_alias_after_native_truncation() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let dockspace_id = ui.get_id("Long host window dockspace");
    let long_name = "x".repeat(237);
    ui.window(&long_name).build(|| {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.dock_space(dockspace_id, [100.0, 100.0]);
            }))
            .is_err()
        );
    });
    assert!(unsafe { imgui::sys::igDockBuilderGetNode(dockspace_id.raw()).is_null() });
}

#[test]
fn get_id_preserves_interior_nul_bytes_for_distinct_dockspaces() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    ui.window("Interior NUL ID host").build(|| {
        let nul_id = ui.get_id("dock\0left");
        let question_id = ui.get_id("dock?left");
        assert_ne!(nul_id, question_id);
        assert_eq!(ui.dock_space(nul_id, [100.0, 100.0]), nul_id);
        assert_eq!(ui.dock_space(question_id, [100.0, 100.0]), question_id);
    });
}

#[test]
fn dockspace_over_viewport_keeps_zero_id_auto_generation_but_rejects_private_flags() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let private_central =
        imgui::DockNodeFlags::from_bits_retain(imgui::sys::ImGuiDockNodeFlags_CentralNode);

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = ui.dockspace_over_main_viewport_with_flags(0.into(), private_central);
        }))
        .is_err()
    );

    let id = ui.dockspace_over_main_viewport_with_flags(0.into(), imgui::DockNodeFlags::NONE);
    assert_ne!(id.raw(), 0);
}

#[test]
fn automatic_main_dockspace_id_ignores_a_reentered_host_window_id_stack() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let host_name = format!("WindowOverViewport_{:08X}", ui.main_viewport().id().raw());
    ui.window(&host_name).build(|| {
        let _scope = ui.push_id("nested scope");
        let scoped_id = ui.get_id("DockSpace");
        assert_eq!(
            ui.dockspace_over_main_viewport_with_flags(scoped_id, imgui::DockNodeFlags::NONE),
            scoped_id
        );
        let automatic_id =
            ui.dockspace_over_main_viewport_with_flags(0.into(), imgui::DockNodeFlags::NONE);
        assert_ne!(automatic_id, scoped_id);
    });
}

#[test]
fn ordinary_docking_submission_rejects_disabled_docking_before_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context_without_docking(&mut ctx);
    let ui = ctx.frame();
    let root_id = ui.get_id("Disabled docking");
    let options = imgui::DockspaceOptions::new(root_id).unwrap();

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = ui.dockspace_over_main_viewport();
        }))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = ui.dock_space(root_id, [100.0, 100.0]);
        }))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ui.set_next_window_dock_id(root_id);
        }))
        .is_err()
    );
    assert_eq!(
        ui.dockspace_over_main_viewport_with_layout(
            &options,
            &imgui::DockLayout::tabs([] as [&str; 0]),
            imgui::DockLayoutApply::IfMissing,
        ),
        Err(imgui::DockLayoutError::DockingDisabled)
    );

    let _ = ctx.render();
}

#[test]
fn duplicate_dockspace_ids_are_rejected_before_native_submission() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let explicit_id;
    {
        let ui = ctx.frame();
        explicit_id = ui.get_id("Explicit duplicate dockspace");
        assert_eq!(
            ui.dockspace_over_main_viewport_with_flags(explicit_id, imgui::DockNodeFlags::NONE,),
            explicit_id
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.dockspace_over_main_viewport_with_flags(
                    explicit_id,
                    imgui::DockNodeFlags::NONE,
                );
            }))
            .is_err()
        );
    }
    let _ = ctx.render();

    {
        let ui = ctx.frame();
        assert_eq!(
            ui.dockspace_over_main_viewport_with_flags(explicit_id, imgui::DockNodeFlags::NONE,),
            explicit_id
        );
    }
    let _ = ctx.render();

    let generated;
    {
        let ui = ctx.frame();
        generated = ui.dockspace_over_main_viewport();
        assert_ne!(generated.raw(), 0);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.dockspace_over_main_viewport();
            }))
            .is_err()
        );
    }
    let _ = ctx.render();

    {
        let ui = ctx.frame();
        assert_eq!(
            ui.dockspace_over_main_viewport_with_flags(generated, imgui::DockNodeFlags::NONE),
            generated
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui.dockspace_over_main_viewport();
            }))
            .is_err()
        );
    }
    let _ = ctx.render();
}

#[test]
fn keep_alive_only_may_repeat_without_claiming_a_visible_dockspace() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let ui = ctx.frame();
    let root_id = ui.get_id("Repeated keep-alive dockspace");

    assert_eq!(
        ui.dock_space_with_class(
            root_id,
            [100.0, 100.0],
            imgui::DockNodeFlags::KEEP_ALIVE_ONLY,
            None,
        ),
        root_id
    );
    assert_eq!(
        ui.dock_space_with_class(
            root_id,
            [100.0, 100.0],
            imgui::DockNodeFlags::KEEP_ALIVE_ONLY,
            None,
        ),
        root_id
    );

    let _ = ctx.render();
}

#[test]
fn ordinary_main_dockspace_rejects_child_ids_and_late_visible_submission() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let root_id;
    let child_id;
    let layout = imgui::DockLayout::split(
        imgui::DockSplit::Left,
        0.4,
        imgui::DockLayout::tabs(["Ordinary main left"]),
        imgui::DockLayout::tabs(["Ordinary main right"]),
    );
    {
        let ui = ctx.frame();
        root_id = ui.get_id("Ordinary main dockspace contract");
        let options = imgui::DockspaceOptions::new(root_id).unwrap();
        ui.dockspace_over_main_viewport_with_layout(
            &options,
            &layout,
            imgui::DockLayoutApply::Replace,
        )
        .unwrap();
        ui.window("Ordinary main left").build(|| ui.text("left"));
        ui.window("Ordinary main right").build(|| ui.text("right"));
        child_id = window_dock_id("Ordinary main left");
    }
    let _ = ctx.render();

    {
        let ui = ctx.frame();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = ui
                    .dockspace_over_main_viewport_with_flags(child_id, imgui::DockNodeFlags::NONE);
            }))
            .is_err()
        );
        assert_eq!(
            ui.dockspace_over_main_viewport_with_flags(
                root_id,
                imgui::DockNodeFlags::KEEP_ALIVE_ONLY,
            ),
            root_id
        );

        ui.window("Ordinary main left").build(|| ui.text("left"));
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ =
                    ui.dockspace_over_main_viewport_with_flags(root_id, imgui::DockNodeFlags::NONE);
            }))
            .is_err()
        );
        assert_eq!(
            ui.dockspace_over_main_viewport_with_flags(
                root_id,
                imgui::DockNodeFlags::KEEP_ALIVE_ONLY,
            ),
            root_id
        );
        ui.window("Ordinary main right").build(|| ui.text("right"));

        let root = unsafe { imgui::sys::igDockBuilderGetNode(root_id.raw()) };
        assert!(!root.is_null());
        assert!(unsafe { imgui::sys::ImGuiDockNode_IsSplitNode(root) });
        assert_ne!(window_dock_id("Ordinary main left").raw(), 0);
        assert_ne!(window_dock_id("Ordinary main right").raw(), 0);
    }
    let _ = ctx.render();
}

#[test]
fn ordinary_current_window_dockspace_rejects_late_visible_submission() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let root_id;
    let child_id;
    let layout = imgui::DockLayout::split(
        imgui::DockSplit::Left,
        0.4,
        imgui::DockLayout::tabs(["Ordinary current left"]),
        imgui::DockLayout::tabs(["Ordinary current right"]),
    );
    {
        let ui = ctx.frame();
        root_id = ui.get_id("Ordinary current dockspace contract");
        let options = imgui::DockspaceOptions::new(root_id).unwrap();
        ui.window("Ordinary current host")
            .size([500.0, 400.0], imgui::Condition::Always)
            .build(|| {
                ui.dock_space_with_layout(
                    &options,
                    [500.0, 400.0],
                    &layout,
                    imgui::DockLayoutApply::Replace,
                )
                .unwrap();
            });
        ui.window("Ordinary current left").build(|| ui.text("left"));
        ui.window("Ordinary current right")
            .build(|| ui.text("right"));
        child_id = window_dock_id("Ordinary current left");
    }
    let _ = ctx.render();

    {
        let ui = ctx.frame();
        ui.window("Ordinary current keepalive").build(|| {
            assert_eq!(
                ui.dock_space_with_class(
                    root_id,
                    [500.0, 400.0],
                    imgui::DockNodeFlags::KEEP_ALIVE_ONLY,
                    None,
                ),
                root_id
            );
        });
        ui.window("Ordinary current left").build(|| ui.text("left"));
        ui.window("Ordinary current late host")
            .size([500.0, 400.0], imgui::Condition::Always)
            .build(|| {
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _ = ui.dock_space_with_class(
                            child_id,
                            [500.0, 400.0],
                            imgui::DockNodeFlags::NONE,
                            None,
                        );
                    }))
                    .is_err()
                );
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let _ = ui.dock_space_with_class(
                            root_id,
                            [500.0, 400.0],
                            imgui::DockNodeFlags::NONE,
                            None,
                        );
                    }))
                    .is_err()
                );
            });
        ui.window("Ordinary current right")
            .build(|| ui.text("right"));

        let root = unsafe { imgui::sys::igDockBuilderGetNode(root_id.raw()) };
        assert!(!root.is_null());
        assert!(unsafe { imgui::sys::ImGuiDockNode_IsSplitNode(root) });
        assert_ne!(window_dock_id("Ordinary current left").raw(), 0);
        assert_ne!(window_dock_id("Ordinary current right").raw(), 0);
    }
    let _ = ctx.render();
}

#[test]
fn ordinary_late_submission_cannot_recover_a_window_already_undocked_by_imgui() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let root_id;
    let layout = imgui::DockLayout::split(
        imgui::DockSplit::Left,
        0.4,
        imgui::DockLayout::tabs(["Already undocked left"]),
        imgui::DockLayout::tabs(["Still docked right"]),
    );
    {
        let ui = ctx.frame();
        root_id = ui.get_id("Ordinary automatic undock contract");
        let options = imgui::DockspaceOptions::new(root_id).unwrap();
        ui.dockspace_over_main_viewport_with_layout(
            &options,
            &layout,
            imgui::DockLayoutApply::Replace,
        )
        .unwrap();
        ui.window("Already undocked left").build(|| ui.text("left"));
        ui.window("Still docked right").build(|| ui.text("right"));
    }
    let _ = ctx.render();

    {
        let ui = ctx.frame();
        ui.window("Already undocked left").build(|| ui.text("left"));
        assert_eq!(window_dock_id("Already undocked left"), imgui::Id::from(0));

        assert_eq!(
            ui.dockspace_over_main_viewport_with_flags(root_id, imgui::DockNodeFlags::NONE),
            root_id
        );
        ui.window("Still docked right").build(|| ui.text("right"));
        assert_ne!(window_dock_id("Still docked right").raw(), 0);
    }
    let _ = ctx.render();
}

#[test]
fn duplicate_declarative_submission_preserves_the_first_layout() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let ui = ctx.frame();
    let root_id = ui.get_id("Duplicate declarative dockspace");
    let options = imgui::DockspaceOptions::new(root_id)
        .unwrap()
        .flags(imgui::DockNodeFlags::KEEP_ALIVE_ONLY);
    let first = imgui::DockLayout::split(
        imgui::DockSplit::Left,
        0.4,
        imgui::DockLayout::tabs(["Left"]),
        imgui::DockLayout::tabs(["Right"]),
    );

    ui.dockspace_over_main_viewport_with_layout(&options, &first, imgui::DockLayoutApply::Replace)
        .unwrap();
    assert_eq!(
        ui.dockspace_over_main_viewport_with_layout(
            &options,
            &imgui::DockLayout::tabs(["Replacement"]),
            imgui::DockLayoutApply::Replace,
        ),
        Err(imgui::DockLayoutError::DuplicateDockspaceSubmission { root_id })
    );

    let root = unsafe { imgui::sys::igDockBuilderGetNode(root_id.raw()) };
    assert!(!root.is_null());
    assert!(unsafe { imgui::sys::ImGuiDockNode_IsSplitNode(root) });
    let _ = ctx.render();
}

#[test]
fn skipped_main_host_still_allows_only_one_layout_application() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let host_name;
    {
        let ui = ctx.frame();
        host_name = format!("WindowOverViewport_{:08X}", ui.main_viewport().id().raw());
        let _ = ui
            .window(&host_name)
            .collapsed(true, imgui::Condition::Always)
            .build(|| {});
    }
    let _ = ctx.render();

    let ui = ctx.frame();
    assert!(
        ui.window(&host_name)
            .collapsed(true, imgui::Condition::Always)
            .build(|| {})
            .is_none()
    );

    let root_id = ui.get_id("Skipped main host declarative dockspace");
    let options = imgui::DockspaceOptions::new(root_id).unwrap();
    let first = imgui::DockLayout::split(
        imgui::DockSplit::Left,
        0.4,
        imgui::DockLayout::tabs(["Skipped left"]),
        imgui::DockLayout::tabs(["Skipped right"]),
    );
    ui.dockspace_over_main_viewport_with_layout(&options, &first, imgui::DockLayoutApply::Replace)
        .unwrap();
    assert_eq!(
        ui.dockspace_over_main_viewport_with_layout(
            &options,
            &imgui::DockLayout::tabs(["Replacement"]),
            imgui::DockLayoutApply::Replace,
        ),
        Err(imgui::DockLayoutError::DuplicateDockspaceSubmission { root_id })
    );

    let root = unsafe { imgui::sys::igDockBuilderGetNode(root_id.raw()) };
    assert!(!root.is_null());
    assert!(unsafe { imgui::sys::ImGuiDockNode_IsSplitNode(root) });
    let _ = ctx.render();
}

#[test]
fn docking_enable_is_frozen_after_the_first_frame() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context_without_docking(&mut ctx);
    ctx.frame().text("first frame without docking");
    let _ = ctx.render();

    let flags = ctx.io().config_flags() | imgui::ConfigFlags::DOCKING_ENABLE;
    ctx.io_mut().set_config_flags(flags);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = ctx.frame();
        }))
        .is_err()
    );
    assert_eq!(
        ctx.frame_lifecycle_state(),
        imgui::FrameLifecycleState::Rendered
    );
    drop(ctx);

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    ctx.frame().text("first frame with docking");
    let _ = ctx.render();

    let flags = ctx.io().config_flags() & !imgui::ConfigFlags::DOCKING_ENABLE;
    ctx.io_mut().set_config_flags(flags);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = ctx.frame();
        }))
        .is_err()
    );
    assert_eq!(
        ctx.frame_lifecycle_state(),
        imgui::FrameLifecycleState::Rendered
    );
}

#[test]
fn invalid_declarative_layout_returns_before_dockspace_submission() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let ui = ctx.frame();
    let root_id = ui.get_id("Invalid declarative dock layout");
    let options = imgui::DockspaceOptions::new(root_id).unwrap();
    let invalid = imgui::DockLayout::split(
        imgui::DockSplit::Left,
        1.0,
        imgui::DockLayout::tabs(["Left"]),
        imgui::DockLayout::tabs(["Right"]),
    );

    assert!(matches!(
        ui.dockspace_over_main_viewport_with_layout(
            &options,
            &invalid,
            imgui::DockLayoutApply::Replace,
        ),
        Err(imgui::DockLayoutError::InvalidSplitRatio { ratio: 1.0 })
    ));
    assert!(unsafe { imgui::sys::igDockBuilderGetNode(root_id.raw()).is_null() });
}

#[test]
fn invalid_replace_preserves_and_keeps_the_existing_layout_alive() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let root_id;
    {
        let ui = ctx.frame();
        root_id = ui.get_id("Transactional declarative dock layout");
        let options = imgui::DockspaceOptions::new(root_id).unwrap();
        let layout = imgui::DockLayout::split(
            imgui::DockSplit::Left,
            0.4,
            imgui::DockLayout::tabs(["Transactional left"]),
            imgui::DockLayout::tabs(["Transactional right"]),
        );
        ui.dockspace_over_main_viewport_with_layout(
            &options,
            &layout,
            imgui::DockLayoutApply::Replace,
        )
        .unwrap();
        ui.window("Transactional left").build(|| ui.text("left"));
        ui.window("Transactional right").build(|| ui.text("right"));
    }
    let _ = ctx.render();

    {
        let ui = ctx.frame();
        let options = imgui::DockspaceOptions::new(root_id).unwrap();
        ui.dockspace_over_main_viewport_with_layout(
            &options,
            &imgui::DockLayout::tabs(["Unused replacement"]),
            imgui::DockLayoutApply::IfMissing,
        )
        .unwrap();
        ui.window("Transactional left").build(|| ui.text("left"));
        ui.window("Transactional right").build(|| ui.text("right"));
    }
    let _ = ctx.render();

    let mut before = String::new();
    ctx.save_ini_settings(&mut before);

    {
        let ui = ctx.frame();
        let options = imgui::DockspaceOptions::new(root_id).unwrap();
        let invalid = imgui::DockLayout::split(
            imgui::DockSplit::Right,
            1.0,
            imgui::DockLayout::tabs(["Replacement"]),
            imgui::DockLayout::tabs(["Discarded"]),
        );
        assert_eq!(
            ui.dockspace_over_main_viewport_with_layout(
                &options,
                &invalid,
                imgui::DockLayoutApply::Replace,
            ),
            Err(imgui::DockLayoutError::InvalidSplitRatio { ratio: 1.0 })
        );
        ui.window("Transactional left").build(|| ui.text("left"));
        ui.window("Transactional right").build(|| ui.text("right"));
    }
    let _ = ctx.render();

    let root = unsafe { imgui::sys::igDockBuilderGetNode(root_id.raw()) };
    assert!(!root.is_null());
    assert!(unsafe { imgui::sys::ImGuiDockNode_IsSplitNode(root) });
    let mut after = String::new();
    ctx.save_ini_settings(&mut after);
    assert_eq!(after, before);
}

#[test]
fn layout_rejects_a_declared_window_already_submitted_on_a_new_root() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let root_id = ui.get_id("Late new declarative dock layout");
    ui.window("Already submitted replacement")
        .build(|| ui.text("submitted first"));
    let options = imgui::DockspaceOptions::new(root_id).unwrap();
    assert_eq!(
        ui.dockspace_over_main_viewport_with_layout(
            &options,
            &imgui::DockLayout::tabs(["Already submitted replacement"]),
            imgui::DockLayoutApply::Replace,
        ),
        Err(imgui::DockLayoutError::WindowSubmittedBeforeDockspace { root_id })
    );
    assert!(unsafe { imgui::sys::igDockBuilderGetNode(root_id.raw()).is_null() });
    let _ = ctx.render();
}

#[test]
fn every_recoverable_preflight_error_keeps_the_existing_layout_alive() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let root_id;
    let layout = imgui::DockLayout::split(
        imgui::DockSplit::Left,
        0.4,
        imgui::DockLayout::tabs(["Preflight left"]),
        imgui::DockLayout::tabs(["Preflight right"]),
    );
    {
        let ui = ctx.frame();
        root_id = ui.get_id("Preflight-preserved dock layout");
        let options = imgui::DockspaceOptions::new(root_id).unwrap();
        ui.dockspace_over_main_viewport_with_layout(
            &options,
            &layout,
            imgui::DockLayoutApply::Replace,
        )
        .unwrap();
        ui.window("Preflight left").build(|| ui.text("left"));
        ui.window("Preflight right").build(|| ui.text("right"));
    }
    let _ = ctx.render();

    {
        let ui = ctx.frame();
        let options = imgui::DockspaceOptions::new(root_id).unwrap();
        ui.dockspace_over_main_viewport_with_layout(
            &options,
            &layout,
            imgui::DockLayoutApply::IfMissing,
        )
        .unwrap();
        ui.window("Preflight left").build(|| ui.text("left"));
        ui.window("Preflight right").build(|| ui.text("right"));
    }
    let _ = ctx.render();
    let mut before = String::new();
    ctx.save_ini_settings(&mut before);

    {
        let ui = ctx.frame();
        let invalid_class = imgui::WindowClass::new(ui.get_id("Invalid preflight class"))
            .dock_node_flags_override_set(imgui::WindowClassDockNodeFlags::from_bits_retain(
                imgui::sys::ImGuiDockNodeFlags_KeepAliveOnly as i32,
            ));
        let options = imgui::DockspaceOptions::new(root_id)
            .unwrap()
            .window_class(invalid_class);
        assert!(matches!(
            ui.dockspace_over_main_viewport_with_layout(
                &options,
                &layout,
                imgui::DockLayoutApply::Replace,
            ),
            Err(imgui::DockLayoutError::InvalidWindowClass(_))
        ));
        ui.window("Preflight left").build(|| ui.text("left"));
        ui.window("Preflight right").build(|| ui.text("right"));
        assert_ne!(window_dock_id("Preflight left").raw(), 0);
        assert_ne!(window_dock_id("Preflight right").raw(), 0);
    }
    let _ = ctx.render();

    {
        let ui = ctx.frame();
        let options = imgui::DockspaceOptions::new(root_id).unwrap();
        assert_eq!(
            ui.dock_space_with_layout(
                &options,
                [0.0, 100.0],
                &layout,
                imgui::DockLayoutApply::Replace,
            ),
            Err(imgui::DockLayoutError::InvalidHostSize { size: [0.0, 100.0] })
        );
        ui.window("Preflight left").build(|| ui.text("left"));
        ui.window("Preflight right").build(|| ui.text("right"));
        assert_ne!(window_dock_id("Preflight left").raw(), 0);
        assert_ne!(window_dock_id("Preflight right").raw(), 0);
    }
    let _ = ctx.render();

    {
        let ui = ctx.frame();
        let options = imgui::DockspaceOptions::new(root_id).unwrap();
        let long_name = "h".repeat(237);
        ui.window(&long_name)
            .flags(imgui::WindowFlags::NO_SAVED_SETTINGS)
            .build(|| {
                assert_eq!(
                    ui.dock_space_with_layout(
                        &options,
                        [100.0, 100.0],
                        &layout,
                        imgui::DockLayoutApply::Replace,
                    ),
                    Err(imgui::DockLayoutError::HostWindowNameTooLong {
                        bytes: 237,
                        max_bytes: 236,
                    })
                );
            });
        ui.window("Preflight left").build(|| ui.text("left"));
        ui.window("Preflight right").build(|| ui.text("right"));
        assert_ne!(window_dock_id("Preflight left").raw(), 0);
        assert_ne!(window_dock_id("Preflight right").raw(), 0);
    }
    let _ = ctx.render();

    let root = unsafe { imgui::sys::igDockBuilderGetNode(root_id.raw()) };
    assert!(!root.is_null());
    assert!(unsafe { imgui::sys::ImGuiDockNode_IsSplitNode(root) });
    let mut after = String::new();
    ctx.save_ini_settings(&mut after);
    assert_eq!(after, before);
}

#[test]
fn replacement_is_rejected_after_a_hosted_window_was_submitted() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let root_id;
    let layout = imgui::DockLayout::split(
        imgui::DockSplit::Left,
        0.4,
        imgui::DockLayout::tabs(["Ordered left"]),
        imgui::DockLayout::tabs(["Ordered right"]),
    );
    {
        let ui = ctx.frame();
        root_id = ui.get_id("Ordered declarative dock layout");
        let options = imgui::DockspaceOptions::new(root_id).unwrap();
        ui.dockspace_over_main_viewport_with_layout(
            &options,
            &layout,
            imgui::DockLayoutApply::Replace,
        )
        .unwrap();
        ui.window("Ordered left").build(|| ui.text("left"));
        ui.window("Ordered right").build(|| ui.text("right"));
    }
    let _ = ctx.render();

    {
        let ui = ctx.frame();
        let keep_alive = imgui::DockspaceOptions::new(root_id)
            .unwrap()
            .flags(imgui::DockNodeFlags::KEEP_ALIVE_ONLY);
        ui.dockspace_over_main_viewport_with_layout(
            &keep_alive,
            &layout,
            imgui::DockLayoutApply::IfMissing,
        )
        .unwrap();
        ui.window("Ordered left").build(|| ui.text("left"));

        assert_eq!(
            ui.dockspace_over_main_viewport_with_layout(
                &keep_alive,
                &imgui::DockLayout::tabs(["Replacement"]),
                imgui::DockLayoutApply::Replace,
            ),
            Err(imgui::DockLayoutError::WindowSubmittedBeforeDockspace { root_id })
        );
        ui.window("Ordered right").build(|| ui.text("right"));

        let root = unsafe { imgui::sys::igDockBuilderGetNode(root_id.raw()) };
        assert!(!root.is_null());
        assert!(unsafe { imgui::sys::ImGuiDockNode_IsSplitNode(root) });
        assert_ne!(window_dock_id("Ordered left").raw(), 0);
        assert_ne!(window_dock_id("Ordered right").raw(), 0);
    }
    let _ = ctx.render();
}

#[test]
fn declarative_root_id_cannot_alias_an_existing_child_node() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let root_id;
    let child_id;
    let layout = imgui::DockLayout::split(
        imgui::DockSplit::Left,
        0.4,
        imgui::DockLayout::tabs(["Collision left"]),
        imgui::DockLayout::tabs(["Collision right"]),
    );
    {
        let ui = ctx.frame();
        root_id = ui.get_id("Child collision dock layout");
        let options = imgui::DockspaceOptions::new(root_id).unwrap();
        ui.dockspace_over_main_viewport_with_layout(
            &options,
            &layout,
            imgui::DockLayoutApply::Replace,
        )
        .unwrap();
        ui.window("Collision left").build(|| ui.text("left"));
        ui.window("Collision right").build(|| ui.text("right"));
        child_id = window_dock_id("Collision left");
        assert_ne!(child_id.raw(), 0);
        assert_ne!(child_id, root_id);
    }
    let _ = ctx.render();

    {
        let ui = ctx.frame();
        let correct_options = imgui::DockspaceOptions::new(root_id).unwrap();
        ui.dockspace_over_main_viewport_with_layout(
            &correct_options,
            &layout,
            imgui::DockLayoutApply::IfMissing,
        )
        .unwrap();
        ui.window("Collision left").build(|| ui.text("left"));
        ui.window("Collision right").build(|| ui.text("right"));
    }
    let _ = ctx.render();
    let mut before = String::new();
    ctx.save_ini_settings(&mut before);

    {
        let ui = ctx.frame();
        let colliding_options = imgui::DockspaceOptions::new(child_id).unwrap();
        assert_eq!(
            ui.dockspace_over_main_viewport_with_layout(
                &colliding_options,
                &imgui::DockLayout::tabs(["Replacement"]),
                imgui::DockLayoutApply::Replace,
            ),
            Err(imgui::DockLayoutError::ExistingNodeIsNotDockspaceRoot { id: child_id })
        );

        let correct_options = imgui::DockspaceOptions::new(root_id).unwrap();
        ui.dockspace_over_main_viewport_with_layout(
            &correct_options,
            &layout,
            imgui::DockLayoutApply::IfMissing,
        )
        .unwrap();
        ui.window("Collision left").build(|| ui.text("left"));
        ui.window("Collision right").build(|| ui.text("right"));
    }
    let _ = ctx.render();

    let root = unsafe { imgui::sys::igDockBuilderGetNode(root_id.raw()) };
    assert!(!root.is_null());
    assert!(unsafe { imgui::sys::ImGuiDockNode_IsSplitNode(root) });
    let mut after = String::new();
    ctx.save_ini_settings(&mut after);
    assert_eq!(after, before);
}

#[test]
fn nested_replacement_remaps_every_leaf_without_leaking_the_staging_root() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let root_id;
    {
        let ui = ctx.frame();
        root_id = ui.get_id("Nested replacement dock layout");
        let options = imgui::DockspaceOptions::new(root_id).unwrap();
        ui.dockspace_over_main_viewport_with_layout(
            &options,
            &imgui::DockLayout::tabs(["Initial"]),
            imgui::DockLayoutApply::Replace,
        )
        .unwrap();
        ui.window("Initial").build(|| ui.text("initial"));
    }
    let _ = ctx.render();

    let nested = imgui::DockLayout::split(
        imgui::DockSplit::Left,
        0.25,
        imgui::DockLayout::tabs(["Nested left A", "Nested left B"]),
        imgui::DockLayout::split(
            imgui::DockSplit::Down,
            0.35,
            imgui::DockLayout::tabs(["Nested bottom"]),
            imgui::DockLayout::split(
                imgui::DockSplit::Right,
                0.4,
                imgui::DockLayout::tabs(["Nested right"]),
                imgui::DockLayout::tabs(["Nested center"]),
            ),
        ),
    );
    {
        let ui = ctx.frame();
        let options = imgui::DockspaceOptions::new(root_id).unwrap();
        ui.dockspace_over_main_viewport_with_layout(
            &options,
            &nested,
            imgui::DockLayoutApply::Replace,
        )
        .unwrap();
        for name in [
            "Nested left A",
            "Nested left B",
            "Nested bottom",
            "Nested right",
            "Nested center",
        ] {
            ui.window(name).build(|| ui.text(name));
        }

        let left_a = window_dock_id("Nested left A");
        let left_b = window_dock_id("Nested left B");
        let bottom = window_dock_id("Nested bottom");
        let right = window_dock_id("Nested right");
        let center = window_dock_id("Nested center");
        assert_eq!(left_a, left_b);
        assert_ne!(left_a.raw(), 0);
        assert_ne!(bottom.raw(), 0);
        assert_ne!(right.raw(), 0);
        assert_ne!(center.raw(), 0);
        assert_ne!(left_a, bottom);
        assert_ne!(bottom, right);
        assert_ne!(right, center);
    }
    let _ = ctx.render();

    {
        let ui = ctx.frame();
        let options = imgui::DockspaceOptions::new(root_id).unwrap();
        ui.dockspace_over_main_viewport_with_layout(
            &options,
            &nested,
            imgui::DockLayoutApply::IfMissing,
        )
        .unwrap();
        for name in [
            "Nested left A",
            "Nested left B",
            "Nested bottom",
            "Nested right",
            "Nested center",
        ] {
            ui.window(name).build(|| ui.text(name));
        }

        let left = window_dock_id("Nested left A");
        let bottom = window_dock_id("Nested bottom");
        let right = window_dock_id("Nested right");
        let center = window_dock_id("Nested center");
        let left_rect =
            unsafe { imgui::sys::ImGuiDockNode_Rect(imgui::sys::igDockBuilderGetNode(left.raw())) };
        let bottom_rect = unsafe {
            imgui::sys::ImGuiDockNode_Rect(imgui::sys::igDockBuilderGetNode(bottom.raw()))
        };
        let right_rect = unsafe {
            imgui::sys::ImGuiDockNode_Rect(imgui::sys::igDockBuilderGetNode(right.raw()))
        };
        let center_rect = unsafe {
            imgui::sys::ImGuiDockNode_Rect(imgui::sys::igDockBuilderGetNode(center.raw()))
        };
        assert!(
            left_rect.Max.x <= center_rect.Min.x,
            "left [{}, {}] must precede center [{}, {}]",
            left_rect.Min.x,
            left_rect.Max.x,
            center_rect.Min.x,
            center_rect.Max.x,
        );
        assert!(
            bottom_rect.Min.y >= center_rect.Max.y,
            "bottom [{}, {}] must follow center [{}, {}]",
            bottom_rect.Min.y,
            bottom_rect.Max.y,
            center_rect.Min.y,
            center_rect.Max.y,
        );
        assert!(
            right_rect.Min.x >= center_rect.Max.x,
            "right [{}, {}] must follow center [{}, {}]",
            right_rect.Min.x,
            right_rect.Max.x,
            center_rect.Min.x,
            center_rect.Max.x,
        );
    }
    let _ = ctx.render();

    let mut ini = String::new();
    ctx.save_ini_settings(&mut ini);
    let docking = ini.split("[Docking][Data]").nth(1).unwrap();
    let dockspace_roots = docking
        .lines()
        .filter(|line| line.trim_start().starts_with("DockSpace"))
        .count();
    assert_eq!(dockspace_roots, 1, "staging dockspace leaked into INI");
    assert!(docking.contains(&format!("ID=0x{:08X}", root_id.raw())));
}

#[test]
fn declarative_layout_preserves_if_missing_and_rebuilds_on_replace() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let root_id;
    {
        let ui = ctx.frame();
        root_id = ui.get_id("Declarative dock layout lifecycle");
        let options = imgui::DockspaceOptions::new(root_id).unwrap();
        let initial = imgui::DockLayout::split(
            imgui::DockSplit::Left,
            0.35,
            imgui::DockLayout::tabs(["Left"]),
            imgui::DockLayout::tabs(["Right"]),
        );
        ui.dockspace_over_main_viewport_with_layout(
            &options,
            &initial,
            imgui::DockLayoutApply::Replace,
        )
        .unwrap();

        let root = unsafe { imgui::sys::igDockBuilderGetNode(root_id.raw()) };
        assert!(!root.is_null());
        assert!(unsafe { imgui::sys::ImGuiDockNode_IsSplitNode(root) });
        ui.window("Left").build(|| ui.text("left"));
        ui.window("Right").build(|| ui.text("right"));
    }
    let _ = ctx.render();

    {
        let ui = ctx.frame();
        let options = imgui::DockspaceOptions::new(root_id).unwrap();
        ui.dockspace_over_main_viewport_with_layout(
            &options,
            &imgui::DockLayout::tabs(["Replacement"]),
            imgui::DockLayoutApply::IfMissing,
        )
        .unwrap();

        let root = unsafe { imgui::sys::igDockBuilderGetNode(root_id.raw()) };
        assert!(!root.is_null());
        assert!(unsafe { imgui::sys::ImGuiDockNode_IsSplitNode(root) });
        ui.window("Left").build(|| ui.text("left"));
        ui.window("Right").build(|| ui.text("right"));
    }
    let _ = ctx.render();

    {
        let ui = ctx.frame();
        let options = imgui::DockspaceOptions::new(root_id).unwrap();
        ui.dockspace_over_main_viewport_with_layout(
            &options,
            &imgui::DockLayout::tabs(["Replacement"]),
            imgui::DockLayoutApply::Replace,
        )
        .unwrap();

        let root = unsafe { imgui::sys::igDockBuilderGetNode(root_id.raw()) };
        assert!(!root.is_null());
        assert!(unsafe { imgui::sys::ImGuiDockNode_IsLeafNode(root) });
        assert!(!unsafe { imgui::sys::ImGuiDockNode_IsSplitNode(root) });
        ui.window("Replacement").build(|| ui.text("replacement"));
    }
    let _ = ctx.render();
}

#[test]
fn replace_creates_the_submitted_dockspace_geometry() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let viewport = ui.main_viewport();
    let root_id = ui.get_id("Declarative dock layout metadata");
    let class_id = ui.get_id("Declarative dock layout class");
    let options = imgui::DockspaceOptions::new(root_id)
        .unwrap()
        .flags(imgui::DockNodeFlags::NO_RESIZE)
        .window_class(imgui::WindowClass::new(class_id));

    ui.dockspace_over_main_viewport_with_layout(
        &options,
        &imgui::DockLayout::tabs([] as [&str; 0]),
        imgui::DockLayoutApply::Replace,
    )
    .unwrap();

    let root = unsafe { imgui::sys::igDockBuilderGetNode(root_id.raw()) };
    assert!(!root.is_null());
    assert!(unsafe { imgui::sys::ImGuiDockNode_IsDockSpace(root) });
    let rect = unsafe { imgui::sys::ImGuiDockNode_Rect(root) };
    assert!((rect.Min.x - viewport.work_pos()[0]).abs() <= f32::EPSILON);
    assert!((rect.Min.y - viewport.work_pos()[1]).abs() <= f32::EPSILON);
    assert!(
        (rect.Max.x - (viewport.work_pos()[0] + viewport.work_size()[0])).abs() <= f32::EPSILON
    );
    assert!(
        (rect.Max.y - (viewport.work_pos()[1] + viewport.work_size()[1])).abs() <= f32::EPSILON
    );

    let _ = ctx.render();
}

#[test]
fn current_window_layout_uses_the_actual_cursor_position() {
    let _guard = test_guard();
    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let root_id = ui.get_id("Current-window declarative dock layout");
    ui.window("Current-window dock host")
        .position([80.0, 90.0], imgui::Condition::Always)
        .size([400.0, 300.0], imgui::Condition::Always)
        .build(|| {
            let cursor = ui.cursor_screen_pos();
            let options = imgui::DockspaceOptions::new(root_id).unwrap();
            ui.dock_space_with_layout(
                &options,
                [200.0, 150.0],
                &imgui::DockLayout::tabs([] as [&str; 0]),
                imgui::DockLayoutApply::Replace,
            )
            .unwrap();

            let root = unsafe { imgui::sys::igDockBuilderGetNode(root_id.raw()) };
            assert!(!root.is_null());
            let rect = unsafe { imgui::sys::ImGuiDockNode_Rect(root) };
            assert!((rect.Min.x - cursor[0]).abs() <= f32::EPSILON);
            assert!((rect.Min.y - cursor[1]).abs() <= f32::EPSILON);
        });
    let _ = ctx.render();
}

#[test]
fn declarative_layout_binds_its_owner_context_and_restores_the_foreign_context() {
    let _guard = test_guard();
    let mut owner = imgui::Context::create();
    prepare_context(&mut owner);
    let owner_raw = owner.as_raw();
    let foreign_raw = unsafe { imgui::sys::igCreateContext(std::ptr::null_mut()) };
    assert!(!foreign_raw.is_null());
    unsafe {
        imgui::sys::igSetCurrentContext(owner_raw);
    }
    let ui = owner.frame();
    let root_id = ui.get_id("Owner-bound declarative dock layout");
    let options = imgui::DockspaceOptions::new(root_id).unwrap();

    unsafe {
        imgui::sys::igSetCurrentContext(foreign_raw);
    }
    assert_eq!(unsafe { imgui::sys::igGetCurrentContext() }, foreign_raw);

    ui.dockspace_over_main_viewport_with_layout(
        &options,
        &imgui::DockLayout::tabs(["Owner window"]),
        imgui::DockLayoutApply::Replace,
    )
    .unwrap();
    assert_eq!(unsafe { imgui::sys::igGetCurrentContext() }, foreign_raw);

    unsafe {
        imgui::sys::igSetCurrentContext(owner_raw);
        assert!(!imgui::sys::igDockBuilderGetNode(root_id.raw()).is_null());
    }
    let _ = owner.render();
    unsafe {
        imgui::sys::igDestroyContext(foreign_raw);
    }
}

#[test]
fn if_missing_preserves_a_layout_restored_from_ini() {
    let _guard = test_guard();
    let mut ini = String::new();

    {
        let mut ctx = imgui::Context::create();
        prepare_context(&mut ctx);
        let ui = ctx.frame();
        let root_id = ui.get_id("Persisted declarative dock layout");
        let options = imgui::DockspaceOptions::new(root_id).unwrap();
        let layout = imgui::DockLayout::split(
            imgui::DockSplit::Left,
            0.4,
            imgui::DockLayout::tabs(["Persisted left"]),
            imgui::DockLayout::tabs(["Persisted right"]),
        );
        ui.dockspace_over_main_viewport_with_layout(
            &options,
            &layout,
            imgui::DockLayoutApply::Replace,
        )
        .unwrap();
        ui.window("Persisted left").build(|| ui.text("left"));
        ui.window("Persisted right").build(|| ui.text("right"));
        let _ = ctx.render();
        ctx.save_ini_settings(&mut ini);
    }
    assert!(ini.contains("[Docking][Data]"));

    let mut restored = imgui::Context::create();
    prepare_context(&mut restored);
    restored.load_ini_settings(&ini);
    let ui = restored.frame();
    let root_id = ui.get_id("Persisted declarative dock layout");
    let options = imgui::DockspaceOptions::new(root_id).unwrap();
    ui.dockspace_over_main_viewport_with_layout(
        &options,
        &imgui::DockLayout::tabs(["Replacement"]),
        imgui::DockLayoutApply::IfMissing,
    )
    .unwrap();

    let root = unsafe { imgui::sys::igDockBuilderGetNode(root_id.raw()) };
    assert!(!root.is_null());
    assert!(unsafe { imgui::sys::ImGuiDockNode_IsSplitNode(root) });
    let _ = restored.render();
}

#[test]
fn window_class_rejects_invalid_flag_overrides_before_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let ui = ctx.frame();
    let core_owned = imgui::ViewportFlags::IS_PLATFORM_WINDOW
        | imgui::ViewportFlags::IS_PLATFORM_MONITOR
        | imgui::ViewportFlags::OWNED_BY_APP
        | imgui::ViewportFlags::CAN_HOST_OTHER_WINDOWS
        | imgui::ViewportFlags::IS_MINIMIZED
        | imgui::ViewportFlags::IS_FOCUSED;
    let unsupported_class = imgui::WindowClass::new(imgui::Id::from(1u32))
        .viewport_flags_override_set(imgui::WindowClassViewportFlags::from_bits_retain(
            core_owned.bits(),
        ));
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ui.set_next_window_class(&unsupported_class);
        }))
        .is_err()
    );

    let overlapping_class = imgui::WindowClass::new(imgui::Id::from(2u32))
        .viewport_flags_overrides(
            imgui::WindowClassViewportFlags::NO_DECORATION,
            imgui::WindowClassViewportFlags::NO_DECORATION,
        );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ui.set_next_window_class(&overlapping_class);
        }))
        .is_err()
    );

    let private_tab_button =
        imgui::TabItemFlags::from_bits_retain(imgui::sys::ImGuiTabItemFlags_Button as i32);
    let invalid_tab_class = imgui::WindowClass::new(imgui::Id::from(3u32))
        .tab_item_flags_override_set(private_tab_button);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ui.set_next_window_class(&invalid_tab_class);
        }))
        .is_err()
    );

    for (index, bits) in [
        imgui::sys::ImGuiDockNodeFlags_KeepAliveOnly as i32,
        imgui::sys::ImGuiDockNodeFlags_PassthruCentralNode as i32,
        imgui::sys::ImGuiDockNodeFlags_DockSpace as i32,
        imgui::sys::ImGuiDockNodeFlags_CentralNode as i32,
        1 << 30,
    ]
    .into_iter()
    .enumerate()
    {
        let invalid = imgui::WindowClass::new(imgui::Id::from(10 + index as u32))
            .dock_node_flags_override_set(imgui::WindowClassDockNodeFlags::from_bits_retain(bits));
        assert_eq!(
            invalid.validate(),
            Err(imgui::WindowClassError::UnsupportedDockNodeFlags { bits })
        );
    }

    let invalid_layout_class = imgui::WindowClass::new(imgui::Id::from(20u32))
        .dock_node_flags_override_set(imgui::WindowClassDockNodeFlags::from_bits_retain(
            imgui::sys::ImGuiDockNodeFlags_KeepAliveOnly as i32,
        ));
    let invalid_layout_root = ui.get_id("Invalid declarative window class");
    let invalid_options = imgui::DockspaceOptions::new(invalid_layout_root)
        .unwrap()
        .window_class(invalid_layout_class);
    assert_eq!(
        ui.dockspace_over_main_viewport_with_layout(
            &invalid_options,
            &imgui::DockLayout::tabs([] as [&str; 0]),
            imgui::DockLayoutApply::Replace,
        ),
        Err(imgui::DockLayoutError::InvalidWindowClass(
            imgui::WindowClassError::UnsupportedDockNodeFlags {
                bits: imgui::sys::ImGuiDockNodeFlags_KeepAliveOnly as i32,
            },
        ))
    );
    assert!(unsafe { imgui::sys::igDockBuilderGetNode(invalid_layout_root.raw()).is_null() });

    let dockspace_id = ui.get_id("Window class boundaries");
    let valid_tab_options = imgui::TabItemOptions::new()
        .flags(imgui::TabItemFlags::NO_REORDER)
        .placement(imgui::TabItemPlacement::Leading);
    let valid_class = imgui::WindowClass::new(imgui::Id::from(5u32))
        .viewport_flags_overrides(
            imgui::WindowClassViewportFlags::NO_DECORATION,
            imgui::WindowClassViewportFlags::NO_TASK_BAR_ICON,
        )
        .tab_item_flags_override_set(valid_tab_options)
        .dock_node_flags_override_set(
            imgui::WindowClassDockNodeFlags::NO_RESIZE
                | imgui::WindowClassDockNodeFlags::HIDDEN_TAB_BAR
                | imgui::WindowClassDockNodeFlags::NO_CLOSE_BUTTON,
        );
    valid_class.validate().unwrap();
    let _ = ui.window("Window class boundaries").build(|| {
        let _ = ui.dock_space_with_class(
            dockspace_id,
            [100.0, 100.0],
            imgui::DockNodeFlags::NONE,
            Some(&valid_class),
        );
    });
}

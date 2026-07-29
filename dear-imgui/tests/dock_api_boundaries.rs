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

        let _ = ui.dock_space(dockspace_id, [0.0, 0.0]);
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
fn ordinary_docking_submission_rejects_disabled_docking_before_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context_without_docking(&mut ctx);
    let ui = ctx.frame();
    let root_id = ui.get_id("Disabled docking");
    let target = imgui::DockspaceTarget::new(root_id, [0.0, 0.0], [800.0, 600.0]).unwrap();

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
            &target,
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
fn duplicate_declarative_submission_preserves_the_first_layout() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);
    let ui = ctx.frame();
    let viewport = ui.main_viewport();
    let root_id = ui.get_id("Duplicate declarative dockspace");
    let target =
        imgui::DockspaceTarget::new(root_id, viewport.work_pos(), viewport.work_size()).unwrap();
    let first = imgui::DockLayout::split(
        imgui::DockSplit::Left,
        0.4,
        imgui::DockLayout::tabs(["Left"]),
        imgui::DockLayout::tabs(["Right"]),
    );

    ui.dockspace_over_main_viewport_with_layout(&target, &first, imgui::DockLayoutApply::Replace)
        .unwrap();
    assert_eq!(
        ui.dockspace_over_main_viewport_with_layout(
            &target,
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
    let target = imgui::DockspaceTarget::new(root_id, [0.0, 0.0], [800.0, 600.0]).unwrap();
    let invalid = imgui::DockLayout::split(
        imgui::DockSplit::Left,
        1.0,
        imgui::DockLayout::tabs(["Left"]),
        imgui::DockLayout::tabs(["Right"]),
    );

    assert!(matches!(
        ui.dockspace_over_main_viewport_with_layout(
            &target,
            &invalid,
            imgui::DockLayoutApply::Replace,
        ),
        Err(imgui::DockLayoutError::InvalidSplitRatio { ratio: 1.0 })
    ));
    assert!(unsafe { imgui::sys::igDockBuilderGetNode(root_id.raw()).is_null() });
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
        let viewport = ui.main_viewport();
        let target =
            imgui::DockspaceTarget::new(root_id, viewport.work_pos(), viewport.work_size())
                .unwrap();
        let initial = imgui::DockLayout::split(
            imgui::DockSplit::Left,
            0.35,
            imgui::DockLayout::tabs(["Left"]),
            imgui::DockLayout::tabs(["Right"]),
        );
        ui.dockspace_over_main_viewport_with_layout(
            &target,
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
        let viewport = ui.main_viewport();
        let target =
            imgui::DockspaceTarget::new(root_id, viewport.work_pos(), viewport.work_size())
                .unwrap();
        ui.dockspace_over_main_viewport_with_layout(
            &target,
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
        let viewport = ui.main_viewport();
        let target =
            imgui::DockspaceTarget::new(root_id, viewport.work_pos(), viewport.work_size())
                .unwrap();
        ui.dockspace_over_main_viewport_with_layout(
            &target,
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
    let target = imgui::DockspaceTarget::new(root_id, viewport.work_pos(), viewport.work_size())
        .unwrap()
        .flags(imgui::DockNodeFlags::NO_RESIZE)
        .window_class(imgui::WindowClass::new(class_id));

    ui.dockspace_over_main_viewport_with_layout(
        &target,
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
            let target =
                imgui::DockspaceTarget::new(root_id, [500.0, 500.0], [200.0, 150.0]).unwrap();
            ui.dock_space_with_layout(
                &target,
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
    let target = imgui::DockspaceTarget::new(root_id, [0.0, 0.0], [800.0, 600.0]).unwrap();

    unsafe {
        imgui::sys::igSetCurrentContext(foreign_raw);
    }
    assert_eq!(unsafe { imgui::sys::igGetCurrentContext() }, foreign_raw);

    ui.dockspace_over_main_viewport_with_layout(
        &target,
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
        let viewport = ui.main_viewport();
        let target =
            imgui::DockspaceTarget::new(root_id, viewport.work_pos(), viewport.work_size())
                .unwrap();
        let layout = imgui::DockLayout::split(
            imgui::DockSplit::Left,
            0.4,
            imgui::DockLayout::tabs(["Persisted left"]),
            imgui::DockLayout::tabs(["Persisted right"]),
        );
        ui.dockspace_over_main_viewport_with_layout(
            &target,
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
    let viewport = ui.main_viewport();
    let target =
        imgui::DockspaceTarget::new(root_id, viewport.work_pos(), viewport.work_size()).unwrap();
    ui.dockspace_over_main_viewport_with_layout(
        &target,
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
    let raw_unknown = imgui::ViewportFlags::from_bits_retain(1 << 14);
    let unsupported_class =
        imgui::WindowClass::new(imgui::Id::from(1u32)).viewport_flags_override_set(raw_unknown);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ui.set_next_window_class(&unsupported_class);
        }))
        .is_err()
    );

    let overlapping_class = imgui::WindowClass::new(imgui::Id::from(2u32))
        .viewport_flags_overrides(
            imgui::ViewportFlags::NO_DECORATION,
            imgui::ViewportFlags::NO_DECORATION,
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

    let private_dock_flag =
        imgui::DockNodeFlags::from_bits_retain(imgui::sys::ImGuiDockNodeFlags_HiddenTabBar);
    let invalid_dock_class = imgui::WindowClass::new(imgui::Id::from(4u32))
        .dock_node_flags_override_set(private_dock_flag);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ui.set_next_window_class(&invalid_dock_class);
        }))
        .is_err()
    );

    let dockspace_id = ui.get_id("Window class boundaries");
    let valid_tab_options = imgui::TabItemOptions::new()
        .flags(imgui::TabItemFlags::NO_REORDER)
        .placement(imgui::TabItemPlacement::Leading);
    let valid_class = imgui::WindowClass::new(imgui::Id::from(5u32))
        .viewport_flags_overrides(
            imgui::ViewportFlags::NO_DECORATION,
            imgui::ViewportFlags::NO_TASK_BAR_ICON,
        )
        .tab_item_flags_override_set(valid_tab_options)
        .dock_node_flags_override_set(imgui::DockNodeFlags::NO_RESIZE);
    let _ = ui.window("Window class boundaries").build(|| {
        let _ = ui.dock_space_with_class(
            dockspace_id,
            [100.0, 100.0],
            imgui::DockNodeFlags::NONE,
            Some(&valid_class),
        );
    });
}

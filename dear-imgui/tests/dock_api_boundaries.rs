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

    let _ = ctx.font_atlas_mut().build();
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
fn dock_builder_rejects_invalid_flags_and_geometry_before_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let _ui = ctx.frame();
    let private_dockspace =
        imgui::DockNodeFlags::from_bits_retain(imgui::sys::ImGuiDockNodeFlags_DockSpace);

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = imgui::DockBuilder::add_node(0.into(), private_dockspace);
        }))
        .is_err()
    );

    let node_id = imgui::DockBuilder::add_node(0.into(), imgui::DockNodeFlags::NONE);
    assert_ne!(node_id.raw(), 0);

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            imgui::DockBuilder::set_node_size(node_id, [0.0, 100.0]);
        }))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            imgui::DockBuilder::set_node_pos(node_id, [f32::INFINITY, 0.0]);
        }))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = imgui::DockBuilder::split_node(node_id, imgui::SplitDirection::Left, f32::NAN);
        }))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = imgui::DockBuilder::split_node(node_id, imgui::SplitDirection::Left, 1.5);
        }))
        .is_err()
    );
}

#[test]
fn dock_builder_copy_helpers_pass_required_remap_vectors() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let _ui = ctx.frame();
    let src = imgui::DockBuilder::add_node(0.into(), imgui::DockNodeFlags::NONE);
    let dst = imgui::DockBuilder::add_node(0.into(), imgui::DockNodeFlags::NONE);

    imgui::DockBuilder::copy_node(src, dst);
    let remap = imgui::DockBuilder::copy_node_with_remap_out(dst, src);
    assert!(!remap.is_empty());

    imgui::DockBuilder::copy_dock_space(src, dst);
    imgui::DockBuilder::copy_dock_space_with_window_remap(src, dst, &[]);
}

#[test]
fn dock_builder_copy_helpers_reject_missing_source_nodes_before_ffi() {
    let _guard = test_guard();

    let mut ctx = imgui::Context::create();
    prepare_context(&mut ctx);

    let _ui = ctx.frame();

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            imgui::DockBuilder::copy_node(0.into(), 1.into());
        }))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            imgui::DockBuilder::copy_dock_space(999_999.into(), 1.into());
        }))
        .is_err()
    );
}

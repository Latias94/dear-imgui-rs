use super::DockBuilder;
use super::validation::dock_node_depth_from_i32;
use crate::{Id, sys};

#[test]
fn dock_node_depth_rejects_negative_raw_values() {
    assert_eq!(dock_node_depth_from_i32(0), 0);
    assert_eq!(dock_node_depth_from_i32(3), 3);
    assert!(
        std::panic::catch_unwind(|| {
            let _ = dock_node_depth_from_i32(-1);
        })
        .is_err()
    );
}

#[test]
fn copy_dock_space_with_window_remap_rejects_interior_nul_names() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx = crate::Context::create();
    let _ = ctx.font_atlas_mut().build();
    ctx.io_mut().set_display_size([64.0, 64.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);
    let ui = ctx.frame();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            DockBuilder::copy_dock_space_with_window_remap(
                &ui,
                Id::from(1),
                Id::from(2),
                &[("bad\0src", "dst")],
            );
        }))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            DockBuilder::copy_dock_space_with_window_remap(
                &ui,
                Id::from(1),
                Id::from(2),
                &[("src", "bad\0dst")],
            );
        }))
        .is_err()
    );
}

#[test]
fn dock_builder_uses_owner_ui_context_and_restores_previous_current_context() {
    let _guard = crate::test_support::imgui_context_guard();
    let mut ctx_a = crate::Context::create();
    let raw_a = unsafe { sys::igGetCurrentContext() };
    let raw_b = unsafe { sys::igCreateContext(std::ptr::null_mut()) };
    assert!(!raw_b.is_null());

    unsafe { sys::igSetCurrentContext(raw_a) };
    let _ = ctx_a.font_atlas_mut().build();
    ctx_a.io_mut().set_display_size([128.0, 128.0]);
    ctx_a.io_mut().set_delta_time(1.0 / 60.0);

    {
        let ui_a = ctx_a.frame();

        unsafe { sys::igSetCurrentContext(raw_b) };
        assert_eq!(unsafe { sys::igGetCurrentContext() }, raw_b);

        let node_id = DockBuilder::add_node(&ui_a, Id::from(0), crate::DockNodeFlags::NONE);

        assert_eq!(unsafe { sys::igGetCurrentContext() }, raw_b);
        assert!(crate::context::binding::with_bound_context(
            raw_a,
            || unsafe { !sys::igDockBuilderGetNode(node_id.into()).is_null() }
        ));
        assert!(crate::context::binding::with_bound_context(
            raw_b,
            || unsafe { sys::igDockBuilderGetNode(node_id.into()).is_null() }
        ));
    }

    unsafe { sys::igSetCurrentContext(raw_a) };
    let _ = ctx_a.render();
    unsafe { sys::igDestroyContext(raw_b) };

    drop(ctx_a);
}

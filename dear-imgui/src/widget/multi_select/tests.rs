use crate::sys;

use super::*;

fn setup_context() -> crate::Context {
    let mut ctx = crate::Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([128.0, 128.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas().build();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
    ctx
}

#[test]
fn multi_select_indexed_ends_scope_after_render_panic() {
    let mut ctx = setup_context();
    let raw_ctx = ctx.as_raw();

    let ui = ctx.frame();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ui.window("multi_select_panic").build(|| {
            let mut selected = vec![false; 2];
            ui.multi_select_indexed(&mut selected, MultiSelectOptions::new(), |_, idx, _| {
                if idx == 0 {
                    panic!("forced panic while multi-select is active");
                }
            });
        });
    }));

    assert!(result.is_err());
    let _ = ui.window("multi_select_after_panic").build(|| {
        let result = ui.with_multi_select(MultiSelectOptions::new(), None, 0, |_| {});
        assert!(result.requests().is_empty());
    });
    unsafe {
        let imgui_ctx = raw_ctx as *const sys::ImGuiContext;
        assert!((*imgui_ctx).CurrentMultiSelect.is_null());
        assert_eq!((*imgui_ctx).MultiSelectTempDataStacked, 0);
    }
}

#[test]
fn with_multi_select_ends_scope_exactly_once() {
    let mut ctx = setup_context();
    let raw_ctx = ctx.as_raw();

    let ui = ctx.frame();
    let _ = ui.window("multi_select_explicit_end").build(|| {
        let result = ui.with_multi_select(MultiSelectOptions::new(), None, 0, |_| {});
        assert!(result.requests().is_empty());
    });

    unsafe {
        let imgui_ctx = raw_ctx as *const sys::ImGuiContext;
        assert!((*imgui_ctx).CurrentMultiSelect.is_null());
        assert_eq!((*imgui_ctx).MultiSelectTempDataStacked, 0);
    }
}

#[test]
fn with_multi_select_rejects_items_count_over_i32() {
    let mut ctx = setup_context();

    let ui = ctx.frame();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ui.with_multi_select(
            MultiSelectOptions::new(),
            None,
            (i32::MAX as usize) + 1,
            |_| {},
        );
    }));

    assert!(result.is_err());
}

#[test]
fn result_remains_owned_after_a_later_scope_reuses_native_io() {
    let mut ctx = setup_context();

    let ui = ctx.frame();
    let _ = ui.window("multi_select_owned_result").build(|| {
        let first = ui.with_multi_select(MultiSelectOptions::new(), None, 0, |_| {});
        let first_copy = first.clone();

        let second = ui.with_multi_select(MultiSelectOptions::new(), None, 0, |scope| {
            scope.set_range_source_reset(true);
        });

        assert_eq!(first, first_copy);
        assert!(!first.range_source_reset());
        assert!(second.range_source_reset());
    });
}

#[test]
fn outer_scope_does_not_retain_io_pointer_across_nested_scope() {
    let mut ctx = setup_context();

    let ui = ctx.frame();
    let _ = ui.window("nested_multi_select").build(|| {
        let outer = ui.with_multi_select(MultiSelectOptions::new(), None, 0, |outer| {
            let inner = ui.with_multi_select(MultiSelectOptions::new(), None, 0, |_| {});
            assert!(inner.requests().is_empty());
            outer.set_range_source_reset(true);
        });

        assert!(outer.range_source_reset());
    });
}

#[test]
fn multi_select_rejects_escaped_focus_scope_before_end_ffi_and_recovers() {
    let mut ctx = setup_context();
    let ui = ctx.frame();
    let mut escaped_focus = None;

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ui.window("multi_select_focus_scope_order").build(|| {
            ui.with_multi_select(MultiSelectOptions::new(), None, 0, |_| {
                escaped_focus = Some(ui.push_focus_scope(ui.get_id("escaped-focus")));
            });
        });
    }));
    assert!(result.is_err());

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ui.text("recovery is pending");
        }))
        .is_err()
    );

    drop(escaped_focus);
    let _ = ui.window("multi_select_focus_scope_recovered").build(|| {
        ui.text("scope tracker recovered");
    });
}

use crate::sys;

use super::*;

fn setup_context() -> crate::Context {
    let mut ctx = crate::Context::create();
    {
        let io = ctx.io_mut();
        io.set_display_size([128.0, 128.0]);
        io.set_delta_time(1.0 / 60.0);
    }
    let _ = ctx.font_atlas_mut().build();
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
    unsafe {
        let imgui_ctx = raw_ctx as *const sys::ImGuiContext;
        assert!((*imgui_ctx).CurrentMultiSelect.is_null());
        assert_eq!((*imgui_ctx).MultiSelectTempDataStacked, 0);
    }
}

#[test]
fn begin_multi_select_raw_end_is_not_called_twice_on_drop() {
    let mut ctx = setup_context();
    let raw_ctx = ctx.as_raw();

    let ui = ctx.frame();
    let _ = ui.window("multi_select_explicit_end").build(|| {
        let scope = ui.begin_multi_select_raw(MultiSelectOptions::new(), None, 0);
        let _end = scope.end();
    });

    unsafe {
        let imgui_ctx = raw_ctx as *const sys::ImGuiContext;
        assert!((*imgui_ctx).CurrentMultiSelect.is_null());
        assert_eq!((*imgui_ctx).MultiSelectTempDataStacked, 0);
    }
}

#[test]
fn begin_multi_select_raw_rejects_items_count_over_i32() {
    let mut ctx = setup_context();

    let ui = ctx.frame();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ui.begin_multi_select_raw(MultiSelectOptions::new(), None, (i32::MAX as usize) + 1);
    }));

    assert!(result.is_err());
}

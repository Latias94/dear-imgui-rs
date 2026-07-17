use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::{Mutex, OnceLock};

use dear_imgui_rs::{BackendFlags, Context};
use dear_implot::{PlotContext, PlotItemArrayStyle};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn prepare_imgui(imgui: &mut Context) {
    let io = imgui.io_mut();
    io.set_display_size([800.0, 600.0]);
    io.set_delta_time(1.0 / 60.0);
    io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_TEXTURES);
}

#[test]
fn plot_ui_rejects_a_ui_from_another_context_by_identity() {
    let _guard = test_guard();
    let imgui_a = Context::create();
    let plot_a = PlotContext::create(&imgui_a);
    let suspended_a = imgui_a.suspend();

    let mut imgui_b = Context::create();
    prepare_imgui(&mut imgui_b);
    {
        let ui_b = imgui_b.frame();
        let result = catch_unwind(AssertUnwindSafe(|| plot_a.get_plot_ui(ui_b)));
        assert!(result.is_err());
    }
    let _ = imgui_b.render();
    drop(imgui_b);

    let imgui_a = suspended_a
        .activate()
        .unwrap_or_else(|_| panic!("context A should reactivate after context B is dropped"));
    drop(plot_a);
    drop(imgui_a);
}

#[test]
fn plot_binding_restores_imgui_and_implot_contexts_after_panic() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let imgui_raw = imgui.as_raw();
    let plot = PlotContext::create(&imgui);
    let previous_plot = unsafe { dear_implot_sys::ImPlot_GetCurrentContext() };

    {
        let ui = imgui.frame();
        let plot_ui = plot.get_plot_ui(ui);
        unsafe {
            dear_imgui_rs::sys::igSetCurrentContext(ptr::null_mut());
            dear_implot_sys::ImPlot_SetCurrentContext(ptr::null_mut());
        }

        let result = catch_unwind(AssertUnwindSafe(|| {
            plot_ui.with_next_plot_item_array_style(PlotItemArrayStyle::new(), |_| {
                panic!("binding panic probe")
            });
        }));

        assert!(result.is_err());
        assert!(unsafe { dear_imgui_rs::sys::igGetCurrentContext() }.is_null());
        assert!(unsafe { dear_implot_sys::ImPlot_GetCurrentContext() }.is_null());
        unsafe {
            dear_imgui_rs::sys::igSetCurrentContext(imgui_raw);
            dear_implot_sys::ImPlot_SetCurrentContext(previous_plot);
        }
    }

    let _ = imgui.render();
    drop(plot);
}

#[test]
fn dead_imgui_context_rejects_calls_and_plot_drop_skips_ffi() {
    let _guard = test_guard();
    let imgui_a = Context::create();
    let plot_a = PlotContext::create(&imgui_a);
    drop(imgui_a);

    let imgui_b = Context::create();
    let plot_b = PlotContext::create(&imgui_b);
    let plot_b_raw = unsafe { plot_b.raw() };
    unsafe { dear_implot_sys::ImPlot_SetCurrentContext(plot_b_raw) };

    let result = catch_unwind(AssertUnwindSafe(|| plot_a.colormap_count()));
    assert!(result.is_err());
    assert_eq!(
        unsafe { dear_implot_sys::ImPlot_GetCurrentContext() },
        plot_b_raw
    );

    drop(plot_a);
    assert_eq!(
        unsafe { dear_implot_sys::ImPlot_GetCurrentContext() },
        plot_b_raw
    );
    drop(plot_b);
    drop(imgui_b);
}

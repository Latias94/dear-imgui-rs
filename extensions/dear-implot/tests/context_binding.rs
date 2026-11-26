use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, OnceLock};

use dear_imgui_rs::Context;
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
    imgui
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("headless test requires the legacy font-atlas capability")
        .build();
}

#[test]
fn plot_ui_rejects_a_ui_from_another_context_by_identity() {
    let _guard = test_guard();
    let imgui_a = Context::create();
    let plot_a = PlotContext::create(&imgui_a);

    let mut imgui_b = Context::create();
    prepare_imgui(&mut imgui_b);
    {
        let frame = imgui_b.begin_frame();
        let result = catch_unwind(AssertUnwindSafe(|| plot_a.get_plot_ui(frame.ui())));
        assert!(result.is_err());
    }
    drop(imgui_b);

    drop(plot_a);
    drop(imgui_a);
}

#[test]
fn plot_binding_restores_imgui_and_implot_contexts_after_panic() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let plot = PlotContext::create(&imgui);
    let previous_imgui = Context::create();
    let previous_plot = PlotContext::create(&previous_imgui);
    let previous_raw = unsafe { previous_plot.raw() };

    {
        let frame = imgui.begin_frame();
        let plot_ui = plot.get_plot_ui(frame.ui());
        previous_imgui.binding().with_bound_context(|| {
            unsafe { dear_implot_sys::ImPlot_SetCurrentContext(previous_raw) };

            let result = catch_unwind(AssertUnwindSafe(|| {
                unsafe {
                    // The empty style contains no array pointers to validate.
                    plot_ui.with_plot("unwind", |plot| {
                        plot.with_next_plot_item_array_style(PlotItemArrayStyle::new(), |_| {
                            panic!("binding panic probe")
                        });
                    });
                }
            }));

            assert!(result.is_err());
            assert_eq!(
                unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
                previous_imgui.as_raw()
            );
            assert_eq!(
                unsafe { dear_implot_sys::ImPlot_GetCurrentContext() },
                previous_raw
            );
        });
    }
    drop(plot);
}

#[test]
fn dead_imgui_context_rejects_calls_and_plot_drop_preserves_the_current_owner() {
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

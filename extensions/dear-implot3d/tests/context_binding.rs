use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::{Mutex, OnceLock};

use dear_imgui_rs::Context;
use dear_implot3d::{Axis3D, Plot3DContext, Plot3DItemArrayStyle};

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
fn plot3d_ui_rejects_a_ui_from_another_context_by_identity() {
    let _guard = test_guard();
    let imgui_a = Context::create();
    let plot_a = Plot3DContext::create(&imgui_a);
    let suspended_a = imgui_a.suspend_or_panic();

    let mut imgui_b = Context::create();
    prepare_imgui(&mut imgui_b);
    {
        let frame = imgui_b.begin_frame();
        let result = catch_unwind(AssertUnwindSafe(|| plot_a.get_plot_ui(frame.ui())));
        assert!(result.is_err());
    }
    drop(imgui_b);

    let imgui_a = suspended_a
        .activate()
        .unwrap_or_else(|_| panic!("context A should reactivate after context B is dropped"));
    drop(plot_a);
    drop(imgui_a);
}

#[test]
fn plot3d_binding_restores_imgui_and_plot_contexts_after_panic() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let imgui_raw = imgui.as_raw();
    let plot = Plot3DContext::create(&imgui);
    let previous_plot = unsafe { dear_implot3d_sys::ImPlot3D_GetCurrentContext() };

    {
        let frame = imgui.begin_frame();
        let plot_ui = plot.get_plot_ui(frame.ui());
        unsafe {
            dear_imgui_rs::sys::igSetCurrentContext(ptr::null_mut());
            dear_implot3d_sys::ImPlot3D_SetCurrentContext(ptr::null_mut());
        }

        let result = catch_unwind(AssertUnwindSafe(|| {
            unsafe {
                // The empty style contains no array pointers to validate.
                plot_ui.with_next_plot3d_item_array_style(Plot3DItemArrayStyle::new(), |_| {
                    panic!("binding panic probe")
                });
            }
        }));

        assert!(result.is_err());
        assert!(unsafe { dear_imgui_rs::sys::igGetCurrentContext() }.is_null());
        assert!(unsafe { dear_implot3d_sys::ImPlot3D_GetCurrentContext() }.is_null());
        unsafe {
            dear_imgui_rs::sys::igSetCurrentContext(imgui_raw);
            dear_implot3d_sys::ImPlot3D_SetCurrentContext(previous_plot);
        }
    }
    drop(plot);
}

#[test]
fn dead_imgui_context_rejects_calls_and_plot3d_drop_skips_ffi() {
    let _guard = test_guard();
    let imgui_a = Context::create();
    let plot_a = Plot3DContext::create(&imgui_a);
    drop(imgui_a);

    let imgui_b = Context::create();
    let plot_b = Plot3DContext::create(&imgui_b);
    let plot_b_raw = unsafe { plot_b.raw() };
    unsafe { dear_implot3d_sys::ImPlot3D_SetCurrentContext(plot_b_raw) };

    let result = catch_unwind(AssertUnwindSafe(|| plot_a.colormap_count()));
    assert!(result.is_err());
    assert_eq!(
        unsafe { dear_implot3d_sys::ImPlot3D_GetCurrentContext() },
        plot_b_raw
    );

    drop(plot_a);
    assert_eq!(
        unsafe { dear_implot3d_sys::ImPlot3D_GetCurrentContext() },
        plot_b_raw
    );
    drop(plot_b);
    drop(imgui_b);
}

#[test]
#[should_panic(expected = "n_ticks must be at least 2 when labels are provided")]
fn axis_ticks_range_rejects_zero_ticks_with_labels_before_ffi() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let plot = Plot3DContext::create(&imgui);
    let frame = imgui.begin_frame();
    let plot_ui = plot.get_plot_ui(frame.ui());

    // No plot is begun: count validation must run before native setup.
    plot_ui.setup_axis_ticks_range(Axis3D::X, 0.0, 1.0, 0, Some(&[]), false);
}

#[test]
#[should_panic(expected = "n_ticks must be at least 2 when labels are provided")]
fn axis_ticks_range_rejects_one_tick_with_labels_before_ffi() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let plot = Plot3DContext::create(&imgui);
    let frame = imgui.begin_frame();
    let plot_ui = plot.get_plot_ui(frame.ui());

    plot_ui.setup_axis_ticks_range(Axis3D::X, 0.0, 1.0, 1, Some(&["only tick"]), false);
}

#[test]
#[should_panic(expected = "n_ticks must be positive")]
fn axis_ticks_range_rejects_zero_ticks_without_labels_before_ffi() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let plot = Plot3DContext::create(&imgui);
    let frame = imgui.begin_frame();
    let plot_ui = plot.get_plot_ui(frame.ui());

    plot_ui.setup_axis_ticks_range(Axis3D::X, 0.0, 1.0, 0, None, false);
}

#[test]
fn axis_ticks_range_accepts_two_labels_and_unlabeled_counts() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let plot = Plot3DContext::create(&imgui);
    let frame = imgui.begin_frame();
    let plot_ui = plot.get_plot_ui(frame.ui());
    let _plot = plot_ui
        .begin_plot("axis ticks range native probe")
        .build()
        .expect("axis ticks probe plot should begin");

    plot_ui.setup_axis_ticks_range(Axis3D::X, 0.0, 1.0, 2, Some(&["min", "max"]), false);
    plot_ui.setup_axis_ticks_range(Axis3D::Y, 0.0, 1.0, 1, None, false);
    plot_ui.setup_axis_ticks_range(Axis3D::Z, 0.0, 1.0, 2, None, true);
}

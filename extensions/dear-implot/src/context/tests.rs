use super::{PlotContext, validation::axis_tick_count_to_i32};
use crate::sys;
use crate::{Axis, PlotCond, XAxis, YAxis};
use dear_imgui_rs::{Context, ContextBindingError};
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|err| err.into_inner())
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
fn hidden_item_query_survives_frames_and_last_item_resets_on_begin_plot() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let plot = PlotContext::create(&imgui);

    for was_hidden in [false, true] {
        let frame = imgui.begin_frame();
        {
            let ui = plot.get_plot_ui(frame.ui());
            let _token = ui.begin_plot("visibility").expect("plot must begin");
            ui.with_bound_context(|| {
                // SAFETY: Both contexts and the plot are active, and the label is a C literal.
                unsafe {
                    assert!(!sys::ImPlot_IsLastItemHidden());
                    assert_eq!(sys::ImPlot_IsItemHidden(c"series".as_ptr()), was_hidden);
                    sys::ImPlot_HideNextItem(true, sys::ImPlotCond_Always as _);
                }
                crate::DummyPlot::new("series").plot(&ui);
                assert!(unsafe { sys::ImPlot_IsLastItemHidden() });
            });
        }
        let _ = frame.render_legacy();
    }
}

fn histogram_counts(ui: &crate::PlotUi<'_>) -> Vec<f64> {
    ui.with_bound_context(|| unsafe {
        // PlotHistogram2D fills TempDouble1 before drawing. Copy it before the next ImPlot call.
        let bins = &(*sys::ImPlot_GetCurrentContext()).TempDouble1;
        assert!(bins.Size > 0 && !bins.Data.is_null());
        std::slice::from_raw_parts(bins.Data, bins.Size as usize).to_vec()
    })
}

#[test]
fn histogram_2d_bins_both_automatic_ranges_through_the_plot_ui() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let plot = PlotContext::create(&imgui);
    let frame = imgui.begin_frame();
    let ui = plot.get_plot_ui(frame.ui());
    let _token = ui.begin_plot("counts").expect("plot must begin");
    let xs = [10.0, 11.0, 18.0, 20.0, 12.0];
    let ys = [-7.0, 3.0, -6.0, 2.0, -7.0];

    ui.histogram_2d_plot_with_bins(
        "series",
        &xs,
        &ys,
        crate::BinMethod::Sqrt,
        crate::BinMethod::Rice,
    )
    .unwrap();
    // Three X bins and four Y bins. Both maxima belong to the last bin on their axis.
    assert_eq!(
        histogram_counts(&ui),
        [2., 0., 1., 0., 0., 0., 0., 0., 0., 1., 0., 1.]
    );

    ui.histogram_2d_plot("default", &xs, &ys).unwrap();
    assert_eq!(
        histogram_counts(&ui),
        [
            2., 0., 0., 1., 0., 0., 0., 0., 0., 0., 0., 0., 1., 0., 0., 1.
        ]
    );
}

#[test]
fn histogram_2d_scott_matches_native_bins_for_large_offset_samples() {
    use crate::{BinMethod, Histogram2DPlot, Plot};

    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let plot = PlotContext::create(&imgui);
    let frame = imgui.begin_frame();
    let ui = plot.get_plot_ui(frame.ui());
    let base = (1_u64 << 52) as f64;
    let xs = [base, base, base + 2.0, base + 3.0, base + 5.0];
    let ys = [0.0, 1.0, 2.0, 3.0, 4.0];
    ui.set_next_x_axis_limits(XAxis::X1, base, base + 5.0, PlotCond::Always);
    let _token = ui.begin_plot("Scott").expect("plot must begin");
    let range = sys::ImPlotRect {
        X: sys::ImPlotRange {
            Min: base,
            Max: base + 5.0,
        },
        Y: sys::ImPlotRange { Min: 0.0, Max: 4.0 },
    };

    Histogram2DPlot::new("safe", &xs, &ys)
        .with_bins(BinMethod::Scott, 2usize)
        .plot(&ui);
    let safe = histogram_counts(&ui);
    assert_eq!(safe, [2., 3.]);

    let native_peak = ui.with_bound_context(|| unsafe {
        // Explicit bounds isolate native CalculateBins from the upstream automatic X-range typo.
        sys::ImPlot_PlotHistogram2D_doublePtr(
            c"reference".as_ptr(),
            xs.as_ptr(),
            ys.as_ptr(),
            5,
            BinMethod::Scott as i32,
            2,
            range,
            crate::plots::default_plot_spec(),
        )
    });
    assert_eq!(native_peak, 3.0);
    assert_eq!(histogram_counts(&ui), safe);
}

#[test]
fn nested_plot_bindings_restore_imgui_and_implot_contexts() {
    let _guard = test_guard();
    let imgui_a = Context::create();
    let imgui_a_raw = imgui_a.as_raw();
    let plot_a = PlotContext::create(&imgui_a);
    let plot_a_raw = unsafe { plot_a.raw() };
    let suspended_a = imgui_a.suspend_or_panic();

    let imgui_b = Context::create();
    let imgui_b_raw = imgui_b.as_raw();
    let plot_b = PlotContext::create(&imgui_b);
    let plot_b_raw = unsafe { plot_b.raw() };
    unsafe { sys::ImPlot_SetCurrentContext(plot_b_raw) };

    let binding_a = plot_a.binding();
    let binding_b = plot_b.binding();
    binding_a.with_bound_context("test outer ImPlot binding", || {
        assert_eq!(
            unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
            imgui_a_raw
        );
        assert_eq!(unsafe { sys::ImPlot_GetCurrentContext() }, plot_a_raw);

        binding_b.with_bound_context("test inner ImPlot binding", || {
            assert_eq!(
                unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
                imgui_b_raw
            );
            assert_eq!(unsafe { sys::ImPlot_GetCurrentContext() }, plot_b_raw);
        });

        assert_eq!(
            unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
            imgui_a_raw
        );
        assert_eq!(unsafe { sys::ImPlot_GetCurrentContext() }, plot_a_raw);
    });

    assert_eq!(
        unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
        imgui_b_raw
    );
    assert_eq!(unsafe { sys::ImPlot_GetCurrentContext() }, plot_b_raw);

    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        binding_a.with_bound_context("test panicking outer ImPlot binding", || {
            binding_b.with_bound_context("test panicking inner ImPlot binding", || {
                panic!("nested binding panic probe");
            });
        });
    }));
    assert!(panic_result.is_err());
    assert_eq!(
        unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
        imgui_b_raw
    );
    assert_eq!(unsafe { sys::ImPlot_GetCurrentContext() }, plot_b_raw);

    drop(plot_b);
    drop(imgui_b);
    let imgui_a = suspended_a
        .activate()
        .unwrap_or_else(|_| panic!("context A should reactivate after context B is dropped"));
    drop(plot_a);
    drop(imgui_a);
}

#[test]
fn nested_plot_binding_never_restores_a_destroyed_outer_context() {
    let _guard = test_guard();
    let imgui_a = Context::create();
    let plot_a = PlotContext::create(&imgui_a);
    let binding_a = plot_a.binding();
    let mut suspended_a = Some(imgui_a.suspend_or_panic());

    let imgui_b = Context::create();
    let imgui_b_raw = imgui_b.as_raw();
    let plot_b = PlotContext::create(&imgui_b);
    let plot_b_raw = unsafe { plot_b.raw() };
    let binding_b = plot_b.binding();
    unsafe { sys::ImPlot_SetCurrentContext(plot_b_raw) };

    binding_a.with_bound_context("test outer ImPlot binding", || {
        binding_b.with_bound_context("test inner ImPlot binding", || {
            drop(
                suspended_a
                    .take()
                    .expect("context A must still be owned by the test"),
            );
            assert_eq!(
                unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
                imgui_b_raw
            );
        });

        assert!(unsafe { dear_imgui_rs::sys::igGetCurrentContext() }.is_null());
    });

    assert_eq!(
        unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
        imgui_b_raw
    );
    assert_eq!(unsafe { sys::ImPlot_GetCurrentContext() }, plot_b_raw);

    drop(plot_a);
    assert_eq!(unsafe { sys::ImPlot_GetCurrentContext() }, plot_b_raw);
    drop(plot_b);
    drop(imgui_b);
}

#[test]
fn dead_plot_binding_never_enters_the_native_scope() {
    let _guard = test_guard();
    let imgui = Context::create();
    let plot = PlotContext::create(&imgui);
    let binding = plot.binding();
    drop(imgui);

    let entered = Cell::new(false);
    let result = binding.try_with_bound_context(|| entered.set(true));

    assert!(matches!(result, Err(ContextBindingError::NativeDestroyed)));
    assert!(!entered.get());

    drop(plot);
    assert!(!entered.get());
}

#[test]
fn axis_ticks_range_count_is_checked_before_ffi() {
    assert_eq!(axis_tick_count_to_i32("test", 1, false), 1);
    assert_eq!(axis_tick_count_to_i32("test", 2, true), 2);
    assert!(std::panic::catch_unwind(|| axis_tick_count_to_i32("test", 1, true)).is_err());
    assert_eq!(
        axis_tick_count_to_i32("test", i32::MAX as usize, false),
        i32::MAX
    );

    assert!(
        std::panic::catch_unwind(|| axis_tick_count_to_i32("test", 0, false)).is_err(),
        "zero tick counts must not cross the safe API boundary"
    );
    assert!(
        std::panic::catch_unwind(|| {
            axis_tick_count_to_i32("test", i32::MAX as usize + 1, false);
        })
        .is_err(),
        "oversized tick counts must not cross the safe API boundary"
    );
}

#[test]
fn plot_ui_binds_own_context_before_calls() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let plot_a = PlotContext::create(&imgui);
    let raw_a = unsafe { plot_a.raw() };
    let plot_b = PlotContext::create(&imgui);
    let raw_b = unsafe { plot_b.raw() };

    {
        let frame = imgui.begin_frame();
        let plot_ui = plot_a.get_plot_ui(frame.ui());
        unsafe { sys::ImPlot_SetCurrentContext(raw_b) };

        {
            plot_ui.with_bound_context(|| {
                assert_eq!(unsafe { sys::ImPlot_GetCurrentContext() }, raw_a);
            })
        }
        assert_eq!(unsafe { sys::ImPlot_GetCurrentContext() }, raw_b);

        plot_ui.set_next_axes_to_fit();

        assert_eq!(unsafe { sys::ImPlot_GetCurrentContext() }, raw_b);
    }

    drop(plot_b);
    drop(plot_a);
}

#[test]
fn plot_token_binds_own_context_before_drop() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let plot_a = PlotContext::create(&imgui);
    let plot_b = PlotContext::create(&imgui);
    let raw_b = unsafe { plot_b.raw() };

    {
        let frame = imgui.begin_frame();
        let plot_ui = plot_a.get_plot_ui(frame.ui());
        let token = plot_ui.begin_plot("token").expect("failed to begin plot");

        unsafe { sys::ImPlot_SetCurrentContext(raw_b) };
        drop(token);

        assert_eq!(unsafe { sys::ImPlot_GetCurrentContext() }, raw_b);
    }

    drop(plot_b);
    drop(plot_a);
}

#[test]
fn style_and_plot_clip_tokens_bind_own_context_before_drop() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let plot_a = PlotContext::create(&imgui);
    let plot_b = PlotContext::create(&imgui);
    let raw_b = unsafe { plot_b.raw() };

    {
        let frame = imgui.begin_frame();
        let plot_ui = plot_a.get_plot_ui(frame.ui());
        let style = plot_ui.push_style_var_f32(crate::StyleVar::MinorAlpha, 0.5);
        unsafe { sys::ImPlot_SetCurrentContext(raw_b) };
        drop(style);
        assert_eq!(unsafe { sys::ImPlot_GetCurrentContext() }, raw_b);

        let token = plot_ui.begin_plot("clip").expect("failed to begin plot");
        let clip = token.push_plot_clip_rect(0.0);
        unsafe { sys::ImPlot_SetCurrentContext(raw_b) };
        drop(clip);
        assert_eq!(unsafe { sys::ImPlot_GetCurrentContext() }, raw_b);
        drop(token);
    }

    drop(plot_b);
    drop(plot_a);
}

#[test]
fn dropping_current_plot_context_clears_current_context() {
    let _guard = test_guard();
    let imgui = Context::create();
    let plot = PlotContext::create(&imgui);
    let raw = unsafe { plot.raw() };

    unsafe { sys::ImPlot_SetCurrentContext(raw) };
    drop(plot);

    assert!(unsafe { sys::ImPlot_GetCurrentContext() }.is_null());
}

#[test]
fn dropping_non_current_plot_context_restores_previous_context() {
    let _guard = test_guard();
    let imgui = Context::create();
    let plot_a = PlotContext::create(&imgui);
    let plot_b = PlotContext::create(&imgui);
    let raw_b = unsafe { plot_b.raw() };

    unsafe { sys::ImPlot_SetCurrentContext(raw_b) };
    drop(plot_a);

    assert_eq!(unsafe { sys::ImPlot_GetCurrentContext() }, raw_b);
    drop(plot_b);
}

#[test]
#[should_panic(expected = "PlotUi::set_next_x_axis_limits() min must be finite")]
fn set_next_axis_limits_rejects_non_finite_values_before_ffi() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let plot = PlotContext::create(&imgui);

    {
        let frame = imgui.begin_frame();
        let plot_ui = plot.get_plot_ui(frame.ui());
        plot_ui.set_next_x_axis_limits(XAxis::X1, f64::NAN, 1.0, PlotCond::Once);
    }
}

#[test]
#[should_panic(expected = "PlotUi::setup_axis_zoom_constraints() min must be positive")]
fn axis_zoom_constraints_reject_non_positive_min_before_ffi() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let plot = PlotContext::create(&imgui);

    {
        let frame = imgui.begin_frame();
        let plot_ui = plot.get_plot_ui(frame.ui());
        let token = plot_ui
            .begin_plot("constraints")
            .expect("failed to begin plot");
        plot_ui.setup_axis_zoom_constraints(Axis::Y1, 0.0, 10.0);
        token.end();
    }
}

#[test]
fn typed_axis_apis_accept_valid_axes() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    // Storage outlives PlotContext, including during unwinding.
    let mut x_min = 0.0;
    let mut x_max = 1.0;
    let mut min = 0.0;
    let mut max = 1.0;
    let plot = PlotContext::create(&imgui);

    {
        let frame = imgui.begin_frame();
        let plot_ui = plot.get_plot_ui(frame.ui());
        plot_ui.set_next_axis_to_fit(Axis::X1);

        // SAFETY: distinct, stable storage outlives PlotContext and is not accessed while linked.
        unsafe { plot_ui.set_next_axis_links(Axis::X1, Some(&mut x_min), Some(&mut x_max)) };
        plot_ui.set_next_x_axis_limits(XAxis::X1, -2.0, 2.0, PlotCond::Always);

        let token = plot_ui
            .begin_plot("typed-axis")
            .expect("failed to begin plot");
        plot_ui.setup_x_axis(XAxis::X1, None, crate::AxisFlags::NONE);
        plot_ui.setup_y_axis(YAxis::Y1, None, crate::AxisFlags::NONE);
        // SAFETY: Y1 is enabled and setup is unlocked; storage outlives PlotContext.
        unsafe { plot_ui.setup_axis_links(Axis::Y1, Some(&mut min), Some(&mut max)) };
        plot_ui.setup_y_axis_limits(YAxis::Y1, 2.0, 5.0, PlotCond::Always);
        plot_ui.setup_axis_limits_constraints(Axis::Y1, -10.0, 10.0);
        plot_ui.setup_axis_zoom_constraints(Axis::Y1, 0.1, 20.0);
        token.end();
    }
    drop(plot);
    assert_eq!((x_min, x_max), (-2.0, 2.0));
    assert_eq!((min, max), (2.0, 5.0));
}

#[test]
fn secondary_axis_links_are_explicitly_unlinked_on_the_same_plot() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    // Storage also outlives PlotContext if any frame panics before unlinking.
    let mut min = 0.0;
    let mut max = 1.0;
    let plot = PlotContext::create(&imgui);

    {
        let frame = imgui.begin_frame();
        let plot_ui = plot.get_plot_ui(frame.ui());
        let token = plot_ui
            .begin_plot("secondary-links")
            .expect("plot must begin");
        plot_ui.setup_y_axis(YAxis::Y2, None, crate::AxisFlags::NONE);
        // SAFETY: Y2 is enabled; distinct storage outlives PlotContext even on unwinding.
        unsafe { plot_ui.setup_axis_links(Axis::Y2, Some(&mut min), Some(&mut max)) };
        plot_ui.setup_y_axis_limits(YAxis::Y2, 3.0, 7.0, PlotCond::Always);
        token.end();
    }
    {
        let frame = imgui.begin_frame();
        let plot_ui = plot.get_plot_ui(frame.ui());
        let token = plot_ui
            .begin_plot("secondary-links")
            .expect("plot must begin");
        plot_ui.setup_y_axis(YAxis::Y2, None, crate::AxisFlags::NONE);
        // SAFETY: the same plot ID and enabled axis are in their setup phase.
        unsafe { plot_ui.setup_axis_links(Axis::Y2, None, None) };
        token.end();
    }
    assert_eq!((min, max), (3.0, 7.0));
    // Explicit unlinking permits access before PlotContext is destroyed.
    min = 11.0;
    max = 13.0;
    {
        let frame = imgui.begin_frame();
        let plot_ui = plot.get_plot_ui(frame.ui());
        let token = plot_ui
            .begin_plot("secondary-links")
            .expect("plot must begin");
        token.end();
    }
    drop(plot);
    // A disabled Y2 must not write its old limits back through retained pointers.
    assert_eq!((min, max), (11.0, 13.0));
}

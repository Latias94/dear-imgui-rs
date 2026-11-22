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
fn style_editor_saves_and_reverts_each_plot_context_independently() {
    use crate::Colormap;

    let _guard = test_guard();
    let mut imgui = Context::create();
    imgui.set_ini_filename::<std::path::PathBuf>(None).unwrap();
    prepare_imgui(&mut imgui);
    let a = PlotContext::create(&imgui);
    let b = PlotContext::create(&imgui);
    let mut show = |owner: &PlotContext, next_button: Option<&str>| {
        let frame = imgui.begin_frame();
        let ui = frame.ui();
        ui.window("Plot style reference")
            .size([600.0, 440.0], dear_imgui_rs::Condition::Always)
            .build(|| {
                owner.get_plot_ui(ui).show_style_editor();
                if let Some(button) = next_button {
                    // Activate the editor's ordinary button on the following frame.
                    unsafe { dear_imgui_rs::sys::igActivateItemByID(ui.get_id(button).raw()) };
                }
            })
            .unwrap();
    };
    a.set_style_colormap(Colormap::Hot);
    b.set_style_colormap(Colormap::Cool);
    show(&a, Some("Save Ref"));
    a.set_style_colormap(Colormap::Viridis);
    show(&a, None);
    show(&b, None);
    a.set_style_colormap(Colormap::Dark);
    b.set_style_colormap(Colormap::Deep);
    show(&b, Some("Revert Ref"));
    show(&b, Some("Revert Ref"));
    assert_eq!(b.style_colormap_index(), Some(Colormap::Cool.index()));
    show(&a, None);
    assert_eq!(a.style_colormap_index(), Some(Colormap::Viridis.index()));
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
            ui.with_plot("visibility", |plot| {
                plot.with_bound_context(|| {
                    // SAFETY: Both contexts and the plot are active, and the label is a C literal.
                    unsafe {
                        assert!(!sys::ImPlot_IsLastItemHidden());
                        assert_eq!(sys::ImPlot_IsItemHidden(c"series".as_ptr()), was_hidden);
                        sys::ImPlot_HideNextItem(true, sys::ImPlotCond_Always as _);
                    }
                    crate::DummyPlot::new("series").plot(plot);
                    assert!(unsafe { sys::ImPlot_IsLastItemHidden() });
                })
            })
            .expect("plot must begin");
        }
        let _ = frame.render_legacy();
    }
}

fn histogram_counts(ui: &crate::PlotScope<'_>) -> Vec<f64> {
    ui.with_bound_context(|| unsafe {
        // PlotHistogram2D fills TempDouble1 before drawing. Copy it before the next ImPlot call.
        let bins = &(*sys::ImPlot_GetCurrentContext()).TempDouble1;
        assert!(bins.Size > 0 && !bins.Data.is_null());
        std::slice::from_raw_parts(bins.Data, bins.Size as usize).to_vec()
    })
}

#[test]
fn asymmetric_error_bars_fit_the_signed_endpoints() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let plot = PlotContext::create(&imgui);
    let frame = imgui.begin_frame();
    let ui = plot.get_plot_ui(frame.ui());
    ui.set_next_axes_to_fit();
    ui.with_plot("signed errors", |plot| {
        // Both endpoints can be above the estimate: 10 - (-2) and 10 + 5.
        plot.asymmetric_error_bars_plot("interval", &[1.0], &[10.0], &[-2.0], &[5.0])
            .unwrap();
        plot.with_bound_context(|| unsafe {
            let plot = (*sys::ImPlot_GetCurrentContext()).CurrentPlot;
            let fit = (*plot).Axes[YAxis::Y1 as usize].FitExtents;
            assert_eq!((fit.Min, fit.Max), (12.0, 15.0));
        });
    })
    .expect("plot must begin");
}

#[test]
fn histogram_1d_counts_constant_samples_in_a_finite_bin() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let plot = PlotContext::create(&imgui);
    let frame = imgui.begin_frame();
    let ui = plot.get_plot_ui(frame.ui());
    ui.with_plot("constant samples", |plot| {
        plot.histogram_plot_with_bins("sensor", &[7.0; 3], crate::BinMethod::Scott)
            .unwrap();
        plot.with_bound_context(|| unsafe {
            // PlotHistogram stores its bin centers and counts in the two scratch vectors.
            let ctx = &*sys::ImPlot_GetCurrentContext();
            assert_eq!((ctx.TempDouble1.Size, ctx.TempDouble2.Size), (1, 1));
            assert_eq!((*ctx.TempDouble1.Data, *ctx.TempDouble2.Data), (7.0, 3.0));
        });
        assert!(
            plot.histogram_plot_with_bins("invalid", &[7.0], 0usize)
                .is_err()
        );
    })
    .expect("plot must begin");
}

#[test]
fn histogram_2d_bins_both_automatic_ranges_through_the_plot_ui() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let plot = PlotContext::create(&imgui);
    let frame = imgui.begin_frame();
    let ui = plot.get_plot_ui(frame.ui());
    ui.with_plot("counts", |plot| {
        let xs = [10.0, 11.0, 18.0, 20.0, 12.0];
        let ys = [-7.0, 3.0, -6.0, 2.0, -7.0];

        plot.histogram_2d_plot_with_bins(
            "series",
            &xs,
            &ys,
            crate::BinMethod::Sqrt,
            crate::BinMethod::Rice,
        )
        .unwrap();
        // Three X bins and four Y bins. Both maxima belong to the last bin on their axis.
        assert_eq!(
            histogram_counts(plot),
            [2., 0., 1., 0., 0., 0., 0., 0., 0., 1., 0., 1.]
        );

        plot.histogram_2d_plot("default", &xs, &ys).unwrap();
        assert_eq!(
            histogram_counts(plot),
            [
                2., 0., 0., 1., 0., 0., 0., 0., 0., 0., 0., 0., 1., 0., 0., 1.
            ]
        );
    })
    .expect("plot must begin");
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
    ui.with_plot("Scott", |plot| {
        let range = sys::ImPlotRect {
            X: sys::ImPlotRange {
                Min: base,
                Max: base + 5.0,
            },
            Y: sys::ImPlotRange { Min: 0.0, Max: 4.0 },
        };

        Histogram2DPlot::new("safe", &xs, &ys)
            .with_bins(BinMethod::Scott, 2usize)
            .plot(plot);
        let safe = histogram_counts(plot);
        assert_eq!(safe, [2., 3.]);

        let native_peak = plot.with_bound_context(|| unsafe {
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
        assert_eq!(histogram_counts(plot), safe);
    })
    .expect("plot must begin");
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

    plot_a.with_bound_context("test outer ImPlot binding", || {
        assert_eq!(
            unsafe { dear_imgui_rs::sys::igGetCurrentContext() },
            imgui_a_raw
        );
        assert_eq!(unsafe { sys::ImPlot_GetCurrentContext() }, plot_a_raw);

        plot_b.with_bound_context("test inner ImPlot binding", || {
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
        plot_a.with_bound_context("test panicking outer ImPlot binding", || {
            plot_b.with_bound_context("test panicking inner ImPlot binding", || {
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
    let mut suspended_a = Some(imgui_a.suspend_or_panic());

    let imgui_b = Context::create();
    let imgui_b_raw = imgui_b.as_raw();
    let plot_b = PlotContext::create(&imgui_b);
    let plot_b_raw = unsafe { plot_b.raw() };
    unsafe { sys::ImPlot_SetCurrentContext(plot_b_raw) };

    plot_a.with_bound_context("test outer ImPlot binding", || {
        plot_b.with_bound_context("test inner ImPlot binding", || {
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
fn plot_owner_keeps_its_thread_and_destroys_the_native_plot_after_imgui_drops() {
    let _guard = test_guard();
    let imgui = Context::create();
    let plot = PlotContext::create(&imgui);
    unsafe { sys::ImPlot_SetCurrentContext(plot.raw()) };
    drop(imgui);

    let entered = Cell::new(false);
    let result = plot.try_with_bound_context(|| entered.set(true));

    assert!(matches!(result, Err(ContextBindingError::NativeDestroyed)));
    assert!(!entered.get());

    std::thread::spawn(|| {
        assert!(matches!(
            dear_imgui_rs::ContextThreadLease::acquire(),
            Err(dear_imgui_rs::ImGuiError::ContextThreadConflict)
        ))
    })
    .join()
    .unwrap();
    drop(plot);
    std::thread::spawn(|| {
        let _lease = dear_imgui_rs::ContextThreadLease::acquire().unwrap();
        assert!(unsafe { sys::ImPlot_GetCurrentContext() }.is_null());
    })
    .join()
    .unwrap();
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
fn plot_and_clip_scopes_restore_the_previous_native_context() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let a = PlotContext::create(&imgui);
    let b = PlotContext::create(&imgui);
    let raw_b = unsafe { b.raw() };
    let frame = imgui.begin_frame();
    let ui = a.get_plot_ui(frame.ui());
    ui.with_style_var_f32(crate::StyleVar::MinorAlpha, 0.5, || {
        ui.with_plot("clip", |plot| {
            plot.with_plot_clip_rect(0.0, || unsafe { sys::ImPlot_SetCurrentContext(raw_b) });
            assert_eq!(unsafe { sys::ImPlot_GetCurrentContext() }, raw_b);
        })
        .expect("visible plot");
    });
    assert_eq!(unsafe { sys::ImPlot_GetCurrentContext() }, raw_b);
    a.with_bound_context("closed scopes", || unsafe {
        assert!((*a.raw()).CurrentPlot.is_null());
        assert_eq!((*a.raw()).StyleModifiers.Size, 0);
    });
}

#[test]
fn temporary_context_switch_forgets_a_destroyed_previous_plot_owner() {
    let _guard = test_guard();
    let imgui = Context::create();
    let a = PlotContext::create(&imgui);
    let b = PlotContext::create(&imgui);
    unsafe { sys::ImPlot_SetCurrentContext(a.raw()) };
    b.with_bound_context("temporary owner", || drop(a));
    assert!(unsafe { sys::ImPlot_GetCurrentContext() }.is_null());
}

#[test]
fn styles_check_the_context_and_a_nested_plot_cannot_consume_outer_arrays() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let a = PlotContext::create(&imgui);
    let b = PlotContext::create(&imgui);
    let frame = imgui.begin_frame();
    let ui = frame.ui();
    let outer = a.get_plot_ui(ui);
    let unknown = crate::ColormapIndex::from(a.colormap_count());
    assert!(catch_unwind(AssertUnwindSafe(|| outer.with_colormap(unknown, || ()))).is_err());
    assert!(
        catch_unwind(AssertUnwindSafe(|| outer.with_style_var_f32(
            crate::StyleVar::PlotPadding,
            0.5,
            || ()
        )))
        .is_err()
    );
    ui.window("same-window plots")
        .size([400.0, 400.0], dear_imgui_rs::Condition::Always)
        .build(|| {
            outer
                .with_plot_size("outer style", [200.0, 140.0], |plot| {
                    let colors = [0xff00ffff; 2];
                    let style = crate::PlotItemArrayStyle::new().with_line_colors(&colors);
                    // SAFETY: the sole styled item has two vertices and a packed color for each one.
                    unsafe {
                        plot.with_next_plot_item_array_style(style, |plot| {
                            b.get_plot_ui(ui)
                                .with_plot_size("inner", [120.0, 90.0], |inner| {
                                    assert!(inner.next_spec.get().is_none());
                                    // Even in the same window, only the inner extension is current.
                                    assert!(
                                        catch_unwind(AssertUnwindSafe(|| plot.plot_pos())).is_err()
                                    );
                                    inner.plot_line("plain", &[0.0, 1.0], &[1.0, 2.0]);
                                })
                                .unwrap();
                            assert_eq!(
                                plot.next_spec.get().unwrap().LineColors.cast_const(),
                                colors.as_ptr()
                            );
                            plot.plot_line("colored", &[0.0, 1.0], &[1.0, 2.0]);
                            assert!(plot.next_spec.get().is_none());
                        })
                    };
                })
                .unwrap();
        })
        .unwrap();
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
#[should_panic(expected = "PlotSetup::setup_axis_zoom_constraints() min must be positive")]
fn axis_zoom_constraints_reject_non_positive_min_before_ffi() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let plot = PlotContext::create(&imgui);

    {
        let frame = imgui.begin_frame();
        let plot_ui = plot.get_plot_ui(frame.ui());
        plot_ui
            .with_plot_setup(
                "constraints",
                [-1.0, 0.0],
                |setup| {
                    setup.setup_axis_zoom_constraints(Axis::Y1, 0.0, 10.0);
                },
                |_| (),
            )
            .expect("plot must begin");
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

        plot_ui
            .with_plot_setup(
                "typed-axis",
                [-1.0, 0.0],
                |setup| {
                    setup.setup_x_axis(XAxis::X1, None, crate::AxisFlags::NONE);
                    setup.setup_y_axis(YAxis::Y1, None, crate::AxisFlags::NONE);
                    // SAFETY: Y1 is enabled and setup is unlocked; storage outlives PlotContext.
                    unsafe { setup.setup_axis_links(Axis::Y1, Some(&mut min), Some(&mut max)) };
                    setup.setup_y_axis_limits(YAxis::Y1, 2.0, 5.0, PlotCond::Always);
                    setup.setup_axis_limits_constraints(Axis::Y1, -10.0, 10.0);
                    setup.setup_axis_zoom_constraints(Axis::Y1, 0.1, 20.0);
                },
                |_| (),
            )
            .expect("plot must begin");
    }
    drop(plot);
    assert_eq!((x_min, x_max), (-2.0, 2.0));
    assert_eq!((min, max), (2.0, 5.0));
}

#[test]
fn plot_end_writes_back_and_detaches_secondary_axis_links() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let owner = PlotContext::create(&imgui);
    let mut min = 0.0;
    let mut max = 1.0;
    let frame = imgui.begin_frame();
    let plot_ui = owner.get_plot_ui(frame.ui());
    let raw = plot_ui
        .with_plot_setup(
            "secondary-links",
            [-1.0, 0.0],
            |setup| {
                setup.setup_y_axis(YAxis::Y2, None, crate::AxisFlags::NONE);
                // SAFETY: distinct, stable storage remains exclusively available until the plot ends.
                unsafe { setup.setup_axis_links(Axis::Y2, Some(&mut min), Some(&mut max)) };
                setup.setup_y_axis_limits(YAxis::Y2, 3.0, 7.0, PlotCond::Always);
            },
            |plot| plot.with_bound_context(|| unsafe { sys::ImPlot_GetCurrentPlot() }),
        )
        .unwrap();
    assert_eq!((min, max), (3.0, 7.0));
    // The owner is alive and no subsequent plot has grown its pool.
    let axis = unsafe { &(*raw).Axes[YAxis::Y2 as usize] };
    assert!(axis.LinkedMin.is_null() && axis.LinkedMax.is_null());
}

#[test]
fn plot_and_axis_titles_preserve_literal_percent_signs() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let owner = PlotContext::create(&imgui);
    let frame = imgui.begin_frame();
    let plots = owner.get_plot_ui(frame.ui());
    plots
        .with_plot_setup(
            "Capacity at 75%",
            [240.0, 160.0],
            |setup| {
                let x = String::from("Load (%)");
                let y = String::from("Yield (%)");
                setup.setup_axes(
                    Some(&x),
                    Some(&y),
                    crate::AxisFlags::NONE,
                    crate::AxisFlags::NONE,
                );
            },
            |plot| {
                plot.with_bound_context(|| unsafe {
                    // All three offsets refer to the plot's own text buffer after the setup strings drop.
                    let native = &*sys::ImPlot_GetCurrentPlot();
                    let buf = &native.TextBuffer.Buf;
                    let text = std::slice::from_raw_parts(buf.Data.cast::<u8>(), buf.Size as usize);
                    for (offset, expected) in [
                        (native.TitleOffset, "Capacity at 75%"),
                        (native.Axes[XAxis::X1 as usize].LabelOffset, "Load (%)"),
                        (native.Axes[YAxis::Y1 as usize].LabelOffset, "Yield (%)"),
                    ] {
                        let offset = usize::try_from(offset).expect("visible label");
                        let label = std::ffi::CStr::from_bytes_until_nul(&text[offset..]).unwrap();
                        assert_eq!(label.to_bytes(), expected.as_bytes());
                    }
                })
            },
        )
        .expect("visible plot");
}

#[test]
fn phases_balance_host_tokens_and_native_ids_on_return_and_unwind() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let owner = PlotContext::create(&imgui);

    for panic_in in [None, Some("setup"), Some("drawing")] {
        let frame = imgui.begin_frame();
        let ui = frame.ui();
        let plots = owner.get_plot_ui(ui);
        let depths = || {
            ui.with_bound_context(|| unsafe {
                let window = &*dear_imgui_rs::sys::igGetCurrentWindowRead();
                (window.IDStack.Size, (*window.DrawList)._ClipRectStack.Size)
            })
        };
        let (ids, clips) = depths();
        let mut setup_id = None;
        let result = catch_unwind(AssertUnwindSafe(|| {
            plots
                .with_plot_setup(
                    "phases",
                    [220.0, 180.0],
                    |_| {
                        setup_id = Some(ui.push_id("setup id"));
                        assert_eq!(depths(), (ids + 1, clips));
                        if panic_in == Some("setup") {
                            panic!("leave the setup closure");
                        }
                    },
                    |plot| {
                        // The setup token has already been balanced. Only ImPlot's item ID is active.
                        assert_eq!(depths(), (ids + 1, clips));
                        assert!(
                            catch_unwind(AssertUnwindSafe(|| plots.with_plot("nested", |_| ())))
                                .is_err()
                        );
                        assert!(
                            catch_unwind(
                                AssertUnwindSafe(|| plots.show_metrics_window(&mut false))
                            )
                            .is_err()
                        );
                        assert!(
                            catch_unwind(AssertUnwindSafe(|| plots.show_demo_window(&mut false)))
                                .is_err()
                        );
                        plot.with_plot_clip_rect(0.0, || {
                            assert_eq!(depths(), (ids + 1, clips + 1));
                            std::mem::forget(ui.push_id("nested id"));
                            if panic_in == Some("drawing") {
                                panic!("leave the drawing closure");
                            }
                        });
                        assert_eq!(depths(), (ids + 1, clips));
                    },
                )
                .expect("visible plot");
        }));
        assert_eq!(result.is_err(), panic_in.is_some());
        assert_eq!(depths(), (ids, clips));
        drop(setup_id);
        assert_eq!(depths(), (ids, clips));
        assert!(unsafe { (*owner.raw()).CurrentPlot.is_null() });
    }
}

#[test]
fn clipped_plot_skips_both_phases_and_leaves_the_context_ready() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let owner = PlotContext::create(&imgui);
    let frame = imgui.begin_frame();
    let ui = frame.ui();
    ui.window("clipped plot host")
        .size([320.0, 320.0], dear_imgui_rs::Condition::Always)
        .build(|| {
            let plots = owner.get_plot_ui(ui);
            let clip = ui.push_clip_rect([0.0; 2], [0.0; 2], true);
            let skipped: Option<()> = plots.with_plot_setup(
                "clipped",
                [160.0, 60.0],
                |_| panic!("clipped setup"),
                |_| panic!("clipped draw"),
            );
            assert!(skipped.is_none());
            drop(clip);
            assert!(unsafe { (*owner.raw()).CurrentPlot.is_null() });
            assert_eq!(
                plots.with_plot_size("visible", [160.0, 60.0], |_| 7),
                Some(7)
            );
        })
        .unwrap();
}

#[test]
fn implicit_axes_follow_the_current_pair_and_secondary_axes_require_setup() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let owner = PlotContext::create(&imgui);
    let frame = imgui.begin_frame();
    let plots = owner.get_plot_ui(frame.ui());
    plots.set_next_x_axis_limits(XAxis::X2, 10.0, 20.0, PlotCond::Always);
    plots
        .with_plot_setup(
            "axes",
            [240.0, 180.0],
            |setup| {
                assert!(
                    catch_unwind(AssertUnwindSafe(|| setup.setup_y_axis_limits(
                        YAxis::Y2,
                        2.0,
                        4.0,
                        PlotCond::Always
                    )))
                    .is_err()
                );
                setup.setup_x_axis(XAxis::X2, None, crate::AxisFlags::NONE);
                setup.setup_y_axis(YAxis::Y2, None, crate::AxisFlags::NONE);
                setup.setup_x_axis_scale(XAxis::X2, crate::AxisScale::Log10);
                setup.setup_y_axis_scale(YAxis::Y2, crate::AxisScale::SymLog);
                setup.setup_y_axis_limits(YAxis::Y2, 30.0, 40.0, PlotCond::Always);
            },
            |plot| {
                plot.set_axes(XAxis::X2, YAxis::Y2);
                let limits = plot.plot_limits(None, None);
                assert_eq!(
                    (limits.X.Min, limits.X.Max, limits.Y.Min, limits.Y.Max),
                    (10.0, 20.0, 30.0, 40.0)
                );
                let point = sys::ImPlotPoint { x: 17.0, y: 36.0 };
                let pixels = plot.plot_to_pixels(point, None);
                assert_eq!(
                    pixels,
                    plot.plot_to_pixels_axes(point, XAxis::X2, YAxis::Y2)
                );
                let recovered = plot.pixels_to_plot(pixels, None);
                assert!(
                    (recovered.x - point.x).abs() < 0.001 && (recovered.y - point.y).abs() < 0.001
                );
                assert!(
                    catch_unwind(AssertUnwindSafe(|| plot.set_axes(XAxis::X3, YAxis::Y1))).is_err()
                );
            },
        )
        .expect("visible plot");
}

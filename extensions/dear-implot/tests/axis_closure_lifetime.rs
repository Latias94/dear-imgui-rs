use dear_imgui_rs::Context;
use dear_implot::{AxisFlags, PlotContext, PlotError, XAxis, YAxisConfig};
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
fn plot_owns_callbacks_and_native_copies_tick_labels() {
    use dear_implot::YAxis;
    use dear_implot_sys as sys;
    use std::{cell::Cell, ffi::CStr, rc::Rc};

    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let outer = PlotContext::create(&imgui);
    let inner = PlotContext::create(&imgui);
    let frame = imgui.begin_frame();
    let ui = frame.ui();
    let plot_ui = outer.get_plot_ui(ui);
    let calls = Rc::new(Cell::new(0));
    let owned = calls.clone();
    let raw = plot_ui
        .with_plot_setup(
            "outer",
            [240.0, 180.0],
            |setup| {
                // SAFETY: callbacks only read the coordinate and update a Rust counter.
                unsafe {
                    setup.setup_x_axis_format_closure(XAxis::X1, move |_| {
                        owned.set(owned.get() + 1);
                        "outer tick".to_owned()
                    });
                    setup.setup_x_axis_transform_closure(XAxis::X1, |v| v, |v| v);
                }
                setup.setup_x_axis_ticks_positions(XAxis::X1, &[0.0], None, false);
                let text = String::from("copied label");
                setup.setup_y_axis_ticks_positions(
                    YAxis::Y1,
                    &[0.0],
                    Some(&[text.as_str()]),
                    false,
                );
            },
            |plot| {
                assert!(calls.get() >= 1);
                ui.window("nested plot host")
                    .size([220.0, 160.0], dear_imgui_rs::Condition::Always)
                    .build(|| {
                        inner
                            .get_plot_ui(ui)
                            .with_plot_size("inner", [100.0, 80.0], |child| {
                                child.plot_line("inner series", &[0.0, 1.0], &[1.0, 0.0]);
                                assert!(
                                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                                        || plot.plot_pos()
                                    ))
                                    .is_err()
                                );
                            })
                            .unwrap();
                    })
                    .unwrap();
                plot.plot_line("outer series", &[0.0, 1.0], &[0.0, 1.0]);
                // The owner is alive and its pool cannot grow until this plot ends.
                let raw = unsafe { (*outer.raw()).CurrentPlot };
                for (axis, expected) in [
                    (XAxis::X1 as usize, "outer tick"),
                    (YAxis::Y1 as usize, "copied label"),
                ] {
                    let ticker = unsafe { &mut (*raw).Axes[axis].Ticker };
                    assert_eq!(unsafe { sys::ImPlotTicker_TickCount(ticker) }, 1);
                    let text = unsafe { CStr::from_ptr(sys::ImPlotTicker_GetText_Int(ticker, 0)) };
                    assert_eq!(text.to_str().unwrap(), expected);
                }
                raw
            },
        )
        .unwrap();
    assert_eq!(Rc::strong_count(&calls), 1);
    // No subsequent plot has relocated the pool. The finished plot holds no Rust callback.
    let axis = unsafe { &(*raw).Axes[XAxis::X1 as usize] };
    assert!(axis.Formatter.is_none() && axis.FormatterData.is_null());
    assert!(
        axis.TransformForward.is_none()
            && axis.TransformInverse.is_none()
            && axis.TransformData.is_null()
    );
}

#[test]
fn multi_axis_plot_rejects_interior_nul_labels() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let plot_ctx = PlotContext::create(&imgui);

    let frame = imgui.begin_frame();
    let plot_ui = plot_ctx.get_plot_ui(frame.ui());

    let plot = dear_implot::MultiAxisPlot::new("t").add_y_axis(YAxisConfig {
        label: Some("a\0b"),
        flags: AxisFlags::NONE,
        range: None,
    });

    match plot.build(&plot_ui, |_| ()) {
        Ok(_) => panic!("expected interior NUL label to be rejected"),
        Err(err) => assert!(matches!(err, PlotError::StringConversion(_))),
    }
}

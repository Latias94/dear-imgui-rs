use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::{Mutex, OnceLock};

use dear_imgui_rs::{Condition, Context};
use dear_implot3d::{Axis3D, Axis3DFlags, Colormap, Plot3DContext};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn assert_rejected<R>(expected: &str, f: impl FnOnce() -> R) {
    let reason = catch_unwind(AssertUnwindSafe(f))
        .err()
        .expect("operation succeeded");
    let message = reason
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| reason.downcast_ref::<&str>().copied())
        .expect("expected a Rust diagnostic");
    assert!(
        message.contains(expected),
        "unexpected rejection: {message}"
    );
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
            plot_ui.begin_plot("bound plot").build(|plot| {
                assert!(plot.get_frame_size()[0] > 0.0);
                panic!("binding panic probe");
            });
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
fn dead_imgui_context_rejects_calls_and_destruction_preserves_the_current_plot_context() {
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
fn tick_counts_are_checked_and_temporary_labels_are_consumed_during_setup() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let context = Plot3DContext::create(&imgui);
    let frame = imgui.begin_frame();
    context
        .get_plot_ui(frame.ui())
        .begin_plot("ticks")
        .build_with_setup(
            |setup| {
                for (count, labels) in [
                    (0, None),
                    (0, Some([].as_slice())),
                    (1, Some(["single"].as_slice())),
                ] {
                    assert_rejected("n_ticks", || {
                        setup.setup_axis_ticks_range(Axis3D::X, 0.0, 1.0, count, labels, false)
                    });
                }
                let labels = [String::from("min"), String::from("max")];
                let refs = labels.each_ref().map(String::as_str);
                setup.setup_axis(Axis3D::X, &String::from("length"), Axis3DFlags::NONE);
                setup.setup_axis_ticks_range(Axis3D::X, 0.0, 1.0, 2, Some(&refs), false);
                setup.setup_axis_ticks_values(Axis3D::Y, &[0.0, 1.0], Some(&refs), false);
                setup.setup_axis_ticks_range(Axis3D::Z, 0.0, 1.0, 1, None, true);
            },
            |plot| assert!(plot.get_frame_size()[0] > 0.0),
        )
        .unwrap();
}

#[test]
fn axis_menu_renders_percent_labels_verbatim_and_uses_the_default_axis_name() {
    use dear_imgui_rs::sys;
    use std::ffi::CStr;

    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let context = Plot3DContext::create(&imgui);
    let flags = Axis3DFlags::NONE;
    for (popup, expected) in [
        ("##XAxisContext", "CPU (%)"),
        ("##YAxisContext", "90% busy / 10% idle"),
        ("##ZAxisContext", "Z-Axis"),
    ] {
        let frame = imgui.begin_frame();
        let ui = frame.ui();
        let menu_text = ui
            .window("axis menus")
            .position([20.0, 20.0], Condition::Always)
            .size([640.0, 480.0], Condition::Always)
            .build(|| {
                context
                    .get_plot_ui(ui)
                    .begin_plot("usage")
                    .build_with_setup(
                        |setup| {
                            setup.setup_axes("CPU (%)", "", "", flags, flags, flags);
                            setup.setup_axis(Axis3D::Y, "90% busy / 10% idle", flags);
                        },
                        |_| {
                            ui.open_popup(popup);
                            // Start after the plot is drawn. EndPlot now emits the actual popup heading.
                            ui.with_bound_context(|| unsafe { sys::igLogToBuffer(0) });
                        },
                    )
                    .expect("visible plot");
                let text = ui.with_bound_context(|| unsafe {
                    // The current context owns this NUL-terminated buffer until LogFinish.
                    let buffer = &raw mut (*sys::igGetCurrentContext()).LogBuffer;
                    CStr::from_ptr(sys::ImGuiTextBuffer_c_str(buffer)).to_owned()
                });
                ui.log_finish();
                text
            })
            .expect("visible window");
        let text = menu_text.to_str().unwrap();
        assert!(
            text.lines().any(|line| line.trim() == expected),
            "{popup}: {text:?}"
        );
    }
}

#[test]
fn plot_restores_its_host_before_locking_setup_and_on_unwind() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let context = Plot3DContext::create(&imgui);
    for stop_in_setup in [true, false] {
        let frame = imgui.begin_frame();
        let ui = frame.ui();
        let plot_ui = context.get_plot_ui(ui);
        ui.window("plot host")
            .size([640.0, 480.0], Condition::Always)
            .build(|| {
                let clip_depth = || {
                    ui.with_bound_context(|| unsafe {
                        (*dear_imgui_rs::sys::igGetWindowDrawList())
                            ._ClipRectStack
                            .Size
                    })
                };
                let before = clip_depth();
                let outer_clip = ui.push_clip_rect([0.0; 2], [800.0, 600.0], true);
                let mut late_tooltip = None;
                let result = catch_unwind(AssertUnwindSafe(|| {
                    plot_ui
                        .begin_plot("unwind")
                        .size([200.0, 100.0])
                        .build_with_setup(
                            |_| {
                                drop(outer_clip);
                                assert_eq!(clip_depth(), before + 2);
                                std::mem::forget(ui.push_clip_rect([0.0; 2], [800.0, 600.0], true));
                                if stop_in_setup {
                                    panic!("plot callback");
                                }
                            },
                            |plot| {
                                assert_eq!(clip_depth(), before + 3);
                                ui.with_native_window_scope(|_| {
                                    assert_rejected("host window", || plot.get_frame_size());
                                });
                                late_tooltip = ui.begin_tooltip();
                                assert!(late_tooltip.is_some());
                                assert_rejected("host window", || plot.get_frame_size());
                                panic!("plot callback");
                            },
                        );
                }));
                assert_eq!(
                    result.unwrap_err().downcast_ref::<&str>(),
                    Some(&"plot callback")
                );
                assert_eq!(clip_depth(), before);
                drop(late_tooltip);
                assert_eq!(clip_depth(), before);
                assert_eq!(
                    plot_ui
                        .begin_plot("next")
                        .size([200.0, 100.0])
                        .build(|_| 17),
                    Some(17)
                );
                let _offscreen = ui.push_clip_rect([-50.0; 2], [-40.0; 2], false);
                assert_eq!(
                    plot_ui
                        .begin_plot("clipped")
                        .build_with_setup(|_| panic!("unexpected setup"), |_| 1),
                    None
                );
            })
            .unwrap();
    }
}

#[test]
fn nested_contexts_draw_only_on_their_own_host_and_colormaps_restore_on_unwind() {
    let _guard = test_guard();
    let mut imgui = Context::create();
    prepare_imgui(&mut imgui);
    let first = Plot3DContext::create(&imgui);
    let second = Plot3DContext::create(&imgui);
    let frame = imgui.begin_frame();
    let ui = frame.ui();
    let a = first.get_plot_ui(ui);
    let b = second.get_plot_ui(ui);
    let initial = first.style_colormap_index();
    assert_eq!(first.colormap_size(first.colormap_count()), 0);
    assert!(
        a.with_colormap_name("unregistered colormap", || unreachable!())
            .is_none()
    );
    assert_rejected("unregistered ImPlot3D colormap", || {
        a.with_colormap(first.colormap_count(), || ())
    });
    let result = catch_unwind(AssertUnwindSafe(|| {
        a.with_colormap(Colormap::Hot, || {
            assert_eq!(first.style_colormap_index(), Some(Colormap::Hot.index()));
            a.begin_plot("outer")
                .build(|outer| {
                    assert_rejected("no open plot", || a.begin_plot("nested").build(|_| ()));
                    assert_rejected("no open plot", || a.show_metrics_window());
                    ui.window("second context")
                        .size([300.0, 200.0], Condition::Always)
                        .build(|| {
                            b.begin_plot("inner")
                                .build(|inner| {
                                    assert!(inner.get_frame_size()[0] > 0.0);
                                    assert_rejected("host window", || outer.next_colormap_color());
                                })
                                .unwrap();
                        })
                        .unwrap();
                    assert!(outer.next_colormap_color().iter().all(|v| v.is_finite()));
                })
                .unwrap();
            panic!("restore colormap");
        })
    }));
    assert_eq!(
        result.unwrap_err().downcast_ref::<&str>(),
        Some(&"restore colormap")
    );
    assert_eq!(first.style_colormap_index(), initial);
}

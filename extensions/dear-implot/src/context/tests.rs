use super::{PlotContext, validation::axis_tick_count_to_i32};
use crate::sys;
use crate::{Axis, PlotCond, XAxis, YAxis};
use dear_imgui_rs::{BackendFlags, Context};
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
    io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_TEXTURES);
}

#[test]
fn axis_ticks_range_count_is_checked_before_ffi() {
    assert_eq!(axis_tick_count_to_i32("test", 1), 1);
    assert_eq!(axis_tick_count_to_i32("test", i32::MAX as usize), i32::MAX);

    assert!(
        std::panic::catch_unwind(|| axis_tick_count_to_i32("test", 0)).is_err(),
        "zero tick counts must not cross the safe API boundary"
    );
    assert!(
        std::panic::catch_unwind(|| {
            axis_tick_count_to_i32("test", i32::MAX as usize + 1);
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
        let ui = imgui.frame();
        let plot_ui = plot_a.get_plot_ui(&ui);
        unsafe { sys::ImPlot_SetCurrentContext(raw_b) };

        {
            let _guard = plot_ui.bind();
            assert_eq!(unsafe { sys::ImPlot_GetCurrentContext() }, raw_a);
        }
        assert_eq!(unsafe { sys::ImPlot_GetCurrentContext() }, raw_b);

        plot_ui.set_next_axes_to_fit();

        assert_eq!(unsafe { sys::ImPlot_GetCurrentContext() }, raw_b);
    }
    let _ = imgui.render();

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
        let ui = imgui.frame();
        let plot_ui = plot_a.get_plot_ui(&ui);
        let token = plot_ui.begin_plot("token").expect("failed to begin plot");

        unsafe { sys::ImPlot_SetCurrentContext(raw_b) };
        drop(token);

        assert_eq!(unsafe { sys::ImPlot_GetCurrentContext() }, raw_b);
    }
    let _ = imgui.render();

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
        let ui = imgui.frame();
        let plot_ui = plot_a.get_plot_ui(&ui);
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
    let _ = imgui.render();

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
        let ui = imgui.frame();
        let plot_ui = plot.get_plot_ui(&ui);
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
        let ui = imgui.frame();
        let plot_ui = plot.get_plot_ui(&ui);
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
    let plot = PlotContext::create(&imgui);

    {
        let ui = imgui.frame();
        let plot_ui = plot.get_plot_ui(&ui);
        plot_ui.set_next_axis_to_fit(Axis::X1);

        let token = plot_ui
            .begin_plot("typed-axis")
            .expect("failed to begin plot");
        plot_ui.setup_x_axis(XAxis::X1, None, crate::AxisFlags::NONE);
        plot_ui.setup_y_axis(YAxis::Y1, None, crate::AxisFlags::NONE);
        let mut min = 0.0;
        let mut max = 1.0;
        plot_ui.setup_axis_links(Axis::Y1, Some(&mut min), Some(&mut max));
        plot_ui.setup_axis_limits_constraints(Axis::Y1, -10.0, 10.0);
        plot_ui.setup_axis_zoom_constraints(Axis::Y1, 0.1, 20.0);
        token.end();
    }

    let _ = imgui.render();
    drop(plot);
}

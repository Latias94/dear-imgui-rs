use super::callbacks::AxisCallbacks;
use super::{PlotContext, PlotUi};
use crate::sys;
use dear_imgui_rs::{DrawListMut, NativeWindowScope, Ui};

/// Drawing and queries for one plot after its axes have been configured.
///
/// ```compile_fail
/// use dear_implot::{AxisFlags, PlotScope, XAxis};
/// fn configure_during_drawing(plot: &PlotScope<'_>) {
///     plot.setup_x_axis(XAxis::X1, None, AxisFlags::NONE);
/// }
/// ```
pub struct PlotScope<'plot> {
    pub(crate) ui: &'plot Ui,
    pub(crate) host: &'plot NativeWindowScope<'plot>,
    pub(crate) context: &'plot PlotContext,
    raw: *mut sys::ImPlotPlot,
    pub(crate) next_spec: std::cell::Cell<Option<sys::ImPlotSpec_c>>,
}

/// Axis and legend configuration for the plot's setup closure.
///
/// ```compile_fail
/// use dear_implot::PlotSetup;
/// fn submit_during_setup(setup: &PlotSetup<'_>) {
///     setup.plot_line("line", &[0.0, 1.0], &[2.0, 3.0]);
/// }
/// ```
pub struct PlotSetup<'plot> {
    pub(crate) plot: &'plot PlotScope<'plot>,
    pub(super) callbacks: &'plot mut AxisCallbacks,
}

pub(super) struct ActivePlot<'plot> {
    pub(super) plot: PlotScope<'plot>,
    callbacks: AxisCallbacks,
}

impl<'plot> ActivePlot<'plot> {
    pub(super) fn new(
        ui: &PlotUi<'plot>,
        host: &'plot NativeWindowScope<'plot>,
        raw: *mut sys::ImPlotPlot,
    ) -> Self {
        Self {
            plot: PlotScope {
                ui: ui.ui,
                context: ui.context,
                host,
                raw,
                next_spec: Default::default(),
            },
            callbacks: AxisCallbacks::default(),
        }
    }

    pub(super) fn setup(&mut self) -> PlotSetup<'_> {
        PlotSetup {
            plot: &self.plot,
            callbacks: &mut self.callbacks,
        }
    }
}

impl Drop for ActivePlot<'_> {
    fn drop(&mut self) {
        self.plot.with_bound_context(|| unsafe {
            // EndPlot also finishes setup when the setup closure unwinds. Keep all registered
            // callbacks alive until it returns, then detach them from the cached native axes.
            sys::ImPlot_EndPlot();
            self.callbacks.detach(self.plot.raw);
            // Native Axis::Reset leaves these pointers in cached, possibly disabled axes.
            // Writeback is complete. A later use of this plot ID can establish fresh links.
            for axis in &mut (*self.plot.raw).Axes {
                axis.LinkedMin = std::ptr::null_mut();
                axis.LinkedMax = std::ptr::null_mut();
            }
        });
    }
}

impl PlotScope<'_> {
    pub(crate) fn with_bound_context<R>(&self, f: impl FnOnce() -> R) -> R {
        self.context.with_ui(self.ui, || {
            assert!(
                self.host.is_current(),
                "plot requires its original window and table cell"
            );
            assert_eq!(
                unsafe { sys::ImPlot_GetCurrentPlot() },
                self.raw,
                "plot is no longer current"
            );
            f()
        })
    }

    pub(crate) fn with_axes<R>(&self, axes: &[sys::ImAxis], f: impl FnOnce() -> R) -> R {
        self.with_bound_context(|| {
            for &axis in axes {
                if axis != crate::IMPLOT_AUTO {
                    assert!(
                        unsafe { (*self.raw).Axes[axis as usize].Enabled },
                        "enable the requested axis during plot setup"
                    );
                }
            }
            f()
        })
    }

    /// Returns the window draw list that ImPlot uses for this plot.
    pub fn plot_draw_list(&self) -> DrawListMut<'_> {
        self.with_bound_context(|| self.ui.get_window_draw_list())
    }

    /// Draw with the plot clip rectangle. Nested Dear ImGui scopes close before the clip is popped.
    pub fn with_plot_clip_rect<R>(&self, expand: f32, draw: impl FnOnce() -> R) -> R {
        assert!(expand.is_finite(), "plot clip expansion must be finite");
        self.host.run(|| {
            self.with_bound_context(|| unsafe { sys::ImPlot_PushPlotClipRect(expand) });
            with_cleanup(
                || self.with_bound_context(|| unsafe { sys::ImPlot_PopPlotClipRect() }),
                || self.host.run(draw),
            )
        })
    }
}

pub(crate) fn with_cleanup<R>(end: impl FnOnce(), body: impl FnOnce() -> R) -> R {
    struct Guard<F: FnOnce()>(Option<F>);
    impl<F: FnOnce()> Drop for Guard<F> {
        fn drop(&mut self) {
            if let Some(end) = self.0.take() {
                end();
            }
        }
    }
    let _guard = Guard(Some(end));
    body()
}

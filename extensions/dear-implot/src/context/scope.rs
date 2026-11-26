use super::callbacks::AxisCallbacks;
use super::validation::is_valid_axis_range;
use super::{PlotContext, PlotUi};
use crate::{Axis, sys};
use dear_imgui_rs::{DrawListMut, NativeWindowScope, Ui};
use std::cell::Cell;

pub(super) type AxisLink<'links> = (Option<&'links Cell<f64>>, Option<&'links Cell<f64>>);

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
/// fn submit_during_setup(setup: &PlotSetup<'_, '_>) {
///     setup.plot_line("line", &[0.0, 1.0], &[2.0, 3.0]);
/// }
/// ```
pub struct PlotSetup<'plot, 'links> {
    pub(crate) plot: &'plot PlotScope<'plot>,
    pub(super) callbacks: &'plot mut AxisCallbacks,
    pub(super) axis_links: &'plot mut [AxisLink<'links>; Axis::Y3 as usize + 1],
}

pub(super) struct ActivePlot<'plot, 'links> {
    pub(super) plot: PlotScope<'plot>,
    callbacks: AxisCallbacks,
    axis_links: [AxisLink<'links>; Axis::Y3 as usize + 1],
}

impl<'plot, 'links> ActivePlot<'plot, 'links> {
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
            axis_links: Default::default(),
        }
    }

    pub(super) fn setup(&mut self) -> PlotSetup<'_, 'links> {
        PlotSetup {
            plot: &self.plot,
            callbacks: &mut self.callbacks,
            axis_links: &mut self.axis_links,
        }
    }
}

impl Drop for ActivePlot<'_, '_> {
    fn drop(&mut self) {
        self.plot.with_bound_context(|| unsafe {
            // EndPlot also finishes setup when the setup closure unwinds. Keep all registered
            // callbacks alive until it returns, then detach them from the cached native axes.
            sys::ImPlot_EndPlot();
            self.callbacks.detach(self.plot.raw);
            // EndPlot has finished writing to native subplot links, but cached axes can
            // retain those pointers. Rust Cell addresses were never passed to ImPlot.
            let axes = &mut (*self.plot.raw).Axes;
            for (axis, &(link_min, link_max)) in axes.iter_mut().zip(&self.axis_links) {
                axis.LinkedMin = std::ptr::null_mut();
                axis.LinkedMax = std::ptr::null_mut();
                if link_min.is_none() && link_max.is_none() {
                    continue;
                }
                let range = axis.Range;
                if is_valid_axis_range(range.Min, range.Max) {
                    if let Some(min) = link_min {
                        min.set(range.Min);
                    }
                    if let Some(max) = link_max {
                        max.set(range.Max);
                    }
                }
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

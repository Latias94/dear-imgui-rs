use crate::Plot3DUi;
use crate::layout::NextItemStyle;
use dear_imgui_rs::{DrawListMut, NativeWindowScope};

/// Axis and box configuration, borrowed before the plot locks setup.
///
/// ```compile_fail
/// use dear_implot3d::{Line3DFlags, Plot3DSetup};
/// fn draw_during_setup(setup: &Plot3DSetup<'_>) {
///     setup.plot_line_f32("line", &[0.0; 2], &[0.0; 2], &[0.0; 2], Line3DFlags::NONE);
/// }
/// ```
pub struct Plot3DSetup<'plot> {
    pub(crate) plot: &'plot Plot3DScope<'plot>,
}

impl Plot3DSetup<'_> {
    pub(crate) fn with_bound_context<R>(&self, f: impl FnOnce() -> R) -> R {
        self.plot.with_bound_context(f)
    }
}

/// A drawing interface borrowed from an open plot after setup has finished.
///
/// ```compile_fail
/// use dear_implot3d::Plot3DScope;
/// fn reconfigure(plot: &Plot3DScope<'_>) { plot.setup_box_scale(1.0, 1.0, 1.0); }
/// ```
///
/// ```compile_fail
/// use dear_implot3d::{Plot3DScope, Plot3DUi};
/// fn escape<'a>(ui: &Plot3DUi<'a>) -> &'a Plot3DScope<'a> {
///     ui.begin_plot("plot").build(|plot| plot).unwrap()
/// }
/// ```
pub struct Plot3DScope<'plot> {
    pub(crate) ui: Plot3DUi<'plot>,
    pub(crate) host: &'plot NativeWindowScope<'plot>,
    pub(crate) next_item: NextItemStyle,
}

impl Plot3DScope<'_> {
    pub(crate) fn with_bound_context<R>(&self, f: impl FnOnce() -> R) -> R {
        assert!(
            self.host.is_current(),
            "ImPlot3D requires the plot's host window and table cell"
        );
        self.ui.with_bound_context(f)
    }

    /// Borrows the draw list of the window hosting this plot.
    pub fn plot_draw_list(&self) -> DrawListMut<'_> {
        // Upstream GetPlotDrawList returns exactly ImGui::GetWindowDrawList(). The core wrapper
        // also remembers the window Begin instance for subsequent draw-list operations.
        self.with_bound_context(|| self.ui.imgui.get_window_draw_list())
    }
}

use crate::{Plot3DContext, Plot3DFlags, Plot3DScope, Plot3DSetup, Plot3DUi, imvec2, sys};

/// Configures a 3D plot in the current ImGui window and table cell.
pub struct Plot3DBuilder<'ui> {
    pub(crate) ui: Plot3DUi<'ui>,
    pub(crate) title: String,
    pub(crate) size: [f32; 2],
    pub(crate) flags: Plot3DFlags,
}

impl Plot3DBuilder<'_> {
    pub fn size(mut self, size: [f32; 2]) -> Self {
        self.size = size;
        self
    }

    pub fn flags(mut self, flags: Plot3DFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Draws a plot with the default setup. Returns `None` if skipped or the title is invalid.
    pub fn build<R>(self, draw: impl FnOnce(&Plot3DScope<'_>) -> R) -> Option<R> {
        self.build_with_setup(|_| {}, draw)
    }

    /// Configures the axes, finishes native setup, and draws the plot. A skipped plot or an empty
    /// or NUL-containing title runs neither closure. Both closures can use other ImGui windows
    /// and must return to the host window and table cell to operate on this plot.
    ///
    /// # Panics
    ///
    /// Starting a second plot on this ImPlot3D context while one is open panics before BeginPlot.
    pub fn build_with_setup<R>(
        self,
        setup: impl FnOnce(&Plot3DSetup<'_>),
        draw: impl FnOnce(&Plot3DScope<'_>) -> R,
    ) -> Option<R> {
        if self.title.is_empty() || self.title.contains('\0') {
            return None;
        }
        let ui = self.ui;
        let _exclusive = ui.context.enter_plot();
        ui.imgui.with_native_window_scope(|host| {
            let began = ui.with_bound_context(|| {
                dear_imgui_rs::with_scratch_txt(&self.title, |title| unsafe {
                    sys::ImPlot3D_BeginPlot(
                        title,
                        imvec2(self.size[0], self.size[1]),
                        self.flags.bits() as i32,
                    )
                })
            });
            if !began {
                return None;
            }
            let _end = EndPlot(ui.context);
            let plot = Plot3DScope {
                ui,
                host,
                next_item: Default::default(),
            };
            host.run(|| {
                setup(&Plot3DSetup {
                    plot: &plot,
                    axes: Default::default(),
                });
            });

            // GetPlotRectPos calls SetupLock, which pushes a second native clip rect. Install it
            // after all setup-closure scopes have closed and before application drawing begins.
            ui.with_bound_context(|| unsafe { sys::ImPlot3D_GetPlotRectPos() });
            Some(host.run(|| draw(&plot)))
        })
    }
}

struct EndPlot<'a>(&'a Plot3DContext);

impl Drop for EndPlot<'_> {
    fn drop(&mut self) {
        self.0
            .with_bound_context(|| unsafe { sys::ImPlot3D_EndPlot() });
    }
}

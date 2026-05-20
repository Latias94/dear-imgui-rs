use crate::{PlotContext, PlotUi};
use dear_imgui_rs::Ui;

/// Ui extension for obtaining a PlotUi from an ImPlot PlotContext
pub trait ImPlotExt {
    fn implot<'ui>(&'ui self, ctx: &'ui PlotContext) -> PlotUi<'ui>;
}

impl ImPlotExt for Ui {
    fn implot<'ui>(&'ui self, ctx: &'ui PlotContext) -> PlotUi<'ui> {
        ctx.get_plot_ui(self)
    }
}

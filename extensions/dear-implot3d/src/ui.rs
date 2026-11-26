mod binding;
mod core;
#[cfg(feature = "mint")]
mod mint;
mod scope;

pub use core::Plot3DUi;
pub use scope::{Plot3DScope, Plot3DSetup};

/// Extension methods for using ImPlot3D from an ImGui frame.
pub trait ImPlot3DExt {
    /// Borrows the plot context and the frame. The frame must belong to the owning ImGui context.
    fn implot3d<'ui>(&'ui self, context: &'ui crate::Plot3DContext) -> Plot3DUi<'ui>;
}

impl ImPlot3DExt for dear_imgui_rs::Ui {
    fn implot3d<'ui>(&'ui self, context: &'ui crate::Plot3DContext) -> Plot3DUi<'ui> {
        context.get_plot_ui(self)
    }
}

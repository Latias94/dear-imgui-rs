use crate::{Plot3DBuilder, Plot3DUi};
use dear_imgui_rs::Ui;

/// Ui extension for obtaining a Plot3DUi
pub trait ImPlot3DExt {
    fn implot3d<'ui>(&'ui self) -> Plot3DUi<'ui>;
}

impl ImPlot3DExt for Ui {
    fn implot3d<'ui>(&'ui self) -> Plot3DUi<'ui> {
        Plot3DUi { _ui: self }
    }
}

impl<'ui> Plot3DUi<'ui> {
    /// Convenience builder entry from Ui
    pub fn plot3d<S: AsRef<str>>(&self, title: S) -> Plot3DBuilder {
        self.begin_plot(title)
    }
}

use std::marker::PhantomData;

use crate::{debug_end_plot, sys};
use dear_imgui_rs::Ui;

use super::binding::Plot3DContextBinding;

/// RAII token that ends the plot on drop
///
/// This token is returned by `Plot3DBuilder::build()` and automatically calls
/// `ImPlot3D_EndPlot()` when it goes out of scope, ensuring proper cleanup.
pub struct Plot3DToken<'ui> {
    pub(crate) binding: Plot3DContextBinding,
    pub(crate) imgui_alive: Option<dear_imgui_rs::ContextAliveToken>,
    pub(crate) _lifetime: PhantomData<&'ui Ui>,
}

impl Drop for Plot3DToken<'_> {
    fn drop(&mut self) {
        if let Some(alive) = &self.imgui_alive {
            assert!(
                alive.is_alive(),
                "dear-implot3d: ImGui context has been dropped"
            );
        }
        self.binding.bind();
        unsafe {
            debug_end_plot();
            sys::ImPlot3D_EndPlot();
        }
    }
}

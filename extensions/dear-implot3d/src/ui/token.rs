use std::marker::PhantomData;

use crate::{debug_end_plot, sys};
use dear_imgui_rs::{DrawListMut, Ui};

use super::binding::Plot3DContextBinding;

/// RAII token that ends the plot on drop
///
/// This token is returned by `Plot3DBuilder::build()` and automatically calls
/// `ImPlot3D_EndPlot()` when it goes out of scope, ensuring proper cleanup.
pub struct Plot3DToken<'ui> {
    pub(crate) binding: Plot3DContextBinding,
    pub(crate) imgui_alive: Option<dear_imgui_rs::ContextAliveToken>,
    pub(crate) ui: &'ui Ui,
    pub(crate) _lifetime: PhantomData<&'ui Ui>,
}

impl<'ui> Plot3DToken<'ui> {
    /// Get the active 3D plot draw list as a frame-bound wrapper.
    pub fn plot_draw_list(&self) -> Option<DrawListMut<'_>> {
        if let Some(alive) = &self.imgui_alive {
            assert!(
                alive.is_alive(),
                "dear-implot3d: ImGui context has been dropped"
            );
        }
        let _guard = self.binding.bind();
        let draw_list = unsafe { sys::ImPlot3D_GetPlotDrawList() };
        if draw_list.is_null() {
            None
        } else {
            Some(unsafe { DrawListMut::from_raw_mut(self.ui, draw_list) })
        }
    }
}

impl Drop for Plot3DToken<'_> {
    fn drop(&mut self) {
        if let Some(alive) = &self.imgui_alive {
            assert!(
                alive.is_alive(),
                "dear-implot3d: ImGui context has been dropped"
            );
        }
        let _guard = self.binding.bind();
        unsafe {
            debug_end_plot();
            sys::ImPlot3D_EndPlot();
        }
    }
}

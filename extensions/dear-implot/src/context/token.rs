use super::callbacks::PlotScopeGuard;
use super::core::PlotContextBinding;
use crate::sys;

/// Token that represents an active plot
///
/// The plot will be automatically ended when this token is dropped.
pub struct PlotToken<'ui> {
    binding: PlotContextBinding,
    imgui_alive: Option<dear_imgui_rs::ContextAliveToken>,
    _scope: PlotScopeGuard,
    _lifetime: std::marker::PhantomData<&'ui ()>,
}

impl<'ui> PlotToken<'ui> {
    /// Create a new PlotToken (internal use only)
    pub(crate) fn new(
        binding: PlotContextBinding,
        imgui_alive: Option<dear_imgui_rs::ContextAliveToken>,
    ) -> Self {
        Self {
            binding,
            imgui_alive,
            _scope: PlotScopeGuard::new(),
            _lifetime: std::marker::PhantomData,
        }
    }

    /// Manually end the plot
    ///
    /// This is called automatically when the token is dropped,
    /// but you can call it manually if needed.
    pub fn end(self) {
        // The actual ending happens in Drop
    }
}

impl<'ui> Drop for PlotToken<'ui> {
    fn drop(&mut self) {
        if let Some(alive) = &self.imgui_alive {
            assert!(
                alive.is_alive(),
                "dear-implot: ImGui context has been dropped"
            );
        }
        self.binding.bind("dear-implot: PlotToken");
        unsafe {
            sys::ImPlot_EndPlot();
        }
    }
}

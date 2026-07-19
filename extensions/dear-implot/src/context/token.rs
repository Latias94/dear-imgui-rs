use super::callbacks::PlotScopeGuard;
use super::core::PlotContextBinding;
use crate::sys;
use dear_imgui_rs::{DrawListMut, Ui};
use std::marker::PhantomData;
use std::rc::Rc;

/// Token that represents an active plot
///
/// The plot will be automatically ended when this token is dropped.
pub struct PlotToken<'ui> {
    binding: PlotContextBinding,
    ui: &'ui Ui,
    _scope: PlotScopeGuard,
    _lifetime: PhantomData<&'ui ()>,
}

impl<'ui> PlotToken<'ui> {
    /// Create a new PlotToken (internal use only)
    pub(crate) fn new(binding: PlotContextBinding, ui: &'ui Ui) -> Self {
        Self {
            binding,
            ui,
            _scope: PlotScopeGuard::new(),
            _lifetime: PhantomData,
        }
    }

    /// Manually end the plot
    ///
    /// This is called automatically when the token is dropped,
    /// but you can call it manually if needed.
    pub fn end(self) {
        // The actual ending happens in Drop
    }

    /// Get the active plot draw list as a frame-bound wrapper.
    pub fn plot_draw_list(&self) -> Option<DrawListMut<'_>> {
        self.binding
            .with_bound_context("dear-implot: PlotToken", || {
                let draw_list = unsafe { sys::ImPlot_GetPlotDrawList() };
                if draw_list.is_null() {
                    None
                } else {
                    Some(unsafe { DrawListMut::from_raw_mut(self.ui, draw_list) })
                }
            })
    }

    /// Push a plot clip rect on this active plot.
    ///
    /// The returned token pops the clip rect when dropped and cannot outlive
    /// the active plot token it was created from.
    pub fn push_plot_clip_rect(&self, expand: f32) -> PlotClipRectToken<'_> {
        assert!(
            expand.is_finite(),
            "PlotToken::push_plot_clip_rect() expand must be finite"
        );
        self.binding
            .with_bound_context("dear-implot: PlotToken", || {
                unsafe { sys::ImPlot_PushPlotClipRect(expand) };
                PlotClipRectToken {
                    binding: self.binding.clone(),
                    was_popped: false,
                    _lifetime: PhantomData,
                    _not_send_or_sync: PhantomData,
                }
            })
    }
}

impl<'ui> Drop for PlotToken<'ui> {
    fn drop(&mut self) {
        let _ = self.binding.try_with_bound_context(|| unsafe {
            sys::ImPlot_EndPlot();
        });
    }
}

/// Token for a pushed ImPlot plot clip rect.
#[must_use]
pub struct PlotClipRectToken<'plot> {
    binding: PlotContextBinding,
    was_popped: bool,
    _lifetime: PhantomData<&'plot ()>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl PlotClipRectToken<'_> {
    /// Pop the plot clip rect immediately instead of waiting for drop.
    pub fn pop(mut self) {
        self.pop_inner();
    }

    /// Pop the plot clip rect immediately instead of waiting for drop.
    pub fn end(mut self) {
        self.pop_inner();
    }

    fn pop_inner(&mut self) {
        if self.was_popped {
            panic!("Attempted to pop an ImPlot plot clip rect token twice.");
        }
        self.binding
            .with_bound_context("dear-implot: PlotClipRectToken", || {
                unsafe { sys::ImPlot_PopPlotClipRect() };
            });
        self.was_popped = true;
    }
}

impl Drop for PlotClipRectToken<'_> {
    fn drop(&mut self) {
        if !self.was_popped {
            let _ = self
                .binding
                .try_with_bound_context(|| unsafe { sys::ImPlot_PopPlotClipRect() });
            self.was_popped = true;
        }
    }
}

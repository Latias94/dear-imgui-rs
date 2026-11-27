use super::*;

impl Ui {
    /// Draw with the current window's draw list while its Context is bound.
    ///
    /// The borrow and any builders or stack tokens derived from it cannot escape the closure.
    ///
    /// ```compile_fail,E0521
    /// use dear_imgui_rs::{DrawListMut, Ui};
    ///
    /// fn escape(ui: &Ui) {
    ///     let mut saved: Option<&DrawListMut<'_>> = None;
    ///     ui.with_window_draw_list(|draw_list| saved = Some(draw_list));
    ///     drop(saved);
    /// }
    /// ```
    #[doc(alias = "GetWindowDrawList")]
    pub fn with_window_draw_list<R>(&self, draw: impl FnOnce(&DrawListMut<'_>) -> R) -> R {
        self.with_bound_context(|| draw(&DrawListMut::window(self)))
    }

    /// Access to the background draw list
    #[doc(alias = "GetBackgroundDrawList")]
    pub fn get_background_draw_list(&self) -> DrawListMut<'_> {
        DrawListMut::background(self)
    }

    /// Access to the foreground draw list
    #[doc(alias = "GetForegroundDrawList")]
    pub fn get_foreground_draw_list(&self) -> DrawListMut<'_> {
        DrawListMut::foreground(self)
    }
}

use super::*;

impl Ui {
    /// Access to the current window's draw list
    #[doc(alias = "GetWindowDrawList")]
    pub fn get_window_draw_list(&self) -> DrawListMut<'_> {
        DrawListMut::window(self)
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

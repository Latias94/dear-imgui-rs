use crate::types::Vec2;
/// Miscellaneous widgets
///
/// This module contains miscellaneous UI components that don't fit into other categories.
use crate::ui::Ui;
use dear_imgui_sys as sys;

/// # Widgets: Miscellaneous
impl<'frame> Ui<'frame> {
    /// Get the global Dear ImGui time
    ///
    /// This is the time since Dear ImGui was initialized, incremented by delta_time every frame.
    /// Useful for animations and time-based effects.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// let time = ui.get_time();
    /// let animated_value = (time * 2.0).sin() * 0.5 + 0.5; // Sine wave animation
    /// ui.progress_bar(animated_value as f32, Some("Animated"));
    /// # true
    /// # });
    /// ```
    pub fn get_time(&mut self) -> f64 {
        unsafe { sys::ImGui_GetTime() }
    }

    /// Get the global Dear ImGui frame count
    ///
    /// This is incremented by 1 every frame. Useful for frame-based logic.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// let frame_count = ui.get_frame_count();
    /// ui.text(&format!("Frame: {}", frame_count));
    /// # true
    /// # });
    /// ```
    pub fn get_frame_count(&mut self) -> i32 {
        unsafe { sys::ImGui_GetFrameCount() }
    }

    /// Get the current cursor position in screen coordinates
    ///
    /// Returns the cursor position in screen coordinates (useful for drawing).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// let screen_pos = ui.get_cursor_screen_pos();
    /// ui.text(&format!("Cursor at: ({:.1}, {:.1})", screen_pos.x, screen_pos.y));
    /// # true
    /// # });
    /// ```
    pub fn get_cursor_screen_pos(&mut self) -> Vec2 {
        unsafe {
            let pos = sys::ImGui_GetCursorScreenPos();
            Vec2::new(pos.x, pos.y)
        }
    }

    /// Get the delta time for the current frame
    ///
    /// This is the time elapsed since the last frame, useful for frame-rate independent animations.
    /// This is equivalent to ImGui::GetIO().DeltaTime in C++.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Example").show(|ui| {
    /// let delta_time = ui.get_delta_time();
    /// ui.text(&format!("Frame time: {:.3}ms", delta_time * 1000.0));
    /// # true
    /// # });
    /// ```
    pub fn get_delta_time(&mut self) -> f32 {
        unsafe {
            let io = sys::ImGui_GetIO();
            (*io).DeltaTime
        }
    }
}

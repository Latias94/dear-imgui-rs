use super::*;

impl Io {
    /// Returns whether to use MacOSX-style behaviors.
    #[doc(alias = "ConfigMacOSXBehaviors")]
    pub fn config_macosx_behaviors(&self) -> bool {
        self.inner().ConfigMacOSXBehaviors
    }

    /// Set whether to use MacOSX-style behaviors.
    #[doc(alias = "ConfigMacOSXBehaviors")]
    pub fn set_config_macosx_behaviors(&mut self, enabled: bool) {
        self.inner_mut().ConfigMacOSXBehaviors = enabled;
    }

    /// Returns whether to trickle input events through the queue.
    #[doc(alias = "ConfigInputTrickleEventQueue")]
    pub fn config_input_trickle_event_queue(&self) -> bool {
        self.inner().ConfigInputTrickleEventQueue
    }

    /// Set whether to trickle input events through the queue.
    #[doc(alias = "ConfigInputTrickleEventQueue")]
    pub fn set_config_input_trickle_event_queue(&mut self, enabled: bool) {
        self.inner_mut().ConfigInputTrickleEventQueue = enabled;
    }

    /// Returns whether the input text cursor blinks.
    #[doc(alias = "ConfigInputTextCursorBlink")]
    pub fn config_input_text_cursor_blink(&self) -> bool {
        self.inner().ConfigInputTextCursorBlink
    }

    /// Set whether the input text cursor blinks.
    #[doc(alias = "ConfigInputTextCursorBlink")]
    pub fn set_config_input_text_cursor_blink(&mut self, enabled: bool) {
        self.inner_mut().ConfigInputTextCursorBlink = enabled;
    }

    /// Returns whether Enter keeps the input text active.
    #[doc(alias = "ConfigInputTextEnterKeepActive")]
    pub fn config_input_text_enter_keep_active(&self) -> bool {
        self.inner().ConfigInputTextEnterKeepActive
    }

    /// Set whether Enter keeps the input text active.
    #[doc(alias = "ConfigInputTextEnterKeepActive")]
    pub fn set_config_input_text_enter_keep_active(&mut self, enabled: bool) {
        self.inner_mut().ConfigInputTextEnterKeepActive = enabled;
    }

    /// Returns whether click-drag on numeric widgets turns into text input.
    #[doc(alias = "ConfigDragClickToInputText")]
    pub fn config_drag_click_to_input_text(&self) -> bool {
        self.inner().ConfigDragClickToInputText
    }

    /// Set whether click-drag on numeric widgets turns into text input.
    #[doc(alias = "ConfigDragClickToInputText")]
    pub fn set_config_drag_click_to_input_text(&mut self, enabled: bool) {
        self.inner_mut().ConfigDragClickToInputText = enabled;
    }

    /// Returns whether windows can be moved only from their title bar.
    ///
    /// When enabled, click-dragging on empty window content will no longer move the window.
    /// This can be useful in multi-viewport setups to avoid accidental platform window moves
    /// while interacting with in-window widgets (e.g. gizmos in a scene view).
    #[doc(alias = "ConfigWindowsMoveFromTitleBarOnly")]
    pub fn config_windows_move_from_title_bar_only(&self) -> bool {
        self.inner().ConfigWindowsMoveFromTitleBarOnly
    }

    /// Set whether windows can be moved only from their title bar.
    ///
    /// Note: This is typically latched by Dear ImGui at the start of the frame. Prefer setting it
    /// during initialization or before calling `Context::frame()`.
    #[doc(alias = "ConfigWindowsMoveFromTitleBarOnly")]
    pub fn set_config_windows_move_from_title_bar_only(&mut self, enabled: bool) {
        self.inner_mut().ConfigWindowsMoveFromTitleBarOnly = enabled;
    }

    /// Returns whether windows can be resized from their edges.
    #[doc(alias = "ConfigWindowsResizeFromEdges")]
    pub fn config_windows_resize_from_edges(&self) -> bool {
        self.inner().ConfigWindowsResizeFromEdges
    }

    /// Set whether windows can be resized from their edges.
    #[doc(alias = "ConfigWindowsResizeFromEdges")]
    pub fn set_config_windows_resize_from_edges(&mut self, enabled: bool) {
        self.inner_mut().ConfigWindowsResizeFromEdges = enabled;
    }

    /// Returns whether Ctrl+C copies window contents.
    #[doc(alias = "ConfigWindowsCopyContentsWithCtrlC")]
    pub fn config_windows_copy_contents_with_ctrl_c(&self) -> bool {
        self.inner().ConfigWindowsCopyContentsWithCtrlC
    }

    /// Set whether Ctrl+C copies window contents.
    #[doc(alias = "ConfigWindowsCopyContentsWithCtrlC")]
    pub fn set_config_windows_copy_contents_with_ctrl_c(&mut self, enabled: bool) {
        self.inner_mut().ConfigWindowsCopyContentsWithCtrlC = enabled;
    }

    /// Returns whether scrollbars scroll by page.
    #[doc(alias = "ConfigScrollbarScrollByPage")]
    pub fn config_scrollbar_scroll_by_page(&self) -> bool {
        self.inner().ConfigScrollbarScrollByPage
    }

    /// Set whether scrollbars scroll by page.
    #[doc(alias = "ConfigScrollbarScrollByPage")]
    pub fn set_config_scrollbar_scroll_by_page(&mut self, enabled: bool) {
        self.inner_mut().ConfigScrollbarScrollByPage = enabled;
    }

    /// Returns the memory compact timer (seconds).
    #[doc(alias = "ConfigMemoryCompactTimer")]
    pub fn config_memory_compact_timer(&self) -> f32 {
        self.inner().ConfigMemoryCompactTimer
    }

    /// Set the memory compact timer (seconds).
    #[doc(alias = "ConfigMemoryCompactTimer")]
    pub fn set_config_memory_compact_timer(&mut self, seconds: f32) {
        assert_memory_compact_timer("Io::set_config_memory_compact_timer()", seconds);
        self.inner_mut().ConfigMemoryCompactTimer = seconds;
    }

    /// Returns the time for a double-click (seconds).
    #[doc(alias = "MouseDoubleClickTime")]
    pub fn mouse_double_click_time(&self) -> f32 {
        self.inner().MouseDoubleClickTime
    }

    /// Set the time for a double-click (seconds).
    #[doc(alias = "MouseDoubleClickTime")]
    pub fn set_mouse_double_click_time(&mut self, seconds: f32) {
        assert_non_negative_f32("Io::set_mouse_double_click_time()", "seconds", seconds);
        self.inner_mut().MouseDoubleClickTime = seconds;
    }

    /// Returns the maximum distance to qualify as a double-click (pixels).
    #[doc(alias = "MouseDoubleClickMaxDist")]
    pub fn mouse_double_click_max_dist(&self) -> f32 {
        self.inner().MouseDoubleClickMaxDist
    }

    /// Set the maximum distance to qualify as a double-click (pixels).
    #[doc(alias = "MouseDoubleClickMaxDist")]
    pub fn set_mouse_double_click_max_dist(&mut self, pixels: f32) {
        assert_non_negative_f32("Io::set_mouse_double_click_max_dist()", "pixels", pixels);
        self.inner_mut().MouseDoubleClickMaxDist = pixels;
    }

    /// Returns the distance threshold for starting a drag (pixels).
    #[doc(alias = "MouseDragThreshold")]
    pub fn mouse_drag_threshold(&self) -> f32 {
        self.inner().MouseDragThreshold
    }

    /// Set the distance threshold for starting a drag (pixels).
    #[doc(alias = "MouseDragThreshold")]
    pub fn set_mouse_drag_threshold(&mut self, pixels: f32) {
        assert_non_negative_f32("Io::set_mouse_drag_threshold()", "pixels", pixels);
        self.inner_mut().MouseDragThreshold = pixels;
    }

    /// Returns the key repeat delay (seconds).
    #[doc(alias = "KeyRepeatDelay")]
    pub fn key_repeat_delay(&self) -> f32 {
        self.inner().KeyRepeatDelay
    }

    /// Set the key repeat delay (seconds).
    #[doc(alias = "KeyRepeatDelay")]
    pub fn set_key_repeat_delay(&mut self, seconds: f32) {
        assert_non_negative_f32("Io::set_key_repeat_delay()", "seconds", seconds);
        self.inner_mut().KeyRepeatDelay = seconds;
    }

    /// Returns the key repeat rate (seconds).
    #[doc(alias = "KeyRepeatRate")]
    pub fn key_repeat_rate(&self) -> f32 {
        self.inner().KeyRepeatRate
    }

    /// Set the key repeat rate (seconds).
    #[doc(alias = "KeyRepeatRate")]
    pub fn set_key_repeat_rate(&mut self, seconds: f32) {
        assert_non_negative_f32("Io::set_key_repeat_rate()", "seconds", seconds);
        self.inner_mut().KeyRepeatRate = seconds;
    }
}

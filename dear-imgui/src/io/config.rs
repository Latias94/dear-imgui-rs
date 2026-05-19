use crate::io::{
    ConfigFlags, Io, assert_memory_compact_timer, assert_non_negative_f32, validate_config_flags,
};

impl Io {
    /// Configuration flags
    pub fn config_flags(&self) -> ConfigFlags {
        ConfigFlags::from_bits_truncate(self.inner().ConfigFlags)
    }

    /// Set configuration flags
    pub fn set_config_flags(&mut self, flags: ConfigFlags) {
        validate_config_flags("Io::set_config_flags()", flags);
        self.inner_mut().ConfigFlags = flags.bits();
    }

    /// Returns whether to swap gamepad buttons for navigation.
    #[doc(alias = "ConfigNavSwapGamepadButtons")]
    pub fn config_nav_swap_gamepad_buttons(&self) -> bool {
        self.inner().ConfigNavSwapGamepadButtons
    }

    /// Set whether to swap gamepad buttons for navigation.
    #[doc(alias = "ConfigNavSwapGamepadButtons")]
    pub fn set_config_nav_swap_gamepad_buttons(&mut self, enabled: bool) {
        self.inner_mut().ConfigNavSwapGamepadButtons = enabled;
    }

    /// Returns whether navigation can move the mouse cursor.
    #[doc(alias = "ConfigNavMoveSetMousePos")]
    pub fn config_nav_move_set_mouse_pos(&self) -> bool {
        self.inner().ConfigNavMoveSetMousePos
    }

    /// Set whether navigation can move the mouse cursor.
    #[doc(alias = "ConfigNavMoveSetMousePos")]
    pub fn set_config_nav_move_set_mouse_pos(&mut self, enabled: bool) {
        self.inner_mut().ConfigNavMoveSetMousePos = enabled;
    }

    /// Returns whether to capture keyboard inputs during navigation.
    #[doc(alias = "ConfigNavCaptureKeyboard")]
    pub fn config_nav_capture_keyboard(&self) -> bool {
        self.inner().ConfigNavCaptureKeyboard
    }

    /// Set whether to capture keyboard inputs during navigation.
    #[doc(alias = "ConfigNavCaptureKeyboard")]
    pub fn set_config_nav_capture_keyboard(&mut self, enabled: bool) {
        self.inner_mut().ConfigNavCaptureKeyboard = enabled;
    }

    /// Returns whether Escape clears the focused item.
    #[doc(alias = "ConfigNavEscapeClearFocusItem")]
    pub fn config_nav_escape_clear_focus_item(&self) -> bool {
        self.inner().ConfigNavEscapeClearFocusItem
    }

    /// Set whether Escape clears the focused item.
    #[doc(alias = "ConfigNavEscapeClearFocusItem")]
    pub fn set_config_nav_escape_clear_focus_item(&mut self, enabled: bool) {
        self.inner_mut().ConfigNavEscapeClearFocusItem = enabled;
    }

    /// Returns whether Escape clears the focused window.
    #[doc(alias = "ConfigNavEscapeClearFocusWindow")]
    pub fn config_nav_escape_clear_focus_window(&self) -> bool {
        self.inner().ConfigNavEscapeClearFocusWindow
    }

    /// Set whether Escape clears the focused window.
    #[doc(alias = "ConfigNavEscapeClearFocusWindow")]
    pub fn set_config_nav_escape_clear_focus_window(&mut self, enabled: bool) {
        self.inner_mut().ConfigNavEscapeClearFocusWindow = enabled;
    }

    /// Returns whether the navigation cursor visibility is automatically managed.
    #[doc(alias = "ConfigNavCursorVisibleAuto")]
    pub fn config_nav_cursor_visible_auto(&self) -> bool {
        self.inner().ConfigNavCursorVisibleAuto
    }

    /// Set whether the navigation cursor visibility is automatically managed.
    #[doc(alias = "ConfigNavCursorVisibleAuto")]
    pub fn set_config_nav_cursor_visible_auto(&mut self, enabled: bool) {
        self.inner_mut().ConfigNavCursorVisibleAuto = enabled;
    }

    /// Returns whether the navigation cursor is always visible.
    #[doc(alias = "ConfigNavCursorVisibleAlways")]
    pub fn config_nav_cursor_visible_always(&self) -> bool {
        self.inner().ConfigNavCursorVisibleAlways
    }

    /// Set whether the navigation cursor is always visible.
    #[doc(alias = "ConfigNavCursorVisibleAlways")]
    pub fn set_config_nav_cursor_visible_always(&mut self, enabled: bool) {
        self.inner_mut().ConfigNavCursorVisibleAlways = enabled;
    }

    /// Returns whether docking is prevented from splitting nodes.
    #[doc(alias = "ConfigDockingNoSplit")]
    pub fn config_docking_no_split(&self) -> bool {
        self.inner().ConfigDockingNoSplit
    }

    /// Set whether docking is prevented from splitting nodes.
    #[doc(alias = "ConfigDockingNoSplit")]
    pub fn set_config_docking_no_split(&mut self, enabled: bool) {
        self.inner_mut().ConfigDockingNoSplit = enabled;
    }

    /// Returns whether docking over other windows is disabled.
    #[doc(alias = "ConfigDockingNoDockingOver")]
    pub fn config_docking_no_docking_over(&self) -> bool {
        self.inner().ConfigDockingNoDockingOver
    }

    /// Set whether docking over other windows is disabled.
    #[doc(alias = "ConfigDockingNoDockingOver")]
    pub fn set_config_docking_no_docking_over(&mut self, enabled: bool) {
        self.inner_mut().ConfigDockingNoDockingOver = enabled;
    }

    /// Returns whether docking requires holding Shift.
    #[doc(alias = "ConfigDockingWithShift")]
    pub fn config_docking_with_shift(&self) -> bool {
        self.inner().ConfigDockingWithShift
    }

    /// Set whether docking requires holding Shift.
    #[doc(alias = "ConfigDockingWithShift")]
    pub fn set_config_docking_with_shift(&mut self, enabled: bool) {
        self.inner_mut().ConfigDockingWithShift = enabled;
    }

    /// Returns whether docking uses a tab bar when possible.
    #[doc(alias = "ConfigDockingAlwaysTabBar")]
    pub fn config_docking_always_tab_bar(&self) -> bool {
        self.inner().ConfigDockingAlwaysTabBar
    }

    /// Set whether docking uses a tab bar when possible.
    #[doc(alias = "ConfigDockingAlwaysTabBar")]
    pub fn set_config_docking_always_tab_bar(&mut self, enabled: bool) {
        self.inner_mut().ConfigDockingAlwaysTabBar = enabled;
    }

    /// Returns whether docking payloads are rendered transparently.
    #[doc(alias = "ConfigDockingTransparentPayload")]
    pub fn config_docking_transparent_payload(&self) -> bool {
        self.inner().ConfigDockingTransparentPayload
    }

    /// Set whether docking payloads are rendered transparently.
    #[doc(alias = "ConfigDockingTransparentPayload")]
    pub fn set_config_docking_transparent_payload(&mut self, enabled: bool) {
        self.inner_mut().ConfigDockingTransparentPayload = enabled;
    }

    /// Returns whether viewports should avoid auto-merging.
    #[doc(alias = "ConfigViewportsNoAutoMerge")]
    pub fn config_viewports_no_auto_merge(&self) -> bool {
        self.inner().ConfigViewportsNoAutoMerge
    }

    /// Set whether viewports should avoid auto-merging.
    #[doc(alias = "ConfigViewportsNoAutoMerge")]
    pub fn set_config_viewports_no_auto_merge(&mut self, enabled: bool) {
        self.inner_mut().ConfigViewportsNoAutoMerge = enabled;
    }

    /// Returns whether viewports should avoid task bar icons.
    #[doc(alias = "ConfigViewportsNoTaskBarIcon")]
    pub fn config_viewports_no_task_bar_icon(&self) -> bool {
        self.inner().ConfigViewportsNoTaskBarIcon
    }

    /// Set whether viewports should avoid task bar icons.
    #[doc(alias = "ConfigViewportsNoTaskBarIcon")]
    pub fn set_config_viewports_no_task_bar_icon(&mut self, enabled: bool) {
        self.inner_mut().ConfigViewportsNoTaskBarIcon = enabled;
    }

    /// Returns whether viewports should avoid platform window decorations.
    #[doc(alias = "ConfigViewportsNoDecoration")]
    pub fn config_viewports_no_decoration(&self) -> bool {
        self.inner().ConfigViewportsNoDecoration
    }

    /// Set whether viewports should avoid platform window decorations.
    #[doc(alias = "ConfigViewportsNoDecoration")]
    pub fn set_config_viewports_no_decoration(&mut self, enabled: bool) {
        self.inner_mut().ConfigViewportsNoDecoration = enabled;
    }

    /// Returns whether viewports should not have a default parent.
    #[doc(alias = "ConfigViewportsNoDefaultParent")]
    pub fn config_viewports_no_default_parent(&self) -> bool {
        self.inner().ConfigViewportsNoDefaultParent
    }

    /// Set whether viewports should not have a default parent.
    #[doc(alias = "ConfigViewportsNoDefaultParent")]
    pub fn set_config_viewports_no_default_parent(&mut self, enabled: bool) {
        self.inner_mut().ConfigViewportsNoDefaultParent = enabled;
    }

    /// Returns whether platform focus also sets ImGui focus in viewports.
    #[doc(alias = "ConfigViewportsPlatformFocusSetsImGuiFocus")]
    pub fn config_viewports_platform_focus_sets_imgui_focus(&self) -> bool {
        self.inner().ConfigViewportsPlatformFocusSetsImGuiFocus
    }

    /// Set whether platform focus also sets ImGui focus in viewports.
    #[doc(alias = "ConfigViewportsPlatformFocusSetsImGuiFocus")]
    pub fn set_config_viewports_platform_focus_sets_imgui_focus(&mut self, enabled: bool) {
        self.inner_mut().ConfigViewportsPlatformFocusSetsImGuiFocus = enabled;
    }

    /// Returns whether fonts are scaled by DPI.
    #[doc(alias = "ConfigDpiScaleFonts")]
    pub fn config_dpi_scale_fonts(&self) -> bool {
        self.inner().ConfigDpiScaleFonts
    }

    /// Set whether fonts are scaled by DPI.
    #[doc(alias = "ConfigDpiScaleFonts")]
    pub fn set_config_dpi_scale_fonts(&mut self, enabled: bool) {
        self.inner_mut().ConfigDpiScaleFonts = enabled;
    }

    /// Returns whether viewports are scaled by DPI.
    #[doc(alias = "ConfigDpiScaleViewports")]
    pub fn config_dpi_scale_viewports(&self) -> bool {
        self.inner().ConfigDpiScaleViewports
    }

    /// Set whether viewports are scaled by DPI.
    #[doc(alias = "ConfigDpiScaleViewports")]
    pub fn set_config_dpi_scale_viewports(&mut self, enabled: bool) {
        self.inner_mut().ConfigDpiScaleViewports = enabled;
    }

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

    /// Returns whether error recovery is enabled.
    #[doc(alias = "ConfigErrorRecovery")]
    pub fn config_error_recovery(&self) -> bool {
        self.inner().ConfigErrorRecovery
    }

    /// Set whether error recovery is enabled.
    #[doc(alias = "ConfigErrorRecovery")]
    pub fn set_config_error_recovery(&mut self, enabled: bool) {
        self.inner_mut().ConfigErrorRecovery = enabled;
    }

    /// Returns whether error recovery enables asserts.
    #[doc(alias = "ConfigErrorRecoveryEnableAssert")]
    pub fn config_error_recovery_enable_assert(&self) -> bool {
        self.inner().ConfigErrorRecoveryEnableAssert
    }

    /// Set whether error recovery enables asserts.
    #[doc(alias = "ConfigErrorRecoveryEnableAssert")]
    pub fn set_config_error_recovery_enable_assert(&mut self, enabled: bool) {
        self.inner_mut().ConfigErrorRecoveryEnableAssert = enabled;
    }

    /// Returns whether error recovery enables debug logs.
    #[doc(alias = "ConfigErrorRecoveryEnableDebugLog")]
    pub fn config_error_recovery_enable_debug_log(&self) -> bool {
        self.inner().ConfigErrorRecoveryEnableDebugLog
    }

    /// Set whether error recovery enables debug logs.
    #[doc(alias = "ConfigErrorRecoveryEnableDebugLog")]
    pub fn set_config_error_recovery_enable_debug_log(&mut self, enabled: bool) {
        self.inner_mut().ConfigErrorRecoveryEnableDebugLog = enabled;
    }

    /// Returns whether error recovery enables tooltips.
    #[doc(alias = "ConfigErrorRecoveryEnableTooltip")]
    pub fn config_error_recovery_enable_tooltip(&self) -> bool {
        self.inner().ConfigErrorRecoveryEnableTooltip
    }

    /// Set whether error recovery enables tooltips.
    #[doc(alias = "ConfigErrorRecoveryEnableTooltip")]
    pub fn set_config_error_recovery_enable_tooltip(&mut self, enabled: bool) {
        self.inner_mut().ConfigErrorRecoveryEnableTooltip = enabled;
    }

    /// Returns whether Dear ImGui thinks a debugger is present.
    #[doc(alias = "ConfigDebugIsDebuggerPresent")]
    pub fn config_debug_is_debugger_present(&self) -> bool {
        self.inner().ConfigDebugIsDebuggerPresent
    }

    /// Set whether Dear ImGui thinks a debugger is present.
    #[doc(alias = "ConfigDebugIsDebuggerPresent")]
    pub fn set_config_debug_is_debugger_present(&mut self, enabled: bool) {
        self.inner_mut().ConfigDebugIsDebuggerPresent = enabled;
    }

    /// Returns whether to highlight ID conflicts.
    #[doc(alias = "ConfigDebugHighlightIdConflicts")]
    pub fn config_debug_highlight_id_conflicts(&self) -> bool {
        self.inner().ConfigDebugHighlightIdConflicts
    }

    /// Set whether to highlight ID conflicts.
    #[doc(alias = "ConfigDebugHighlightIdConflicts")]
    pub fn set_config_debug_highlight_id_conflicts(&mut self, enabled: bool) {
        self.inner_mut().ConfigDebugHighlightIdConflicts = enabled;
    }

    /// Returns whether to show the item picker when highlighting ID conflicts.
    #[doc(alias = "ConfigDebugHighlightIdConflictsShowItemPicker")]
    pub fn config_debug_highlight_id_conflicts_show_item_picker(&self) -> bool {
        self.inner().ConfigDebugHighlightIdConflictsShowItemPicker
    }

    /// Set whether to show the item picker when highlighting ID conflicts.
    #[doc(alias = "ConfigDebugHighlightIdConflictsShowItemPicker")]
    pub fn set_config_debug_highlight_id_conflicts_show_item_picker(&mut self, enabled: bool) {
        self.inner_mut()
            .ConfigDebugHighlightIdConflictsShowItemPicker = enabled;
    }

    /// Returns whether `Begin()` returns `true` once.
    #[doc(alias = "ConfigDebugBeginReturnValueOnce")]
    pub fn config_debug_begin_return_value_once(&self) -> bool {
        self.inner().ConfigDebugBeginReturnValueOnce
    }

    /// Set whether `Begin()` returns `true` once.
    #[doc(alias = "ConfigDebugBeginReturnValueOnce")]
    pub fn set_config_debug_begin_return_value_once(&mut self, enabled: bool) {
        self.inner_mut().ConfigDebugBeginReturnValueOnce = enabled;
    }

    /// Returns whether `Begin()` returns `true` in a loop.
    #[doc(alias = "ConfigDebugBeginReturnValueLoop")]
    pub fn config_debug_begin_return_value_loop(&self) -> bool {
        self.inner().ConfigDebugBeginReturnValueLoop
    }

    /// Set whether `Begin()` returns `true` in a loop.
    #[doc(alias = "ConfigDebugBeginReturnValueLoop")]
    pub fn set_config_debug_begin_return_value_loop(&mut self, enabled: bool) {
        self.inner_mut().ConfigDebugBeginReturnValueLoop = enabled;
    }

    /// Returns whether to ignore focus loss.
    #[doc(alias = "ConfigDebugIgnoreFocusLoss")]
    pub fn config_debug_ignore_focus_loss(&self) -> bool {
        self.inner().ConfigDebugIgnoreFocusLoss
    }

    /// Set whether to ignore focus loss.
    #[doc(alias = "ConfigDebugIgnoreFocusLoss")]
    pub fn set_config_debug_ignore_focus_loss(&mut self, enabled: bool) {
        self.inner_mut().ConfigDebugIgnoreFocusLoss = enabled;
    }

    /// Returns whether to display ini settings debug tools.
    #[doc(alias = "ConfigDebugIniSettings")]
    pub fn config_debug_ini_settings(&self) -> bool {
        self.inner().ConfigDebugIniSettings
    }

    /// Set whether to display ini settings debug tools.
    #[doc(alias = "ConfigDebugIniSettings")]
    pub fn set_config_debug_ini_settings(&mut self, enabled: bool) {
        self.inner_mut().ConfigDebugIniSettings = enabled;
    }
}

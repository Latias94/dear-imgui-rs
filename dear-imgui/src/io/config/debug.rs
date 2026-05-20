use super::*;

impl Io {
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

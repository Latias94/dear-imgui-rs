use super::*;

impl Io {
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
}

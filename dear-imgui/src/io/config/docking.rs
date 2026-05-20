use super::*;

impl Io {
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
}

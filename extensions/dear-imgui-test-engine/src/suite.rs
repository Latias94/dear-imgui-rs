use dear_imgui_test_engine_sys as sys;

const NATIVE_DEFAULT_TESTS: &[&str] = &["basic_interaction", "input_value"];

const UPSTREAM_DOCKING_TESTS: &[&str] = &[
    "docking_move_does_not_dock",
    "docking_basic_1",
    "docking_api_builder_1",
    "docking_api_set_next",
    "docking_into_parent_node",
    "docking_auto_nodes_size",
    "docking_focus_1",
    "docking_undock_tabs_and_nodes",
    "docking_hide_tabbar",
    "docking_over_child",
    "docking_over_dockspace",
    "docking_tab_order",
    "docking_tab_order_hidden_tabbar",
    "docking_tab_order_preserve",
    "docking_tab_focus_restore",
    "docking_tab_amend",
    "docking_tab_clipped_is_hovered",
    "docking_dockspace_item_query",
    "docking_dockspace_keep_alive",
    "docking_dockspace_passthru_hover",
    "docking_dockspace_passthru_padding",
    "docking_preserve_docking_info",
    "docking_focus_from_menu",
    "docking_focus_from_host",
    "docking_focus_from_host_nav",
    "docking_focus_nodes_1",
    "docking_focus_nodes_nested",
    "docking_tab_state",
    "docking_undock_simple",
    "docking_undock_large",
    "docking_undock_from_dockspace_size",
    "docking_undock_focus_retention",
    "docking_sizing_1",
    "docking_split_payload",
    "docking_window_appearing",
    "docking_window_appearing_layout",
    "docking_popup_parent",
    "docking_dockspace_tab_amend",
    "docking_settings_invalid_1",
];

const UPSTREAM_VIEWPORT_TESTS: &[&str] = &[
    "viewport_basic_1",
    "viewport_translate",
    "viewport_parent_id",
    "viewport_platform_focus",
    "viewport_platform_focus_2",
    "viewport_platform_focus_3",
    "viewport_platform_focus_4",
    "viewport_platform_close",
    "viewport_platform_close_2",
    "viewport_owner_change_1",
    "viewport_owner_change_2",
];

/// A maintained native Test Engine suite with a pinned registration manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BuiltInTestSuite {
    /// The two small integration tests owned by this repository.
    NativeDefaults,
    /// The official upstream docking suite.
    UpstreamDocking,
    /// The official upstream viewport suite.
    ///
    /// Registration requires docking, plus attached platform and renderer backends that both
    /// advertise multi-viewport support. Run this suite with real platform windows when platform
    /// behavior is part of the contract.
    UpstreamViewports,
}

impl BuiltInTestSuite {
    #[must_use]
    pub const fn category(self) -> &'static str {
        match self {
            Self::NativeDefaults => "demo_tests",
            Self::UpstreamDocking => "docking",
            Self::UpstreamViewports => "viewport",
        }
    }

    #[must_use]
    pub const fn expected_test_names(self) -> &'static [&'static str] {
        match self {
            Self::NativeDefaults => NATIVE_DEFAULT_TESTS,
            Self::UpstreamDocking => UPSTREAM_DOCKING_TESTS,
            Self::UpstreamViewports => UPSTREAM_VIEWPORT_TESTS,
        }
    }

    #[must_use]
    pub const fn expected_test_count(self) -> usize {
        self.expected_test_names().len()
    }

    pub(crate) const fn as_raw(self) -> sys::ImGuiTestEngineBuiltinTestSuite {
        match self {
            Self::NativeDefaults => sys::ImGuiTestEngineBuiltinTestSuite_NativeDefaults,
            Self::UpstreamDocking => sys::ImGuiTestEngineBuiltinTestSuite_UpstreamDocking,
            Self::UpstreamViewports => sys::ImGuiTestEngineBuiltinTestSuite_UpstreamViewports,
        }
    }
}

/// Exact manifest returned after one built-in suite has been registered.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredTestSuite {
    suite: BuiltInTestSuite,
    test_names: Vec<String>,
    engine_identity: usize,
}

impl RegisteredTestSuite {
    pub(crate) fn new(
        suite: BuiltInTestSuite,
        test_names: Vec<String>,
        engine_identity: usize,
    ) -> Self {
        Self {
            suite,
            test_names,
            engine_identity,
        }
    }

    #[must_use]
    pub const fn suite(&self) -> BuiltInTestSuite {
        self.suite
    }

    #[must_use]
    pub fn test_names(&self) -> &[String] {
        &self.test_names
    }

    #[must_use]
    pub fn test_count(&self) -> usize {
        self.test_names.len()
    }

    pub(crate) const fn engine_identity(&self) -> usize {
        self.engine_identity
    }
}

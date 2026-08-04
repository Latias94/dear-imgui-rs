use super::protocol::ImguiViewportId;

/// Native viewport runtime failure reported through Context lifecycle operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImguiViewportRuntimeError {
    /// A native callback re-entered the Bevy viewport runtime before its prior call completed.
    CallbackReentered,
    /// A native backend field changed after the Bevy viewport bridge claimed it.
    CallbackOwnership(ImguiViewportCallbackOwnershipError),
    /// Two live native viewport instances published the same current numeric ID.
    ViewportIdCollision { viewport_id: ImguiViewportId },
    /// The bridge exhausted its stable viewport instance generation space.
    ViewportInstanceGenerationExhausted,
    /// A callback referenced a native viewport instance not registered by this bridge.
    ViewportInstanceUnavailable,
}

impl std::fmt::Display for ImguiViewportRuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CallbackReentered => {
                formatter.write_str("a Dear ImGui viewport callback re-entered the Bevy runtime")
            }
            Self::CallbackOwnership(error) => error.fmt(formatter),
            Self::ViewportIdCollision { viewport_id } => write!(
                formatter,
                "two live Dear ImGui viewport instances published ID {:#010X}",
                viewport_id.raw()
            ),
            Self::ViewportInstanceGenerationExhausted => formatter.write_str(
                "the Bevy viewport bridge exhausted its stable instance generation space",
            ),
            Self::ViewportInstanceUnavailable => formatter
                .write_str("a Dear ImGui viewport callback referenced an unknown native instance"),
        }
    }
}

impl std::error::Error for ImguiViewportRuntimeError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ImguiViewportCallbackInstallError {
    BackendPlatformUserData,
    BackendPlatformName,
    BackendFlag { flag: &'static str },
    CallbackSlot { slot: &'static str },
    MainViewportField { field: &'static str },
    PlatformMonitors,
}

impl std::fmt::Display for ImguiViewportCallbackInstallError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BackendPlatformUserData => {
                formatter.write_str("Dear ImGui BackendPlatformUserData is already owned")
            }
            Self::BackendPlatformName => {
                formatter.write_str("Dear ImGui BackendPlatformName is already owned")
            }
            Self::BackendFlag { flag } => {
                write!(
                    formatter,
                    "Dear ImGui backend flag `{flag}` is already owned"
                )
            }
            Self::CallbackSlot { slot } => {
                write!(formatter, "Dear ImGui {slot} callback is already owned")
            }
            Self::MainViewportField { field } => {
                write!(
                    formatter,
                    "Dear ImGui main viewport {field} is already owned"
                )
            }
            Self::PlatformMonitors => {
                formatter.write_str("Dear ImGui PlatformIO.Monitors is already owned")
            }
        }
    }
}

impl std::error::Error for ImguiViewportCallbackInstallError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImguiViewportCallbackOwnershipError {
    /// Another backend replaced `BackendPlatformUserData` while Bevy's callbacks were installed.
    BackendPlatformUserDataReplaced,
    /// Another backend replaced Bevy's exact `BackendPlatformName` allocation.
    BackendPlatformNameReplaced,
    /// Another backend changed a capability bit owned by the Bevy viewport bridge.
    BackendFlagReplaced { flag: &'static str },
    /// Another backend replaced one of Bevy's platform callbacks.
    PlatformCallbackReplaced { slot: &'static str },
    /// A foreign platform callback appeared while Bevy-owned platform handles were live.
    PlatformCallbackInstalled { slot: &'static str },
    /// A renderer callback appeared while Bevy-owned platform handles were live.
    RendererCallbackInstalled { slot: &'static str },
    /// Another backend replaced the monitor vector published by Bevy.
    PlatformMonitorsReplaced,
    /// A viewport field no longer contained the handle allocation owned by Bevy.
    ViewportFieldReplaced { field: &'static str },
    /// The bridge's installed callback fingerprint was unavailable.
    CallbackContractUnavailable,
}

impl std::fmt::Display for ImguiViewportCallbackOwnershipError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BackendPlatformUserDataReplaced => {
                formatter.write_str("Dear ImGui BackendPlatformUserData was replaced")
            }
            Self::BackendPlatformNameReplaced => {
                formatter.write_str("Dear ImGui BackendPlatformName was replaced")
            }
            Self::BackendFlagReplaced { flag } => {
                write!(formatter, "Dear ImGui backend flag `{flag}` was replaced")
            }
            Self::PlatformCallbackReplaced { slot } => {
                write!(formatter, "Dear ImGui {slot} callback was replaced")
            }
            Self::PlatformCallbackInstalled { slot } => {
                write!(
                    formatter,
                    "foreign Dear ImGui {slot} callback was installed"
                )
            }
            Self::RendererCallbackInstalled { slot } => {
                write!(
                    formatter,
                    "foreign Dear ImGui {slot} callback was installed"
                )
            }
            Self::PlatformMonitorsReplaced => {
                formatter.write_str("Dear ImGui PlatformIO.Monitors was replaced")
            }
            Self::ViewportFieldReplaced { field } => {
                write!(formatter, "Dear ImGui viewport {field} was replaced")
            }
            Self::CallbackContractUnavailable => formatter
                .write_str("Dear ImGui viewport callback ownership contract was unavailable"),
        }
    }
}

impl std::error::Error for ImguiViewportCallbackOwnershipError {}

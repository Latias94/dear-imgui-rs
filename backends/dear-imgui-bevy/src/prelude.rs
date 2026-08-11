//! Common imports for applications using the Bevy backend.

pub use crate::context::{
    ImguiAppExt, ImguiContextAdmissionError, ImguiContextConfig, ImguiContextError,
    ImguiContextRetired, ImguiContextRetirementId, ImguiContexts, ImguiFrame, ImguiPass,
    ImguiPassError, ImguiPluginInstallError, ImguiPrimaryChange, ImguiPrimaryPass,
    ImguiShutdownError, ImguiSystemConfigs, IntoImguiSystemConfigs,
};
pub use crate::input::{
    ImguiInputCapture, ImguiInputCaptureState, imgui_context_wants_keyboard_input,
    imgui_context_wants_pointer_input, imgui_context_wants_text_input,
    imgui_primary_wants_keyboard_input, imgui_primary_wants_pointer_input,
    imgui_primary_wants_text_input, imgui_window_wants_keyboard_input,
    imgui_window_wants_pointer_input, imgui_window_wants_text_input,
};
pub use crate::viewport::{
    ImguiViewportCamera, ImguiViewportId, ImguiViewportInstanceId, ImguiViewportWindow,
    ImguiViewportWindowConfig, ImguiViewportWindowConfigError,
};
pub use crate::{
    ContextId, ImguiDriverScheduleError, ImguiDriverSchedulePlacement, ImguiPlugin,
    ImguiPluginConfig,
};

#[cfg(feature = "render")]
pub use crate::ImguiRenderSystems;
#[cfg(feature = "bevy-ui")]
pub use crate::ImguiUiRenderOrder;
#[cfg(feature = "render")]
pub use crate::route::{
    ImguiCameraInputSource, ImguiDiagnostic, ImguiDiagnosticKind, ImguiDiagnostics,
    ImguiInputPolicy, ImguiInputRoute, ImguiInputSource, ImguiLogicalInputSource, ImguiRenderRoute,
};
#[cfg(feature = "render")]
pub use crate::{ImguiBevyTextures, ImguiTexture, ImguiTextureRegistrationError};

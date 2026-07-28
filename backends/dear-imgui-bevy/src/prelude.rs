//! Common imports for applications using the Bevy backend.

pub use crate::context::{
    ImguiContextAdmissionError, ImguiContextConfig, ImguiContextError, ImguiContexts, ImguiUi,
};
pub use crate::input::{
    ImguiInputCapture, ImguiInputCaptureState, ImguiInputSystems,
    imgui_context_wants_keyboard_input, imgui_context_wants_pointer_input,
    imgui_context_wants_text_input, imgui_primary_wants_keyboard_input,
    imgui_primary_wants_pointer_input, imgui_primary_wants_text_input,
    imgui_window_wants_keyboard_input, imgui_window_wants_pointer_input,
    imgui_window_wants_text_input,
};
pub use crate::schedule::ImguiPrimaryContextPass;
pub use crate::viewport::{
    ImguiViewportCamera, ImguiViewportId, ImguiViewportWindow, ImguiViewportWindowConfig,
    ImguiViewportWindowConfigError,
};
pub use crate::{ContextId, ImguiPlugin, ImguiPluginConfig};

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

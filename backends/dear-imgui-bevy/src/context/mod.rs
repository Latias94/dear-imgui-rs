//! Main-thread ownership and serial scheduling of Dear ImGui Contexts.

mod backend_contract;
mod driver;
mod lifecycle;
mod mailbox;
mod owner;
mod pass;
mod platform;
mod plugin;
mod registry;
mod retirement;
mod shutdown;
mod viewport_attachment;

#[cfg(feature = "render")]
pub use backend_contract::ImguiRendererOwnershipError;
pub(crate) use backend_contract::{BackendAttachment, ImguiActiveRendererContextError};
pub use backend_contract::{ImguiContextRemovalPendingReason, ImguiContextScopeError};
pub(crate) use driver::{drive_imgui_contexts, install_context_lifecycle};
#[cfg(feature = "render")]
pub(crate) use mailbox::{ImguiFrameMailbox, PendingFrame};
pub(crate) use owner::ContextOwner;
pub use pass::{
    ImguiFrame, ImguiPass, ImguiPassError, ImguiPrimaryPass, ImguiSystemConfigs,
    IntoImguiSystemConfigs,
};
pub(crate) use pass::{PassIdentity, run_pass};
#[cfg(feature = "render")]
pub(crate) use plugin::ImguiBackendRuntime;
pub use plugin::{ImguiPlugin, ImguiPluginConfig, ImguiPluginInstallError};
pub use registry::{
    ImguiContextAdmissionError, ImguiContextConfig, ImguiContextError, ImguiContextRetired,
    ImguiContextRetirementId, ImguiContexts, ImguiPrimaryChange,
};
pub(crate) use retirement::{
    ImguiContextRetirementSink, ImguiContextRetirements, begin_context_retirements,
    finish_context_retirements, install_context_retirements,
};
pub use shutdown::{ImguiAppExt, ImguiShutdownError};

#[cfg(test)]
#[path = "tests/lifecycle.rs"]
mod lifecycle_tests;

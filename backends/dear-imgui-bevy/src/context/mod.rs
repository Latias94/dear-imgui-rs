//! Main-thread ownership and serial scheduling of Dear ImGui Contexts.

mod active;
mod driver;
mod mailbox;
pub(crate) mod ownership;
mod platform;
mod registry;

pub use active::ImguiUi;
pub(crate) use active::{ActiveUiCapability, ImguiActiveUi};
pub(crate) use driver::{drive_imgui_contexts, install_context_lifecycle};
pub use mailbox::{ImguiContextFrameOutput, ImguiFrameOutput, ImguiFrameState};
#[cfg(feature = "render")]
pub(crate) use mailbox::{ImguiFrameMailbox, PendingFrame};
pub(crate) use ownership::ImguiActiveRendererContextError;
pub use registry::{
    ImguiContextAdmissionError, ImguiContextConfig, ImguiContextError, ImguiContexts,
};

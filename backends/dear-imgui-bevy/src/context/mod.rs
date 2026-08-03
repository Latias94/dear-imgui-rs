//! Main-thread ownership and serial scheduling of Dear ImGui Contexts.

mod driver;
mod lifecycle;
mod mailbox;
pub(crate) mod ownership;
mod pass;
mod platform;
mod registry;
mod shutdown;

pub(crate) use driver::{drive_imgui_contexts, install_context_lifecycle};
#[cfg(feature = "render")]
pub(crate) use mailbox::{ImguiFrameMailbox, PendingFrame};
pub(crate) use ownership::ImguiActiveRendererContextError;
pub use pass::{ImguiFrame, ImguiPass, ImguiPrimaryPass, ImguiSystem};
#[doc(hidden)]
pub use pass::{ImguiFrameAdapter, ImguiSystemMarker};
pub(crate) use pass::{PassIdentity, run_pass};
pub use registry::{
    ImguiContextAdmissionError, ImguiContextConfig, ImguiContextError, ImguiContexts,
    ImguiPrimaryChange,
};
pub use shutdown::{ImguiAppExt, ImguiShutdownError};

#[cfg(test)]
#[path = "tests/lifecycle.rs"]
mod lifecycle_tests;

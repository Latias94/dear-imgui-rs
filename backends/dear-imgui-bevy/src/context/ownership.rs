//! Compatibility surface for the original Context ownership module.
//!
//! Implementations live in focused sibling modules; this facade preserves existing crate-internal
//! paths and the public re-exports exposed from `lib.rs`.

pub(crate) use super::backend_contract::BackendAttachment;
#[cfg(feature = "render")]
pub(crate) use super::backend_contract::ImguiActiveRendererContextError;
#[cfg(feature = "render")]
pub use super::backend_contract::ImguiRendererOwnershipError;
pub use super::backend_contract::{ImguiContextRemovalPendingReason, ImguiContextScopeError};
pub(crate) use super::owner::ContextOwner;
#[cfg(feature = "render")]
pub(crate) use super::plugin::ImguiBackendRuntime;
pub use super::plugin::{ImguiPlugin, ImguiPluginConfig};
pub(crate) use super::retirement::{
    ImguiContextRetirementSink, ImguiContextRetirements, begin_context_retirements,
    finish_context_retirements, install_context_retirements,
};

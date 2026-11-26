//! ImGui context lifecycle
//!
//! Owns Dear ImGui contexts and confines them to one OS thread. Obtain a `Ui`
//! from `Context::frame()` and render using your chosen backend.

mod attachment;
pub(crate) mod binding;
mod clipboard;
mod core;
mod error;
mod fonts;
mod frame;
pub(crate) mod ime;
mod platform;
mod settings;
mod snapshot_hub;
#[cfg(test)]
mod tests;
mod texture_registry;
mod texture_sync;

pub use self::attachment::{
    ContextAttachment, ContextAttachmentDetachError, ContextAttachmentError,
    ContextAttachmentHandle, ContextAttachmentLease, ContextAttachmentPhase, ContextAttachmentRole,
    ContextAttachmentTeardownError, ContextDestroyed, ContextPlatformAttachmentRelease,
    ContextPlatformAttachmentReleaseError, ContextPlatformWindowTeardown,
    ContextPlatformWindowTeardownError, ContextTeardown,
};
pub use self::binding::{
    ContextAliveToken, ContextBinding, ContextBindingError, ContextId, ContextLifecycle,
    ContextThreadLease,
};
pub use self::core::Context;
pub use self::error::{ContextOperationError, ContextScopeError};
pub use self::frame::{
    FrameLifecycleStamp, FrameLifecycleState, FramePrepareOptions, FrameResult, FrameToken,
};
pub use self::ime::{ImePreedit, ImePreeditError};
pub use self::snapshot_hub::RendererTextureReset;
pub(crate) use self::texture_registry::SharedTextureRegistry;

// Dear ImGui is not thread-safe. Context stays on its owner thread; detached
// rendering moves only a Context-created FrameSnapshot across threads.

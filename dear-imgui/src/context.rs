//! ImGui context lifecycle
//!
//! Creates, manages and destroys the single active Dear ImGui context used by
//! the crate. Obtain a `Ui` each frame via `Context::frame()` and render using
//! your chosen backend. See struct-level docs for details and caveats about one
//! active context at a time.

mod attachment;
pub(crate) mod binding;
mod clipboard;
mod core;
mod fonts;
mod frame;
mod platform;
mod settings;
mod snapshot_hub;
mod suspended;
#[cfg(test)]
mod tests;
mod texture_registry;

pub use self::attachment::{
    ContextAttachment, ContextAttachmentError, ContextAttachmentLease, ContextAttachmentPhase,
    ContextAttachmentRole, ContextAttachmentTeardownError, ContextDestroyed, ContextTeardown,
};
pub use self::binding::{
    ContextAliveToken, ContextBinding, ContextBindingError, ContextId, ContextLifecycle,
};
pub use self::core::Context;
pub use self::frame::{FrameLifecycleState, FramePrepareOptions, FrameResult, FrameToken};
pub use self::snapshot_hub::RendererTextureReset;
pub use self::suspended::SuspendedContext;
pub(crate) use self::texture_registry::SharedTextureRegistry;

// Dear ImGui is not thread-safe. Context stays on its owner thread; detached
// rendering moves only a Context-created FrameSnapshot across threads.

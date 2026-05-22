//! ImGui context lifecycle
//!
//! Creates, manages and destroys the single active Dear ImGui context used by
//! the crate. Obtain a `Ui` each frame via `Context::frame()` and render using
//! your chosen backend. See struct-level docs for details and caveats about one
//! active context at a time.

mod binding;
mod clipboard;
mod core;
mod fonts;
mod frame;
mod platform;
mod settings;
mod suspended;
#[cfg(test)]
mod tests;
mod texture_registry;

pub use self::core::{Context, ContextAliveToken};
pub use self::frame::{FrameLifecycleState, FramePrepareOptions, FrameResult, FrameToken};
pub use self::suspended::SuspendedContext;
pub use self::texture_registry::RegisteredUserTexture;

pub(crate) use self::texture_registry::unregister_user_texture_from_all_contexts;

// Dear ImGui is not thread-safe. The Context must not be sent or shared across
// threads. If you need multi-threaded rendering, capture render data via
// OwnedDrawData and move that to another thread for rendering.

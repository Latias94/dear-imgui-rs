//! Trees and collapsing headers
//!
//! Tree nodes and collapsing headers for hierarchical content. See
//! `TreeNodeFlags` for customization options.
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]
mod builder;
mod entry;
mod id;
mod token;

pub use builder::TreeNode;
pub use id::TreeNodeId;
pub use token::TreeNodeToken;

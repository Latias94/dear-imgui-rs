//! Draw data structures for Dear ImGui rendering
//!
//! This module provides safe Rust wrappers around Dear ImGui's draw data structures,
//! which contain all the information needed to render a frame.

mod callbacks;
mod cmd;
mod core;
mod list;
#[cfg(test)]
mod tests;
mod textures;
mod vertex;

pub use cmd::{DrawCmd, DrawCmdIterator, DrawCmdParams, RawCallbackCommand};
pub use core::DrawData;
pub use list::{DrawList, DrawListIterator};
pub use vertex::{DrawIdx, DrawVert};

pub(crate) use callbacks::{StandardDrawCallback, classify_standard_draw_callback};

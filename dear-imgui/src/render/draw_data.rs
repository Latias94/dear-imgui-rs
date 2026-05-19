//! Draw data structures for Dear ImGui rendering
//!
//! This module provides safe Rust wrappers around Dear ImGui's draw data structures,
//! which contain all the information needed to render a frame.

mod callbacks;
mod cmd;
mod core;
mod list;
mod owned;
#[cfg(test)]
mod tests;
mod textures;
mod vertex;

pub use cmd::{DrawCmd, DrawCmdIterator, DrawCmdParams};
pub use core::DrawData;
pub use list::{DrawList, DrawListIterator, OwnedDrawList};
pub use owned::OwnedDrawData;
pub use textures::{TextureDataMut, TextureIterator, TextureMutCursor};
pub use vertex::{DrawIdx, DrawVert};

pub(crate) use callbacks::{
    StandardDrawCallback, assert_draw_list_cloneable, classify_standard_draw_callback,
};

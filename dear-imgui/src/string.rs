//! String helpers (ImString and scratch buffers)
//!
//! Utilities for working with strings across the Rust <-> Dear ImGui FFI
//! boundary.
//!
//! - `ImString`: an owned, growable UTF-8 string that maintains a trailing
//!   NUL byte as required by C APIs. Useful for zero-copy text editing via
//!   ImGui callbacks.
//! - `UiBuffer`: an internal scratch buffer used by [`Ui`] methods to stage
//!   temporary C strings for widget labels and hints.
//!
//! Example (zero-copy text input with `ImString`):
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! let mut s = ImString::with_capacity(256);
//! if ui.input_text_imstr("Edit", &mut s).build() {
//!     // edited in-place, no extra copies
//! }
//! ```
//!
//! The unused `ImStr` alias and allocating `im_str!` compatibility macro are intentionally absent:
//! ```compile_fail
//! let _: dear_imgui_rs::ImStr<'static> = std::borrow::Cow::Borrowed("text");
//! ```
//! ```compile_fail
//! let _ = dear_imgui_rs::im_str!("text");
//! ```
mod buffer;
mod im_string;
mod scratch;

#[cfg(test)]
mod tests;

pub use buffer::UiBuffer;
pub use im_string::ImString;
pub(crate) use scratch::tls_scratch_txt;
pub use scratch::{
    with_scratch_txt, with_scratch_txt_slice, with_scratch_txt_slice_with_opt,
    with_scratch_txt_three, with_scratch_txt_two,
};

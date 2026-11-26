//! Scratch string buffers
//!
//! [`UiBuffer`] stages temporary C strings for widget labels and hints.
//!
//! The unused `ImStr` alias and allocating `im_str!` compatibility macro are intentionally absent:
//! ```compile_fail
//! let _: dear_imgui_rs::ImStr<'static> = std::borrow::Cow::Borrowed("text");
//! ```
//! ```compile_fail
//! let _ = dear_imgui_rs::im_str!("text");
//! ```
mod buffer;
mod scratch;

#[cfg(test)]
mod tests;

pub use buffer::UiBuffer;
pub(crate) use scratch::{ScratchBuffers, with_scratch_txt_range};
pub use scratch::{
    with_scratch_txt, with_scratch_txt_slice, with_scratch_txt_slice_with_opt,
    with_scratch_txt_three, with_scratch_txt_two,
};

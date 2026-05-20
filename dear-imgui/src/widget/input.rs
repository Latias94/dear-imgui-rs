//! Text and scalar inputs
//!
//! Single-line and multi-line text inputs backed by `String` or `ImString`
//! (zero-copy), plus number input helpers. Builders provide flags and
//! callback hooks for validation and behavior tweaks.
//!
//! Quick examples:
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! // Text (String)
//! let mut s = String::from("hello");
//! ui.input_text("Name", &mut s).build();
//!
//! // Text (ImString, zero-copy)
//! let mut im = ImString::with_capacity(64);
//! ui.input_text_imstr("ImStr", &mut im).build();
//!
//! // Numbers
//! let mut i = 0i32;
//! let mut f = 1.0f32;
//! ui.input_int("Count", &mut i);
//! ui.input_float("Scale", &mut f);
//! ```
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]
// NOTE: Keep explicit `as i32`/`as u32` casts when bridging bindgen-generated flags into the
// Dear ImGui C ABI. Bindgen may represent the same C enum/typedef with different Rust integer
// types across platforms/toolchains; our wrappers intentionally pin the expected width/sign at
// the FFI call sites.
mod buffers;
mod callback_bridge;
mod callbacks;
mod entry;
mod multiline;
mod numeric;
mod single_line;
#[cfg(test)]
mod tests;
mod validation;

pub use callbacks::*;
pub use multiline::{InputTextMultiline, InputTextMultilineImStr, InputTextMultilineWithCb};
pub use numeric::*;
pub use single_line::{InputText, InputTextImStr};

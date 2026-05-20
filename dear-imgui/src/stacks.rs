//! Push/pop stacks (fonts, style, etc.)
//!
//! RAII-style wrappers for ImGui parameter stacks: fonts, style colors/vars and
//! more. Tokens returned by `Ui::push_*` pop automatically when dropped.
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions,
    clippy::unnecessary_cast
)]

mod font;
mod id;
mod layout;
mod style;

pub use font::FontStackToken;
pub use id::{FocusScopeToken, IdStackToken};
pub use layout::{ItemWidthStackToken, TextWrapPosStackToken};
pub use style::{ColorStackToken, StyleStackToken};

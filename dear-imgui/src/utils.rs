//! Miscellaneous utilities
//!
//! Helper flags and `Ui` extension methods for common queries (hovered/focused
//! checks, item rectangles, etc.). These are thin wrappers around Dear ImGui
//! functions for convenience and type safety.
//!

mod counts;
mod focus;
mod general;
mod geometry;
mod hover_flags;
mod input;
mod item;
mod logging;
mod style_color;
mod validation;
mod visibility;
mod window;

pub use focus::FocusedFlags;
pub use hover_flags::{ItemHoveredFlags, TooltipHoveredFlags, WindowHoveredFlags};
pub(crate) use hover_flags::{validate_item_hovered_flags, validate_tooltip_hovered_flags};
pub use logging::LogAutoOpenDepth;

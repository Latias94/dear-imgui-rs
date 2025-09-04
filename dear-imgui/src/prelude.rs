/// Prelude module for convenient imports
///
/// This module re-exports the most commonly used types and traits from the
/// Dear ImGui library. Import this module to get access to the most important
/// functionality with a single use statement.
///
/// # Example
///
/// ```rust
/// use dear_imgui::prelude::*;
///
/// // Now you have access to Context, Result, Vec2, Color, etc.
/// let mut ctx = Context::new()?;
/// ```
pub use crate::{Context, Frame, ImGuiError, Result};

pub use crate::types::{Color, Id, Vec2, Vec4};

pub use crate::window::{Window, WindowFlags};

pub use crate::ui::Ui;

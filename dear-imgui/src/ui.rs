/// UI building functionality
///
/// This module contains the main UI building interface that allows you to
/// create Dear ImGui widgets and controls. The actual widget implementations
/// are in the `widget` module.

use crate::frame::Frame;
use crate::style::{push_style_var, StyleVar, StyleVarToken};
use std::marker::PhantomData;

/// UI builder for creating Dear ImGui widgets
///
/// The `Ui` struct provides methods for creating all the standard Dear ImGui
/// widgets like buttons, text, sliders, etc. It is typically obtained from
/// a window or other container.
///
/// # Example
///
/// ```rust,no_run
/// # use dear_imgui::Context;
/// # let mut ctx = Context::new().unwrap();
/// # let mut frame = ctx.frame();
/// frame.window("Example").show(|ui| {
///     ui.text("Hello, world!");
///     if ui.button("Click me") {
///         println!("Button clicked!");
///     }
/// });
/// ```
pub struct Ui<'frame> {
    _frame: PhantomData<&'frame mut Frame<'frame>>,
}

impl<'frame> Ui<'frame> {
    /// Create a new UI builder (internal use only)
    pub(crate) fn new() -> Self {
        Self {
            _frame: PhantomData,
        }
    }

    /// Push a style variable to the stack
    ///
    /// Returns a token that will automatically pop the style variable when dropped.
    /// You can also manually call `pop()` on the token.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::*;
    /// # let ui = Ui::new();
    /// let _style = ui.push_style_var(StyleVar::Alpha(0.5));
    /// // UI elements here will be semi-transparent
    /// // Style is automatically popped when _style is dropped
    /// ```
    pub fn push_style_var(&self, style_var: StyleVar) -> StyleVarToken<'_> {
        push_style_var(style_var)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_creation() {
        let _ui = Ui::new();
    }
}

use crate::frame::Frame;
/// UI building functionality
///
/// This module contains the main UI building interface that allows you to
/// create Dear ImGui widgets and controls. The actual widget implementations
/// are in the `widget` module.
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_creation() {
        let _ui = Ui::new();
    }
}

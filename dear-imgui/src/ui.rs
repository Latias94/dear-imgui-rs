/// UI building functionality
///
/// This module contains the main UI building interface that allows you to
/// create Dear ImGui widgets and controls. The actual widget implementations
/// are in the `widget` module.
use crate::frame::Frame;
use crate::string::UiBuffer;
use crate::style::{push_style_var, StyleVar, StyleVarToken};
use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::os::raw::c_char;

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
    buffer: UnsafeCell<UiBuffer>,
}

impl<'frame> Ui<'frame> {
    /// Create a new UI builder (internal use only)
    pub(crate) fn new() -> Self {
        Self {
            _frame: PhantomData,
            buffer: UnsafeCell::new(UiBuffer::new(1024)), // 1KB default buffer
        }
    }

    /// Internal method to push a single text to our scratch buffer.
    ///
    /// This method follows the imgui-rs pattern for efficient string handling.
    /// It pushes the string to a temporary buffer and returns a pointer to it.
    pub(crate) fn scratch_txt(&self, txt: impl AsRef<str>) -> *const c_char {
        unsafe {
            let buffer = &mut *self.buffer.get();
            buffer.scratch_txt(txt)
        }
    }

    /// Internal method to push an optional text to our scratch buffer.
    pub(crate) fn scratch_txt_opt(&self, txt: Option<impl AsRef<str>>) -> *const c_char {
        unsafe {
            let buffer = &mut *self.buffer.get();
            buffer.scratch_txt_opt(txt)
        }
    }

    /// Internal method to push two texts to our scratch buffer.
    pub(crate) fn scratch_txt_two(
        &self,
        txt_0: impl AsRef<str>,
        txt_1: impl AsRef<str>,
    ) -> (*const c_char, *const c_char) {
        unsafe {
            let buffer = &mut *self.buffer.get();
            buffer.scratch_txt_two(txt_0, txt_1)
        }
    }

    /// Helper method, same as [`Self::scratch_txt_two`] but with one optional value
    pub(crate) fn scratch_txt_with_opt(
        &self,
        txt_0: impl AsRef<str>,
        txt_1: Option<impl AsRef<str>>,
    ) -> (*const c_char, *const c_char) {
        unsafe {
            let buffer = &mut *self.buffer.get();
            buffer.scratch_txt_with_opt(txt_0, txt_1)
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

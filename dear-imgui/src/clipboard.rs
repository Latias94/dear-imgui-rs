//! Clipboard support for Dear ImGui
//! 
//! Provides access to the system clipboard for text operations.

use dear_imgui_sys as sys;
use std::ffi::{CStr, CString};
use crate::ui::Ui;

/// Clipboard functionality for UI
impl<'frame> Ui<'frame> {
    /// Get text from the system clipboard
    /// 
    /// Returns the clipboard text as a String, or None if the clipboard is empty
    /// or contains non-text data.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// if let Some(text) = ui.get_clipboard_text() {
    ///     ui.text(format!("Clipboard: {}", text));
    /// } else {
    ///     ui.text("Clipboard is empty");
    /// }
    /// # true });
    /// ```
    pub fn get_clipboard_text(&self) -> Option<String> {
        unsafe {
            let ptr = sys::ImGui_GetClipboardText();
            if ptr.is_null() {
                None
            } else {
                let c_str = CStr::from_ptr(ptr);
                c_str.to_str().ok().map(|s| s.to_string())
            }
        }
    }
    
    /// Set text to the system clipboard
    /// 
    /// # Arguments
    /// 
    /// * `text` - The text to copy to the clipboard
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// if ui.button("Copy to Clipboard") {
    ///     ui.set_clipboard_text("Hello, clipboard!");
    /// }
    /// # true });
    /// ```
    pub fn set_clipboard_text(&mut self, text: impl AsRef<str>) {
        let text = text.as_ref();
        if let Ok(c_text) = CString::new(text) {
            unsafe {
                sys::ImGui_SetClipboardText(c_text.as_ptr());
            }
        }
    }
    
    /// Check if the clipboard contains text
    /// 
    /// Returns true if the clipboard contains text data that can be retrieved.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// if ui.has_clipboard_text() {
    ///     ui.text("Clipboard has text");
    ///     if ui.button("Paste") {
    ///         if let Some(text) = ui.get_clipboard_text() {
    ///             // Use the clipboard text
    ///             ui.text(format!("Pasted: {}", text));
    ///         }
    ///     }
    /// } else {
    ///     ui.text("Clipboard is empty");
    /// }
    /// # true });
    /// ```
    pub fn has_clipboard_text(&self) -> bool {
        unsafe {
            let ptr = sys::ImGui_GetClipboardText();
            !ptr.is_null()
        }
    }
    
    /// Create a text input with clipboard integration
    /// 
    /// This is a convenience method that creates a text input with built-in
    /// copy/paste functionality using Ctrl+C and Ctrl+V.
    /// 
    /// # Arguments
    /// 
    /// * `label` - The label for the input field
    /// * `buffer` - Mutable reference to the text buffer
    /// 
    /// # Returns
    /// 
    /// `true` if the text was modified, `false` otherwise
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let mut text = String::from("Hello");
    /// 
    /// if ui.input_text_with_clipboard("Text", &mut text) {
    ///     println!("Text changed to: {}", text);
    /// }
    /// 
    /// // Show clipboard operations
    /// if ui.button("Copy Text") {
    ///     ui.set_clipboard_text(&text);
    /// }
    /// ui.same_line();
    /// if ui.button("Paste Text") {
    ///     if let Some(clipboard_text) = ui.get_clipboard_text() {
    ///         text = clipboard_text;
    ///     }
    /// }
    /// # true });
    /// ```
    pub fn input_text_with_clipboard(&mut self, label: impl AsRef<str>, buffer: &mut String) -> bool {
        // First, handle clipboard shortcuts
        let io = unsafe { &*sys::ImGui_GetIO() };
        let ctrl_pressed = io.KeyCtrl;
        
        if ctrl_pressed {
            if unsafe { sys::ImGui_IsKeyPressed(sys::ImGuiKey_C as i32, false) } {
                // Ctrl+C - Copy selected text or all text to clipboard
                self.set_clipboard_text(&*buffer);
            } else if unsafe { sys::ImGui_IsKeyPressed(sys::ImGuiKey_V as i32, false) } {
                // Ctrl+V - Paste from clipboard
                if let Some(clipboard_text) = self.get_clipboard_text() {
                    *buffer = clipboard_text;
                    return true;
                }
            }
        }
        
        // Use regular input text
        self.input_text(label, buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Context;

    #[test]
    fn test_clipboard_operations() {
        let mut ctx = Context::new().expect("Failed to create context");
        let mut frame = ctx.frame();
        
        frame.window("Test").show(|ui| {
            // Test setting clipboard text
            ui.set_clipboard_text("Test clipboard content");
            
            // Test checking if clipboard has text
            let has_text = ui.has_clipboard_text();
            
            // Test getting clipboard text
            if let Some(text) = ui.get_clipboard_text() {
                assert_eq!(text, "Test clipboard content");
            }
            
            true
        });
    }

    #[test]
    fn test_clipboard_empty() {
        let mut ctx = Context::new().expect("Failed to create context");
        let mut frame = ctx.frame();
        
        frame.window("Test").show(|ui| {
            // Test with empty clipboard (might not be empty in real environment)
            let _has_text = ui.has_clipboard_text();
            let _text = ui.get_clipboard_text();
            
            true
        });
    }

    #[test]
    fn test_input_text_with_clipboard() {
        let mut ctx = Context::new().expect("Failed to create context");
        let mut frame = ctx.frame();
        
        frame.window("Test").show(|ui| {
            let mut text = String::from("Initial text");
            
            // Test clipboard-enabled input text
            ui.input_text_with_clipboard("Test Input", &mut text);
            
            true
        });
    }
}

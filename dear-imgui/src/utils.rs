//! Utility functions for Dear ImGui
//! 
//! This module provides various utility functions for text measurement,
//! coordinate calculations, and other common operations.

use dear_imgui_sys as sys;
use std::ffi::CString;
use crate::ui::Ui;
use crate::types::Vec2;

/// Utility functions for UI
impl<'frame> Ui<'frame> {
    /// Calculate the size of text when rendered
    /// 
    /// This function calculates how much space the given text will occupy
    /// when rendered with the current font.
    /// 
    /// # Arguments
    /// 
    /// * `text` - The text to measure
    /// * `wrap_width` - Optional text wrapping width. Use `None` for no wrapping.
    /// 
    /// # Returns
    /// 
    /// A `Vec2` containing the width and height of the text in pixels.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let text_size = ui.calc_text_size("Hello, World!", None);
    /// ui.text(format!("Text size: {:.1} x {:.1}", text_size.x, text_size.y));
    /// 
    /// // With text wrapping
    /// let wrapped_size = ui.calc_text_size("This is a long text that will wrap", Some(200.0));
    /// ui.text(format!("Wrapped size: {:.1} x {:.1}", wrapped_size.x, wrapped_size.y));
    /// # true });
    /// ```
    pub fn calc_text_size(&self, text: impl AsRef<str>, wrap_width: Option<f32>) -> Vec2 {
        let text = text.as_ref();
        let c_text = CString::new(text).unwrap_or_default();
        let wrap_width = wrap_width.unwrap_or(-1.0);
        
        unsafe {
            let size = sys::ImGui_CalcTextSize(
                c_text.as_ptr(),
                std::ptr::null(), // text_end (null means auto-detect)
                false, // hide_text_after_double_hash
                wrap_width,
            );
            Vec2::new(size.x, size.y)
        }
    }
    
    /// Get the current cursor position in window coordinates
    /// 
    /// Returns the position where the next widget will be placed.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let cursor_pos = ui.get_cursor_pos();
    /// ui.text(format!("Cursor at: {:.1}, {:.1}", cursor_pos.x, cursor_pos.y));
    /// # true });
    /// ```
    pub fn get_cursor_pos(&self) -> Vec2 {
        unsafe {
            let pos = sys::ImGui_GetCursorPos();
            Vec2::new(pos.x, pos.y)
        }
    }
    
    /// Set the cursor position in window coordinates
    /// 
    /// # Arguments
    /// 
    /// * `pos` - The new cursor position
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// ui.set_cursor_pos([100.0, 50.0]);
    /// ui.text("This text is at position (100, 50)");
    /// # true });
    /// ```
    pub fn set_cursor_pos(&mut self, pos: impl Into<Vec2>) {
        let pos = pos.into();
        unsafe {
            let vec = sys::ImVec2 { x: pos.x, y: pos.y };
            sys::ImGui_SetCursorPos(&vec);
        }
    }
    

    
    /// Get the available content region size
    /// 
    /// Returns the size of the content region that is available for widgets.
    /// This excludes window padding, scrollbars, etc.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let content_size = ui.get_content_region_avail();
    /// ui.text(format!("Available: {:.1} x {:.1}", content_size.x, content_size.y));
    /// # true });
    /// ```
    pub fn get_content_region_avail(&self) -> Vec2 {
        unsafe {
            let size = sys::ImGui_GetContentRegionAvail();
            Vec2::new(size.x, size.y)
        }
    }
    
    /// Get the maximum content region size
    /// 
    /// Returns the maximum size of the content region.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let max_size = ui.get_content_region_max();
    /// ui.text(format!("Max region: {:.1} x {:.1}", max_size.x, max_size.y));
    /// # true });
    /// ```
    pub fn get_content_region_max(&self) -> Vec2 {
        unsafe {
            let size = sys::ImGui_GetContentRegionMax();
            Vec2::new(size.x, size.y)
        }
    }
    
    /// Get the current window position
    /// 
    /// Returns the position of the current window in screen coordinates.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let window_pos = ui.get_window_pos();
    /// ui.text(format!("Window at: {:.1}, {:.1}", window_pos.x, window_pos.y));
    /// # true });
    /// ```
    pub fn get_window_pos(&self) -> Vec2 {
        unsafe {
            let pos = sys::ImGui_GetWindowPos();
            Vec2::new(pos.x, pos.y)
        }
    }
    
    /// Get the current window size
    /// 
    /// Returns the size of the current window.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let window_size = ui.get_window_size();
    /// ui.text(format!("Window size: {:.1} x {:.1}", window_size.x, window_size.y));
    /// # true });
    /// ```
    pub fn get_window_size(&self) -> Vec2 {
        unsafe {
            let size = sys::ImGui_GetWindowSize();
            Vec2::new(size.x, size.y)
        }
    }
    
    /// Get the height of a single line of text
    /// 
    /// Returns the height of text rendered with the current font.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let line_height = ui.get_text_line_height();
    /// ui.text(format!("Line height: {:.1}", line_height));
    /// # true });
    /// ```
    pub fn get_text_line_height(&self) -> f32 {
        unsafe {
            sys::ImGui_GetTextLineHeight()
        }
    }
    
    /// Get the height of a single line of text with spacing
    /// 
    /// Returns the height of text plus the spacing between lines.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let line_height_with_spacing = ui.get_text_line_height_with_spacing();
    /// ui.text(format!("Line height with spacing: {:.1}", line_height_with_spacing));
    /// # true });
    /// ```
    pub fn get_text_line_height_with_spacing(&self) -> f32 {
        unsafe {
            sys::ImGui_GetTextLineHeightWithSpacing()
        }
    }
    
    /// Get the height of the current frame
    /// 
    /// Returns the height of UI elements like buttons, input fields, etc.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let frame_height = ui.get_frame_height();
    /// ui.text(format!("Frame height: {:.1}", frame_height));
    /// # true });
    /// ```
    pub fn get_frame_height(&self) -> f32 {
        unsafe {
            sys::ImGui_GetFrameHeight()
        }
    }
    
    /// Get the height of the current frame with spacing
    /// 
    /// Returns the height of UI elements plus the spacing between them.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let frame_height_with_spacing = ui.get_frame_height_with_spacing();
    /// ui.text(format!("Frame height with spacing: {:.1}", frame_height_with_spacing));
    /// # true });
    /// ```
    pub fn get_frame_height_with_spacing(&self) -> f32 {
        unsafe {
            sys::ImGui_GetFrameHeightWithSpacing()
        }
    }
    
    /// Check if a rectangle contains a point
    /// 
    /// Utility function to test if a point is inside a rectangle.
    /// 
    /// # Arguments
    /// 
    /// * `rect_min` - Top-left corner of the rectangle
    /// * `rect_max` - Bottom-right corner of the rectangle  
    /// * `point` - The point to test
    /// 
    /// # Returns
    /// 
    /// `true` if the point is inside the rectangle, `false` otherwise
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let rect_min = [10.0, 10.0];
    /// let rect_max = [100.0, 50.0];
    /// let point = [50.0, 30.0];
    /// 
    /// if ui.is_point_in_rect(rect_min, rect_max, point) {
    ///     ui.text("Point is inside rectangle");
    /// } else {
    ///     ui.text("Point is outside rectangle");
    /// }
    /// # true });
    /// ```
    pub fn is_point_in_rect(&self, rect_min: impl Into<Vec2>, rect_max: impl Into<Vec2>, point: impl Into<Vec2>) -> bool {
        let rect_min = rect_min.into();
        let rect_max = rect_max.into();
        let point = point.into();
        
        point.x >= rect_min.x && point.x <= rect_max.x && 
        point.y >= rect_min.y && point.y <= rect_max.y
    }
}

#[cfg(test)]
mod tests {
    use crate::Context;

    #[test]
    fn test_text_size_calculation() {
        let mut ctx = Context::new().expect("Failed to create context");
        let mut frame = ctx.frame();
        
        frame.window("Test").show(|ui| {
            let size = ui.calc_text_size("Hello", None);
            assert!(size.x > 0.0);
            assert!(size.y > 0.0);
            
            true
        });
    }

    #[test]
    fn test_cursor_operations() {
        let mut ctx = Context::new().expect("Failed to create context");
        let mut frame = ctx.frame();
        
        frame.window("Test").show(|ui| {
            let _initial_pos = ui.get_cursor_pos();
            
            ui.set_cursor_pos([100.0, 50.0]);
            let new_pos = ui.get_cursor_pos();
            
            assert!((new_pos.x - 100.0).abs() < 0.1);
            assert!((new_pos.y - 50.0).abs() < 0.1);
            
            true
        });
    }

    #[test]
    fn test_window_info() {
        let mut ctx = Context::new().expect("Failed to create context");
        let mut frame = ctx.frame();
        
        frame.window("Test").show(|ui| {
            let _window_pos = ui.get_window_pos();
            let window_size = ui.get_window_size();
            let content_avail = ui.get_content_region_avail();
            
            // Basic sanity checks
            assert!(window_size.x > 0.0);
            assert!(window_size.y > 0.0);
            assert!(content_avail.x >= 0.0);
            assert!(content_avail.y >= 0.0);
            
            true
        });
    }

    #[test]
    fn test_text_metrics() {
        let mut ctx = Context::new().expect("Failed to create context");
        let mut frame = ctx.frame();
        
        frame.window("Test").show(|ui| {
            let line_height = ui.get_text_line_height();
            let line_height_with_spacing = ui.get_text_line_height_with_spacing();
            let frame_height = ui.get_frame_height();
            let frame_height_with_spacing = ui.get_frame_height_with_spacing();
            
            assert!(line_height > 0.0);
            assert!(line_height_with_spacing >= line_height);
            assert!(frame_height > 0.0);
            assert!(frame_height_with_spacing >= frame_height);
            
            true
        });
    }

    #[test]
    fn test_point_in_rect() {
        let mut ctx = Context::new().expect("Failed to create context");
        let mut frame = ctx.frame();
        
        frame.window("Test").show(|ui| {
            // Point inside rectangle
            assert!(ui.is_point_in_rect([0.0, 0.0], [100.0, 100.0], [50.0, 50.0]));
            
            // Point outside rectangle
            assert!(!ui.is_point_in_rect([0.0, 0.0], [100.0, 100.0], [150.0, 50.0]));
            
            // Point on edge (should be inside)
            assert!(ui.is_point_in_rect([0.0, 0.0], [100.0, 100.0], [100.0, 100.0]));
            
            true
        });
    }
}

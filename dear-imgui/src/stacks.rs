//! Stack management system for Dear ImGui
//!
//! This module provides safe stack management for Dear ImGui's various state stacks.
//! It uses the token system to ensure that stack operations are properly paired
//! and automatically cleaned up.

use dear_imgui_sys as sys;
use crate::ui::Ui;
use crate::style::StyleColor;
use crate::fonts::Font;

// Create token types for different stack operations
crate::create_token!(
    /// Tracks a font pushed to the font stack that can be popped by calling `.end()`
    /// or by dropping.
    pub struct FontStackToken<'ui>;

    /// Pops a change from the font stack.
    drop { sys::ImGui_PopFont() }
);

/// Tracks style colors pushed to the color stack
pub struct StyleColorStackToken<'ui> {
    count: i32,
    _phantom: std::marker::PhantomData<&'ui crate::ui::Ui<'ui>>,
}

impl<'ui> StyleColorStackToken<'ui> {
    /// Creates a new style color stack token
    pub(crate) fn new(count: i32) -> Self {
        Self {
            count,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Manually end the token
    pub fn end(self) {
        // Drop will handle the cleanup
    }
}

impl Drop for StyleColorStackToken<'_> {
    fn drop(&mut self) {
        unsafe {
            sys::ImGui_PopStyleColor(self.count);
        }
    }
}

/// Tracks style variables pushed to the style stack
pub struct StyleVarStackToken<'ui> {
    count: i32,
    _phantom: std::marker::PhantomData<&'ui crate::ui::Ui<'ui>>,
}

impl<'ui> StyleVarStackToken<'ui> {
    /// Creates a new style variable stack token
    pub(crate) fn new(count: i32) -> Self {
        Self {
            count,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Manually end the token
    pub fn end(self) {
        // Drop will handle the cleanup
    }
}

impl Drop for StyleVarStackToken<'_> {
    fn drop(&mut self) {
        unsafe {
            sys::ImGui_PopStyleVar(self.count);
        }
    }
}

crate::create_token!(
    /// Tracks an item width pushed to the item width stack that can be popped by calling `.end()`
    /// or by dropping.
    pub struct ItemWidthStackToken<'ui>;

    /// Pops a change from the item width stack.
    drop { sys::ImGui_PopItemWidth() }
);

crate::create_token!(
    /// Tracks a text wrap position pushed to the text wrap stack that can be popped by calling `.end()`
    /// or by dropping.
    pub struct TextWrapPosStackToken<'ui>;

    /// Pops a change from the text wrap stack.
    drop { sys::ImGui_PopTextWrapPos() }
);

/// Stack management functionality for UI
impl<'frame> Ui<'frame> {
    /// Push a font onto the font stack
    ///
    /// This changes the current font for all subsequent text rendering
    /// until the returned token is dropped.
    ///
    /// # Arguments
    ///
    /// * `font` - The font to push onto the stack
    ///
    /// # Returns
    ///
    /// A token that automatically pops the font when dropped
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// // Assuming we have a font loaded
    /// if let Some(font) = ui.fonts().get_font(0) {
    ///     let _font_token = ui.push_font(font);
    ///     ui.text("This text uses the pushed font");
    ///     // Font is automatically popped when _font_token is dropped
    /// }
    /// # true });
    /// ```
    pub fn push_font(&mut self, font: &Font) -> FontStackToken<'_> {
        unsafe {
            // ImGui_PushFont takes font and font_size_base_unscaled
            sys::ImGui_PushFont(font.raw(), 0.0);
        }
        FontStackToken::new()
    }

    /// Push a style color onto the color stack
    /// 
    /// This temporarily changes a style color until the returned token is dropped.
    /// 
    /// # Arguments
    /// 
    /// * `color_id` - The style color to modify
    /// * `color` - The new color value
    /// 
    /// # Returns
    /// 
    /// A token that automatically pops the color when dropped
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let _color_token = ui.push_style_color(StyleColor::Button, [1.0, 0.0, 0.0, 1.0]);
    /// ui.button("Red Button");
    /// // Color is automatically popped when _color_token is dropped
    /// # true });
    /// ```
    pub fn push_style_color(&mut self, color_id: StyleColor, color: [f32; 4]) -> StyleColorStackToken<'_> {
        unsafe {
            let vec4 = sys::ImVec4 {
                x: color[0],
                y: color[1],
                z: color[2],
                w: color[3],
            };
            sys::ImGui_PushStyleColor1(color_id as i32, &vec4);
        }
        StyleColorStackToken::new(1)
    }

    /// Push multiple style colors onto the color stack
    /// 
    /// This is more efficient than calling `push_style_color` multiple times.
    /// 
    /// # Arguments
    /// 
    /// * `colors` - Slice of (color_id, color) pairs
    /// 
    /// # Returns
    /// 
    /// A token that automatically pops all colors when dropped
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let colors = [
    ///     (StyleColor::Button, [1.0, 0.0, 0.0, 1.0]),
    ///     (StyleColor::ButtonHovered, [1.0, 0.5, 0.5, 1.0]),
    ///     (StyleColor::ButtonActive, [0.8, 0.0, 0.0, 1.0]),
    /// ];
    /// let _color_token = ui.push_style_colors(&colors);
    /// ui.button("Styled Button");
    /// // All colors are automatically popped when _color_token is dropped
    /// # true });
    /// ```
    pub fn push_style_colors(&mut self, colors: &[(StyleColor, [f32; 4])]) -> StyleColorStackToken<'_> {
        for &(color_id, color) in colors {
            unsafe {
                let vec4 = sys::ImVec4 {
                    x: color[0],
                    y: color[1],
                    z: color[2],
                    w: color[3],
                };
                sys::ImGui_PushStyleColor1(color_id as i32, &vec4);
            }
        }
        StyleColorStackToken::new(colors.len() as i32)
    }

    /// Push a style variable (float) onto the style stack
    ///
    /// This temporarily changes a style variable until the returned token is dropped.
    ///
    /// # Arguments
    ///
    /// * `var_id` - The style variable ID (from ImGuiStyleVar enum)
    /// * `value` - The new float value
    ///
    /// # Returns
    ///
    /// A token that automatically pops the variable when dropped
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let _var_token = ui.push_style_var_f32(0, 0.5); // Alpha
    /// ui.text("Semi-transparent text");
    /// // Style variable is automatically popped when _var_token is dropped
    /// # true });
    /// ```
    pub fn push_style_var_f32(&mut self, var_id: i32, value: f32) -> StyleVarStackToken<'_> {
        unsafe {
            sys::ImGui_PushStyleVar(var_id, value);
        }
        StyleVarStackToken::new(1)
    }

    /// Push a style variable (Vec2) onto the style stack
    ///
    /// # Arguments
    ///
    /// * `var_id` - The style variable ID (from ImGuiStyleVar enum)
    /// * `value` - The new Vec2 value
    ///
    /// # Returns
    ///
    /// A token that automatically pops the variable when dropped
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let _var_token = ui.push_style_var_vec2(1, [10.0, 10.0]); // WindowPadding
    /// ui.text("Text with custom window padding");
    /// // Style variable is automatically popped when _var_token is dropped
    /// # true });
    /// ```
    pub fn push_style_var_vec2(&mut self, var_id: i32, value: [f32; 2]) -> StyleVarStackToken<'_> {
        unsafe {
            let vec2 = sys::ImVec2 {
                x: value[0],
                y: value[1],
            };
            sys::ImGui_PushStyleVar1(var_id, &vec2);
        }
        StyleVarStackToken::new(1)
    }

    /// Push an item width onto the item width stack
    /// 
    /// This sets the width for the next item(s) until the token is dropped.
    /// 
    /// # Arguments
    /// 
    /// * `width` - The item width (0.0 = default, >0.0 = fixed width, <0.0 = relative to right edge)
    /// 
    /// # Returns
    /// 
    /// A token that automatically pops the item width when dropped
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let _width_token = ui.push_item_width(200.0);
    /// ui.input_text("Wide input", &mut String::new());
    /// // Item width is automatically popped when _width_token is dropped
    /// # true });
    /// ```
    pub fn push_item_width(&mut self, width: f32) -> ItemWidthStackToken<'_> {
        unsafe {
            sys::ImGui_PushItemWidth(width);
        }
        ItemWidthStackToken::new()
    }

    /// Push a text wrap position onto the text wrap stack
    /// 
    /// This sets the text wrap position for subsequent text until the token is dropped.
    /// 
    /// # Arguments
    /// 
    /// * `wrap_pos_x` - The wrap position (0.0 = no wrap, >0.0 = wrap at position, <0.0 = wrap relative to right edge)
    /// 
    /// # Returns
    /// 
    /// A token that automatically pops the text wrap position when dropped
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let _wrap_token = ui.push_text_wrap_pos(200.0);
    /// ui.text("This is a very long text that will wrap at 200 pixels");
    /// // Text wrap position is automatically popped when _wrap_token is dropped
    /// # true });
    /// ```
    pub fn push_text_wrap_pos(&mut self, wrap_pos_x: f32) -> TextWrapPosStackToken<'_> {
        unsafe {
            sys::ImGui_PushTextWrapPos(wrap_pos_x);
        }
        TextWrapPosStackToken::new()
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_token_types() {
        // Test that token types are created correctly
        // These tests mainly verify that the macros work and tokens can be created

        let _font_token = FontStackToken::new();
        let _color_token = StyleColorStackToken::new(1);
        let _var_token = StyleVarStackToken::new(1);
        let _width_token = ItemWidthStackToken::new();
        let _wrap_token = TextWrapPosStackToken::new();

        // If we get here, all token types were created successfully
    }

    // Note: Integration tests with actual UI would require a full context setup
    // and are better suited for the examples or integration test suite
}

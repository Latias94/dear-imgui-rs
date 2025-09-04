//! Keyboard navigation and accessibility features

use crate::ui::Ui;
use dear_imgui_sys as sys;

/// Navigation and accessibility features
///
/// This module provides enhanced keyboard navigation and accessibility support
/// for Dear ImGui applications.

/// Navigation direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavDirection {
    /// Navigate left
    Left,
    /// Navigate right
    Right,
    /// Navigate up
    Up,
    /// Navigate down
    Down,
}

/// Navigation input
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavInput {
    /// Activate/select current item
    Activate,
    /// Cancel/close current item
    Cancel,
    /// Navigate to previous item
    Previous,
    /// Navigate to next item
    Next,
    /// Move focus left
    FocusLeft,
    /// Move focus right
    FocusRight,
    /// Move focus up
    FocusUp,
    /// Move focus down
    FocusDown,
}

/// # Navigation and Accessibility
impl<'frame> Ui<'frame> {
    /// Set keyboard focus to the next widget
    ///
    /// This is useful for programmatically controlling focus flow.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::Context;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Navigation Demo").show(|ui| {
    /// if ui.button("Focus Next") {
    ///     ui.set_keyboard_focus_here(0);
    /// }
    /// ui.input_text("Input 1", &mut String::new());
    /// ui.input_text("Input 2", &mut String::new());
    /// # });
    /// ```
    pub fn set_keyboard_focus_here(&mut self, offset: i32) {
        unsafe {
            sys::ImGui_SetKeyboardFocusHere(offset);
        }
    }

    /// Check if any item is focused
    ///
    /// # Returns
    ///
    /// `true` if any item in the current window has keyboard focus
    pub fn is_any_item_focused(&self) -> bool {
        unsafe { sys::ImGui_IsAnyItemFocused() }
    }

    /// Check if any item is active
    ///
    /// # Returns
    ///
    /// `true` if any item in the current window is active
    pub fn is_any_item_active(&self) -> bool {
        unsafe { sys::ImGui_IsAnyItemActive() }
    }

    /// Check if the current item is visible
    ///
    /// # Returns
    ///
    /// `true` if the current item is visible (not clipped)
    pub fn is_item_visible(&self) -> bool {
        unsafe { sys::ImGui_IsItemVisible() }
    }

    /// Check if the current item was clicked
    ///
    /// # Arguments
    ///
    /// * `mouse_button` - Mouse button to check (0 = left, 1 = right, 2 = middle)
    ///
    /// # Returns
    ///
    /// `true` if the current item was clicked with the specified mouse button
    pub fn is_item_clicked(&self, mouse_button: i32) -> bool {
        unsafe { sys::ImGui_IsItemClicked(mouse_button) }
    }

    /// Check if the current item is being edited
    ///
    /// # Returns
    ///
    /// `true` if the current item is being edited (e.g., text input)
    pub fn is_item_edited(&self) -> bool {
        unsafe { sys::ImGui_IsItemEdited() }
    }

    /// Check if the current item was activated
    ///
    /// # Returns
    ///
    /// `true` if the current item was activated (e.g., button pressed, checkbox toggled)
    pub fn is_item_activated(&self) -> bool {
        unsafe { sys::ImGui_IsItemActivated() }
    }

    /// Check if the current item was deactivated
    ///
    /// # Returns
    ///
    /// `true` if the current item was deactivated after being active
    pub fn is_item_deactivated(&self) -> bool {
        unsafe { sys::ImGui_IsItemDeactivated() }
    }

    /// Check if the current item was deactivated after being edited
    ///
    /// # Returns
    ///
    /// `true` if the current item was deactivated after being edited
    pub fn is_item_deactivated_after_edit(&self) -> bool {
        unsafe { sys::ImGui_IsItemDeactivatedAfterEdit() }
    }

    /// Enable or disable keyboard navigation for the current window
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable keyboard navigation
    pub fn set_nav_enabled(&mut self, enabled: bool) {
        // Note: This is a conceptual function - actual implementation would depend on
        // specific ImGui version and available APIs
        if enabled {
            // Enable navigation
        } else {
            // Disable navigation
        }
    }

    /// Check if keyboard navigation is currently enabled
    ///
    /// # Returns
    ///
    /// `true` if keyboard navigation is enabled
    pub fn is_nav_enabled(&self) -> bool {
        // Note: This is a conceptual function - actual implementation would depend on
        // specific ImGui version and available APIs
        true // Default to enabled
    }

    /// Get the current navigation input state
    ///
    /// # Arguments
    ///
    /// * `input` - Navigation input to check
    ///
    /// # Returns
    ///
    /// `true` if the navigation input is currently active
    pub fn is_nav_input_down(&self, _input: NavInput) -> bool {
        // Note: Navigation input APIs might not be available in all ImGui versions
        // This is a placeholder implementation
        false
    }

    /// Check if a navigation input was just pressed
    ///
    /// # Arguments
    ///
    /// * `input` - Navigation input to check
    ///
    /// # Returns
    ///
    /// `true` if the navigation input was just pressed this frame
    pub fn is_nav_input_pressed(&self, _input: NavInput) -> bool {
        // Note: Navigation input APIs might not be available in all ImGui versions
        // This is a placeholder implementation
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nav_direction_enum() {
        assert_eq!(NavDirection::Left, NavDirection::Left);
        assert_ne!(NavDirection::Left, NavDirection::Right);
    }

    #[test]
    fn test_nav_input_enum() {
        assert_eq!(NavInput::Activate, NavInput::Activate);
        assert_ne!(NavInput::Activate, NavInput::Cancel);
    }
}

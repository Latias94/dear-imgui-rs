//! Debug windows and development tools for Dear ImGui
//!
//! This module provides access to Dear ImGui's built-in debug and development tools,
//! including the demo window, metrics window, debug log, and stack tool.

use crate::ui::Ui;
use dear_imgui_sys as sys;
use std::ffi::CString;

/// Debug window functionality for UI
impl<'frame> Ui<'frame> {
    /// Show the Dear ImGui demo window
    ///
    /// This window demonstrates most of Dear ImGui's features and serves as a
    /// comprehensive example and testing ground. It's extremely useful for
    /// learning the API and testing functionality.
    ///
    /// # Arguments
    ///
    /// * `open` - Mutable reference to a boolean controlling window visibility.
    ///            Set to `None` if you don't want a close button.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let mut show_demo = true;
    ///
    /// // Show demo window with close button
    /// ui.show_demo_window(Some(&mut show_demo));
    ///
    /// // Show demo window without close button (always visible)
    /// ui.show_demo_window(None);
    /// # true });
    /// ```
    pub fn show_demo_window(&mut self, open: Option<&mut bool>) {
        unsafe {
            match open {
                Some(open_ref) => {
                    sys::ImGui_ShowDemoWindow(open_ref as *mut bool);
                }
                None => {
                    sys::ImGui_ShowDemoWindow(std::ptr::null_mut());
                }
            }
        }
    }

    /// Show the Dear ImGui metrics/debugger window
    ///
    /// This window shows detailed information about Dear ImGui's internal state,
    /// including performance metrics, memory usage, draw call information,
    /// and various debugging tools.
    ///
    /// # Arguments
    ///
    /// * `open` - Mutable reference to a boolean controlling window visibility.
    ///            Set to `None` if you don't want a close button.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let mut show_metrics = true;
    ///
    /// // Show metrics window with close button
    /// ui.show_metrics_window(Some(&mut show_metrics));
    ///
    /// // Show metrics window without close button
    /// ui.show_metrics_window(None);
    /// # true });
    /// ```
    pub fn show_metrics_window(&mut self, open: Option<&mut bool>) {
        unsafe {
            match open {
                Some(open_ref) => {
                    sys::ImGui_ShowMetricsWindow(open_ref as *mut bool);
                }
                None => {
                    sys::ImGui_ShowMetricsWindow(std::ptr::null_mut());
                }
            }
        }
    }

    /// Show the Dear ImGui debug log window
    ///
    /// This window displays Dear ImGui's internal debug log, which includes
    /// warnings, errors, and other diagnostic information. Very useful for
    /// debugging issues with your UI.
    ///
    /// # Arguments
    ///
    /// * `open` - Mutable reference to a boolean controlling window visibility.
    ///            Set to `None` if you don't want a close button.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let mut show_debug_log = true;
    ///
    /// // Show debug log window with close button
    /// ui.show_debug_log_window(Some(&mut show_debug_log));
    ///
    /// // Show debug log window without close button
    /// ui.show_debug_log_window(None);
    /// # true });
    /// ```
    pub fn show_debug_log_window(&mut self, open: Option<&mut bool>) {
        unsafe {
            match open {
                Some(open_ref) => {
                    sys::ImGui_ShowDebugLogWindow(open_ref as *mut bool);
                }
                None => {
                    sys::ImGui_ShowDebugLogWindow(std::ptr::null_mut());
                }
            }
        }
    }

    /// Show the Dear ImGui ID stack tool window
    ///
    /// This window shows the current ID stack, which is extremely useful for
    /// debugging ID conflicts and understanding how Dear ImGui generates
    /// unique IDs for widgets.
    ///
    /// # Arguments
    ///
    /// * `open` - Mutable reference to a boolean controlling window visibility.
    ///            Set to `None` if you don't want a close button.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let mut show_stack_tool = true;
    ///
    /// // Show stack tool window with close button
    /// ui.show_stack_tool_window(Some(&mut show_stack_tool));
    ///
    /// // Show stack tool window without close button
    /// ui.show_stack_tool_window(None);
    /// # true });
    /// ```
    pub fn show_stack_tool_window(&mut self, open: Option<&mut bool>) {
        unsafe {
            match open {
                Some(open_ref) => {
                    sys::ImGui_ShowIDStackToolWindow(open_ref as *mut bool);
                }
                None => {
                    sys::ImGui_ShowIDStackToolWindow(std::ptr::null_mut());
                }
            }
        }
    }

    /// Show the Dear ImGui about window
    ///
    /// This window displays information about the Dear ImGui version,
    /// build configuration, and credits.
    ///
    /// # Arguments
    ///
    /// * `open` - Mutable reference to a boolean controlling window visibility.
    ///            Set to `None` if you don't want a close button.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let mut show_about = true;
    ///
    /// // Show about window with close button
    /// ui.show_about_window(Some(&mut show_about));
    ///
    /// // Show about window without close button
    /// ui.show_about_window(None);
    /// # true });
    /// ```
    pub fn show_about_window(&mut self, open: Option<&mut bool>) {
        unsafe {
            match open {
                Some(open_ref) => {
                    sys::ImGui_ShowAboutWindow(open_ref as *mut bool);
                }
                None => {
                    sys::ImGui_ShowAboutWindow(std::ptr::null_mut());
                }
            }
        }
    }

    /// Show the Dear ImGui style editor window
    ///
    /// This window provides a comprehensive interface for editing Dear ImGui's
    /// style settings, including colors, sizes, and other visual properties.
    /// Changes are applied in real-time.
    ///
    /// # Arguments
    ///
    /// * `open` - Mutable reference to a boolean controlling window visibility.
    ///            Set to `None` if you don't want a close button.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let mut show_style_editor = true;
    ///
    /// // Show style editor window with close button
    /// ui.show_style_editor(Some(&mut show_style_editor));
    ///
    /// // Show style editor window without close button
    /// ui.show_style_editor(None);
    /// # true });
    /// ```
    pub fn show_style_editor(&mut self, open: Option<&mut bool>) {
        unsafe {
            match open {
                Some(open_ref) => {
                    // ImGui_ShowStyleEditor doesn't take an open parameter
                    // We'll need to wrap it in a window if we want close functionality
                    if *open_ref && self.begin_window("Style Editor", Some(open_ref), 0) {
                        sys::ImGui_ShowStyleEditor(std::ptr::null_mut());
                        self.end_window();
                    }
                }
                None => {
                    sys::ImGui_ShowStyleEditor(std::ptr::null_mut());
                }
            }
        }
    }

    /// Show the Dear ImGui user guide window
    ///
    /// This window displays a comprehensive user guide with tips, tricks,
    /// and explanations of Dear ImGui concepts and features.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// // Show user guide (usually called from within another window)
    /// ui.show_user_guide();
    /// # true });
    /// ```
    pub fn show_user_guide(&mut self) {
        unsafe {
            sys::ImGui_ShowUserGuide();
        }
    }

    /// Get the Dear ImGui version string
    ///
    /// Returns the version string of the Dear ImGui library being used.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("Test").show(|ui| {
    /// let version = ui.get_version();
    /// ui.text(format!("Dear ImGui version: {}", version));
    /// # true });
    /// ```
    pub fn get_version(&self) -> &'static str {
        unsafe {
            let version_ptr = sys::ImGui_GetVersion();
            let c_str = std::ffi::CStr::from_ptr(version_ptr);
            c_str.to_str().unwrap_or("Unknown")
        }
    }

    // Helper method for creating windows (used internally)
    fn begin_window(&mut self, title: &str, open: Option<&mut bool>, flags: i32) -> bool {
        let title_cstr = CString::new(title).unwrap_or_default();
        unsafe {
            match open {
                Some(open_ref) => {
                    sys::ImGui_Begin(title_cstr.as_ptr(), open_ref as *mut bool, flags)
                }
                None => sys::ImGui_Begin(title_cstr.as_ptr(), std::ptr::null_mut(), flags),
            }
        }
    }

    fn end_window(&mut self) {
        unsafe {
            sys::ImGui_End();
        }
    }
}

/// Debug utilities that don't require a UI context
pub struct Debug;

impl Debug {
    /// Check if Dear ImGui is in debug mode
    ///
    /// Returns true if Dear ImGui was compiled with debug features enabled.
    pub fn is_debug_build() -> bool {
        // This would need to be determined at compile time or through a feature flag
        cfg!(debug_assertions)
    }

    /// Log a debug message to Dear ImGui's debug log
    ///
    /// Note: This function is currently a placeholder as ImGui_LogText
    /// requires variadic arguments which are complex to handle safely in Rust.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to log
    ///
    /// # Examples
    ///
    /// ```rust
    /// use dear_imgui::Debug;
    ///
    /// Debug::log("This is a debug message");
    /// ```
    pub fn log(message: impl AsRef<str>) {
        let _message = message.as_ref();
        // TODO: Implement proper logging when we have a safe way to handle
        // variadic arguments or when Dear ImGui provides a non-variadic logging function
        #[cfg(debug_assertions)]
        {
            eprintln!("[Dear ImGui Debug] {}", _message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Context;

    #[test]
    fn test_debug_windows() {
        let mut ctx = Context::new().expect("Failed to create context");
        let mut frame = ctx.frame();

        frame.window("Test").show(|ui| {
            let mut show_demo = true;
            let mut show_metrics = true;
            let mut show_debug_log = true;
            let mut show_stack_tool = true;
            let mut show_about = true;
            let mut show_style_editor = true;

            // Test all debug windows
            ui.show_demo_window(Some(&mut show_demo));
            ui.show_metrics_window(Some(&mut show_metrics));
            ui.show_debug_log_window(Some(&mut show_debug_log));
            ui.show_stack_tool_window(Some(&mut show_stack_tool));
            ui.show_about_window(Some(&mut show_about));
            ui.show_style_editor(Some(&mut show_style_editor));

            // Test without close buttons
            ui.show_demo_window(None);
            ui.show_metrics_window(None);
            ui.show_debug_log_window(None);
            ui.show_stack_tool_window(None);
            ui.show_about_window(None);
            ui.show_style_editor(None);

            // Test other debug functions
            ui.show_user_guide();
            let _version = ui.get_version();

            true
        });
    }

    #[test]
    fn test_debug_utilities() {
        let _is_debug = Debug::is_debug_build();
        Debug::log("Test debug message");
    }

    #[test]
    fn test_version_string() {
        let mut ctx = Context::new().expect("Failed to create context");
        let mut frame = ctx.frame();

        frame.window("Test").show(|ui| {
            let version = ui.get_version();
            assert!(!version.is_empty());
            assert!(!version.is_empty());

            true
        });
    }
}

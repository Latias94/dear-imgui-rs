//! Docking space functionality for Dear ImGui
//!
//! This module provides high-level Rust bindings for Dear ImGui's docking system,
//! allowing you to create dockable windows and manage dock spaces.

use crate::sys;
use crate::ui::Ui;
use std::ptr;

/// Docking-related functionality
impl Ui {
    /// Creates a dockspace over the main viewport
    ///
    /// This is a convenience function that creates a dockspace covering the entire main viewport.
    /// It's equivalent to calling `dock_space` with the main viewport's ID and size.
    ///
    /// # Returns
    ///
    /// The ID of the created dockspace
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let dockspace_id = ui.dockspace_over_main_viewport();
    /// ```
    #[doc(alias = "DockSpaceOverViewport")]
    pub fn dockspace_over_main_viewport(&self) -> sys::ImGuiID {
        unsafe {
            sys::ImGui_DockSpaceOverViewport(
                0, // Use 0 to auto-generate ID
                sys::ImGui_GetMainViewport(),
                sys::ImGuiDockNodeFlags_PassthruCentralNode as i32,
                ptr::null(),
            )
        }
    }

    /// Creates a dockspace with the specified ID and size
    ///
    /// # Parameters
    ///
    /// * `id` - The ID for the dockspace (use 0 to auto-generate)
    /// * `size` - The size of the dockspace in pixels
    ///
    /// # Returns
    ///
    /// The ID of the created dockspace
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let dockspace_id = ui.dock_space(0, [800.0, 600.0]);
    /// ```
    #[doc(alias = "DockSpace")]
    pub fn dock_space(&self, id: sys::ImGuiID, size: [f32; 2]) -> sys::ImGuiID {
        unsafe {
            let size_vec = sys::ImVec2 {
                x: size[0],
                y: size[1],
            };
            sys::ImGui_DockSpace(
                id,
                &size_vec as *const _,
                sys::ImGuiDockNodeFlags_None as i32,
                ptr::null(),
            )
        }
    }

    /// Sets the dock ID for the next window
    ///
    /// This function must be called before creating a window to dock it to a specific dock node.
    ///
    /// # Parameters
    ///
    /// * `dock_id` - The ID of the dock node to dock the next window to
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// let dockspace_id = ui.dockspace_over_main_viewport();
    /// ui.set_next_window_dock_id(dockspace_id);
    /// ui.window("Docked Window").build(|| {
    ///     ui.text("This window will be docked!");
    /// });
    /// ```
    #[doc(alias = "SetNextWindowDockID")]
    pub fn set_next_window_dock_id(&self, dock_id: sys::ImGuiID) {
        unsafe {
            sys::ImGui_SetNextWindowDockID(dock_id, sys::ImGuiCond_Always);
        }
    }

    /// Gets the dock ID of the current window
    ///
    /// # Returns
    ///
    /// The dock ID of the current window, or 0 if the window is not docked
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// ui.window("My Window").build(|| {
    ///     let dock_id = ui.get_window_dock_id();
    ///     if dock_id != 0 {
    ///         ui.text(format!("This window is docked with ID: {}", dock_id));
    ///     } else {
    ///         ui.text("This window is not docked");
    ///     }
    /// });
    /// ```
    #[doc(alias = "GetWindowDockID")]
    pub fn get_window_dock_id(&self) -> sys::ImGuiID {
        unsafe { sys::ImGui_GetWindowDockID() }
    }

    /// Checks if the current window is docked
    ///
    /// # Returns
    ///
    /// `true` if the current window is docked, `false` otherwise
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use dear_imgui::*;
    /// # let mut ctx = Context::create();
    /// # let ui = ctx.frame();
    /// ui.window("My Window").build(|| {
    ///     if ui.is_window_docked() {
    ///         ui.text("This window is docked!");
    ///     } else {
    ///         ui.text("This window is floating");
    ///     }
    /// });
    /// ```
    #[doc(alias = "IsWindowDocked")]
    pub fn is_window_docked(&self) -> bool {
        unsafe { sys::ImGui_IsWindowDocked() }
    }
}

// TODO: Add more advanced docking functionality:
// - DockBuilder API for programmatic dock layout creation
// - Dock node manipulation functions
// - Custom dock node flags and configuration
// - Viewport-specific docking functions

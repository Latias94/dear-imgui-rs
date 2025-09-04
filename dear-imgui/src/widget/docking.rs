use crate::types::Vec2;
use crate::ui::Ui;

/// Docking widgets
///
/// This module contains all docking-related UI components for creating dockable layouts.
/// Note: Docking requires the docking feature to be enabled in Dear ImGui.

/// # Widgets: Docking
///
/// These functions provide docking support when the docking feature is enabled.
impl<'frame> Ui<'frame> {
    /// Create a dock space
    ///
    /// A dock space is a central area where windows can be docked.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use dear_imgui::{Context, Vec2};
    /// # let mut ctx = Context::new().unwrap();
    /// # let mut frame = ctx.frame();
    /// # frame.window("DockSpace Demo").show(|ui| {
    /// // Create a dock space that fills the entire window
    /// let _dock_id = ui.dock_space("MainDockSpace", Vec2::new(0.0, 0.0));
    /// # });
    /// ```
    #[cfg(feature = "docking")]
    pub fn dock_space(&mut self, id: impl AsRef<str>, size: Vec2) -> u32 {
        let size_vec = dear_imgui_sys::ImVec2 {
            x: size.x,
            y: size.y,
        };

        unsafe {
            let dock_id = dear_imgui_sys::ImGui_GetID(self.scratch_txt(id));
            dear_imgui_sys::ImGui_DockSpace(
                dock_id,
                &size_vec as *const _,
                0,                // Default flags
                std::ptr::null(), // No window class
            )
        }
    }

    /// Create a dock space (placeholder when docking is disabled)
    #[cfg(not(feature = "docking"))]
    pub fn dock_space(&mut self, _id: impl AsRef<str>, _size: Vec2) -> u32 {
        // Placeholder implementation - docking feature not enabled
        0
    }

    /// Create a dock space with flags
    #[cfg(feature = "docking")]
    pub fn dock_space_with_flags(&mut self, id: impl AsRef<str>, size: Vec2, flags: i32) -> u32 {
        let size_vec = dear_imgui_sys::ImVec2 {
            x: size.x,
            y: size.y,
        };

        unsafe {
            let dock_id = dear_imgui_sys::ImGui_GetID(self.scratch_txt(id));
            dear_imgui_sys::ImGui_DockSpace(
                dock_id,
                &size_vec as *const _,
                flags,
                std::ptr::null(), // No window class
            )
        }
    }

    /// Create a dock space with flags (placeholder when docking is disabled)
    #[cfg(not(feature = "docking"))]
    pub fn dock_space_with_flags(&mut self, _id: impl AsRef<str>, _size: Vec2, _flags: i32) -> u32 {
        // Placeholder implementation - docking feature not enabled
        0
    }

    /// Create a dock space over viewport
    #[cfg(feature = "docking")]
    pub fn dock_space_over_viewport(&mut self, id: impl AsRef<str>) -> u32 {
        unsafe {
            let dock_id = dear_imgui_sys::ImGui_GetID(self.scratch_txt(id));
            let viewport = dear_imgui_sys::ImGui_GetMainViewport();
            dear_imgui_sys::ImGui_DockSpaceOverViewport(
                dock_id,
                viewport,
                0,                // Default flags
                std::ptr::null(), // No window class
            )
        }
    }

    /// Create a dock space over viewport (placeholder when docking is disabled)
    #[cfg(not(feature = "docking"))]
    pub fn dock_space_over_viewport(&mut self, _id: impl AsRef<str>) -> u32 {
        // Placeholder implementation - docking feature not enabled
        0
    }

    /// Set next window dock ID
    #[cfg(feature = "docking")]
    pub fn set_next_window_dock_id(&mut self, dock_id: u32, cond: i32) {
        unsafe {
            dear_imgui_sys::ImGui_SetNextWindowDockID(dock_id, cond);
        }
    }

    /// Set next window dock ID (placeholder when docking is disabled)
    #[cfg(not(feature = "docking"))]
    pub fn set_next_window_dock_id(&mut self, _dock_id: u32, _cond: i32) {
        // Placeholder implementation - docking feature not enabled
    }

    /// Get window dock ID
    #[cfg(feature = "docking")]
    pub fn get_window_dock_id(&mut self) -> u32 {
        unsafe { dear_imgui_sys::ImGui_GetWindowDockID() }
    }

    /// Get window dock ID (placeholder when docking is disabled)
    #[cfg(not(feature = "docking"))]
    pub fn get_window_dock_id(&mut self) -> u32 {
        // Placeholder implementation - docking feature not enabled
        0
    }

    /// Check if window is docked
    #[cfg(feature = "docking")]
    pub fn is_window_docked(&mut self) -> bool {
        unsafe { dear_imgui_sys::ImGui_IsWindowDocked() }
    }

    /// Check if window is docked (placeholder when docking is disabled)
    #[cfg(not(feature = "docking"))]
    pub fn is_window_docked(&mut self) -> bool {
        // Placeholder implementation - docking feature not enabled
        false
    }

    /// Dock a window to a specific dock node
    #[cfg(feature = "docking")]
    pub fn dock_builder_dock_window(&mut self, window_name: impl AsRef<str>, node_id: u32) {
        unsafe {
            dear_imgui_sys::ImGui_DockBuilderDockWindow(self.scratch_txt(window_name), node_id);
        }
    }

    /// Dock a window to a specific dock node (placeholder when docking is disabled)
    #[cfg(not(feature = "docking"))]
    pub fn dock_builder_dock_window(&mut self, _window_name: impl AsRef<str>, _node_id: u32) {
        // Placeholder implementation - docking feature not enabled
    }

    /// Get dock builder central node
    #[cfg(feature = "docking")]
    pub fn dock_builder_get_central_node(&mut self, node_id: u32) -> u32 {
        unsafe {
            let node = dear_imgui_sys::ImGui_DockBuilderGetNode(node_id);
            if node.is_null() {
                0
            } else {
                node_id
            }
        }
    }

    /// Get dock builder central node (placeholder when docking is disabled)
    #[cfg(not(feature = "docking"))]
    pub fn dock_builder_get_central_node(&mut self, _node_id: u32) -> u32 {
        // Placeholder implementation - docking feature not enabled
        0
    }

    /// Finish dock builder setup
    #[cfg(feature = "docking")]
    pub fn dock_builder_finish(&mut self, node_id: u32) {
        unsafe {
            dear_imgui_sys::ImGui_DockBuilderFinish(node_id);
        }
    }

    /// Finish dock builder setup (placeholder when docking is disabled)
    #[cfg(not(feature = "docking"))]
    pub fn dock_builder_finish(&mut self, _node_id: u32) {
        // Placeholder implementation - docking feature not enabled
    }

    /// Split a dock node
    #[cfg(feature = "docking")]
    pub fn dock_builder_split_node(
        &mut self,
        node_id: u32,
        split_dir: i32,
        size_ratio: f32,
    ) -> (u32, u32) {
        let mut out_id_at_dir: u32 = 0;
        let mut out_id_at_opposite_dir: u32 = 0;

        unsafe {
            dear_imgui_sys::ImGui_DockBuilderSplitNode(
                node_id,
                split_dir,
                size_ratio,
                &mut out_id_at_dir as *mut u32,
                &mut out_id_at_opposite_dir as *mut u32,
            );
        }

        (out_id_at_dir, out_id_at_opposite_dir)
    }

    /// Split a dock node (placeholder when docking is disabled)
    #[cfg(not(feature = "docking"))]
    pub fn dock_builder_split_node(
        &mut self,
        _node_id: u32,
        _split_dir: i32,
        _size_ratio: f32,
    ) -> (u32, u32) {
        // Placeholder implementation - docking feature not enabled
        (0, 0)
    }
}

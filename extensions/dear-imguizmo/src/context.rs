//! ImGuizmo context management

use crate::{sys, Error, Result};
use dear_imgui::{Context as ImGuiContext, Ui};
use parking_lot::Mutex;
use std::sync::Arc;

/// ImGuizmo context that manages the lifetime and state of ImGuizmo operations
pub struct GuizmoContext {
    _imgui_ctx: Arc<Mutex<()>>, // Ensure ImGui context outlives this
}

impl GuizmoContext {
    /// Create a new ImGuizmo context
    ///
    /// This must be called after creating the ImGui context and should be kept alive
    /// for the duration of ImGuizmo usage.
    pub fn create(_imgui_ctx: &ImGuiContext) -> Self {
        Self {
            _imgui_ctx: Arc::new(Mutex::new(())),
        }
    }

    /// Get a GuizmoUi instance for the current frame
    ///
    /// This should be called once per frame after calling `imgui_ctx.frame()`.
    pub fn get_ui<'ui>(&self, ui: &'ui Ui) -> GuizmoUi<'ui> {
        GuizmoUi::new(ui)
    }
}

/// Per-frame ImGuizmo UI interface
///
/// This provides access to all ImGuizmo functionality for a single frame.
/// It automatically manages the ImGui context and ensures proper cleanup.
pub struct GuizmoUi<'ui> {
    ui: &'ui Ui,
}

impl<'ui> GuizmoUi<'ui> {
    /// Create a new GuizmoUi instance
    pub(crate) fn new(ui: &'ui Ui) -> Self {
        Self { ui }
    }

    /// Get the underlying ImGui Ui reference
    pub fn ui(&self) -> &Ui {
        self.ui
    }

    /// Set the drawing rectangle for ImGuizmo operations
    ///
    /// This defines the screen area where gizmos will be rendered and interact.
    /// Usually this should match your 3D viewport area.
    pub fn set_rect(&self, x: f32, y: f32, width: f32, height: f32) {
        unsafe {
            sys::ImGuizmo_SetRect(x, y, width, height);
        }
    }

    /// Set whether to use orthographic projection
    ///
    /// Default is false (perspective projection).
    pub fn set_orthographic(&self, orthographic: bool) {
        unsafe {
            sys::ImGuizmo_SetOrthographic(orthographic);
        }
    }

    /// Enable or disable ImGuizmo rendering
    ///
    /// When disabled, gizmos are rendered with a gray, semi-transparent appearance.
    pub fn enable(&self, enabled: bool) {
        unsafe {
            sys::ImGuizmo_Enable(enabled);
        }
    }

    /// Check if ImGuizmo is currently being used (any gizmo is active)
    pub fn is_using(&self) -> bool {
        unsafe { sys::ImGuizmo_IsUsing() }
    }

    /// Check if the mouse is over a specific operation's gizmo
    pub fn is_over_operation(&self, operation: crate::Operation) -> bool {
        unsafe { sys::ImGuizmo_IsOver_Operation(operation.bits() as i32) }
    }

    /// Check if the mouse is over a specific 3D position within a pixel radius
    pub fn is_over_position(&self, position: &[f32; 3], pixel_radius: f32) -> bool {
        unsafe { sys::ImGuizmo_IsOver_Position(position.as_ptr() as *mut f32, pixel_radius) }
    }

    /// Draw a reference grid
    ///
    /// # Arguments
    /// * `view` - View matrix (16 floats)
    /// * `projection` - Projection matrix (16 floats)
    /// * `matrix` - Grid transformation matrix (16 floats)
    /// * `grid_size` - Size of grid cells
    pub fn draw_grid(
        &self,
        view: &[f32; 16],
        projection: &[f32; 16],
        matrix: &[f32; 16],
        grid_size: f32,
    ) {
        unsafe {
            sys::ImGuizmo_DrawGrid(
                view.as_ptr(),
                projection.as_ptr(),
                matrix.as_ptr(),
                grid_size,
            );
        }
    }

    /// Draw debug cubes
    ///
    /// Renders cubes with face colors corresponding to face normals.
    /// Useful for debugging and testing.
    ///
    /// # Arguments
    /// * `view` - View matrix (16 floats)
    /// * `projection` - Projection matrix (16 floats)
    /// * `matrices` - Array of transformation matrices (each 16 floats)
    pub fn draw_cubes(&self, view: &[f32; 16], projection: &[f32; 16], matrices: &[[f32; 16]]) {
        unsafe {
            sys::ImGuizmo_DrawCubes(
                view.as_ptr(),
                projection.as_ptr(),
                matrices.as_ptr() as *const f32,
                matrices.len() as i32,
            );
        }
    }

    /// Decompose a transformation matrix into translation, rotation, and scale components
    ///
    /// # Arguments
    /// * `matrix` - Input transformation matrix (16 floats)
    ///
    /// # Returns
    /// * `translation` - Translation vector (3 floats)
    /// * `rotation` - Rotation in degrees (3 floats)
    /// * `scale` - Scale vector (3 floats)
    pub fn decompose_matrix(
        &self,
        matrix: &[f32; 16],
    ) -> Result<(crate::Vector3, crate::Vector3, crate::Vector3)> {
        let mut translation = [0.0f32; 3];
        let mut rotation = [0.0f32; 3];
        let mut scale = [0.0f32; 3];

        unsafe {
            sys::ImGuizmo_DecomposeMatrixToComponents(
                matrix.as_ptr(),
                translation.as_mut_ptr(),
                rotation.as_mut_ptr(),
                scale.as_mut_ptr(),
            );
        }

        Ok((translation, rotation, scale))
    }

    /// Recompose a transformation matrix from translation, rotation, and scale components
    ///
    /// # Arguments
    /// * `translation` - Translation vector (3 floats)
    /// * `rotation` - Rotation in degrees (3 floats)
    /// * `scale` - Scale vector (3 floats)
    ///
    /// # Returns
    /// * Transformation matrix (16 floats)
    pub fn recompose_matrix(
        &self,
        translation: &crate::Vector3,
        rotation: &crate::Vector3,
        scale: &crate::Vector3,
    ) -> crate::Matrix4 {
        let mut matrix = [0.0f32; 16];

        unsafe {
            sys::ImGuizmo_RecomposeMatrixFromComponents(
                translation.as_ptr(),
                rotation.as_ptr(),
                scale.as_ptr(),
                matrix.as_mut_ptr(),
            );
        }

        matrix
    }

    /// Create a manipulation builder for configuring and executing gizmo operations
    pub fn manipulate<'a>(
        &'a self,
        view: &'a [f32; 16],
        projection: &'a [f32; 16],
    ) -> crate::ManipulateBuilder<'a> {
        crate::ManipulateBuilder::new(self, view, projection)
    }

    /// Create a view manipulation builder for camera controls
    pub fn view_manipulate<'a>(
        &'a self,
        view: &'a mut [f32; 16],
    ) -> crate::ViewManipulateBuilder<'a> {
        crate::ViewManipulateBuilder::new(self, view)
    }

    /// Push an ID onto the ImGuizmo ID stack
    ///
    /// This is useful when you have multiple gizmos and need to distinguish between them.
    pub fn push_id(&self, id: &str) {
        let c_str = crate::to_cstring(id);
        unsafe {
            sys::ImGuizmo_PushID_Str(c_str.as_ptr());
        }
    }

    /// Push an integer ID onto the ImGuizmo ID stack
    pub fn push_id_int(&self, id: i32) {
        unsafe {
            sys::ImGuizmo_PushID_Int(id);
        }
    }

    /// Push a pointer ID onto the ImGuizmo ID stack
    pub fn push_id_ptr(&self, id: *const std::ffi::c_void) {
        unsafe {
            sys::ImGuizmo_PushID_Ptr(id);
        }
    }

    /// Pop an ID from the ImGuizmo ID stack
    pub fn pop_id(&self) {
        unsafe {
            sys::ImGuizmo_PopID();
        }
    }
}

//! Gizmo manipulation builders and operations

use crate::{sys, GuizmoUi, ManipulationResult, Matrix4, Mode, Operation, Vector2, Vector3};
use std::ptr;

/// Builder for configuring and executing gizmo manipulations
pub struct ManipulateBuilder<'a> {
    ui: &'a GuizmoUi<'a>,
    view: &'a Matrix4,
    projection: &'a Matrix4,
    operation: Operation,
    mode: Mode,
    matrix: Option<&'a mut Matrix4>,
    snap: Option<&'a Vector3>,
    local_bounds: Option<&'a [f32; 6]>, // min_x, min_y, min_z, max_x, max_y, max_z
    bounds_snap: Option<&'a Vector3>,
}

impl<'a> ManipulateBuilder<'a> {
    /// Create a new manipulation builder
    pub(crate) fn new(ui: &'a GuizmoUi<'a>, view: &'a Matrix4, projection: &'a Matrix4) -> Self {
        Self {
            ui,
            view,
            projection,
            operation: Operation::TRANSLATE,
            mode: Mode::World,
            matrix: None,
            snap: None,
            local_bounds: None,
            bounds_snap: None,
        }
    }

    /// Set the gizmo operation type
    pub fn operation(mut self, operation: Operation) -> Self {
        self.operation = operation;
        self
    }

    /// Set the manipulation mode (local or world space)
    pub fn mode(mut self, mode: Mode) -> Self {
        self.mode = mode;
        self
    }

    /// Set the transformation matrix to manipulate
    pub fn matrix(mut self, matrix: &'a mut Matrix4) -> Self {
        self.matrix = Some(matrix);
        self
    }

    /// Set snapping values for the operation
    ///
    /// For translation: snap values for X, Y, Z axes
    /// For rotation: snap value for angle (only X component used)
    /// For scale: snap value for scale (only X component used)
    pub fn snap(mut self, snap: &'a Vector3) -> Self {
        self.snap = Some(snap);
        self
    }

    /// Set local bounds for the manipulation
    ///
    /// Format: [min_x, min_y, min_z, max_x, max_y, max_z]
    pub fn local_bounds(mut self, bounds: &'a [f32; 6]) -> Self {
        self.local_bounds = Some(bounds);
        self
    }

    /// Set bounds snapping values
    pub fn bounds_snap(mut self, bounds_snap: &'a Vector3) -> Self {
        self.bounds_snap = Some(bounds_snap);
        self
    }

    /// Execute the manipulation and return the result
    pub fn build(self) -> Option<ManipulationResult> {
        let matrix = self.matrix?;
        let mut delta_matrix = [0.0f32; 16];

        let snap_ptr = self.snap.map_or(ptr::null(), |s| s.as_ptr());
        let bounds_ptr = self.local_bounds.map_or(ptr::null(), |b| b.as_ptr());
        let bounds_snap_ptr = self.bounds_snap.map_or(ptr::null(), |bs| bs.as_ptr());

        let used = unsafe {
            sys::ImGuizmo_Manipulate(
                self.view.as_ptr(),
                self.projection.as_ptr(),
                self.operation.bits() as i32,
                Into::<sys::MODE>::into(self.mode) as i32,
                matrix.as_mut_ptr(),
                delta_matrix.as_mut_ptr(),
                snap_ptr,
                bounds_ptr,
                bounds_snap_ptr,
            )
        };

        let hovered = self.ui.is_over_operation(self.operation);

        Some(ManipulationResult {
            used,
            delta_matrix: if used { Some(delta_matrix) } else { None },
            hovered,
        })
    }
}

/// Builder for configuring and executing view manipulations (camera controls)
pub struct ViewManipulateBuilder<'a> {
    ui: &'a GuizmoUi<'a>,
    view: &'a mut Matrix4,
    length: f32,
    position: Vector2,
    size: Vector2,
    background_color: u32,
    projection: Option<&'a Matrix4>,
    operation: Option<Operation>,
    mode: Option<Mode>,
    matrix: Option<&'a mut Matrix4>,
}

impl<'a> ViewManipulateBuilder<'a> {
    /// Create a new view manipulation builder
    pub(crate) fn new(ui: &'a GuizmoUi<'a>, view: &'a mut Matrix4) -> Self {
        Self {
            ui,
            view,
            length: 8.0,
            position: [0.0, 0.0],
            size: [128.0, 128.0],
            background_color: 0x80808080, // Semi-transparent gray
            projection: None,
            operation: None,
            mode: None,
            matrix: None,
        }
    }

    /// Set the length of the view cube
    pub fn length(mut self, length: f32) -> Self {
        self.length = length;
        self
    }

    /// Set the position of the view cube on screen
    pub fn position(mut self, x: f32, y: f32) -> Self {
        self.position = [x, y];
        self
    }

    /// Set the size of the view cube
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = [width, height];
        self
    }

    /// Set the background color (RGBA as u32)
    pub fn background_color(mut self, color: u32) -> Self {
        self.background_color = color;
        self
    }

    /// Use the extended version with projection matrix and manipulation
    pub fn with_manipulation(
        mut self,
        projection: &'a Matrix4,
        operation: Operation,
        mode: Mode,
        matrix: &'a mut Matrix4,
    ) -> Self {
        self.projection = Some(projection);
        self.operation = Some(operation);
        self.mode = Some(mode);
        self.matrix = Some(matrix);
        self
    }

    /// Execute the view manipulation
    pub fn build(self) {
        unsafe {
            if let (Some(projection), Some(operation), Some(mode), Some(matrix)) =
                (self.projection, self.operation, self.mode, self.matrix)
            {
                // Extended version with manipulation
                sys::ImGuizmo_ViewManipulate_Extended(
                    self.view.as_mut_ptr(),
                    projection.as_ptr(),
                    operation.bits() as i32,
                    Into::<sys::MODE>::into(mode) as i32,
                    matrix.as_mut_ptr(),
                    self.length,
                    self.position[0],
                    self.position[1],
                    self.size[0],
                    self.size[1],
                    self.background_color,
                );
            } else {
                // Simple version
                sys::ImGuizmo_ViewManipulate(
                    self.view.as_mut_ptr(),
                    self.length,
                    sys::ImVec2 {
                        x: self.position[0],
                        y: self.position[1],
                    },
                    sys::ImVec2 {
                        x: self.size[0],
                        y: self.size[1],
                    },
                    self.background_color,
                );
            }
        }
    }
}

/// RAII guard for ImGuizmo ID stack management
pub struct IdGuard<'a> {
    _ui: &'a GuizmoUi<'a>,
}

impl<'a> IdGuard<'a> {
    /// Create a new ID guard and push the ID
    pub fn new(ui: &'a GuizmoUi<'a>, id: &str) -> Self {
        ui.push_id(id);
        Self { _ui: ui }
    }

    /// Create a new ID guard with an integer ID
    pub fn new_int(ui: &'a GuizmoUi<'a>, id: i32) -> Self {
        ui.push_id_int(id);
        Self { _ui: ui }
    }

    /// Create a new ID guard with a pointer ID
    pub fn new_ptr(ui: &'a GuizmoUi<'a>, id: *const std::ffi::c_void) -> Self {
        ui.push_id_ptr(id);
        Self { _ui: ui }
    }
}

impl<'a> Drop for IdGuard<'a> {
    fn drop(&mut self) {
        self._ui.pop_id();
    }
}

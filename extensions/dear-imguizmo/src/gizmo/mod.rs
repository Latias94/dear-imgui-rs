//! Core gizmo functionality
//!
//! This module contains the main gizmo manipulation logic, including
//! translation, rotation, and scaling operations.

pub mod bounds;
pub mod manipulate;
pub mod rotate;
pub mod scale;
pub mod translate;

/// Type of manipulation currently being performed
/// This corresponds to the MOVETYPE enum in the original ImGuizmo
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManipulationType {
    /// No manipulation (MT_NONE)
    None = 0,
    /// Moving along X axis (MT_MOVE_X)
    MoveX = 1,
    /// Moving along Y axis (MT_MOVE_Y)
    MoveY = 2,
    /// Moving along Z axis (MT_MOVE_Z)
    MoveZ = 3,
    /// Moving in YZ plane (MT_MOVE_YZ)
    MoveYZ = 4,
    /// Moving in ZX plane (MT_MOVE_ZX)
    MoveZX = 5,
    /// Moving in XY plane (MT_MOVE_XY)
    MoveXY = 6,
    /// Moving in screen space (MT_MOVE_SCREEN)
    MoveScreen = 7,
    /// Rotating around X axis (MT_ROTATE_X)
    RotateX = 8,
    /// Rotating around Y axis (MT_ROTATE_Y)
    RotateY = 9,
    /// Rotating around Z axis (MT_ROTATE_Z)
    RotateZ = 10,
    /// Screen space rotation (MT_ROTATE_SCREEN)
    RotateScreen = 11,
    /// Scaling along X axis (MT_SCALE_X)
    ScaleX = 12,
    /// Scaling along Y axis (MT_SCALE_Y)
    ScaleY = 13,
    /// Scaling along Z axis (MT_SCALE_Z)
    ScaleZ = 14,
    /// Uniform scaling (MT_SCALE_XYZ)
    ScaleXYZ = 15,
}

impl ManipulationType {
    /// Check if this is a translation type
    pub fn is_translate_type(self) -> bool {
        matches!(
            self,
            ManipulationType::MoveX
                | ManipulationType::MoveY
                | ManipulationType::MoveZ
                | ManipulationType::MoveYZ
                | ManipulationType::MoveZX
                | ManipulationType::MoveXY
                | ManipulationType::MoveScreen
        )
    }

    /// Check if this is a rotation type
    pub fn is_rotate_type(self) -> bool {
        matches!(
            self,
            ManipulationType::RotateX
                | ManipulationType::RotateY
                | ManipulationType::RotateZ
                | ManipulationType::RotateScreen
        )
    }

    /// Check if this is a scale type
    pub fn is_scale_type(self) -> bool {
        matches!(
            self,
            ManipulationType::ScaleX
                | ManipulationType::ScaleY
                | ManipulationType::ScaleZ
                | ManipulationType::ScaleXYZ
        )
    }

    /// Get the axis index for single-axis operations (0=X, 1=Y, 2=Z)
    pub fn axis_index(self) -> Option<usize> {
        match self {
            ManipulationType::MoveX | ManipulationType::RotateX | ManipulationType::ScaleX => {
                Some(0)
            }
            ManipulationType::MoveY | ManipulationType::RotateY | ManipulationType::ScaleY => {
                Some(1)
            }
            ManipulationType::MoveZ | ManipulationType::RotateZ | ManipulationType::ScaleZ => {
                Some(2)
            }
            _ => None,
        }
    }

    /// Check if this manipulation type involves X axis
    pub fn is_x_axis(&self) -> bool {
        matches!(
            self,
            ManipulationType::MoveX
                | ManipulationType::MoveXY
                | ManipulationType::MoveZX
                | ManipulationType::RotateX
                | ManipulationType::ScaleX
        )
    }

    /// Check if this manipulation type involves Y axis
    pub fn is_y_axis(&self) -> bool {
        matches!(
            self,
            ManipulationType::MoveY
                | ManipulationType::MoveXY
                | ManipulationType::MoveYZ
                | ManipulationType::RotateY
                | ManipulationType::ScaleY
        )
    }

    /// Check if this manipulation type involves Z axis
    pub fn is_z_axis(&self) -> bool {
        matches!(
            self,
            ManipulationType::MoveZ
                | ManipulationType::MoveZX
                | ManipulationType::MoveYZ
                | ManipulationType::RotateZ
                | ManipulationType::ScaleZ
        )
    }

    /// Check if this is a plane manipulation type
    pub fn is_plane_type(&self) -> bool {
        matches!(
            self,
            ManipulationType::MoveXY | ManipulationType::MoveYZ | ManipulationType::MoveZX
        )
    }
}

// Re-export public API
pub use manipulate::*;
pub use rotate::*;
pub use scale::*;

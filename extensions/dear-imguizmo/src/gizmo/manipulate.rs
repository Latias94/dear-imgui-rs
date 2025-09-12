//! Main manipulation logic for ImGuizmo
//!
//! This module contains the core manipulation functions that coordinate
//! between different transformation types.

use crate::types::{Mat4, Mode, Operation};
use crate::{GuizmoError, GuizmoResult};

/// Result of a manipulation operation
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ManipulationResult {
    /// Whether the matrix was modified
    pub modified: bool,
    /// The delta transformation that was applied
    pub delta: Option<Mat4>,
    /// Which specific operation was performed
    pub operation_used: Option<Operation>,
}

/// Core manipulation function
///
/// This function coordinates between different transformation types and
/// handles the main manipulation logic.
pub fn manipulate(
    view: &Mat4,
    projection: &Mat4,
    operation: Operation,
    mode: Mode,
    matrix: &mut Mat4,
    delta_matrix: Option<&mut Mat4>,
    _snap: Option<&[f32; 3]>,
    _local_bounds: Option<&[f32; 6]>,
    _bounds_snap: Option<&[f32; 3]>,
) -> GuizmoResult<ManipulationResult> {
    // Validate inputs
    if !is_matrix_valid(view) {
        return Err(GuizmoError::invalid_matrix("Invalid view matrix"));
    }
    if !is_matrix_valid(projection) {
        return Err(GuizmoError::invalid_matrix("Invalid projection matrix"));
    }
    if !is_matrix_valid(matrix) {
        return Err(GuizmoError::invalid_matrix("Invalid transformation matrix"));
    }

    // Initialize result
    let mut result = ManipulationResult::default();

    // Store original matrix for delta calculation
    let original_matrix = *matrix;

    // TODO: Implement actual manipulation logic
    // This is a placeholder for the complex manipulation system

    crate::guizmo_trace!("Manipulate: operation={:?}, mode={:?}", operation, mode);

    // Calculate delta if requested
    if let Some(delta) = delta_matrix {
        *delta = matrix.inverse() * original_matrix;
        result.delta = Some(*delta);
    }

    // Check if matrix was actually modified
    result.modified = !matrices_approximately_equal(matrix, &original_matrix, 1e-6);

    if result.modified {
        result.operation_used = Some(operation);
        crate::guizmo_debug!("Matrix was modified by manipulation");
    }

    Ok(result)
}

/// Check if a matrix is valid (not NaN, not infinite)
fn is_matrix_valid(matrix: &Mat4) -> bool {
    matrix.to_cols_array().iter().all(|&x| x.is_finite())
}

/// Check if two matrices are approximately equal
fn matrices_approximately_equal(a: &Mat4, b: &Mat4, epsilon: f32) -> bool {
    let a_array = a.to_cols_array();
    let b_array = b.to_cols_array();

    a_array
        .iter()
        .zip(b_array.iter())
        .all(|(&a_val, &b_val)| (a_val - b_val).abs() < epsilon)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn test_matrix_validation() {
        let valid_matrix = Mat4::IDENTITY;
        assert!(is_matrix_valid(&valid_matrix));

        let invalid_matrix = Mat4::from_cols_array(&[
            f32::NAN,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
        ]);
        assert!(!is_matrix_valid(&invalid_matrix));
    }

    #[test]
    fn test_matrix_equality() {
        let a = Mat4::IDENTITY;
        let b = Mat4::IDENTITY;
        assert!(matrices_approximately_equal(&a, &b, 1e-6));

        let c = Mat4::from_translation(Vec3::new(0.000001, 0.0, 0.0));
        assert!(matrices_approximately_equal(&a, &c, 1e-5));
        assert!(!matrices_approximately_equal(&a, &c, 1e-7));
    }

    #[test]
    fn test_manipulation_result() {
        let result = ManipulationResult::default();
        assert!(!result.modified);
        assert!(result.delta.is_none());
        assert!(result.operation_used.is_none());
    }
}

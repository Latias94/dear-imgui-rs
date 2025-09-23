//! Translation gizmo implementation
//!
//! This module handles translation (movement) operations for the gizmo.

use crate::context::GuizmoContext;
use crate::error::GuizmoResult;
use crate::gizmo::ManipulationType;
use crate::types::{Mat4, Mode, Operation, Vec3};

/// Handle translation manipulation
/// Returns true if the matrix was modified
pub fn handle_translation(
    context: &GuizmoContext,
    matrix: &mut Mat4,
    delta_matrix: Option<&mut Mat4>,
    operation: Operation,
    manipulation_type: &mut ManipulationType,
    snap: Option<&[f32; 3]>,
) -> GuizmoResult<bool> {
    // Check if we should handle translation
    if !operation.intersects(Operation::TRANSLATE) || *manipulation_type != ManipulationType::None {
        return Ok(false);
    }

    let mut state = context.state.borrow_mut();
    let apply_rotation_locally =
        state.mode == Mode::Local || *manipulation_type == ManipulationType::MoveScreen;
    let mut modified = false;

    // If we're currently manipulating
    if (state.using != ManipulationType::None)
        && (context.get_current_id() == state.editing_id)
        && manipulation_type.is_translate_type()
    {
        // Capture mouse input
        unsafe {
            dear_imgui::sys::igSetNextFrameWantCaptureMouse(true);
        }

        // Compute intersection with translation plane
        let signed_length = crate::math::intersect_ray_plane(
            state.ray_origin,
            state.ray_vector,
            state.translation_plane,
        );
        let len = signed_length.abs();
        let new_pos = state.ray_origin + state.ray_vector * len;

        // Compute delta
        let new_origin = new_pos - state.relative_origin * state.screen_factor;
        let mut delta = new_origin - state.model_matrix.w_axis.truncate();

        // Apply single axis constraint
        if let Some(axis_index) = manipulation_type.axis_index() {
            if axis_index < 3 {
                let axis_value = match axis_index {
                    0 => state.model_matrix.x_axis.truncate(),
                    1 => state.model_matrix.y_axis.truncate(),
                    2 => state.model_matrix.z_axis.truncate(),
                    _ => unreachable!(),
                };
                let length_on_axis = axis_value.dot(delta);
                delta = axis_value * length_on_axis;
            }
        }

        // Apply snapping
        if let Some(snap_values) = snap {
            let mut cumulative_delta =
                state.model_matrix.w_axis.truncate() + delta - state.matrix_origin;

            if apply_rotation_locally {
                let model_source_normalized =
                    crate::math::orthonormalize_matrix(&state.model_source);
                let model_source_normalized_inverse = model_source_normalized.inverse();

                // Transform to local space
                cumulative_delta =
                    model_source_normalized_inverse.transform_vector3(cumulative_delta);
                compute_snap(&mut cumulative_delta, snap_values);
                cumulative_delta = model_source_normalized.transform_vector3(cumulative_delta);
            } else {
                compute_snap(&mut cumulative_delta, snap_values);
            }

            delta = state.matrix_origin + cumulative_delta - state.model_matrix.w_axis.truncate();
        }

        // Check if delta changed
        if delta != state.translation_last_delta {
            modified = true;
        }
        state.translation_last_delta = delta;

        // Compute result matrix and delta matrix
        let delta_matrix_translation = Mat4::from_translation(delta);
        if let Some(delta_mat) = delta_matrix {
            *delta_mat = delta_matrix_translation;
        }

        let result = state.model_source * delta_matrix_translation;
        *matrix = result;

        // Check if mouse is released
        let io = unsafe { dear_imgui::sys::igGetIO_Nil() };
        if !unsafe { (*io).MouseDown[0] } {
            state.using = ManipulationType::None;
        }

        *manipulation_type = state.current_manipulation_type;
    } else {
        // Find new possible way to move
        let gizmo_hit_proportion = Vec3::ZERO; // TODO: implement hit proportion
        let move_type = get_move_type(&state, operation, Some(gizmo_hit_proportion))?;
        state.over_gizmo_hotspot |= move_type != ManipulationType::None;

        if move_type != ManipulationType::None {
            unsafe {
                dear_imgui::sys::igSetNextFrameWantCaptureMouse(true);
            }
        }

        if GuizmoContext::can_activate() && move_type != ManipulationType::None {
            state.using = *manipulation_type;
            state.editing_id = context.get_current_id();
            state.current_manipulation_type = move_type;

            // Setup translation plane
            setup_translation_plane(&mut state, move_type)?;
        }

        *manipulation_type = move_type;
    }

    Ok(modified)
}

/// Compute snap for translation values
fn compute_snap(value: &mut Vec3, snap: &[f32; 3]) {
    for i in 0..3 {
        if snap[i] > 0.0 {
            value[i] = (value[i] / snap[i]).round() * snap[i];
        }
    }
}

/// Get the movement type based on mouse position and operation
fn get_move_type(
    state: &crate::context::GuizmoState,
    operation: Operation,
    _gizmo_hit_proportion: Option<Vec3>,
) -> GuizmoResult<ManipulationType> {
    if !operation.intersects(Operation::TRANSLATE)
        || (state.using != ManipulationType::None)
        || !state.is_over
    {
        return Ok(ManipulationType::None);
    }

    // TODO: Implement proper hit testing against gizmo geometry
    // For now, return None - this will be implemented when we add the drawing system
    Ok(ManipulationType::None)
}

/// Setup translation plane for manipulation
fn setup_translation_plane(
    state: &mut crate::context::GuizmoState,
    move_type: ManipulationType,
) -> GuizmoResult<()> {
    // Compute move plane normals
    let move_plane_normals = [
        state.model_matrix.x_axis.truncate(), // right
        state.model_matrix.y_axis.truncate(), // up
        state.model_matrix.z_axis.truncate(), // forward
        state.model_matrix.x_axis.truncate(), // right (for planes)
        state.model_matrix.y_axis.truncate(), // up (for planes)
        state.model_matrix.z_axis.truncate(), // forward (for planes)
        -state.camera_dir,                    // screen
    ];

    let camera_to_model_normalized =
        (state.model_matrix.w_axis.truncate() - state.camera_eye).normalize();

    // Compute orthogonal vectors for plane setup
    let plane_index = match move_type {
        ManipulationType::MoveX => 0,
        ManipulationType::MoveY => 1,
        ManipulationType::MoveZ => 2,
        ManipulationType::MoveYZ => 0,
        ManipulationType::MoveZX => 1,
        ManipulationType::MoveXY => 2,
        ManipulationType::MoveScreen => 6,
        _ => return Ok(()),
    };

    if plane_index < 6 {
        let mut plane_normal = move_plane_normals[plane_index];
        if plane_index < 3 {
            // Single axis movement - create plane perpendicular to axis
            let ortho_vector = plane_normal.cross(camera_to_model_normalized);
            plane_normal = plane_normal.cross(ortho_vector).normalize();
        }

        // Build plane equation
        state.translation_plane =
            crate::math::build_plane(state.model_matrix.w_axis.truncate(), plane_normal);
    } else {
        // Screen space movement
        state.translation_plane =
            crate::math::build_plane(state.model_matrix.w_axis.truncate(), -state.camera_dir);
    }

    // Compute intersection point
    let len = crate::math::intersect_ray_plane(
        state.ray_origin,
        state.ray_vector,
        state.translation_plane,
    );
    state.translation_plane_origin = state.ray_origin + state.ray_vector * len;
    state.matrix_origin = state.model_matrix.w_axis.truncate();
    state.relative_origin = (state.translation_plane_origin - state.model_matrix.w_axis.truncate())
        * (1.0 / state.screen_factor);

    Ok(())
}

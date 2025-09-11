//! Interaction system for ImGuizmo
//!
//! This module handles mouse and keyboard interactions with gizmo elements.

use crate::context::GuizmoState;
use crate::gizmo::ManipulationType;
use crate::types::{Mat4, Rect, Vec2, Vec3};
use crate::{GuizmoError, GuizmoResult};
use dear_imgui::Ui;

/// Result of a hit test operation
#[derive(Debug, Clone, PartialEq)]
pub struct HitTestResult {
    /// Whether the mouse hit a gizmo element
    pub hit: bool,
    /// The type of manipulation that was hit
    pub manipulation_type: ManipulationType,
    /// Distance from camera (for depth sorting)
    pub distance: f32,
    /// Hit proportion along the axis/plane (0.0 to 1.0)
    pub proportion: Vec3,
}

impl Default for HitTestResult {
    fn default() -> Self {
        Self {
            hit: false,
            manipulation_type: ManipulationType::None,
            distance: f32::INFINITY,
            proportion: Vec3::ZERO,
        }
    }
}

/// Check if mouse is over a gizmo element
pub fn is_over_gizmo(ui: &Ui, state: &GuizmoState, mouse_pos: Vec2) -> GuizmoResult<HitTestResult> {
    let mut best_result = HitTestResult::default();

    // Test translation axes
    if let Some(result) = test_translation_axes(state, mouse_pos)? {
        if result.hit && result.distance < best_result.distance {
            best_result = result;
        }
    }

    // Test translation planes
    if let Some(result) = test_translation_planes(state, mouse_pos)? {
        if result.hit && result.distance < best_result.distance {
            best_result = result;
        }
    }

    // Test rotation circles
    if let Some(result) = test_rotation_circles(state, mouse_pos)? {
        if result.hit && result.distance < best_result.distance {
            best_result = result;
        }
    }

    // Test scale handles
    if let Some(result) = test_scale_handles(state, mouse_pos)? {
        if result.hit && result.distance < best_result.distance {
            best_result = result;
        }
    }

    Ok(best_result)
}

/// Handle mouse interaction with gizmo
pub fn handle_mouse_interaction(
    ui: &Ui,
    state: &mut GuizmoState,
    mouse_pos: Vec2,
    mouse_down: bool,
    mouse_clicked: bool,
    mouse_released: bool,
) -> GuizmoResult<bool> {
    let mut modified = false;

    // Check for hover
    let hit_result = is_over_gizmo(ui, state, mouse_pos)?;
    state.is_over = hit_result.hit;

    if mouse_clicked && hit_result.hit {
        // Start manipulation
        state.using = hit_result.manipulation_type;
        state.mouse_down_pos = mouse_pos;
        state.model_source = state.model_matrix;
        state.model_source_inverse = state.model_inverse;

        // Store initial values for different manipulation types
        match hit_result.manipulation_type {
            ManipulationType::RotateX | ManipulationType::RotateY | ManipulationType::RotateZ => {
                state.rotation_start_angle = calculate_rotation_angle(state, mouse_pos)?;
            }
            ManipulationType::ScaleX
            | ManipulationType::ScaleY
            | ManipulationType::ScaleZ
            | ManipulationType::ScaleXYZ => {
                state.scale_start_mouse_pos = mouse_pos;
            }
            _ => {}
        }

        modified = true;
    } else if mouse_released && state.using != ManipulationType::None {
        // End manipulation
        state.using = ManipulationType::None;
        modified = true;
    } else if mouse_down && state.using != ManipulationType::None {
        // Continue manipulation
        let delta = mouse_pos - state.mouse_down_pos;

        match state.using {
            ManipulationType::MoveX
            | ManipulationType::MoveY
            | ManipulationType::MoveZ
            | ManipulationType::MoveXY
            | ManipulationType::MoveYZ
            | ManipulationType::MoveZX => {
                modified = handle_translation_interaction(state, mouse_pos, delta)?;
            }
            ManipulationType::RotateX | ManipulationType::RotateY | ManipulationType::RotateZ => {
                modified = handle_rotation_interaction(state, mouse_pos)?;
            }
            ManipulationType::ScaleX
            | ManipulationType::ScaleY
            | ManipulationType::ScaleZ
            | ManipulationType::ScaleXYZ => {
                modified = handle_scale_interaction(state, mouse_pos, delta)?;
            }
            _ => {}
        }
    }

    Ok(modified)
}

/// Test collision with translation axes
fn test_translation_axes(
    state: &GuizmoState,
    mouse_pos: Vec2,
) -> GuizmoResult<Option<HitTestResult>> {
    let gizmo_center = state.model_matrix.transform_point3(Vec3::ZERO);
    let screen_center =
        crate::draw::project_to_screen(&state.mvp, gizmo_center, &state.viewport.as_viewport())?;
    let gizmo_size = crate::draw::calculate_gizmo_size(
        &state.view_projection,
        gizmo_center,
        state.gizmo_size_clip_space,
    );

    let axis_length = gizmo_size * 0.8;
    let hit_threshold = 10.0; // pixels

    let mut best_result = HitTestResult::default();

    // Test X axis (red)
    let x_end = Vec2::new(screen_center.x + axis_length, screen_center.y);
    let x_distance = distance_point_to_line_segment(mouse_pos, screen_center, x_end);
    if x_distance < hit_threshold {
        let distance = (gizmo_center - state.camera_eye).length();
        if distance < best_result.distance {
            best_result = HitTestResult {
                hit: true,
                manipulation_type: ManipulationType::MoveX,
                distance,
                proportion: Vec3::new(1.0, 0.0, 0.0),
            };
        }
    }

    // Test Y axis (green)
    let y_end = Vec2::new(screen_center.x, screen_center.y - axis_length);
    let y_distance = distance_point_to_line_segment(mouse_pos, screen_center, y_end);
    if y_distance < hit_threshold {
        let distance = (gizmo_center - state.camera_eye).length();
        if distance < best_result.distance {
            best_result = HitTestResult {
                hit: true,
                manipulation_type: ManipulationType::MoveY,
                distance,
                proportion: Vec3::new(0.0, 1.0, 0.0),
            };
        }
    }

    // Test Z axis (blue) - simplified 2D projection
    let z_end = Vec2::new(
        screen_center.x + axis_length * 0.7,
        screen_center.y + axis_length * 0.7,
    );
    let z_distance = distance_point_to_line_segment(mouse_pos, screen_center, z_end);
    if z_distance < hit_threshold {
        let distance = (gizmo_center - state.camera_eye).length();
        if distance < best_result.distance {
            best_result = HitTestResult {
                hit: true,
                manipulation_type: ManipulationType::MoveZ,
                distance,
                proportion: Vec3::new(0.0, 0.0, 1.0),
            };
        }
    }

    Ok(if best_result.hit {
        Some(best_result)
    } else {
        None
    })
}

/// Test collision with translation planes
fn test_translation_planes(
    state: &GuizmoState,
    mouse_pos: Vec2,
) -> GuizmoResult<Option<HitTestResult>> {
    let gizmo_center = state.model_matrix.transform_point3(Vec3::ZERO);
    let screen_center =
        crate::draw::project_to_screen(&state.mvp, gizmo_center, &state.viewport.as_viewport())?;
    let gizmo_size = crate::draw::calculate_gizmo_size(
        &state.view_projection,
        gizmo_center,
        state.gizmo_size_clip_space,
    );

    let plane_size = gizmo_size * 0.3;
    let mut best_result = HitTestResult::default();

    // Test XY plane
    let xy_rect = Rect::new(
        screen_center.x + plane_size * 0.2,
        screen_center.y - plane_size,
        plane_size * 0.8,
        plane_size * 0.8,
    );
    if xy_rect.contains(mouse_pos.x, mouse_pos.y) {
        let distance = (gizmo_center - state.camera_eye).length();
        best_result = HitTestResult {
            hit: true,
            manipulation_type: ManipulationType::MoveXY,
            distance,
            proportion: Vec3::new(1.0, 1.0, 0.0),
        };
    }

    // Test XZ plane
    let xz_rect = Rect::new(
        screen_center.x + plane_size * 0.2,
        screen_center.y + plane_size * 0.2,
        plane_size * 0.8,
        plane_size * 0.8,
    );
    if xz_rect.contains(mouse_pos.x, mouse_pos.y) {
        let distance = (gizmo_center - state.camera_eye).length();
        if distance < best_result.distance || !best_result.hit {
            best_result = HitTestResult {
                hit: true,
                manipulation_type: ManipulationType::MoveZX,
                distance,
                proportion: Vec3::new(1.0, 0.0, 1.0),
            };
        }
    }

    // Test YZ plane
    let yz_rect = Rect::new(
        screen_center.x - plane_size,
        screen_center.y - plane_size,
        plane_size * 0.8,
        plane_size * 0.8,
    );
    if yz_rect.contains(mouse_pos.x, mouse_pos.y) {
        let distance = (gizmo_center - state.camera_eye).length();
        if distance < best_result.distance || !best_result.hit {
            best_result = HitTestResult {
                hit: true,
                manipulation_type: ManipulationType::MoveYZ,
                distance,
                proportion: Vec3::new(0.0, 1.0, 1.0),
            };
        }
    }

    Ok(if best_result.hit {
        Some(best_result)
    } else {
        None
    })
}

/// Test collision with rotation circles
fn test_rotation_circles(
    state: &GuizmoState,
    mouse_pos: Vec2,
) -> GuizmoResult<Option<HitTestResult>> {
    let gizmo_center = state.model_matrix.transform_point3(Vec3::ZERO);
    let screen_center =
        crate::draw::project_to_screen(&state.mvp, gizmo_center, &state.viewport.as_viewport())?;
    let gizmo_size = crate::draw::calculate_gizmo_size(
        &state.view_projection,
        gizmo_center,
        state.gizmo_size_clip_space,
    );

    let radius = gizmo_size * 0.8;
    let thickness = 15.0; // pixels
    let distance_to_center = (mouse_pos - screen_center).length();

    let mut best_result = HitTestResult::default();

    // Test if mouse is near any rotation circle
    if (distance_to_center - radius).abs() < thickness {
        let distance = (gizmo_center - state.camera_eye).length();

        // For simplicity, we'll just return X rotation for now
        // In a full implementation, you'd determine which axis based on camera orientation
        best_result = HitTestResult {
            hit: true,
            manipulation_type: ManipulationType::RotateX,
            distance,
            proportion: Vec3::new(1.0, 0.0, 0.0),
        };
    }

    Ok(if best_result.hit {
        Some(best_result)
    } else {
        None
    })
}

/// Test collision with scale handles
fn test_scale_handles(state: &GuizmoState, mouse_pos: Vec2) -> GuizmoResult<Option<HitTestResult>> {
    let gizmo_center = state.model_matrix.transform_point3(Vec3::ZERO);
    let screen_center =
        crate::draw::project_to_screen(&state.mvp, gizmo_center, &state.viewport.as_viewport())?;
    let gizmo_size = crate::draw::calculate_gizmo_size(
        &state.view_projection,
        gizmo_center,
        state.gizmo_size_clip_space,
    );

    let axis_length = gizmo_size * 0.8;
    let handle_size = gizmo_size * 0.08;
    let mut best_result = HitTestResult::default();

    // Test X scale handle
    let x_handle_pos = Vec2::new(screen_center.x + axis_length, screen_center.y);
    if (mouse_pos - x_handle_pos).length() < handle_size {
        let distance = (gizmo_center - state.camera_eye).length();
        best_result = HitTestResult {
            hit: true,
            manipulation_type: ManipulationType::ScaleX,
            distance,
            proportion: Vec3::new(1.0, 0.0, 0.0),
        };
    }

    // Test Y scale handle
    let y_handle_pos = Vec2::new(screen_center.x, screen_center.y - axis_length);
    if (mouse_pos - y_handle_pos).length() < handle_size {
        let distance = (gizmo_center - state.camera_eye).length();
        if distance < best_result.distance || !best_result.hit {
            best_result = HitTestResult {
                hit: true,
                manipulation_type: ManipulationType::ScaleY,
                distance,
                proportion: Vec3::new(0.0, 1.0, 0.0),
            };
        }
    }

    // Test Z scale handle
    let z_handle_pos = Vec2::new(
        screen_center.x + axis_length * 0.7,
        screen_center.y + axis_length * 0.7,
    );
    if (mouse_pos - z_handle_pos).length() < handle_size {
        let distance = (gizmo_center - state.camera_eye).length();
        if distance < best_result.distance || !best_result.hit {
            best_result = HitTestResult {
                hit: true,
                manipulation_type: ManipulationType::ScaleZ,
                distance,
                proportion: Vec3::new(0.0, 0.0, 1.0),
            };
        }
    }

    Ok(if best_result.hit {
        Some(best_result)
    } else {
        None
    })
}

// Helper functions for geometric calculations

/// Calculate distance from point to line segment
fn distance_point_to_line_segment(point: Vec2, line_start: Vec2, line_end: Vec2) -> f32 {
    let line_vec = line_end - line_start;
    let point_vec = point - line_start;

    let line_len_sq = line_vec.length_squared();
    if line_len_sq < f32::EPSILON {
        return point_vec.length();
    }

    let t = (point_vec.dot(line_vec) / line_len_sq).clamp(0.0, 1.0);
    let projection = line_start + line_vec * t;
    (point - projection).length()
}

/// Calculate rotation angle for rotation manipulation
fn calculate_rotation_angle(state: &GuizmoState, mouse_pos: Vec2) -> GuizmoResult<f32> {
    let gizmo_center = state.model_matrix.transform_point3(Vec3::ZERO);
    let screen_center =
        crate::draw::project_to_screen(&state.mvp, gizmo_center, &state.viewport.as_viewport())?;

    let mouse_vec = mouse_pos - screen_center;
    Ok(mouse_vec.y.atan2(mouse_vec.x))
}

/// Handle translation interaction
fn handle_translation_interaction(
    state: &mut GuizmoState,
    mouse_pos: Vec2,
    _delta: Vec2,
) -> GuizmoResult<bool> {
    // Calculate translation based on mouse movement
    let gizmo_center = state.model_matrix.transform_point3(Vec3::ZERO);
    let screen_center =
        crate::draw::project_to_screen(&state.mvp, gizmo_center, &state.viewport.as_viewport())?;

    let mouse_delta = mouse_pos - state.mouse_down_pos;
    let sensitivity = 0.01; // Adjust as needed

    let mut translation = Vec3::ZERO;

    match state.using {
        ManipulationType::MoveX => {
            translation.x = mouse_delta.x * sensitivity;
        }
        ManipulationType::MoveY => {
            translation.y = -mouse_delta.y * sensitivity; // Invert Y for screen coordinates
        }
        ManipulationType::MoveZ => {
            translation.z = mouse_delta.x * sensitivity; // Use X movement for Z
        }
        ManipulationType::MoveXY => {
            translation.x = mouse_delta.x * sensitivity;
            translation.y = -mouse_delta.y * sensitivity;
        }
        ManipulationType::MoveYZ => {
            translation.y = -mouse_delta.y * sensitivity;
            translation.z = mouse_delta.x * sensitivity;
        }
        ManipulationType::MoveZX => {
            translation.z = mouse_delta.x * sensitivity;
            translation.x = mouse_delta.y * sensitivity;
        }
        _ => return Ok(false),
    }

    // Apply translation to model matrix
    let translation_matrix = Mat4::from_translation(translation);
    state.model_matrix = translation_matrix * state.model_source;
    state.model_inverse = state.model_matrix.inverse();

    Ok(true)
}

/// Handle rotation interaction
fn handle_rotation_interaction(state: &mut GuizmoState, mouse_pos: Vec2) -> GuizmoResult<bool> {
    let current_angle = calculate_rotation_angle(state, mouse_pos)?;
    let angle_delta = current_angle - state.rotation_start_angle;

    let mut rotation_axis = Vec3::ZERO;
    match state.using {
        ManipulationType::RotateX => rotation_axis.x = 1.0,
        ManipulationType::RotateY => rotation_axis.y = 1.0,
        ManipulationType::RotateZ => rotation_axis.z = 1.0,
        _ => return Ok(false),
    }

    // Create rotation matrix
    let rotation_matrix = Mat4::from_axis_angle(rotation_axis, angle_delta);

    // Apply rotation to model matrix
    state.model_matrix = rotation_matrix * state.model_source;
    state.model_inverse = state.model_matrix.inverse();

    Ok(true)
}

/// Handle scale interaction
fn handle_scale_interaction(
    state: &mut GuizmoState,
    mouse_pos: Vec2,
    delta: Vec2,
) -> GuizmoResult<bool> {
    let sensitivity = 0.01;
    let scale_factor = 1.0 + delta.length() * sensitivity;

    let mut scale = Vec3::ONE;
    match state.using {
        ManipulationType::ScaleX => {
            scale.x = scale_factor;
        }
        ManipulationType::ScaleY => {
            scale.y = scale_factor;
        }
        ManipulationType::ScaleZ => {
            scale.z = scale_factor;
        }
        ManipulationType::ScaleXYZ => {
            scale = Vec3::splat(scale_factor);
        }
        _ => return Ok(false),
    }

    // Create scale matrix
    let scale_matrix = Mat4::from_scale(scale);

    // Apply scale to model matrix
    state.model_matrix = scale_matrix * state.model_source;
    state.model_inverse = state.model_matrix.inverse();

    Ok(true)
}

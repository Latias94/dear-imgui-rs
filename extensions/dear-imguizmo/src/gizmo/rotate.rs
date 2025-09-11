//! Rotation manipulation implementation
//!
//! This module handles rotation gizmo operations, including mouse interaction
//! and matrix transformations for rotation manipulations.

use crate::{
    context::GuizmoContext,
    draw::{calculate_gizmo_size, project_to_screen},
    error::{GuizmoError, GuizmoResult},
    gizmo::ManipulationType,
    math::*,
    types::*,
};
use dear_imgui::Ui;
use glam::{Mat4, Quat, Vec2, Vec3, Vec4};

/// Handle rotation manipulation
pub fn handle_rotation(
    ui: &Ui,
    context: &GuizmoContext,
    operation: Operation,
    manipulation_type: ManipulationType,
) -> GuizmoResult<bool> {
    let mut state = context.state.borrow_mut();

    // Only handle rotation operations
    if !manipulation_type.is_rotate_type() {
        return Ok(false);
    }

    let io = ui.io();
    let mouse_pos_array = io.mouse_pos();
    let mouse_pos = Vec2::new(mouse_pos_array[0], mouse_pos_array[1]);
    let is_mouse_down = ui.is_mouse_down(dear_imgui::MouseButton::Left);
    let is_mouse_clicked = ui.is_mouse_clicked(dear_imgui::MouseButton::Left);
    let is_mouse_released = ui.is_mouse_released(dear_imgui::MouseButton::Left);

    // Get current matrices
    let model_matrix = state.model_matrix;
    let view_matrix = state.view_matrix;
    let projection_matrix = state.projection_matrix;
    let mvp = state.mvp;

    // Calculate gizmo center in world space
    let gizmo_center = model_matrix.transform_point3(Vec3::ZERO);

    // Project to screen space
    let screen_center = project_to_screen(&mvp, gizmo_center, &state.viewport.as_viewport())?;

    // Calculate gizmo size
    let gizmo_size = calculate_gizmo_size(
        &state.view_projection,
        gizmo_center,
        state.gizmo_size_clip_space,
    );

    // Check if mouse is over rotation handles
    let mut is_over = false;
    let mut active_axis = None;

    match manipulation_type {
        ManipulationType::RotateX => {
            // Check if mouse is over X rotation circle
            let distance = (mouse_pos - screen_center).length();
            if distance >= gizmo_size * 0.8 && distance <= gizmo_size * 1.2 {
                is_over = true;
                active_axis = Some(0);
            }
        }
        ManipulationType::RotateY => {
            // Check if mouse is over Y rotation circle
            let distance = (mouse_pos - screen_center).length();
            if distance >= gizmo_size * 0.9 && distance <= gizmo_size * 1.1 {
                is_over = true;
                active_axis = Some(1);
            }
        }
        ManipulationType::RotateZ => {
            // Check if mouse is over Z rotation circle
            let distance = (mouse_pos - screen_center).length();
            if distance >= gizmo_size * 1.0 && distance <= gizmo_size * 1.3 {
                is_over = true;
                active_axis = Some(2);
            }
        }
        ManipulationType::RotateScreen => {
            // Screen space rotation - outer ring
            let distance = (mouse_pos - screen_center).length();
            if distance >= gizmo_size * 1.2 && distance <= gizmo_size * 1.5 {
                is_over = true;
                active_axis = Some(3); // Screen axis
            }
        }
        _ => {}
    }

    state.is_over = is_over;

    // Handle mouse interaction
    if is_over && is_mouse_clicked {
        // Start rotation
        state.using = manipulation_type;
        state.current_manipulation_type = manipulation_type;

        // Store initial state
        state.model_source = model_matrix;
        state.model_source_inverse = model_matrix.inverse();

        // Calculate initial rotation angle
        let mouse_dir = (mouse_pos - screen_center).normalize();
        state.rotation_start_angle = mouse_dir.y.atan2(mouse_dir.x);

        return Ok(true);
    }

    if state.using == manipulation_type && is_mouse_down {
        // Continue rotation
        let mouse_dir = (mouse_pos - screen_center).normalize();
        let current_angle = mouse_dir.y.atan2(mouse_dir.x);
        let angle_delta = current_angle - state.rotation_start_angle;

        // Apply rotation based on axis
        let rotation_quat = match active_axis {
            Some(0) => {
                // X axis rotation
                let x_axis = state.model_source.transform_vector3(Vec3::X).normalize();
                Quat::from_axis_angle(x_axis, angle_delta)
            }
            Some(1) => {
                // Y axis rotation
                let y_axis = state.model_source.transform_vector3(Vec3::Y).normalize();
                Quat::from_axis_angle(y_axis, angle_delta)
            }
            Some(2) => {
                // Z axis rotation
                let z_axis = state.model_source.transform_vector3(Vec3::Z).normalize();
                Quat::from_axis_angle(z_axis, angle_delta)
            }
            Some(3) => {
                // Screen space rotation
                let view_dir = state.camera_dir;
                Quat::from_axis_angle(view_dir, angle_delta)
            }
            _ => Quat::IDENTITY,
        };

        // Apply rotation to model matrix
        let rotation_matrix = Mat4::from_quat(rotation_quat);
        state.model_matrix = rotation_matrix * state.model_source;

        return Ok(true);
    }

    if state.using == manipulation_type && is_mouse_released {
        // End rotation
        state.using = ManipulationType::None;
        return Ok(true);
    }

    Ok(false)
}

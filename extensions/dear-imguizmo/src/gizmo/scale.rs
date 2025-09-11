//! Scale manipulation implementation
//!
//! This module handles scale gizmo operations, including mouse interaction
//! and matrix transformations for scaling manipulations.

use crate::{
    context::GuizmoContext,
    draw::{calculate_gizmo_size, project_to_screen},
    error::GuizmoResult,
    gizmo::ManipulationType,
    types::*,
};
use dear_imgui::Ui;
use glam::{Mat4, Vec2, Vec3};

/// Handle scale manipulation
pub fn handle_scale(
    ui: &Ui,
    context: &GuizmoContext,
    _operation: Operation,
    manipulation_type: ManipulationType,
) -> GuizmoResult<bool> {
    let mut state = context.state.borrow_mut();

    // Only handle scale operations
    if !manipulation_type.is_scale_type() {
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

    // Check if mouse is over scale handles
    let mut is_over = false;
    let mut active_axis = None;

    match manipulation_type {
        ManipulationType::ScaleX => {
            // Check if mouse is over X scale handle
            let handle_pos = screen_center + Vec2::new(gizmo_size * 0.8, 0.0);
            let distance = (mouse_pos - handle_pos).length();
            if distance <= gizmo_size * 0.1 {
                is_over = true;
                active_axis = Some(0);
            }
        }
        ManipulationType::ScaleY => {
            // Check if mouse is over Y scale handle
            let handle_pos = screen_center + Vec2::new(0.0, -gizmo_size * 0.8);
            let distance = (mouse_pos - handle_pos).length();
            if distance <= gizmo_size * 0.1 {
                is_over = true;
                active_axis = Some(1);
            }
        }
        ManipulationType::ScaleZ => {
            // Check if mouse is over Z scale handle
            let handle_pos = screen_center + Vec2::new(gizmo_size * 0.6, gizmo_size * 0.6);
            let distance = (mouse_pos - handle_pos).length();
            if distance <= gizmo_size * 0.1 {
                is_over = true;
                active_axis = Some(2);
            }
        }
        ManipulationType::ScaleXYZ => {
            // Check if mouse is over center scale handle (uniform scaling)
            let distance = (mouse_pos - screen_center).length();
            if distance <= gizmo_size * 0.15 {
                is_over = true;
                active_axis = Some(3); // Uniform scale
            }
        }
        _ => {}
    }

    state.is_over = is_over;

    // Handle mouse interaction
    if is_over && is_mouse_clicked {
        // Start scaling
        state.using = manipulation_type;
        state.current_manipulation_type = manipulation_type;

        // Store initial state
        state.model_source = model_matrix;
        state.model_source_inverse = model_matrix.inverse();
        state.scale_start_mouse_pos = mouse_pos;

        return Ok(true);
    }

    if state.using == manipulation_type && is_mouse_down {
        // Continue scaling
        let mouse_delta = mouse_pos - state.scale_start_mouse_pos;
        let scale_sensitivity = 0.01;

        // Calculate scale factor based on mouse movement
        let scale_factor = match active_axis {
            Some(0) => {
                // X axis scaling
                let delta = mouse_delta.x * scale_sensitivity;
                Vec3::new(1.0 + delta, 1.0, 1.0)
            }
            Some(1) => {
                // Y axis scaling
                let delta = -mouse_delta.y * scale_sensitivity; // Invert Y for screen coordinates
                Vec3::new(1.0, 1.0 + delta, 1.0)
            }
            Some(2) => {
                // Z axis scaling
                let delta = mouse_delta.x * scale_sensitivity;
                Vec3::new(1.0, 1.0, 1.0 + delta)
            }
            Some(3) => {
                // Uniform scaling
                let delta = (mouse_delta.x - mouse_delta.y) * scale_sensitivity * 0.5;
                Vec3::splat(1.0 + delta)
            }
            _ => Vec3::ONE,
        };

        // Clamp scale factors to prevent negative or zero scaling
        let scale_factor = Vec3::new(
            scale_factor.x.max(0.01),
            scale_factor.y.max(0.01),
            scale_factor.z.max(0.01),
        );

        // Apply scaling to model matrix
        let scale_matrix = Mat4::from_scale(scale_factor);
        state.model_matrix = scale_matrix * state.model_source;

        return Ok(true);
    }

    if state.using == manipulation_type && is_mouse_released {
        // End scaling
        state.using = ManipulationType::None;
        return Ok(true);
    }

    Ok(false)
}

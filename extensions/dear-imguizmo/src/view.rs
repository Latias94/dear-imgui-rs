//! View manipulation functionality
//!
//! This module handles camera/view manipulation operations, including
//! view cube, camera rotation, and view matrix manipulation.

use crate::{context::GuizmoContext, error::GuizmoResult, types::*};
use dear_imgui::Ui;
use glam::{Mat4, Quat, Vec2, Vec3};

/// ViewManipulate result
#[derive(Debug, Clone, Default)]
pub struct ViewManipulateResult {
    /// Whether the view was modified
    pub modified: bool,
    /// Whether the mouse is over the view manipulator
    pub is_over: bool,
    /// Whether the view manipulator is being used
    pub is_using: bool,
}

/// Handle view manipulation with a view cube
pub fn view_manipulate(
    ui: &Ui,
    context: &GuizmoContext,
    view: &mut Mat4,
    length: f32,
    position: [f32; 2],
    size: [f32; 2],
    background_color: u32,
) -> GuizmoResult<ViewManipulateResult> {
    let mut state = context.state.borrow_mut();
    let mut result = ViewManipulateResult::default();

    let io = ui.io();
    let mouse_pos_array = io.mouse_pos();
    let mouse_pos = Vec2::new(mouse_pos_array[0], mouse_pos_array[1]);
    let is_mouse_down = ui.is_mouse_down(dear_imgui::MouseButton::Left);
    let is_mouse_clicked = ui.is_mouse_clicked(dear_imgui::MouseButton::Left);
    let is_mouse_released = ui.is_mouse_released(dear_imgui::MouseButton::Left);

    // Define view manipulator rectangle
    let view_rect = Rect::new(position[0], position[1], size[0], size[1]);

    // Check if mouse is over the view manipulator
    result.is_over = view_rect.contains(mouse_pos.x, mouse_pos.y);
    state.is_view_manipulate_hovered = result.is_over;

    // Get draw list for rendering
    let draw_list = ui.get_window_draw_list();

    // Draw background
    draw_list
        .add_rect(
            [view_rect.x, view_rect.y],
            [view_rect.right(), view_rect.bottom()],
            background_color,
        )
        .filled(true)
        .build();

    // Draw border
    draw_list
        .add_rect(
            [view_rect.x, view_rect.y],
            [view_rect.right(), view_rect.bottom()],
            0xFF000000, // Black border
        )
        .build();

    // Calculate view cube center
    let cube_center = Vec2::new(
        view_rect.x + view_rect.width * 0.5,
        view_rect.y + view_rect.height * 0.5,
    );

    // Extract camera direction from view matrix
    let view_forward = Vec3::new(view.z_axis.x, view.z_axis.y, view.z_axis.z).normalize();
    let view_right = Vec3::new(view.x_axis.x, view.x_axis.y, view.x_axis.z).normalize();
    let view_up = Vec3::new(view.y_axis.x, view.y_axis.y, view.y_axis.z).normalize();

    // Draw view cube faces
    draw_view_cube_faces(
        &draw_list,
        cube_center,
        length * 0.4,
        &view_forward,
        &view_right,
        &view_up,
    )?;

    // Handle mouse interaction
    if result.is_over && is_mouse_clicked {
        // Start view manipulation
        state.is_using_view_manipulate = true;
        state.mouse_down_pos = mouse_pos;
        state.model_source = *view;
        result.is_using = true;
    }

    if state.is_using_view_manipulate && is_mouse_down {
        // Continue view manipulation
        let mouse_delta = mouse_pos - state.mouse_down_pos;
        let sensitivity = 0.01;

        // Calculate rotation based on mouse movement
        let rotation_x = Quat::from_axis_angle(view_right, -mouse_delta.y * sensitivity);
        let rotation_y = Quat::from_axis_angle(Vec3::Y, -mouse_delta.x * sensitivity);
        let combined_rotation = rotation_y * rotation_x;

        // Apply rotation to view matrix
        let rotation_matrix = Mat4::from_quat(combined_rotation);
        *view = rotation_matrix * state.model_source;

        result.modified = true;
        result.is_using = true;
    }

    if state.is_using_view_manipulate && is_mouse_released {
        // End view manipulation
        state.is_using_view_manipulate = false;
        result.modified = true;
    }

    result.is_using = state.is_using_view_manipulate;

    Ok(result)
}

/// Draw view cube faces
fn draw_view_cube_faces(
    draw_list: &dear_imgui::DrawListMut,
    center: Vec2,
    size: f32,
    forward: &Vec3,
    right: &Vec3,
    up: &Vec3,
) -> GuizmoResult<()> {
    // Define cube face normals
    let face_normals = [
        Vec3::X,     // Right
        Vec3::NEG_X, // Left
        Vec3::Y,     // Up
        Vec3::NEG_Y, // Down
        Vec3::Z,     // Forward
        Vec3::NEG_Z, // Back
    ];

    let face_colors = [
        0xFF0000FF, // Red (Right)
        0xFF00FFFF, // Cyan (Left)
        0xFF00FF00, // Green (Up)
        0xFFFF00FF, // Magenta (Down)
        0xFFFF0000, // Blue (Forward)
        0xFFFFFF00, // Yellow (Back)
    ];

    let _face_labels = ["X", "-X", "Y", "-Y", "Z", "-Z"];

    // Calculate face visibility and positions
    for (i, &normal) in face_normals.iter().enumerate() {
        // Check if face is visible (facing towards camera)
        let dot_product = normal.dot(*forward);
        if dot_product > 0.0 {
            continue; // Face is facing away from camera
        }

        // Project face center to screen
        let face_center_3d = normal * size * 0.5;

        // Simple orthographic projection for the view cube
        let screen_x = center.x + face_center_3d.dot(*right) * size;
        let screen_y = center.y - face_center_3d.dot(*up) * size;

        let face_size = size * 0.3;

        // Draw face as a rectangle
        draw_list
            .add_rect(
                [screen_x - face_size * 0.5, screen_y - face_size * 0.5],
                [screen_x + face_size * 0.5, screen_y + face_size * 0.5],
                face_colors[i],
            )
            .filled(true)
            .build();

        // Draw face border
        draw_list
            .add_rect(
                [screen_x - face_size * 0.5, screen_y - face_size * 0.5],
                [screen_x + face_size * 0.5, screen_y + face_size * 0.5],
                0xFF000000, // Black border
            )
            .build();

        // Draw face label (simplified - just draw a small circle for now)
        draw_list
            .add_circle(
                [screen_x, screen_y],
                face_size * 0.1,
                0xFFFFFFFF, // White
            )
            .filled(true)
            .build();
    }

    Ok(())
}

/// Snap view to predefined orientations
pub fn snap_view_to_direction(view: &mut Mat4, direction: ViewDirection) -> GuizmoResult<()> {
    let (forward, up) = match direction {
        ViewDirection::Front => (Vec3::NEG_Z, Vec3::Y),
        ViewDirection::Back => (Vec3::Z, Vec3::Y),
        ViewDirection::Left => (Vec3::NEG_X, Vec3::Y),
        ViewDirection::Right => (Vec3::X, Vec3::Y),
        ViewDirection::Top => (Vec3::NEG_Y, Vec3::Z),
        ViewDirection::Bottom => (Vec3::Y, Vec3::NEG_Z),
    };

    let right = up.cross(forward).normalize();
    let up = forward.cross(right).normalize();

    // Create new view matrix
    *view = Mat4::look_at_rh(
        Vec3::ZERO, // Eye position (will be adjusted by caller)
        forward,    // Target direction
        up,         // Up vector
    );

    Ok(())
}

/// Predefined view directions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewDirection {
    Front,
    Back,
    Left,
    Right,
    Top,
    Bottom,
}

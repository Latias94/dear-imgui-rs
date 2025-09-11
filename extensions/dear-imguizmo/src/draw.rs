//! Drawing system for ImGuizmo
//!
//! This module handles all the rendering operations for gizmo elements.

use crate::context::GuizmoContext;
use crate::gizmo::ManipulationType;
use crate::types::{Color, Mat4, Mode, Operation, Rect, Vec2, Vec3, Vec4};
use crate::{GuizmoError, GuizmoResult};
use dear_imgui::{DrawListMut, Ui};
use glam::Vec4Swizzles;

/// Colors for gizmo axes
pub const AXIS_COLORS: [u32; 4] = [
    0xFF0000FF, // Red for X axis
    0xFF00FF00, // Green for Y axis
    0xFFFF0000, // Blue for Z axis
    0xFFFFFFFF, // White for universal/screen
];

/// Highlighted colors for gizmo axes
pub const AXIS_COLORS_HIGHLIGHTED: [u32; 4] = [
    0xFF4444FF, // Lighter red for X axis
    0xFF44FF44, // Lighter green for Y axis
    0xFFFF4444, // Lighter blue for Z axis
    0xFFFFFFFF, // White for universal/screen
];

/// Draw a translation gizmo
pub fn draw_translation_gizmo(
    draw_list: &DrawListMut,
    mvp: Mat4,
    model_matrix: Mat4,
    viewport: Rect,
    gizmo_size: f32,
    operation: Operation,
    manipulation_type: ManipulationType,
) -> GuizmoResult<()> {
    println!(
        "DEBUG: draw_translation_gizmo called with operation: {:?}, manipulation_type: {:?}",
        operation, manipulation_type
    );

    if !operation.intersects(Operation::TRANSLATE) {
        println!("DEBUG: Operation does not intersect TRANSLATE, returning");
        return Ok(());
    }

    println!("DEBUG: Proceeding with translation gizmo drawing");

    // Get transformation matrices
    let model_view_proj = mvp;
    let model = model_matrix;

    // Transform gizmo center to screen space
    let gizmo_center = model.transform_point3(Vec3::ZERO);
    let _screen_center =
        project_to_screen(&model_view_proj, gizmo_center, &viewport.as_viewport())?;

    // Debug: Skip 3D projection for now, just draw test lines
    // draw_translation_axes(&draw_list, &model_view_proj, &model, &viewport.as_viewport(), gizmo_size, ManipulationType::None)?;

    // Debug: Draw a simple test line to verify drawing works
    println!("DEBUG: Drawing test line at [100,100] to [200,200]");
    // Green color: 0xAA_BB_GG_RR format = 0xFF_00_FF_00 (opaque green)
    draw_list
        .add_line([100.0, 100.0], [200.0, 200.0], 0xFF00FF00)
        .thickness(5.0)
        .build();

    // Debug: Draw test axes at screen center
    let viewport_array = viewport.as_viewport();
    let center_x = viewport_array[2] / 2.0;
    let center_y = viewport_array[3] / 2.0;
    let axis_len = 100.0;

    println!(
        "DEBUG: Drawing test axes at center [{}, {}] with length {}",
        center_x, center_y, axis_len
    );

    // X axis - red: 0xFF_00_00_FF (opaque red)
    draw_list
        .add_line(
            [center_x, center_y],
            [center_x + axis_len, center_y],
            0xFF0000FF,
        )
        .thickness(3.0)
        .build();
    // Y axis - green: 0xFF_00_FF_00 (opaque green)
    draw_list
        .add_line(
            [center_x, center_y],
            [center_x, center_y + axis_len],
            0xFF00FF00,
        )
        .thickness(3.0)
        .build();
    // Z axis - blue: 0xFF_FF_00_00 (opaque blue)
    draw_list
        .add_line(
            [center_x, center_y],
            [center_x + axis_len * 0.7, center_y - axis_len * 0.7],
            0xFFFF0000,
        )
        .thickness(3.0)
        .build();

    // Draw plane handles if needed
    if manipulation_type.is_plane_type() {
        draw_translation_planes(
            &draw_list,
            &model_view_proj,
            &model,
            &viewport.as_viewport(),
            gizmo_size,
            manipulation_type,
        )?;
    }

    Ok(())
}

/// Draw rotation gizmo
pub fn draw_rotation_gizmo(
    draw_list: &DrawListMut,
    context: &GuizmoContext,
    operation: Operation,
    _manipulation_type: ManipulationType,
) -> GuizmoResult<()> {
    if !operation.intersects(Operation::ROTATE) {
        return Ok(());
    }

    let state = context.state.borrow();

    // Get transformation matrices
    let model_view_proj = state.mvp;
    let model = state.model_matrix;

    // Transform gizmo center to screen space
    let gizmo_center = model.transform_point3(Vec3::ZERO);
    let _screen_center = project_to_screen(
        &model_view_proj,
        gizmo_center,
        &state.viewport.as_viewport(),
    )?;

    // Calculate gizmo size based on distance
    let gizmo_size = calculate_gizmo_size(
        &state.view_projection,
        gizmo_center,
        state.gizmo_size_clip_space,
    );

    // Debug: Always draw rotation circles regardless of manipulation type
    draw_rotation_circles(
        &draw_list,
        &model_view_proj,
        &model,
        &state.viewport.as_viewport(),
        gizmo_size,
        ManipulationType::None,
    )?;

    Ok(())
}

/// Draw scale gizmo
pub fn draw_scale_gizmo(
    draw_list: &DrawListMut,
    context: &GuizmoContext,
    operation: Operation,
    _manipulation_type: ManipulationType,
) -> GuizmoResult<()> {
    if !operation.intersects(Operation::SCALE) {
        return Ok(());
    }

    let state = context.state.borrow();

    // Get transformation matrices
    let model_view_proj = state.mvp;
    let model = state.model_matrix;

    // Transform gizmo center to screen space
    let gizmo_center = model.transform_point3(Vec3::ZERO);
    let _screen_center = project_to_screen(
        &model_view_proj,
        gizmo_center,
        &state.viewport.as_viewport(),
    )?;

    // Calculate gizmo size based on distance
    let gizmo_size = calculate_gizmo_size(
        &state.view_projection,
        gizmo_center,
        state.gizmo_size_clip_space,
    );

    // Debug: Always draw scale handles regardless of manipulation type
    draw_scale_handles(
        &draw_list,
        &model_view_proj,
        &model,
        &state.viewport.as_viewport(),
        gizmo_size,
        ManipulationType::None,
    )?;

    Ok(())
}

/// Draw translation axes with proper 3D projection
fn draw_translation_axes(
    draw_list: &DrawListMut,
    mvp: &Mat4,
    model_matrix: &Mat4,
    viewport: &[f32; 4],
    gizmo_size: f32,
    manipulation_type: ManipulationType,
) -> GuizmoResult<()> {
    #[cfg(feature = "tracing")]
    tracing::info!(
        "draw_translation_axes called with gizmo_size: {}",
        gizmo_size
    );
    let axis_length = gizmo_size;
    let arrow_size = gizmo_size * 0.15;

    // Get gizmo center in world space
    let gizmo_center = model_matrix.transform_point3(Vec3::ZERO);

    // Define axis directions in world space
    let x_axis = model_matrix.transform_vector3(Vec3::X);
    let y_axis = model_matrix.transform_vector3(Vec3::Y);
    let z_axis = model_matrix.transform_vector3(Vec3::Z);

    // Draw X axis (red)
    if manipulation_type == ManipulationType::None || manipulation_type.is_x_axis() {
        let color = if manipulation_type == ManipulationType::MoveX {
            AXIS_COLORS_HIGHLIGHTED[0]
        } else {
            AXIS_COLORS[0]
        };

        let axis_end = gizmo_center + x_axis * axis_length;
        let center_screen = project_to_screen(mvp, gizmo_center, viewport)?;
        let end_screen = project_to_screen(mvp, axis_end, viewport)?;

        // Draw axis line
        draw_list
            .add_line(
                [center_screen.x, center_screen.y],
                [end_screen.x, end_screen.y],
                color,
            )
            .thickness(4.0)
            .build();

        // Draw arrow head
        let direction = (end_screen - center_screen).normalize();
        draw_arrow_head(draw_list, end_screen, direction, arrow_size, color)?;
    }

    // Draw Y axis (green)
    if manipulation_type == ManipulationType::None || manipulation_type.is_y_axis() {
        let color = if manipulation_type == ManipulationType::MoveY {
            AXIS_COLORS_HIGHLIGHTED[1]
        } else {
            AXIS_COLORS[1]
        };

        let axis_end = gizmo_center + y_axis * axis_length;
        let center_screen = project_to_screen(mvp, gizmo_center, viewport)?;
        let end_screen = project_to_screen(mvp, axis_end, viewport)?;

        // Draw axis line
        draw_list
            .add_line(
                [center_screen.x, center_screen.y],
                [end_screen.x, end_screen.y],
                color,
            )
            .thickness(4.0)
            .build();

        // Draw arrow head
        let direction = (end_screen - center_screen).normalize();
        draw_arrow_head(draw_list, end_screen, direction, arrow_size, color)?;
    }

    // Draw Z axis (blue)
    if manipulation_type == ManipulationType::None || manipulation_type.is_z_axis() {
        let color = if manipulation_type == ManipulationType::MoveZ {
            AXIS_COLORS_HIGHLIGHTED[2]
        } else {
            AXIS_COLORS[2]
        };

        let axis_end = gizmo_center + z_axis * axis_length;
        let center_screen = project_to_screen(mvp, gizmo_center, viewport)?;
        let end_screen = project_to_screen(mvp, axis_end, viewport)?;

        // Draw axis line
        draw_list
            .add_line(
                [center_screen.x, center_screen.y],
                [end_screen.x, end_screen.y],
                color,
            )
            .thickness(4.0)
            .build();

        // Draw arrow head
        let direction = (end_screen - center_screen).normalize();
        draw_arrow_head(draw_list, end_screen, direction, arrow_size, color)?;
    }

    Ok(())
}

/// Draw translation plane handles with proper 3D projection
fn draw_translation_planes(
    draw_list: &DrawListMut,
    mvp: &Mat4,
    model_matrix: &Mat4,
    viewport: &[f32; 4],
    gizmo_size: f32,
    manipulation_type: ManipulationType,
) -> GuizmoResult<()> {
    let plane_size = gizmo_size * 0.25;
    let alpha = 0x60000000; // Semi-transparent

    // Get gizmo center in world space
    let gizmo_center = model_matrix.transform_point3(Vec3::ZERO);

    // XY plane (blue handle)
    if manipulation_type == ManipulationType::MoveXY {
        let color = (AXIS_COLORS[2] & 0x00FFFFFF) | alpha;

        // Define plane corners in world space
        let p1 = gizmo_center
            + model_matrix.transform_vector3(Vec3::new(plane_size * 0.3, plane_size * 0.3, 0.0));
        let p2 = gizmo_center
            + model_matrix.transform_vector3(Vec3::new(plane_size, plane_size * 0.3, 0.0));
        let p3 =
            gizmo_center + model_matrix.transform_vector3(Vec3::new(plane_size, plane_size, 0.0));
        let p4 = gizmo_center
            + model_matrix.transform_vector3(Vec3::new(plane_size * 0.3, plane_size, 0.0));

        // Project to screen space
        let s1 = project_to_screen(mvp, p1, viewport)?;
        let s2 = project_to_screen(mvp, p2, viewport)?;
        let s3 = project_to_screen(mvp, p3, viewport)?;
        let s4 = project_to_screen(mvp, p4, viewport)?;

        // Draw filled quad using two triangles
        draw_list
            .add_triangle([s1.x, s1.y], [s2.x, s2.y], [s3.x, s3.y], color)
            .filled(true)
            .build();
        draw_list
            .add_triangle([s1.x, s1.y], [s3.x, s3.y], [s4.x, s4.y], color)
            .filled(true)
            .build();
    }

    // XZ plane (green handle)
    if manipulation_type == ManipulationType::MoveZX {
        let color = (AXIS_COLORS[1] & 0x00FFFFFF) | alpha;

        // Define plane corners in world space
        let p1 = gizmo_center
            + model_matrix.transform_vector3(Vec3::new(plane_size * 0.3, 0.0, plane_size * 0.3));
        let p2 = gizmo_center
            + model_matrix.transform_vector3(Vec3::new(plane_size, 0.0, plane_size * 0.3));
        let p3 =
            gizmo_center + model_matrix.transform_vector3(Vec3::new(plane_size, 0.0, plane_size));
        let p4 = gizmo_center
            + model_matrix.transform_vector3(Vec3::new(plane_size * 0.3, 0.0, plane_size));

        // Project to screen space
        let s1 = project_to_screen(mvp, p1, viewport)?;
        let s2 = project_to_screen(mvp, p2, viewport)?;
        let s3 = project_to_screen(mvp, p3, viewport)?;
        let s4 = project_to_screen(mvp, p4, viewport)?;

        // Draw filled quad using two triangles
        draw_list
            .add_triangle([s1.x, s1.y], [s2.x, s2.y], [s3.x, s3.y], color)
            .filled(true)
            .build();
        draw_list
            .add_triangle([s1.x, s1.y], [s3.x, s3.y], [s4.x, s4.y], color)
            .filled(true)
            .build();
    }

    // YZ plane (red handle)
    if manipulation_type == ManipulationType::MoveYZ {
        let color = (AXIS_COLORS[0] & 0x00FFFFFF) | alpha;

        // Define plane corners in world space
        let p1 = gizmo_center
            + model_matrix.transform_vector3(Vec3::new(0.0, plane_size * 0.3, plane_size * 0.3));
        let p2 = gizmo_center
            + model_matrix.transform_vector3(Vec3::new(0.0, plane_size, plane_size * 0.3));
        let p3 =
            gizmo_center + model_matrix.transform_vector3(Vec3::new(0.0, plane_size, plane_size));
        let p4 = gizmo_center
            + model_matrix.transform_vector3(Vec3::new(0.0, plane_size * 0.3, plane_size));

        // Project to screen space
        let s1 = project_to_screen(mvp, p1, viewport)?;
        let s2 = project_to_screen(mvp, p2, viewport)?;
        let s3 = project_to_screen(mvp, p3, viewport)?;
        let s4 = project_to_screen(mvp, p4, viewport)?;

        // Draw filled quad using two triangles
        draw_list
            .add_triangle([s1.x, s1.y], [s2.x, s2.y], [s3.x, s3.y], color)
            .filled(true)
            .build();
        draw_list
            .add_triangle([s1.x, s1.y], [s3.x, s3.y], [s4.x, s4.y], color)
            .filled(true)
            .build();
    }

    Ok(())
}

/// Draw rotation circles with proper 3D projection
fn draw_rotation_circles(
    draw_list: &DrawListMut,
    mvp: &Mat4,
    model_matrix: &Mat4,
    viewport: &[f32; 4],
    gizmo_size: f32,
    manipulation_type: ManipulationType,
) -> GuizmoResult<()> {
    let radius = gizmo_size * 0.9;
    let thickness = 4.0;
    let segments = 64;

    // Get gizmo center in world space
    let gizmo_center = model_matrix.transform_point3(Vec3::ZERO);

    // X axis rotation (red circle) - YZ plane
    if manipulation_type == ManipulationType::None || manipulation_type.is_x_axis() {
        let color = if manipulation_type == ManipulationType::RotateX {
            AXIS_COLORS_HIGHLIGHTED[0]
        } else {
            AXIS_COLORS[0]
        };

        draw_3d_circle(
            draw_list,
            mvp,
            gizmo_center,
            radius,
            Vec3::X,
            color,
            thickness,
            segments,
            viewport,
        )?;
    }

    // Y axis rotation (green circle) - XZ plane
    if manipulation_type == ManipulationType::None || manipulation_type.is_y_axis() {
        let color = if manipulation_type == ManipulationType::RotateY {
            AXIS_COLORS_HIGHLIGHTED[1]
        } else {
            AXIS_COLORS[1]
        };

        draw_3d_circle(
            draw_list,
            mvp,
            gizmo_center,
            radius,
            Vec3::Y,
            color,
            thickness,
            segments,
            viewport,
        )?;
    }

    // Z axis rotation (blue circle) - XY plane
    if manipulation_type == ManipulationType::None || manipulation_type.is_z_axis() {
        let color = if manipulation_type == ManipulationType::RotateZ {
            AXIS_COLORS_HIGHLIGHTED[2]
        } else {
            AXIS_COLORS[2]
        };

        draw_3d_circle(
            draw_list,
            mvp,
            gizmo_center,
            radius,
            Vec3::Z,
            color,
            thickness,
            segments,
            viewport,
        )?;
    }

    Ok(())
}

/// Draw a 3D circle around a specific axis
fn draw_3d_circle(
    draw_list: &DrawListMut,
    mvp: &Mat4,
    center: Vec3,
    radius: f32,
    axis: Vec3,
    color: u32,
    thickness: f32,
    segments: u32,
    viewport: &[f32; 4],
) -> GuizmoResult<()> {
    // Create two perpendicular vectors to the axis
    let up = if axis.dot(Vec3::Y).abs() < 0.9 {
        Vec3::Y
    } else {
        Vec3::X
    };
    let right = axis.cross(up).normalize();
    let forward = axis.cross(right).normalize();

    let mut prev_screen: Option<Vec2> = None;

    for i in 0..=segments {
        let angle = (i as f32 / segments as f32) * 2.0 * std::f32::consts::PI;
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        // Calculate point on circle in 3D space
        let circle_point = center + (right * cos_a + forward * sin_a) * radius;

        // Project to screen space
        let screen_point = project_to_screen(mvp, circle_point, viewport)?;

        // Draw line segment
        if let Some(prev) = prev_screen {
            draw_list
                .add_line([prev.x, prev.y], [screen_point.x, screen_point.y], color)
                .thickness(thickness)
                .build();
        }

        prev_screen = Some(screen_point);
    }

    Ok(())
}

/// Draw scale handles with proper 3D projection
fn draw_scale_handles(
    draw_list: &DrawListMut,
    mvp: &Mat4,
    model_matrix: &Mat4,
    viewport: &[f32; 4],
    gizmo_size: f32,
    manipulation_type: ManipulationType,
) -> GuizmoResult<()> {
    let axis_length = gizmo_size;
    let handle_size = gizmo_size * 0.1;

    // Get gizmo center in world space
    let gizmo_center = model_matrix.transform_point3(Vec3::ZERO);

    // Define axis directions in world space
    let x_axis = model_matrix.transform_vector3(Vec3::X);
    let y_axis = model_matrix.transform_vector3(Vec3::Y);
    let z_axis = model_matrix.transform_vector3(Vec3::Z);

    // X axis (red)
    if manipulation_type == ManipulationType::None || manipulation_type.is_x_axis() {
        let color = if manipulation_type == ManipulationType::ScaleX {
            AXIS_COLORS_HIGHLIGHTED[0]
        } else {
            AXIS_COLORS[0]
        };

        let axis_end = gizmo_center + x_axis * axis_length;
        let center_screen = project_to_screen(mvp, gizmo_center, viewport)?;
        let end_screen = project_to_screen(mvp, axis_end, viewport)?;

        // Draw axis line
        draw_list
            .add_line(
                [center_screen.x, center_screen.y],
                [end_screen.x, end_screen.y],
                color,
            )
            .thickness(4.0)
            .build();

        // Draw scale handle (cube)
        draw_3d_cube(draw_list, mvp, axis_end, handle_size, color, viewport)?;
    }

    // Y axis (green)
    if manipulation_type == ManipulationType::None || manipulation_type.is_y_axis() {
        let color = if manipulation_type == ManipulationType::ScaleY {
            AXIS_COLORS_HIGHLIGHTED[1]
        } else {
            AXIS_COLORS[1]
        };

        let axis_end = gizmo_center + y_axis * axis_length;
        let center_screen = project_to_screen(mvp, gizmo_center, viewport)?;
        let end_screen = project_to_screen(mvp, axis_end, viewport)?;

        // Draw axis line
        draw_list
            .add_line(
                [center_screen.x, center_screen.y],
                [end_screen.x, end_screen.y],
                color,
            )
            .thickness(4.0)
            .build();

        // Draw scale handle (cube)
        draw_3d_cube(draw_list, mvp, axis_end, handle_size, color, viewport)?;
    }

    // Z axis (blue)
    if manipulation_type == ManipulationType::None || manipulation_type.is_z_axis() {
        let color = if manipulation_type == ManipulationType::ScaleZ {
            AXIS_COLORS_HIGHLIGHTED[2]
        } else {
            AXIS_COLORS[2]
        };

        let axis_end = gizmo_center + z_axis * axis_length;
        let center_screen = project_to_screen(mvp, gizmo_center, viewport)?;
        let end_screen = project_to_screen(mvp, axis_end, viewport)?;

        // Draw axis line
        draw_list
            .add_line(
                [center_screen.x, center_screen.y],
                [end_screen.x, end_screen.y],
                color,
            )
            .thickness(4.0)
            .build();

        // Draw scale handle (cube)
        draw_3d_cube(draw_list, mvp, axis_end, handle_size, color, viewport)?;
    }

    // Uniform scale handle (center cube)
    if manipulation_type == ManipulationType::None
        || manipulation_type == ManipulationType::ScaleXYZ
    {
        let color = if manipulation_type == ManipulationType::ScaleXYZ {
            AXIS_COLORS_HIGHLIGHTED[3]
        } else {
            AXIS_COLORS[3]
        };

        draw_3d_cube(
            draw_list,
            mvp,
            gizmo_center,
            handle_size * 0.7,
            color,
            viewport,
        )?;
    }

    Ok(())
}

/// Draw a 3D cube at the specified position
fn draw_3d_cube(
    draw_list: &DrawListMut,
    mvp: &Mat4,
    center: Vec3,
    size: f32,
    color: u32,
    viewport: &[f32; 4],
) -> GuizmoResult<()> {
    let half_size = size * 0.5;

    // Define cube vertices
    let vertices = [
        center + Vec3::new(-half_size, -half_size, -half_size), // 0
        center + Vec3::new(half_size, -half_size, -half_size),  // 1
        center + Vec3::new(half_size, half_size, -half_size),   // 2
        center + Vec3::new(-half_size, half_size, -half_size),  // 3
        center + Vec3::new(-half_size, -half_size, half_size),  // 4
        center + Vec3::new(half_size, -half_size, half_size),   // 5
        center + Vec3::new(half_size, half_size, half_size),    // 6
        center + Vec3::new(-half_size, half_size, half_size),   // 7
    ];

    // Project vertices to screen space
    let mut screen_verts = Vec::new();
    for vertex in &vertices {
        screen_verts.push(project_to_screen(mvp, *vertex, viewport)?);
    }

    // Define cube edges
    let edges = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0), // Back face
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4), // Front face
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7), // Connecting edges
    ];

    // Draw edges
    for (start, end) in &edges {
        let start_pos = screen_verts[*start];
        let end_pos = screen_verts[*end];

        draw_list
            .add_line([start_pos.x, start_pos.y], [end_pos.x, end_pos.y], color)
            .thickness(2.0)
            .build();
    }

    Ok(())
}

/// Draw an arrow head
fn draw_arrow_head(
    draw_list: &DrawListMut,
    position: Vec2,
    direction: Vec2,
    size: f32,
    color: u32,
) -> GuizmoResult<()> {
    let perpendicular = Vec2::new(-direction.y, direction.x);
    let tip = position;
    let base1 = tip - direction * size + perpendicular * size * 0.5;
    let base2 = tip - direction * size - perpendicular * size * 0.5;

    // Draw triangle
    draw_list
        .add_triangle(
            [tip.x, tip.y],
            [base1.x, base1.y],
            [base2.x, base2.y],
            color,
        )
        .filled(true)
        .build();

    Ok(())
}

/// Project a 3D point to screen space
pub fn project_to_screen(
    model_view_proj: &Mat4,
    point: Vec3,
    viewport: &[f32; 4],
) -> GuizmoResult<Vec2> {
    let clip_space = *model_view_proj * Vec4::new(point.x, point.y, point.z, 1.0);

    // Debug: Log projection details
    #[cfg(feature = "tracing")]
    tracing::debug!(
        "Projecting point: {:?}, clip_space: {:?}, w: {}",
        point,
        clip_space,
        clip_space.w
    );

    // Temporarily disable this check to see if we can get some rendering
    if clip_space.w <= -1.0 {
        // Only reject points very far behind camera
        return Err(GuizmoError::InvalidMatrix {
            reason: "Point behind camera or division by zero".to_string(),
        });
    }

    let ndc = clip_space.truncate() / clip_space.w;

    let screen_x = (ndc.x + 1.0) * 0.5 * viewport[2] + viewport[0];
    let screen_y = (1.0 - ndc.y) * 0.5 * viewport[3] + viewport[1];

    let result = Vec2::new(screen_x, screen_y);

    #[cfg(feature = "tracing")]
    tracing::debug!("Screen position: {:?}", result);

    Ok(result)
}

/// Calculate gizmo size based on distance from camera
pub fn calculate_gizmo_size(view: &Mat4, position: Vec3, size_clip_space: f32) -> f32 {
    let view_pos = view.transform_point3(position);
    let distance = view_pos.length();

    // Scale gizmo size based on distance to maintain consistent screen size
    let base_size = 100.0; // Base size in pixels
    let scale_factor = distance * size_clip_space;

    base_size * scale_factor.max(0.1) // Minimum size to prevent disappearing
}

/// Draw a grid in 3D space
pub fn draw_grid(
    _view: &Mat4,
    _projection: &Mat4,
    _matrix: &Mat4,
    _grid_size: f32,
) -> GuizmoResult<()> {
    // TODO: Implement grid drawing
    Ok(())
}

/// Draw debug cubes
pub fn draw_cubes(_view: &Mat4, _projection: &Mat4, _matrices: &[Mat4]) -> GuizmoResult<()> {
    // TODO: Implement cube drawing
    Ok(())
}

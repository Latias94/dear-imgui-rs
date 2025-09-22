//! Drawing system for ImGuizmo
//!
//! This module handles all the rendering operations for gizmo elements.

use crate::context::GuizmoContext;
use crate::gizmo::ManipulationType;
use crate::types::{ColorExt, Mat4, Operation, Rect, Vec2, Vec3, Vec4};
use crate::{GuizmoError, GuizmoResult};
use dear_imgui::DrawListMut;

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
    context: &GuizmoContext,
    operation: Operation,
    manipulation_type: ManipulationType,
) -> GuizmoResult<()> {
    if !operation.intersects(Operation::TRANSLATE) {
        return Ok(());
    }

    let state = context.state.borrow();
    let model_view_proj = state.mvp;
    let model = state.model_matrix;
    let viewport = state.viewport.as_viewport();

    // Compute gizmo size for consistent on-screen scale
    let gizmo_center = model.transform_point3(Vec3::ZERO);
    let gizmo_size = calculate_gizmo_size(
        &state.view_matrix,
        gizmo_center,
        state.gizmo_size_clip_space,
    );

    // Draw 3D axes和平面（带可见性/哈线/翻转）
    let axis_flags = state.below_axis_limit;
    let plane_flags = state.below_plane_limit;
    let allow_axis_flip = state.allow_axis_flip;
    let camera_dir = state.camera_dir;
    let hatched_thickness = state.style.hatched_axis_line_thickness;
    let hatched_color =
        state.style.colors[crate::types::ColorElement::HatchedAxisLines as usize].as_u32();

    draw_translation_axes(
        draw_list,
        &model_view_proj,
        &model,
        &viewport,
        gizmo_size,
        manipulation_type,
        axis_flags,
        allow_axis_flip,
        camera_dir,
        state.style.translation_line_thickness,
        state.style.translation_line_arrow_size,
        hatched_thickness,
        hatched_color,
        [
            state.style.colors[crate::types::ColorElement::DirectionX as usize].as_u32(),
            state.style.colors[crate::types::ColorElement::DirectionY as usize].as_u32(),
            state.style.colors[crate::types::ColorElement::DirectionZ as usize].as_u32(),
        ],
    )?;

    draw_translation_planes(
        draw_list,
        &model_view_proj,
        &model,
        &viewport,
        gizmo_size,
        manipulation_type,
        plane_flags,
        [
            state.style.colors[crate::types::ColorElement::PlaneZ as usize].as_u32(), // XY plane
            state.style.colors[crate::types::ColorElement::PlaneY as usize].as_u32(), // XZ plane
            state.style.colors[crate::types::ColorElement::PlaneX as usize].as_u32(), // YZ plane
        ],
    )?;

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
    let screen_center = project_to_screen(
        &model_view_proj,
        gizmo_center,
        &state.viewport.as_viewport(),
    )?;

    // Calculate gizmo size based on distance
    let gizmo_size = calculate_gizmo_size(
        &state.view_matrix,
        gizmo_center,
        state.gizmo_size_clip_space,
    );

    // Always draw rotation circles; highlight is driven by manipulation_type
    draw_rotation_circles(
        draw_list,
        &model_view_proj,
        &model,
        &state.viewport.as_viewport(),
        gizmo_size,
        _manipulation_type,
        state.style.rotation_line_thickness,
    )?;

    Ok(())
}

/// Draw scale gizmo
pub fn draw_scale_gizmo(
    draw_list: &DrawListMut,
    context: &GuizmoContext,
    operation: Operation,
    manipulation_type: ManipulationType,
) -> GuizmoResult<()> {
    if !operation.intersects(Operation::SCALE | Operation::SCALE_UNIFORM) {
        return Ok(());
    }

    let state = context.state.borrow();

    // Get transformation matrices
    let model_view_proj = state.mvp;
    let model = state.model_matrix;

    // Transform gizmo center to screen space
    let gizmo_center = model.transform_point3(Vec3::ZERO);
    let screen_center = project_to_screen(
        &model_view_proj,
        gizmo_center,
        &state.viewport.as_viewport(),
    )?;

    // Calculate gizmo size based on distance
    let gizmo_size = calculate_gizmo_size(
        &state.view_matrix,
        gizmo_center,
        state.gizmo_size_clip_space,
    );

    draw_scale_handles(
        draw_list,
        &model_view_proj,
        &model,
        &state.viewport.as_viewport(),
        gizmo_size,
        manipulation_type,
        state.style.scale_line_thickness,
    )?;

    // Draw center circle for uniform scale / universal (highlight when ScaleXYZ)
    if operation.intersects(Operation::SCALE) || operation.intersects(Operation::SCALE_UNIFORM) {
        let radius = state.style.center_circle_size.max(2.0);
        let col = if manipulation_type == ManipulationType::ScaleXYZ {
            state.style.colors[crate::types::ColorElement::Selection as usize].as_u32()
        } else {
            0xFFFFFFFF
        };
        draw_list
            .add_circle([screen_center.x, screen_center.y], radius, col)
            .filled(true)
            .build();
    }

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
    axis_flags: [bool; 3],
    allow_axis_flip: bool,
    camera_dir: Vec3,
    line_thickness: f32,
    arrow_size_px: f32,
    hatched_thickness: f32,
    hatched_color: u32,
    axis_colors: [u32; 3],
) -> GuizmoResult<()> {
    #[cfg(feature = "tracing")]
    tracing::info!(
        "draw_translation_axes called with gizmo_size: {}",
        gizmo_size
    );
    let axis_length = gizmo_size;
    // Arrow size from param (pixels); fallback to proportional if 0
    let arrow_size = if arrow_size_px > 0.0 {
        arrow_size_px
    } else {
        gizmo_size * 0.15
    };

    // Get gizmo center in world space
    let gizmo_center = model_matrix.transform_point3(Vec3::ZERO);

    // Define axis directions in world space
    let mut x_axis = model_matrix.transform_vector3(Vec3::X).normalize();
    let mut y_axis = model_matrix.transform_vector3(Vec3::Y).normalize();
    let mut z_axis = model_matrix.transform_vector3(Vec3::Z).normalize();

    // Draw X axis (style color)
    if manipulation_type == ManipulationType::None || manipulation_type.is_x_axis() {
        let color = axis_colors[0];
        // Note: Allow axis flip disabled for now in this patch step
        if allow_axis_flip && x_axis.dot(camera_dir) < 0.0 {
            x_axis = -x_axis;
        }
        let axis_end = gizmo_center + x_axis * axis_length;
        let center_screen = project_to_screen(mvp, gizmo_center, viewport)?;
        let end_screen = project_to_screen(mvp, axis_end, viewport)?;

        // Draw axis line / hatched when below limit
        if hatched_thickness > 0.0 && axis_flags[0] {
            draw_hatched_line(
                draw_list,
                center_screen,
                end_screen,
                hatched_color,
                hatched_thickness,
            );
        } else {
            let thickness = line_thickness.max(1.0);
            draw_list
                .add_line(
                    [center_screen.x, center_screen.y],
                    [end_screen.x, end_screen.y],
                    color,
                )
                .thickness(thickness)
                .build();
        }

        // Draw arrow head
        let direction = (end_screen - center_screen).normalize();
        draw_arrow_head(draw_list, end_screen, direction, arrow_size, color)?;
    }

    // Draw Y axis (style color)
    if manipulation_type == ManipulationType::None || manipulation_type.is_y_axis() {
        let color = axis_colors[1];
        if allow_axis_flip && y_axis.dot(camera_dir) < 0.0 {
            y_axis = -y_axis;
        }
        let axis_end = gizmo_center + y_axis * axis_length;
        let center_screen = project_to_screen(mvp, gizmo_center, viewport)?;
        let end_screen = project_to_screen(mvp, axis_end, viewport)?;
        if hatched_thickness > 0.0 && axis_flags[1] {
            draw_hatched_line(
                draw_list,
                center_screen,
                end_screen,
                hatched_color,
                hatched_thickness,
            );
        } else {
            let thickness = line_thickness.max(1.0);
            draw_list
                .add_line(
                    [center_screen.x, center_screen.y],
                    [end_screen.x, end_screen.y],
                    color,
                )
                .thickness(thickness)
                .build();
        }

        // Draw arrow head
        let direction = (end_screen - center_screen).normalize();
        draw_arrow_head(draw_list, end_screen, direction, arrow_size, color)?;
    }

    // Draw Z axis (style color)
    if manipulation_type == ManipulationType::None || manipulation_type.is_z_axis() {
        let color = axis_colors[2];
        if allow_axis_flip && z_axis.dot(camera_dir) < 0.0 {
            z_axis = -z_axis;
        }
        let axis_end = gizmo_center + z_axis * axis_length;
        let center_screen = project_to_screen(mvp, gizmo_center, viewport)?;
        let end_screen = project_to_screen(mvp, axis_end, viewport)?;
        if hatched_thickness > 0.0 && axis_flags[2] {
            draw_hatched_line(
                draw_list,
                center_screen,
                end_screen,
                hatched_color,
                hatched_thickness,
            );
        } else {
            let thickness = line_thickness.max(1.0);
            draw_list
                .add_line(
                    [center_screen.x, center_screen.y],
                    [end_screen.x, end_screen.y],
                    color,
                )
                .thickness(thickness)
                .build();
        }

        // Draw arrow head
        let direction = (end_screen - center_screen).normalize();
        draw_arrow_head(draw_list, end_screen, direction, arrow_size, color)?;
    }

    Ok(())
}

/// Draw translation planes near the gizmo center
fn draw_translation_planes(
    draw_list: &DrawListMut,
    mvp: &Mat4,
    model_matrix: &Mat4,
    viewport: &[f32; 4],
    gizmo_size: f32,
    manipulation_type: ManipulationType,
    plane_flags: [bool; 3],
    plane_colors: [u32; 3], // [XY, XZ, YZ]
) -> GuizmoResult<()> {
    let center = model_matrix.transform_point3(Vec3::ZERO);
    let x_axis = model_matrix.transform_vector3(Vec3::X).normalize();
    let y_axis = model_matrix.transform_vector3(Vec3::Y).normalize();
    let z_axis = model_matrix.transform_vector3(Vec3::Z).normalize();

    let plane_size = gizmo_size * 0.35;

    let make_quad = |u: Vec3, v: Vec3| -> GuizmoResult<[Vec2; 4]> {
        let p0 = center + (u + v) * plane_size * 0.2;
        let p1 = p0 + u * plane_size * 0.8;
        let p2 = p0 + (u + v) * plane_size * 0.8;
        let p3 = p0 + v * plane_size * 0.8;
        Ok([
            project_to_screen(mvp, p0, viewport)?,
            project_to_screen(mvp, p1, viewport)?,
            project_to_screen(mvp, p2, viewport)?,
            project_to_screen(mvp, p3, viewport)?,
        ])
    };

    // XY plane (Z normal)
    if manipulation_type == ManipulationType::None || manipulation_type.is_plane_type() {
        let quad = make_quad(x_axis, y_axis)?;
        let mut col = if manipulation_type == ManipulationType::MoveXY {
            0x88FFFF00
        } else {
            plane_colors[0]
        };
        if plane_flags[2] {
            col = (col & 0x00FFFFFF) | (0x33 << 24);
        }
        draw_list
            .add_triangle(
                [quad[0].x, quad[0].y],
                [quad[1].x, quad[1].y],
                [quad[2].x, quad[2].y],
                col,
            )
            .filled(true)
            .build();
        draw_list
            .add_triangle(
                [quad[0].x, quad[0].y],
                [quad[2].x, quad[2].y],
                [quad[3].x, quad[3].y],
                col,
            )
            .filled(true)
            .build();

        // XZ plane (Y normal)
        let quad = make_quad(x_axis, z_axis)?;
        let mut col = if manipulation_type == ManipulationType::MoveZX {
            0x88FF00FF
        } else {
            plane_colors[1]
        };
        if plane_flags[1] {
            col = (col & 0x00FFFFFF) | (0x33 << 24);
        }
        draw_list
            .add_triangle(
                [quad[0].x, quad[0].y],
                [quad[1].x, quad[1].y],
                [quad[2].x, quad[2].y],
                col,
            )
            .filled(true)
            .build();
        draw_list
            .add_triangle(
                [quad[0].x, quad[0].y],
                [quad[2].x, quad[2].y],
                [quad[3].x, quad[3].y],
                col,
            )
            .filled(true)
            .build();

        // YZ plane (X normal)
        let quad = make_quad(y_axis, z_axis)?;
        let mut col = if manipulation_type == ManipulationType::MoveYZ {
            0x8800FFFF
        } else {
            plane_colors[2]
        };
        if plane_flags[0] {
            col = (col & 0x00FFFFFF) | (0x33 << 24);
        }
        draw_list
            .add_triangle(
                [quad[0].x, quad[0].y],
                [quad[1].x, quad[1].y],
                [quad[2].x, quad[2].y],
                col,
            )
            .filled(true)
            .build();
        draw_list
            .add_triangle(
                [quad[0].x, quad[0].y],
                [quad[2].x, quad[2].y],
                [quad[3].x, quad[3].y],
                col,
            )
            .filled(true)
            .build();
    }

    Ok(())
}

// (duplicate variant removed)

/// Draw rotation circles with proper 3D projection
fn draw_rotation_circles(
    draw_list: &DrawListMut,
    mvp: &Mat4,
    model_matrix: &Mat4,
    viewport: &[f32; 4],
    gizmo_size: f32,
    manipulation_type: ManipulationType,
    line_thickness: f32,
) -> GuizmoResult<()> {
    let radius = gizmo_size * 0.9;
    let thickness = line_thickness.max(1.0);
    let segments = 64;

    // Get gizmo center in world space
    let gizmo_center = model_matrix.transform_point3(Vec3::ZERO);

    // X axis rotation (red circle) - YZ plane
    if manipulation_type == ManipulationType::None || manipulation_type.is_x_axis() {
        let color = if manipulation_type == ManipulationType::RotateX
            || manipulation_type == ManipulationType::None
        {
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
        let color = if manipulation_type == ManipulationType::RotateY
            || manipulation_type == ManipulationType::None
        {
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
        let color = if manipulation_type == ManipulationType::RotateZ
            || manipulation_type == ManipulationType::None
        {
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
    line_thickness: f32,
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
        let thickness = crate::context::GuizmoContext::new()
            .get_style()
            .scale_line_thickness;
        draw_list
            .add_line(
                [center_screen.x, center_screen.y],
                [end_screen.x, end_screen.y],
                color,
            )
            .thickness(thickness)
            .build();

        // Draw scale handle (cube)
        let thickness = line_thickness;
        draw_3d_cube(
            draw_list,
            mvp,
            axis_end,
            handle_size,
            color,
            viewport,
            thickness,
        )?;
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
        let thickness = crate::context::GuizmoContext::new()
            .get_style()
            .scale_line_thickness;
        draw_list
            .add_line(
                [center_screen.x, center_screen.y],
                [end_screen.x, end_screen.y],
                color,
            )
            .thickness(thickness)
            .build();

        // Draw scale handle (cube)
        let thickness = line_thickness;
        draw_3d_cube(
            draw_list,
            mvp,
            axis_end,
            handle_size,
            color,
            viewport,
            thickness,
        )?;
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
        let thickness = crate::context::GuizmoContext::new()
            .get_style()
            .scale_line_thickness;
        draw_list
            .add_line(
                [center_screen.x, center_screen.y],
                [end_screen.x, end_screen.y],
                color,
            )
            .thickness(thickness)
            .build();

        // Draw scale handle (cube)
        let thickness = line_thickness;
        draw_3d_cube(
            draw_list,
            mvp,
            axis_end,
            handle_size,
            color,
            viewport,
            thickness,
        )?;
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

        let thickness = line_thickness;
        draw_3d_cube(
            draw_list,
            mvp,
            gizmo_center,
            handle_size * 0.7,
            color,
            viewport,
            thickness,
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
    line_thickness: f32,
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
            .thickness(line_thickness.max(1.0))
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

/// Draw hatched axis line (alternating segments) when axis is nearly parallel to view
fn draw_hatched_line(draw_list: &DrawListMut, start: Vec2, end: Vec2, color: u32, thickness: f32) {
    let dir = end - start;
    let len = dir.length();
    if len <= 0.0 {
        return;
    }
    let step = 8.0; // pixels per segment
    let gap = 6.0; // pixels gap
    let n = (len / (step + gap)).floor() as i32;
    let unit = dir / len;
    let mut cur = start;
    for _ in 0..n {
        let seg_end = cur + unit * step;
        draw_list
            .add_line([cur.x, cur.y], [seg_end.x, seg_end.y], color)
            .thickness(thickness)
            .build();
        cur = seg_end + unit * gap;
    }
}

/// Draw a grid in 3D space
pub fn draw_grid(
    draw_list: &DrawListMut,
    view: &Mat4,
    projection: &Mat4,
    model: &Mat4,
    viewport: &Rect,
    grid_size: f32,
) -> GuizmoResult<()> {
    // Draw a simple XZ grid centered at model origin
    let vp = *projection * *view;
    let mvp = vp * *model;

    let step = 1.0f32;
    let half = grid_size;
    let color_major = 0x33444444u32;
    let color_minor = 0x22111111u32;

    for i in (-half as i32)..=(half as i32) {
        let t = i as f32 * step;
        // lines parallel to X (vary Z)
        let a = Vec3::new(-half, 0.0, t);
        let b = Vec3::new(half, 0.0, t);
        let a2 = project_to_screen(&mvp, a, &viewport.as_viewport())?;
        let b2 = project_to_screen(&mvp, b, &viewport.as_viewport())?;
        let col = if i % 10 == 0 {
            color_major
        } else {
            color_minor
        };
        draw_list.add_line([a2.x, a2.y], [b2.x, b2.y], col).build();

        // lines parallel to Z (vary X)
        let c = Vec3::new(t, 0.0, -half);
        let d = Vec3::new(t, 0.0, half);
        let c2 = project_to_screen(&mvp, c, &viewport.as_viewport())?;
        let d2 = project_to_screen(&mvp, d, &viewport.as_viewport())?;
        let col = if i % 10 == 0 {
            color_major
        } else {
            color_minor
        };
        draw_list.add_line([c2.x, c2.y], [d2.x, d2.y], col).build();
    }
    Ok(())
}

/// Draw debug cubes
pub fn draw_cubes(
    draw_list: &DrawListMut,
    view: &Mat4,
    projection: &Mat4,
    matrices: &[Mat4],
    viewport: &Rect,
) -> GuizmoResult<()> {
    let vp = *projection * *view;
    for m in matrices {
        // Draw an AABB of unit cube transformed by model
        let center = m.transform_point3(Vec3::ZERO);
        let size = 0.5f32;
        draw_3d_cube(
            draw_list,
            &(vp * *m),
            center,
            size,
            0x55FFFFFF,
            &viewport.as_viewport(),
            2.0,
        )?;
    }
    Ok(())
}

/// Draw local bounds (AABB) given as [minx, maxx, miny, maxy, minz, maxz] in local space
pub fn draw_local_bounds(
    draw_list: &DrawListMut,
    mvp: &Mat4,
    viewport: &Rect,
    local_bounds: &[f32; 6],
) -> GuizmoResult<()> {
    let min = Vec3::new(local_bounds[0], local_bounds[2], local_bounds[4]);
    let max = Vec3::new(local_bounds[1], local_bounds[3], local_bounds[5]);
    let corners = [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(max.x, max.y, max.z),
        Vec3::new(min.x, max.y, max.z),
    ];
    let mut s = [[0.0f32; 2]; 8];
    for (i, c) in corners.iter().enumerate() {
        let p = project_to_screen(mvp, *c, &viewport.as_viewport())?;
        s[i] = [p.x, p.y];
    }
    let edges = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0), // bottom
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4), // top
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7), // verticals
    ];
    for (a, b) in edges.iter() {
        draw_list.add_line(s[*a], s[*b], 0xFFFFFFFF).build();
    }
    Ok(())
}

/// Draw small corner handles for bounds editing, return screen positions for hit-testing
pub fn draw_bounds_handles(
    draw_list: &DrawListMut,
    mvp: &Mat4,
    viewport: &Rect,
    local_bounds: &[f32; 6],
) -> GuizmoResult<[[f32; 2]; 8]> {
    let min = Vec3::new(local_bounds[0], local_bounds[2], local_bounds[4]);
    let max = Vec3::new(local_bounds[1], local_bounds[3], local_bounds[5]);
    let corners = [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(max.x, max.y, max.z),
        Vec3::new(min.x, max.y, max.z),
    ];
    let mut pts = [[0.0f32; 2]; 8];
    for (i, c) in corners.iter().enumerate() {
        let p = project_to_screen(mvp, *c, &viewport.as_viewport())?;
        pts[i] = [p.x, p.y];
        draw_list
            .add_circle([p.x, p.y], 4.0, 0xFFFFFFFF)
            .filled(true)
            .build();
        draw_list.add_circle([p.x, p.y], 4.0, 0xFF000000).build();
    }
    Ok(pts)
}

/// Draw face handles (centers of faces) and return their screen positions in order:
/// [minX,maxX,minY,maxY,minZ,maxZ]
pub fn draw_bounds_face_handles(
    draw_list: &DrawListMut,
    mvp: &Mat4,
    viewport: &Rect,
    local_bounds: &[f32; 6],
) -> GuizmoResult<[[f32; 2]; 6]> {
    let min = Vec3::new(local_bounds[0], local_bounds[2], local_bounds[4]);
    let max = Vec3::new(local_bounds[1], local_bounds[3], local_bounds[5]);
    let centers = [
        Vec3::new(min.x, (min.y + max.y) * 0.5, (min.z + max.z) * 0.5), // minX
        Vec3::new(max.x, (min.y + max.y) * 0.5, (min.z + max.z) * 0.5), // maxX
        Vec3::new((min.x + max.x) * 0.5, min.y, (min.z + max.z) * 0.5), // minY
        Vec3::new((min.x + max.x) * 0.5, max.y, (min.z + max.z) * 0.5), // maxY
        Vec3::new((min.x + max.x) * 0.5, (min.y + max.y) * 0.5, min.z), // minZ
        Vec3::new((min.x + max.x) * 0.5, (min.y + max.y) * 0.5, max.z), // maxZ
    ];
    let mut pts = [[0.0f32; 2]; 6];
    for (i, c) in centers.iter().enumerate() {
        let p = project_to_screen(mvp, *c, &viewport.as_viewport())?;
        pts[i] = [p.x, p.y];
        // Draw small squares for faces
        let s = 5.0;
        draw_list
            .add_rect([p.x - s, p.y - s], [p.x + s, p.y + s], 0xFFFFFFFF)
            .filled(true)
            .build();
        draw_list
            .add_rect([p.x - s, p.y - s], [p.x + s, p.y + s], 0xFF000000)
            .build();
    }
    Ok(pts)
}

/// Draw edge handles (centers of edges) and return their screen positions.
/// Order:
///  - X-parallel: (y=min,z=min), (y=min,z=max), (y=max,z=min), (y=max,z=max) => indices 0..3
///  - Y-parallel: (x=min,z=min), (x=min,z=max), (x=max,z=min), (x=max,z=max) => indices 4..7
///  - Z-parallel: (x=min,y=min), (x=min,y=max), (x=max,y=min), (x=max,y=max) => indices 8..11
pub fn draw_bounds_edge_handles(
    draw_list: &DrawListMut,
    mvp: &Mat4,
    viewport: &Rect,
    local_bounds: &[f32; 6],
) -> GuizmoResult<[[f32; 2]; 12]> {
    let min = Vec3::new(local_bounds[0], local_bounds[2], local_bounds[4]);
    let max = Vec3::new(local_bounds[1], local_bounds[3], local_bounds[5]);
    let mut centers = [Vec3::ZERO; 12];
    // X-parallel (vary X, fixed YZ corners)
    centers[0] = Vec3::new((min.x + max.x) * 0.5, min.y, min.z);
    centers[1] = Vec3::new((min.x + max.x) * 0.5, min.y, max.z);
    centers[2] = Vec3::new((min.x + max.x) * 0.5, max.y, min.z);
    centers[3] = Vec3::new((min.x + max.x) * 0.5, max.y, max.z);
    // Y-parallel
    centers[4] = Vec3::new(min.x, (min.y + max.y) * 0.5, min.z);
    centers[5] = Vec3::new(min.x, (min.y + max.y) * 0.5, max.z);
    centers[6] = Vec3::new(max.x, (min.y + max.y) * 0.5, min.z);
    centers[7] = Vec3::new(max.x, (min.y + max.y) * 0.5, max.z);
    // Z-parallel
    centers[8] = Vec3::new(min.x, min.y, (min.z + max.z) * 0.5);
    centers[9] = Vec3::new(min.x, max.y, (min.z + max.z) * 0.5);
    centers[10] = Vec3::new(max.x, min.y, (min.z + max.z) * 0.5);
    centers[11] = Vec3::new(max.x, max.y, (min.z + max.z) * 0.5);

    let mut pts = [[0.0f32; 2]; 12];
    for (i, c) in centers.iter().enumerate() {
        let p = project_to_screen(mvp, *c, &viewport.as_viewport())?;
        pts[i] = [p.x, p.y];
        // Draw diamond (rotated square)
        let s = 4.0;
        let a = [p.x, p.y - s];
        let b = [p.x + s, p.y];
        let c2 = [p.x, p.y + s];
        let d = [p.x - s, p.y];
        draw_list
            .add_triangle(a, b, c2, 0xFFFFFFFF)
            .filled(true)
            .build();
        draw_list
            .add_triangle(a, c2, d, 0xFFFFFFFF)
            .filled(true)
            .build();
        draw_list.add_line(a, b, 0xFF000000).build();
        draw_list.add_line(b, c2, 0xFF000000).build();
        draw_list.add_line(c2, d, 0xFF000000).build();
        draw_list.add_line(d, a, 0xFF000000).build();
    }
    Ok(pts)
}

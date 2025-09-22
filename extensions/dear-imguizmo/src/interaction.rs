//! Interaction system for ImGuizmo
//!
//! This module handles mouse and keyboard interactions with gizmo elements.

use crate::context::GuizmoState;
use crate::gizmo::ManipulationType;
use crate::types::{Mat4, Rect, Vec2, Vec3};
use crate::GuizmoResult;
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
    /// Screen-space distance from mouse to hit target (lower is better)
    pub screen_distance: f32,
    /// Hit proportion along the axis/plane (0.0 to 1.0)
    pub proportion: Vec3,
}

impl Default for HitTestResult {
    fn default() -> Self {
        Self {
            hit: false,
            manipulation_type: ManipulationType::None,
            distance: f32::INFINITY,
            screen_distance: f32::INFINITY,
            proportion: Vec3::ZERO,
        }
    }
}

/// Check if mouse is over a gizmo element
pub fn is_over_gizmo(
    _ui: &Ui,
    state: &GuizmoState,
    mouse_pos: Vec2,
) -> GuizmoResult<HitTestResult> {
    // Collect candidates
    let mut candidates: Vec<HitTestResult> = Vec::new();
    let enable_translate = state
        .operation
        .intersects(crate::types::Operation::TRANSLATE);
    let enable_rotate = state.operation.intersects(crate::types::Operation::ROTATE);
    let enable_scale = state.operation.intersects(crate::types::Operation::SCALE);
    let enable_scale_uniform = state
        .operation
        .intersects(crate::types::Operation::SCALE_UNIFORM);

    if enable_translate {
        if let Some(r) = test_translation_axes(state, mouse_pos)? {
            if r.hit {
                candidates.push(r);
            }
        }
        if let Some(r) = test_translation_planes(state, mouse_pos)? {
            if r.hit {
                candidates.push(r);
            }
        }
    }
    if enable_rotate {
        if let Some(r) = test_rotation_circles(state, mouse_pos)? {
            if r.hit {
                candidates.push(r);
            }
        }
    }
    // scale handles handles both uniform and per-axis; the test internally gates per-axis
    if enable_scale || enable_scale_uniform {
        if let Some(r) = test_scale_handles(state, mouse_pos)? {
            if r.hit {
                candidates.push(r);
            }
        }
    }

    // Choose best by (priority, screen_distance)
    let mut best = HitTestResult::default();
    let mut best_key = (u32::MAX, f32::INFINITY);
    for r in candidates.iter() {
        let pri = compute_priority(state, r.manipulation_type);
        let key = (pri, r.screen_distance);
        if key < best_key {
            best_key = key;
            best = r.clone();
        }
    }

    // Universal 细化：当同时命中中心统一缩放与旋转环时，根据距离偏好旋转或中心
    if state
        .operation
        .intersects(crate::types::Operation::SCALE_UNIFORM)
        && state.operation.intersects(crate::types::Operation::ROTATE)
    {
        let center = candidates
            .iter()
            .filter(|c| c.manipulation_type == ManipulationType::ScaleXYZ)
            .min_by(|a, b| a.screen_distance.total_cmp(&b.screen_distance));
        let ring = candidates
            .iter()
            .filter(|c| {
                matches!(
                    c.manipulation_type,
                    ManipulationType::RotateX
                        | ManipulationType::RotateY
                        | ManipulationType::RotateZ
                )
            })
            .min_by(|a, b| a.screen_distance.total_cmp(&b.screen_distance));
        if let (Some(c), Some(r)) = (center, ring) {
            // 如果靠近环明显多于中心（80% 比例），优先旋转；否则中心
            if r.screen_distance < c.screen_distance * 0.8 {
                best = r.clone();
            } else {
                best = c.clone();
            }
        }
    }

    Ok(best)
}

fn compute_priority(state: &GuizmoState, ty: ManipulationType) -> u32 {
    use ManipulationType::*;
    match ty {
        // In Universal 模式下，让中心统一缩放与旋转同优先级，后续用屏幕距离细化
        ScaleXYZ => {
            if state
                .operation
                .intersects(crate::types::Operation::SCALE_UNIFORM)
            {
                1
            } else {
                6
            }
        }
        RotateX | RotateY | RotateZ => 1,
        MoveX | MoveY | MoveZ => 2,
        ScaleX | ScaleY | ScaleZ => 3,
        MoveXY | MoveYZ | MoveZX => 4,
        _ => 6,
    }
}

/// Handle mouse interaction with gizmo
pub fn handle_mouse_interaction(
    ui: &Ui,
    state: &mut GuizmoState,
    mouse_pos: Vec2,
    mouse_down: bool,
    mouse_clicked: bool,
    mouse_released: bool,
    snap: Option<&[f32; 3]>,
) -> GuizmoResult<bool> {
    let mut modified = false;

    // Check for hover
    let hit_result = is_over_gizmo(ui, state, mouse_pos)?;
    state.is_over = hit_result.hit;

    // Bounds selection check (corner > edge > face)
    if mouse_clicked && state.local_bounds.is_some() {
        if let Some(idx) = is_over_bounds_corners(state, mouse_pos)? {
            state.using_bounds = true;
            state.selected_bounds_corner = idx as i32;
            state.selected_bounds_face = -1;
            state.selected_bounds_edge = -1;
            state.mouse_down_pos = mouse_pos;
            state.model_source = state.model_matrix;
            setup_bounds_drag_start(state, Some(idx as i32), None, None);
            return Ok(true);
        } else if let Some(edge) = is_over_bounds_edges(state, mouse_pos)? {
            state.using_bounds = true;
            state.selected_bounds_edge = edge as i32;
            state.selected_bounds_corner = -1;
            state.selected_bounds_face = -1;
            state.mouse_down_pos = mouse_pos;
            state.model_source = state.model_matrix;
            setup_bounds_drag_start(state, None, Some(edge as i32), None);
            return Ok(true);
        } else if let Some(face) = is_over_bounds_faces(state, mouse_pos)? {
            state.using_bounds = true;
            state.selected_bounds_face = face as i32;
            state.selected_bounds_corner = -1;
            state.selected_bounds_edge = -1;
            state.mouse_down_pos = mouse_pos;
            state.model_source = state.model_matrix;
            setup_bounds_drag_start(state, None, None, Some(face as i32));
            return Ok(true);
        }
    }

    if mouse_clicked && hit_result.hit {
        // Start manipulation
        state.using = hit_result.manipulation_type;
        state.mouse_down_pos = mouse_pos;
        state.model_source = state.model_matrix;
        state.model_source_inverse = state.model_inverse;

        // Store initial values for different manipulation types
        match hit_result.manipulation_type {
            // Prepare translation plane/origin
            ManipulationType::MoveX
            | ManipulationType::MoveY
            | ManipulationType::MoveZ
            | ManipulationType::MoveXY
            | ManipulationType::MoveYZ
            | ManipulationType::MoveZX => {
                setup_translation_start(state)?;
            }
            ManipulationType::RotateX | ManipulationType::RotateY | ManipulationType::RotateZ => {
                // Compute start angle on the selected axis via ray-plane intersection
                let axis = match hit_result.manipulation_type {
                    ManipulationType::RotateX => get_rotation_axis_world(state, 0),
                    ManipulationType::RotateY => get_rotation_axis_world(state, 1),
                    _ => get_rotation_axis_world(state, 2),
                };
                state.rotation_start_angle = rotation_angle_on_axis(state, mouse_pos, axis)?;
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
    } else if mouse_released && (state.using != ManipulationType::None || state.using_bounds) {
        // End manipulation
        state.using = ManipulationType::None;
        state.using_bounds = false;
        state.selected_bounds_corner = -1;
        state.selected_bounds_edge = -1;
        state.selected_bounds_face = -1;
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
                modified = handle_translation_interaction(state, mouse_pos, delta, snap)?;
            }
            ManipulationType::RotateX | ManipulationType::RotateY | ManipulationType::RotateZ => {
                modified = handle_rotation_interaction(state, mouse_pos, snap)?;
            }
            ManipulationType::ScaleX
            | ManipulationType::ScaleY
            | ManipulationType::ScaleZ
            | ManipulationType::ScaleXYZ => {
                modified = handle_scale_interaction(state, mouse_pos, delta, snap)?;
            }
            _ => {}
        }
    } else if mouse_down && state.using_bounds {
        // Continue bounds manipulation
        modified = handle_bounds_interaction(state, mouse_pos, snap)?;
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
        &state.view_matrix,
        gizmo_center,
        state.gizmo_size_clip_space,
    );

    let axis_length = gizmo_size * 0.8;
    let hit_threshold = 10.0; // pixels

    let mut best_result = HitTestResult::default();

    // Test X axis (red) using projected endpoint
    let x_world = state
        .model_matrix
        .transform_point3(Vec3::X * axis_length + Vec3::ZERO);
    let x_end = crate::draw::project_to_screen(&state.mvp, x_world, &state.viewport.as_viewport())?;
    let x_distance = distance_point_to_line_segment(mouse_pos, screen_center, x_end);
    if x_distance < hit_threshold {
        let distance = (gizmo_center - state.camera_eye).length();
        if distance < best_result.distance {
            best_result = HitTestResult {
                hit: true,
                manipulation_type: ManipulationType::MoveX,
                distance,
                screen_distance: x_distance,
                proportion: Vec3::new(1.0, 0.0, 0.0),
            };
        }
    }

    // Test Y axis (green) using projected endpoint
    let y_world = state
        .model_matrix
        .transform_point3(Vec3::Y * axis_length + Vec3::ZERO);
    let y_end = crate::draw::project_to_screen(&state.mvp, y_world, &state.viewport.as_viewport())?;
    let y_distance = distance_point_to_line_segment(mouse_pos, screen_center, y_end);
    if y_distance < hit_threshold {
        let distance = (gizmo_center - state.camera_eye).length();
        if distance < best_result.distance {
            best_result = HitTestResult {
                hit: true,
                manipulation_type: ManipulationType::MoveY,
                distance,
                screen_distance: y_distance,
                proportion: Vec3::new(0.0, 1.0, 0.0),
            };
        }
    }

    // Test Z axis (blue) using projected endpoint
    let z_world = state
        .model_matrix
        .transform_point3(Vec3::Z * axis_length + Vec3::ZERO);
    let z_end = crate::draw::project_to_screen(&state.mvp, z_world, &state.viewport.as_viewport())?;
    let z_distance = distance_point_to_line_segment(mouse_pos, screen_center, z_end);
    if z_distance < hit_threshold {
        let distance = (gizmo_center - state.camera_eye).length();
        if distance < best_result.distance {
            best_result = HitTestResult {
                hit: true,
                manipulation_type: ManipulationType::MoveZ,
                distance,
                screen_distance: z_distance,
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
    // Reuse the same quad geometry as draw_translation_planes for accuracy
    let center = state.model_matrix.transform_point3(Vec3::ZERO);
    let x_axis = state.model_matrix.transform_vector3(Vec3::X).normalize();
    let y_axis = state.model_matrix.transform_vector3(Vec3::Y).normalize();
    let z_axis = state.model_matrix.transform_vector3(Vec3::Z).normalize();

    let gizmo_size =
        crate::draw::calculate_gizmo_size(&state.view_matrix, center, state.gizmo_size_clip_space);
    let plane_size = gizmo_size * 0.35;

    let project = |p: Vec3| -> GuizmoResult<Vec2> {
        crate::draw::project_to_screen(&state.mvp, p, &state.viewport.as_viewport())
    };

    let make_quad = |u: Vec3, v: Vec3| -> GuizmoResult<[Vec2; 4]> {
        let p0 = center + (u + v) * plane_size * 0.2;
        let p1 = p0 + u * plane_size * 0.8;
        let p2 = p0 + (u + v) * plane_size * 0.8;
        let p3 = p0 + v * plane_size * 0.8;
        Ok([project(p0)?, project(p1)?, project(p2)?, project(p3)?])
    };

    fn cross2(a: Vec2, b: Vec2) -> f32 {
        a.x * b.y - a.y * b.x
    }
    fn point_in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
        let ab = b - a;
        let bc = c - b;
        let ca = a - c;
        let ap = p - a;
        let bp = p - b;
        let cp = p - c;
        let c1 = cross2(ab, ap);
        let c2 = cross2(bc, bp);
        let c3 = cross2(ca, cp);
        (c1 >= 0.0 && c2 >= 0.0 && c3 >= 0.0) || (c1 <= 0.0 && c2 <= 0.0 && c3 <= 0.0)
    }
    fn point_in_quad(p: Vec2, q: &[Vec2; 4]) -> bool {
        point_in_triangle(p, q[0], q[1], q[2]) || point_in_triangle(p, q[0], q[2], q[3])
    }

    let mut best: Option<HitTestResult> = None;
    let dist_cam = (center - state.camera_eye).length();

    // XY plane (Z normal)
    {
        let q = make_quad(x_axis, y_axis)?;
        if point_in_quad(mouse_pos, &q) {
            best = Some(HitTestResult {
                hit: true,
                manipulation_type: ManipulationType::MoveXY,
                distance: dist_cam,
                screen_distance: 0.0,
                proportion: Vec3::new(1.0, 1.0, 0.0),
            });
        }
    }
    // XZ plane (Y normal)
    {
        let q = make_quad(x_axis, z_axis)?;
        if point_in_quad(mouse_pos, &q) {
            let hr = HitTestResult {
                hit: true,
                manipulation_type: ManipulationType::MoveZX,
                distance: dist_cam,
                screen_distance: 0.0,
                proportion: Vec3::new(1.0, 0.0, 1.0),
            };
            if best
                .as_ref()
                .map(|b| hr.distance < b.distance)
                .unwrap_or(true)
            {
                best = Some(hr);
            }
        }
    }
    // YZ plane (X normal)
    {
        let q = make_quad(y_axis, z_axis)?;
        if point_in_quad(mouse_pos, &q) {
            let hr = HitTestResult {
                hit: true,
                manipulation_type: ManipulationType::MoveYZ,
                distance: dist_cam,
                screen_distance: 0.0,
                proportion: Vec3::new(0.0, 1.0, 1.0),
            };
            if best
                .as_ref()
                .map(|b| hr.distance < b.distance)
                .unwrap_or(true)
            {
                best = Some(hr);
            }
        }
    }

    Ok(best)
}

/// Test collision with rotation circles
fn test_rotation_circles(
    state: &GuizmoState,
    mouse_pos: Vec2,
) -> GuizmoResult<Option<HitTestResult>> {
    // Sample circle points for each axis ring and find closest distance
    let gizmo_center = state.model_matrix.transform_point3(Vec3::ZERO);
    let gizmo_size = crate::draw::calculate_gizmo_size(
        &state.view_matrix,
        gizmo_center,
        state.gizmo_size_clip_space,
    );
    let radius = gizmo_size * 0.9;
    let viewport = state.viewport.as_viewport();
    let mvp = state.mvp;

    // Thickness threshold based on style
    let thickness = state.style.rotation_line_thickness.max(1.0);
    let hit_threshold = thickness * 2.5; // generous margin

    // Helper: build polyline of projected circle for an axis
    fn project_circle(
        mvp: &Mat4,
        center: Vec3,
        radius: f32,
        axis: Vec3,
        viewport: &[f32; 4],
        segments: u32,
    ) -> GuizmoResult<Vec<Vec2>> {
        // Create two perpendicular vectors to axis
        let up = if axis.dot(Vec3::Y).abs() < 0.9 {
            Vec3::Y
        } else {
            Vec3::X
        };
        let right = axis.cross(up).normalize();
        let forward = axis.cross(right).normalize();
        let mut pts = Vec::with_capacity((segments + 1) as usize);
        for i in 0..=segments {
            let angle = (i as f32 / segments as f32) * 2.0 * std::f32::consts::PI;
            let cos_a = angle.cos();
            let sin_a = angle.sin();
            let p3 = center + (right * cos_a + forward * sin_a) * radius;
            let p2 = crate::draw::project_to_screen(mvp, p3, viewport)?;
            pts.push(p2);
        }
        Ok(pts)
    }

    fn polyline_min_distance(p: Vec2, poly: &[Vec2]) -> f32 {
        if poly.len() < 2 {
            return f32::INFINITY;
        }
        let mut best = f32::INFINITY;
        for w in poly.windows(2) {
            let d = distance_point_to_line_segment(p, w[0], w[1]);
            if d < best {
                best = d;
            }
        }
        best
    }

    let segments = 64u32;
    let mut best_result = HitTestResult::default();

    // X ring (YZ plane)
    let circ_x = project_circle(&mvp, gizmo_center, radius, Vec3::X, &viewport, segments)?;
    let dist_x = polyline_min_distance(mouse_pos, &circ_x);
    if dist_x < hit_threshold {
        let distance = (gizmo_center - state.camera_eye).length();
        best_result = HitTestResult {
            hit: true,
            manipulation_type: ManipulationType::RotateX,
            distance,
            screen_distance: dist_x,
            proportion: Vec3::new(1.0, 0.0, 0.0),
        };
    }

    // Y ring (XZ plane)
    let circ_y = project_circle(&mvp, gizmo_center, radius, Vec3::Y, &viewport, segments)?;
    let dist_y = polyline_min_distance(mouse_pos, &circ_y);
    if dist_y < hit_threshold && (!best_result.hit || dist_y < dist_x) {
        let distance = (gizmo_center - state.camera_eye).length();
        best_result = HitTestResult {
            hit: true,
            manipulation_type: ManipulationType::RotateY,
            distance,
            screen_distance: dist_y,
            proportion: Vec3::new(0.0, 1.0, 0.0),
        };
    }

    // Z ring (XY plane)
    let circ_z = project_circle(&mvp, gizmo_center, radius, Vec3::Z, &viewport, segments)?;
    let dist_z = polyline_min_distance(mouse_pos, &circ_z);
    if dist_z < hit_threshold && (!best_result.hit || dist_z < dist_x.min(dist_y)) {
        let distance = (gizmo_center - state.camera_eye).length();
        best_result = HitTestResult {
            hit: true,
            manipulation_type: ManipulationType::RotateZ,
            distance,
            screen_distance: dist_z,
            proportion: Vec3::new(0.0, 0.0, 1.0),
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
        &state.view_matrix,
        gizmo_center,
        state.gizmo_size_clip_space,
    );

    let axis_length = gizmo_size * 0.8;
    let handle_size = gizmo_size * 0.08;
    let mut best_result = HitTestResult::default();

    // Uniform scale center handle (circle) hit-test
    let center_radius = state.style.center_circle_size.max(2.0);
    let center_dist = (mouse_pos - screen_center).length();
    if center_dist <= center_radius * 1.2 {
        let distance = (gizmo_center - state.camera_eye).length();
        best_result = HitTestResult {
            hit: true,
            manipulation_type: ManipulationType::ScaleXYZ,
            distance,
            screen_distance: center_dist,
            proportion: Vec3::splat(1.0),
        };
    }

    // In UNIVERSAL 模式下仅启用中心统一缩放；轴向缩放需显式包含 SCALE
    if !state.operation.intersects(crate::types::Operation::SCALE) {
        return Ok(if best_result.hit {
            Some(best_result)
        } else {
            None
        });
    }

    // Test X scale handle
    let x_handle_pos = Vec2::new(screen_center.x + axis_length, screen_center.y);
    let dx = (mouse_pos - x_handle_pos).length();
    if dx < handle_size {
        let distance = (gizmo_center - state.camera_eye).length();
        best_result = HitTestResult {
            hit: true,
            manipulation_type: ManipulationType::ScaleX,
            distance,
            screen_distance: dx,
            proportion: Vec3::new(1.0, 0.0, 0.0),
        };
    }

    // Test Y scale handle
    let y_handle_pos = Vec2::new(screen_center.x, screen_center.y - axis_length);
    let dy = (mouse_pos - y_handle_pos).length();
    if dy < handle_size {
        let distance = (gizmo_center - state.camera_eye).length();
        if distance < best_result.distance || !best_result.hit {
            best_result = HitTestResult {
                hit: true,
                manipulation_type: ManipulationType::ScaleY,
                distance,
                screen_distance: dy,
                proportion: Vec3::new(0.0, 1.0, 0.0),
            };
        }
    }

    // Test Z scale handle
    let z_handle_pos = Vec2::new(
        screen_center.x + axis_length * 0.7,
        screen_center.y + axis_length * 0.7,
    );
    let dz = (mouse_pos - z_handle_pos).length();
    if dz < handle_size {
        let distance = (gizmo_center - state.camera_eye).length();
        if distance < best_result.distance || !best_result.hit {
            best_result = HitTestResult {
                hit: true,
                manipulation_type: ManipulationType::ScaleZ,
                distance,
                screen_distance: dz,
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

/// Test if mouse is over a bounds corner, return Some(index) if hit
pub(crate) fn is_over_bounds_corners(
    state: &GuizmoState,
    mouse_pos: Vec2,
) -> GuizmoResult<Option<usize>> {
    let local = match state.local_bounds {
        Some(lb) => lb,
        None => return Ok(None),
    };
    // Project corners
    let min = Vec3::new(local[0], local[2], local[4]);
    let max = Vec3::new(local[1], local[3], local[5]);
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
    let radius = 8.0; // pixels
    let mut best: Option<(usize, f32)> = None;
    for (i, c) in corners.iter().enumerate() {
        let p = crate::draw::project_to_screen(&state.mvp, *c, &state.viewport.as_viewport())?;
        let d = (mouse_pos - p).length();
        if d <= radius {
            if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                best = Some((i, d));
            }
        }
    }
    Ok(best.map(|(i, _)| i))
}

/// Test if mouse is over a bounds face center, return Some(face_index) if hit
/// face_index order: [minX(0),maxX(1),minY(2),maxY(3),minZ(4),maxZ(5)]
pub(crate) fn is_over_bounds_faces(
    state: &GuizmoState,
    mouse_pos: Vec2,
) -> GuizmoResult<Option<usize>> {
    let local = match state.local_bounds {
        Some(lb) => lb,
        None => return Ok(None),
    };
    // Compute face centers in local
    let min = Vec3::new(local[0], local[2], local[4]);
    let max = Vec3::new(local[1], local[3], local[5]);
    let centers = [
        Vec3::new(min.x, (min.y + max.y) * 0.5, (min.z + max.z) * 0.5), // minX
        Vec3::new(max.x, (min.y + max.y) * 0.5, (min.z + max.z) * 0.5), // maxX
        Vec3::new((min.x + max.x) * 0.5, min.y, (min.z + max.z) * 0.5), // minY
        Vec3::new((min.x + max.x) * 0.5, max.y, (min.z + max.z) * 0.5), // maxY
        Vec3::new((min.x + max.x) * 0.5, (min.y + max.y) * 0.5, min.z), // minZ
        Vec3::new((min.x + max.x) * 0.5, (min.y + max.y) * 0.5, max.z), // maxZ
    ];
    let mut best: Option<(usize, f32)> = None;
    let radius = 8.0; // pixels
    for (i, c) in centers.iter().enumerate() {
        let p = crate::draw::project_to_screen(&state.mvp, *c, &state.viewport.as_viewport())?;
        let d = (mouse_pos - p).length();
        if d <= radius {
            if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                best = Some((i, d));
            }
        }
    }
    Ok(best.map(|(i, _)| i))
}

/// Test if mouse is over a bounds edge center, return Some(edge_index) if hit
/// Edge order:
///  - X-parallel: (y=min,z=min), (y=min,z=max), (y=max,z=min), (y=max,z=max) => 0..3
///  - Y-parallel: (x=min,z=min), (x=min,z=max), (x=max,z=min), (x=max,z=max) => 4..7
///  - Z-parallel: (x=min,y=min), (x=min,y=max), (x=max,y=min), (x=max,y=max) => 8..11
pub(crate) fn is_over_bounds_edges(
    state: &GuizmoState,
    mouse_pos: Vec2,
) -> GuizmoResult<Option<usize>> {
    let local = match state.local_bounds {
        Some(lb) => lb,
        None => return Ok(None),
    };
    let min = Vec3::new(local[0], local[2], local[4]);
    let max = Vec3::new(local[1], local[3], local[5]);
    let centers = [
        // X-parallel
        Vec3::new((min.x + max.x) * 0.5, min.y, min.z),
        Vec3::new((min.x + max.x) * 0.5, min.y, max.z),
        Vec3::new((min.x + max.x) * 0.5, max.y, min.z),
        Vec3::new((min.x + max.x) * 0.5, max.y, max.z),
        // Y-parallel
        Vec3::new(min.x, (min.y + max.y) * 0.5, min.z),
        Vec3::new(min.x, (min.y + max.y) * 0.5, max.z),
        Vec3::new(max.x, (min.y + max.y) * 0.5, min.z),
        Vec3::new(max.x, (min.y + max.y) * 0.5, max.z),
        // Z-parallel
        Vec3::new(min.x, min.y, (min.z + max.z) * 0.5),
        Vec3::new(min.x, max.y, (min.z + max.z) * 0.5),
        Vec3::new(max.x, min.y, (min.z + max.z) * 0.5),
        Vec3::new(max.x, max.y, (min.z + max.z) * 0.5),
    ];
    let radius = 8.0; // pixels
    let mut best: Option<(usize, f32)> = None;
    for (i, c) in centers.iter().enumerate() {
        let p = crate::draw::project_to_screen(&state.mvp, *c, &state.viewport.as_viewport())?;
        let d = (mouse_pos - p).length();
        if d <= radius {
            if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                best = Some((i, d));
            }
        }
    }
    Ok(best.map(|(i, _)| i))
}

/// Handle bounds dragging interaction (simplified)
fn handle_bounds_interaction(
    state: &mut GuizmoState,
    mouse_pos: Vec2,
    snap: Option<&[f32; 3]>,
) -> GuizmoResult<bool> {
    let lb = match state.local_bounds {
        Some(b) => b,
        None => return Ok(false),
    };
    // Setup corner/edge/face handle in local space
    let lx = [lb[0], lb[1]];
    let ly = [lb[2], lb[3]];
    let lz = [lb[4], lb[5]];
    let (corner_local, anchor_local) = if state.selected_bounds_corner >= 0 {
        let corner = state.selected_bounds_corner as usize;
        let ix = (corner & 1) as usize; // x bit
        let iy = ((corner >> 1) & 1) as usize; // y bit
        let iz = ((corner >> 2) & 1) as usize; // z bit
        (
            Vec3::new(lx[ix], ly[iy], lz[iz]),
            Vec3::new(lx[1 - ix], ly[1 - iy], lz[1 - iz]),
        )
    } else if state.selected_bounds_edge >= 0 {
        // Edge order: X-parallel(0..3), Y-parallel(4..7), Z-parallel(8..11)
        let e = state.selected_bounds_edge as usize;
        if e < 4 {
            let ysel = if e == 0 || e == 1 { 0 } else { 1 };
            let zsel = if e == 0 || e == 2 { 0 } else { 1 };
            // No scaling along X: keep same X for corner and anchor (use mid X)
            let xmid = (lx[0] + lx[1]) * 0.5;
            (
                Vec3::new(xmid, ly[ysel], lz[zsel]),
                Vec3::new(xmid, ly[1 - ysel], lz[1 - zsel]),
            )
        } else if e < 8 {
            let xsel = if e == 4 || e == 5 { 0 } else { 1 };
            let zsel = if e == 4 || e == 6 { 0 } else { 1 };
            let ymid = (ly[0] + ly[1]) * 0.5;
            (
                Vec3::new(lx[xsel], ymid, lz[zsel]),
                Vec3::new(lx[1 - xsel], ymid, lz[1 - zsel]),
            )
        } else {
            let xsel = if e == 8 || e == 9 { 0 } else { 1 };
            let ysel = if e == 8 || e == 10 { 0 } else { 1 };
            let zmid = (lz[0] + lz[1]) * 0.5;
            (
                Vec3::new(lx[xsel], ly[ysel], zmid),
                Vec3::new(lx[1 - xsel], ly[1 - ysel], zmid),
            )
        }
    } else if state.selected_bounds_face >= 0 {
        // face index order: [minX(0),maxX(1),minY(2),maxY(3),minZ(4),maxZ(5)]
        let face = state.selected_bounds_face as usize;
        match face {
            0 | 1 => {
                let ix = if face == 0 { 0 } else { 1 };
                (
                    Vec3::new(lx[ix], ly[0], lz[0]),
                    Vec3::new(lx[1 - ix], ly[0], lz[0]),
                )
            }
            2 | 3 => {
                let iy = if face == 2 { 0 } else { 1 };
                (
                    Vec3::new(lx[0], ly[iy], lz[0]),
                    Vec3::new(lx[0], ly[1 - iy], lz[0]),
                )
            }
            4 | 5 => {
                let iz = if face == 4 { 0 } else { 1 };
                (
                    Vec3::new(lx[0], ly[0], lz[iz]),
                    Vec3::new(lx[0], ly[0], lz[1 - iz]),
                )
            }
            _ => return Ok(false),
        }
    } else {
        return Ok(false);
    };
    // World axes from orthonormalized model
    let ex = state.model_local.transform_vector3(Vec3::X).normalize();
    let ey = state.model_local.transform_vector3(Vec3::Y).normalize();
    let ez = state.model_local.transform_vector3(Vec3::Z).normalize();

    // Choose dragging plane: exclude the axis most aligned with camera
    let dx = ex.dot(state.camera_dir).abs();
    let dy = ey.dot(state.camera_dir).abs();
    let dz = ez.dot(state.camera_dir).abs();
    let (u_axis, v_axis) = if dx > dy && dx > dz {
        (ey, ez)
    } else if dy > dz {
        (ex, ez)
    } else {
        (ex, ey)
    };

    // Plane through current corner world position
    let corner_world = state.model_source.transform_point3(corner_local);
    let plane_normal = u_axis.cross(v_axis).normalize();

    // Intersect mouse ray with this plane
    let t = crate::math::intersect_ray_plane(
        state.ray_origin,
        state.ray_vector,
        [
            plane_normal.x,
            plane_normal.y,
            plane_normal.z,
            -plane_normal.dot(corner_world),
        ],
    );
    if !t.is_finite() {
        return Ok(false);
    }
    let hit_world = state.ray_origin + state.ray_vector * t;

    // Convert to local
    let hit_local4 = state.model_source_inverse * hit_world.extend(1.0);
    let hit_local = hit_local4.truncate() / hit_local4.w.max(f32::EPSILON);

    // Compute per-axis scale relative to anchor
    let denom = corner_local - anchor_local;
    let mut scale = Vec3::ONE;
    if denom.x.abs() > f32::EPSILON {
        scale.x = (hit_local.x - anchor_local.x) / denom.x;
    }
    if denom.y.abs() > f32::EPSILON {
        scale.y = (hit_local.y - anchor_local.y) / denom.y;
    }
    if denom.z.abs() > f32::EPSILON {
        scale.z = (hit_local.z - anchor_local.z) / denom.z;
    }

    // Snap scale distances from anchor
    if let Some(s) = snap {
        if s.len() >= 3 {
            if s[0] > 0.0 && denom.x.abs() > f32::EPSILON {
                let len = (hit_local.x - anchor_local.x);
                let snapped = (len / s[0]).round() * s[0];
                scale.x = snapped / denom.x;
            }
            if s[1] > 0.0 && denom.y.abs() > f32::EPSILON {
                let len = (hit_local.y - anchor_local.y);
                let snapped = (len / s[1]).round() * s[1];
                scale.y = snapped / denom.y;
            }
            if s[2] > 0.0 && denom.z.abs() > f32::EPSILON {
                let len = (hit_local.z - anchor_local.z);
                let snapped = (len / s[2]).round() * s[2];
                scale.z = snapped / denom.z;
            }
        }
    }

    // Apply local scale about anchor
    let t_neg = Mat4::from_translation(-anchor_local);
    let s_m = Mat4::from_scale(scale.max(Vec3::splat(0.001)));
    let t_pos = Mat4::from_translation(anchor_local);
    let local = t_pos * s_m * t_neg;
    state.model_matrix = state.model_source * local;
    state.model_inverse = state.model_matrix.inverse();

    // Recompute local bounds in real time to reflect dragged face/edge/corner
    // newP = anchor + scale * (P - anchor)
    let mut new_min = Vec3::new(lx[0], ly[0], lz[0]);
    let mut new_max = Vec3::new(lx[1], ly[1], lz[1]);
    // Apply per-axis scaling around anchor to both min/max
    for i in 0..3 {
        let (a, s, mn, mx) = match i {
            0 => (anchor_local.x, scale.x, new_min.x, new_max.x),
            1 => (anchor_local.y, scale.y, new_min.y, new_max.y),
            _ => (anchor_local.z, scale.z, new_min.z, new_max.z),
        };
        let mn2 = a + s * (mn - a);
        let mx2 = a + s * (mx - a);
        let (lo, hi) = if mn2 <= mx2 { (mn2, mx2) } else { (mx2, mn2) };
        match i {
            0 => {
                new_min.x = lo;
                new_max.x = hi;
            }
            1 => {
                new_min.y = lo;
                new_max.y = hi;
            }
            _ => {
                new_min.z = lo;
                new_max.z = hi;
            }
        }
    }
    // Apply bounds_snap on resulting sizes per active axis
    if let Some(bs) = state.bounds_snap {
        let eps = 1e-4;
        // helper to snap along axis
        for i in 0..3 {
            let active = match i {
                0 => denom.x.abs() > f32::EPSILON,
                1 => denom.y.abs() > f32::EPSILON,
                _ => denom.z.abs() > f32::EPSILON,
            };
            let snap_val = bs[i];
            if active && snap_val > 0.0 {
                // current size
                let (mn, mx, a) = match i {
                    0 => (new_min.x, new_max.x, anchor_local.x),
                    1 => (new_min.y, new_max.y, anchor_local.y),
                    _ => (new_min.z, new_max.z, anchor_local.z),
                };
                let mut size = (mx - mn).abs();
                if size > eps {
                    let snapped = (size / snap_val).round() * snap_val;
                    // adjust opposite side while keeping anchor side fixed
                    let anchor_is_min = (a - mn).abs() < (a - mx).abs();
                    let (mut mn2, mut mx2) = (mn, mx);
                    if anchor_is_min {
                        mn2 = a;
                        mx2 = a + snapped;
                    } else {
                        mx2 = a;
                        mn2 = a - snapped;
                    }
                    // enforce ordering
                    let (lo, hi) = if mn2 <= mx2 { (mn2, mx2) } else { (mx2, mn2) };
                    match i {
                        0 => {
                            new_min.x = lo;
                            new_max.x = hi;
                        }
                        1 => {
                            new_min.y = lo;
                            new_max.y = hi;
                        }
                        _ => {
                            new_min.z = lo;
                            new_max.z = hi;
                        }
                    }
                    // adjust scale accordingly
                    let ratio = if snapped > eps { snapped / size } else { 1.0 };
                    match i {
                        0 => scale.x *= ratio,
                        1 => scale.y *= ratio,
                        _ => scale.z *= ratio,
                    }
                }
            }
        }
        // Rebuild model transform with snapped scale
        let t_neg = Mat4::from_translation(-anchor_local);
        let s_m = Mat4::from_scale(scale.max(Vec3::splat(0.001)));
        let t_pos = Mat4::from_translation(anchor_local);
        let local = t_pos * s_m * t_neg;
        state.model_matrix = state.model_source * local;
        state.model_inverse = state.model_matrix.inverse();
    }
    state.local_bounds = Some([
        new_min.x, new_max.x, new_min.y, new_max.y, new_min.z, new_max.z,
    ]);
    Ok(true)
}
/// Handle translation interaction
fn handle_translation_interaction(
    state: &mut GuizmoState,
    mouse_pos: Vec2,
    _delta: Vec2,
    snap: Option<&[f32; 3]>,
) -> GuizmoResult<bool> {
    // World-space translation via ray-plane intersection + axis/plane projection
    let center = state.model_source.transform_point3(Vec3::ZERO);
    let ex = state.model_local.transform_vector3(Vec3::X).normalize();
    let ey = state.model_local.transform_vector3(Vec3::Y).normalize();
    let ez = state.model_local.transform_vector3(Vec3::Z).normalize();

    // Intersect current ray with stored plane
    let t = crate::math::intersect_ray_plane(
        state.ray_origin,
        state.ray_vector,
        state.translation_plane,
    );
    if !t.is_finite() {
        return Ok(false);
    }
    let hit = state.ray_origin + state.ray_vector * t;
    let start = state.translation_plane_origin + state.relative_origin;
    let delta_world = hit - start;

    let mut translation = Vec3::ZERO;
    match state.using {
        ManipulationType::MoveX => {
            let d = delta_world.dot(ex);
            let mut d = d;
            if let Some(s) = snap {
                if s.len() >= 1 && s[0] > 0.0 {
                    d = (d / s[0]).round() * s[0];
                }
            }
            translation = ex * d;
        }
        ManipulationType::MoveY => {
            let d = delta_world.dot(ey);
            let mut d = d;
            if let Some(s) = snap {
                if s.len() >= 2 && s[1] > 0.0 {
                    d = (d / s[1]).round() * s[1];
                }
            }
            translation = ey * d;
        }
        ManipulationType::MoveZ => {
            let d = delta_world.dot(ez);
            let mut d = d;
            if let Some(s) = snap {
                if s.len() >= 3 && s[2] > 0.0 {
                    d = (d / s[2]).round() * s[2];
                }
            }
            translation = ez * d;
        }
        ManipulationType::MoveXY => {
            let mut ux = delta_world.dot(ex);
            let mut vy = delta_world.dot(ey);
            if let Some(s) = snap {
                if s.len() >= 2 {
                    if s[0] > 0.0 {
                        ux = (ux / s[0]).round() * s[0];
                    }
                    if s[1] > 0.0 {
                        vy = (vy / s[1]).round() * s[1];
                    }
                }
            }
            translation = ex * ux + ey * vy;
        }
        ManipulationType::MoveYZ => {
            let mut uy = delta_world.dot(ey);
            let mut vz = delta_world.dot(ez);
            if let Some(s) = snap {
                if s.len() >= 3 {
                    if s[1] > 0.0 {
                        uy = (uy / s[1]).round() * s[1];
                    }
                    if s[2] > 0.0 {
                        vz = (vz / s[2]).round() * s[2];
                    }
                }
            }
            translation = ey * uy + ez * vz;
        }
        ManipulationType::MoveZX => {
            let mut uz = delta_world.dot(ez);
            let mut vx = delta_world.dot(ex);
            if let Some(s) = snap {
                if s.len() >= 3 {
                    if s[2] > 0.0 {
                        uz = (uz / s[2]).round() * s[2];
                    }
                    if s[0] > 0.0 {
                        vx = (vx / s[0]).round() * s[0];
                    }
                }
            }
            translation = ez * uz + ex * vx;
        }
        _ => return Ok(false),
    }

    // Apply translation to model matrix
    let translation_matrix = Mat4::from_translation(translation);
    state.model_matrix = translation_matrix * state.model_source;
    state.model_inverse = state.model_matrix.inverse();
    Ok(true)
}

/// Setup translation plane and origin at the start of a translation interaction
fn setup_translation_start(state: &mut GuizmoState) -> GuizmoResult<()> {
    let center = state.model_source.transform_point3(Vec3::ZERO);
    let ex = state.model_local.transform_vector3(Vec3::X).normalize();
    let ey = state.model_local.transform_vector3(Vec3::Y).normalize();
    let ez = state.model_local.transform_vector3(Vec3::Z).normalize();
    let plane_normal = match state.using {
        ManipulationType::MoveX => {
            let mut n = ex.cross(state.camera_dir);
            if n.length_squared() < 1e-6 {
                n = ex.cross(state.camera_up);
            }
            if n.length_squared() < 1e-6 {
                n = ex.cross(state.camera_right);
            }
            n.normalize()
        }
        ManipulationType::MoveY => {
            let mut n = ey.cross(state.camera_dir);
            if n.length_squared() < 1e-6 {
                n = ey.cross(state.camera_up);
            }
            if n.length_squared() < 1e-6 {
                n = ey.cross(state.camera_right);
            }
            n.normalize()
        }
        ManipulationType::MoveZ => {
            let mut n = ez.cross(state.camera_dir);
            if n.length_squared() < 1e-6 {
                n = ez.cross(state.camera_up);
            }
            if n.length_squared() < 1e-6 {
                n = ez.cross(state.camera_right);
            }
            n.normalize()
        }
        ManipulationType::MoveXY => ez,
        ManipulationType::MoveYZ => ex,
        ManipulationType::MoveZX => ey,
        _ => Vec3::Z,
    };
    state.translation_plane = crate::math::build_plane(center, plane_normal);
    state.translation_plane_origin = center;
    state.matrix_origin = center;
    // Compute initial hit and store relative origin
    let t0 = crate::math::intersect_ray_plane(
        state.ray_origin,
        state.ray_vector,
        state.translation_plane,
    );
    if t0.is_finite() {
        let hit0 = state.ray_origin + state.ray_vector * t0;
        state.relative_origin = hit0 - center;
    } else {
        state.relative_origin = Vec3::ZERO;
    }
    Ok(())
}

/// Handle rotation interaction
fn handle_rotation_interaction(
    state: &mut GuizmoState,
    mouse_pos: Vec2,
    snap: Option<&[f32; 3]>,
) -> GuizmoResult<bool> {
    // Determine world-space axis for rotation
    let axis = match state.using {
        ManipulationType::RotateX => get_rotation_axis_world(state, 0),
        ManipulationType::RotateY => get_rotation_axis_world(state, 1),
        ManipulationType::RotateZ => get_rotation_axis_world(state, 2),
        _ => return Ok(false),
    };

    let current_angle = rotation_angle_on_axis(state, mouse_pos, axis)?;
    let mut angle_delta = current_angle - state.rotation_start_angle;

    // Optional snap (use snap[0] as degrees)
    if let Some(s) = snap {
        if !s.is_empty() && s[0] > 0.0 {
            let step = s[0].to_radians();
            angle_delta = (angle_delta / step).round() * step;
        }
    }

    // Rotate around object origin (center) in world-space about selected axis
    let center = state.model_source.transform_point3(Vec3::ZERO);
    let t_neg = Mat4::from_translation(-center);
    let t_pos = Mat4::from_translation(center);
    let rotation_matrix = Mat4::from_axis_angle(axis, angle_delta);
    state.model_matrix = t_pos * rotation_matrix * t_neg * state.model_source;
    state.model_inverse = state.model_matrix.inverse();
    Ok(true)
}

/// Handle scale interaction
fn handle_scale_interaction(
    state: &mut GuizmoState,
    _mouse_pos: Vec2,
    delta: Vec2,
    snap: Option<&[f32; 3]>,
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

    // Optional snap: use snap[0] as uniform step or per-axis if provided > 0
    if let Some(s) = snap {
        if s.len() >= 3 {
            let sx = if s[0] > 0.0 { s[0] } else { 0.0 };
            let sy = if s[1] > 0.0 { s[1] } else { 0.0 };
            let sz = if s[2] > 0.0 { s[2] } else { 0.0 };
            if sx > 0.0 {
                scale.x = (scale.x / sx).round() * sx;
            }
            if sy > 0.0 {
                scale.y = (scale.y / sy).round() * sy;
            }
            if sz > 0.0 {
                scale.z = (scale.z / sz).round() * sz;
            }
        } else if s.len() >= 1 && s[0] > 0.0 {
            let step = s[0];
            scale.x = (scale.x / step).round() * step;
            scale.y = (scale.y / step).round() * step;
            scale.z = (scale.z / step).round() * step;
        }
    }

    // Create scale matrix
    let scale_matrix = Mat4::from_scale(scale);

    // Apply scale to model matrix
    state.model_matrix = scale_matrix * state.model_source;
    state.model_inverse = state.model_matrix.inverse();

    Ok(true)
}

/// Initialize bounds drag parameters (best axis, axes set, local pivot, and drag plane)
fn setup_bounds_drag_start(
    state: &mut GuizmoState,
    corner: Option<i32>,
    edge: Option<i32>,
    face: Option<i32>,
) {
    let ex = state.model_local.transform_vector3(Vec3::X).normalize();
    let ey = state.model_local.transform_vector3(Vec3::Y).normalize();
    let ez = state.model_local.transform_vector3(Vec3::Z).normalize();
    let dx = ex.dot(state.camera_dir).abs();
    let dy = ey.dot(state.camera_dir).abs();
    let dz = ez.dot(state.camera_dir).abs();
    let (best_axis, second, third) = if dx > dy && dx > dz {
        (0, 1, 2)
    } else if dy > dz {
        (1, 0, 2)
    } else {
        (2, 0, 1)
    };
    state.bounds_best_axis = best_axis as i32;

    let lb = match state.local_bounds {
        Some(b) => b,
        None => return,
    };
    let lx = [lb[0], lb[1]];
    let ly = [lb[2], lb[3]];
    let lz = [lb[4], lb[5]];
    let mut pivot = Vec3::ZERO;
    if let Some(ci) = corner {
        let c = ci as usize;
        let ix = (c & 1) as usize;
        let iy = ((c >> 1) & 1) as usize;
        let iz = ((c >> 2) & 1) as usize;
        pivot = Vec3::new(lx[1 - ix], ly[1 - iy], lz[1 - iz]);
        state.bounds_axes = [second as i32, third as i32];
    } else if let Some(ei) = edge {
        let e = ei as usize;
        if e < 4 {
            // X-parallel
            state.bounds_axes = [1, -1];
            let ysel = if e == 0 || e == 1 { 0 } else { 1 };
            let zsel = if e == 0 || e == 2 { 0 } else { 1 };
            pivot = Vec3::new((lx[0] + lx[1]) * 0.5, ly[1 - ysel], lz[1 - zsel]);
        } else if e < 8 {
            // Y-parallel
            state.bounds_axes = [0, -1];
            let xsel = if e == 4 || e == 5 { 0 } else { 1 };
            let zsel = if e == 4 || e == 6 { 0 } else { 1 };
            pivot = Vec3::new(lx[1 - xsel], (ly[0] + ly[1]) * 0.5, lz[1 - zsel]);
        } else {
            // Z-parallel
            state.bounds_axes = [2, -1];
            let xsel = if e == 8 || e == 9 { 0 } else { 1 };
            let ysel = if e == 8 || e == 10 { 0 } else { 1 };
            pivot = Vec3::new(lx[1 - xsel], ly[1 - ysel], (lz[0] + lz[1]) * 0.5);
        }
    } else if let Some(fi) = face {
        let f = fi as usize;
        match f {
            0 | 1 => {
                state.bounds_axes = [0, -1];
            }
            2 | 3 => {
                state.bounds_axes = [1, -1];
            }
            _ => {
                state.bounds_axes = [2, -1];
            }
        }
        pivot = match f {
            0 => Vec3::new(lx[1], (ly[0] + ly[1]) * 0.5, (lz[0] + lz[1]) * 0.5),
            1 => Vec3::new(lx[0], (ly[0] + ly[1]) * 0.5, (lz[0] + lz[1]) * 0.5),
            2 => Vec3::new((lx[0] + lx[1]) * 0.5, ly[1], (lz[0] + lz[1]) * 0.5),
            3 => Vec3::new((lx[0] + lx[1]) * 0.5, ly[0], (lz[0] + lz[1]) * 0.5),
            4 => Vec3::new((lx[0] + lx[1]) * 0.5, (ly[0] + ly[1]) * 0.5, lz[1]),
            _ => Vec3::new((lx[0] + lx[1]) * 0.5, (ly[0] + ly[1]) * 0.5, lz[0]),
        };
    }
    state.bounds_local_pivot = pivot;
    let best_dir_world = match best_axis {
        0 => ex,
        1 => ey,
        _ => ez,
    };
    let pivot_world = state.model_source.transform_point3(pivot);
    let plane = crate::math::build_plane(pivot_world, best_dir_world);
    state.translation_plane = plane;
    state.translation_plane_origin = pivot_world;
}

/// Compute the world rotation axis according to current mode
fn get_rotation_axis_world(state: &GuizmoState, axis_index: usize) -> Vec3 {
    // axis_index: 0=X,1=Y,2=Z
    match state.mode {
        crate::types::Mode::Local => {
            let v = match axis_index {
                0 => Vec3::X,
                1 => Vec3::Y,
                _ => Vec3::Z,
            };
            state.model_local.transform_vector3(v).normalize()
        }
        crate::types::Mode::World => match axis_index {
            0 => Vec3::X,
            1 => Vec3::Y,
            _ => Vec3::Z,
        },
    }
}

/// Compute angle on axis plane by intersecting mouse ray with plane and measuring around basis
fn rotation_angle_on_axis(
    state: &GuizmoState,
    mouse_pos: Vec2,
    axis_world: Vec3,
) -> GuizmoResult<f32> {
    let center = state.model_source.transform_point3(Vec3::ZERO);

    // Recompute ray for provided mouse position (override state ray)
    // Build NDC from mouse and unproject near/far via inverse VP
    let viewport = state.viewport;
    let ndc_x = (mouse_pos.x - viewport.x) / viewport.width * 2.0 - 1.0;
    let ndc_y = 1.0 - (mouse_pos.y - viewport.y) / viewport.height * 2.0;
    let view_proj_inverse = state.view_projection.inverse();
    let far_point = glam::Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
    let world_far = view_proj_inverse * far_point;
    let world_far_pos = world_far.truncate() / world_far.w;
    let ray_origin = if state.orthographic {
        let near_point = glam::Vec4::new(ndc_x, ndc_y, -1.0, 1.0);
        let world_near = view_proj_inverse * near_point;
        world_near.truncate() / world_near.w
    } else {
        state.camera_eye
    };
    let mut ray_dir = if state.orthographic {
        state.camera_dir
    } else {
        (world_far_pos - ray_origin).normalize()
    };
    if !ray_dir.is_finite() {
        ray_dir = state.ray_vector;
    }

    // Intersect with rotation plane (normal = axis_world)
    let plane = crate::math::build_plane(center, axis_world);
    let t = crate::math::intersect_ray_plane(ray_origin, ray_dir, plane);
    if !t.is_finite() {
        // Fallback: use screen-based angle
        return calculate_rotation_angle(state, mouse_pos);
    }
    let hit = ray_origin + ray_dir * t;
    let vec = (hit - center);

    // Build orthonormal basis on plane
    let up = if axis_world.dot(Vec3::Y).abs() < 0.9 {
        Vec3::Y
    } else {
        Vec3::X
    };
    let right = axis_world.cross(up).normalize();
    let forward = axis_world.cross(right).normalize();

    // Project hit vector onto basis and compute atan2
    let x = vec.dot(right);
    let y = vec.dot(forward);
    Ok(y.atan2(x))
}

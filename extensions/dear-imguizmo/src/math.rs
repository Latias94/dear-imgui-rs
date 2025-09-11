//! Mathematical utilities for ImGuizmo
//!
//! This module provides matrix operations, transformations, and mathematical
//! utilities using the glam library for efficient computation.

use crate::types::{Mat4, Vec3, Vec4};
use crate::{GuizmoError, GuizmoResult};
use glam::{Quat, Vec2};

/// Matrix decomposition result
#[derive(Debug, Clone, PartialEq)]
pub struct MatrixDecomposition {
    /// Translation component
    pub translation: Vec3,
    /// Rotation component (as Euler angles in degrees)
    pub rotation: Vec3,
    /// Scale component
    pub scale: Vec3,
}

/// Decompose a 4x4 transformation matrix into translation, rotation, and scale components
///
/// This function extracts the transformation components from a matrix, similar to
/// the original ImGuizmo::DecomposeMatrixToComponents function.
///
/// # Arguments
/// * `matrix` - The 4x4 transformation matrix to decompose
///
/// # Returns
/// * `Ok(MatrixDecomposition)` - The decomposed components
/// * `Err(GuizmoError)` - If the matrix cannot be decomposed (e.g., singular matrix)
pub fn decompose_matrix(matrix: &Mat4) -> GuizmoResult<MatrixDecomposition> {
    // Extract translation (last column)
    let translation = matrix.w_axis.truncate();

    // Extract the 3x3 rotation/scale matrix
    let upper_left = glam::Mat3::from_cols(
        matrix.x_axis.truncate(),
        matrix.y_axis.truncate(),
        matrix.z_axis.truncate(),
    );

    // Extract scale (length of each column)
    let scale = Vec3::new(
        upper_left.x_axis.length(),
        upper_left.y_axis.length(),
        upper_left.z_axis.length(),
    );

    // Check for zero scale
    if scale.x.abs() < f32::EPSILON || scale.y.abs() < f32::EPSILON || scale.z.abs() < f32::EPSILON
    {
        return Err(GuizmoError::invalid_matrix(
            "Matrix has zero scale component",
        ));
    }

    // Extract rotation by normalizing the scaled matrix
    let rotation_matrix = glam::Mat3::from_cols(
        upper_left.x_axis / scale.x,
        upper_left.y_axis / scale.y,
        upper_left.z_axis / scale.z,
    );

    // Convert rotation matrix to quaternion, then to Euler angles
    let quat = Quat::from_mat3(&rotation_matrix);
    let (x, y, z) = quat.to_euler(glam::EulerRot::XYZ);

    // Convert to degrees
    let rotation = Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees());

    Ok(MatrixDecomposition {
        translation,
        rotation,
        scale,
    })
}

/// Recompose a transformation matrix from translation, rotation, and scale components
///
/// This function creates a 4x4 transformation matrix from individual components,
/// similar to the original ImGuizmo::RecomposeMatrixFromComponents function.
///
/// # Arguments
/// * `translation` - Translation vector
/// * `rotation` - Rotation as Euler angles in degrees
/// * `scale` - Scale vector
///
/// # Returns
/// * The composed 4x4 transformation matrix
pub fn recompose_matrix(translation: Vec3, rotation: Vec3, scale: Vec3) -> Mat4 {
    // Convert rotation from degrees to radians
    let rotation_rad = Vec3::new(
        rotation.x.to_radians(),
        rotation.y.to_radians(),
        rotation.z.to_radians(),
    );

    // Create rotation quaternion from Euler angles
    let quat = Quat::from_euler(
        glam::EulerRot::XYZ,
        rotation_rad.x,
        rotation_rad.y,
        rotation_rad.z,
    );

    // Create transformation matrix
    Mat4::from_scale_rotation_translation(scale, quat, translation)
}

/// Check if a matrix is orthonormal (rotation matrix)
pub fn is_orthonormal(matrix: &Mat4) -> bool {
    let upper_left = glam::Mat3::from_cols(
        matrix.x_axis.truncate(),
        matrix.y_axis.truncate(),
        matrix.z_axis.truncate(),
    );

    // Check if the matrix is orthogonal (transpose equals inverse)
    let transposed = upper_left.transpose();
    let should_be_identity = upper_left * transposed;
    let identity = glam::Mat3::IDENTITY;

    // Check if close to identity matrix
    const EPSILON: f32 = 1e-6;
    (should_be_identity - identity).abs_diff_eq(glam::Mat3::ZERO, EPSILON)
}

/// Normalize a matrix to make it orthonormal
pub fn orthonormalize_matrix(matrix: &Mat4) -> Mat4 {
    let translation = matrix.w_axis.truncate();

    // Extract and orthonormalize the 3x3 part
    let mut x_axis = matrix.x_axis.truncate().normalize();
    let mut y_axis = matrix.y_axis.truncate();
    let mut z_axis = matrix.z_axis.truncate();

    // Gram-Schmidt orthogonalization
    y_axis = (y_axis - x_axis * x_axis.dot(y_axis)).normalize();
    z_axis = (z_axis - x_axis * x_axis.dot(z_axis) - y_axis * y_axis.dot(z_axis)).normalize();

    Mat4::from_cols(
        x_axis.extend(0.0),
        y_axis.extend(0.0),
        z_axis.extend(0.0),
        translation.extend(1.0),
    )
}

/// Transform a point by a matrix
pub fn transform_point(point: Vec3, matrix: &Mat4) -> Vec3 {
    let homogeneous = matrix * point.extend(1.0);
    homogeneous.truncate()
}

/// Transform a vector by a matrix (ignoring translation)
pub fn transform_vector(vector: Vec3, matrix: &Mat4) -> Vec3 {
    let homogeneous = matrix * vector.extend(0.0);
    homogeneous.truncate()
}

/// Project a 3D point to screen coordinates
pub fn project_point(point: Vec3, mvp_matrix: &Mat4, viewport: &crate::Rect) -> Vec2 {
    let clip_space = mvp_matrix * point.extend(1.0);

    // Perspective divide
    let ndc = if clip_space.w != 0.0 {
        Vec3::new(
            clip_space.x / clip_space.w,
            clip_space.y / clip_space.w,
            clip_space.z / clip_space.w,
        )
    } else {
        clip_space.truncate()
    };

    // Convert to screen coordinates
    Vec2::new(
        viewport.x + (ndc.x + 1.0) * 0.5 * viewport.width,
        viewport.y + (1.0 - ndc.y) * 0.5 * viewport.height,
    )
}

/// Unproject a screen point to world coordinates
pub fn unproject_point(
    screen_pos: Vec2,
    depth: f32,
    mvp_matrix: &Mat4,
    viewport: &crate::Rect,
) -> GuizmoResult<Vec3> {
    // Convert screen coordinates to NDC
    let ndc = Vec3::new(
        (screen_pos.x - viewport.x) / viewport.width * 2.0 - 1.0,
        1.0 - (screen_pos.y - viewport.y) / viewport.height * 2.0,
        depth * 2.0 - 1.0,
    );

    // Inverse MVP transformation
    let inverse_mvp = mvp_matrix.inverse();
    let clip_space = ndc.extend(1.0);
    let world_space = inverse_mvp * clip_space;

    // Perspective divide
    if world_space.w.abs() < f32::EPSILON {
        return Err(GuizmoError::math_operation(
            "Cannot unproject point: w component is zero",
        ));
    }

    Ok(world_space.truncate() / world_space.w)
}

/// Calculate the distance from a point to a line segment
pub fn point_to_line_distance(point: Vec3, line_start: Vec3, line_end: Vec3) -> f32 {
    let line_vec = line_end - line_start;
    let point_vec = point - line_start;

    let line_length_sq = line_vec.length_squared();
    if line_length_sq < f32::EPSILON {
        // Line is actually a point
        return point_vec.length();
    }

    let t = (point_vec.dot(line_vec) / line_length_sq).clamp(0.0, 1.0);
    let projection = line_start + line_vec * t;
    (point - projection).length()
}

/// Calculate the distance from a point to a plane
pub fn point_to_plane_distance(point: Vec3, plane_point: Vec3, plane_normal: Vec3) -> f32 {
    let normalized_normal = plane_normal.normalize();
    (point - plane_point).dot(normalized_normal).abs()
}

/// Check if a ray intersects with a sphere
pub fn ray_sphere_intersection(
    ray_origin: Vec3,
    ray_direction: Vec3,
    sphere_center: Vec3,
    sphere_radius: f32,
) -> Option<f32> {
    let oc = ray_origin - sphere_center;
    let a = ray_direction.dot(ray_direction);
    let b = 2.0 * oc.dot(ray_direction);
    let c = oc.dot(oc) - sphere_radius * sphere_radius;

    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        None
    } else {
        let sqrt_discriminant = discriminant.sqrt();
        let t1 = (-b - sqrt_discriminant) / (2.0 * a);
        let t2 = (-b + sqrt_discriminant) / (2.0 * a);

        // Return the closest positive intersection
        if t1 > 0.0 {
            Some(t1)
        } else if t2 > 0.0 {
            Some(t2)
        } else {
            None
        }
    }
}

/// Create a perspective projection matrix
///
/// # Arguments
/// * `fov_degrees` - Field of view in degrees
/// * `aspect_ratio` - Aspect ratio (width/height)
/// * `near` - Near clipping plane
/// * `far` - Far clipping plane
pub fn perspective_matrix(fov_degrees: f32, aspect_ratio: f32, near: f32, far: f32) -> Mat4 {
    let fov_rad = fov_degrees.to_radians();
    let f = 1.0 / (fov_rad * 0.5).tan();

    Mat4::from_cols(
        glam::Vec4::new(f / aspect_ratio, 0.0, 0.0, 0.0),
        glam::Vec4::new(0.0, f, 0.0, 0.0),
        glam::Vec4::new(0.0, 0.0, (far + near) / (near - far), -1.0),
        glam::Vec4::new(0.0, 0.0, (2.0 * far * near) / (near - far), 0.0),
    )
}

/// Create a frustum projection matrix
///
/// # Arguments
/// * `left`, `right`, `bottom`, `top` - Frustum bounds
/// * `near`, `far` - Near and far clipping planes
pub fn frustum_matrix(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4 {
    let temp = 2.0 * near;
    let temp2 = right - left;
    let temp3 = top - bottom;
    let temp4 = far - near;

    Mat4::from_cols(
        glam::Vec4::new(temp / temp2, 0.0, 0.0, 0.0),
        glam::Vec4::new(0.0, temp / temp3, 0.0, 0.0),
        glam::Vec4::new(
            (right + left) / temp2,
            (top + bottom) / temp3,
            -(far + near) / temp4,
            -1.0,
        ),
        glam::Vec4::new(0.0, 0.0, -(temp * far) / temp4, 0.0),
    )
}

/// Create a look-at view matrix
///
/// # Arguments
/// * `eye` - Camera position
/// * `target` - Target position to look at
/// * `up` - Up vector
pub fn look_at_matrix(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
    let forward = (target - eye).normalize();
    let right = forward.cross(up).normalize();
    let up = right.cross(forward);

    Mat4::from_cols(
        right.extend(0.0),
        up.extend(0.0),
        (-forward).extend(0.0),
        eye.extend(1.0),
    )
    .inverse()
}

/// Compute camera ray from screen coordinates
///
/// # Arguments
/// * `screen_pos` - Screen position (in pixels)
/// * `viewport` - Viewport rectangle
/// * `view_projection_inverse` - Inverse of view-projection matrix
///
/// # Returns
/// * `(ray_origin, ray_direction)` - Camera ray in world space
pub fn compute_camera_ray(
    screen_pos: Vec2,
    viewport: &crate::Rect,
    view_projection_inverse: &Mat4,
) -> (Vec3, Vec3) {
    // Convert screen coordinates to normalized device coordinates
    let ndc_x = ((screen_pos.x - viewport.x) / viewport.width) * 2.0 - 1.0;
    let ndc_y = 1.0 - ((screen_pos.y - viewport.y) / viewport.height) * 2.0;

    // Near and far points in NDC
    let near_ndc = glam::Vec4::new(ndc_x, ndc_y, 0.0, 1.0);
    let far_ndc = glam::Vec4::new(ndc_x, ndc_y, 1.0, 1.0);

    // Transform to world space
    let near_world = *view_projection_inverse * near_ndc;
    let far_world = *view_projection_inverse * far_ndc;

    // Perspective divide
    let ray_origin = if near_world.w.abs() > f32::EPSILON {
        near_world.truncate() / near_world.w
    } else {
        near_world.truncate()
    };

    let ray_end = if far_world.w.abs() > f32::EPSILON {
        far_world.truncate() / far_world.w
    } else {
        far_world.truncate()
    };

    let ray_direction = (ray_end - ray_origin).normalize();

    (ray_origin, ray_direction)
}

/// Linear interpolation between two values
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Smooth step interpolation
pub fn smooth_step(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Check if a ray intersects with a plane
///
/// # Arguments
/// * `ray_origin` - Origin of the ray
/// * `ray_direction` - Direction of the ray (should be normalized)
/// * `plane_point` - A point on the plane
/// * `plane_normal` - Normal vector of the plane (should be normalized)
///
/// # Returns
/// * `Some(distance)` if intersection occurs, `None` otherwise
pub fn ray_plane_intersection(
    ray_origin: Vec3,
    ray_direction: Vec3,
    plane_point: Vec3,
    plane_normal: Vec3,
) -> Option<f32> {
    let denom = plane_normal.dot(ray_direction);
    if denom.abs() < f32::EPSILON {
        return None; // Ray is parallel to plane
    }

    let t = (plane_point - ray_origin).dot(plane_normal) / denom;
    if t >= 0.0 {
        Some(t)
    } else {
        None
    }
}

/// Clamp a value between min and max
pub fn clamp(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// Intersect a ray with a plane
/// Returns the distance along the ray to the intersection point
pub fn intersect_ray_plane(ray_origin: Vec3, ray_direction: Vec3, plane: [f32; 4]) -> f32 {
    let plane_normal = Vec3::new(plane[0], plane[1], plane[2]);
    let plane_distance = plane[3];

    let denominator = ray_direction.dot(plane_normal);
    if denominator.abs() < f32::EPSILON {
        // Ray is parallel to plane
        return f32::INFINITY;
    }

    let t = -(ray_origin.dot(plane_normal) + plane_distance) / denominator;
    t
}

/// Build a plane equation from a point and normal
/// Returns [a, b, c, d] where ax + by + cz + d = 0
pub fn build_plane(point: Vec3, normal: Vec3) -> [f32; 4] {
    let normalized = normal.normalize();
    let d = -point.dot(normalized);
    [normalized.x, normalized.y, normalized.z, d]
}

/// Convert degrees to radians
pub fn deg_to_rad(degrees: f32) -> f32 {
    degrees * std::f32::consts::PI / 180.0
}

/// Convert radians to degrees
pub fn rad_to_deg(radians: f32) -> f32 {
    radians * 180.0 / std::f32::consts::PI
}

/// Check if a matrix contains only finite values (no NaN or infinity)
pub fn is_matrix_finite(matrix: &Mat4) -> bool {
    matrix.to_cols_array().iter().all(|&x| x.is_finite())
}

/// Check if a matrix is approximately equal to another matrix
pub fn matrices_approximately_equal(a: &Mat4, b: &Mat4, epsilon: f32) -> bool {
    let a_array = a.to_cols_array();
    let b_array = b.to_cols_array();

    a_array
        .iter()
        .zip(b_array.iter())
        .all(|(&a_val, &b_val)| (a_val - b_val).abs() < epsilon)
}

/// Get the length of a segment in clip space
pub fn get_segment_length_clip_space(start: Vec3, direction: Vec3, view_projection: &Mat4) -> f32 {
    let start_clip = view_projection.transform_point3(start);
    let end_clip = view_projection.transform_point3(start + direction);

    // Convert to normalized device coordinates
    let start_ndc = if start_clip.z != 0.0 {
        Vec3::new(
            start_clip.x / start_clip.z,
            start_clip.y / start_clip.z,
            start_clip.z,
        )
    } else {
        start_clip
    };

    let end_ndc = if end_clip.z != 0.0 {
        Vec3::new(end_clip.x / end_clip.z, end_clip.y / end_clip.z, end_clip.z)
    } else {
        end_clip
    };

    // Return the 2D distance in screen space
    (end_ndc.truncate() - start_ndc.truncate()).length()
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_matrix_decomposition_identity() {
        let identity = Mat4::IDENTITY;
        let decomp = decompose_matrix(&identity).unwrap();

        assert_relative_eq!(decomp.translation, Vec3::ZERO, epsilon = 1e-6);
        assert_relative_eq!(decomp.rotation, Vec3::ZERO, epsilon = 1e-6);
        assert_relative_eq!(decomp.scale, Vec3::ONE, epsilon = 1e-6);
    }

    #[test]
    fn test_matrix_recomposition() {
        let translation = Vec3::new(1.0, 2.0, 3.0);
        let rotation = Vec3::new(30.0, 45.0, 60.0);
        let scale = Vec3::new(2.0, 1.5, 0.5);

        let matrix = recompose_matrix(translation, rotation, scale);
        let decomp = decompose_matrix(&matrix).unwrap();

        assert_relative_eq!(decomp.translation, translation, epsilon = 1e-5);
        assert_relative_eq!(decomp.scale, scale, epsilon = 1e-5);
        // Rotation comparison is more complex due to Euler angle ambiguity
    }

    #[test]
    fn test_orthonormal_check() {
        let identity = Mat4::IDENTITY;
        assert!(is_orthonormal(&identity));

        let scaled = Mat4::from_scale(Vec3::new(2.0, 2.0, 2.0));
        assert!(!is_orthonormal(&scaled));
    }

    #[test]
    fn test_point_transformation() {
        let point = Vec3::new(1.0, 0.0, 0.0);
        let translation_matrix = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));

        let transformed = transform_point(point, &translation_matrix);
        assert_relative_eq!(transformed, Vec3::new(2.0, 2.0, 3.0), epsilon = 1e-6);
    }

    #[test]
    fn test_ray_sphere_intersection() {
        let ray_origin = Vec3::new(0.0, 0.0, -5.0);
        let ray_direction = Vec3::new(0.0, 0.0, 1.0);
        let sphere_center = Vec3::ZERO;
        let sphere_radius = 1.0;

        let intersection =
            ray_sphere_intersection(ray_origin, ray_direction, sphere_center, sphere_radius);
        assert!(intersection.is_some());
        assert_relative_eq!(intersection.unwrap(), 4.0, epsilon = 1e-6);
    }
}

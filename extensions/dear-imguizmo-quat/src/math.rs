use crate::types::{QuatLike, Vec3Like};
use dear_imguizmo_quat_sys as sys;

/// Write quaternion extracted from a 4x4 column-major matrix into `out`.
///
/// Example
/// ```no_run
/// use dear_imguizmo_quat::quat_from_mat4_to;
/// let m = [1.0f32; 16];
/// let mut q = [0.0f32; 4];
/// quat_from_mat4_to(&m, &mut q);
/// ```
pub fn quat_from_mat4_to<Q: QuatLike>(mat: &[f32; 16], out: &mut Q) {
    let mut sq = sys::quat {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };
    // The C API takes a mutable pointer but does not need to mutate the input matrix.
    // Copy into a local buffer to avoid casting away constness (and stay sound if it ever writes).
    let mut mat_copy = *mat;
    unsafe { sys::quat_cast(mat_copy.as_mut_ptr(), &mut sq as *mut _) };
    out.set_from_xyzw([sq.x, sq.y, sq.z, sq.w]);
}

/// Write quaternion and position extracted from a 4x4 column-major matrix into `out_q` and `out_pos`.
///
/// Example
/// ```no_run
/// use dear_imguizmo_quat::quat_pos_from_mat4_to;
/// let m = [1.0f32; 16];
/// let mut q = [0.0f32; 4];
/// let mut p = [0.0f32; 3];
/// quat_pos_from_mat4_to(&m, &mut q, &mut p);
/// ```
pub fn quat_pos_from_mat4_to<Q: QuatLike, V3: Vec3Like>(
    mat: &[f32; 16],
    out_q: &mut Q,
    out_pos: &mut V3,
) {
    let mut sq = sys::quat {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };
    let mut sp = sys::vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    let mut mat_copy = *mat;
    unsafe { sys::quat_pos_cast(mat_copy.as_mut_ptr(), &mut sq as *mut _, &mut sp as *mut _) };
    out_q.set_from_xyzw([sq.x, sq.y, sq.z, sq.w]);
    out_pos.set_from_array([sp.x, sp.y, sp.z]);
}

use dear_imguizmo_sys as sys;

/// A world-space picking ray computed from a mouse position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MouseRay {
    /// World-space point on the camera near plane.
    pub origin: [f32; 3],
    /// Normalized world-space direction.
    pub direction: [f32; 3],
}

/// Trait to abstract over 4x4 column-major matrices used by ImGuizmo.
pub trait Mat4Like: Sized {
    fn to_cols_array(&self) -> [f32; 16];
    fn set_from_cols_array(&mut self, arr: [f32; 16]);
    fn identity() -> Self;
    fn from_cols_array(arr: [f32; 16]) -> Self {
        let mut out = Self::identity();
        out.set_from_cols_array(arr);
        out
    }
}

impl Mat4Like for [f32; 16] {
    fn to_cols_array(&self) -> [f32; 16] {
        *self
    }
    fn set_from_cols_array(&mut self, arr: [f32; 16]) {
        *self = arr;
    }
    fn identity() -> Self {
        [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ]
    }
}

#[cfg(feature = "glam")]
impl Mat4Like for glam::Mat4 {
    fn to_cols_array(&self) -> [f32; 16] {
        self.to_cols_array()
    }
    fn set_from_cols_array(&mut self, arr: [f32; 16]) {
        *self = glam::Mat4::from_cols_array(&arr);
    }
    fn identity() -> Self {
        glam::Mat4::IDENTITY
    }
}

#[cfg(feature = "mint")]
impl Mat4Like for mint::ColumnMatrix4<f32> {
    fn to_cols_array(&self) -> [f32; 16] {
        [
            self.x.x, self.x.y, self.x.z, self.x.w, self.y.x, self.y.y, self.y.z, self.y.w,
            self.z.x, self.z.y, self.z.z, self.z.w, self.w.x, self.w.y, self.w.z, self.w.w,
        ]
    }
    fn set_from_cols_array(&mut self, arr: [f32; 16]) {
        self.x.x = arr[0];
        self.x.y = arr[1];
        self.x.z = arr[2];
        self.x.w = arr[3];
        self.y.x = arr[4];
        self.y.y = arr[5];
        self.y.z = arr[6];
        self.y.w = arr[7];
        self.z.x = arr[8];
        self.z.y = arr[9];
        self.z.z = arr[10];
        self.z.w = arr[11];
        self.w.x = arr[12];
        self.w.y = arr[13];
        self.w.z = arr[14];
        self.w.w = arr[15];
    }
    fn identity() -> Self {
        mint::ColumnMatrix4 {
            x: mint::Vector4 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            },
            y: mint::Vector4 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
                w: 0.0,
            },
            z: mint::Vector4 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
                w: 0.0,
            },
            w: mint::Vector4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
        }
    }
}

// Matrix utilities (Decompose/Recompose) mirroring ImGuizmo helpers
pub fn decompose_matrix<T: Mat4Like>(mat: &T) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let mut arr = mat.to_cols_array();
    let mut tr = [0.0f32; 3];
    let mut rt = [0.0f32; 3];
    let mut sc = [1.0f32; 3];
    unsafe {
        sys::ImGuizmo_DecomposeMatrixToComponents(
            arr.as_mut_ptr(),
            tr.as_mut_ptr(),
            rt.as_mut_ptr(),
            sc.as_mut_ptr(),
        );
    }
    (tr, rt, sc)
}

pub fn recompose_matrix<T: Mat4Like>(
    translation: &[f32; 3],
    rotation: &[f32; 3],
    scale: &[f32; 3],
) -> T {
    let mut out = [0.0f32; 16];
    let mut tr = *translation;
    let mut rt = *rotation;
    let mut sc = *scale;
    unsafe {
        sys::ImGuizmo_RecomposeMatrixFromComponents(
            tr.as_mut_ptr(),
            rt.as_mut_ptr(),
            sc.as_mut_ptr(),
            out.as_mut_ptr(),
        );
    }
    T::from_cols_array(out)
}

/// Compute a world-space picking ray without reading ImGui frame state.
///
/// Returns `None` when the rectangle is empty, an input is not finite, or the
/// matrices cannot produce a finite ray.
#[must_use]
pub fn compute_mouse_ray<V: Mat4Like, P: Mat4Like>(
    view: &V,
    projection: &P,
    mouse_position: impl Into<[f32; 2]>,
    rect_position: impl Into<[f32; 2]>,
    rect_size: impl Into<[f32; 2]>,
) -> Option<MouseRay> {
    let mouse_position = mouse_position.into();
    let rect_position = rect_position.into();
    let rect_size = rect_size.into();
    let inputs_are_finite = mouse_position
        .iter()
        .chain(&rect_position)
        .chain(&rect_size)
        .all(|value| value.is_finite());
    if !inputs_are_finite || rect_size[0] <= 0.0 || rect_size[1] <= 0.0 {
        return None;
    }

    let view = view.to_cols_array();
    let projection = projection.to_cols_array();
    let mut origin = [0.0; 3];
    let mut direction = [0.0; 3];
    unsafe {
        sys::ImGuizmo_ComputeMouseRay(
            view.as_ptr(),
            projection.as_ptr(),
            sys::ImVec2_c {
                x: mouse_position[0],
                y: mouse_position[1],
            },
            sys::ImVec2_c {
                x: rect_position[0],
                y: rect_position[1],
            },
            sys::ImVec2_c {
                x: rect_size[0],
                y: rect_size[1],
            },
            origin.as_mut_ptr(),
            direction.as_mut_ptr(),
        );
    }

    origin
        .iter()
        .chain(&direction)
        .all(|value| value.is_finite())
        .then_some(MouseRay { origin, direction })
}

#[cfg(test)]
mod tests {
    use super::{MouseRay, compute_mouse_ray};

    #[test]
    fn computes_center_ray_from_identity_matrices() {
        let identity = <[f32; 16] as super::Mat4Like>::identity();
        let ray = compute_mouse_ray(
            &identity,
            &identity,
            [60.0, 45.0],
            [10.0, 20.0],
            [100.0, 50.0],
        );

        let MouseRay { origin, direction } = ray.expect("identity matrices produce a ray");
        for (actual, expected) in origin.into_iter().zip([0.0, 0.0, 0.0]) {
            assert!((actual - expected).abs() < 1.0e-6);
        }
        for (actual, expected) in direction.into_iter().zip([0.0, 0.0, 1.0]) {
            assert!((actual - expected).abs() < 1.0e-6);
        }
    }

    #[test]
    fn rejects_empty_or_non_finite_rectangles() {
        let identity = <[f32; 16] as super::Mat4Like>::identity();

        assert_eq!(
            compute_mouse_ray(&identity, &identity, [0.0, 0.0], [0.0, 0.0], [0.0, 1.0]),
            None
        );
        assert_eq!(
            compute_mouse_ray(
                &identity,
                &identity,
                [0.0, 0.0],
                [0.0, 0.0],
                [f32::NAN, 1.0],
            ),
            None
        );
    }
}

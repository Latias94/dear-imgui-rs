use dear_imguizmo_sys as sys;

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
    fn to_cols_array(&self) -> [f32; 16] { *self }
    fn set_from_cols_array(&mut self, arr: [f32; 16]) { *self = arr; }
    fn identity() -> Self {
        [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ]
    }
}

#[cfg(feature = "glam")]
impl Mat4Like for glam::Mat4 {
    fn to_cols_array(&self) -> [f32; 16] { self.to_cols_array() }
    fn set_from_cols_array(&mut self, arr: [f32; 16]) { *self = glam::Mat4::from_cols_array(&arr); }
    fn identity() -> Self { glam::Mat4::IDENTITY }
}

#[cfg(feature = "mint")]
impl Mat4Like for mint::ColumnMatrix4<f32> {
    fn to_cols_array(&self) -> [f32; 16] {
        [
            self.x.x, self.x.y, self.x.z, self.x.w,
            self.y.x, self.y.y, self.y.z, self.y.w,
            self.z.x, self.z.y, self.z.z, self.z.w,
            self.w.x, self.w.y, self.w.z, self.w.w,
        ]
    }
    fn set_from_cols_array(&mut self, arr: [f32; 16]) {
        self.x.x = arr[0];  self.x.y = arr[1];  self.x.z = arr[2];  self.x.w = arr[3];
        self.y.x = arr[4];  self.y.y = arr[5];  self.y.z = arr[6];  self.y.w = arr[7];
        self.z.x = arr[8];  self.z.y = arr[9];  self.z.z = arr[10]; self.z.w = arr[11];
        self.w.x = arr[12]; self.w.y = arr[13]; self.w.z = arr[14]; self.w.w = arr[15];
    }
    fn identity() -> Self {
        mint::ColumnMatrix4 {
            x: mint::Vector4 { x: 1.0, y: 0.0, z: 0.0, w: 0.0 },
            y: mint::Vector4 { x: 0.0, y: 1.0, z: 0.0, w: 0.0 },
            z: mint::Vector4 { x: 0.0, y: 0.0, z: 1.0, w: 0.0 },
            w: mint::Vector4 { x: 0.0, y: 0.0, z: 0.0, w: 1.0 },
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


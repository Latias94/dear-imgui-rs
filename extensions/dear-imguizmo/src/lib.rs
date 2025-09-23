//! High-level safe helpers for ImGuizmo integrated with dear-imgui

use dear_imgui::Ui;
use dear_imgui_sys as imgui_sys;
use dear_imguizmo_sys as sys;
use thiserror::Error;

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

bitflags::bitflags! {
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Operation: u32 {
        const TRANSLATE_X = 1 << 0;
        const TRANSLATE_Y = 1 << 1;
        const TRANSLATE_Z = 1 << 2;
        const ROTATE_X    = 1 << 3;
        const ROTATE_Y    = 1 << 4;
        const ROTATE_Z    = 1 << 5;
        const ROTATE_SCREEN = 1 << 6;
        const SCALE_X     = 1 << 7;
        const SCALE_Y     = 1 << 8;
        const SCALE_Z     = 1 << 9;
        const BOUNDS      = 1 << 10;
        const SCALE_UNIFORM_X = 1 << 11;
        const SCALE_UNIFORM_Y = 1 << 12;
        const SCALE_UNIFORM_Z = 1 << 13;

        const TRANSLATE = Self::TRANSLATE_X.bits() | Self::TRANSLATE_Y.bits() | Self::TRANSLATE_Z.bits();
        const ROTATE    = Self::ROTATE_X.bits() | Self::ROTATE_Y.bits() | Self::ROTATE_Z.bits() | Self::ROTATE_SCREEN.bits();
        const SCALE     = Self::SCALE_X.bits() | Self::SCALE_Y.bits() | Self::SCALE_Z.bits();
        const SCALE_UNIFORM = Self::SCALE_UNIFORM_X.bits() | Self::SCALE_UNIFORM_Y.bits() | Self::SCALE_UNIFORM_Z.bits();
        const UNIVERSAL = Self::TRANSLATE.bits() | Self::ROTATE.bits() | Self::SCALE_UNIFORM.bits();
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Local,
    World,
}

fn op_to_sys(flags: Operation) -> sys::OPERATION {
    flags.bits() as sys::OPERATION
}
fn mode_to_sys(m: Mode) -> sys::MODE {
    match m {
        Mode::Local => 0 as sys::MODE,
        Mode::World => 1 as sys::MODE,
    }
}

#[derive(Debug, Error)]
pub enum GizmoError {
    #[error("local_bounds must have length 6 (min/max per axis)")]
    InvalidLocalBounds,
}

/// Context handle; lightweight wrapper to bind ImGui context and build a GizmoUi
#[derive(Default, Clone, Copy)]
pub struct GuizmoContext;

impl GuizmoContext {
    pub fn new() -> Self {
        Self
    }

    /// Begin an ImGuizmo frame for the given ImGui `Ui`.
    /// Call exactly once per frame before using GizmoUi functions.
    pub fn begin_frame<'ui>(&self, _ui: &'ui Ui) -> GizmoUi<'ui> {
        unsafe {
            sys::ImGuizmo_SetImGuiContext(imgui_sys::igGetCurrentContext());
            sys::ImGuizmo_BeginFrame();
        }
        GizmoUi { _ui }
    }
}

pub struct GizmoUi<'ui> {
    _ui: &'ui Ui,
}

impl<'ui> GizmoUi<'ui> {
    // ID helpers to match ImGuizmo ID stack usage in demos
    pub fn set_id(&self, id: i32) {
        unsafe { sys::ImGuizmo_SetID(id) }
    }
    pub fn push_id_int(&self, id: i32) {
        unsafe { sys::ImGuizmo_PushID_Int(id) }
    }
    pub fn pop_id(&self) {
        unsafe { sys::ImGuizmo_PopID() }
    }
    pub fn set_rect(&self, x: f32, y: f32, width: f32, height: f32) {
        unsafe { sys::ImGuizmo_SetRect(x, y, width, height) }
    }
    pub fn set_orthographic(&self, is_ortho: bool) {
        unsafe { sys::ImGuizmo_SetOrthographic(is_ortho) }
    }
    pub fn allow_axis_flip(&self, enable: bool) {
        unsafe { sys::ImGuizmo_AllowAxisFlip(enable) }
    }
    pub fn set_gizmo_size_clip_space(&self, value: f32) {
        unsafe { sys::ImGuizmo_SetGizmoSizeClipSpace(value) }
    }
    pub fn set_axis_limit(&self, value: f32) {
        unsafe { sys::ImGuizmo_SetAxisLimit(value) }
    }
    pub fn set_plane_limit(&self, value: f32) {
        unsafe { sys::ImGuizmo_SetPlaneLimit(value) }
    }

    pub fn draw_grid<T: Mat4Like>(&self, view: &T, projection: &T, model: &T, grid_size: f32) {
        unsafe {
            sys::ImGuizmo_DrawGrid(
                view.to_cols_array().as_ptr(),
                projection.to_cols_array().as_ptr(),
                model.to_cols_array().as_ptr(),
                grid_size,
            )
        }
    }

    pub fn draw_cubes<T: Mat4Like>(&self, view: &T, projection: &T, matrices: &[T]) {
        let count = matrices.len() as i32;
        if count == 0 {
            return;
        }
        let mut flat: Vec<f32> = Vec::with_capacity((count as usize) * 16);
        for m in matrices {
            flat.extend_from_slice(&m.to_cols_array());
        }
        unsafe {
            sys::ImGuizmo_DrawCubes(
                view.to_cols_array().as_ptr(),
                projection.to_cols_array().as_ptr(),
                flat.as_ptr(),
                count,
            )
        }
    }

    pub fn set_drawlist_window(&self) {
        unsafe { sys::ImGuizmo_SetDrawlist(imgui_sys::igGetWindowDrawList()) }
    }
    pub fn set_drawlist_background(&self) {
        unsafe {
            sys::ImGuizmo_SetDrawlist(imgui_sys::igGetBackgroundDrawList(std::ptr::null_mut()))
        }
    }
    pub fn set_drawlist_foreground(&self) {
        unsafe {
            sys::ImGuizmo_SetDrawlist(imgui_sys::igGetForegroundDrawList_ViewportPtr(
                std::ptr::null_mut(),
            ))
        }
    }

    /// Manipulate using the currently bound draw list.
    /// Call one of `set_drawlist_window/background/foreground` before invoking.
    pub fn manipulate<T: Mat4Like>(
        &self,
        view: &T,
        projection: &T,
        operation: Operation,
        mode: Mode,
        model_matrix: &mut T,
        mut delta_matrix: Option<&mut T>,
        snap: Option<&[f32; 3]>,
        local_bounds: Option<&[f32; 6]>,
        bounds_snap: Option<&[f32; 3]>,
    ) -> Result<bool, GizmoError> {
        if let Some(lb) = local_bounds {
            if lb.len() != 6 {
                return Err(GizmoError::InvalidLocalBounds);
            }
        }
        let mut model_arr = model_matrix.to_cols_array();
        let mut delta_arr = match &delta_matrix {
            Some(dm) => dm.to_cols_array(),
            None => T::identity().to_cols_array(),
        };
        let used = unsafe {
            sys::ImGuizmo_Manipulate(
                view.to_cols_array().as_ptr(),
                projection.to_cols_array().as_ptr(),
                op_to_sys(operation),
                mode_to_sys(mode),
                model_arr.as_mut_ptr(),
                delta_arr.as_mut_ptr(),
                snap.map(|s| s.as_ptr()).unwrap_or(std::ptr::null()),
                local_bounds.map(|b| b.as_ptr()).unwrap_or(std::ptr::null()),
                bounds_snap.map(|b| b.as_ptr()).unwrap_or(std::ptr::null()),
            )
        };
        model_matrix.set_from_cols_array(model_arr);
        if let Some(dm) = &mut delta_matrix {
            dm.set_from_cols_array(delta_arr);
        }
        Ok(used)
    }

    pub fn view_manipulate<T: Mat4Like>(
        &self,
        view: &mut T,
        length: f32,
        position: [f32; 2],
        size: [f32; 2],
        background_color: u32,
    ) -> bool {
        let mut arr = view.to_cols_array();
        unsafe {
            sys::ImGuizmo_ViewManipulate_Float(
                arr.as_mut_ptr(),
                length,
                sys::ImVec2 {
                    x: position[0],
                    y: position[1],
                },
                sys::ImVec2 {
                    x: size[0],
                    y: size[1],
                },
                background_color,
            );
        }
        view.set_from_cols_array(arr);
        unsafe { sys::ImGuizmo_IsUsingViewManipulate() }
    }

    pub fn enable(&self, enable: bool) {
        unsafe { sys::ImGuizmo_Enable(enable) }
    }
    pub fn is_over(&self) -> bool {
        unsafe { sys::ImGuizmo_IsOver_Nil() }
    }
    pub fn is_using(&self) -> bool {
        unsafe { sys::ImGuizmo_IsUsing() }
    }
    pub fn is_over_operation(&self, operation: Operation) -> bool {
        unsafe { sys::ImGuizmo_IsOver_OPERATION(op_to_sys(operation)) }
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

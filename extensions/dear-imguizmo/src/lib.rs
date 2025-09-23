//! High-level safe helpers for ImGuizmo integrated with dear-imgui

use dear_imgui::Ui;
use dear_imgui_sys as imgui_sys;
use dear_imguizmo_sys as sys;
use glam::Mat4;
use thiserror::Error;

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

    pub fn get_ui<'ui>(&self, _ui: &'ui Ui) -> GizmoUi<'ui> {
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

    pub fn draw_grid(&self, view: &Mat4, projection: &Mat4, model: &Mat4, grid_size: f32) {
        unsafe {
            sys::ImGuizmo_DrawGrid(
                view.to_cols_array().as_ptr(),
                projection.to_cols_array().as_ptr(),
                model.to_cols_array().as_ptr(),
                grid_size,
            )
        }
    }

    pub fn draw_cubes(&self, view: &Mat4, projection: &Mat4, matrices: &[Mat4]) {
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

    pub fn manipulate_with_options(
        &self,
        _draw_list: &dear_imgui::DrawListMut<'_>,
        view: &Mat4,
        projection: &Mat4,
        operation: Operation,
        mode: Mode,
        model_matrix: &mut Mat4,
        mut delta_matrix: Option<&mut Mat4>,
        snap: Option<&[f32; 3]>,
        local_bounds: Option<&[f32; 6]>,
        bounds_snap: Option<&[f32; 3]>,
    ) -> Result<bool, GizmoError> {
        // Bind current window drawlist (cannot get raw from DrawListMut; use ig API)
        self.set_drawlist_window();
        if let Some(lb) = local_bounds {
            if lb.len() != 6 {
                return Err(GizmoError::InvalidLocalBounds);
            }
        }
        // Prepare mutable arrays for model and optional delta outputs
        let mut model_arr = model_matrix.to_cols_array();
        let mut delta_arr = match &delta_matrix {
            Some(dm) => dm.to_cols_array(),
            None => Mat4::IDENTITY.to_cols_array(),
        };
        let delta_ptr = delta_arr.as_mut_ptr();
        let snap_ptr = snap.map(|s| s.as_ptr()).unwrap_or(std::ptr::null());
        let lb_ptr = local_bounds.map(|b| b.as_ptr()).unwrap_or(std::ptr::null());
        let bs_ptr = bounds_snap.map(|b| b.as_ptr()).unwrap_or(std::ptr::null());
        let used = unsafe {
            sys::ImGuizmo_Manipulate(
                view.to_cols_array().as_ptr(),
                projection.to_cols_array().as_ptr(),
                op_to_sys(operation),
                mode_to_sys(mode),
                model_arr.as_mut_ptr(),
                delta_ptr,
                snap_ptr,
                lb_ptr,
                bs_ptr,
            )
        };
        // Write back results
        *model_matrix = Mat4::from_cols_array(&model_arr);
        if let Some(dm) = &mut delta_matrix {
            **dm = Mat4::from_cols_array(&delta_arr);
        }
        Ok(used)
    }

    pub fn view_manipulate(
        &self,
        view: &mut Mat4,
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
        *view = Mat4::from_cols_array(&arr);
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

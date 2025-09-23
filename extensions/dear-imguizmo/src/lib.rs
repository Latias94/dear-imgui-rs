//! High-level safe helpers for ImGuizmo integrated with dear-imgui

use dear_imguizmo_sys as sys;
use dear_imgui::{self as imgui, Ui};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Operation {
    Translate,
    Rotate,
    Scale,
    Universal,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Local,
    World,
}

impl From<Operation> for sys::OPERATION {
    fn from(op: Operation) -> Self {
        match op {
            Operation::Translate => sys::OPERATION_TRANSLATE,
            Operation::Rotate => sys::OPERATION_ROTATE,
            Operation::Scale => sys::OPERATION_SCALE,
            Operation::Universal => sys::OPERATION_UNIVERSAL,
        }
    }
}

impl From<Mode> for sys::MODE {
    fn from(m: Mode) -> Self {
        match m {
            Mode::Local => sys::MODE_LOCAL,
            Mode::World => sys::MODE_WORLD,
        }
    }
}

/// Call per-frame to initialize ImGuizmo state
pub fn begin_frame(_ui: &Ui) {
    unsafe {
        sys::ImGuizmo_BeginFrame();
    }
}

/// Set drawing rectangle in screen coordinates
pub fn set_rect(x: f32, y: f32, width: f32, height: f32) {
    unsafe { sys::ImGuizmo_SetRect(x, y, width, height) }
}

/// Set orthographic projection flag
pub fn set_orthographic(is_ortho: bool) {
    unsafe { sys::ImGuizmo_SetOrthographic(is_ortho) }
}

/// Perform a manipulation on the given model matrix.
/// Returns true if the gizmo is being used.
pub fn manipulate(
    view: &[f32; 16],
    projection: &[f32; 16],
    operation: Operation,
    mode: Mode,
    model_matrix: &mut [f32; 16],
    delta_matrix: Option<&mut [f32; 16]>,
    snap: Option<&[f32; 3]>,
) -> bool {
    let mut delta_tmp: [f32; 16];
    let delta_ptr = if let Some(delta) = delta_matrix { delta.as_mut_ptr() } else {
        delta_tmp = [0.0; 16];
        std::ptr::null_mut()
    };
    let snap_ptr = snap.map(|s| s.as_ptr()).unwrap_or(std::ptr::null());
    unsafe {
        sys::ImGuizmo_Manipulate(
            view.as_ptr(),
            projection.as_ptr(),
            operation.into(),
            mode.into(),
            model_matrix.as_mut_ptr(),
            delta_ptr,
            snap_ptr,
            std::ptr::null(),
            std::ptr::null(),
        )
    }
}

/// Convenience: set draw list to current window's draw list
pub fn set_drawlist(ui: &Ui) {
    unsafe {
        // Safety: ImGui current window draw list is valid during UI building
        let draw_list = ui.get_window_draw_list();
        sys::ImGuizmo_SetDrawlist(draw_list.raw());
    }
}

/// Enable or disable ImGuizmo globally
pub fn enable(enable: bool) {
    unsafe { sys::ImGuizmo_Enable(enable) }
}

/// Query whether the gizmo is hovered/used
pub fn is_over() -> bool { unsafe { sys::ImGuizmo_IsOver_Nil() } }
pub fn is_using() -> bool { unsafe { sys::ImGuizmo_IsUsing() } }
pub fn is_using_any() -> bool { unsafe { sys::ImGuizmo_IsUsingAny() } }

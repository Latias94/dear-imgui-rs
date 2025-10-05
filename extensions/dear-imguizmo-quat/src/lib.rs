//! Safe helpers for ImGuIZMO.quat integrated with dear-imgui.
//!
//! Quick start (arrays):
//! ```no_run
//! # fn demo(ui: &dear_imgui_rs::Ui) {
//! use dear_imguizmo_quat::{GizmoQuatExt, Mode};
//! let mut q = [0.0, 0.0, 0.0, 1.0];
//! let used = ui.gizmo_quat().builder().mode(Mode::MODE_DUAL).quat("##q", &mut q);
//! # let _ = used; }
//! ```

mod math;
mod types;
mod ui;

pub use math::{quat_from_mat4_to, quat_pos_from_mat4_to};
pub use types::{Mode, Modifiers, QuatLike, Vec3Like, Vec4Like};
pub use ui::{GizmoQuatBuilder, GizmoQuatExt, GizmoQuatUi};

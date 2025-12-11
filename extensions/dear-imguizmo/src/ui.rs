use dear_imgui_rs::Ui;
use dear_imgui_sys as imgui_sys;
use dear_imguizmo_sys as sys;
use std::ffi::CString;

use crate::mat::Mat4Like;
use crate::style::Style;
use crate::types::{AxisMask, DrawListTarget, GuizmoId, Mode, Operation, Vec3Like};

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
    pub(crate) _ui: &'ui Ui,
}

impl<'ui> GizmoUi<'ui> {
    // ID helpers to match ImGuizmo ID stack usage in demos
    pub fn set_id(&self, id: i32) {
        unsafe { sys::ImGuizmo_SetID(id) }
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
    pub fn set_axis_mask(&self, mask: AxisMask) {
        let x = mask.contains(AxisMask::X);
        let y = mask.contains(AxisMask::Y);
        let z = mask.contains(AxisMask::Z);
        unsafe { sys::ImGuizmo_SetAxisMask(x, y, z) }
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
    pub fn set_drawlist(&self, target: DrawListTarget) {
        match target {
            DrawListTarget::Window => self.set_drawlist_window(),
            DrawListTarget::Background => self.set_drawlist_background(),
            DrawListTarget::Foreground => self.set_drawlist_foreground(),
        }
    }

    /// Unsafe: set a raw ImGui drawlist pointer for ImGuizmo to render into.
    ///
    /// Prefer using `set_drawlist_*` safe variants. Only use this when you
    /// have a valid `*mut ImDrawList` whose lifetime is at least the duration
    /// of the current frame.
    pub unsafe fn set_drawlist_raw(&self, drawlist: *mut imgui_sys::ImDrawList) {
        sys::ImGuizmo_SetDrawlist(drawlist)
    }

    /// Manipulate using the currently bound draw list.
    /// Call one of `set_drawlist_*` before invoking.
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
    ) -> bool {
        let mut model_arr = model_matrix.to_cols_array();
        let mut delta_arr = match &delta_matrix {
            Some(dm) => dm.to_cols_array(),
            None => T::identity().to_cols_array(),
        };
        let used = unsafe {
            sys::ImGuizmo_Manipulate(
                view.to_cols_array().as_ptr(),
                projection.to_cols_array().as_ptr(),
                operation.into(),
                mode.into(),
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
        used
    }

    pub fn view_manipulate<T: Mat4Like>(
        &self,
        view: &mut T,
        length: f32,
        position: impl Into<[f32; 2]>,
        size: impl Into<[f32; 2]>,
        background_color: u32,
    ) -> bool {
        let mut arr = view.to_cols_array();
        let position = position.into();
        let size = size.into();
        unsafe {
            sys::ImGuizmo_ViewManipulate_Float(
                arr.as_mut_ptr(),
                length,
                sys::ImVec2_c {
                    x: position[0],
                    y: position[1],
                },
                sys::ImVec2_c {
                    x: size[0],
                    y: size[1],
                },
                background_color,
            );
        }
        view.set_from_cols_array(arr);
        unsafe { sys::ImGuizmo_IsUsingViewManipulate() }
    }

    /// Extended view manipulator that also takes projection and edits a target matrix.
    pub fn view_manipulate_with_camera<T: Mat4Like>(
        &self,
        view: &mut T,
        projection: &T,
        operation: Operation,
        mode: Mode,
        matrix: &mut T,
        length: f32,
        position: impl Into<[f32; 2]>,
        size: impl Into<[f32; 2]>,
        background_color: u32,
    ) -> bool {
        let mut view_arr = view.to_cols_array();
        let mut matrix_arr = matrix.to_cols_array();
        let position = position.into();
        let size = size.into();
        unsafe {
            sys::ImGuizmo_ViewManipulate_FloatPtr(
                view_arr.as_mut_ptr(),
                projection.to_cols_array().as_ptr(),
                operation.into(),
                mode.into(),
                matrix_arr.as_mut_ptr(),
                length,
                sys::ImVec2_c {
                    x: position[0],
                    y: position[1],
                },
                sys::ImVec2_c {
                    x: size[0],
                    y: size[1],
                },
                background_color,
            );
        }
        view.set_from_cols_array(view_arr);
        matrix.set_from_cols_array(matrix_arr);
        unsafe { sys::ImGuizmo_IsUsingViewManipulate() }
    }

    /// Convenience: set rect from pos/size vectors
    pub fn set_rect_pos_size(&self, pos: impl Into<[f32; 2]>, size: impl Into<[f32; 2]>) {
        let pos = pos.into();
        let size = size.into();
        self.set_rect(pos[0], pos[1], size[0], size[1]);
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
    pub fn is_using_any(&self) -> bool {
        unsafe { sys::ImGuizmo_IsUsingAny() }
    }
    pub fn is_using_view_manipulate(&self) -> bool {
        unsafe { sys::ImGuizmo_IsUsingViewManipulate() }
    }
    pub fn is_view_manipulate_hovered(&self) -> bool {
        unsafe { sys::ImGuizmo_IsViewManipulateHovered() }
    }
    pub fn is_over_operation(&self, operation: Operation) -> bool {
        unsafe { sys::ImGuizmo_IsOver_OPERATION(operation.into()) }
    }

    /// Test if the mouse is within `pixel_radius` of a world position once
    /// ImGuizmo has a valid view/projection context for the current frame.
    ///
    /// Notes:
    /// - This uses ImGuizmo internal `ViewProjection` from the last compute
    ///   (e.g. after calling `manipulate`, `draw_grid`, etc.).
    /// - `position` is a world-space 3D point.
    pub fn is_over_at<V: Vec3Like>(&self, position: V, pixel_radius: f32) -> bool {
        let mut p = position.to_array();
        unsafe { sys::ImGuizmo_IsOver_FloatPtr(p.as_mut_ptr(), pixel_radius) }
    }

    /// Push an ID for ImGuizmo's own ID stack and return a guard that pops on drop.
    pub fn push_id<'a, I>(&self, id: I) -> IdToken<'ui>
    where
        I: Into<GuizmoId<'a>>,
    {
        let id: GuizmoId<'a> = id.into();
        unsafe {
            match id {
                GuizmoId::Int(i) => sys::ImGuizmo_PushID_Int(i),
                GuizmoId::Str(s) => {
                    let c = CString::new(s).expect("string contained NUL");
                    sys::ImGuizmo_PushID_Str(c.as_ptr())
                }
                GuizmoId::Ptr(p) => sys::ImGuizmo_PushID_Ptr(p),
            }
        }
        IdToken { _ui: self._ui }
    }
    /// Convenience for string ID push without needing to keep the guard name verbose.
    pub fn push_id_str(&self, id: &str) -> IdToken<'ui> {
        self.push_id(GuizmoId::Str(id))
    }
    /// Obtain a hashed ID value following ImGuizmo's ID scheme.
    pub fn get_id_str(&self, id: &str) -> imgui_sys::ImGuiID {
        let c = CString::new(id).expect("string contained NUL");
        unsafe { sys::ImGuizmo_GetID_Str(c.as_ptr()) }
    }

    /// Obtain a hashed ID from a pointer following ImGuizmo's ID scheme.
    pub fn get_id_ptr<T>(&self, ptr: *const T) -> imgui_sys::ImGuiID {
        unsafe { sys::ImGuizmo_GetID_Ptr(ptr as *const std::ffi::c_void) }
    }

    /// Access ImGuizmo global style through a safe wrapper bound to this UI lifetime.
    pub fn style(&self) -> Style<'ui> {
        let ptr = unsafe { sys::ImGuizmo_GetStyle() };
        Style {
            ptr,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Start a builder-style manipulation call
    pub fn manipulate_config<T: Mat4Like>(
        &'ui self,
        view: &'ui T,
        projection: &'ui T,
        model: &'ui mut T,
    ) -> crate::op::Manipulate<'ui, T> {
        crate::op::Manipulate::new(self, view, projection, model)
    }
}

/// RAII token that pops an ImGuizmo ID when dropped.
pub struct IdToken<'ui> {
    pub(crate) _ui: &'ui Ui,
}
impl<'ui> Drop for IdToken<'ui> {
    fn drop(&mut self) {
        unsafe { sys::ImGuizmo_PopID() }
    }
}

/// Extension methods on dear-imgui's Ui to access ImGuizmo in a unified way
pub trait GuizmoExt {
    fn guizmo(&self) -> GizmoUi<'_>;
}
impl GuizmoExt for Ui {
    fn guizmo(&self) -> GizmoUi<'_> {
        GuizmoContext::new().begin_frame(self)
    }
}

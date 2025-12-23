use dear_imgui_rs::Ui;
use dear_imguizmo_quat_sys as sys;
use std::os::raw::c_char;

use crate::types::{
    Mode, Modifiers, QuatLike, Vec3Like, Vec4Like, from_sys_quat, from_sys_vec3, from_sys_vec4,
    modifiers_to_sys, to_sys_quat, to_sys_vec3, to_sys_vec4,
};

fn with_label_ptr<R>(label: &str, f: impl FnOnce(*const c_char) -> R) -> R {
    assert!(!label.contains('\0'), "label contained NUL");
    dear_imgui_rs::with_scratch_txt(label, f)
}

/// Lightweight handle to call ImGuIZMO.quat functions within a Ui frame
#[derive(Clone, Copy)]
pub struct GizmoQuatUi<'ui> {
    pub(crate) _ui: &'ui Ui,
}

/// Extension methods on dear-imgui's Ui to access ImGuIZMO.quat
pub trait GizmoQuatExt {
    fn gizmo_quat(&self) -> GizmoQuatUi<'_>;
}
impl GizmoQuatExt for Ui {
    fn gizmo_quat(&self) -> GizmoQuatUi<'_> {
        GizmoQuatUi { _ui: self }
    }
}

/// Builder for `gizmo3D` calls with size/mode configured ergonomically.
///
/// Examples
///
/// Basic quaternion gizmo (arrays):
/// ```no_run
/// # fn demo(ui: &dear_imgui_rs::Ui) {
/// use dear_imguizmo_quat::{GizmoQuatExt, Mode};
/// let mut q = [0.0, 0.0, 0.0, 1.0];
/// let used = ui
///     .gizmo_quat()
///     .builder()
///     .size(220.0)
///     .mode(Mode::MODE_DUAL | Mode::CUBE_AT_ORIGIN)
///     .quat("##rot", &mut q);
/// # let _ = used; }
/// ```
///
/// Pan+Dolly + quaternion + light direction (glam):
/// ```no_run
/// # fn demo(ui: &dear_imgui_rs::Ui) {
/// use dear_imguizmo_quat::{GizmoQuatExt, Mode};
/// let mut pan_dolly = glam::Vec3::ZERO;
/// let mut rot = glam::Quat::IDENTITY;
/// let mut light = glam::Vec3::new(1.0, 0.0, 0.0);
/// let used = ui
///     .gizmo_quat()
///     .builder()
///     .mode(Mode::MODE_PAN_DOLLY | Mode::MODE_DUAL | Mode::CUBE_AT_ORIGIN)
///     .pan_dolly_quat_light_vec3("##g", &mut pan_dolly, &mut rot, &mut light);
/// # let _ = used; }
/// ```
pub struct GizmoQuatBuilder<'ui> {
    ui: GizmoQuatUi<'ui>,
    size: f32,
    mode: Mode,
}
impl<'ui> GizmoQuatBuilder<'ui> {
    /// Create a new builder with sensible defaults (size=220.0, mode=Dual|CubeAtOrigin)
    pub fn new(ui: GizmoQuatUi<'ui>) -> Self {
        Self {
            ui,
            size: 220.0,
            mode: Mode::MODE_DUAL | Mode::CUBE_AT_ORIGIN,
        }
    }
    /// Set the widget size in pixels
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
    /// Set the operation mode bitflags
    pub fn mode(mut self, mode: Mode) -> Self {
        self.mode = mode;
        self
    }

    /// Display a quaternion gizmo (axes rotation only)
    pub fn quat<Q: QuatLike>(self, label: &str, q: &mut Q) -> bool {
        self.ui.gizmo3d_quat(label, q, self.size, self.mode)
    }
    /// Display a quaternion gizmo with a second light quaternion
    pub fn quat_light_quat<Q: QuatLike>(self, label: &str, q: &mut Q, light: &mut Q) -> bool {
        self.ui
            .gizmo3d_quat_with_light_quat(label, q, light, self.size, self.mode)
    }
    /// Display a quaternion gizmo with an axis-angle light vec4 (xyz axis, w radians)
    pub fn quat_light_vec4<Q: QuatLike, V4: Vec4Like>(
        self,
        label: &str,
        q: &mut Q,
        v: &mut V4,
    ) -> bool {
        self.ui
            .gizmo3d_quat_with_light_vec4(label, q, v, self.size, self.mode)
    }
    /// Display a quaternion gizmo with a light direction vec3
    pub fn quat_light_vec3<Q: QuatLike, V3: Vec3Like>(
        self,
        label: &str,
        q: &mut Q,
        v: &mut V3,
    ) -> bool {
        self.ui
            .gizmo3d_quat_with_light_vec3(label, q, v, self.size, self.mode)
    }
    /// Display Pan+Dolly vec3 + quaternion gizmo
    pub fn pan_dolly_quat<V3: Vec3Like, Q: QuatLike>(
        self,
        label: &str,
        vm: &mut V3,
        q: &mut Q,
    ) -> bool {
        self.ui
            .gizmo3d_pan_dolly_quat(label, vm, q, self.size, self.mode)
    }
    /// Display Pan+Dolly vec3 + light vec4 gizmo
    pub fn pan_dolly_vec4<V3: Vec3Like, V4: Vec4Like>(
        self,
        label: &str,
        vm: &mut V3,
        v: &mut V4,
    ) -> bool {
        self.ui
            .gizmo3d_pan_dolly_vec4(label, vm, v, self.size, self.mode)
    }
    /// Display Pan+Dolly vec3 + light vec3 gizmo
    pub fn pan_dolly_vec3<V3: Vec3Like>(self, label: &str, vm: &mut V3, v: &mut V3) -> bool {
        self.ui
            .gizmo3d_pan_dolly_vec3(label, vm, v, self.size, self.mode)
    }
    /// Display Pan+Dolly vec3 + quaternion + light quaternion gizmo
    pub fn pan_dolly_quat_light_quat<V3: Vec3Like, Q: QuatLike>(
        self,
        label: &str,
        vm: &mut V3,
        q: &mut Q,
        ql: &mut Q,
    ) -> bool {
        self.ui
            .gizmo3d_pan_dolly_quat_light_quat(label, vm, q, ql, self.size, self.mode)
    }
    /// Display Pan+Dolly vec3 + quaternion + light vec4 gizmo
    pub fn pan_dolly_quat_light_vec4<V3: Vec3Like, Q: QuatLike, V4: Vec4Like>(
        self,
        label: &str,
        vm: &mut V3,
        q: &mut Q,
        v: &mut V4,
    ) -> bool {
        self.ui
            .gizmo3d_pan_dolly_quat_light_vec4(label, vm, q, v, self.size, self.mode)
    }
    /// Display Pan+Dolly vec3 + quaternion + light vec3 gizmo
    pub fn pan_dolly_quat_light_vec3<V3: Vec3Like, Q: QuatLike>(
        self,
        label: &str,
        vm: &mut V3,
        q: &mut Q,
        v: &mut V3,
    ) -> bool {
        self.ui
            .gizmo3d_pan_dolly_quat_light_vec3(label, vm, q, v, self.size, self.mode)
    }
}

impl<'ui> GizmoQuatUi<'ui> {
    /// Start a builder to configure size/mode ergonomically, then choose a gizmo variant.
    ///
    /// Example
    /// ```no_run
    /// # fn demo(ui: &dear_imgui_rs::Ui) {
    /// use dear_imguizmo_quat::{GizmoQuatExt, Mode};
    /// let mut q = [0.0, 0.0, 0.0, 1.0];
    /// let used = ui.gizmo_quat().builder().mode(Mode::MODE_DUAL).quat("##q", &mut q);
    /// # let _ = used; }
    /// ```
    pub fn builder(&self) -> GizmoQuatBuilder<'_> {
        GizmoQuatBuilder::new(*self)
    }

    /// Gizmo with quaternion for axes rotation
    pub fn gizmo3d_quat<Q: QuatLike>(&self, label: &str, q: &mut Q, size: f32, mode: Mode) -> bool {
        let mut sq = to_sys_quat(q);
        let used = with_label_ptr(label, |label_ptr| unsafe {
            sys::iggizmo3D_quatPtrFloat(label_ptr, &mut sq as *mut _, size, mode.bits())
        });
        from_sys_quat(q, sq);
        used
    }

    /// Gizmo with quaternion and light quaternion
    pub fn gizmo3d_quat_with_light_quat<Q: QuatLike>(
        &self,
        label: &str,
        q: &mut Q,
        light: &mut Q,
        size: f32,
        mode: Mode,
    ) -> bool {
        let mut sq = to_sys_quat(q);
        let mut sl = to_sys_quat(light);
        let used = with_label_ptr(label, |label_ptr| unsafe {
            sys::iggizmo3D_quatPtrquatPtr(
                label_ptr,
                &mut sq as *mut _,
                &mut sl as *mut _,
                size,
                mode.bits(),
            )
        });
        from_sys_quat(q, sq);
        from_sys_quat(light, sl);
        used
    }

    /// Gizmo with quaternion and light axis-angle vec4 (xyz axis, w radians)
    pub fn gizmo3d_quat_with_light_vec4<Q: QuatLike, V4: Vec4Like>(
        &self,
        label: &str,
        q: &mut Q,
        v: &mut V4,
        size: f32,
        mode: Mode,
    ) -> bool {
        let mut sq = to_sys_quat(q);
        let mut sv = to_sys_vec4(v);
        let used = with_label_ptr(label, |label_ptr| unsafe {
            sys::iggizmo3D_quatPtrvec4Ptr(
                label_ptr,
                &mut sq as *mut _,
                &mut sv as *mut _,
                size,
                mode.bits(),
            )
        });
        from_sys_quat(q, sq);
        from_sys_vec4(v, sv);
        used
    }

    /// Gizmo with quaternion and light vector3 direction
    pub fn gizmo3d_quat_with_light_vec3<Q: QuatLike, V3: Vec3Like>(
        &self,
        label: &str,
        q: &mut Q,
        v: &mut V3,
        size: f32,
        mode: Mode,
    ) -> bool {
        let mut sq = to_sys_quat(q);
        let mut sv = to_sys_vec3(v);
        let used = with_label_ptr(label, |label_ptr| unsafe {
            sys::iggizmo3D_quatPtrvec3Ptr(
                label_ptr,
                &mut sq as *mut _,
                &mut sv as *mut _,
                size,
                mode.bits(),
            )
        });
        from_sys_quat(q, sq);
        from_sys_vec3(v, sv);
        used
    }

    /// Gizmo with pan/dolly vec3 and quaternion
    pub fn gizmo3d_pan_dolly_quat<V3: Vec3Like, Q: QuatLike>(
        &self,
        label: &str,
        vm: &mut V3,
        q: &mut Q,
        size: f32,
        mode: Mode,
    ) -> bool {
        let mut svm = to_sys_vec3(vm);
        let mut sq = to_sys_quat(q);
        let used = with_label_ptr(label, |label_ptr| unsafe {
            sys::iggizmo3D_vec3PtrquatPtrFloat(
                label_ptr,
                &mut svm as *mut _,
                &mut sq as *mut _,
                size,
                mode.bits(),
            )
        });
        from_sys_vec3(vm, svm);
        from_sys_quat(q, sq);
        used
    }

    /// Gizmo with pan/dolly vec3 and light vec4
    pub fn gizmo3d_pan_dolly_vec4<V3: Vec3Like, V4: Vec4Like>(
        &self,
        label: &str,
        vm: &mut V3,
        v: &mut V4,
        size: f32,
        mode: Mode,
    ) -> bool {
        let mut svm = to_sys_vec3(vm);
        let mut sv4 = to_sys_vec4(v);
        let used = with_label_ptr(label, |label_ptr| unsafe {
            sys::iggizmo3D_vec3Ptrvec4Ptr(
                label_ptr,
                &mut svm as *mut _,
                &mut sv4 as *mut _,
                size,
                mode.bits(),
            )
        });
        from_sys_vec3(vm, svm);
        from_sys_vec4(v, sv4);
        used
    }

    /// Gizmo with pan/dolly vec3 and light vec3
    pub fn gizmo3d_pan_dolly_vec3<V3: Vec3Like>(
        &self,
        label: &str,
        vm: &mut V3,
        v: &mut V3,
        size: f32,
        mode: Mode,
    ) -> bool {
        let mut svm = to_sys_vec3(vm);
        let mut sv3 = to_sys_vec3(v);
        let used = with_label_ptr(label, |label_ptr| unsafe {
            sys::iggizmo3D_vec3Ptrvec3Ptr(
                label_ptr,
                &mut svm as *mut _,
                &mut sv3 as *mut _,
                size,
                mode.bits(),
            )
        });
        from_sys_vec3(vm, svm);
        from_sys_vec3(v, sv3);
        used
    }

    /// Gizmo with pan/dolly vec3, quaternion and light quaternion
    pub fn gizmo3d_pan_dolly_quat_light_quat<V3: Vec3Like, Q: QuatLike>(
        &self,
        label: &str,
        vm: &mut V3,
        q: &mut Q,
        ql: &mut Q,
        size: f32,
        mode: Mode,
    ) -> bool {
        let mut svm = to_sys_vec3(vm);
        let mut sq = to_sys_quat(q);
        let mut sql = to_sys_quat(ql);
        let used = with_label_ptr(label, |label_ptr| unsafe {
            sys::iggizmo3D_vec3PtrquatPtrquatPtr(
                label_ptr,
                &mut svm as *mut _,
                &mut sq as *mut _,
                &mut sql as *mut _,
                size,
                mode.bits(),
            )
        });
        from_sys_vec3(vm, svm);
        from_sys_quat(q, sq);
        from_sys_quat(ql, sql);
        used
    }

    /// Gizmo with pan/dolly vec3, quaternion and light vec4
    pub fn gizmo3d_pan_dolly_quat_light_vec4<V3: Vec3Like, Q: QuatLike, V4: Vec4Like>(
        &self,
        label: &str,
        vm: &mut V3,
        q: &mut Q,
        v: &mut V4,
        size: f32,
        mode: Mode,
    ) -> bool {
        let mut svm = to_sys_vec3(vm);
        let mut sq = to_sys_quat(q);
        let mut sv4 = to_sys_vec4(v);
        let used = with_label_ptr(label, |label_ptr| unsafe {
            sys::iggizmo3D_vec3PtrquatPtrvec4Ptr(
                label_ptr,
                &mut svm as *mut _,
                &mut sq as *mut _,
                &mut sv4 as *mut _,
                size,
                mode.bits(),
            )
        });
        from_sys_vec3(vm, svm);
        from_sys_quat(q, sq);
        from_sys_vec4(v, sv4);
        used
    }

    /// Gizmo with pan/dolly vec3, quaternion and light vec3
    pub fn gizmo3d_pan_dolly_quat_light_vec3<V3: Vec3Like, Q: QuatLike>(
        &self,
        label: &str,
        vm: &mut V3,
        q: &mut Q,
        v: &mut V3,
        size: f32,
        mode: Mode,
    ) -> bool {
        let mut svm = to_sys_vec3(vm);
        let mut sq = to_sys_quat(q);
        let mut sv3 = to_sys_vec3(v);
        let used = with_label_ptr(label, |label_ptr| unsafe {
            sys::iggizmo3D_vec3PtrquatPtrvec3Ptr(
                label_ptr,
                &mut svm as *mut _,
                &mut sq as *mut _,
                &mut sv3 as *mut _,
                size,
                mode.bits(),
            )
        });
        from_sys_vec3(vm, svm);
        from_sys_quat(q, sq);
        from_sys_vec3(v, sv3);
        used
    }

    /// Convenience: set direction/sphere colors using u32 colors (ImGui packed RGBA)
    pub fn set_direction_colors_u32(&self, dir: u32, plane: u32) {
        unsafe { sys::imguiGizmo_setDirectionColor_U32U32(dir, plane) }
    }
    pub fn set_direction_color_u32(&self, color: u32) {
        unsafe { sys::imguiGizmo_setDirectionColor_U32(color) }
    }
    pub fn restore_direction_color(&self) {
        unsafe { sys::imguiGizmo_restoreDirectionColor() }
    }
    pub fn set_sphere_colors_u32(&self, a: u32, b: u32) {
        unsafe { sys::imguiGizmo_setSphereColors_U32(a, b) }
    }
    pub fn restore_sphere_colors_u32(&self) {
        unsafe { sys::imguiGizmo_restoreSphereColors() }
    }

    /// Set direction/plane colors using float rgba
    pub fn set_direction_colors_vec4(&self, dir: [f32; 4], plane: [f32; 4]) {
        unsafe {
            sys::imguiGizmo_setDirectionColor_Vec4Vec4(
                sys::ImVec4 {
                    x: dir[0],
                    y: dir[1],
                    z: dir[2],
                    w: dir[3],
                },
                sys::ImVec4 {
                    x: plane[0],
                    y: plane[1],
                    z: plane[2],
                    w: plane[3],
                },
            )
        }
    }
    pub fn set_direction_color_vec4(&self, color: [f32; 4]) {
        unsafe {
            sys::imguiGizmo_setDirectionColor_Vec4(sys::ImVec4 {
                x: color[0],
                y: color[1],
                z: color[2],
                w: color[3],
            })
        }
    }
    pub fn set_sphere_colors_vec4(&self, a: [f32; 4], b: [f32; 4]) {
        unsafe {
            sys::imguiGizmo_setSphereColors_Vec4(
                sys::ImVec4 {
                    x: a[0],
                    y: a[1],
                    z: a[2],
                    w: a[3],
                },
                sys::ImVec4 {
                    x: b[0],
                    y: b[1],
                    z: b[2],
                    w: b[3],
                },
            )
        }
    }

    /// Global gizmo feel and scales
    pub fn set_gizmo_feeling_rot(&self, f: f32) {
        unsafe { sys::imguiGizmo_setGizmoFeelingRot(f) }
    }
    pub fn gizmo_feeling_rot(&self) -> f32 {
        unsafe { sys::imguiGizmo_getGizmoFeelingRot() }
    }
    pub fn set_dolly_scale(&self, f: f32) {
        unsafe { sys::imguiGizmo_setDollyScale(f) }
    }
    pub fn dolly_scale(&self) -> f32 {
        unsafe { sys::imguiGizmo_getDollyScale() }
    }
    pub fn set_dolly_wheel_scale(&self, f: f32) {
        unsafe { sys::imguiGizmo_setDollyWheelScale(f) }
    }
    pub fn dolly_wheel_scale(&self) -> f32 {
        unsafe { sys::imguiGizmo_getDollyWheelScale() }
    }
    pub fn set_pan_scale(&self, f: f32) {
        unsafe { sys::imguiGizmo_setPanScale(f) }
    }
    pub fn pan_scale(&self) -> f32 {
        unsafe { sys::imguiGizmo_getPanScale() }
    }

    /// Set the keyboard modifier that activates Pan.
    /// Default: Control. Combine with bitflags if desired.
    pub fn set_pan_modifier(&self, m: Modifiers) {
        unsafe { sys::imguiGizmo_setPanModifier(modifiers_to_sys(m)) }
    }
    /// Set the keyboard modifier that activates Dolly/Zoom.
    /// Default: Shift. Combine with bitflags if desired.
    pub fn set_dolly_modifier(&self, m: Modifiers) {
        unsafe { sys::imguiGizmo_setDollyModifier(modifiers_to_sys(m)) }
    }

    /// Flip options
    pub fn flip_rot_on_x(&self, b: bool) {
        unsafe { sys::imguiGizmo_flipRotOnX(b) }
    }
    pub fn flip_rot_on_y(&self, b: bool) {
        unsafe { sys::imguiGizmo_flipRotOnY(b) }
    }
    pub fn flip_rot_on_z(&self, b: bool) {
        unsafe { sys::imguiGizmo_flipRotOnZ(b) }
    }
    pub fn flip_pan_x(&self, b: bool) {
        unsafe { sys::imguiGizmo_setFlipPanX(b) }
    }
    pub fn flip_pan_y(&self, b: bool) {
        unsafe { sys::imguiGizmo_setFlipPanY(b) }
    }
    pub fn flip_dolly(&self, b: bool) {
        unsafe { sys::imguiGizmo_setFlipDolly(b) }
    }
    pub fn is_flip_rot_on_x(&self) -> bool {
        unsafe { sys::imguiGizmo_getFlipRotOnX() }
    }
    pub fn is_flip_rot_on_y(&self) -> bool {
        unsafe { sys::imguiGizmo_getFlipRotOnY() }
    }
    pub fn is_flip_rot_on_z(&self) -> bool {
        unsafe { sys::imguiGizmo_getFlipRotOnZ() }
    }
    pub fn is_flip_pan_x(&self) -> bool {
        unsafe { sys::imguiGizmo_getFlipPanX() }
    }
    pub fn is_flip_pan_y(&self) -> bool {
        unsafe { sys::imguiGizmo_getFlipPanY() }
    }
    pub fn is_flip_dolly(&self) -> bool {
        unsafe { sys::imguiGizmo_getFlipDolly() }
    }

    /// Reverse axis directions
    pub fn reverse_x(&self, b: bool) {
        unsafe { sys::imguiGizmo_reverseX(b) }
    }
    pub fn reverse_y(&self, b: bool) {
        unsafe { sys::imguiGizmo_reverseY(b) }
    }
    pub fn reverse_z(&self, b: bool) {
        unsafe { sys::imguiGizmo_reverseZ(b) }
    }
    pub fn is_reverse_x(&self) -> bool {
        unsafe { sys::imguiGizmo_getReverseX() }
    }
    pub fn is_reverse_y(&self) -> bool {
        unsafe { sys::imguiGizmo_getReverseY() }
    }
    pub fn is_reverse_z(&self) -> bool {
        unsafe { sys::imguiGizmo_getReverseZ() }
    }

    /// Resize helpers
    pub fn resize_axes_of<V3: Vec3Like>(&self, new_size: &V3) {
        let s = to_sys_vec3(new_size);
        unsafe { sys::imguiGizmo_resizeAxesOf(s) }
    }
    pub fn restore_axes_size(&self) {
        unsafe { sys::imguiGizmo_restoreAxesSize() }
    }
    pub fn resize_solid_of(&self, new_size: f32) {
        unsafe { sys::imguiGizmo_resizeSolidOf(new_size) }
    }
    pub fn restore_solid_size(&self) {
        unsafe { sys::imguiGizmo_restoreSolidSize() }
    }

    // Note: checkTowards* and getTransforms* variants require an imguiGizmo instance pointer
    // which is not exposed by the C API, so they are intentionally not wrapped here.
}

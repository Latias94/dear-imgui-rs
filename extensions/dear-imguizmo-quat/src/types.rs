use dear_imguizmo_quat_sys as sys;

/// Trait for quaternion-like types convertible to/from [x,y,z,w].
///
/// Implemented for `[f32; 4]`, `glam::Quat` (feature `glam`) and
/// `mint::Quaternion<f32>` (feature `mint`).
pub trait QuatLike {
    fn to_xyzw(&self) -> [f32; 4];
    fn set_from_xyzw(&mut self, v: [f32; 4]);
}

/// Trait for vec3-like types convertible to/from [x,y,z].
///
/// Implemented for `[f32; 3]`, `glam::Vec3` (feature `glam`) and
/// `mint::Vector3<f32>` (feature `mint`).
pub trait Vec3Like {
    fn to_array(&self) -> [f32; 3];
    fn set_from_array(&mut self, v: [f32; 3]);
}

/// Trait for vec4-like types convertible to/from [x,y,z,w].
///
/// Implemented for `[f32; 4]`, `glam::Vec4` (feature `glam`) and
/// `mint::Vector4<f32>` (feature `mint`).
pub trait Vec4Like {
    fn to_array(&self) -> [f32; 4];
    fn set_from_array(&mut self, v: [f32; 4]);
}

impl QuatLike for [f32; 4] {
    fn to_xyzw(&self) -> [f32; 4] {
        *self
    }
    fn set_from_xyzw(&mut self, v: [f32; 4]) {
        *self = v;
    }
}
impl Vec3Like for [f32; 3] {
    fn to_array(&self) -> [f32; 3] {
        *self
    }
    fn set_from_array(&mut self, v: [f32; 3]) {
        *self = v;
    }
}
impl Vec4Like for [f32; 4] {
    fn to_array(&self) -> [f32; 4] {
        *self
    }
    fn set_from_array(&mut self, v: [f32; 4]) {
        *self = v;
    }
}

#[cfg(feature = "glam")]
impl QuatLike for glam::Quat {
    fn to_xyzw(&self) -> [f32; 4] {
        [self.x, self.y, self.z, self.w]
    }
    fn set_from_xyzw(&mut self, v: [f32; 4]) {
        *self = glam::Quat::from_xyzw(v[0], v[1], v[2], v[3]);
    }
}
#[cfg(feature = "glam")]
impl Vec3Like for glam::Vec3 {
    fn to_array(&self) -> [f32; 3] {
        self.to_array()
    }
    fn set_from_array(&mut self, v: [f32; 3]) {
        *self = glam::Vec3::from_array(v);
    }
}
#[cfg(feature = "glam")]
impl Vec4Like for glam::Vec4 {
    fn to_array(&self) -> [f32; 4] {
        self.to_array()
    }
    fn set_from_array(&mut self, v: [f32; 4]) {
        *self = glam::Vec4::from_array(v);
    }
}

#[cfg(feature = "mint")]
impl QuatLike for mint::Quaternion<f32> {
    fn to_xyzw(&self) -> [f32; 4] {
        [self.v.x, self.v.y, self.v.z, self.s]
    }
    fn set_from_xyzw(&mut self, v: [f32; 4]) {
        self.v.x = v[0];
        self.v.y = v[1];
        self.v.z = v[2];
        self.s = v[3];
    }
}
#[cfg(feature = "mint")]
impl Vec3Like for mint::Vector3<f32> {
    fn to_array(&self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }
    fn set_from_array(&mut self, v: [f32; 3]) {
        self.x = v[0];
        self.y = v[1];
        self.z = v[2];
    }
}
#[cfg(feature = "mint")]
impl Vec4Like for mint::Vector4<f32> {
    fn to_array(&self) -> [f32; 4] {
        [self.x, self.y, self.z, self.w]
    }
    fn set_from_array(&mut self, v: [f32; 4]) {
        self.x = v[0];
        self.y = v[1];
        self.z = v[2];
        self.w = v[3];
    }
}

bitflags::bitflags! {
    /// Flags for gizmo modes and aspects (mirror C++ enum values).
    #[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct Mode: u32 {
        const MODE_3_AXES      = 0x0001;
        const MODE_DIRECTION   = 0x0002;
        const MODE_DIR_PLANE   = 0x0004;
        const MODE_DUAL        = 0x0008;
        const MODE_PAN_DOLLY   = 0x0010;
        const CUBE_AT_ORIGIN   = 0x0100;
        const SPHERE_AT_ORIGIN = 0x0200;
        const NO_SOLID_AT_ORIGIN = 0x0400;
        const MODE_FULL_AXES   = 0x0800;
    }
}

bitflags::bitflags! {
    /// Key modifier flags for Pan/Dolly actions (mirror vGizmo3D).
    /// Values match vg::ev*Modifier: Shift=1, Control=1<<1, Alt=1<<2, Super=1<<3.
    #[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct Modifiers: i32 {
        const NONE    = 0;
        const SHIFT   = 1;
        const CONTROL = 1 << 1;
        const ALT     = 1 << 2;
        const SUPER   = 1 << 3;
    }
}

#[inline]
pub(crate) fn modifiers_to_sys(m: Modifiers) -> sys::vgModifiers {
    m.bits() as sys::vgModifiers
}

#[inline]
pub(crate) fn to_sys_quat<Q: QuatLike>(q: &Q) -> sys::quat {
    let a = q.to_xyzw();
    sys::quat {
        x: a[0],
        y: a[1],
        z: a[2],
        w: a[3],
    }
}
#[inline]
pub(crate) fn from_sys_quat<Q: QuatLike>(out: &mut Q, s: sys::quat) {
    out.set_from_xyzw([s.x, s.y, s.z, s.w]);
}
#[inline]
pub(crate) fn to_sys_vec3<V: Vec3Like>(v: &V) -> sys::vec3 {
    let a = v.to_array();
    sys::vec3 {
        x: a[0],
        y: a[1],
        z: a[2],
    }
}
#[inline]
pub(crate) fn from_sys_vec3<V: Vec3Like>(out: &mut V, s: sys::vec3) {
    out.set_from_array([s.x, s.y, s.z]);
}
#[inline]
pub(crate) fn to_sys_vec4<V: Vec4Like>(v: &V) -> sys::vec4 {
    let a = v.to_array();
    sys::vec4 {
        x: a[0],
        y: a[1],
        z: a[2],
        w: a[3],
    }
}
#[inline]
pub(crate) fn from_sys_vec4<V: Vec4Like>(out: &mut V, s: sys::vec4) {
    out.set_from_array([s.x, s.y, s.z, s.w]);
}

use dear_imguizmo_sys as sys;

bitflags::bitflags! {
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Operation: u32 {
        const TRANSLATE_X     = sys::TRANSLATE_X as u32;
        const TRANSLATE_Y     = sys::TRANSLATE_Y as u32;
        const TRANSLATE_Z     = sys::TRANSLATE_Z as u32;
        const ROTATE_X        = sys::ROTATE_X as u32;
        const ROTATE_Y        = sys::ROTATE_Y as u32;
        const ROTATE_Z        = sys::ROTATE_Z as u32;
        const ROTATE_SCREEN   = sys::ROTATE_SCREEN as u32;
        const SCALE_X         = sys::SCALE_X as u32;
        const SCALE_Y         = sys::SCALE_Y as u32;
        const SCALE_Z         = sys::SCALE_Z as u32;
        const BOUNDS          = sys::BOUNDS as u32;
        const SCALE_UNIFORM_X = sys::SCALE_XU as u32;
        const SCALE_UNIFORM_Y = sys::SCALE_YU as u32;
        const SCALE_UNIFORM_Z = sys::SCALE_ZU as u32;

        const TRANSLATE    = sys::TRANSLATE as u32;
        const ROTATE       = sys::ROTATE as u32;
        const SCALE        = sys::SCALE as u32;
        const SCALE_UNIFORM= sys::SCALEU as u32;
        const UNIVERSAL    = sys::UNIVERSAL as u32;
    }
}

impl From<Operation> for sys::OPERATION {
    fn from(value: Operation) -> Self {
        value.bits() as sys::OPERATION
    }
}

bitflags::bitflags! {
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
    pub struct AxisMask: u8 {
        const X = 1 << 0;
        const Y = 1 << 1;
        const Z = 1 << 2;
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Mode {
    Local = sys::LOCAL as u32,
    World = sys::WORLD as u32,
}

impl From<Mode> for sys::MODE {
    fn from(value: Mode) -> Self {
        value as sys::MODE
    }
}

/// Color slots used by ImGuizmo style
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Color {
    DirectionX = sys::DIRECTION_X as u32,
    DirectionY = sys::DIRECTION_Y as u32,
    DirectionZ = sys::DIRECTION_Z as u32,
    PlaneX = sys::PLANE_X as u32,
    PlaneY = sys::PLANE_Y as u32,
    PlaneZ = sys::PLANE_Z as u32,
    Selection = sys::SELECTION as u32,
    Inactive = sys::INACTIVE as u32,
    TranslationLine = sys::TRANSLATION_LINE as u32,
    ScaleLine = sys::SCALE_LINE as u32,
    RotationUsingBorder = sys::ROTATION_USING_BORDER as u32,
    RotationUsingFill = sys::ROTATION_USING_FILL as u32,
    HatchedAxisLines = sys::HATCHED_AXIS_LINES as u32,
    Text = sys::TEXT as u32,
    TextShadow = sys::TEXT_SHADOW as u32,
    Count = sys::COUNT as u32,
}

/// Draw list destination for ImGuizmo rendering
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DrawListTarget {
    Window,
    Background,
    Foreground,
}

/// Identifier accepted by ImGuizmo's ID stack
#[derive(Copy, Clone, Debug)]
pub enum GuizmoId<'a> {
    Int(i32),
    Str(&'a str),
    /// A non-NUL-terminated byte slice, passed via the `str_begin/str_end` C API.
    ///
    /// This allows IDs containing interior NUL bytes and avoids `CString` allocation.
    Bytes(&'a [u8]),
    Ptr(*const std::ffi::c_void),
}
impl From<i32> for GuizmoId<'_> {
    fn from(v: i32) -> Self {
        GuizmoId::Int(v)
    }
}
impl<'a> From<&'a str> for GuizmoId<'a> {
    fn from(s: &'a str) -> Self {
        GuizmoId::Str(s)
    }
}
impl<'a> From<&'a [u8]> for GuizmoId<'a> {
    fn from(bytes: &'a [u8]) -> Self {
        GuizmoId::Bytes(bytes)
    }
}
impl<T> From<*const T> for GuizmoId<'_> {
    fn from(p: *const T) -> Self {
        GuizmoId::Ptr(p as _)
    }
}
impl<T> From<*mut T> for GuizmoId<'_> {
    fn from(p: *mut T) -> Self {
        GuizmoId::Ptr(p as _)
    }
}

/// Bounds type for typed bounds passing
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Bounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

/// Simple 3D vector adaptor for glam/mint/[f32;3]/tuples
pub trait Vec3Like {
    fn to_array(self) -> [f32; 3];
}
impl Vec3Like for [f32; 3] {
    fn to_array(self) -> [f32; 3] {
        self
    }
}
impl Vec3Like for (f32, f32, f32) {
    fn to_array(self) -> [f32; 3] {
        [self.0, self.1, self.2]
    }
}
#[cfg(feature = "glam")]
impl Vec3Like for glam::Vec3 {
    fn to_array(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }
}
#[cfg(feature = "mint")]
impl Vec3Like for mint::Vector3<f32> {
    fn to_array(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }
}

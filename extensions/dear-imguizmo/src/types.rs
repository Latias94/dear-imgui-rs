use dear_imguizmo_sys as sys;

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

impl From<Operation> for sys::OPERATION {
    fn from(value: Operation) -> Self { value.bits() as sys::OPERATION }
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
    Local = 0,
    World = 1,
}

impl From<Mode> for sys::MODE {
    fn from(value: Mode) -> Self {
        match value {
            Mode::Local => 0 as sys::MODE,
            Mode::World => 1 as sys::MODE,
        }
    }
}

/// Color slots used by ImGuizmo style
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Color {
    DirectionX = 0,
    DirectionY = 1,
    DirectionZ = 2,
    PlaneX = 3,
    PlaneY = 4,
    PlaneZ = 5,
    Selection = 6,
    Inactive = 7,
    TranslationLine = 8,
    ScaleLine = 9,
    RotationUsingBorder = 10,
    RotationUsingFill = 11,
    HatchedAxisLines = 12,
    Text = 13,
    TextShadow = 14,
    Count = 15,
}

/// Draw list destination for ImGuizmo rendering
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DrawListTarget { Window, Background, Foreground }

/// Identifier accepted by ImGuizmo's ID stack
#[derive(Copy, Clone, Debug)]
pub enum GuizmoId<'a> {
    Int(i32),
    Str(&'a str),
    Ptr(*const std::ffi::c_void),
}
impl From<i32> for GuizmoId<'_> { fn from(v: i32) -> Self { GuizmoId::Int(v) } }
impl<'a> From<&'a str> for GuizmoId<'a> { fn from(s: &'a str) -> Self { GuizmoId::Str(s) } }
impl<T> From<*const T> for GuizmoId<'_> { fn from(p: *const T) -> Self { GuizmoId::Ptr(p as _) } }
impl<T> From<*mut T> for GuizmoId<'_> { fn from(p: *mut T) -> Self { GuizmoId::Ptr(p as _) } }

/// Bounds type for typed bounds passing
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Bounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

/// Simple 3D vector adaptor for glam/mint/[f32;3]/tuples
pub trait Vec3Like { fn to_array(self) -> [f32; 3]; }
impl Vec3Like for [f32; 3] { fn to_array(self) -> [f32;3] { self } }
impl Vec3Like for (f32, f32, f32) { fn to_array(self) -> [f32;3] { [self.0, self.1, self.2] } }
#[cfg(feature = "glam")] impl Vec3Like for glam::Vec3 { fn to_array(self) -> [f32;3] { [self.x, self.y, self.z] } }
#[cfg(feature = "mint")] impl Vec3Like for mint::Vector3<f32> { fn to_array(self) -> [f32;3] { [self.x, self.y, self.z] } }

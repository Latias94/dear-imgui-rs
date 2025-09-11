//! Type definitions for ImGuizmo operations

use crate::sys;
use bitflags::bitflags;

bitflags! {
    /// Gizmo operation types
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Operation: u32 {
        /// Translate along X axis
        const TRANSLATE_X = 1;
        /// Translate along Y axis
        const TRANSLATE_Y = 2;
        /// Translate along Z axis
        const TRANSLATE_Z = 4;
        /// Rotate around X axis
        const ROTATE_X = 8;
        /// Rotate around Y axis
        const ROTATE_Y = 16;
        /// Rotate around Z axis
        const ROTATE_Z = 32;
        /// Rotate around screen axis
        const ROTATE_SCREEN = 64;
        /// Scale along X axis
        const SCALE_X = 128;
        /// Scale along Y axis
        const SCALE_Y = 256;
        /// Scale along Z axis
        const SCALE_Z = 512;
        /// Bounds manipulation
        const BOUNDS = 1024;
        /// Uniform scale along X axis
        const SCALE_XU = 2048;
        /// Uniform scale along Y axis
        const SCALE_YU = 4096;
        /// Uniform scale along Z axis
        const SCALE_ZU = 8192;

        /// All translation operations
        const TRANSLATE = Self::TRANSLATE_X.bits() | Self::TRANSLATE_Y.bits() | Self::TRANSLATE_Z.bits();
        /// All rotation operations
        const ROTATE = Self::ROTATE_X.bits() | Self::ROTATE_Y.bits() | Self::ROTATE_Z.bits() | Self::ROTATE_SCREEN.bits();
        /// All scale operations
        const SCALE = Self::SCALE_X.bits() | Self::SCALE_Y.bits() | Self::SCALE_Z.bits();
        /// All uniform scale operations
        const SCALEU = Self::SCALE_XU.bits() | Self::SCALE_YU.bits() | Self::SCALE_ZU.bits();
        /// All operations (universal gizmo)
        const UNIVERSAL = Self::TRANSLATE.bits() | Self::ROTATE.bits() | Self::SCALEU.bits() | Self::BOUNDS.bits();
    }
}

/// Gizmo manipulation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Local space manipulation
    Local,
    /// World space manipulation
    World,
}

impl From<Mode> for sys::MODE {
    fn from(mode: Mode) -> Self {
        match mode {
            Mode::Local => 0,
            Mode::World => 1,
        }
    }
}

/// Color indices for gizmo styling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorType {
    /// X direction color
    DirectionX,
    /// Y direction color
    DirectionY,
    /// Z direction color
    DirectionZ,
    /// X plane color
    PlaneX,
    /// Y plane color
    PlaneY,
    /// Z plane color
    PlaneZ,
    /// Selection color
    Selection,
    /// Inactive color
    Inactive,
    /// Translation line color
    TranslationLine,
    /// Scale line color
    ScaleLine,
    /// Rotation border color
    RotationUsingBorder,
    /// Rotation fill color
    RotationUsingFill,
    /// Hatched axis lines color
    HatchedAxisLines,
    /// Text color
    Text,
    /// Text shadow color
    TextShadow,
}

impl From<ColorType> for sys::COLOR {
    fn from(color: ColorType) -> Self {
        match color {
            ColorType::DirectionX => 0,
            ColorType::DirectionY => 1,
            ColorType::DirectionZ => 2,
            ColorType::PlaneX => 3,
            ColorType::PlaneY => 4,
            ColorType::PlaneZ => 5,
            ColorType::Selection => 6,
            ColorType::Inactive => 7,
            ColorType::TranslationLine => 8,
            ColorType::ScaleLine => 9,
            ColorType::RotationUsingBorder => 10,
            ColorType::RotationUsingFill => 11,
            ColorType::HatchedAxisLines => 12,
            ColorType::Text => 13,
            ColorType::TextShadow => 14,
        }
    }
}

/// 4x4 transformation matrix
pub type Matrix4 = [f32; 16];

/// 3D vector for translation, rotation (in degrees), or scale
pub type Vector3 = [f32; 3];

/// 2D vector for screen coordinates
pub type Vector2 = [f32; 2];

/// RGBA color
pub type Color = [f32; 4];

/// Rectangle definition
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// X coordinate
    pub x: f32,
    /// Y coordinate
    pub y: f32,
    /// Width
    pub width: f32,
    /// Height
    pub height: f32,
}

impl Rect {
    /// Create a new rectangle
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Manipulation result containing delta information
#[derive(Debug, Clone, PartialEq)]
pub struct ManipulationResult {
    /// Whether the gizmo was used this frame
    pub used: bool,
    /// Delta transformation matrix (if available)
    pub delta_matrix: Option<Matrix4>,
    /// Whether the gizmo is currently being hovered
    pub hovered: bool,
}

impl Default for ManipulationResult {
    fn default() -> Self {
        Self {
            used: false,
            delta_matrix: None,
            hovered: false,
        }
    }
}

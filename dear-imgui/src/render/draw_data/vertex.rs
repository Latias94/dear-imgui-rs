use crate::sys;

/// Vertex format used by Dear ImGui
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DrawVert {
    /// Position (2D)
    pub pos: [f32; 2],
    /// UV coordinates
    pub uv: [f32; 2],
    /// Color (packed RGBA)
    pub col: u32,
}

// Ensure our Rust-side vertex/index types stay layout-compatible with the raw sys bindings.
const _: [(); std::mem::size_of::<sys::ImDrawVert>()] = [(); std::mem::size_of::<DrawVert>()];
const _: [(); std::mem::align_of::<sys::ImDrawVert>()] = [(); std::mem::align_of::<DrawVert>()];

impl DrawVert {
    /// Creates a new draw vertex with u32 color
    pub fn new(pos: [f32; 2], uv: [f32; 2], col: u32) -> Self {
        Self { pos, uv, col }
    }

    /// Creates a new draw vertex from RGBA bytes
    pub fn from_rgba(pos: [f32; 2], uv: [f32; 2], rgba: [u8; 4]) -> Self {
        let col = ((rgba[3] as u32) << 24)
            | ((rgba[2] as u32) << 16)
            | ((rgba[1] as u32) << 8)
            | (rgba[0] as u32);
        Self { pos, uv, col }
    }

    /// Extracts RGBA bytes from the packed color
    pub fn rgba(&self) -> [u8; 4] {
        [
            (self.col & 0xFF) as u8,
            ((self.col >> 8) & 0xFF) as u8,
            ((self.col >> 16) & 0xFF) as u8,
            ((self.col >> 24) & 0xFF) as u8,
        ]
    }
}

/// Index type used by Dear ImGui
pub type DrawIdx = u16;

const _: [(); std::mem::size_of::<sys::ImDrawIdx>()] = [(); std::mem::size_of::<DrawIdx>()];
const _: [(); std::mem::align_of::<sys::ImDrawIdx>()] = [(); std::mem::align_of::<DrawIdx>()];

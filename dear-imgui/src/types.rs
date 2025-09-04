/// Common types and type aliases for Dear ImGui

/// 2D vector type
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };
    pub const ONE: Vec2 = Vec2 { x: 1.0, y: 1.0 };

    /// Create a new Vec2
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl From<[f32; 2]> for Vec2 {
    fn from(arr: [f32; 2]) -> Self {
        Self {
            x: arr[0],
            y: arr[1],
        }
    }
}

impl From<Vec2> for [f32; 2] {
    fn from(vec: Vec2) -> Self {
        [vec.x, vec.y]
    }
}

/// 4D vector type (commonly used for colors)
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vec4 {
    pub const ZERO: Vec4 = Vec4 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 0.0,
    };
    pub const ONE: Vec4 = Vec4 {
        x: 1.0,
        y: 1.0,
        z: 1.0,
        w: 1.0,
    };

    /// Create a new Vec4
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }
}

impl From<[f32; 4]> for Vec4 {
    fn from(arr: [f32; 4]) -> Self {
        Self {
            x: arr[0],
            y: arr[1],
            z: arr[2],
            w: arr[3],
        }
    }
}

impl From<Vec4> for [f32; 4] {
    fn from(vec: Vec4) -> Self {
        [vec.x, vec.y, vec.z, vec.w]
    }
}

/// Color type (alias for Vec4)
pub type Color = Vec4;

impl Color {
    pub const WHITE: Color = Color {
        x: 1.0,
        y: 1.0,
        z: 1.0,
        w: 1.0,
    };
    pub const BLACK: Color = Color {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };
    pub const RED: Color = Color {
        x: 1.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };
    pub const GREEN: Color = Color {
        x: 0.0,
        y: 1.0,
        z: 0.0,
        w: 1.0,
    };
    pub const BLUE: Color = Color {
        x: 0.0,
        y: 0.0,
        z: 1.0,
        w: 1.0,
    };
    pub const TRANSPARENT: Color = Color {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 0.0,
    };

    /// Create a new color from RGB values (alpha = 1.0)
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self {
            x: r,
            y: g,
            z: b,
            w: 1.0,
        }
    }

    /// Create a new color from RGBA values
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self {
            x: r,
            y: g,
            z: b,
            w: a,
        }
    }

    /// Get the red component
    pub fn r(&self) -> f32 {
        self.x
    }

    /// Get the green component
    pub fn g(&self) -> f32 {
        self.y
    }

    /// Get the blue component
    pub fn b(&self) -> f32 {
        self.z
    }

    /// Get the alpha component
    pub fn a(&self) -> f32 {
        self.w
    }
}

/// Type-safe ID for Dear ImGui widgets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Id(u32);

impl Id {
    /// Create an ID from a string
    pub fn new(s: &str) -> Self {
        Self(hash_string(s))
    }

    /// Create an ID with an additional index
    pub fn with_index(self, index: usize) -> Self {
        Self(self.0.wrapping_add(index as u32))
    }

    /// Get the raw ID value
    pub fn raw(self) -> u32 {
        self.0
    }
}

impl From<&str> for Id {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for Id {
    fn from(s: String) -> Self {
        Self::new(&s)
    }
}

impl From<u32> for Id {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

/// Simple string hash function (FNV-1a)
fn hash_string(s: &str) -> u32 {
    let mut hash = 2166136261u32;
    for byte in s.bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vec2() {
        let v = Vec2::new(1.0, 2.0);
        assert_eq!(v.x, 1.0);
        assert_eq!(v.y, 2.0);

        let arr: [f32; 2] = v.into();
        assert_eq!(arr, [1.0, 2.0]);
    }

    #[test]
    fn test_color() {
        let c = Color::rgb(1.0, 0.5, 0.0);
        assert_eq!(c.r(), 1.0);
        assert_eq!(c.g(), 0.5);
        assert_eq!(c.b(), 0.0);
        assert_eq!(c.a(), 1.0);
    }

    #[test]
    fn test_id() {
        let id1 = Id::new("test");
        let id2 = Id::new("test");
        let id3 = Id::new("different");

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);

        let indexed = id1.with_index(42);
        assert_ne!(id1, indexed);
    }
}

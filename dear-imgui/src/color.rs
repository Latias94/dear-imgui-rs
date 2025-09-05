use std::fmt;

/// RGBA color with 32-bit floating point components
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Color {
    /// Red component (0.0 to 1.0)
    pub r: f32,
    /// Green component (0.0 to 1.0)
    pub g: f32,
    /// Blue component (0.0 to 1.0)
    pub b: f32,
    /// Alpha component (0.0 to 1.0)
    pub a: f32,
}

impl Color {
    /// Creates a new color from RGBA components
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Creates a new color from RGB components with full alpha
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self::new(r, g, b, 1.0)
    }

    /// Creates a new color from RGBA bytes (0-255)
    pub fn from_rgba_bytes(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::new(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        )
    }

    /// Creates a new color from RGB bytes (0-255) with full alpha
    pub fn from_rgb_bytes(r: u8, g: u8, b: u8) -> Self {
        Self::from_rgba_bytes(r, g, b, 255)
    }

    /// Creates a new color from a 32-bit RGBA value
    pub fn from_rgba_u32(rgba: u32) -> Self {
        Self::from_rgba_bytes(
            ((rgba >> 24) & 0xFF) as u8,
            ((rgba >> 16) & 0xFF) as u8,
            ((rgba >> 8) & 0xFF) as u8,
            (rgba & 0xFF) as u8,
        )
    }

    /// Creates a new color from a 32-bit RGB value with full alpha
    pub fn from_rgb_u32(rgb: u32) -> Self {
        Self::from_rgba_u32((rgb << 8) | 0xFF)
    }

    /// Converts the color to a 32-bit RGBA value
    pub fn to_rgba_u32(self) -> u32 {
        let r = (self.r * 255.0).round() as u32;
        let g = (self.g * 255.0).round() as u32;
        let b = (self.b * 255.0).round() as u32;
        let a = (self.a * 255.0).round() as u32;
        (r << 24) | (g << 16) | (b << 8) | a
    }

    /// Converts the color to an array of f32 components
    pub fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// Creates a color from an array of f32 components
    pub fn from_array(arr: [f32; 4]) -> Self {
        Self::new(arr[0], arr[1], arr[2], arr[3])
    }

    /// Returns a color with modified alpha
    pub fn with_alpha(mut self, alpha: f32) -> Self {
        self.a = alpha;
        self
    }

    /// Linearly interpolates between two colors
    pub fn lerp(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self::new(
            self.r + (other.r - self.r) * t,
            self.g + (other.g - self.g) * t,
            self.b + (other.b - self.b) * t,
            self.a + (other.a - self.a) * t,
        )
    }

    /// Converts RGB to HSV color space
    pub fn to_hsv(self) -> (f32, f32, f32) {
        let max = self.r.max(self.g).max(self.b);
        let min = self.r.min(self.g).min(self.b);
        let delta = max - min;

        let h = if delta == 0.0 {
            0.0
        } else if max == self.r {
            60.0 * (((self.g - self.b) / delta) % 6.0)
        } else if max == self.g {
            60.0 * (((self.b - self.r) / delta) + 2.0)
        } else {
            60.0 * (((self.r - self.g) / delta) + 4.0)
        };

        let s = if max == 0.0 { 0.0 } else { delta / max };
        let v = max;

        (h, s, v)
    }

    /// Creates a color from HSV color space
    pub fn from_hsv(h: f32, s: f32, v: f32) -> Self {
        let h = h % 360.0;
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;

        let (r, g, b) = if h < 60.0 {
            (c, x, 0.0)
        } else if h < 120.0 {
            (x, c, 0.0)
        } else if h < 180.0 {
            (0.0, c, x)
        } else if h < 240.0 {
            (0.0, x, c)
        } else if h < 300.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };

        Self::new(r + m, g + m, b + m, 1.0)
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::new(1.0, 1.0, 1.0, 1.0) // White
    }
}

impl From<[f32; 4]> for Color {
    fn from(arr: [f32; 4]) -> Self {
        Self::from_array(arr)
    }
}

impl From<Color> for [f32; 4] {
    fn from(color: Color) -> Self {
        color.to_array()
    }
}

impl From<(f32, f32, f32, f32)> for Color {
    fn from((r, g, b, a): (f32, f32, f32, f32)) -> Self {
        Self::new(r, g, b, a)
    }
}

impl From<Color> for (f32, f32, f32, f32) {
    fn from(color: Color) -> Self {
        (color.r, color.g, color.b, color.a)
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "rgba({:.3}, {:.3}, {:.3}, {:.3})",
            self.r, self.g, self.b, self.a
        )
    }
}

/// Common color constants
impl Color {
    /// Transparent black
    pub const TRANSPARENT: Color = Color::new(0.0, 0.0, 0.0, 0.0);
    /// Black
    pub const BLACK: Color = Color::new(0.0, 0.0, 0.0, 1.0);
    /// White
    pub const WHITE: Color = Color::new(1.0, 1.0, 1.0, 1.0);
    /// Red
    pub const RED: Color = Color::new(1.0, 0.0, 0.0, 1.0);
    /// Green
    pub const GREEN: Color = Color::new(0.0, 1.0, 0.0, 1.0);
    /// Blue
    pub const BLUE: Color = Color::new(0.0, 0.0, 1.0, 1.0);
    /// Yellow
    pub const YELLOW: Color = Color::new(1.0, 1.0, 0.0, 1.0);
    /// Cyan
    pub const CYAN: Color = Color::new(0.0, 1.0, 1.0, 1.0);
    /// Magenta
    pub const MAGENTA: Color = Color::new(1.0, 0.0, 1.0, 1.0);
    /// Gray
    pub const GRAY: Color = Color::new(0.5, 0.5, 0.5, 1.0);
    /// Light gray
    pub const LIGHT_GRAY: Color = Color::new(0.75, 0.75, 0.75, 1.0);
    /// Dark gray
    pub const DARK_GRAY: Color = Color::new(0.25, 0.25, 0.25, 1.0);
}

/// Color edit flags for color picker widgets
#[repr(i32)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum ColorEditFlags {
    /// No flags
    None = 0,
    /// ColorEdit, ColorPicker, ColorButton: ignore Alpha component (will only read 3 components from the input pointer).
    NoAlpha = 1 << 1,
    /// ColorEdit: disable picker when clicking on colored square.
    NoPicker = 1 << 2,
    /// ColorEdit: disable toggling options menu when right-clicking on inputs/small preview.
    NoOptions = 1 << 3,
    /// ColorEdit, ColorPicker: disable colored square preview next to the inputs. (e.g. to show only the inputs)
    NoSmallPreview = 1 << 4,
    /// ColorEdit, ColorPicker: disable inputs sliders/text widgets (e.g. to show only the small preview colored square).
    NoInputs = 1 << 5,
    /// ColorEdit, ColorPicker, ColorButton: disable tooltip when hovering the preview.
    NoTooltip = 1 << 6,
    /// ColorEdit, ColorPicker: disable display of inline text label (the label is still forwarded to the tooltip and picker).
    NoLabel = 1 << 7,
    /// ColorPicker: disable bigger color preview on right side of the picker, use small colored square preview instead.
    NoSidePreview = 1 << 8,
    /// ColorEdit: disable drag and drop target. ColorButton: disable drag and drop source.
    NoDragDrop = 1 << 9,
    /// ColorButton: disable border (which is enforced by default)
    NoBorder = 1 << 10,
}

/// Color format for display
#[repr(i32)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum ColorFormat {
    /// RGB format
    RGB = 1 << 16,
    /// HSV format
    HSV = 1 << 17,
    /// Hex format
    Hex = 1 << 18,
}

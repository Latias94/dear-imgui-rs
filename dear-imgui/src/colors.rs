//! Color utilities
//!
//! Lightweight RGBA color type and helpers for converting to/from Dear ImGui
//! packed colors. Useful for configuring styles and draw-list primitives.
//!
//! Quick example:
//! ```no_run
//! # use dear_imgui_rs::*;
//! let white = Color::rgb(1.0, 1.0, 1.0);
//! let abgr = white.to_imgui_u32();
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! ui.text(format!("0x{:08x}", abgr));
//! ```
//!
use std::fmt;

use crate::sys;

/// RGBA color with 32-bit floating point components
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self::new(r, g, b, 1.0)
    }
    pub fn from_rgba_bytes(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self::new(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        )
    }
    pub fn from_rgb_bytes(r: u8, g: u8, b: u8) -> Self {
        Self::from_rgba_bytes(r, g, b, 255)
    }
    /// Construct from an ImGui-packed color (ImU32 ABGR order).
    ///
    /// ImGui packs colors with IM_COL32(R,G,B,A) into `(A<<24)|(B<<16)|(G<<8)|R`.
    /// This converts that ABGR-packed u32 into an RGBA float Color.
    pub fn from_imgui_u32(abgr: u32) -> Self {
        let a = ((abgr >> 24) & 0xFF) as u8;
        let b = ((abgr >> 16) & 0xFF) as u8;
        let g = ((abgr >> 8) & 0xFF) as u8;
        let r = (abgr & 0xFF) as u8;
        Self::from_rgba_bytes(r, g, b, a)
    }

    /// Construct from an opaque 24-bit RGB value (0xRRGGBB).
    pub fn from_rgb_u32(rgb: u32) -> Self {
        Self::from_rgba_bytes(
            ((rgb >> 16) & 0xFF) as u8,
            ((rgb >> 8) & 0xFF) as u8,
            (rgb & 0xFF) as u8,
            255,
        )
    }

    /// Pack to ImGui ImU32 ABGR order `(A<<24)|(B<<16)|(G<<8)|R`.
    pub fn to_imgui_u32(self) -> u32 {
        let r = (self.r * 255.0).round() as u32;
        let g = (self.g * 255.0).round() as u32;
        let b = (self.b * 255.0).round() as u32;
        let a = (self.a * 255.0).round() as u32;
        (a << 24) | (b << 16) | (g << 8) | r
    }
    pub fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
    pub fn from_array(arr: [f32; 4]) -> Self {
        Self::new(arr[0], arr[1], arr[2], arr[3])
    }
    pub fn with_alpha(mut self, alpha: f32) -> Self {
        self.a = alpha;
        self
    }
    pub fn lerp(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self::new(
            self.r + (other.r - self.r) * t,
            self.g + (other.g - self.g) * t,
            self.b + (other.b - self.b) * t,
            self.a + (other.a - self.a) * t,
        )
    }

    /// Convert RGB to HSV using Dear ImGui semantics (h in [0, 1]).
    ///
    /// Note: this differs from [`Color::to_hsv`], which returns hue in degrees.
    #[doc(alias = "ColorConvertRGBtoHSV")]
    pub fn to_hsv01(self) -> (f32, f32, f32) {
        let mut h = 0.0;
        let mut s = 0.0;
        let mut v = 0.0;
        unsafe {
            sys::igColorConvertRGBtoHSV(self.r, self.g, self.b, &mut h, &mut s, &mut v);
        }
        (h, s, v)
    }

    /// Convert HSV to RGB using Dear ImGui semantics (h in [0, 1]).
    ///
    /// Note: this differs from [`Color::from_hsv`], which expects hue in degrees.
    #[doc(alias = "ColorConvertHSVtoRGB")]
    pub fn from_hsv01(h: f32, s: f32, v: f32) -> Self {
        let mut r = 0.0;
        let mut g = 0.0;
        let mut b = 0.0;
        unsafe {
            sys::igColorConvertHSVtoRGB(h, s, v, &mut r, &mut g, &mut b);
        }
        Self::rgb(r, g, b)
    }

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
        Self::new(1.0, 1.0, 1.0, 1.0)
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
    pub const TRANSPARENT: Color = Color::new(0.0, 0.0, 0.0, 0.0);
    pub const BLACK: Color = Color::new(0.0, 0.0, 0.0, 1.0);
    pub const WHITE: Color = Color::new(1.0, 1.0, 1.0, 1.0);
    pub const RED: Color = Color::new(1.0, 0.0, 0.0, 1.0);
    pub const GREEN: Color = Color::new(0.0, 1.0, 0.0, 1.0);
    pub const BLUE: Color = Color::new(0.0, 0.0, 1.0, 1.0);
    pub const YELLOW: Color = Color::new(1.0, 1.0, 0.0, 1.0);
    pub const CYAN: Color = Color::new(0.0, 1.0, 1.0, 1.0);
    pub const MAGENTA: Color = Color::new(1.0, 0.0, 1.0, 1.0);
}

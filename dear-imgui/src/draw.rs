//! Immediate drawing helpers (DrawList)
//!
//! Safe wrappers over Dear ImGui draw lists plus optional low-level primitives
//! for custom geometry. Prefer high-level builders; resort to `prim_*` only
//! when you need exact control and understand the safety requirements.
//!
//! Example (basic drawing):
//! ```no_run
//! # use dear_imgui_rs::*;
//! # let mut ctx = Context::create();
//! # let ui = ctx.frame();
//! ui.with_window_draw_list(|dl| {
//!     dl.add_line([10.0, 10.0], [100.0, 100.0], [1.0, 1.0, 1.0, 1.0])
//!         .thickness(2.0)
//!         .build();
//!     dl.add_text([12.0, 12.0], [1.0, 0.8, 0.2, 1.0], "Hello DrawList");
//! });
//! ```
//!
//! The old public text pixel-snap token and push method were removed. The closure scope is the
//! only public form because directly restored draw-list flags cannot be made correct for every
//! safe Rust drop order.
//!
//! ```compile_fail
//! use dear_imgui_rs::DrawListTextNoPixelSnapToken;
//! ```
//!
//! ```compile_fail
//! use dear_imgui_rs::DrawListMut;
//! fn removed_api(draw_list: &DrawListMut<'_>) {
//!     let _ = draw_list.push_text_no_pixel_snap();
//! }
//! ```
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions,
    clippy::unnecessary_cast
)]

use std::num::NonZeroUsize;

use bitflags::bitflags;

use crate::colors::Color;
use crate::sys;
use crate::validation::{assert_finite_f32, assert_finite_vec2, assert_finite_vec4};

mod list;
mod primitives;

pub use list::{
    ChannelsSplit, DrawListClipRectToken, DrawListMut, DrawListTextureToken, RawDrawCallback,
};
pub use primitives::{BezierCurve, Circle, Line, Polyline, Rect, Triangle};

/// Packed RGBA color compatible with imgui-rs
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ImColor32(u32);

impl ImColor32 {
    /// Convenience constant for solid black.
    pub const BLACK: Self = Self(0xff_00_00_00);
    /// Convenience constant for solid white.
    pub const WHITE: Self = Self(0xff_ff_ff_ff);
    /// Convenience constant for full transparency.
    pub const TRANSPARENT: Self = Self(0);

    /// Construct a color from 4 single-byte `u8` channel values
    #[inline]
    pub const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self(((a as u32) << 24) | (r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
    }

    /// Construct a fully opaque color from 3 single-byte `u8` channel values
    #[inline]
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self::from_rgba(r, g, b, 0xff)
    }

    /// Clamp each channel to `0.0..=1.0` and round to the nearest byte.
    pub fn from_rgba_f32s(r: f32, g: f32, b: f32, a: f32) -> Self {
        let quantize = |channel: f32| (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
        Self::from_rgba(quantize(r), quantize(g), quantize(b), quantize(a))
    }

    /// Return the bits of the color as a u32
    #[inline]
    pub const fn to_bits(self) -> u32 {
        self.0
    }
}

impl From<Color> for ImColor32 {
    fn from(color: Color) -> Self {
        Self::from_rgba_f32s(color.r, color.g, color.b, color.a)
    }
}

impl From<[f32; 4]> for ImColor32 {
    fn from(arr: [f32; 4]) -> Self {
        Self::from_rgba_f32s(arr[0], arr[1], arr[2], arr[3])
    }
}

impl From<(f32, f32, f32, f32)> for ImColor32 {
    fn from((r, g, b, a): (f32, f32, f32, f32)) -> Self {
        Self::from_rgba_f32s(r, g, b, a)
    }
}

impl From<[f32; 3]> for ImColor32 {
    fn from(arr: [f32; 3]) -> Self {
        Self::from_rgba_f32s(arr[0], arr[1], arr[2], 1.0)
    }
}

impl From<(f32, f32, f32)> for ImColor32 {
    fn from((r, g, b): (f32, f32, f32)) -> Self {
        Self::from_rgba_f32s(r, g, b, 1.0)
    }
}

impl From<ImColor32> for u32 {
    fn from(color: ImColor32) -> Self {
        color.0
    }
}

impl From<u32> for ImColor32 {
    fn from(color: u32) -> Self {
        ImColor32(color)
    }
}

bitflags! {
    /// Options for drawing primitives and configuring draw-list scopes.
    ///
    /// Per-primitive flags add to the current draw-list defaults. To disable an
    /// inherited option, use [`DrawListMut::with_draw_flags`]. Stroke
    /// positions are mutually exclusive values: choose one instead of combining them.
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct DrawFlags: u32 {
        const NONE = sys::ImDrawFlags_None as u32;
        const ROUND_TOP_LEFT = sys::ImDrawFlags_RoundTopLeft as u32;
        const ROUND_TOP_RIGHT = sys::ImDrawFlags_RoundTopRight as u32;
        const ROUND_BOTTOM_LEFT = sys::ImDrawFlags_RoundBottomLeft as u32;
        const ROUND_BOTTOM_RIGHT = sys::ImDrawFlags_RoundBottomRight as u32;
        /// Disable corner rounding even when the rounding radius is positive.
        const ROUND_NONE = sys::ImDrawFlags_RoundNone as u32;
        /// Round every corner. This is also the default for a positive radius.
        const ROUND_ALL = sys::ImDrawFlags_RoundAll as u32;
        const ROUND_TOP = sys::ImDrawFlags_RoundTop as u32;
        const ROUND_BOTTOM = sys::ImDrawFlags_RoundBottom as u32;
        const ROUND_LEFT = sys::ImDrawFlags_RoundLeft as u32;
        const ROUND_RIGHT = sys::ImDrawFlags_RoundRight as u32;
        const CLOSED = sys::ImDrawFlags_Closed as u32;
        /// Use miter joins. Avoid this for corners sharper than 90 degrees.
        const JOIN_MITER = sys::ImDrawFlags_JoinMiter as u32;
        const CAP_SQUARE = sys::ImDrawFlags_CapSquare as u32;
        const AA_FILL = sys::ImDrawFlags_AAFill as u32;
        const AA_LINES = sys::ImDrawFlags_AALines as u32;
        /// Anti-alias line ends. Requires anti-aliased lines in the scope or primitive.
        const AA_LINE_ENDS = sys::ImDrawFlags_AALineEnds as u32;
        /// Place the stroke inside the outline; the default for closed shapes.
        const STROKE_INSIDE = sys::ImDrawFlags_StrokeInside as u32;
        /// Center the stroke on the outline; the default for lines and paths.
        const STROKE_CENTER = sys::ImDrawFlags_StrokeCenter as u32;
        /// Center with an integer-pixel bias, useful for axis-aligned shapes.
        const STROKE_CENTER_BIASED = sys::ImDrawFlags_StrokeCenterBiased as u32;
        const STROKE_OUTSIDE = sys::ImDrawFlags_StrokeOutside as u32;
        /// Restore pre-1.93 stroke positioning and line-end behavior.
        const STROKE_LEGACY = sys::ImDrawFlags_StrokeLegacy as u32;
        const TEXT_NO_PIXEL_SNAP = sys::ImDrawFlags_TextNoPixelSnap as u32;
        /// Allow baked rounded-corner textures. Configured through draw-list scopes.
        const USE_TEX_FOR_ROUND_CORNERS = sys::ImDrawFlags_UseTexForRoundCorners as u32;
        /// Allow vertex offsets. Enable only when the renderer supports them.
        const USE_VTX_OFFSET = sys::ImDrawFlags_UseVtxOffset as u32;
    }
}

/// Segment count for draw-list APIs where Dear ImGui accepts `0` as "auto".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DrawSegmentCount(Option<NonZeroUsize>);

impl DrawSegmentCount {
    /// Let Dear ImGui choose the tessellation count.
    pub const AUTO: Self = Self(None);

    #[inline]
    pub const fn auto() -> Self {
        Self::AUTO
    }

    /// Create an explicit positive segment count.
    #[inline]
    pub const fn new(count: usize) -> Option<Self> {
        if count > 0 && count <= i32::MAX as usize {
            match NonZeroUsize::new(count) {
                Some(count) => Some(Self(Some(count))),
                None => None,
            }
        } else {
            None
        }
    }

    /// Create an explicit positive segment count.
    ///
    /// Panics if `count` is zero or exceeds Dear ImGui's `int` range.
    #[inline]
    pub const fn count(count: usize) -> Self {
        match Self::new(count) {
            Some(count) => count,
            None => {
                panic!(
                    "DrawSegmentCount::count() requires a positive count within Dear ImGui's i32 range"
                )
            }
        }
    }

    /// Return the explicit segment count, or `None` for automatic tessellation.
    #[inline]
    pub const fn get(self) -> Option<usize> {
        match self.0 {
            Some(count) => Some(count.get()),
            None => None,
        }
    }

    #[inline]
    fn into_i32(self, caller: &str) -> i32 {
        match self.0 {
            Some(count) => count_to_i32(caller, "num_segments", count.get()),
            None => 0,
        }
    }
}

impl From<NonZeroUsize> for DrawSegmentCount {
    #[inline]
    fn from(value: NonZeroUsize) -> Self {
        Self::new(value.get()).expect("segment count exceeded Dear ImGui's i32 range")
    }
}

/// Segment count for regular n-gon drawing. Dear ImGui requires at least three sides.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DrawNgonSegmentCount(usize);

impl DrawNgonSegmentCount {
    #[inline]
    pub fn new(count: usize) -> Option<Self> {
        (count >= 3 && count <= i32::MAX as usize).then_some(Self(count))
    }

    #[inline]
    pub const fn get(self) -> usize {
        self.0
    }

    #[inline]
    fn into_i32(self, caller: &str) -> i32 {
        assert!(self.0 >= 3, "{caller} num_segments must be at least 3");
        count_to_i32(caller, "num_segments", self.0)
    }
}

impl TryFrom<usize> for DrawNgonSegmentCount {
    type Error = ();

    #[inline]
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Self::new(value).ok_or(())
    }
}

fn assert_non_negative_f32(caller: &str, name: &str, value: f32) {
    assert_finite_f32(caller, name, value);
    assert!(value >= 0.0, "{caller} {name} must be non-negative");
}

fn assert_positive_f32(caller: &str, name: &str, value: f32) {
    assert_finite_f32(caller, name, value);
    assert!(value > 0.0, "{caller} {name} must be positive");
}

fn count_to_i32(caller: &str, name: &str, value: usize) -> i32 {
    i32::try_from(value)
        .unwrap_or_else(|_| panic!("{caller} {name} exceeded Dear ImGui's i32 range"))
}

fn assert_non_negative_vec2(caller: &str, name: &str, value: sys::ImVec2) {
    assert_finite_vec2(caller, name, value.into());
    assert!(
        value.x >= 0.0 && value.y >= 0.0,
        "{caller} {name} must contain non-negative values"
    );
}

fn finite_vec2(caller: &str, name: &str, value: impl Into<sys::ImVec2>) -> sys::ImVec2 {
    let value = value.into();
    assert_finite_vec2(caller, name, value.into());
    value
}

fn finite_point(caller: &str, index: usize, point: impl Into<sys::ImVec2>) -> sys::ImVec2 {
    let point = point.into();
    assert!(
        point.x.is_finite() && point.y.is_finite(),
        "{caller} points[{index}] must contain finite values"
    );
    point
}

fn non_negative_vec2(caller: &str, name: &str, value: impl Into<sys::ImVec2>) -> sys::ImVec2 {
    let value = value.into();
    assert_non_negative_vec2(caller, name, value);
    value
}

fn finite_vec4(caller: &str, name: &str, value: impl Into<sys::ImVec4>) -> sys::ImVec4 {
    let value = value.into();
    assert_finite_vec4(caller, name, value.into());
    value
}

fn assert_path_not_empty(draw_list: *mut sys::ImDrawList, caller: &str) {
    let path_size = unsafe { (*draw_list)._Path.Size };
    assert!(
        path_size > 0,
        "{caller} requires a current path point; call path_line_to() first"
    );
}

fn assert_arc_fast_steps(caller: &str, a_min_of_12: i32, a_max_of_12: i32) {
    assert!(
        (0..=12).contains(&a_min_of_12),
        "{caller} a_min_of_12 must be in 0..=12"
    );
    assert!(
        (0..=12).contains(&a_max_of_12),
        "{caller} a_max_of_12 must be in 0..=12"
    );
}

const STROKE_POSITION: DrawFlags = DrawFlags::from_bits_retain(sys::ImDrawFlags_StrokeMask_ as u32);

const STROKE_FLAGS: DrawFlags = DrawFlags::from_bits_retain(
    STROKE_POSITION.bits()
        | DrawFlags::JOIN_MITER.bits()
        | DrawFlags::CAP_SQUARE.bits()
        | DrawFlags::AA_LINES.bits()
        | DrawFlags::AA_LINE_ENDS.bits(),
);

const FILL_FLAGS: DrawFlags =
    DrawFlags::from_bits_retain(DrawFlags::AA_FILL.bits() | DrawFlags::JOIN_MITER.bits());

const CORNER_FLAGS: DrawFlags = DrawFlags::from_bits_retain(sys::ImDrawFlags_RoundMask_ as u32);

fn assert_draw_flags(caller: &str, flags: DrawFlags, supported: DrawFlags) {
    assert!(
        flags.difference(supported).is_empty(),
        "{caller} flags contain unsupported ImDrawFlags bits"
    );
    let stroke = flags.bits() & sys::ImDrawFlags_StrokeMask_ as u32;
    assert!(
        matches!(stroke >> 17, 0 | 1 | 2 | 3 | 4 | 7),
        "{caller} flags contain an invalid stroke position"
    );
    assert!(
        !flags.contains(DrawFlags::ROUND_NONE) || !flags.intersects(DrawFlags::ROUND_ALL),
        "{caller} ROUND_NONE cannot be combined with rounded corners"
    );
}

fn assert_polyline_flags(caller: &str, flags: DrawFlags) {
    assert_draw_flags(caller, flags, STROKE_FLAGS | FILL_FLAGS | DrawFlags::CLOSED);
}

fn assert_corner_flags(caller: &str, flags: DrawFlags) {
    assert_draw_flags(caller, flags, CORNER_FLAGS);
}

#[cfg(test)]
fn draw_list_counts(draw_list: *mut sys::ImDrawList) -> (i32, i32) {
    unsafe { ((*draw_list).VtxBuffer.Size, (*draw_list).IdxBuffer.Size) }
}

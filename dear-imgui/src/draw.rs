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
//! let dl = ui.get_window_draw_list();
//! dl.add_line([10.0, 10.0], [100.0, 100.0], [1.0, 1.0, 1.0, 1.0])
//!     .thickness(2.0)
//!     .build();
//! dl.add_text([12.0, 12.0], [1.0, 0.8, 0.2, 1.0], "Hello DrawList");
//! ```
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions,
    clippy::unnecessary_cast
)]
use bitflags::bitflags;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::num::NonZeroUsize;

use crate::colors::Color;
use crate::internal::len_i32;
use crate::sys;

thread_local! {
    static BORROWED_DRAW_LISTS: RefCell<Vec<usize>> = RefCell::new(Vec::new());
}

// (MintVec2 legacy alias removed; draw APIs now accept Into<sys::ImVec2>)

fn assert_finite_f32(caller: &str, name: &str, value: f32) {
    assert!(value.is_finite(), "{caller} {name} must be finite");
}

fn assert_non_negative_f32(caller: &str, name: &str, value: f32) {
    assert_finite_f32(caller, name, value);
    assert!(value >= 0.0, "{caller} {name} must be non-negative");
}

fn assert_positive_f32(caller: &str, name: &str, value: f32) {
    assert_finite_f32(caller, name, value);
    assert!(value > 0.0, "{caller} {name} must be positive");
}

fn assert_non_negative_i32(caller: &str, name: &str, value: i32) {
    assert!(value >= 0, "{caller} {name} must be non-negative");
}

fn count_to_i32(caller: &str, name: &str, value: usize) -> i32 {
    i32::try_from(value)
        .unwrap_or_else(|_| panic!("{caller} {name} exceeded Dear ImGui's i32 range"))
}

fn assert_finite_vec2(caller: &str, name: &str, value: sys::ImVec2) {
    assert!(
        value.x.is_finite() && value.y.is_finite(),
        "{caller} {name} must contain finite values"
    );
}

fn assert_non_negative_vec2(caller: &str, name: &str, value: sys::ImVec2) {
    assert_finite_vec2(caller, name, value);
    assert!(
        value.x >= 0.0 && value.y >= 0.0,
        "{caller} {name} must contain non-negative values"
    );
}

fn assert_finite_vec4(caller: &str, name: &str, value: sys::ImVec4) {
    assert!(
        value.x.is_finite() && value.y.is_finite() && value.z.is_finite() && value.w.is_finite(),
        "{caller} {name} must contain finite values"
    );
}

fn finite_vec2(caller: &str, name: &str, value: impl Into<sys::ImVec2>) -> sys::ImVec2 {
    let value = value.into();
    assert_finite_vec2(caller, name, value);
    value
}

fn non_negative_vec2(caller: &str, name: &str, value: impl Into<sys::ImVec2>) -> sys::ImVec2 {
    let value = value.into();
    assert_non_negative_vec2(caller, name, value);
    value
}

fn finite_vec4(caller: &str, name: &str, value: impl Into<sys::ImVec4>) -> sys::ImVec4 {
    let value = value.into();
    assert_finite_vec4(caller, name, value);
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

fn assert_polyline_flags(caller: &str, flags: PolylineFlags) {
    assert!(
        flags.difference(PolylineFlags::CLOSED).is_empty(),
        "{caller} flags contain unsupported ImDrawFlags bits"
    );
}

fn assert_corner_flags(caller: &str, flags: DrawCornerFlags) {
    let supported = sys::ImDrawFlags_RoundCornersMask_ as u32;
    assert!(
        flags.bits() & !supported == 0,
        "{caller} flags contain unsupported ImDrawFlags bits"
    );
}

#[cfg(test)]
fn draw_list_counts(draw_list: *mut sys::ImDrawList) -> (i32, i32) {
    unsafe { ((*draw_list).VtxBuffer.Size, (*draw_list).IdxBuffer.Size) }
}

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

    /// Construct from f32 values in range 0.0..=1.0
    pub fn from_rgba_f32s(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self::from_rgba(
            (r.clamp(0.0, 1.0) * 255.0) as u8,
            (g.clamp(0.0, 1.0) * 255.0) as u8,
            (b.clamp(0.0, 1.0) * 255.0) as u8,
            (a.clamp(0.0, 1.0) * 255.0) as u8,
        )
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

// Removed legacy local Vec2 in favor of passing `impl Into<sys::ImVec2>` and using arrays/tuples.
bitflags! {
    /// Draw list flags
    #[repr(transparent)]
    pub struct DrawListFlags: i32 {
        /// No flags
        const NONE = sys::ImDrawListFlags_None as i32;
        /// Enable anti-aliased lines/borders
        const ANTI_ALIASED_LINES = sys::ImDrawListFlags_AntiAliasedLines as i32;
        /// Enable anti-aliased lines/borders using textures where possible
        const ANTI_ALIASED_LINES_USE_TEX = sys::ImDrawListFlags_AntiAliasedLinesUseTex as i32;
        /// Enable anti-aliased edge around filled shapes
        const ANTI_ALIASED_FILL = sys::ImDrawListFlags_AntiAliasedFill as i32;
        /// Can emit 'VtxOffset > 0' to allow large meshes
        const ALLOW_VTX_OFFSET = sys::ImDrawListFlags_AllowVtxOffset as i32;
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

bitflags! {
    /// Flags accepted by `AddPolyline()` and `PathStroke()`.
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct PolylineFlags: u32 {
        const NONE = sys::ImDrawFlags_None as u32;
        const CLOSED = sys::ImDrawFlags_Closed as u32;
    }
}

impl Default for PolylineFlags {
    fn default() -> Self {
        Self::NONE
    }
}

bitflags! {
    /// Corner rounding flags accepted by rectangle and rounded-image drawing APIs.
    ///
    /// Dear ImGui uses zero as "default to all corners" when `rounding > 0`.
    /// Use [`DrawCornerFlags::NO_ROUNDING`] to explicitly disable rounding.
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct DrawCornerFlags: u32 {
        const DEFAULT = sys::ImDrawFlags_None as u32;
        const TOP_LEFT = sys::ImDrawFlags_RoundCornersTopLeft as u32;
        const TOP_RIGHT = sys::ImDrawFlags_RoundCornersTopRight as u32;
        const BOTTOM_LEFT = sys::ImDrawFlags_RoundCornersBottomLeft as u32;
        const BOTTOM_RIGHT = sys::ImDrawFlags_RoundCornersBottomRight as u32;
        const TOP = sys::ImDrawFlags_RoundCornersTop as u32;
        const BOTTOM = sys::ImDrawFlags_RoundCornersBottom as u32;
        const LEFT = sys::ImDrawFlags_RoundCornersLeft as u32;
        const RIGHT = sys::ImDrawFlags_RoundCornersRight as u32;
        const ALL = sys::ImDrawFlags_RoundCornersAll as u32;
        const NO_ROUNDING = sys::ImDrawFlags_RoundCornersNone as u32;
    }
}

impl Default for DrawCornerFlags {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Object implementing the custom draw API.
///
/// Called from [`Ui::get_window_draw_list`], [`Ui::get_background_draw_list`] or [`Ui::get_foreground_draw_list`].
/// Only one mutable wrapper can exist for the same raw draw list on the same thread at a time.
/// The program will panic when attempting to wrap the same draw list twice.
pub struct DrawListMut<'ui> {
    draw_list: *mut sys::ImDrawList,
    _phantom: PhantomData<&'ui crate::Ui>,
}

struct ChannelsSplitMergeGuard<'ui> {
    draw_list: &'ui DrawListMut<'ui>,
}

impl Drop for ChannelsSplitMergeGuard<'_> {
    fn drop(&mut self) {
        unsafe { sys::ImDrawList_ChannelsMerge(self.draw_list.draw_list) };
    }
}

struct DrawListClipRectGuard<'ui> {
    draw_list: &'ui DrawListMut<'ui>,
}

impl Drop for DrawListClipRectGuard<'_> {
    fn drop(&mut self) {
        unsafe { sys::ImDrawList_PopClipRect(self.draw_list.draw_list) };
    }
}

/// Tracks a texture pushed to a draw-list texture stack.
///
/// The texture is popped when the token is dropped or when [`Self::pop`] is
/// called explicitly.
#[must_use]
pub struct DrawListTextureToken<'draw_list, 'tex> {
    draw_list: *mut sys::ImDrawList,
    _phantom: PhantomData<(&'draw_list (), &'tex mut crate::texture::TextureData)>,
}

impl<'draw_list, 'tex> DrawListTextureToken<'draw_list, 'tex> {
    fn new(draw_list: *mut sys::ImDrawList) -> Self {
        Self {
            draw_list,
            _phantom: PhantomData,
        }
    }

    /// Pop the texture immediately instead of waiting for drop.
    #[doc(alias = "PopTexture")]
    pub fn pop(self) {}
}

impl Drop for DrawListTextureToken<'_, '_> {
    fn drop(&mut self) {
        unsafe { sys::ImDrawList_PopTexture(self.draw_list) }
    }
}

impl Drop for DrawListMut<'_> {
    fn drop(&mut self) {
        let ptr = self.draw_list as usize;
        BORROWED_DRAW_LISTS.with(|borrowed| {
            let mut borrowed = borrowed.borrow_mut();
            if let Some(index) = borrowed.iter().position(|&value| value == ptr) {
                borrowed.swap_remove(index);
            }
        });
    }
}

impl<'ui> DrawListMut<'ui> {
    fn borrow_draw_list(draw_list: *mut sys::ImDrawList) {
        assert!(
            !draw_list.is_null(),
            "DrawListMut::borrow_draw_list() received a null draw list"
        );
        let ptr = draw_list as usize;
        BORROWED_DRAW_LISTS.with(|borrowed| {
            let mut borrowed = borrowed.borrow_mut();
            if borrowed.contains(&ptr) {
                panic!("A DrawListMut is already in use for this draw list");
            }
            borrowed.push(ptr);
        });
    }

    fn from_raw(draw_list: *mut sys::ImDrawList) -> Self {
        Self::borrow_draw_list(draw_list);
        Self {
            draw_list,
            _phantom: PhantomData,
        }
    }

    /// Wrap a raw ImDrawList pointer for the current Dear ImGui frame.
    ///
    /// # Safety
    ///
    /// `draw_list` must be a valid mutable draw-list pointer owned by the active
    /// Dear ImGui frame and remain valid for `'ui`. The caller must also ensure
    /// the pointer is not independently mutated while the returned wrapper is
    /// alive.
    pub unsafe fn from_raw_mut(_ui: &'ui crate::Ui, draw_list: *mut sys::ImDrawList) -> Self {
        Self::from_raw(draw_list)
    }

    pub(crate) fn window(_ui: &'ui crate::Ui) -> Self {
        Self::from_raw(unsafe { sys::igGetWindowDrawList() })
    }

    pub(crate) fn background(_ui: &'ui crate::Ui) -> Self {
        let viewport = unsafe { sys::igGetMainViewport() };
        Self::from_raw(unsafe { sys::igGetBackgroundDrawList(viewport) })
    }

    pub(crate) fn foreground(_ui: &'ui crate::Ui) -> Self {
        let viewport = unsafe { sys::igGetMainViewport() };
        Self::from_raw(unsafe { sys::igGetForegroundDrawList_ViewportPtr(viewport) })
    }
}

/// Drawing functions
impl<'ui> DrawListMut<'ui> {
    /// Split draw into multiple channels and merge automatically at the end of the closure.
    #[doc(alias = "ChannelsSplit")]
    pub fn channels_split<F: FnOnce(&ChannelsSplit<'ui>)>(&'ui self, channels_count: u32, f: F) {
        assert!(channels_count > 0, "channels_count must be greater than 0");
        let channels_count_i32 =
            i32::try_from(channels_count).expect("channels_count exceeded ImGui's i32 range");

        unsafe { sys::ImDrawList_ChannelsSplit(self.draw_list, channels_count_i32) };
        let _merge_guard = ChannelsSplitMergeGuard { draw_list: self };
        f(&ChannelsSplit {
            draw_list: self,
            channels_count,
        });
    }
    /// Returns a line from point `p1` to `p2` with color `c`.
    pub fn add_line<C>(
        &'ui self,
        p1: impl Into<sys::ImVec2>,
        p2: impl Into<sys::ImVec2>,
        c: C,
    ) -> Line<'ui>
    where
        C: Into<ImColor32>,
    {
        Line::new(self, p1, p2, c)
    }

    /// Draw a horizontal line from `min_x` to `max_x` at `y`.
    #[doc(alias = "AddLineH")]
    pub fn add_line_h<C>(&self, min_x: f32, max_x: f32, y: f32, col: C, thickness: f32)
    where
        C: Into<ImColor32>,
    {
        assert_finite_f32("DrawListMut::add_line_h()", "min_x", min_x);
        assert_finite_f32("DrawListMut::add_line_h()", "max_x", max_x);
        assert_finite_f32("DrawListMut::add_line_h()", "y", y);
        assert_positive_f32("DrawListMut::add_line_h()", "thickness", thickness);

        unsafe {
            sys::ImDrawList_AddLineH(
                self.draw_list,
                min_x,
                max_x,
                y,
                col.into().into(),
                thickness,
            )
        }
    }

    /// Draw a vertical line from `min_y` to `max_y` at `x`.
    #[doc(alias = "AddLineV")]
    pub fn add_line_v<C>(&self, x: f32, min_y: f32, max_y: f32, col: C, thickness: f32)
    where
        C: Into<ImColor32>,
    {
        assert_finite_f32("DrawListMut::add_line_v()", "x", x);
        assert_finite_f32("DrawListMut::add_line_v()", "min_y", min_y);
        assert_finite_f32("DrawListMut::add_line_v()", "max_y", max_y);
        assert_positive_f32("DrawListMut::add_line_v()", "thickness", thickness);

        unsafe {
            sys::ImDrawList_AddLineV(
                self.draw_list,
                x,
                min_y,
                max_y,
                col.into().into(),
                thickness,
            )
        }
    }

    /// Returns a rectangle whose upper-left corner is at point `p1`
    /// and lower-right corner is at point `p2`, with color `c`.
    pub fn add_rect<C>(
        &'ui self,
        p1: impl Into<sys::ImVec2>,
        p2: impl Into<sys::ImVec2>,
        c: C,
    ) -> Rect<'ui>
    where
        C: Into<ImColor32>,
    {
        Rect::new(self, p1, p2, c)
    }

    /// Draw a filled rectangle with per-corner colors (counter-clockwise from upper-left).
    #[doc(alias = "AddRectFilledMultiColor")]
    pub fn add_rect_filled_multicolor<C1, C2, C3, C4>(
        &self,
        p1: impl Into<sys::ImVec2>,
        p2: impl Into<sys::ImVec2>,
        col_upr_left: C1,
        col_upr_right: C2,
        col_bot_right: C3,
        col_bot_left: C4,
    ) where
        C1: Into<ImColor32>,
        C2: Into<ImColor32>,
        C3: Into<ImColor32>,
        C4: Into<ImColor32>,
    {
        let p_min = finite_vec2("DrawListMut::add_rect_filled_multicolor()", "p1", p1);
        let p_max = finite_vec2("DrawListMut::add_rect_filled_multicolor()", "p2", p2);
        let c_ul: u32 = col_upr_left.into().into();
        let c_ur: u32 = col_upr_right.into().into();
        let c_br: u32 = col_bot_right.into().into();
        let c_bl: u32 = col_bot_left.into().into();
        unsafe {
            sys::ImDrawList_AddRectFilledMultiColor(
                self.draw_list,
                p_min,
                p_max,
                c_ul,
                c_ur,
                c_br,
                c_bl,
            );
        }
    }

    /// Returns a circle with the given `center`, `radius` and `color`.
    pub fn add_circle<C>(
        &'ui self,
        center: impl Into<sys::ImVec2>,
        radius: f32,
        color: C,
    ) -> Circle<'ui>
    where
        C: Into<ImColor32>,
    {
        Circle::new(self, center, radius, color)
    }

    /// Returns a Bezier curve stretching from `pos0` to `pos1`, whose
    /// curvature is defined by `cp0` and `cp1`.
    #[doc(alias = "AddBezier", alias = "AddBezierCubic")]
    pub fn add_bezier_curve(
        &'ui self,
        pos0: impl Into<sys::ImVec2>,
        cp0: impl Into<sys::ImVec2>,
        cp1: impl Into<sys::ImVec2>,
        pos1: impl Into<sys::ImVec2>,
        color: impl Into<ImColor32>,
    ) -> BezierCurve<'ui> {
        BezierCurve::new(self, pos0, cp0, cp1, pos1, color)
    }

    /// Returns a triangle with the given 3 vertices `p1`, `p2` and `p3` and color `c`.
    #[doc(alias = "AddTriangleFilled", alias = "AddTriangle")]
    pub fn add_triangle<C>(
        &'ui self,
        p1: impl Into<sys::ImVec2>,
        p2: impl Into<sys::ImVec2>,
        p3: impl Into<sys::ImVec2>,
        c: C,
    ) -> Triangle<'ui>
    where
        C: Into<ImColor32>,
    {
        Triangle::new(self, p1, p2, p3, c)
    }

    /// Returns a polygonal line. If filled is rendered as a convex
    /// polygon, if not filled is drawn as a line specified by
    /// [`Polyline::thickness`] (default 1.0)
    #[doc(alias = "AddPolyline", alias = "AddConvexPolyFilled")]
    pub fn add_polyline<C, P>(&'ui self, points: Vec<P>, c: C) -> Polyline<'ui>
    where
        C: Into<ImColor32>,
        P: Into<sys::ImVec2>,
    {
        Polyline::new(self, points, c)
    }

    // ========== Path Drawing Functions ==========

    /// Clear the current path (i.e. start a new path).
    #[doc(alias = "PathClear")]
    pub fn path_clear(&self) {
        unsafe {
            // PathClear is inline: _Path.Size = 0;
            let draw_list = self.draw_list;
            (*draw_list)._Path.Size = 0;
        }
    }

    /// Add a point to the current path.
    #[doc(alias = "PathLineTo")]
    pub fn path_line_to(&self, pos: impl Into<sys::ImVec2>) {
        let pos = finite_vec2("DrawListMut::path_line_to()", "pos", pos);
        unsafe { sys::ImDrawList_PathLineTo(self.draw_list, pos) }
    }

    /// Add a point to the current path, merging duplicate points.
    #[doc(alias = "PathLineToMergeDuplicate")]
    pub fn path_line_to_merge_duplicate(&self, pos: impl Into<sys::ImVec2>) {
        let pos = finite_vec2("DrawListMut::path_line_to_merge_duplicate()", "pos", pos);
        unsafe { sys::ImDrawList_PathLineToMergeDuplicate(self.draw_list, pos) }
    }

    /// Add an arc to the current path.
    #[doc(alias = "PathArcTo")]
    pub fn path_arc_to(
        &self,
        center: impl Into<sys::ImVec2>,
        radius: f32,
        a_min: f32,
        a_max: f32,
        num_segments: impl Into<DrawSegmentCount>,
    ) {
        let center_vec = finite_vec2("DrawListMut::path_arc_to()", "center", center);
        assert_non_negative_f32("DrawListMut::path_arc_to()", "radius", radius);
        assert_finite_f32("DrawListMut::path_arc_to()", "a_min", a_min);
        assert_finite_f32("DrawListMut::path_arc_to()", "a_max", a_max);
        let num_segments = num_segments.into().into_i32("DrawListMut::path_arc_to()");

        unsafe {
            sys::ImDrawList_PathArcTo(
                self.draw_list,
                center_vec,
                radius,
                a_min,
                a_max,
                num_segments,
            );
        }
    }

    /// Add an arc to the current path using fast precomputed angles.
    #[doc(alias = "PathArcToFast")]
    pub fn path_arc_to_fast(
        &self,
        center: impl Into<sys::ImVec2>,
        radius: f32,
        a_min_of_12: i32,
        a_max_of_12: i32,
    ) {
        let center_vec = finite_vec2("DrawListMut::path_arc_to_fast()", "center", center);
        assert_non_negative_f32("DrawListMut::path_arc_to_fast()", "radius", radius);
        assert_arc_fast_steps("DrawListMut::path_arc_to_fast()", a_min_of_12, a_max_of_12);

        unsafe {
            sys::ImDrawList_PathArcToFast(
                self.draw_list,
                center_vec,
                radius,
                a_min_of_12,
                a_max_of_12,
            );
        }
    }

    /// Add a rectangle to the current path.
    #[doc(alias = "PathRect")]
    pub fn path_rect(
        &self,
        rect_min: impl Into<sys::ImVec2>,
        rect_max: impl Into<sys::ImVec2>,
        rounding: f32,
        flags: DrawCornerFlags,
    ) {
        let min_vec = finite_vec2("DrawListMut::path_rect()", "rect_min", rect_min);
        let max_vec = finite_vec2("DrawListMut::path_rect()", "rect_max", rect_max);
        assert_non_negative_f32("DrawListMut::path_rect()", "rounding", rounding);
        assert_corner_flags("DrawListMut::path_rect()", flags);

        unsafe {
            sys::ImDrawList_PathRect(
                self.draw_list,
                min_vec,
                max_vec,
                rounding,
                flags.bits() as sys::ImDrawFlags,
            );
        }
    }

    /// Add an elliptical arc to the current path.
    #[doc(alias = "PathEllipticalArcTo")]
    pub fn path_elliptical_arc_to(
        &self,
        center: impl Into<sys::ImVec2>,
        radius: impl Into<sys::ImVec2>,
        rot: f32,
        a_min: f32,
        a_max: f32,
        num_segments: impl Into<DrawSegmentCount>,
    ) {
        let center = finite_vec2("DrawListMut::path_elliptical_arc_to()", "center", center);
        let radius = non_negative_vec2("DrawListMut::path_elliptical_arc_to()", "radius", radius);
        assert_finite_f32("DrawListMut::path_elliptical_arc_to()", "rot", rot);
        assert_finite_f32("DrawListMut::path_elliptical_arc_to()", "a_min", a_min);
        assert_finite_f32("DrawListMut::path_elliptical_arc_to()", "a_max", a_max);
        let num_segments = num_segments
            .into()
            .into_i32("DrawListMut::path_elliptical_arc_to()");

        unsafe {
            sys::ImDrawList_PathEllipticalArcTo(
                self.draw_list,
                center,
                radius,
                rot,
                a_min,
                a_max,
                num_segments,
            )
        }
    }

    /// Add a quadratic bezier curve to the current path.
    #[doc(alias = "PathBezierQuadraticCurveTo")]
    pub fn path_bezier_quadratic_curve_to(
        &self,
        p2: impl Into<sys::ImVec2>,
        p3: impl Into<sys::ImVec2>,
        num_segments: impl Into<DrawSegmentCount>,
    ) {
        let p2 = finite_vec2("DrawListMut::path_bezier_quadratic_curve_to()", "p2", p2);
        let p3 = finite_vec2("DrawListMut::path_bezier_quadratic_curve_to()", "p3", p3);
        let num_segments = num_segments
            .into()
            .into_i32("DrawListMut::path_bezier_quadratic_curve_to()");
        assert_path_not_empty(
            self.draw_list,
            "DrawListMut::path_bezier_quadratic_curve_to()",
        );

        unsafe { sys::ImDrawList_PathBezierQuadraticCurveTo(self.draw_list, p2, p3, num_segments) }
    }

    /// Add a cubic bezier curve to the current path.
    #[doc(alias = "PathBezierCubicCurveTo")]
    pub fn path_bezier_cubic_curve_to(
        &self,
        p2: impl Into<sys::ImVec2>,
        p3: impl Into<sys::ImVec2>,
        p4: impl Into<sys::ImVec2>,
        num_segments: impl Into<DrawSegmentCount>,
    ) {
        let p2 = finite_vec2("DrawListMut::path_bezier_cubic_curve_to()", "p2", p2);
        let p3 = finite_vec2("DrawListMut::path_bezier_cubic_curve_to()", "p3", p3);
        let p4 = finite_vec2("DrawListMut::path_bezier_cubic_curve_to()", "p4", p4);
        let num_segments = num_segments
            .into()
            .into_i32("DrawListMut::path_bezier_cubic_curve_to()");
        assert_path_not_empty(self.draw_list, "DrawListMut::path_bezier_cubic_curve_to()");

        unsafe { sys::ImDrawList_PathBezierCubicCurveTo(self.draw_list, p2, p3, p4, num_segments) }
    }

    /// Stroke the current path with the specified color and thickness.
    #[doc(alias = "PathStroke")]
    pub fn path_stroke(&self, color: impl Into<ImColor32>, flags: PolylineFlags, thickness: f32) {
        assert_polyline_flags("DrawListMut::path_stroke()", flags);
        assert_positive_f32("DrawListMut::path_stroke()", "thickness", thickness);

        unsafe {
            // PathStroke is inline: AddPolyline(_Path.Data, _Path.Size, col, thickness, flags); _Path.Size = 0;
            let draw_list = self.draw_list;
            let path = &mut (*draw_list)._Path;

            if path.Size > 0 {
                sys::ImDrawList_AddPolyline(
                    self.draw_list,
                    path.Data,
                    path.Size,
                    color.into().into(),
                    thickness,
                    flags.bits() as sys::ImDrawFlags,
                );
                path.Size = 0; // Clear path after stroking
            }
        }
    }

    /// Fill the current path as a convex polygon.
    #[doc(alias = "PathFillConvex")]
    pub fn path_fill_convex(&self, color: impl Into<ImColor32>) {
        unsafe {
            // PathFillConvex is inline: AddConvexPolyFilled(_Path.Data, _Path.Size, col); _Path.Size = 0;
            let draw_list = self.draw_list;
            let path = &mut (*draw_list)._Path;

            if path.Size > 0 {
                sys::ImDrawList_AddConvexPolyFilled(
                    self.draw_list,
                    path.Data,
                    path.Size,
                    color.into().into(),
                );
                path.Size = 0; // Clear path after filling
            }
        }
    }

    /// Draw a text whose upper-left corner is at point `pos`.
    pub fn add_text(
        &self,
        pos: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
        text: impl AsRef<str>,
    ) {
        use std::os::raw::c_char;

        let text = text.as_ref();
        let pos = finite_vec2("DrawListMut::add_text()", "pos", pos);
        let col = col.into();

        unsafe {
            let start = text.as_ptr() as *const c_char;
            let end = (start as usize + text.len()) as *const c_char;
            sys::ImDrawList_AddText_Vec2(self.draw_list, pos, col.into(), start, end);
        }
    }

    /// Draw text with an explicit font and optional fine CPU clip rectangle.
    ///
    /// This mirrors Dear ImGui's `ImDrawList::AddText(ImFont*, ...)` overload.
    #[doc(alias = "AddText")]
    pub fn add_text_with_font(
        &self,
        font: &crate::fonts::Font,
        font_size: f32,
        pos: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
        text: impl AsRef<str>,
        wrap_width: f32,
        cpu_fine_clip_rect: Option<[f32; 4]>,
    ) {
        use std::os::raw::c_char;
        let text = text.as_ref();
        let pos = finite_vec2("DrawListMut::add_text_with_font()", "pos", pos);
        assert_non_negative_f32("DrawListMut::add_text_with_font()", "font_size", font_size);
        assert_non_negative_f32(
            "DrawListMut::add_text_with_font()",
            "wrap_width",
            wrap_width,
        );
        let col = col.into();
        let font_ptr = crate::fonts::validate_font_for_current_context(
            font,
            "DrawListMut::add_text_with_font()",
        );

        let clip_vec4 = cpu_fine_clip_rect
            .map(|r| finite_vec4("DrawListMut::add_text_with_font()", "cpu_fine_clip_rect", r));
        let clip_ptr = match clip_vec4.as_ref() {
            Some(v) => v as *const sys::ImVec4,
            None => std::ptr::null(),
        };

        unsafe {
            let start = text.as_ptr() as *const c_char;
            let end = (start as usize + text.len()) as *const c_char;
            sys::ImDrawList_AddText_FontPtr(
                self.draw_list,
                font_ptr,
                font_size,
                pos,
                col.into(),
                start,
                end,
                wrap_width,
                clip_ptr,
            );
        }
    }

    // channels_split is provided on DrawListMut

    /// Push a texture on the drawlist texture stack (ImGui 1.92+)
    ///
    /// While pushed, image and primitives will use this texture unless otherwise specified.
    ///
    /// Example:
    /// ```no_run
    /// # use dear_imgui_rs::*;
    /// # fn demo(ui: &Ui) {
    /// let dl = ui.get_window_draw_list();
    /// let tex = texture::TextureId::new(1);
    /// unsafe { dl.push_texture(tex) };
    /// dl.add_image(tex, [10.0,10.0], [110.0,110.0], [0.0,0.0], [1.0,1.0], Color::WHITE);
    /// dl.pop_texture();
    /// # }
    /// ```
    #[doc(alias = "PushTexture")]
    ///
    /// # Safety
    ///
    /// The pushed texture reference remains on Dear ImGui's draw-list texture stack until
    /// [`Self::pop_texture`] is called. If this is a managed texture reference, the referenced
    /// texture data must remain valid until the stack entry is popped and any draw commands using it
    /// have been consumed. Prefer [`Self::push_texture_token`] or [`Self::with_texture`] for scoped
    /// safe usage.
    pub unsafe fn push_texture<'tex>(&self, texture: impl Into<crate::texture::TextureRef<'tex>>) {
        let tex_ref = texture.into().raw();
        unsafe { sys::ImDrawList_PushTexture(self.draw_list, tex_ref) }
    }

    /// Push a texture on the draw-list texture stack and return an RAII token.
    ///
    /// Prefer this or [`Self::with_texture`] for scoped usage that remains
    /// balanced if a panic unwinds through the scope. The manual
    /// [`Self::push_texture`] / [`Self::pop_texture`] pair is kept for
    /// compatibility with existing push/pop-style code.
    #[doc(alias = "PushTexture")]
    pub fn push_texture_token<'tex>(
        &self,
        texture: impl Into<crate::texture::TextureRef<'tex>>,
    ) -> DrawListTextureToken<'_, 'tex> {
        unsafe { self.push_texture(texture) };
        DrawListTextureToken::new(self.draw_list)
    }

    /// Pop the last texture from the drawlist texture stack (ImGui 1.92+)
    #[doc(alias = "PopTexture")]
    pub fn pop_texture(&self) {
        unsafe {
            sys::ImDrawList_PopTexture(self.draw_list);
        }
    }

    /// Push a texture, run `f`, then pop the texture.
    ///
    /// The texture is popped during unwinding if `f` panics.
    #[doc(alias = "PushTexture", alias = "PopTexture")]
    pub fn with_texture<'tex, R>(
        &self,
        texture: impl Into<crate::texture::TextureRef<'tex>>,
        f: impl FnOnce() -> R,
    ) -> R {
        let _texture = self.push_texture_token(texture);
        f()
    }

    /// Push a clip rectangle, optionally intersecting with the current clip rect.
    #[doc(alias = "PushClipRect")]
    pub fn push_clip_rect(
        &self,
        clip_rect_min: impl Into<sys::ImVec2>,
        clip_rect_max: impl Into<sys::ImVec2>,
        intersect_with_current: bool,
    ) {
        let clip_rect_min = finite_vec2(
            "DrawListMut::push_clip_rect()",
            "clip_rect_min",
            clip_rect_min,
        );
        let clip_rect_max = finite_vec2(
            "DrawListMut::push_clip_rect()",
            "clip_rect_max",
            clip_rect_max,
        );

        unsafe {
            sys::ImDrawList_PushClipRect(
                self.draw_list,
                clip_rect_min,
                clip_rect_max,
                intersect_with_current,
            )
        }
    }

    /// Push a full-screen clip rectangle.
    #[doc(alias = "PushClipRectFullScreen")]
    pub fn push_clip_rect_full_screen(&self) {
        unsafe { sys::ImDrawList_PushClipRectFullScreen(self.draw_list) }
    }

    /// Pop the last clip rectangle.
    #[doc(alias = "PopClipRect")]
    pub fn pop_clip_rect(&self) {
        unsafe { sys::ImDrawList_PopClipRect(self.draw_list) }
    }

    /// Get current minimum clip rectangle point.
    pub fn clip_rect_min(&self) -> [f32; 2] {
        let out = unsafe { sys::ImDrawList_GetClipRectMin(self.draw_list) };
        out.into()
    }

    /// Get current maximum clip rectangle point.
    pub fn clip_rect_max(&self) -> [f32; 2] {
        let out = unsafe { sys::ImDrawList_GetClipRectMax(self.draw_list) };
        out.into()
    }

    /// Convenience: push a clip rect, run f, pop.
    pub fn with_clip_rect<F>(
        &self,
        clip_rect_min: impl Into<sys::ImVec2>,
        clip_rect_max: impl Into<sys::ImVec2>,
        f: F,
    ) where
        F: FnOnce(),
    {
        self.push_clip_rect(clip_rect_min, clip_rect_max, false);
        let _clip_rect_guard = DrawListClipRectGuard { draw_list: self };
        f();
    }

    /// Add an image quad (axis-aligned). Tint via `col`.
    #[doc(alias = "AddImage")]
    pub fn add_image<'tex>(
        &self,
        texture: impl Into<crate::texture::TextureRef<'tex>>,
        p_min: impl Into<sys::ImVec2>,
        p_max: impl Into<sys::ImVec2>,
        uv_min: impl Into<sys::ImVec2>,
        uv_max: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
    ) {
        // Example:
        // let tex = texture::TextureId::new(5);
        // self.add_image(tex, [10.0,10.0], [110.0,110.0], [0.0,0.0], [1.0,1.0], Color::WHITE);
        let p_min = finite_vec2("DrawListMut::add_image()", "p_min", p_min);
        let p_max = finite_vec2("DrawListMut::add_image()", "p_max", p_max);
        let uv_min = finite_vec2("DrawListMut::add_image()", "uv_min", uv_min);
        let uv_max = finite_vec2("DrawListMut::add_image()", "uv_max", uv_max);
        let col = col.into().to_bits();
        let tex_ref = texture.into().raw();
        unsafe {
            sys::ImDrawList_AddImage(self.draw_list, tex_ref, p_min, p_max, uv_min, uv_max, col)
        }
    }

    /// Add an image with 4 arbitrary corners.
    #[doc(alias = "AddImageQuad")]
    pub fn add_image_quad<'tex>(
        &self,
        texture: impl Into<crate::texture::TextureRef<'tex>>,
        p1: impl Into<sys::ImVec2>,
        p2: impl Into<sys::ImVec2>,
        p3: impl Into<sys::ImVec2>,
        p4: impl Into<sys::ImVec2>,
        uv1: impl Into<sys::ImVec2>,
        uv2: impl Into<sys::ImVec2>,
        uv3: impl Into<sys::ImVec2>,
        uv4: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
    ) {
        // Example:
        // let tex = texture::TextureId::new(5);
        // self.add_image_quad(
        //     tex,
        //     [10.0,10.0], [110.0,20.0], [120.0,120.0], [5.0,100.0],
        //     [0.0,0.0], [1.0,0.0], [1.0,1.0], [0.0,1.0],
        //     Color::WHITE,
        // );
        let p1 = finite_vec2("DrawListMut::add_image_quad()", "p1", p1);
        let p2 = finite_vec2("DrawListMut::add_image_quad()", "p2", p2);
        let p3 = finite_vec2("DrawListMut::add_image_quad()", "p3", p3);
        let p4 = finite_vec2("DrawListMut::add_image_quad()", "p4", p4);
        let uv1 = finite_vec2("DrawListMut::add_image_quad()", "uv1", uv1);
        let uv2 = finite_vec2("DrawListMut::add_image_quad()", "uv2", uv2);
        let uv3 = finite_vec2("DrawListMut::add_image_quad()", "uv3", uv3);
        let uv4 = finite_vec2("DrawListMut::add_image_quad()", "uv4", uv4);
        let col = col.into().to_bits();
        let tex_ref = texture.into().raw();
        unsafe {
            sys::ImDrawList_AddImageQuad(
                self.draw_list,
                tex_ref,
                p1,
                p2,
                p3,
                p4,
                uv1,
                uv2,
                uv3,
                uv4,
                col,
            )
        }
    }

    /// Add an axis-aligned rounded image.
    #[doc(alias = "AddImageRounded")]
    pub fn add_image_rounded<'tex>(
        &self,
        texture: impl Into<crate::texture::TextureRef<'tex>>,
        p_min: impl Into<sys::ImVec2>,
        p_max: impl Into<sys::ImVec2>,
        uv_min: impl Into<sys::ImVec2>,
        uv_max: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
        rounding: f32,
        flags: DrawCornerFlags,
    ) {
        // Example:
        // let tex = texture::TextureId::new(5);
        // self.add_image_rounded(
        //     tex,
        //     [10.0,10.0], [110.0,110.0],
        //     [0.0,0.0], [1.0,1.0],
        //     Color::WHITE,
        //     8.0,
        //     DrawCornerFlags::ALL,
        // );
        let p_min = finite_vec2("DrawListMut::add_image_rounded()", "p_min", p_min);
        let p_max = finite_vec2("DrawListMut::add_image_rounded()", "p_max", p_max);
        let uv_min = finite_vec2("DrawListMut::add_image_rounded()", "uv_min", uv_min);
        let uv_max = finite_vec2("DrawListMut::add_image_rounded()", "uv_max", uv_max);
        assert_non_negative_f32("DrawListMut::add_image_rounded()", "rounding", rounding);
        assert_corner_flags("DrawListMut::add_image_rounded()", flags);
        let col = col.into().to_bits();
        let tex_ref = texture.into().raw();
        unsafe {
            sys::ImDrawList_AddImageRounded(
                self.draw_list,
                tex_ref,
                p_min,
                p_max,
                uv_min,
                uv_max,
                col,
                rounding,
                flags.bits() as sys::ImDrawFlags,
            )
        }
    }

    /// Draw a quadrilateral outline given four points.
    #[doc(alias = "AddQuad")]
    pub fn add_quad<C>(
        &self,
        p1: impl Into<sys::ImVec2>,
        p2: impl Into<sys::ImVec2>,
        p3: impl Into<sys::ImVec2>,
        p4: impl Into<sys::ImVec2>,
        col: C,
        thickness: f32,
    ) where
        C: Into<ImColor32>,
    {
        let p1 = finite_vec2("DrawListMut::add_quad()", "p1", p1);
        let p2 = finite_vec2("DrawListMut::add_quad()", "p2", p2);
        let p3 = finite_vec2("DrawListMut::add_quad()", "p3", p3);
        let p4 = finite_vec2("DrawListMut::add_quad()", "p4", p4);
        assert_positive_f32("DrawListMut::add_quad()", "thickness", thickness);

        unsafe {
            sys::ImDrawList_AddQuad(self.draw_list, p1, p2, p3, p4, col.into().into(), thickness)
        }
    }

    /// Draw a filled quadrilateral given four points.
    #[doc(alias = "AddQuadFilled")]
    pub fn add_quad_filled<C>(
        &self,
        p1: impl Into<sys::ImVec2>,
        p2: impl Into<sys::ImVec2>,
        p3: impl Into<sys::ImVec2>,
        p4: impl Into<sys::ImVec2>,
        col: C,
    ) where
        C: Into<ImColor32>,
    {
        let p1 = finite_vec2("DrawListMut::add_quad_filled()", "p1", p1);
        let p2 = finite_vec2("DrawListMut::add_quad_filled()", "p2", p2);
        let p3 = finite_vec2("DrawListMut::add_quad_filled()", "p3", p3);
        let p4 = finite_vec2("DrawListMut::add_quad_filled()", "p4", p4);

        unsafe { sys::ImDrawList_AddQuadFilled(self.draw_list, p1, p2, p3, p4, col.into().into()) }
    }

    /// Draw a regular n-gon outline.
    #[doc(alias = "AddNgon")]
    pub fn add_ngon<C>(
        &self,
        center: impl Into<sys::ImVec2>,
        radius: f32,
        col: C,
        num_segments: DrawNgonSegmentCount,
        thickness: f32,
    ) where
        C: Into<ImColor32>,
    {
        let center = finite_vec2("DrawListMut::add_ngon()", "center", center);
        assert_non_negative_f32("DrawListMut::add_ngon()", "radius", radius);
        assert_positive_f32("DrawListMut::add_ngon()", "thickness", thickness);
        let num_segments = num_segments.into_i32("DrawListMut::add_ngon()");

        unsafe {
            sys::ImDrawList_AddNgon(
                self.draw_list,
                center,
                radius,
                col.into().into(),
                num_segments,
                thickness,
            )
        }
    }

    /// Draw a filled regular n-gon.
    #[doc(alias = "AddNgonFilled")]
    pub fn add_ngon_filled<C>(
        &self,
        center: impl Into<sys::ImVec2>,
        radius: f32,
        col: C,
        num_segments: DrawNgonSegmentCount,
    ) where
        C: Into<ImColor32>,
    {
        let center = finite_vec2("DrawListMut::add_ngon_filled()", "center", center);
        assert_non_negative_f32("DrawListMut::add_ngon_filled()", "radius", radius);
        let num_segments = num_segments.into_i32("DrawListMut::add_ngon_filled()");

        unsafe {
            sys::ImDrawList_AddNgonFilled(
                self.draw_list,
                center,
                radius,
                col.into().into(),
                num_segments,
            )
        }
    }

    /// Draw an ellipse outline.
    #[doc(alias = "AddEllipse")]
    pub fn add_ellipse<C>(
        &self,
        center: impl Into<sys::ImVec2>,
        radius: impl Into<sys::ImVec2>,
        col: C,
        rot: f32,
        num_segments: impl Into<DrawSegmentCount>,
        thickness: f32,
    ) where
        C: Into<ImColor32>,
    {
        let center = finite_vec2("DrawListMut::add_ellipse()", "center", center);
        let radius = non_negative_vec2("DrawListMut::add_ellipse()", "radius", radius);
        assert_finite_f32("DrawListMut::add_ellipse()", "rot", rot);
        assert_positive_f32("DrawListMut::add_ellipse()", "thickness", thickness);
        let num_segments = num_segments.into().into_i32("DrawListMut::add_ellipse()");

        unsafe {
            sys::ImDrawList_AddEllipse(
                self.draw_list,
                center,
                radius,
                col.into().into(),
                rot,
                num_segments,
                thickness,
            )
        }
    }

    /// Draw a filled ellipse.
    #[doc(alias = "AddEllipseFilled")]
    pub fn add_ellipse_filled<C>(
        &self,
        center: impl Into<sys::ImVec2>,
        radius: impl Into<sys::ImVec2>,
        col: C,
        rot: f32,
        num_segments: impl Into<DrawSegmentCount>,
    ) where
        C: Into<ImColor32>,
    {
        let center = finite_vec2("DrawListMut::add_ellipse_filled()", "center", center);
        let radius = non_negative_vec2("DrawListMut::add_ellipse_filled()", "radius", radius);
        assert_finite_f32("DrawListMut::add_ellipse_filled()", "rot", rot);
        let num_segments = num_segments
            .into()
            .into_i32("DrawListMut::add_ellipse_filled()");

        unsafe {
            sys::ImDrawList_AddEllipseFilled(
                self.draw_list,
                center,
                radius,
                col.into().into(),
                rot,
                num_segments,
            )
        }
    }

    /// Draw a quadratic Bezier curve directly.
    #[doc(alias = "AddBezierQuadratic")]
    pub fn add_bezier_quadratic<C>(
        &self,
        p1: impl Into<sys::ImVec2>,
        p2: impl Into<sys::ImVec2>,
        p3: impl Into<sys::ImVec2>,
        col: C,
        thickness: f32,
        num_segments: impl Into<DrawSegmentCount>,
    ) where
        C: Into<ImColor32>,
    {
        let p1 = finite_vec2("DrawListMut::add_bezier_quadratic()", "p1", p1);
        let p2 = finite_vec2("DrawListMut::add_bezier_quadratic()", "p2", p2);
        let p3 = finite_vec2("DrawListMut::add_bezier_quadratic()", "p3", p3);
        assert_positive_f32(
            "DrawListMut::add_bezier_quadratic()",
            "thickness",
            thickness,
        );
        let num_segments = num_segments
            .into()
            .into_i32("DrawListMut::add_bezier_quadratic()");

        unsafe {
            sys::ImDrawList_AddBezierQuadratic(
                self.draw_list,
                p1,
                p2,
                p3,
                col.into().into(),
                thickness,
                num_segments,
            )
        }
    }

    /// Fill a concave polygon (Dear ImGui 1.92+).
    #[doc(alias = "AddConcavePolyFilled")]
    pub fn add_concave_poly_filled<C, P>(&self, points: &[P], col: C)
    where
        C: Into<ImColor32>,
        P: Copy + Into<sys::ImVec2>,
    {
        let count = len_i32(
            "DrawListMut::add_concave_poly_filled()",
            "points",
            points.len(),
        );
        let mut buf: Vec<sys::ImVec2> = Vec::with_capacity(points.len());
        for (i, p) in points.iter().copied().enumerate() {
            let name = format!("points[{i}]");
            buf.push(finite_vec2(
                "DrawListMut::add_concave_poly_filled()",
                &name,
                p,
            ));
        }
        unsafe {
            sys::ImDrawList_AddConcavePolyFilled(
                self.draw_list,
                buf.as_ptr(),
                count,
                col.into().into(),
            )
        }
    }

    /// Fill the current path as a concave polygon (Dear ImGui 1.92+).
    #[doc(alias = "PathFillConcave")]
    pub fn path_fill_concave(&self, color: impl Into<ImColor32>) {
        unsafe { sys::ImDrawList_PathFillConcave(self.draw_list, color.into().into()) }
    }

    /// Insert a raw draw callback.
    ///
    /// # Safety
    ///
    /// - `callback` must be an `extern "C"` function compatible with `ImDrawCallback` and must not unwind
    ///   across the FFI boundary.
    /// - `userdata` must remain valid until the draw list is executed by the renderer.
    /// - If you allocate memory and store its pointer in `userdata`, you are responsible for reclaiming it
    ///   from within the callback or otherwise ensuring no leaks occur. Note that callbacks are only invoked
    ///   if the draw list is actually rendered.
    #[doc(alias = "AddCallback")]
    pub unsafe fn add_callback(
        &self,
        callback: sys::ImDrawCallback,
        userdata: *mut std::os::raw::c_void,
        userdata_size: usize,
    ) {
        unsafe { sys::ImDrawList_AddCallback(self.draw_list, callback, userdata, userdata_size) }
    }

    /// Insert a new draw command (forces a new draw call boundary).
    #[doc(alias = "AddDrawCmd")]
    pub fn add_draw_cmd(&self) {
        unsafe { sys::ImDrawList_AddDrawCmd(self.draw_list) }
    }

    /// Clone the current draw list output into an owned, independent copy.
    ///
    /// The returned draw list is heap-allocated by Dear ImGui and will be destroyed on drop.
    ///
    /// # Panics
    ///
    /// Panics if the draw list contains user callbacks. Dear ImGui's clone operation copies
    /// callback userdata as an opaque pointer, which cannot be duplicated safely by this safe API.
    #[doc(alias = "CloneOutput")]
    pub fn clone_output(&self) -> crate::render::OwnedDrawList {
        unsafe {
            crate::render::draw_data::assert_draw_list_cloneable(
                self.draw_list.cast_const(),
                "DrawListMut::clone_output",
            );
            crate::render::OwnedDrawList::from_raw(sys::ImDrawList_CloneOutput(self.draw_list))
        }
    }
}

/// Represent the drawing interface within a call to `channels_split`.
pub struct ChannelsSplit<'ui> {
    draw_list: &'ui DrawListMut<'ui>,
    channels_count: u32,
}

impl ChannelsSplit<'_> {
    /// Change current channel. Panics if `channel_index >= channels_count`.
    #[doc(alias = "ChannelsSetCurrent")]
    pub fn set_current(&self, channel_index: u32) {
        assert!(
            channel_index < self.channels_count,
            "Channel index {} out of range {}",
            channel_index,
            self.channels_count
        );
        unsafe {
            sys::ImDrawList_ChannelsSetCurrent(self.draw_list.draw_list, channel_index as i32)
        };
    }
}

/// A safe builder for registering a Rust callback to be executed during draw.
#[must_use = "call .build() to register the callback"]
pub struct Callback<'ui, F> {
    draw_list: &'ui DrawListMut<'ui>,
    callback: F,
}

impl<'ui, F: FnOnce() + 'static> Callback<'ui, F> {
    /// Construct a new callback builder. Typically created via `DrawListMut::add_callback_safe`.
    pub fn new(draw_list: &'ui DrawListMut<'_>, callback: F) -> Self {
        Self {
            draw_list,
            callback,
        }
    }

    /// Register the callback with the draw list.
    pub fn build(self) {
        use std::os::raw::c_void;
        // Box the closure so we can pass an owning pointer to C.
        //
        // Note: Dear ImGui's `ImDrawList::AddCallback()` optionally copies `userdata` bytes into an
        // internal unaligned byte buffer when `userdata_size != 0`. That mode is suitable only for
        // plain-old-data payloads; it must not be used for Rust closures.
        let ptr: *mut F = Box::into_raw(Box::new(self.callback));
        unsafe {
            sys::ImDrawList_AddCallback(
                self.draw_list.draw_list,
                Some(Self::run_callback),
                ptr as *mut c_void,
                0,
            );
        }
    }

    unsafe extern "C" fn run_callback(
        _parent_list: *const sys::ImDrawList,
        cmd: *const sys::ImDrawCmd,
    ) {
        if cmd.is_null() {
            return;
        }
        let cmd_ptr = cmd as *mut sys::ImDrawCmd;
        if unsafe { (*cmd_ptr).UserCallbackData.is_null() } {
            return;
        }
        if unsafe { (*cmd_ptr).UserCallbackDataOffset } != -1 {
            eprintln!("dear-imgui-rs: unexpected UserCallbackDataOffset (expected -1)");
            std::process::abort();
        }
        if unsafe { (*cmd_ptr).UserCallbackDataSize } != 0 {
            eprintln!("dear-imgui-rs: unexpected UserCallbackDataSize (expected 0)");
            std::process::abort();
        }
        // Compute pointer to our boxed closure (respect offset if ever used)
        let data_ptr = unsafe { (*cmd_ptr).UserCallbackData as *mut F };
        if data_ptr.is_null() {
            return;
        }
        // Take ownership and clear the pointer/size to avoid double-free or re-entry
        unsafe {
            (*cmd_ptr).UserCallbackData = std::ptr::null_mut();
            (*cmd_ptr).UserCallbackDataSize = 0;
            (*cmd_ptr).UserCallbackDataOffset = 0;
        }
        let cb = unsafe { Box::from_raw(data_ptr) };
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            cb();
        }));
        if res.is_err() {
            eprintln!("dear-imgui-rs: panic in DrawList callback");
            std::process::abort();
        }
    }
}

#[cfg(test)]
mod callback_tests {
    use super::*;

    #[test]
    fn safe_draw_callback_uses_direct_user_data_pointer() {
        fn noop() {}

        let shared = unsafe { sys::ImDrawListSharedData_ImDrawListSharedData() };
        assert!(!shared.is_null());
        let raw_draw_list = unsafe { sys::ImDrawList_ImDrawList(shared) };
        assert!(!raw_draw_list.is_null());

        // Ensure CmdBuffer.Size > 0 (required by AddCallback).
        unsafe { sys::ImDrawList_AddDrawCmd(raw_draw_list) };

        let draw_list = DrawListMut {
            draw_list: raw_draw_list,
            _phantom: PhantomData,
        };
        draw_list.add_callback_safe(noop).build();

        let cmd_buffer = unsafe { &(*draw_list.draw_list).CmdBuffer };
        assert!(cmd_buffer.Size > 0);
        assert!(!cmd_buffer.Data.is_null());

        let (cmd_ptr, cmd_copy) = {
            let cmds = unsafe {
                let len = usize::try_from(cmd_buffer.Size)
                    .expect("expected non-negative CmdBuffer.Size in test");
                std::slice::from_raw_parts(cmd_buffer.Data, len)
            };
            let (i, cmd) = cmds
                .iter()
                .enumerate()
                .find(|(_, cmd)| cmd.UserCallback.is_some() && !cmd.UserCallbackData.is_null())
                .expect("expected callback command to be present");

            let cmd_ptr = unsafe { cmd_buffer.Data.add(i) as *const sys::ImDrawCmd };
            (cmd_ptr, *cmd)
        };

        assert!(cmd_copy.UserCallback.is_some());
        assert_eq!(cmd_copy.UserCallbackDataOffset, -1);
        assert_eq!(cmd_copy.UserCallbackDataSize, 0);
        assert!(!cmd_copy.UserCallbackData.is_null());

        // Run the callback once to reclaim the boxed closure and avoid leaking in the test.
        unsafe { cmd_copy.UserCallback.unwrap()(draw_list.draw_list as *const _, cmd_ptr) }

        let cmd_after = unsafe { *cmd_ptr };
        assert!(cmd_after.UserCallbackData.is_null());

        unsafe {
            sys::ImDrawList_destroy(raw_draw_list);
            sys::ImDrawListSharedData_destroy(shared);
        }
    }

    #[test]
    fn clone_output_rejects_user_callbacks() {
        fn noop() {}

        let shared = unsafe { sys::ImDrawListSharedData_ImDrawListSharedData() };
        assert!(!shared.is_null());
        let raw_draw_list = unsafe { sys::ImDrawList_ImDrawList(shared) };
        assert!(!raw_draw_list.is_null());

        unsafe { sys::ImDrawList_AddDrawCmd(raw_draw_list) };

        let draw_list = DrawListMut {
            draw_list: raw_draw_list,
            _phantom: PhantomData,
        };
        draw_list.add_callback_safe(noop).build();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = draw_list.clone_output();
        }));
        assert!(result.is_err());

        let cmd_buffer = unsafe { &(*draw_list.draw_list).CmdBuffer };
        let cmd_ptr = {
            let cmds = unsafe {
                let len = usize::try_from(cmd_buffer.Size)
                    .expect("expected non-negative CmdBuffer.Size in test");
                std::slice::from_raw_parts(cmd_buffer.Data, len)
            };
            let (i, _) = cmds
                .iter()
                .enumerate()
                .find(|(_, cmd)| cmd.UserCallback.is_some() && !cmd.UserCallbackData.is_null())
                .expect("expected callback command to be present");

            unsafe { cmd_buffer.Data.add(i) as *const sys::ImDrawCmd }
        };
        let cmd_copy = unsafe { *cmd_ptr };
        unsafe { cmd_copy.UserCallback.unwrap()(draw_list.draw_list as *const _, cmd_ptr) }

        unsafe {
            sys::ImDrawList_destroy(raw_draw_list);
            sys::ImDrawListSharedData_destroy(shared);
        }
    }
}

#[cfg(test)]
mod channels_tests {
    use super::*;

    #[test]
    fn with_clip_rect_pops_after_panic() {
        let mut ctx = crate::Context::create();
        {
            let io = ctx.io_mut();
            io.set_display_size([128.0, 128.0]);
            io.set_delta_time(1.0 / 60.0);
        }
        let _ = ctx.font_atlas_mut().build();
        let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

        let ui = ctx.frame();
        let draw_list = ui.get_window_draw_list();
        let raw_draw_list = draw_list.draw_list;
        let initial_stack_size = unsafe { (*raw_draw_list)._ClipRectStack.Size };

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            draw_list.with_clip_rect([0.0, 0.0], [8.0, 8.0], || {
                assert_eq!(
                    unsafe { (*raw_draw_list)._ClipRectStack.Size },
                    initial_stack_size + 1
                );
                panic!("forced panic while draw-list clip rect is pushed");
            });
        }));

        assert!(result.is_err());
        assert_eq!(
            unsafe { (*raw_draw_list)._ClipRectStack.Size },
            initial_stack_size
        );
    }

    #[test]
    fn with_texture_pops_after_panic() {
        let mut ctx = crate::Context::create();
        {
            let io = ctx.io_mut();
            io.set_display_size([128.0, 128.0]);
            io.set_delta_time(1.0 / 60.0);
        }
        let _ = ctx.font_atlas_mut().build();
        let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

        let ui = ctx.frame();
        let draw_list = ui.get_window_draw_list();
        let raw_draw_list = draw_list.draw_list;
        let initial_stack_size = unsafe { (*raw_draw_list)._TextureStack.Size };

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            draw_list.with_texture(crate::texture::TextureId::new(1), || {
                assert_eq!(
                    unsafe { (*raw_draw_list)._TextureStack.Size },
                    initial_stack_size + 1
                );
                panic!("forced panic while draw-list texture is pushed");
            });
        }));

        assert!(result.is_err());
        assert_eq!(
            unsafe { (*raw_draw_list)._TextureStack.Size },
            initial_stack_size
        );
    }

    #[test]
    fn channels_split_merges_after_panic() {
        let shared = unsafe { sys::ImDrawListSharedData_ImDrawListSharedData() };
        assert!(!shared.is_null());
        let raw_draw_list = unsafe { sys::ImDrawList_ImDrawList(shared) };
        assert!(!raw_draw_list.is_null());

        unsafe { sys::ImDrawList_AddDrawCmd(raw_draw_list) };

        let draw_list = DrawListMut {
            draw_list: raw_draw_list,
            _phantom: PhantomData,
        };

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            draw_list.channels_split(2, |channels| {
                channels.set_current(1);
                panic!("forced panic while channels are split");
            });
        }));

        assert!(result.is_err());
        unsafe {
            assert_eq!((*raw_draw_list)._Splitter._Count, 1);
            assert_eq!((*raw_draw_list)._Splitter._Current, 0);
        }

        unsafe {
            sys::ImDrawList_destroy(raw_draw_list);
            sys::ImDrawListSharedData_destroy(shared);
        }
    }

    #[test]
    fn channels_split_rejects_zero_channels() {
        let shared = unsafe { sys::ImDrawListSharedData_ImDrawListSharedData() };
        assert!(!shared.is_null());
        let raw_draw_list = unsafe { sys::ImDrawList_ImDrawList(shared) };
        assert!(!raw_draw_list.is_null());

        let draw_list = DrawListMut {
            draw_list: raw_draw_list,
            _phantom: PhantomData,
        };
        let initial_count = unsafe { (*raw_draw_list)._Splitter._Count };
        let initial_current = unsafe { (*raw_draw_list)._Splitter._Current };

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            draw_list.channels_split(0, |_| {});
        }));

        assert!(result.is_err());
        unsafe {
            assert_eq!((*raw_draw_list)._Splitter._Count, initial_count);
            assert_eq!((*raw_draw_list)._Splitter._Current, initial_current);
        }

        unsafe {
            sys::ImDrawList_destroy(raw_draw_list);
            sys::ImDrawListSharedData_destroy(shared);
        }
    }

    #[test]
    fn draw_list_point_count_helpers_reject_overflow() {
        assert!(
            std::panic::catch_unwind(|| {
                let _ = len_i32("Polyline::build()", "points", (i32::MAX as usize) + 1);
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| {
                let _ = len_i32(
                    "DrawListMut::add_concave_poly_filled()",
                    "points",
                    (i32::MAX as usize) + 1,
                );
            })
            .is_err()
        );
    }
}

#[cfg(test)]
mod draw_numeric_tests {
    use super::*;

    struct TestDrawList {
        shared: *mut sys::ImDrawListSharedData,
        raw: *mut sys::ImDrawList,
    }

    impl TestDrawList {
        fn new() -> Self {
            let shared = unsafe { sys::ImDrawListSharedData_ImDrawListSharedData() };
            assert!(!shared.is_null());
            let raw = unsafe { sys::ImDrawList_ImDrawList(shared) };
            assert!(!raw.is_null());
            Self { shared, raw }
        }

        fn draw_list(&self) -> DrawListMut<'static> {
            DrawListMut {
                draw_list: self.raw,
                _phantom: PhantomData,
            }
        }

        fn path_size(&self) -> i32 {
            unsafe { (*self.raw)._Path.Size }
        }

        fn clip_stack_size(&self) -> i32 {
            unsafe { (*self.raw)._ClipRectStack.Size }
        }
    }

    impl Drop for TestDrawList {
        fn drop(&mut self) {
            unsafe {
                sys::ImDrawList_destroy(self.raw);
                sys::ImDrawListSharedData_destroy(self.shared);
            }
        }
    }

    fn assert_panics_without_buffer_change(
        fixture: &TestDrawList,
        f: impl FnOnce(&DrawListMut<'static>),
    ) {
        let draw_list = fixture.draw_list();
        let before = draw_list_counts(fixture.raw);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&draw_list)));
        assert!(result.is_err());
        assert_eq!(draw_list_counts(fixture.raw), before);
    }

    #[test]
    fn direct_draw_inputs_validate_before_ffi() {
        let fixture = TestDrawList::new();

        assert_panics_without_buffer_change(&fixture, |draw_list| {
            draw_list.add_line_h(f32::NAN, 1.0, 0.0, ImColor32::WHITE, 1.0);
        });
        assert_panics_without_buffer_change(&fixture, |draw_list| {
            draw_list.add_line_v(0.0, 0.0, 1.0, ImColor32::WHITE, 0.0);
        });
        assert_panics_without_buffer_change(&fixture, |draw_list| {
            draw_list.add_quad(
                [f32::NAN, 0.0],
                [1.0, 0.0],
                [1.0, 1.0],
                [0.0, 1.0],
                ImColor32::WHITE,
                1.0,
            );
        });
        assert_panics_without_buffer_change(&fixture, |draw_list| {
            draw_list.add_ellipse(
                [0.0, 0.0],
                [-1.0, 4.0],
                ImColor32::WHITE,
                0.0,
                DrawSegmentCount::AUTO,
                1.0,
            );
        });
        assert_panics_without_buffer_change(&fixture, |draw_list| {
            draw_list.add_bezier_quadratic(
                [0.0, 0.0],
                [1.0, 1.0],
                [2.0, 0.0],
                ImColor32::WHITE,
                1.0,
                DrawSegmentCount::count(0),
            );
        });
        assert_panics_without_buffer_change(&fixture, |draw_list| {
            draw_list.add_image_rounded(
                crate::texture::TextureId::new(1),
                [0.0, 0.0],
                [16.0, 16.0],
                [0.0, 0.0],
                [1.0, 1.0],
                ImColor32::WHITE,
                f32::INFINITY,
                DrawCornerFlags::ALL,
            );
        });
        assert_panics_without_buffer_change(&fixture, |draw_list| {
            draw_list.path_rect(
                [0.0, 0.0],
                [1.0, 1.0],
                1.0,
                DrawCornerFlags::from_bits_retain(sys::ImDrawFlags_Closed as u32),
            );
        });
    }

    #[test]
    fn path_inputs_validate_before_path_mutation() {
        let fixture = TestDrawList::new();
        let draw_list = fixture.draw_list();

        draw_list.path_line_to([1.0, 1.0]);
        let path_size = fixture.path_size();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            draw_list.path_line_to([f32::NAN, 2.0]);
        }));
        assert!(result.is_err());
        assert_eq!(fixture.path_size(), path_size);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            draw_list.path_arc_to([0.0, 0.0], -1.0, 0.0, 1.0, DrawSegmentCount::AUTO);
        }));
        assert!(result.is_err());
        assert_eq!(fixture.path_size(), path_size);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            draw_list.path_arc_to_fast([0.0, 0.0], 1.0, 0, 13);
        }));
        assert!(result.is_err());
        assert_eq!(fixture.path_size(), path_size);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            draw_list.path_bezier_quadratic_curve_to(
                [2.0, 2.0],
                [3.0, 3.0],
                DrawSegmentCount::count(0),
            );
        }));
        assert!(result.is_err());
        assert_eq!(fixture.path_size(), path_size);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            draw_list.path_stroke(
                ImColor32::WHITE,
                PolylineFlags::from_bits_retain(sys::ImDrawFlags_RoundCornersTopLeft as u32),
                1.0,
            );
        }));
        assert!(result.is_err());
        assert_eq!(fixture.path_size(), path_size);

        draw_list.path_clear();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            draw_list.path_bezier_cubic_curve_to(
                [2.0, 2.0],
                [3.0, 3.0],
                [4.0, 4.0],
                DrawSegmentCount::AUTO,
            );
        }));
        assert!(result.is_err());
        assert_eq!(fixture.path_size(), 0);
    }

    #[test]
    fn builder_inputs_validate_before_ffi() {
        let fixture = TestDrawList::new();

        assert_panics_without_buffer_change(&fixture, |draw_list| {
            let _ = draw_list
                .add_line([0.0, 0.0], [1.0, 1.0], ImColor32::WHITE)
                .thickness(0.0);
        });
        assert_panics_without_buffer_change(&fixture, |draw_list| {
            let _ = draw_list.add_circle([0.0, 0.0], -1.0, ImColor32::WHITE);
        });
        assert_panics_without_buffer_change(&fixture, |draw_list| {
            let _ = draw_list
                .add_rect([0.0, 0.0], [1.0, 1.0], ImColor32::WHITE)
                .rounding(f32::NAN);
        });
        assert_panics_without_buffer_change(&fixture, |draw_list| {
            let _ =
                draw_list.add_polyline(vec![[0.0, 0.0], [f32::INFINITY, 1.0]], ImColor32::WHITE);
        });
        assert_panics_without_buffer_change(&fixture, |draw_list| {
            let _ = draw_list
                .add_bezier_curve(
                    [0.0, 0.0],
                    [1.0, 1.0],
                    [2.0, 1.0],
                    [3.0, 0.0],
                    ImColor32::WHITE,
                )
                .num_segments(DrawSegmentCount::count(i32::MAX as usize + 1));
        });
    }

    #[test]
    fn draw_segment_counts_reject_invalid_values_before_ffi() {
        assert!(DrawNgonSegmentCount::new(2).is_none());
        assert!(DrawNgonSegmentCount::new(3).is_some());
        assert!(DrawNgonSegmentCount::new(i32::MAX as usize + 1).is_none());

        assert_eq!(DrawSegmentCount::AUTO.get(), None);
        assert_eq!(
            DrawSegmentCount::new(3).and_then(DrawSegmentCount::get),
            Some(3)
        );
        assert_eq!(DrawSegmentCount::new(0), None);
        assert_eq!(DrawSegmentCount::new(i32::MAX as usize + 1), None);

        assert!(std::panic::catch_unwind(|| DrawSegmentCount::count(0)).is_err());
        assert!(
            std::panic::catch_unwind(|| DrawSegmentCount::count(i32::MAX as usize + 1)).is_err()
        );
    }

    #[test]
    fn text_and_clip_inputs_validate_before_ffi() {
        let mut ctx = crate::Context::create();
        {
            let io = ctx.io_mut();
            io.set_display_size([128.0, 128.0]);
            io.set_delta_time(1.0 / 60.0);
        }
        let _ = ctx.font_atlas_mut().build();
        let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);

        let ui = ctx.frame();
        let draw_list = ui.get_window_draw_list();
        let raw_draw_list = draw_list.draw_list;
        let font = ui.current_font();
        let before = draw_list_counts(raw_draw_list);
        let clip_stack_size = unsafe { (*raw_draw_list)._ClipRectStack.Size };

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            draw_list.add_text([f32::NAN, 0.0], ImColor32::WHITE, "hello");
        }));
        assert!(result.is_err());
        assert_eq!(draw_list_counts(raw_draw_list), before);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            draw_list.add_text_with_font(
                font,
                -1.0,
                [0.0, 0.0],
                ImColor32::WHITE,
                "hello",
                0.0,
                None,
            );
        }));
        assert!(result.is_err());
        assert_eq!(draw_list_counts(raw_draw_list), before);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            draw_list.push_clip_rect([f32::NAN, 0.0], [1.0, 1.0], false);
        }));
        assert!(result.is_err());
        assert_eq!(
            unsafe { (*raw_draw_list)._ClipRectStack.Size },
            clip_stack_size
        );
    }

    #[test]
    fn raw_draw_list_clip_helper_reads_stack_without_mutation() {
        let fixture = TestDrawList::new();
        let before = fixture.clip_stack_size();
        let draw_list = fixture.draw_list();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            draw_list.push_clip_rect([0.0, 0.0], [f32::INFINITY, 1.0], false);
        }));

        assert!(result.is_err());
        assert_eq!(fixture.clip_stack_size(), before);
    }
}

impl<'ui> DrawListMut<'ui> {
    /// Safe variant: add a Rust callback (executed when the draw list is rendered).
    /// Note: if the draw list is never rendered, the callback will not run and its resources won't be reclaimed.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn add_callback_safe<F: FnOnce() + 'static>(&'ui self, callback: F) -> Callback<'ui, F> {
        Callback::new(self, callback)
    }

    /// Safe variant: add a Rust callback (executed when the draw list is rendered).
    ///
    /// On wasm32 targets using the import-style Dear ImGui provider, C code cannot
    /// safely invoke Rust function pointers across module boundaries. For now this
    /// API is disabled on wasm to avoid undefined behaviour; use other mechanisms
    /// (e.g. higher-level rendering hooks) instead.
    #[cfg(target_arch = "wasm32")]
    pub fn add_callback_safe<F: FnOnce() + 'static>(&'ui self, _callback: F) -> Callback<'ui, F> {
        panic!(
            "DrawListMut::add_callback_safe is not supported on wasm32 targets; \
             C->Rust callbacks are not available in the import-style web build."
        );
    }
}

impl<'ui> DrawListMut<'ui> {
    /// Unsafe low-level geometry API: reserve index and vertex space.
    ///
    /// # Safety
    /// Caller must write exactly the reserved amount using `prim_write_*` and ensure valid topology.
    pub unsafe fn prim_reserve(&self, idx_count: i32, vtx_count: i32) {
        assert_non_negative_i32("DrawListMut::prim_reserve()", "idx_count", idx_count);
        assert_non_negative_i32("DrawListMut::prim_reserve()", "vtx_count", vtx_count);
        unsafe { sys::ImDrawList_PrimReserve(self.draw_list, idx_count, vtx_count) }
    }

    /// Unsafe low-level geometry API: unreserve previously reserved space.
    ///
    /// # Safety
    /// Must match a prior call to `prim_reserve` which hasn't been fully written.
    pub unsafe fn prim_unreserve(&self, idx_count: i32, vtx_count: i32) {
        assert_non_negative_i32("DrawListMut::prim_unreserve()", "idx_count", idx_count);
        assert_non_negative_i32("DrawListMut::prim_unreserve()", "vtx_count", vtx_count);
        unsafe { sys::ImDrawList_PrimUnreserve(self.draw_list, idx_count, vtx_count) }
    }

    /// Unsafe low-level geometry API: append a rectangle primitive with a single color.
    ///
    /// # Safety
    /// Only use between `prim_reserve` and completing the reserved writes.
    pub unsafe fn prim_rect(
        &self,
        a: impl Into<sys::ImVec2>,
        b: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
    ) {
        let a = finite_vec2("DrawListMut::prim_rect()", "a", a);
        let b = finite_vec2("DrawListMut::prim_rect()", "b", b);
        unsafe { sys::ImDrawList_PrimRect(self.draw_list, a, b, col.into().into()) }
    }

    /// Unsafe low-level geometry API: append a rectangle primitive with UVs and color.
    ///
    /// # Safety
    /// Only use between `prim_reserve` and completing the reserved writes.
    pub unsafe fn prim_rect_uv(
        &self,
        a: impl Into<sys::ImVec2>,
        b: impl Into<sys::ImVec2>,
        uv_a: impl Into<sys::ImVec2>,
        uv_b: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
    ) {
        let a = finite_vec2("DrawListMut::prim_rect_uv()", "a", a);
        let b = finite_vec2("DrawListMut::prim_rect_uv()", "b", b);
        let uv_a = finite_vec2("DrawListMut::prim_rect_uv()", "uv_a", uv_a);
        let uv_b = finite_vec2("DrawListMut::prim_rect_uv()", "uv_b", uv_b);

        unsafe { sys::ImDrawList_PrimRectUV(self.draw_list, a, b, uv_a, uv_b, col.into().into()) }
    }

    /// Unsafe low-level geometry API: append a quad primitive with UVs and color.
    ///
    /// # Safety
    /// Only use between `prim_reserve` and completing the reserved writes.
    pub unsafe fn prim_quad_uv(
        &self,
        a: impl Into<sys::ImVec2>,
        b: impl Into<sys::ImVec2>,
        c: impl Into<sys::ImVec2>,
        d: impl Into<sys::ImVec2>,
        uv_a: impl Into<sys::ImVec2>,
        uv_b: impl Into<sys::ImVec2>,
        uv_c: impl Into<sys::ImVec2>,
        uv_d: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
    ) {
        let a = finite_vec2("DrawListMut::prim_quad_uv()", "a", a);
        let b = finite_vec2("DrawListMut::prim_quad_uv()", "b", b);
        let c = finite_vec2("DrawListMut::prim_quad_uv()", "c", c);
        let d = finite_vec2("DrawListMut::prim_quad_uv()", "d", d);
        let uv_a = finite_vec2("DrawListMut::prim_quad_uv()", "uv_a", uv_a);
        let uv_b = finite_vec2("DrawListMut::prim_quad_uv()", "uv_b", uv_b);
        let uv_c = finite_vec2("DrawListMut::prim_quad_uv()", "uv_c", uv_c);
        let uv_d = finite_vec2("DrawListMut::prim_quad_uv()", "uv_d", uv_d);

        unsafe {
            sys::ImDrawList_PrimQuadUV(
                self.draw_list,
                a,
                b,
                c,
                d,
                uv_a,
                uv_b,
                uv_c,
                uv_d,
                col.into().into(),
            )
        }
    }

    /// Unsafe low-level geometry API: write a vertex.
    ///
    /// # Safety
    /// Only use to fill space reserved by `prim_reserve`.
    pub unsafe fn prim_write_vtx(
        &self,
        pos: impl Into<sys::ImVec2>,
        uv: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
    ) {
        let pos = finite_vec2("DrawListMut::prim_write_vtx()", "pos", pos);
        let uv = finite_vec2("DrawListMut::prim_write_vtx()", "uv", uv);
        unsafe { sys::ImDrawList_PrimWriteVtx(self.draw_list, pos, uv, col.into().into()) }
    }

    /// Unsafe low-level geometry API: write an index.
    ///
    /// # Safety
    /// Only use to fill space reserved by `prim_reserve`.
    pub unsafe fn prim_write_idx(&self, idx: sys::ImDrawIdx) {
        unsafe { sys::ImDrawList_PrimWriteIdx(self.draw_list, idx) }
    }

    /// Unsafe low-level geometry API: convenience to append one vertex (pos+uv+col).
    ///
    /// # Safety
    /// Only use between `prim_reserve` and completing the reserved writes.
    pub unsafe fn prim_vtx(
        &self,
        pos: impl Into<sys::ImVec2>,
        uv: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
    ) {
        let pos = finite_vec2("DrawListMut::prim_vtx()", "pos", pos);
        let uv = finite_vec2("DrawListMut::prim_vtx()", "uv", uv);
        unsafe { sys::ImDrawList_PrimVtx(self.draw_list, pos, uv, col.into().into()) }
    }
}

/// Represents a line about to be drawn
#[must_use = "should call .build() to draw the object"]
pub struct Line<'ui> {
    p1: [f32; 2],
    p2: [f32; 2],
    color: ImColor32,
    thickness: f32,
    draw_list: &'ui DrawListMut<'ui>,
}

impl<'ui> Line<'ui> {
    fn new<C>(
        draw_list: &'ui DrawListMut<'_>,
        p1: impl Into<sys::ImVec2>,
        p2: impl Into<sys::ImVec2>,
        c: C,
    ) -> Self
    where
        C: Into<ImColor32>,
    {
        Self {
            p1: finite_vec2("Line::new()", "p1", p1).into(),
            p2: finite_vec2("Line::new()", "p2", p2).into(),
            color: c.into(),
            thickness: 1.0,
            draw_list,
        }
    }

    /// Set line's thickness (default to 1.0 pixel)
    pub fn thickness(mut self, thickness: f32) -> Self {
        assert_positive_f32("Line::thickness()", "thickness", thickness);
        self.thickness = thickness;
        self
    }

    /// Draw the line on the window
    pub fn build(self) {
        unsafe {
            let p1 = sys::ImVec2 {
                x: self.p1[0],
                y: self.p1[1],
            };
            let p2 = sys::ImVec2 {
                x: self.p2[0],
                y: self.p2[1],
            };
            sys::ImDrawList_AddLine(
                self.draw_list.draw_list,
                p1,
                p2,
                self.color.into(),
                self.thickness,
            )
        }
    }
}

/// Represents a rectangle about to be drawn
#[must_use = "should call .build() to draw the object"]
pub struct Rect<'ui> {
    p1: [f32; 2],
    p2: [f32; 2],
    color: ImColor32,
    rounding: f32,
    flags: DrawCornerFlags,
    thickness: f32,
    filled: bool,
    draw_list: &'ui DrawListMut<'ui>,
}

impl<'ui> Rect<'ui> {
    fn new<C>(
        draw_list: &'ui DrawListMut<'_>,
        p1: impl Into<sys::ImVec2>,
        p2: impl Into<sys::ImVec2>,
        c: C,
    ) -> Self
    where
        C: Into<ImColor32>,
    {
        Self {
            p1: finite_vec2("Rect::new()", "p1", p1).into(),
            p2: finite_vec2("Rect::new()", "p2", p2).into(),
            color: c.into(),
            rounding: 0.0,
            flags: DrawCornerFlags::ALL,
            thickness: 1.0,
            filled: false,
            draw_list,
        }
    }

    /// Set rectangle's corner rounding (default to 0.0 = no rounding)
    pub fn rounding(mut self, rounding: f32) -> Self {
        assert_non_negative_f32("Rect::rounding()", "rounding", rounding);
        self.rounding = rounding;
        self
    }

    /// Set rectangle's thickness (default to 1.0 pixel). Has no effect if filled
    pub fn thickness(mut self, thickness: f32) -> Self {
        assert_positive_f32("Rect::thickness()", "thickness", thickness);
        self.thickness = thickness;
        self
    }

    /// Draw rectangle as filled
    pub fn filled(mut self, filled: bool) -> Self {
        self.filled = filled;
        self
    }

    /// Set rectangle's corner rounding flags
    pub fn flags(mut self, flags: DrawCornerFlags) -> Self {
        assert_corner_flags("Rect::flags()", flags);
        self.flags = flags;
        self
    }

    /// Draw the rectangle on the window
    pub fn build(self) {
        let p1 = sys::ImVec2 {
            x: self.p1[0],
            y: self.p1[1],
        };
        let p2 = sys::ImVec2 {
            x: self.p2[0],
            y: self.p2[1],
        };

        if self.filled {
            unsafe {
                sys::ImDrawList_AddRectFilled(
                    self.draw_list.draw_list,
                    p1,
                    p2,
                    self.color.into(),
                    self.rounding,
                    self.flags.bits() as sys::ImDrawFlags,
                )
            }
        } else {
            unsafe {
                sys::ImDrawList_AddRect(
                    self.draw_list.draw_list,
                    p1,
                    p2,
                    self.color.into(),
                    self.rounding,
                    self.thickness,
                    self.flags.bits() as sys::ImDrawFlags,
                )
            }
        }
    }
}

/// Represents a circle about to be drawn
#[must_use = "should call .build() to draw the object"]
pub struct Circle<'ui> {
    center: [f32; 2],
    radius: f32,
    color: ImColor32,
    num_segments: DrawSegmentCount,
    thickness: f32,
    filled: bool,
    draw_list: &'ui DrawListMut<'ui>,
}

impl<'ui> Circle<'ui> {
    fn new<C>(
        draw_list: &'ui DrawListMut<'_>,
        center: impl Into<sys::ImVec2>,
        radius: f32,
        color: C,
    ) -> Self
    where
        C: Into<ImColor32>,
    {
        assert_non_negative_f32("Circle::new()", "radius", radius);
        Self {
            center: finite_vec2("Circle::new()", "center", center).into(),
            radius,
            color: color.into(),
            num_segments: DrawSegmentCount::AUTO,
            thickness: 1.0,
            filled: false,
            draw_list,
        }
    }

    /// Set circle's thickness (default to 1.0 pixel). Has no effect if filled
    pub fn thickness(mut self, thickness: f32) -> Self {
        assert_positive_f32("Circle::thickness()", "thickness", thickness);
        self.thickness = thickness;
        self
    }

    /// Draw circle as filled
    pub fn filled(mut self, filled: bool) -> Self {
        self.filled = filled;
        self
    }

    /// Set number of segments (default is automatic tessellation).
    pub fn num_segments(mut self, num_segments: impl Into<DrawSegmentCount>) -> Self {
        self.num_segments = num_segments.into();
        self
    }

    /// Draw the circle on the window
    pub fn build(self) {
        let center = sys::ImVec2 {
            x: self.center[0],
            y: self.center[1],
        };
        let num_segments = self.num_segments.into_i32("Circle::num_segments()");

        if self.filled {
            unsafe {
                sys::ImDrawList_AddCircleFilled(
                    self.draw_list.draw_list,
                    center,
                    self.radius,
                    self.color.into(),
                    num_segments,
                )
            }
        } else {
            unsafe {
                sys::ImDrawList_AddCircle(
                    self.draw_list.draw_list,
                    center,
                    self.radius,
                    self.color.into(),
                    num_segments,
                    self.thickness,
                )
            }
        }
    }
}

/// Represents a Bezier curve about to be drawn
#[must_use = "should call .build() to draw the object"]
pub struct BezierCurve<'ui> {
    pos0: [f32; 2],
    cp0: [f32; 2],
    pos1: [f32; 2],
    cp1: [f32; 2],
    color: ImColor32,
    thickness: f32,
    /// If num_segments is not set, the bezier curve is auto-tessalated.
    num_segments: DrawSegmentCount,
    draw_list: &'ui DrawListMut<'ui>,
}

impl<'ui> BezierCurve<'ui> {
    /// Typically constructed by [`DrawListMut::add_bezier_curve`]
    pub fn new<C>(
        draw_list: &'ui DrawListMut<'_>,
        pos0: impl Into<sys::ImVec2>,
        cp0: impl Into<sys::ImVec2>,
        cp1: impl Into<sys::ImVec2>,
        pos1: impl Into<sys::ImVec2>,
        c: C,
    ) -> Self
    where
        C: Into<ImColor32>,
    {
        Self {
            pos0: finite_vec2("BezierCurve::new()", "pos0", pos0).into(),
            cp0: finite_vec2("BezierCurve::new()", "cp0", cp0).into(),
            cp1: finite_vec2("BezierCurve::new()", "cp1", cp1).into(),
            pos1: finite_vec2("BezierCurve::new()", "pos1", pos1).into(),
            color: c.into(),
            thickness: 1.0,
            num_segments: DrawSegmentCount::AUTO,
            draw_list,
        }
    }

    /// Set curve's thickness (default to 1.0 pixel)
    pub fn thickness(mut self, thickness: f32) -> Self {
        assert_positive_f32("BezierCurve::thickness()", "thickness", thickness);
        self.thickness = thickness;
        self
    }

    /// Set number of segments used to draw the Bezier curve. If not set, the
    /// bezier curve is auto-tessalated.
    pub fn num_segments(mut self, num_segments: impl Into<DrawSegmentCount>) -> Self {
        self.num_segments = num_segments.into();
        self
    }

    /// Draw the curve on the window.
    pub fn build(self) {
        unsafe {
            let pos0: sys::ImVec2 = self.pos0.into();
            let cp0: sys::ImVec2 = self.cp0.into();
            let cp1: sys::ImVec2 = self.cp1.into();
            let pos1: sys::ImVec2 = self.pos1.into();

            sys::ImDrawList_AddBezierCubic(
                self.draw_list.draw_list,
                pos0,
                cp0,
                cp1,
                pos1,
                self.color.into(),
                self.thickness,
                self.num_segments.into_i32("BezierCurve::num_segments()"),
            )
        }
    }
}

/// Represents a poly line about to be drawn
#[must_use = "should call .build() to draw the object"]
pub struct Polyline<'ui> {
    points: Vec<sys::ImVec2>,
    thickness: f32,
    flags: PolylineFlags,
    filled: bool,
    color: ImColor32,
    draw_list: &'ui DrawListMut<'ui>,
}

impl<'ui> Polyline<'ui> {
    fn new<C, P>(draw_list: &'ui DrawListMut<'_>, points: Vec<P>, c: C) -> Self
    where
        C: Into<ImColor32>,
        P: Into<sys::ImVec2>,
    {
        Self {
            points: points
                .into_iter()
                .enumerate()
                .map(|(i, point)| {
                    let name = format!("points[{i}]");
                    finite_vec2("Polyline::new()", &name, point)
                })
                .collect(),
            color: c.into(),
            thickness: 1.0,
            flags: PolylineFlags::NONE,
            filled: false,
            draw_list,
        }
    }

    /// Set line's thickness (default to 1.0 pixel). Has no effect if
    /// shape is filled
    pub fn thickness(mut self, thickness: f32) -> Self {
        assert_positive_f32("Polyline::thickness()", "thickness", thickness);
        self.thickness = thickness;
        self
    }

    /// Set polyline flags. Has no effect if shape is filled.
    pub fn flags(mut self, flags: PolylineFlags) -> Self {
        assert_polyline_flags("Polyline::flags()", flags);
        self.flags = flags;
        self
    }

    /// Draw the polyline as a closed shape. Has no effect if shape is filled.
    pub fn closed(mut self, closed: bool) -> Self {
        self.flags.set(PolylineFlags::CLOSED, closed);
        self
    }

    /// Draw shape as filled convex polygon
    pub fn filled(mut self, filled: bool) -> Self {
        self.filled = filled;
        self
    }

    /// Draw the line on the window
    pub fn build(self) {
        let count = len_i32("Polyline::build()", "points", self.points.len());
        if self.filled {
            unsafe {
                sys::ImDrawList_AddConvexPolyFilled(
                    self.draw_list.draw_list,
                    self.points.as_ptr(),
                    count,
                    self.color.into(),
                )
            }
        } else {
            unsafe {
                sys::ImDrawList_AddPolyline(
                    self.draw_list.draw_list,
                    self.points.as_ptr(),
                    count,
                    self.color.into(),
                    self.thickness,
                    self.flags.bits() as sys::ImDrawFlags,
                )
            }
        }
    }
}

/// Represents a triangle about to be drawn on the window
#[must_use = "should call .build() to draw the object"]
pub struct Triangle<'ui> {
    p1: [f32; 2],
    p2: [f32; 2],
    p3: [f32; 2],
    color: ImColor32,
    thickness: f32,
    filled: bool,
    draw_list: &'ui DrawListMut<'ui>,
}

impl<'ui> Triangle<'ui> {
    fn new<C>(
        draw_list: &'ui DrawListMut<'_>,
        p1: impl Into<sys::ImVec2>,
        p2: impl Into<sys::ImVec2>,
        p3: impl Into<sys::ImVec2>,
        c: C,
    ) -> Self
    where
        C: Into<ImColor32>,
    {
        Self {
            p1: finite_vec2("Triangle::new()", "p1", p1).into(),
            p2: finite_vec2("Triangle::new()", "p2", p2).into(),
            p3: finite_vec2("Triangle::new()", "p3", p3).into(),
            color: c.into(),
            thickness: 1.0,
            filled: false,
            draw_list,
        }
    }

    /// Set triangle's thickness (default to 1.0 pixel)
    pub fn thickness(mut self, thickness: f32) -> Self {
        assert_positive_f32("Triangle::thickness()", "thickness", thickness);
        self.thickness = thickness;
        self
    }

    /// Set to `true` to make a filled triangle (default to `false`).
    pub fn filled(mut self, filled: bool) -> Self {
        self.filled = filled;
        self
    }

    /// Draw the triangle on the window.
    pub fn build(self) {
        let p1 = sys::ImVec2 {
            x: self.p1[0],
            y: self.p1[1],
        };
        let p2 = sys::ImVec2 {
            x: self.p2[0],
            y: self.p2[1],
        };
        let p3 = sys::ImVec2 {
            x: self.p3[0],
            y: self.p3[1],
        };

        if self.filled {
            unsafe {
                sys::ImDrawList_AddTriangleFilled(
                    self.draw_list.draw_list,
                    p1,
                    p2,
                    p3,
                    self.color.into(),
                )
            }
        } else {
            unsafe {
                sys::ImDrawList_AddTriangle(
                    self.draw_list.draw_list,
                    p1,
                    p2,
                    p3,
                    self.color.into(),
                    self.thickness,
                )
            }
        }
    }
}

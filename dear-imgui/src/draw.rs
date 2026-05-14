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

use crate::colors::Color;
use crate::sys;

thread_local! {
    static BORROWED_DRAW_LISTS: RefCell<Vec<usize>> = RefCell::new(Vec::new());
}

// (MintVec2 legacy alias removed; draw APIs now accept Into<sys::ImVec2>)

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
pub struct DrawListTextureToken<'draw_list> {
    draw_list: *mut sys::ImDrawList,
    _phantom: PhantomData<&'draw_list ()>,
}

impl<'draw_list> DrawListTextureToken<'draw_list> {
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

impl Drop for DrawListTextureToken<'_> {
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
        let p_min: sys::ImVec2 = p1.into();
        let p_max: sys::ImVec2 = p2.into();
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
        unsafe { sys::ImDrawList_PathLineTo(self.draw_list, pos.into()) }
    }

    /// Add a point to the current path, merging duplicate points.
    #[doc(alias = "PathLineToMergeDuplicate")]
    pub fn path_line_to_merge_duplicate(&self, pos: impl Into<sys::ImVec2>) {
        unsafe { sys::ImDrawList_PathLineToMergeDuplicate(self.draw_list, pos.into()) }
    }

    /// Add an arc to the current path.
    #[doc(alias = "PathArcTo")]
    pub fn path_arc_to(
        &self,
        center: impl Into<sys::ImVec2>,
        radius: f32,
        a_min: f32,
        a_max: f32,
        num_segments: i32,
    ) {
        unsafe {
            let center_vec: sys::ImVec2 = center.into();
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
        unsafe {
            let center_vec: sys::ImVec2 = center.into();
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
        unsafe {
            let min_vec: sys::ImVec2 = rect_min.into();
            let max_vec: sys::ImVec2 = rect_max.into();
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
        num_segments: i32,
    ) {
        unsafe {
            sys::ImDrawList_PathEllipticalArcTo(
                self.draw_list,
                center.into(),
                radius.into(),
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
        num_segments: i32,
    ) {
        unsafe {
            sys::ImDrawList_PathBezierQuadraticCurveTo(
                self.draw_list,
                p2.into(),
                p3.into(),
                num_segments,
            )
        }
    }

    /// Add a cubic bezier curve to the current path.
    #[doc(alias = "PathBezierCubicCurveTo")]
    pub fn path_bezier_cubic_curve_to(
        &self,
        p2: impl Into<sys::ImVec2>,
        p3: impl Into<sys::ImVec2>,
        p4: impl Into<sys::ImVec2>,
        num_segments: i32,
    ) {
        unsafe {
            sys::ImDrawList_PathBezierCubicCurveTo(
                self.draw_list,
                p2.into(),
                p3.into(),
                p4.into(),
                num_segments,
            )
        }
    }

    /// Stroke the current path with the specified color and thickness.
    #[doc(alias = "PathStroke")]
    pub fn path_stroke(&self, color: impl Into<ImColor32>, flags: PolylineFlags, thickness: f32) {
        unsafe {
            // PathStroke is inline: AddPolyline(_Path.Data, _Path.Size, col, flags, thickness); _Path.Size = 0;
            let draw_list = self.draw_list;
            let path = &mut (*draw_list)._Path;

            if path.Size > 0 {
                sys::ImDrawList_AddPolyline(
                    self.draw_list,
                    path.Data,
                    path.Size,
                    color.into().into(),
                    flags.bits() as sys::ImDrawFlags,
                    thickness,
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
        let pos: sys::ImVec2 = pos.into();
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
        let pos: sys::ImVec2 = pos.into();
        let col = col.into();
        let font_ptr = font.raw();

        let clip_vec4 = cpu_fine_clip_rect.map(|r| sys::ImVec4 {
            x: r[0],
            y: r[1],
            z: r[2],
            w: r[3],
        });
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
    /// dl.push_texture(tex);
    /// dl.add_image(tex, [10.0,10.0], [110.0,110.0], [0.0,0.0], [1.0,1.0], Color::WHITE);
    /// dl.pop_texture();
    /// # }
    /// ```
    #[doc(alias = "PushTexture")]
    pub fn push_texture(&self, texture: impl Into<crate::texture::TextureRef>) {
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
    pub fn push_texture_token(
        &self,
        texture: impl Into<crate::texture::TextureRef>,
    ) -> DrawListTextureToken<'_> {
        self.push_texture(texture);
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
    pub fn with_texture<R>(
        &self,
        texture: impl Into<crate::texture::TextureRef>,
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
        unsafe {
            sys::ImDrawList_PushClipRect(
                self.draw_list,
                clip_rect_min.into(),
                clip_rect_max.into(),
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
    pub fn add_image(
        &self,
        texture: impl Into<crate::texture::TextureRef>,
        p_min: impl Into<sys::ImVec2>,
        p_max: impl Into<sys::ImVec2>,
        uv_min: impl Into<sys::ImVec2>,
        uv_max: impl Into<sys::ImVec2>,
        col: impl Into<ImColor32>,
    ) {
        // Example:
        // let tex = texture::TextureId::new(5);
        // self.add_image(tex, [10.0,10.0], [110.0,110.0], [0.0,0.0], [1.0,1.0], Color::WHITE);
        let p_min: sys::ImVec2 = p_min.into();
        let p_max: sys::ImVec2 = p_max.into();
        let uv_min: sys::ImVec2 = uv_min.into();
        let uv_max: sys::ImVec2 = uv_max.into();
        let col = col.into().to_bits();
        let tex_ref = texture.into().raw();
        unsafe {
            sys::ImDrawList_AddImage(self.draw_list, tex_ref, p_min, p_max, uv_min, uv_max, col)
        }
    }

    /// Add an image with 4 arbitrary corners.
    #[doc(alias = "AddImageQuad")]
    pub fn add_image_quad(
        &self,
        texture: impl Into<crate::texture::TextureRef>,
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
        let p1: sys::ImVec2 = p1.into();
        let p2: sys::ImVec2 = p2.into();
        let p3: sys::ImVec2 = p3.into();
        let p4: sys::ImVec2 = p4.into();
        let uv1: sys::ImVec2 = uv1.into();
        let uv2: sys::ImVec2 = uv2.into();
        let uv3: sys::ImVec2 = uv3.into();
        let uv4: sys::ImVec2 = uv4.into();
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
    pub fn add_image_rounded(
        &self,
        texture: impl Into<crate::texture::TextureRef>,
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
        let p_min: sys::ImVec2 = p_min.into();
        let p_max: sys::ImVec2 = p_max.into();
        let uv_min: sys::ImVec2 = uv_min.into();
        let uv_max: sys::ImVec2 = uv_max.into();
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
        unsafe {
            sys::ImDrawList_AddQuad(
                self.draw_list,
                p1.into(),
                p2.into(),
                p3.into(),
                p4.into(),
                col.into().into(),
                thickness,
            )
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
        unsafe {
            sys::ImDrawList_AddQuadFilled(
                self.draw_list,
                p1.into(),
                p2.into(),
                p3.into(),
                p4.into(),
                col.into().into(),
            )
        }
    }

    /// Draw a regular n-gon outline.
    #[doc(alias = "AddNgon")]
    pub fn add_ngon<C>(
        &self,
        center: impl Into<sys::ImVec2>,
        radius: f32,
        col: C,
        num_segments: i32,
        thickness: f32,
    ) where
        C: Into<ImColor32>,
    {
        unsafe {
            sys::ImDrawList_AddNgon(
                self.draw_list,
                center.into(),
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
        num_segments: i32,
    ) where
        C: Into<ImColor32>,
    {
        unsafe {
            sys::ImDrawList_AddNgonFilled(
                self.draw_list,
                center.into(),
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
        num_segments: i32,
        thickness: f32,
    ) where
        C: Into<ImColor32>,
    {
        unsafe {
            sys::ImDrawList_AddEllipse(
                self.draw_list,
                center.into(),
                radius.into(),
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
        num_segments: i32,
    ) where
        C: Into<ImColor32>,
    {
        unsafe {
            sys::ImDrawList_AddEllipseFilled(
                self.draw_list,
                center.into(),
                radius.into(),
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
        num_segments: i32,
    ) where
        C: Into<ImColor32>,
    {
        unsafe {
            sys::ImDrawList_AddBezierQuadratic(
                self.draw_list,
                p1.into(),
                p2.into(),
                p3.into(),
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
        let mut buf: Vec<sys::ImVec2> = Vec::with_capacity(points.len());
        for p in points.iter().copied() {
            buf.push(p.into());
        }
        let count = match i32::try_from(buf.len()) {
            Ok(n) => n,
            Err(_) => return,
        };
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
        unsafe { sys::ImDrawList_PrimReserve(self.draw_list, idx_count, vtx_count) }
    }

    /// Unsafe low-level geometry API: unreserve previously reserved space.
    ///
    /// # Safety
    /// Must match a prior call to `prim_reserve` which hasn't been fully written.
    pub unsafe fn prim_unreserve(&self, idx_count: i32, vtx_count: i32) {
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
        unsafe { sys::ImDrawList_PrimRect(self.draw_list, a.into(), b.into(), col.into().into()) }
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
        unsafe {
            sys::ImDrawList_PrimRectUV(
                self.draw_list,
                a.into(),
                b.into(),
                uv_a.into(),
                uv_b.into(),
                col.into().into(),
            )
        }
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
        unsafe {
            sys::ImDrawList_PrimQuadUV(
                self.draw_list,
                a.into(),
                b.into(),
                c.into(),
                d.into(),
                uv_a.into(),
                uv_b.into(),
                uv_c.into(),
                uv_d.into(),
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
        unsafe {
            sys::ImDrawList_PrimWriteVtx(self.draw_list, pos.into(), uv.into(), col.into().into())
        }
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
        unsafe { sys::ImDrawList_PrimVtx(self.draw_list, pos.into(), uv.into(), col.into().into()) }
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
            p1: {
                let v: sys::ImVec2 = p1.into();
                v.into()
            },
            p2: {
                let v: sys::ImVec2 = p2.into();
                v.into()
            },
            color: c.into(),
            thickness: 1.0,
            draw_list,
        }
    }

    /// Set line's thickness (default to 1.0 pixel)
    pub fn thickness(mut self, thickness: f32) -> Self {
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
            p1: {
                let v: sys::ImVec2 = p1.into();
                v.into()
            },
            p2: {
                let v: sys::ImVec2 = p2.into();
                v.into()
            },
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
        self.rounding = rounding;
        self
    }

    /// Set rectangle's thickness (default to 1.0 pixel). Has no effect if filled
    pub fn thickness(mut self, thickness: f32) -> Self {
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
                    self.flags.bits() as sys::ImDrawFlags,
                    self.thickness,
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
    num_segments: i32,
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
        Self {
            center: {
                let v: sys::ImVec2 = center.into();
                v.into()
            },
            radius,
            color: color.into(),
            num_segments: 0, // 0 = auto
            thickness: 1.0,
            filled: false,
            draw_list,
        }
    }

    /// Set circle's thickness (default to 1.0 pixel). Has no effect if filled
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness;
        self
    }

    /// Draw circle as filled
    pub fn filled(mut self, filled: bool) -> Self {
        self.filled = filled;
        self
    }

    /// Set number of segments (default to 0 = auto)
    pub fn num_segments(mut self, num_segments: i32) -> Self {
        self.num_segments = num_segments;
        self
    }

    /// Draw the circle on the window
    pub fn build(self) {
        let center = sys::ImVec2 {
            x: self.center[0],
            y: self.center[1],
        };

        if self.filled {
            unsafe {
                sys::ImDrawList_AddCircleFilled(
                    self.draw_list.draw_list,
                    center,
                    self.radius,
                    self.color.into(),
                    self.num_segments,
                )
            }
        } else {
            unsafe {
                sys::ImDrawList_AddCircle(
                    self.draw_list.draw_list,
                    center,
                    self.radius,
                    self.color.into(),
                    self.num_segments,
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
    num_segments: Option<u32>,
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
            pos0: {
                let v: sys::ImVec2 = pos0.into();
                v.into()
            },
            cp0: {
                let v: sys::ImVec2 = cp0.into();
                v.into()
            },
            cp1: {
                let v: sys::ImVec2 = cp1.into();
                v.into()
            },
            pos1: {
                let v: sys::ImVec2 = pos1.into();
                v.into()
            },
            color: c.into(),
            thickness: 1.0,
            num_segments: None,
            draw_list,
        }
    }

    /// Set curve's thickness (default to 1.0 pixel)
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness;
        self
    }

    /// Set number of segments used to draw the Bezier curve. If not set, the
    /// bezier curve is auto-tessalated.
    pub fn num_segments(mut self, num_segments: u32) -> Self {
        self.num_segments = Some(num_segments);
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
                self.num_segments.unwrap_or(0) as i32,
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
            points: points.into_iter().map(Into::into).collect(),
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
        self.thickness = thickness;
        self
    }

    /// Set polyline flags. Has no effect if shape is filled.
    pub fn flags(mut self, flags: PolylineFlags) -> Self {
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
        let count = match i32::try_from(self.points.len()) {
            Ok(n) => n,
            Err(_) => return,
        };
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
                    self.flags.bits() as sys::ImDrawFlags,
                    self.thickness,
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
            p1: {
                let v: sys::ImVec2 = p1.into();
                v.into()
            },
            p2: {
                let v: sys::ImVec2 = p2.into();
                v.into()
            },
            p3: {
                let v: sys::ImVec2 = p3.into();
                v.into()
            },
            color: c.into(),
            thickness: 1.0,
            filled: false,
            draw_list,
        }
    }

    /// Set triangle's thickness (default to 1.0 pixel)
    pub fn thickness(mut self, thickness: f32) -> Self {
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

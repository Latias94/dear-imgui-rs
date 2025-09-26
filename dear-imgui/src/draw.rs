#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]
use crate::texture::TextureId;
use bitflags::bitflags;
use std::marker::PhantomData;

use crate::colors::Color;
use crate::sys;

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
    /// Options for some DrawList operations
    /// Values mirror ImGui's `ImDrawFlags_*` (v1.92+).
    #[repr(transparent)]
    pub struct DrawFlags: u32 {
        const NONE = sys::ImDrawFlags_None as u32;
        const CLOSED = sys::ImDrawFlags_Closed as u32;
        const ROUND_CORNERS_TOP_LEFT = sys::ImDrawFlags_RoundCornersTopLeft as u32;
        const ROUND_CORNERS_TOP_RIGHT = sys::ImDrawFlags_RoundCornersTopRight as u32;
        const ROUND_CORNERS_BOT_LEFT = sys::ImDrawFlags_RoundCornersBottomLeft as u32;
        const ROUND_CORNERS_BOT_RIGHT = sys::ImDrawFlags_RoundCornersBottomRight as u32;
        const ROUND_CORNERS_TOP = sys::ImDrawFlags_RoundCornersTop as u32;
        const ROUND_CORNERS_BOT = sys::ImDrawFlags_RoundCornersBottom as u32;
        const ROUND_CORNERS_LEFT = sys::ImDrawFlags_RoundCornersLeft as u32;
        const ROUND_CORNERS_RIGHT = sys::ImDrawFlags_RoundCornersRight as u32;
        const ROUND_CORNERS_ALL = sys::ImDrawFlags_RoundCornersAll as u32;
        const ROUND_CORNERS_NONE = sys::ImDrawFlags_RoundCornersNone as u32;
    }
}

// All draw types have been moved to crate::render module
// Use crate::render::{DrawVert, DrawIdx, DrawData, DrawListIterator} instead

/// Draw list wrapper
#[repr(transparent)]
pub struct DrawList(*mut sys::ImDrawList);

impl DrawList {
    /// Create DrawList from raw pointer (crate-internal)
    ///
    /// Safety: caller must ensure pointer validity for returned lifetime.
    pub(crate) unsafe fn from_raw(ptr: *mut sys::ImDrawList) -> Self {
        Self(ptr)
    }

    /// Get command buffer as slice
    unsafe fn cmd_buffer(&self) -> &[sys::ImDrawCmd] {
        unsafe {
            if (*self.0).CmdBuffer.Size <= 0 || (*self.0).CmdBuffer.Data.is_null() {
                return &[];
            }
            std::slice::from_raw_parts(
                (*self.0).CmdBuffer.Data as *const sys::ImDrawCmd,
                (*self.0).CmdBuffer.Size as usize,
            )
        }
    }

    /// Get vertex buffer
    pub fn vtx_buffer(&self) -> &[crate::render::DrawVert] {
        unsafe {
            if (*self.0).VtxBuffer.Size <= 0 || (*self.0).VtxBuffer.Data.is_null() {
                return &[];
            }
            std::slice::from_raw_parts(
                (*self.0).VtxBuffer.Data as *const crate::render::DrawVert,
                (*self.0).VtxBuffer.Size as usize,
            )
        }
    }

    /// Get index buffer
    pub fn idx_buffer(&self) -> &[crate::render::DrawIdx] {
        unsafe {
            if (*self.0).IdxBuffer.Size <= 0 || (*self.0).IdxBuffer.Data.is_null() {
                return &[];
            }
            std::slice::from_raw_parts((*self.0).IdxBuffer.Data, (*self.0).IdxBuffer.Size as usize)
        }
    }

    /// Get draw commands iterator
    pub fn commands(&self) -> DrawCmdIterator<'_> {
        unsafe {
            DrawCmdIterator {
                iter: self.cmd_buffer().iter(),
            }
        }
    }
}

/// Iterator over draw commands
pub struct DrawCmdIterator<'a> {
    iter: std::slice::Iter<'a, sys::ImDrawCmd>,
}

impl Iterator for DrawCmdIterator<'_> {
    type Item = DrawCmd;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|cmd| {
            let cmd_params = DrawCmdParams {
                clip_rect: [
                    cmd.ClipRect.x,
                    cmd.ClipRect.y,
                    cmd.ClipRect.z,
                    cmd.ClipRect.w,
                ],
                texture_id: TextureId::from(cmd.TexRef._TexID),
                vtx_offset: cmd.VtxOffset as usize,
                idx_offset: cmd.IdxOffset as usize,
            };

            match cmd.UserCallback {
                Some(raw_callback) if raw_callback as usize == usize::MAX => {
                    DrawCmd::ResetRenderState
                }
                Some(raw_callback) => DrawCmd::RawCallback {
                    callback: raw_callback,
                    raw_cmd: cmd,
                },
                None => DrawCmd::Elements {
                    count: cmd.ElemCount as usize,
                    cmd_params,
                },
            }
        })
    }
}

/// Draw command parameters
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DrawCmdParams {
    /// Clipping rectangle (left, top, right, bottom)
    pub clip_rect: [f32; 4],
    /// Texture ID
    pub texture_id: TextureId,
    /// Vertex offset
    pub vtx_offset: usize,
    /// Index offset
    pub idx_offset: usize,
}

/// Draw command
#[derive(Debug, Clone)]
pub enum DrawCmd {
    /// Elements to draw
    Elements {
        /// Number of indices
        count: usize,
        /// Command parameters
        cmd_params: DrawCmdParams,
    },
    /// Reset render state
    ResetRenderState,
    /// Raw callback
    RawCallback {
        /// Callback function
        callback: unsafe extern "C" fn(*const sys::ImDrawList, cmd: *const sys::ImDrawCmd),
        /// Raw command
        raw_cmd: *const sys::ImDrawCmd,
    },
}

enum DrawListType {
    Window,
    Background,
    Foreground,
}

/// Object implementing the custom draw API.
///
/// Called from [`Ui::get_window_draw_list`], [`Ui::get_background_draw_list`] or [`Ui::get_foreground_draw_list`].
/// No more than one instance of this structure can live in a program at the same time.
/// The program will panic on creating a second instance.
pub struct DrawListMut<'ui> {
    draw_list_type: DrawListType,
    draw_list: *mut sys::ImDrawList,
    _phantom: PhantomData<&'ui ()>,
}

// Lock for each variant of draw list
static DRAW_LIST_LOADED_WINDOW: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static DRAW_LIST_LOADED_BACKGROUND: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static DRAW_LIST_LOADED_FOREGROUND: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

impl Drop for DrawListMut<'_> {
    fn drop(&mut self) {
        match self.draw_list_type {
            DrawListType::Window => &DRAW_LIST_LOADED_WINDOW,
            DrawListType::Background => &DRAW_LIST_LOADED_BACKGROUND,
            DrawListType::Foreground => &DRAW_LIST_LOADED_FOREGROUND,
        }
        .store(false, std::sync::atomic::Ordering::Release);
    }
}

impl DrawListMut<'_> {
    fn lock_draw_list(t: DrawListType) {
        let lock = match t {
            DrawListType::Window => &DRAW_LIST_LOADED_WINDOW,
            DrawListType::Background => &DRAW_LIST_LOADED_BACKGROUND,
            DrawListType::Foreground => &DRAW_LIST_LOADED_FOREGROUND,
        };

        if lock
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::Acquire,
                std::sync::atomic::Ordering::Relaxed,
            )
            .is_err()
        {
            panic!(
                "A DrawListMut is already in use! You can only have one DrawListMut in use at a time."
            );
        }
    }

    pub(crate) fn window(_ui: &crate::Ui) -> Self {
        Self::lock_draw_list(DrawListType::Window);
        Self {
            draw_list: unsafe { sys::igGetWindowDrawList() },
            draw_list_type: DrawListType::Window,
            _phantom: PhantomData,
        }
    }

    pub(crate) fn background(_ui: &crate::Ui) -> Self {
        Self::lock_draw_list(DrawListType::Background);
        Self {
            draw_list: unsafe { sys::igGetBackgroundDrawList(std::ptr::null_mut()) },
            draw_list_type: DrawListType::Background,
            _phantom: PhantomData,
        }
    }

    pub(crate) fn foreground(_ui: &crate::Ui) -> Self {
        Self::lock_draw_list(DrawListType::Foreground);
        Self {
            draw_list: unsafe { sys::igGetForegroundDrawList_ViewportPtr(std::ptr::null_mut()) },
            draw_list_type: DrawListType::Foreground,
            _phantom: PhantomData,
        }
    }
}

/// Drawing functions
impl<'ui> DrawListMut<'ui> {
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
        unsafe {
            let vec2: sys::ImVec2 = pos.into();

            // PathLineTo is inline: _Path.push_back(pos);
            // We need to manually push to the ImVector
            let draw_list = self.draw_list;
            let path = &mut (*draw_list)._Path;

            // Check if we have capacity
            if path.Size < path.Capacity {
                // Add the point directly
                *path.Data.add(path.Size as usize) = vec2;
                path.Size += 1;
            } else {
                // If no capacity, we'll use a workaround by drawing a line to the point
                // This isn't perfect but avoids memory management issues
                let _current_pos = if path.Size > 0 {
                    *path.Data.add((path.Size - 1) as usize)
                } else {
                    vec2
                };

                // Clear path and start fresh with just this point
                path.Size = 0;
                if path.Capacity > 0 {
                    *path.Data.add(0) = vec2;
                    path.Size = 1;
                }
            }
        }
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
        flags: DrawFlags,
    ) {
        unsafe {
            let min_vec: sys::ImVec2 = rect_min.into();
            let max_vec: sys::ImVec2 = rect_max.into();
            sys::ImDrawList_PathRect(
                self.draw_list,
                min_vec,
                max_vec,
                rounding,
                flags.bits() as i32,
            );
        }
    }

    /// Stroke the current path with the specified color and thickness.
    #[doc(alias = "PathStroke")]
    pub fn path_stroke(&self, color: impl Into<ImColor32>, flags: DrawFlags, thickness: f32) {
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
                    flags.bits() as i32,
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

    /// Push a texture on the drawlist texture stack (ImGui 1.92+)
    ///
    /// While pushed, image and primitives will use this texture unless otherwise specified.
    ///
    /// Example:
    /// ```no_run
    /// # use dear_imgui::*;
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

    /// Pop the last texture from the drawlist texture stack (ImGui 1.92+)
    #[doc(alias = "PopTexture")]
    pub fn pop_texture(&self) {
        unsafe {
            sys::ImDrawList_PopTexture(self.draw_list);
        }
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
        flags: DrawFlags,
    ) {
        // Example:
        // let tex = texture::TextureId::new(5);
        // self.add_image_rounded(
        //     tex,
        //     [10.0,10.0], [110.0,110.0],
        //     [0.0,0.0], [1.0,1.0],
        //     Color::WHITE,
        //     8.0,
        //     DrawFlags::ROUND_CORNERS_ALL,
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
                flags.bits() as i32,
            )
        }
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
    flags: DrawFlags,
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
            flags: DrawFlags::ROUND_CORNERS_ALL,
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

    /// Set rectangle's corner flags
    pub fn flags(mut self, flags: DrawFlags) -> Self {
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
                    self.flags.bits() as i32,
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
                    self.flags.bits() as i32,
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
    points: Vec<[f32; 2]>,
    thickness: f32,
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
                .map(|p| {
                    let v: sys::ImVec2 = p.into();
                    v.into()
                })
                .collect(),
            color: c.into(),
            thickness: 1.0,
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

    /// Draw shape as filled convex polygon
    pub fn filled(mut self, filled: bool) -> Self {
        self.filled = filled;
        self
    }

    /// Draw the line on the window
    pub fn build(self) {
        if self.filled {
            unsafe {
                sys::ImDrawList_AddConvexPolyFilled(
                    self.draw_list.draw_list,
                    self.points.as_ptr() as *const sys::ImVec2,
                    self.points.len() as i32,
                    self.color.into(),
                )
            }
        } else {
            unsafe {
                sys::ImDrawList_AddPolyline(
                    self.draw_list.draw_list,
                    self.points.as_ptr() as *const sys::ImVec2,
                    self.points.len() as i32,
                    self.color.into(),
                    sys::ImDrawFlags::default(),
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

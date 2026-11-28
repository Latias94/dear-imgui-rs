use crate::draw::{
    BezierCurve, Circle, DrawFlags, DrawNgonSegmentCount, DrawSegmentCount, FILL_FLAGS, ImColor32,
    Line, Polyline, Rect, STROKE_FLAGS, STROKE_POSITION, Triangle, assert_arc_fast_steps,
    assert_corner_flags, assert_draw_flags, assert_finite_f32, assert_non_negative_f32,
    assert_path_not_empty, assert_polyline_flags, assert_positive_f32, count_to_i32, finite_point,
    finite_vec2, non_negative_vec2,
};
use crate::internal::len_i32;
use crate::sys;

use super::DrawListMut;

impl DrawListMut<'_> {
    /// Returns a line from point `p1` to `p2` with color `c`.
    pub fn add_line<C>(
        &self,
        p1: impl Into<sys::ImVec2>,
        p2: impl Into<sys::ImVec2>,
        c: C,
    ) -> Line<'_>
    where
        C: Into<ImColor32>,
    {
        Line::new(self, p1, p2, c)
    }

    /// Draw a horizontal line from `min_x` to `max_x` at `y`.
    #[doc(alias = "AddLineH")]
    pub fn add_line_h<C>(
        &self,
        min_x: f32,
        max_x: f32,
        y: f32,
        col: C,
        thickness: f32,
        flags: DrawFlags,
    ) where
        C: Into<ImColor32>,
    {
        assert_draw_flags("DrawListMut::add_line_h()", flags, STROKE_FLAGS);
        assert_finite_f32("DrawListMut::add_line_h()", "min_x", min_x);
        assert_finite_f32("DrawListMut::add_line_h()", "max_x", max_x);
        assert_finite_f32("DrawListMut::add_line_h()", "y", y);
        assert_positive_f32("DrawListMut::add_line_h()", "thickness", thickness);

        let color = col.into().into();
        self.with_raw_bound("DrawListMut::add_line_h()", |draw_list| unsafe {
            sys::ImDrawList_AddLineH(
                draw_list,
                min_x,
                max_x,
                y,
                color,
                thickness,
                flags.bits() as sys::ImDrawFlags,
            )
        });
    }

    /// Draw a vertical line from `min_y` to `max_y` at `x`.
    #[doc(alias = "AddLineV")]
    pub fn add_line_v<C>(
        &self,
        x: f32,
        min_y: f32,
        max_y: f32,
        col: C,
        thickness: f32,
        flags: DrawFlags,
    ) where
        C: Into<ImColor32>,
    {
        assert_draw_flags("DrawListMut::add_line_v()", flags, STROKE_FLAGS);
        assert_finite_f32("DrawListMut::add_line_v()", "x", x);
        assert_finite_f32("DrawListMut::add_line_v()", "min_y", min_y);
        assert_finite_f32("DrawListMut::add_line_v()", "max_y", max_y);
        assert_positive_f32("DrawListMut::add_line_v()", "thickness", thickness);

        let color = col.into().into();
        self.with_raw_bound("DrawListMut::add_line_v()", |draw_list| unsafe {
            sys::ImDrawList_AddLineV(
                draw_list,
                x,
                min_y,
                max_y,
                color,
                thickness,
                flags.bits() as sys::ImDrawFlags,
            )
        });
    }

    /// Returns a rectangle whose upper-left corner is at point `p1`
    /// and lower-right corner is at point `p2`, with color `c`.
    pub fn add_rect<C>(
        &self,
        p1: impl Into<sys::ImVec2>,
        p2: impl Into<sys::ImVec2>,
        c: C,
    ) -> Rect<'_>
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
        self.with_raw_bound(
            "DrawListMut::add_rect_filled_multicolor()",
            |draw_list| unsafe {
                sys::ImDrawList_AddRectFilledMultiColor(
                    draw_list, p_min, p_max, c_ul, c_ur, c_br, c_bl,
                );
            },
        );
    }

    /// Returns a circle with the given `center`, `radius` and `color`.
    pub fn add_circle<C>(&self, center: impl Into<sys::ImVec2>, radius: f32, color: C) -> Circle<'_>
    where
        C: Into<ImColor32>,
    {
        Circle::new(self, center, radius, color)
    }

    /// Returns a Bezier curve stretching from `pos0` to `pos1`, whose
    /// curvature is defined by `cp0` and `cp1`.
    #[doc(alias = "AddBezier", alias = "AddBezierCubic")]
    pub fn add_bezier_curve(
        &self,
        pos0: impl Into<sys::ImVec2>,
        cp0: impl Into<sys::ImVec2>,
        cp1: impl Into<sys::ImVec2>,
        pos1: impl Into<sys::ImVec2>,
        color: impl Into<ImColor32>,
    ) -> BezierCurve<'_> {
        BezierCurve::new(self, pos0, cp0, cp1, pos1, color)
    }

    /// Returns a triangle with the given 3 vertices `p1`, `p2` and `p3` and color `c`.
    #[doc(alias = "AddTriangleFilled", alias = "AddTriangle")]
    pub fn add_triangle<C>(
        &self,
        p1: impl Into<sys::ImVec2>,
        p2: impl Into<sys::ImVec2>,
        p3: impl Into<sys::ImVec2>,
        c: C,
    ) -> Triangle<'_>
    where
        C: Into<ImColor32>,
    {
        Triangle::new(self, p1, p2, p3, c)
    }

    /// Returns a polygonal line. If filled is rendered as a convex
    /// polygon, if not filled is drawn as a line specified by
    /// `Polyline::thickness` (default 1.0)
    #[doc(alias = "AddPolyline", alias = "AddConvexPolyFilled")]
    pub fn add_polyline<C, P>(&self, points: Vec<P>, c: C) -> Polyline<'_>
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
        self.with_raw_bound("DrawListMut::path_clear()", |draw_list| unsafe {
            (*draw_list)._Path.Size = 0;
        });
    }

    /// Add a point to the current path.
    #[doc(alias = "PathLineTo")]
    pub fn path_line_to(&self, pos: impl Into<sys::ImVec2>) {
        let pos = finite_vec2("DrawListMut::path_line_to()", "pos", pos);
        self.with_raw_bound("DrawListMut::path_line_to()", |draw_list| unsafe {
            sys::ImDrawList_PathLineTo(draw_list, pos)
        });
    }

    /// Add a point to the current path, merging duplicate points.
    #[doc(alias = "PathLineToMergeDuplicate")]
    pub fn path_line_to_merge_duplicate(&self, pos: impl Into<sys::ImVec2>) {
        let pos = finite_vec2("DrawListMut::path_line_to_merge_duplicate()", "pos", pos);
        self.with_raw_bound(
            "DrawListMut::path_line_to_merge_duplicate()",
            |draw_list| unsafe { sys::ImDrawList_PathLineToMergeDuplicate(draw_list, pos) },
        );
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

        self.with_raw_bound("DrawListMut::path_arc_to()", |draw_list| unsafe {
            sys::ImDrawList_PathArcTo(draw_list, center_vec, radius, a_min, a_max, num_segments);
        });
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

        self.with_raw_bound("DrawListMut::path_arc_to_fast()", |draw_list| unsafe {
            sys::ImDrawList_PathArcToFast(draw_list, center_vec, radius, a_min_of_12, a_max_of_12);
        });
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
        let min_vec = finite_vec2("DrawListMut::path_rect()", "rect_min", rect_min);
        let max_vec = finite_vec2("DrawListMut::path_rect()", "rect_max", rect_max);
        assert_non_negative_f32("DrawListMut::path_rect()", "rounding", rounding);
        assert_corner_flags("DrawListMut::path_rect()", flags);

        self.with_raw_bound("DrawListMut::path_rect()", |draw_list| unsafe {
            sys::ImDrawList_PathRect(
                draw_list,
                min_vec,
                max_vec,
                rounding,
                flags.bits() as sys::ImDrawFlags,
            );
        });
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

        self.with_raw_bound(
            "DrawListMut::path_elliptical_arc_to()",
            |draw_list| unsafe {
                sys::ImDrawList_PathEllipticalArcTo(
                    draw_list,
                    center,
                    radius,
                    rot,
                    a_min,
                    a_max,
                    num_segments,
                )
            },
        );
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
        let caller = "DrawListMut::path_bezier_quadratic_curve_to()";
        self.with_raw_bound(caller, |draw_list| unsafe {
            assert_path_not_empty(draw_list, caller);
            sys::ImDrawList_PathBezierQuadraticCurveTo(draw_list, p2, p3, num_segments);
        });
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
        let caller = "DrawListMut::path_bezier_cubic_curve_to()";
        self.with_raw_bound(caller, |draw_list| unsafe {
            assert_path_not_empty(draw_list, caller);
            sys::ImDrawList_PathBezierCubicCurveTo(draw_list, p2, p3, p4, num_segments);
        });
    }

    /// Stroke the current path with the specified color and thickness.
    #[doc(alias = "PathStroke")]
    pub fn path_stroke(&self, color: impl Into<ImColor32>, thickness: f32, flags: DrawFlags) {
        assert_polyline_flags("DrawListMut::path_stroke()", flags);
        assert_positive_f32("DrawListMut::path_stroke()", "thickness", thickness);

        let color = color.into().into();
        self.with_raw_bound("DrawListMut::path_stroke()", |draw_list| unsafe {
            sys::ImDrawList_PathStroke(
                draw_list,
                color,
                thickness,
                flags.bits() as sys::ImDrawFlags,
            );
        });
    }

    /// Fill the current path as a convex polygon.
    #[doc(alias = "PathFillConvex")]
    pub fn path_fill_convex(&self, color: impl Into<ImColor32>, flags: DrawFlags) {
        assert_draw_flags("DrawListMut::path_fill_convex()", flags, FILL_FLAGS);
        let color = color.into().into();
        self.with_raw_bound("DrawListMut::path_fill_convex()", |draw_list| unsafe {
            sys::ImDrawList_PathFillConvex(draw_list, color, flags.bits() as sys::ImDrawFlags);
        });
    }

    /// Draw a quadrilateral outline given four points.
    #[doc(alias = "AddQuad")]
    #[allow(clippy::too_many_arguments, reason = "mirrors ImDrawList::AddQuad")]
    pub fn add_quad<C>(
        &self,
        p1: impl Into<sys::ImVec2>,
        p2: impl Into<sys::ImVec2>,
        p3: impl Into<sys::ImVec2>,
        p4: impl Into<sys::ImVec2>,
        col: C,
        thickness: f32,
        flags: DrawFlags,
    ) where
        C: Into<ImColor32>,
    {
        assert_draw_flags("DrawListMut::add_quad()", flags, STROKE_FLAGS);
        let p1 = finite_vec2("DrawListMut::add_quad()", "p1", p1);
        let p2 = finite_vec2("DrawListMut::add_quad()", "p2", p2);
        let p3 = finite_vec2("DrawListMut::add_quad()", "p3", p3);
        let p4 = finite_vec2("DrawListMut::add_quad()", "p4", p4);
        assert_positive_f32("DrawListMut::add_quad()", "thickness", thickness);

        let color = col.into().into();
        self.with_raw_bound("DrawListMut::add_quad()", |draw_list| unsafe {
            sys::ImDrawList_AddQuad(
                draw_list,
                p1,
                p2,
                p3,
                p4,
                color,
                thickness,
                flags.bits() as sys::ImDrawFlags,
            )
        });
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

        let color = col.into().into();
        self.with_raw_bound("DrawListMut::add_quad_filled()", |draw_list| unsafe {
            sys::ImDrawList_AddQuadFilled(draw_list, p1, p2, p3, p4, color)
        });
    }

    /// Draw a regular n-gon outline.
    ///
    /// `flags` selects the stroke position. Configure anti-aliasing with
    /// [`Self::with_draw_flags`]; the native implementation does not apply
    /// per-polygon anti-aliasing flags.
    #[doc(alias = "AddNgon")]
    pub fn add_ngon<C>(
        &self,
        center: impl Into<sys::ImVec2>,
        radius: f32,
        col: C,
        num_segments: DrawNgonSegmentCount,
        thickness: f32,
        flags: DrawFlags,
    ) where
        C: Into<ImColor32>,
    {
        assert_draw_flags("DrawListMut::add_ngon()", flags, STROKE_POSITION);
        let center = finite_vec2("DrawListMut::add_ngon()", "center", center);
        assert_non_negative_f32("DrawListMut::add_ngon()", "radius", radius);
        assert_positive_f32("DrawListMut::add_ngon()", "thickness", thickness);
        let num_segments = num_segments.into_i32("DrawListMut::add_ngon()");

        let color = col.into().into();
        self.with_raw_bound("DrawListMut::add_ngon()", |draw_list| unsafe {
            sys::ImDrawList_AddNgon(
                draw_list,
                center,
                radius,
                color,
                num_segments,
                thickness,
                flags.bits() as sys::ImDrawFlags,
            )
        });
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

        let color = col.into().into();
        self.with_raw_bound("DrawListMut::add_ngon_filled()", |draw_list| unsafe {
            sys::ImDrawList_AddNgonFilled(draw_list, center, radius, color, num_segments)
        });
    }

    /// Draw an ellipse outline.
    ///
    /// `flags` selects the stroke position. Configure anti-aliasing with
    /// [`Self::with_draw_flags`]; the native implementation does not apply
    /// per-ellipse anti-aliasing flags.
    #[doc(alias = "AddEllipse")]
    #[allow(clippy::too_many_arguments, reason = "mirrors ImDrawList::AddEllipse")]
    pub fn add_ellipse<C>(
        &self,
        center: impl Into<sys::ImVec2>,
        radius: impl Into<sys::ImVec2>,
        col: C,
        rot: f32,
        num_segments: impl Into<DrawSegmentCount>,
        thickness: f32,
        flags: DrawFlags,
    ) where
        C: Into<ImColor32>,
    {
        assert_draw_flags("DrawListMut::add_ellipse()", flags, STROKE_POSITION);
        let center = finite_vec2("DrawListMut::add_ellipse()", "center", center);
        let radius = non_negative_vec2("DrawListMut::add_ellipse()", "radius", radius);
        assert_finite_f32("DrawListMut::add_ellipse()", "rot", rot);
        assert_positive_f32("DrawListMut::add_ellipse()", "thickness", thickness);
        let num_segments = num_segments.into().into_i32("DrawListMut::add_ellipse()");

        let color = col.into().into();
        self.with_raw_bound("DrawListMut::add_ellipse()", |draw_list| unsafe {
            sys::ImDrawList_AddEllipse(
                draw_list,
                center,
                radius,
                color,
                rot,
                num_segments,
                thickness,
                flags.bits() as sys::ImDrawFlags,
            )
        });
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

        let color = col.into().into();
        self.with_raw_bound("DrawListMut::add_ellipse_filled()", |draw_list| unsafe {
            sys::ImDrawList_AddEllipseFilled(draw_list, center, radius, color, rot, num_segments)
        });
    }

    /// Draw a quadratic Bezier curve directly.
    #[doc(alias = "AddBezierQuadratic")]
    #[allow(
        clippy::too_many_arguments,
        reason = "mirrors ImDrawList::AddBezierQuadratic"
    )]
    pub fn add_bezier_quadratic<C>(
        &self,
        p1: impl Into<sys::ImVec2>,
        p2: impl Into<sys::ImVec2>,
        p3: impl Into<sys::ImVec2>,
        col: C,
        thickness: f32,
        num_segments: impl Into<DrawSegmentCount>,
        flags: DrawFlags,
    ) where
        C: Into<ImColor32>,
    {
        assert_draw_flags("DrawListMut::add_bezier_quadratic()", flags, STROKE_FLAGS);
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

        let color = col.into().into();
        self.with_raw_bound("DrawListMut::add_bezier_quadratic()", |draw_list| unsafe {
            sys::ImDrawList_AddBezierQuadratic(
                draw_list,
                p1,
                p2,
                p3,
                color,
                thickness,
                num_segments,
                flags.bits() as sys::ImDrawFlags,
            )
        });
    }

    /// Fill a concave polygon (Dear ImGui 1.92+).
    #[doc(alias = "AddConcavePolyFilled")]
    pub fn add_concave_poly_filled<C, P>(&self, points: &[P], col: C)
    where
        C: Into<ImColor32>,
        P: Copy + Into<sys::ImVec2>,
    {
        let caller = "DrawListMut::add_concave_poly_filled()";
        let count = len_i32(caller, "points", points.len());
        let mut buf: Vec<sys::ImVec2> = Vec::with_capacity(points.len());
        for (index, point) in points.iter().copied().enumerate() {
            buf.push(finite_point(caller, index, point));
        }
        let color = col.into().into();
        self.with_raw_bound(caller, |draw_list| unsafe {
            sys::ImDrawList_AddConcavePolyFilled(draw_list, buf.as_ptr(), count, color)
        });
    }

    /// Fill the current path as a concave polygon (Dear ImGui 1.92+).
    #[doc(alias = "PathFillConcave")]
    pub fn path_fill_concave(&self, color: impl Into<ImColor32>) {
        let color = color.into().into();
        self.with_raw_bound("DrawListMut::path_fill_concave()", |draw_list| unsafe {
            sys::ImDrawList_PathFillConcave(draw_list, color)
        });
    }

    /// Unsafe low-level geometry API: reserve index and vertex space.
    ///
    /// # Safety
    /// Caller must write exactly the reserved amount using `prim_write_*` and ensure valid topology.
    pub unsafe fn prim_reserve(&self, idx_count: usize, vtx_count: usize) {
        let idx_count = count_to_i32("DrawListMut::prim_reserve()", "idx_count", idx_count);
        let vtx_count = count_to_i32("DrawListMut::prim_reserve()", "vtx_count", vtx_count);
        self.with_raw_bound("DrawListMut::prim_reserve()", |draw_list| unsafe {
            sys::ImDrawList_PrimReserve(draw_list, idx_count, vtx_count)
        });
    }

    /// Unsafe low-level geometry API: unreserve previously reserved space.
    ///
    /// # Safety
    /// Must match a prior call to `prim_reserve` which hasn't been fully written.
    pub unsafe fn prim_unreserve(&self, idx_count: usize, vtx_count: usize) {
        let idx_count = count_to_i32("DrawListMut::prim_unreserve()", "idx_count", idx_count);
        let vtx_count = count_to_i32("DrawListMut::prim_unreserve()", "vtx_count", vtx_count);
        self.with_raw_bound("DrawListMut::prim_unreserve()", |draw_list| unsafe {
            sys::ImDrawList_PrimUnreserve(draw_list, idx_count, vtx_count)
        });
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
        let color = col.into().into();
        self.with_raw_bound("DrawListMut::prim_rect()", |draw_list| unsafe {
            sys::ImDrawList_PrimRect(draw_list, a, b, color)
        });
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

        let color = col.into().into();
        self.with_raw_bound("DrawListMut::prim_rect_uv()", |draw_list| unsafe {
            sys::ImDrawList_PrimRectUV(draw_list, a, b, uv_a, uv_b, color)
        });
    }

    /// Unsafe low-level geometry API: append a quad primitive with UVs and color.
    ///
    /// # Safety
    /// Only use between `prim_reserve` and completing the reserved writes.
    #[allow(clippy::too_many_arguments, reason = "mirrors ImDrawList::PrimQuadUV")]
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

        let color = col.into().into();
        self.with_raw_bound("DrawListMut::prim_quad_uv()", |draw_list| unsafe {
            sys::ImDrawList_PrimQuadUV(draw_list, a, b, c, d, uv_a, uv_b, uv_c, uv_d, color)
        });
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
        let color = col.into().into();
        self.with_raw_bound("DrawListMut::prim_write_vtx()", |draw_list| unsafe {
            sys::ImDrawList_PrimWriteVtx(draw_list, pos, uv, color)
        });
    }

    /// Unsafe low-level geometry API: write an index.
    ///
    /// # Safety
    /// Only use to fill space reserved by `prim_reserve`.
    pub unsafe fn prim_write_idx(&self, idx: sys::ImDrawIdx) {
        self.with_raw_bound("DrawListMut::prim_write_idx()", |draw_list| unsafe {
            sys::ImDrawList_PrimWriteIdx(draw_list, idx)
        });
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
        let color = col.into().into();
        self.with_raw_bound("DrawListMut::prim_vtx()", |draw_list| unsafe {
            sys::ImDrawList_PrimVtx(draw_list, pos, uv, color)
        });
    }
}

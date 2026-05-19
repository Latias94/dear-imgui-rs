use crate::internal::len_i32;
use crate::sys;

use super::super::color::ImColor32;
use super::super::counts::{
    DrawCornerFlags, DrawNgonSegmentCount, DrawSegmentCount, PolylineFlags,
};
use super::super::primitives::{BezierCurve, Circle, Line, Polyline, Rect, Triangle};
use super::super::util::{
    assert_arc_fast_steps, assert_corner_flags, assert_finite_f32, assert_non_negative_f32,
    assert_path_not_empty, assert_polyline_flags, assert_positive_f32, finite_vec2,
    non_negative_vec2,
};
use super::DrawListMut;

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
}

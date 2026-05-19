use crate::internal::len_i32;
use crate::sys;

use super::color::ImColor32;
use super::counts::{DrawCornerFlags, DrawSegmentCount, PolylineFlags};
use super::list::DrawListMut;
use super::util::{
    assert_corner_flags, assert_non_negative_f32, assert_polyline_flags, assert_positive_f32,
    finite_vec2,
};

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
    pub(super) fn new<C>(
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
    pub(super) fn new<C>(
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
    pub(super) fn new<C>(
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
    pub(super) fn new<C, P>(draw_list: &'ui DrawListMut<'_>, points: Vec<P>, c: C) -> Self
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
    pub(super) fn new<C>(
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

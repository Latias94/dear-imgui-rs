use bitflags::bitflags;
use std::num::NonZeroUsize;

use crate::sys;

use super::util::count_to_i32;

bitflags! {
    /// Options for drawing primitives and configuring draw-list scopes.
    ///
    /// Per-primitive flags add to the current draw-list defaults. To disable an
    /// inherited option, use [`super::DrawListMut::with_draw_flags`]. Stroke
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
    pub(super) fn into_i32(self, caller: &str) -> i32 {
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
    pub(super) fn into_i32(self, caller: &str) -> i32 {
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

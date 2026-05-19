use bitflags::bitflags;
use std::num::NonZeroUsize;

use crate::sys;

use super::util::count_to_i32;

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

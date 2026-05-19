use std::cell::RefCell;

use crate::{imgui_sys, sys};

pub(crate) fn len_i32(len: usize) -> Option<i32> {
    i32::try_from(len).ok()
}

pub(crate) fn axis_tick_count_to_i32(caller: &str, count: usize) -> i32 {
    assert!(count > 0, "{caller} n_ticks must be positive");
    i32::try_from(count)
        .unwrap_or_else(|_| panic!("{caller} n_ticks exceeded ImPlot3D's i32 range"))
}

pub(crate) fn surface_count_to_i32(count: usize) -> Option<i32> {
    let count = i32::try_from(count).ok()?;
    (count > 0).then_some(count)
}

pub(crate) const IMPLOT3D_AUTO: i32 = -1;

thread_local! {
    static NEXT_PLOT3D_SPEC: RefCell<Option<sys::ImPlot3DSpec_c>> = RefCell::new(None);
}

/// Sample-index offset used by ImPlot3D item data access.
///
/// ImPlot3D intentionally allows negative and out-of-range offsets for circular
/// buffers, so this is a signed sample offset rather than a Rust slice index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Plot3DDataOffset(i32);

impl Plot3DDataOffset {
    /// No data offset.
    pub const ZERO: Self = Self(0);

    /// Create a sample-index offset.
    #[inline]
    pub const fn samples(offset: i32) -> Self {
        Self(offset)
    }

    #[inline]
    pub(crate) const fn raw(self) -> i32 {
        self.0
    }
}

/// Byte stride used by ImPlot3D item data access.
///
/// Use [`Plot3DDataStride::AUTO`] for contiguous data of the plotted value type,
/// or [`Plot3DDataStride::bytes`] for interleaved/custom layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plot3DDataStride(i32);

impl Plot3DDataStride {
    /// Let ImPlot3D use `sizeof(T)` for the plotted value type.
    pub const AUTO: Self = Self(IMPLOT3D_AUTO);

    /// Create a byte stride.
    ///
    /// Panics if `bytes` is zero or exceeds ImPlot3D's `int` range.
    #[inline]
    pub fn bytes(bytes: usize) -> Self {
        assert!(
            bytes > 0,
            "Plot3DDataStride::bytes() requires a non-zero stride"
        );
        let bytes = i32::try_from(bytes)
            .expect("Plot3DDataStride::bytes() stride exceeded ImPlot3D's int range");
        Self(bytes)
    }

    /// Create the contiguous byte stride for `T`.
    #[inline]
    pub fn for_type<T>() -> Self {
        Self::bytes(std::mem::size_of::<T>())
    }

    #[inline]
    pub(crate) const fn raw(self) -> i32 {
        self.0
    }
}

impl Default for Plot3DDataStride {
    fn default() -> Self {
        Self::AUTO
    }
}

/// Data layout used by ImPlot3D item builders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Plot3DDataLayout {
    offset: Plot3DDataOffset,
    stride: Plot3DDataStride,
}

impl Plot3DDataLayout {
    /// Contiguous data starting at sample offset zero.
    pub const DEFAULT: Self = Self {
        offset: Plot3DDataOffset::ZERO,
        stride: Plot3DDataStride::AUTO,
    };

    /// Create a data layout from a sample offset and byte stride.
    #[inline]
    pub const fn new(offset: Plot3DDataOffset, stride: Plot3DDataStride) -> Self {
        Self { offset, stride }
    }

    /// Create a data layout with a different sample offset.
    #[inline]
    pub const fn with_offset(mut self, offset: Plot3DDataOffset) -> Self {
        self.offset = offset;
        self
    }

    /// Create a data layout with a different byte stride.
    #[inline]
    pub const fn with_stride(mut self, stride: Plot3DDataStride) -> Self {
        self.stride = stride;
        self
    }

    #[inline]
    pub(crate) const fn raw_offset(self) -> i32 {
        self.offset.raw()
    }

    #[inline]
    pub(crate) const fn raw_stride(self) -> i32 {
        self.stride.raw()
    }
}

pub(crate) fn update_next_plot3d_spec(f: impl FnOnce(&mut sys::ImPlot3DSpec_c)) {
    NEXT_PLOT3D_SPEC.with(|cell| {
        let mut guard = cell.borrow_mut();
        let mut spec = guard.take().unwrap_or_else(default_plot3d_spec);
        f(&mut spec);
        *guard = Some(spec);
    })
}

pub(crate) fn take_next_plot3d_spec() -> Option<sys::ImPlot3DSpec_c> {
    NEXT_PLOT3D_SPEC.with(|cell| cell.borrow_mut().take())
}

pub(crate) fn set_next_plot3d_spec(spec: Option<sys::ImPlot3DSpec_c>) {
    NEXT_PLOT3D_SPEC.with(|cell| {
        *cell.borrow_mut() = spec;
    })
}

pub(crate) fn default_plot3d_spec() -> sys::ImPlot3DSpec_c {
    let auto_col = sys::ImVec4_c {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: -1.0,
    };

    sys::ImPlot3DSpec_c {
        LineColor: auto_col,
        LineColors: std::ptr::null_mut(),
        LineWeight: 1.0,
        FillColor: auto_col,
        FillColors: std::ptr::null_mut(),
        FillAlpha: -1.0,
        Marker: sys::ImPlot3DMarker_Auto as _,
        MarkerSize: -1.0,
        MarkerSizes: std::ptr::null_mut(),
        MarkerLineColor: auto_col,
        MarkerLineColors: std::ptr::null_mut(),
        MarkerFillColor: auto_col,
        MarkerFillColors: std::ptr::null_mut(),
        Offset: 0,
        Stride: IMPLOT3D_AUTO,
        Flags: sys::ImPlot3DItemFlags_None as _,
    }
}

pub(crate) fn plot3d_spec_from(flags: u32, layout: Plot3DDataLayout) -> sys::ImPlot3DSpec_c {
    let mut spec = take_next_plot3d_spec().unwrap_or_else(default_plot3d_spec);
    spec.Flags = ((spec.Flags as u32) | flags) as sys::ImPlot3DItemFlags;
    spec.Offset = layout.raw_offset();
    spec.Stride = layout.raw_stride();
    spec
}

pub(crate) trait ImVec2Ctor {
    fn from_xy(x: f32, y: f32) -> Self;
}

impl ImVec2Ctor for sys::ImVec2_c {
    fn from_xy(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl ImVec2Ctor for imgui_sys::ImVec2_c {
    fn from_xy(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[inline]
pub(crate) fn imvec2<T: ImVec2Ctor>(x: f32, y: f32) -> T {
    T::from_xy(x, y)
}

pub(crate) trait ImVec4Ctor {
    fn from_xyzw(x: f32, y: f32, z: f32, w: f32) -> Self;
}

impl ImVec4Ctor for sys::ImVec4_c {
    fn from_xyzw(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }
}

impl ImVec4Ctor for imgui_sys::ImVec4_c {
    fn from_xyzw(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }
}

#[inline]
pub(crate) fn imvec4<T: ImVec4Ctor>(x: f32, y: f32, z: f32, w: f32) -> T {
    T::from_xyzw(x, y, z, w)
}

#[cfg(test)]
mod tests {
    use super::{
        Plot3DDataLayout, Plot3DDataOffset, Plot3DDataStride, axis_tick_count_to_i32,
        plot3d_spec_from, surface_count_to_i32,
    };

    #[test]
    fn data_layout_allows_signed_sample_offsets() {
        let layout = Plot3DDataLayout::DEFAULT.with_offset(Plot3DDataOffset::samples(-8));
        let spec = plot3d_spec_from(0, layout);
        assert_eq!(spec.Offset, -8);
        assert_eq!(spec.Stride, super::IMPLOT3D_AUTO);
    }

    #[test]
    fn data_stride_bytes_are_positive() {
        let stride = Plot3DDataStride::bytes(16);
        let layout = Plot3DDataLayout::DEFAULT.with_stride(stride);
        let spec = plot3d_spec_from(0, layout);
        assert_eq!(spec.Stride, 16);
    }

    #[test]
    #[should_panic(expected = "requires a non-zero stride")]
    fn zero_data_stride_panics_before_ffi() {
        let _ = Plot3DDataStride::bytes(0);
    }

    #[test]
    fn surface_count_checks_implot3d_i32_range() {
        assert_eq!(surface_count_to_i32(1), Some(1));
        assert_eq!(surface_count_to_i32(0), None);
        assert_eq!(surface_count_to_i32(i32::MAX as usize), Some(i32::MAX));
        assert_eq!(surface_count_to_i32(i32::MAX as usize + 1), None);
    }

    #[test]
    fn axis_ticks_range_count_is_checked_before_ffi() {
        assert_eq!(axis_tick_count_to_i32("test", 1), 1);
        assert_eq!(axis_tick_count_to_i32("test", i32::MAX as usize), i32::MAX);

        assert!(
            std::panic::catch_unwind(|| axis_tick_count_to_i32("test", 0)).is_err(),
            "zero tick counts must not cross the safe API boundary"
        );
        assert!(
            std::panic::catch_unwind(|| {
                axis_tick_count_to_i32("test", i32::MAX as usize + 1);
            })
            .is_err(),
            "oversized tick counts must not cross the safe API boundary"
        );
    }
}

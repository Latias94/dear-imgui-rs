use std::borrow::Cow;

use super::{Plot3D, Plot3DError};
use crate::{Plot3DDataLayout, Plot3DScope, Surface3DFlags, sys};

const SURFACE_LABEL_ERROR: &str = "surface label contains NUL";

#[derive(Debug, Clone, Copy)]
pub(crate) struct SurfaceLabel<'a>(&'a str);

impl<'a> SurfaceLabel<'a> {
    pub(crate) fn checked(label: &'a str) -> Result<Self, Plot3DError> {
        if label.contains('\0') {
            Err(Plot3DError::StringConversion(SURFACE_LABEL_ERROR))
        } else {
            Ok(Self(label))
        }
    }

    const fn as_str(self) -> &'a str {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SurfaceGridShape {
    x_count: i32,
    y_count: i32,
    point_count: usize,
}

impl SurfaceGridShape {
    pub(crate) fn checked(x_count: usize, y_count: usize) -> Result<Self, Plot3DError> {
        if x_count == 0 || y_count == 0 {
            return Err(Plot3DError::EmptyData);
        }

        let point_count = x_count
            .checked_mul(y_count)
            .ok_or(Plot3DError::GridSizeOverflow { x_count, y_count })?;
        if point_count > i32::MAX as usize {
            return Err(Plot3DError::GridPointCountOutOfRange {
                x_count,
                y_count,
                point_count,
            });
        }

        // Non-empty dimensions cannot exceed the already-checked point count.
        let range_error = || Plot3DError::GridPointCountOutOfRange {
            x_count,
            y_count,
            point_count,
        };
        let x_count_i32 = i32::try_from(x_count).map_err(|_| range_error())?;
        let y_count_i32 = i32::try_from(y_count).map_err(|_| range_error())?;

        Ok(Self {
            x_count: x_count_i32,
            y_count: y_count_i32,
            point_count,
        })
    }

    pub(crate) fn validate_layout(
        self,
        flags: Surface3DFlags,
        layout: Plot3DDataLayout,
    ) -> Result<Self, Plot3DError> {
        layout
            .sample_count(self.point_count)
            .ok_or(Plot3DError::NativeCountOutOfRange {
                what: "surface samples",
            })?;
        if !flags.contains(Surface3DFlags::NO_LINES) {
            let x = i64::from(self.x_count);
            let y = i64::from(self.y_count);
            // GetterSurfaceLines stores two endpoints per horizontal and vertical edge in int.
            if 2 * ((x - 1) * y + (y - 1) * x) > i64::from(i32::MAX) {
                return Err(Plot3DError::NativeCountOutOfRange {
                    what: "surface wireframe endpoints",
                });
            }
        }
        Ok(self)
    }

    fn validate_grid(self, zs: &[f32], flags: Surface3DFlags) -> Result<Self, Plot3DError> {
        if zs.len() != self.point_count {
            return Err(Plot3DError::GridSizeMismatch {
                x_count: self.x_count as usize,
                y_count: self.y_count as usize,
                expected: self.point_count,
                z_len: zs.len(),
            });
        }
        self.validate_layout(flags, Plot3DDataLayout::DEFAULT)?;
        self.validate_fill_heights(flags, zs.iter().copied())?;
        Ok(self)
    }

    fn validate_fill_heights(
        self,
        flags: Surface3DFlags,
        mut heights: impl Iterator<Item = f32>,
    ) -> Result<(), Plot3DError> {
        // The native fill renderer converts each normalized height into a colormap table index.
        if self.x_count > 1
            && self.y_count > 1
            && !flags.contains(Surface3DFlags::NO_FILL)
            && let Some(index) = heights.position(|z| !z.is_finite())
        {
            return Err(Plot3DError::NonFiniteSurfaceHeight { index });
        }
        Ok(())
    }

    pub(crate) const fn counts_i32(self) -> (i32, i32) {
        (self.x_count, self.y_count)
    }

    const fn point_count(self) -> usize {
        self.point_count
    }
}

#[derive(Debug)]
pub(crate) struct SurfaceGrid<'a> {
    shape: SurfaceGridShape,
    xs: Cow<'a, [f32]>,
    ys: Cow<'a, [f32]>,
    zs: &'a [f32],
}

impl<'a> SurfaceGrid<'a> {
    pub(crate) fn from_axes(
        xs: &'a [f32],
        ys: &'a [f32],
        zs: &'a [f32],
        flags: Surface3DFlags,
    ) -> Result<Self, Plot3DError> {
        let shape = SurfaceGridShape::checked(xs.len(), ys.len())?.validate_grid(zs, flags)?;
        let mut xs_flat = Vec::with_capacity(shape.point_count());
        let mut ys_flat = Vec::with_capacity(shape.point_count());

        for &y in ys {
            for &x in xs {
                xs_flat.push(x);
                ys_flat.push(y);
            }
        }

        Ok(Self {
            shape,
            xs: Cow::Owned(xs_flat),
            ys: Cow::Owned(ys_flat),
            zs,
        })
    }

    pub(crate) fn from_flattened(
        xs: &'a [f32],
        ys: &'a [f32],
        zs: &'a [f32],
        x_count: usize,
        y_count: usize,
        flags: Surface3DFlags,
    ) -> Result<Self, Plot3DError> {
        let shape = SurfaceGridShape::checked(x_count, y_count)?.validate_grid(zs, flags)?;
        validate_coordinate_len(xs.len(), shape.point_count(), "surface x coordinates")?;
        validate_coordinate_len(ys.len(), shape.point_count(), "surface y coordinates")?;

        Ok(Self {
            shape,
            xs: Cow::Borrowed(xs),
            ys: Cow::Borrowed(ys),
            zs,
        })
    }

    pub(crate) fn shape(&self) -> SurfaceGridShape {
        self.shape
    }

    pub(crate) fn xs(&self) -> &[f32] {
        self.xs.as_ref()
    }

    pub(crate) fn ys(&self) -> &[f32] {
        self.ys.as_ref()
    }

    pub(crate) fn zs(&self) -> &[f32] {
        self.zs
    }
}

fn validate_coordinate_len(
    actual: usize,
    expected: usize,
    what: &'static str,
) -> Result<(), Plot3DError> {
    if actual == expected {
        Ok(())
    } else {
        Err(Plot3DError::DataLengthMismatch {
            a: actual,
            b: expected,
            what,
        })
    }
}

fn surface_scale(min: f64, max: f64) -> Result<(f64, f64), Plot3DError> {
    if min.is_finite() && max.is_finite() {
        Ok((min, max))
    } else if min.is_nan() && max.is_nan() {
        // Earlier Rust builders used a pair of NaNs for automatic scaling.
        Ok((0.0, 0.0))
    } else {
        Err(Plot3DError::InvalidSurfaceScale)
    }
}

pub(crate) fn submit_surface_grid(
    ui: &Plot3DScope<'_>,
    label: SurfaceLabel<'_>,
    grid: &SurfaceGrid<'_>,
    scale_min: f64,
    scale_max: f64,
    make_spec: impl FnOnce() -> sys::ImPlot3DSpec_c,
) -> Result<(), Plot3DError> {
    let (scale_min, scale_max) = surface_scale(scale_min, scale_max)?;
    let label = label.as_str();
    let (x_count, y_count) = grid.shape().counts_i32();

    ui.with_bound_context(|| {
        let spec = make_spec();
        dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
            #[cfg(all(test, not(target_arch = "wasm32")))]
            sys::surface_test_probe::dear_implot3d_surface_probe_plot(
                label_ptr,
                grid.xs().as_ptr(),
                grid.ys().as_ptr(),
                grid.zs().as_ptr(),
                x_count,
                y_count,
                scale_min,
                scale_max,
                spec,
            );
            #[cfg(not(all(test, not(target_arch = "wasm32"))))]
            sys::ImPlot3D_PlotSurface_FloatPtr(
                label_ptr,
                grid.xs().as_ptr(),
                grid.ys().as_ptr(),
                grid.zs().as_ptr(),
                x_count,
                y_count,
                scale_min,
                scale_max,
                spec,
            );
        });
    });
    Ok(())
}

pub(crate) unsafe fn submit_surface_raw(
    ui: &Plot3DScope<'_>,
    label: SurfaceLabel<'_>,
    [xs, ys, zs]: [&[f32]; 3],
    shape: SurfaceGridShape,
    [scale_min, scale_max]: [f64; 2],
    flags: Surface3DFlags,
    layout: Plot3DDataLayout,
) -> Result<(), Plot3DError> {
    let (scale_min, scale_max) = surface_scale(scale_min, scale_max)?;
    let stride = usize::try_from(layout.raw_stride()).unwrap_or(size_of::<f32>());
    // A circular offset permutes the same samples. Walk them in storage order, skipping
    // unrelated fields. The raw caller guarantees each strided address is readable and aligned.
    shape.validate_fill_heights(
        flags,
        (0..shape.point_count()).map(|i| unsafe { zs.as_ptr().byte_add(i * stride).read() }),
    )?;
    let label = label.as_str();
    let (x_count, y_count) = shape.counts_i32();

    ui.with_bound_context(|| {
        let spec = ui.next_item.spec(flags.bits(), layout);
        dear_imgui_rs::with_scratch_txt(label, |label_ptr| unsafe {
            sys::ImPlot3D_PlotSurface_FloatPtr(
                label_ptr,
                xs.as_ptr(),
                ys.as_ptr(),
                zs.as_ptr(),
                x_count,
                y_count,
                scale_min,
                scale_max,
                spec,
            );
        });
    });
    Ok(())
}

/// A surface whose X and Y inputs describe grid axes. Filled surfaces require finite Z samples.
///
/// Custom offsets are intentionally unavailable on this safe type. Use the unsafe
/// [`Plot3DScope::surface_f32_raw`] escape hatch for arbitrary native layouts.
///
/// ```compile_fail
/// use dear_implot3d::{Plot3DDataOffset, Surface3D};
///
/// let xs = [0.0, 1.0];
/// let ys = [0.0, 1.0];
/// let zs = [0.0, 1.0, 1.0, 2.0];
/// let _ = Surface3D::new("surface", &xs, &ys, &zs)
///     .offset(Plot3DDataOffset::samples(1));
/// ```
pub struct Surface3D<'a> {
    pub label: &'a str,
    pub xs: &'a [f32],
    pub ys: &'a [f32],
    pub zs: &'a [f32],
    pub scale_min: f64,
    pub scale_max: f64,
    pub flags: Surface3DFlags,
}

impl<'a> Surface3D<'a> {
    pub fn new(label: &'a str, xs: &'a [f32], ys: &'a [f32], zs: &'a [f32]) -> Self {
        Self {
            label,
            xs,
            ys,
            zs,
            scale_min: 0.0,
            scale_max: 0.0,
            flags: Surface3DFlags::NONE,
        }
    }

    /// Set the color range. Two zeros (the default) or two NaNs select the grid's Z range.
    pub fn scale(mut self, min: f64, max: f64) -> Self {
        self.scale_min = min;
        self.scale_max = max;
        self
    }

    pub fn flags(mut self, flags: Surface3DFlags) -> Self {
        self.flags = flags;
        self
    }
}

impl<'a> Plot3D for Surface3D<'a> {
    fn label(&self) -> &str {
        self.label
    }

    fn try_plot(&self, ui: &Plot3DScope<'_>) -> Result<(), Plot3DError> {
        let label = SurfaceLabel::checked(self.label)?;
        let grid = SurfaceGrid::from_axes(self.xs, self.ys, self.zs, self.flags)?;
        submit_surface_grid(ui, label, &grid, self.scale_min, self.scale_max, || {
            ui.next_item
                .spec(self.flags.bits(), Plot3DDataLayout::DEFAULT)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    #[cfg(not(target_arch = "wasm32"))]
    use std::sync::{Mutex, OnceLock};

    use super::{Plot3D, Surface3D, SurfaceGrid, SurfaceGridShape, SurfaceLabel};
    use crate::{Plot3DDataLayout, Plot3DDataOffset, Surface3DFlags, plots::Plot3DError, sys};

    #[test]
    fn axis_grid_flattens_in_row_major_order() {
        let xs = [10.0, 20.0];
        let ys = [1.0, 2.0];
        let zs = [11.0, 21.0, 12.0, 22.0];

        let grid = SurfaceGrid::from_axes(&xs, &ys, &zs, Surface3DFlags::NONE).unwrap();

        assert_eq!(grid.xs(), &[10.0, 20.0, 10.0, 20.0]);
        assert_eq!(grid.ys(), &[1.0, 1.0, 2.0, 2.0]);
        assert_eq!(grid.zs(), &zs);
        assert_eq!(grid.shape().counts_i32(), (2, 2));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn cpp_probe_receives_equal_length_row_major_surface_arrays() {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard = GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let mut imgui = dear_imgui_rs::Context::create();
        let io = imgui.io_mut();
        io.set_display_size([800.0, 600.0]);
        io.set_delta_time(1.0 / 60.0);
        imgui
            .font_atlas()
            .try_claim_legacy_renderer()
            .expect("headless test requires the legacy font-atlas capability")
            .build();
        let plot_context = crate::Plot3DContext::create(&imgui);
        let frame = imgui.begin_frame();
        let plot_ui = plot_context.get_plot_ui(frame.ui());
        plot_ui
            .begin_plot("surface C++ capture")
            .build(|plot| {
                let xs = [10.0, 20.0];
                let ys = [1.0, 2.0];
                let zs = [11.0, 21.0, 12.0, 22.0];
                unsafe { sys::surface_test_probe::dear_implot3d_surface_probe_reset() };
                Surface3D::new("captured surface", &xs, &ys, &zs)
                    .try_plot(plot)
                    .unwrap();

                let mut captured_xs = [0.0; 4];
                let mut captured_ys = [0.0; 4];
                let mut captured_zs = [0.0; 4];
                let mut x_count = 0;
                let mut y_count = 0;
                let point_count = unsafe {
                    sys::surface_test_probe::dear_implot3d_surface_probe_read(
                        captured_xs.as_mut_ptr(),
                        captured_ys.as_mut_ptr(),
                        captured_zs.as_mut_ptr(),
                        captured_xs.len() as i32,
                        &mut x_count,
                        &mut y_count,
                    )
                };

                assert_eq!((x_count, y_count, point_count), (2, 2, 4));
                assert_eq!(captured_xs, [10.0, 20.0, 10.0, 20.0]);
                assert_eq!(captured_ys, [1.0, 1.0, 2.0, 2.0]);
                assert_eq!(captured_zs, zs);
            })
            .expect("surface capture plot should begin");
    }

    #[test]
    fn one_by_n_axis_grid_is_valid() {
        let xs = [3.0];
        let ys = [1.0, 2.0, 3.0];
        let zs = [4.0, 5.0, 6.0];

        let grid = SurfaceGrid::from_axes(&xs, &ys, &zs, Surface3DFlags::NONE).unwrap();

        assert_eq!(grid.xs(), &[3.0, 3.0, 3.0]);
        assert_eq!(grid.ys(), &ys);
        assert_eq!(grid.zs(), &zs);
    }

    #[test]
    fn empty_axes_are_rejected() {
        assert_eq!(SurfaceGridShape::checked(0, 1), Err(Plot3DError::EmptyData));
        assert_eq!(SurfaceGridShape::checked(1, 0), Err(Plot3DError::EmptyData));
    }

    #[test]
    fn surface_wireframe_counts_both_endpoints_of_each_grid_edge() {
        let layout = Plot3DDataLayout::DEFAULT;
        let shape = SurfaceGridShape::checked(32_768, 32_768).unwrap();
        assert!(matches!(
            shape.validate_layout(Surface3DFlags::NONE, layout),
            Err(Plot3DError::NativeCountOutOfRange { .. })
        ));
        assert_eq!(
            shape.validate_layout(Surface3DFlags::NO_LINES, layout),
            Ok(shape)
        );
        let narrow = SurfaceGridShape::checked(1, 32_768).unwrap();
        assert_eq!(
            narrow.validate_layout(Surface3DFlags::NONE, layout),
            Ok(narrow)
        );
    }

    #[test]
    fn large_surface_checks_circular_indices_even_with_lines_disabled() {
        let shape = SurfaceGridShape::checked(40_000, 40_000).unwrap();
        let offset = Plot3DDataLayout::DEFAULT.with_offset(Plot3DDataOffset::samples(-1));
        assert_eq!(
            shape.validate_layout(Surface3DFlags::NO_LINES, offset),
            Err(Plot3DError::NativeCountOutOfRange {
                what: "surface samples"
            })
        );
    }

    #[test]
    fn axis_grid_rejects_z_length_mismatch() {
        assert_eq!(
            SurfaceGrid::from_axes(
                &[0.0, 1.0],
                &[0.0, 1.0],
                &[0.0, 1.0, 2.0],
                Surface3DFlags::NONE
            )
            .unwrap_err(),
            Plot3DError::GridSizeMismatch {
                x_count: 2,
                y_count: 2,
                expected: 4,
                z_len: 3,
            }
        );
    }

    #[test]
    fn checked_shape_rejects_usize_multiplication_overflow() {
        let x_count = usize::MAX / 2 + 1;
        let y_count = 2;

        assert_eq!(
            SurfaceGridShape::checked(x_count, y_count),
            Err(Plot3DError::GridSizeOverflow { x_count, y_count })
        );
    }

    #[test]
    fn checked_shape_rejects_cpp_int_overflow_without_allocating() {
        let x_count = i32::MAX as usize;
        let y_count = 2;
        let point_count = x_count * y_count;

        assert_eq!(
            SurfaceGridShape::checked(x_count, y_count),
            Err(Plot3DError::GridPointCountOutOfRange {
                x_count,
                y_count,
                point_count,
            })
        );
    }

    #[test]
    fn flattened_grid_requires_all_coordinate_arrays_to_match() {
        let error = SurfaceGrid::from_flattened(
            &[0.0; 4],
            &[0.0; 3],
            &[0.0; 4],
            2,
            2,
            Surface3DFlags::NONE,
        )
        .unwrap_err();

        assert_eq!(
            error,
            Plot3DError::DataLengthMismatch {
                a: 3,
                b: 4,
                what: "surface y coordinates",
            }
        );
    }

    #[test]
    fn flattened_grid_borrows_contiguous_coordinates() {
        let xs = [0.0; 4];
        let ys = [0.0; 4];
        let zs = [0.0; 4];
        let grid = SurfaceGrid::from_flattened(&xs, &ys, &zs, 2, 2, Surface3DFlags::NONE).unwrap();

        assert!(matches!(grid.xs, Cow::Borrowed(_)));
        assert!(matches!(grid.ys, Cow::Borrowed(_)));
    }

    #[test]
    fn axis_and_flattened_inputs_share_z_shape_errors() {
        let axis_error =
            SurfaceGrid::from_axes(&[0.0, 1.0], &[0.0, 1.0], &[0.0; 3], Surface3DFlags::NONE)
                .unwrap_err();
        let flat_error = SurfaceGrid::from_flattened(
            &[0.0; 4],
            &[0.0; 4],
            &[0.0; 3],
            2,
            2,
            Surface3DFlags::NONE,
        )
        .unwrap_err();

        assert_eq!(axis_error, flat_error);
    }

    #[test]
    fn surface_validates_heights_before_the_native_colormap_lookup() {
        let axis = [0.0, 1.0];
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let zs = [0.0, invalid, 0.5, 1.0];
            assert_eq!(
                SurfaceGrid::from_axes(&axis, &axis, &zs, Surface3DFlags::NONE).unwrap_err(),
                Plot3DError::NonFiniteSurfaceHeight { index: 1 }
            );
            assert!(SurfaceGrid::from_axes(&axis, &axis, &zs, Surface3DFlags::NO_FILL).is_ok());
        }
    }

    #[test]
    fn surface_labels_reject_embedded_nul_consistently() {
        assert_eq!(
            SurfaceLabel::checked("invalid\0label").map(|_| ()),
            Err(Plot3DError::StringConversion("surface label contains NUL"))
        );
        assert_eq!(SurfaceLabel::checked("valid label").map(|_| ()), Ok(()));
    }

    #[test]
    fn grid_mismatch_display_uses_validated_expected_count() {
        let error = Plot3DError::GridSizeMismatch {
            x_count: usize::MAX,
            y_count: usize::MAX,
            expected: 4,
            z_len: 3,
        };

        assert_eq!(
            error.to_string(),
            format!(
                "grid mismatch: x={} y={} => expected z_len=4, got 3",
                usize::MAX,
                usize::MAX
            )
        );
    }
}

use std::borrow::Cow;

use super::{Plot3D, Plot3DError};
use crate::{Plot3DDataLayout, Plot3DUi, Surface3DFlags, debug_before_plot, plot3d_spec_from, sys};

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

    fn validate_z_len(self, z_len: usize) -> Result<Self, Plot3DError> {
        if z_len == self.point_count {
            Ok(self)
        } else {
            Err(Plot3DError::GridSizeMismatch {
                x_count: self.x_count as usize,
                y_count: self.y_count as usize,
                expected: self.point_count,
                z_len,
            })
        }
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
    ) -> Result<Self, Plot3DError> {
        let shape = SurfaceGridShape::checked(xs.len(), ys.len())?.validate_z_len(zs.len())?;
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
    ) -> Result<Self, Plot3DError> {
        let shape = SurfaceGridShape::checked(x_count, y_count)?.validate_z_len(zs.len())?;
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

pub(crate) fn submit_surface_grid(
    ui: &Plot3DUi<'_>,
    label: SurfaceLabel<'_>,
    grid: &SurfaceGrid<'_>,
    scale_min: f64,
    scale_max: f64,
    make_spec: impl FnOnce() -> sys::ImPlot3DSpec_c,
) {
    let label = label.as_str();
    let (x_count, y_count) = grid.shape().counts_i32();

    ui.with_bound_context(|| {
        debug_before_plot();
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
}

pub(crate) unsafe fn submit_surface_raw(
    ui: &Plot3DUi<'_>,
    label: SurfaceLabel<'_>,
    xs: &[f32],
    ys: &[f32],
    zs: &[f32],
    shape: SurfaceGridShape,
    scale_min: f64,
    scale_max: f64,
    flags: Surface3DFlags,
    layout: Plot3DDataLayout,
) {
    let label = label.as_str();
    let (x_count, y_count) = shape.counts_i32();

    ui.with_bound_context(|| {
        debug_before_plot();
        let spec = plot3d_spec_from(flags.bits(), layout);
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
}

/// A surface whose X and Y inputs describe grid axes.
///
/// Custom offsets are intentionally unavailable on this safe type. Use the unsafe
/// [`Plot3DUi::surface_f32_raw`] escape hatch for arbitrary native layouts.
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
            scale_min: f64::NAN,
            scale_max: f64::NAN,
            flags: Surface3DFlags::NONE,
        }
    }

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

    fn try_plot(&self, ui: &Plot3DUi<'_>) -> Result<(), Plot3DError> {
        let label = SurfaceLabel::checked(self.label)?;
        let grid = SurfaceGrid::from_axes(self.xs, self.ys, self.zs)?;
        submit_surface_grid(ui, label, &grid, self.scale_min, self.scale_max, || {
            plot3d_spec_from(self.flags.bits(), Plot3DDataLayout::DEFAULT)
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    #[cfg(not(target_arch = "wasm32"))]
    use std::sync::{Mutex, OnceLock};

    use super::{Plot3D, Surface3D, SurfaceGrid, SurfaceGridShape, SurfaceLabel};
    use crate::plots::Plot3DError;

    #[test]
    fn axis_grid_flattens_in_row_major_order() {
        let xs = [10.0, 20.0];
        let ys = [1.0, 2.0];
        let zs = [11.0, 21.0, 12.0, 22.0];

        let grid = SurfaceGrid::from_axes(&xs, &ys, &zs).unwrap();

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
        io.set_backend_flags(
            io.backend_flags() | dear_imgui_rs::BackendFlags::RENDERER_HAS_TEXTURES,
        );
        let plot_context = crate::Plot3DContext::create(&imgui);
        let frame = imgui.begin_frame();
        let plot_ui = plot_context.get_plot_ui(frame.ui());
        let _plot = plot_ui
            .begin_plot("surface C++ capture")
            .build()
            .expect("surface capture plot should begin");

        let xs = [10.0, 20.0];
        let ys = [1.0, 2.0];
        let zs = [11.0, 21.0, 12.0, 22.0];
        unsafe { sys::surface_test_probe::dear_implot3d_surface_probe_reset() };
        Surface3D::new("captured surface", &xs, &ys, &zs)
            .try_plot(&plot_ui)
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
    }

    #[test]
    fn one_by_n_axis_grid_is_valid() {
        let xs = [3.0];
        let ys = [1.0, 2.0, 3.0];
        let zs = [4.0, 5.0, 6.0];

        let grid = SurfaceGrid::from_axes(&xs, &ys, &zs).unwrap();

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
    fn axis_grid_rejects_z_length_mismatch() {
        assert_eq!(
            SurfaceGrid::from_axes(&[0.0, 1.0], &[0.0, 1.0], &[0.0, 1.0, 2.0]).unwrap_err(),
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
        let error = SurfaceGrid::from_flattened(&[0.0; 4], &[0.0; 3], &[0.0; 4], 2, 2).unwrap_err();

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
        let grid = SurfaceGrid::from_flattened(&xs, &ys, &zs, 2, 2).unwrap();

        assert!(matches!(grid.xs, Cow::Borrowed(_)));
        assert!(matches!(grid.ys, Cow::Borrowed(_)));
    }

    #[test]
    fn axis_and_flattened_inputs_share_z_shape_errors() {
        let axis_error = SurfaceGrid::from_axes(&[0.0, 1.0], &[0.0, 1.0], &[0.0; 3]).unwrap_err();
        let flat_error =
            SurfaceGrid::from_flattened(&[0.0; 4], &[0.0; 4], &[0.0; 3], 2, 2).unwrap_err();

        assert_eq!(axis_error, flat_error);
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

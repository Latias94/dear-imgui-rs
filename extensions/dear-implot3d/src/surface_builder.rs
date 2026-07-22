use std::borrow::Cow;

use crate::item_style::{Plot3DItemStyle, plot3d_spec_with_style};
use crate::plots::{
    Plot3DError, SurfaceGrid, SurfaceGridShape, SurfaceLabel, submit_surface_grid,
    submit_surface_raw,
};
use crate::{Item3DFlags, Plot3DDataLayout, Plot3DUi, Surface3DFlags, plot3d_spec_from};

/// Surface (grid) plot builder (f32 variant).
pub struct Surface3DBuilder<'ui> {
    pub(crate) _ui: &'ui Plot3DUi<'ui>,
    pub(crate) label: Cow<'ui, str>,
    pub(crate) xs: &'ui [f32],
    pub(crate) ys: &'ui [f32],
    pub(crate) zs: &'ui [f32],
    pub(crate) scale_min: f64,
    pub(crate) scale_max: f64,
    pub(crate) flags: Surface3DFlags,
    pub(crate) item_flags: Item3DFlags,
    pub(crate) style: Plot3DItemStyle,
}

impl<'ui> Surface3DBuilder<'ui> {
    pub fn scale(mut self, min: f64, max: f64) -> Self {
        self.scale_min = min;
        self.scale_max = max;
        self
    }

    pub fn flags(mut self, flags: Surface3DFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Submit the surface after validating its complete grid shape.
    #[must_use = "surface plot errors must be handled"]
    pub fn plot(self) -> Result<(), Plot3DError> {
        let Surface3DBuilder {
            _ui,
            label,
            xs,
            ys,
            zs,
            scale_min,
            scale_max,
            flags,
            item_flags,
            style,
        } = self;

        let label = SurfaceLabel::checked(label.as_ref())?;
        let grid = SurfaceGrid::from_axes(xs, ys, zs)?;
        submit_surface_grid(_ui, label, &grid, scale_min, scale_max, || {
            plot3d_spec_with_style(
                style,
                flags.bits() | item_flags.bits(),
                Plot3DDataLayout::DEFAULT,
            )
        });
        Ok(())
    }
}

impl<'ui> Plot3DUi<'ui> {
    /// Start a surface plot (f32) from X/Y grid axes.
    pub fn surface_f32(
        &'ui self,
        label: impl Into<Cow<'ui, str>>,
        xs: &'ui [f32],
        ys: &'ui [f32],
        zs: &'ui [f32],
    ) -> Surface3DBuilder<'ui> {
        self.with_bound_context(|| Surface3DBuilder {
            _ui: self,
            label: label.into(),
            xs,
            ys,
            zs,
            scale_min: f64::NAN,
            scale_max: f64::NAN,
            flags: Surface3DFlags::NONE,
            item_flags: Item3DFlags::NONE,
            style: Plot3DItemStyle::default(),
        })
    }

    /// Submit a surface from already flattened, contiguous per-vertex arrays.
    ///
    /// `xs_flat`, `ys_flat`, and `zs` must each contain exactly `x_count * y_count` values. The
    /// contiguous path does not accept an offset or stride; use [`Self::surface_f32_raw`] when
    /// the native layout is intentionally non-contiguous.
    ///
    /// ```no_run
    /// use dear_implot3d::{Plot3DError, Plot3DUi, Surface3DFlags};
    ///
    /// fn submit(plot_ui: &Plot3DUi<'_>) -> Result<(), Plot3DError> {
    ///     let xs = [0.0, 1.0, 0.0, 1.0];
    ///     let ys = [0.0, 0.0, 1.0, 1.0];
    ///     let zs = [0.0, 1.0, 1.0, 2.0];
    ///     plot_ui.surface_f32_flat(
    ///         "surface",
    ///         &xs,
    ///         &ys,
    ///         &zs,
    ///         2,
    ///         2,
    ///         0.0,
    ///         0.0,
    ///         Surface3DFlags::NONE,
    ///     )
    /// }
    /// ```
    #[must_use = "surface plot errors must be handled"]
    pub fn surface_f32_flat<S: AsRef<str>>(
        &self,
        label: S,
        xs_flat: &[f32],
        ys_flat: &[f32],
        zs: &[f32],
        x_count: usize,
        y_count: usize,
        scale_min: f64,
        scale_max: f64,
        flags: Surface3DFlags,
    ) -> Result<(), Plot3DError> {
        let label = SurfaceLabel::checked(label.as_ref())?;
        let grid = SurfaceGrid::from_flattened(xs_flat, ys_flat, zs, x_count, y_count)?;
        submit_surface_grid(self, label, &grid, scale_min, scale_max, || {
            plot3d_spec_from(flags.bits(), Plot3DDataLayout::DEFAULT)
        });
        Ok(())
    }

    /// Submit a surface with an explicitly arbitrary native data layout.
    ///
    /// # Safety
    ///
    /// The caller must ensure that every coordinate read performed by ImPlot3D from `xs`, `ys`,
    /// and `zs` using `layout`, `x_count`, and `y_count` addresses initialized, properly aligned,
    /// live `f32` values. The slices need not be exactly `x_count * y_count` elements because a
    /// custom stride or offset may address a larger allocation.
    ///
    /// Calling this arbitrary-layout API without an `unsafe` block is rejected:
    ///
    /// ```compile_fail
    /// use dear_implot3d::{Plot3DDataLayout, Plot3DUi, Surface3DFlags};
    ///
    /// fn submit(plot_ui: &Plot3DUi<'_>) {
    ///     let values = [0.0; 4];
    ///     let _ = plot_ui.surface_f32_raw(
    ///         "surface",
    ///         &values,
    ///         &values,
    ///         &values,
    ///         2,
    ///         2,
    ///         0.0,
    ///         0.0,
    ///         Surface3DFlags::NONE,
    ///         Plot3DDataLayout::DEFAULT,
    ///     );
    /// }
    /// ```
    #[must_use = "surface plot errors must be handled"]
    pub unsafe fn surface_f32_raw<S: AsRef<str>>(
        &self,
        label: S,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
        x_count: usize,
        y_count: usize,
        scale_min: f64,
        scale_max: f64,
        flags: Surface3DFlags,
        layout: Plot3DDataLayout,
    ) -> Result<(), Plot3DError> {
        let label = SurfaceLabel::checked(label.as_ref())?;
        let shape = SurfaceGridShape::checked(x_count, y_count)?;
        unsafe {
            submit_surface_raw(
                self, label, xs, ys, zs, shape, scale_min, scale_max, flags, layout,
            );
        }
        Ok(())
    }
}

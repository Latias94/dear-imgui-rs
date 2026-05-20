use crate::{Line3DFlags, Scatter3DFlags};

use super::core::Plot3DUi;

/// Optional mint support for inputs
///
/// When the `mint` feature is enabled, you can use `mint::Point3<f32>` and `mint::Vector3<f32>`
/// types directly with plotting functions. This provides interoperability with popular math
/// libraries like `glam`, `nalgebra`, `cgmath`, etc.
///
/// # Example
///
/// ```no_run
/// # #[cfg(feature = "mint")]
/// # {
/// use dear_implot3d::*;
/// use mint::Point3;
///
/// # let plot_ui: Plot3DUi = todo!();
/// let points = vec![
///     Point3 { x: 0.0, y: 0.0, z: 0.0 },
///     Point3 { x: 1.0, y: 1.0, z: 1.0 },
///     Point3 { x: 2.0, y: 0.0, z: 2.0 },
/// ];
///
/// if let Some(_token) = plot_ui.begin_plot("Mint Example").build() {
///     plot_ui.plot_line_mint("Line", &points, Line3DFlags::NONE);
/// }
/// # }
/// ```
#[cfg(feature = "mint")]
impl<'ui> Plot3DUi<'ui> {
    /// Plot a 3D line using `mint::Point3<f32>` points
    ///
    /// This is a convenience function that converts mint points to separate x, y, z arrays.
    pub fn plot_line_mint<S: AsRef<str>>(
        &self,
        label: S,
        pts: &[mint::Point3<f32>],
        flags: Line3DFlags,
    ) {
        let mut xs = Vec::with_capacity(pts.len());
        let mut ys = Vec::with_capacity(pts.len());
        let mut zs = Vec::with_capacity(pts.len());
        for p in pts {
            xs.push(p.x);
            ys.push(p.y);
            zs.push(p.z);
        }
        self.plot_line_f32(label, &xs, &ys, &zs, flags);
    }

    /// Plot a 3D scatter using `mint::Point3<f32>` points
    pub fn plot_scatter_mint<S: AsRef<str>>(
        &self,
        label: S,
        pts: &[mint::Point3<f32>],
        flags: Scatter3DFlags,
    ) {
        let mut xs = Vec::with_capacity(pts.len());
        let mut ys = Vec::with_capacity(pts.len());
        let mut zs = Vec::with_capacity(pts.len());
        for p in pts {
            xs.push(p.x);
            ys.push(p.y);
            zs.push(p.z);
        }
        self.plot_scatter_f32(label, &xs, &ys, &zs, flags);
    }

    /// Plot 3D text at a `mint::Point3<f32>` position
    pub fn plot_text_mint(
        &self,
        text: &str,
        pos: mint::Point3<f32>,
        angle: f32,
        pix_offset: [f32; 2],
    ) {
        self.plot_text(text, pos.x, pos.y, pos.z, angle, pix_offset);
    }

    /// Convert a `mint::Point3<f32>` to pixel coordinates
    pub fn plot_to_pixels_mint(&self, point: mint::Point3<f32>) -> [f32; 2] {
        self.plot_to_pixels([point.x, point.y, point.z])
    }
}

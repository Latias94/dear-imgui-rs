//! Modular plot types for ImPlot3D
//!
//! Builder-oriented API mirroring dear-implot's `plots` module.
//! Validation helpers are implementation details and are not part of the public API:
//!
//! ```compile_fail
//! use dear_implot3d::plots::{primitive_vertex_count, validate_lengths, validate_nonempty};
//! ```
//!
//! ```compile_fail
//! use dear_implot3d::{primitive_vertex_count, validate_lengths, validate_nonempty};
//! ```

pub mod image;
pub mod line;
pub mod mesh;
pub mod quads;
pub mod scatter;
pub mod surface;
pub mod triangles;

pub use image::*;
pub use line::*;
pub use mesh::*;
pub use quads::*;
pub use scatter::*;
pub use surface::*;
pub use triangles::*;

/// Errors that can occur during 3D plotting
#[derive(Debug, Clone, PartialEq)]
pub enum Plot3DError {
    EmptyData,
    DataLengthMismatch {
        a: usize,
        b: usize,
        what: &'static str,
    },
    NotMultipleOf {
        len: usize,
        k: usize,
        what: &'static str,
    },
    NativeCountOutOfRange {
        what: &'static str,
    },
    MeshIndexOutOfBounds {
        index: u32,
        vertex_count: usize,
    },
    GridSizeMismatch {
        x_count: usize,
        y_count: usize,
        expected: usize,
        z_len: usize,
    },
    GridSizeOverflow {
        x_count: usize,
        y_count: usize,
    },
    GridPointCountOutOfRange {
        x_count: usize,
        y_count: usize,
        point_count: usize,
    },
    InvalidSurfaceScale,
    NonFiniteSurfaceHeight {
        index: usize,
    },
    StringConversion(&'static str),
}

impl std::fmt::Display for Plot3DError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Plot3DError::EmptyData => write!(f, "data is empty"),
            Plot3DError::DataLengthMismatch { a, b, what } => {
                write!(f, "length mismatch for {}: {} vs {}", what, a, b)
            }
            Plot3DError::NotMultipleOf { len, k, what } => {
                write!(f, "length of {} = {} is not multiple of {}", what, len, k)
            }
            Plot3DError::NativeCountOutOfRange { what } => {
                write!(f, "{what} exceeded ImPlot3D's i32 range")
            }
            Plot3DError::MeshIndexOutOfBounds {
                index,
                vertex_count,
            } => {
                write!(
                    f,
                    "mesh index {index} is out of bounds for {vertex_count} vertices"
                )
            }
            Plot3DError::GridSizeMismatch {
                x_count,
                y_count,
                expected,
                z_len,
            } => write!(
                f,
                "grid mismatch: x={} y={} => expected z_len={}, got {}",
                x_count, y_count, expected, z_len
            ),
            Plot3DError::GridSizeOverflow { x_count, y_count } => write!(
                f,
                "surface grid size overflow: x_count={} y_count={}",
                x_count, y_count
            ),
            Plot3DError::GridPointCountOutOfRange {
                x_count,
                y_count,
                point_count,
            } => write!(
                f,
                "surface grid x_count={} y_count={} has {} points, exceeding ImPlot3D's i32 range",
                x_count, y_count, point_count
            ),
            Plot3DError::InvalidSurfaceScale => f.write_str(
                "surface color scale endpoints must be finite or both NaN for automatic scaling",
            ),
            Plot3DError::NonFiniteSurfaceHeight { index } => {
                write!(f, "surface height at sample {index} must be finite")
            }
            Plot3DError::StringConversion(what) => write!(f, "string conversion error: {}", what),
        }
    }
}

impl std::error::Error for Plot3DError {}

/// Common trait for 3D plot elements.
pub trait Plot3D {
    fn label(&self) -> &str;
    /// Submits this item. Built-in items reject labels containing NUL with
    /// [`Plot3DError::StringConversion`].
    fn try_plot(&self, ui: &crate::Plot3DScope<'_>) -> Result<(), Plot3DError>;
    fn plot(&self, ui: &crate::Plot3DScope<'_>) {
        let _ = self.try_plot(ui);
    }
}

pub(crate) fn validate_item_label(label: &str) -> Result<(), Plot3DError> {
    if label.contains('\0') {
        return Err(Plot3DError::StringConversion(
            "plot item label contains NUL",
        ));
    }
    Ok(())
}

#[inline]
pub(crate) fn validate_nonempty<T>(a: &[T]) -> Result<(), Plot3DError> {
    if a.is_empty() {
        Err(Plot3DError::EmptyData)
    } else {
        Ok(())
    }
}

#[inline]
pub(crate) fn validate_lengths<T, U>(
    a: &[T],
    b: &[U],
    what: &'static str,
) -> Result<(), Plot3DError> {
    if a.len() != b.len() {
        Err(Plot3DError::DataLengthMismatch {
            a: a.len(),
            b: b.len(),
            what,
        })
    } else {
        Ok(())
    }
}

#[inline]
pub(crate) fn primitive_vertex_count(
    len: usize,
    k: usize,
    what: &'static str,
    layout: crate::Plot3DDataLayout,
    draw_lines: bool,
) -> Result<i32, Plot3DError> {
    if len == 0 {
        return Err(Plot3DError::EmptyData);
    }
    if !len.is_multiple_of(k) {
        return Err(Plot3DError::NotMultipleOf { len, k, what });
    }
    // The native triangle and quad wireframe getters double the vertex count.
    if draw_lines && len > i32::MAX as usize / 2 {
        return Err(Plot3DError::NativeCountOutOfRange { what });
    }
    layout
        .sample_count(len)
        .ok_or(Plot3DError::NativeCountOutOfRange { what })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Plot3DDataLayout;

    #[test]
    fn complete_faces_fit_the_selected_native_renderer() {
        let layout = Plot3DDataLayout::DEFAULT;
        assert!(matches!(
            primitive_vertex_count(4, 3, "triangles", layout, true),
            Err(Plot3DError::NotMultipleOf { .. })
        ));
        let count = (i32::MAX as usize / 2 + 1).next_multiple_of(3);
        assert!(primitive_vertex_count(count, 3, "triangles", layout, true).is_err());
        assert_eq!(
            primitive_vertex_count(count, 3, "triangles", layout, false),
            Ok(count as i32)
        );
    }
}

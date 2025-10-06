//! Modular plot types for ImPlot3D
//!
//! Builder-oriented API mirroring dear-implot's `plots` module.

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
    GridSizeMismatch {
        x_count: usize,
        y_count: usize,
        z_len: usize,
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
            Plot3DError::GridSizeMismatch {
                x_count,
                y_count,
                z_len,
            } => write!(
                f,
                "grid mismatch: x={} y={} => expected z_len={}, got {}",
                x_count,
                y_count,
                x_count * y_count,
                z_len
            ),
            Plot3DError::StringConversion(what) => write!(f, "string conversion error: {}", what),
        }
    }
}

impl std::error::Error for Plot3DError {}

/// Common trait for 3D plot elements
pub trait Plot3D {
    fn label(&self) -> &str;
    fn try_plot(&self, ui: &crate::Plot3DUi<'_>) -> Result<(), Plot3DError>;
    fn plot(&self, ui: &crate::Plot3DUi<'_>) {
        let _ = self.try_plot(ui);
    }
}

#[inline]
pub fn validate_nonempty<T>(a: &[T]) -> Result<(), Plot3DError> {
    if a.is_empty() {
        Err(Plot3DError::EmptyData)
    } else {
        Ok(())
    }
}

#[inline]
pub fn validate_lengths<T, U>(a: &[T], b: &[U], what: &'static str) -> Result<(), Plot3DError> {
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
pub fn validate_multiple(len: usize, k: usize, what: &'static str) -> Result<(), Plot3DError> {
    if len % k != 0 {
        Err(Plot3DError::NotMultipleOf { len, k, what })
    } else {
        Ok(())
    }
}

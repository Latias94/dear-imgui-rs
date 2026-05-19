use crate::io::{Io, metric_count_from_i32};

impl Io {
    /// Framerate estimation, in frames per second
    pub fn framerate(&self) -> f32 {
        self.inner().Framerate
    }

    /// Vertices output during last call to render
    pub fn metrics_render_vertices(&self) -> usize {
        metric_count_from_i32(
            "Io::metrics_render_vertices()",
            self.inner().MetricsRenderVertices,
        )
    }

    /// Indices output during last call to render
    pub fn metrics_render_indices(&self) -> usize {
        metric_count_from_i32(
            "Io::metrics_render_indices()",
            self.inner().MetricsRenderIndices,
        )
    }

    /// Number of visible windows
    pub fn metrics_render_windows(&self) -> usize {
        metric_count_from_i32(
            "Io::metrics_render_windows()",
            self.inner().MetricsRenderWindows,
        )
    }

    /// Number of active windows
    pub fn metrics_active_windows(&self) -> usize {
        metric_count_from_i32(
            "Io::metrics_active_windows()",
            self.inner().MetricsActiveWindows,
        )
    }
}

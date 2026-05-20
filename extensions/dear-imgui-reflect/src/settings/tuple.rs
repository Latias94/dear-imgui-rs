use super::*;

/// Preferred render mode for tuple-like values.
#[derive(Clone, Copy, Debug)]
pub enum TupleRenderMode {
    /// Render all elements on a single line.
    Line,
    /// Render elements inside an ImGui table with multiple columns.
    Grid,
}

/// Settings controlling how tuple-like values such as `(A, B)` and `(A, B, C)`
/// are rendered.
#[derive(Clone, Debug)]
pub struct TupleSettings {
    /// Whether the tuple contents are wrapped in a collapsible tree node.
    pub dropdown: bool,
    /// How tuple elements are laid out: line or grid.
    pub render_mode: TupleRenderMode,
    /// Number of columns to use in grid mode (clamped to at least 1 and at
    /// most the number of tuple elements).
    pub columns: usize,
    /// Whether the outer label is rendered on the same line as the tuple
    /// contents (line mode) or above them.
    pub same_line: bool,
    /// Optional minimum width for each element when rendered in grid mode.
    pub min_width: Option<f32>,
}

impl Default for TupleSettings {
    fn default() -> Self {
        Self {
            dropdown: false,
            render_mode: TupleRenderMode::Line,
            columns: 3,
            same_line: true,
            min_width: None,
        }
    }
}

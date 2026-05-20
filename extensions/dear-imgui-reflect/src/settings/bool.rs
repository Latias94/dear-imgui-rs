/// Preferred widget style for boolean fields.
#[derive(Clone, Copy, Debug)]
pub enum BoolStyle {
    /// Render using a standard ImGui checkbox.
    Checkbox,
    /// Render using a toggle button with text for true/false.
    Button,
    /// Render using two radio buttons (true/false).
    Radio,
    /// Render using a two-item dropdown (false/true).
    Dropdown,
}

/// Settings controlling how `bool` fields are edited when no per-field
/// attributes are provided.
#[derive(Clone, Debug)]
pub struct BoolSettings {
    /// Default widget style for `bool` fields.
    pub style: BoolStyle,
}

impl Default for BoolSettings {
    fn default() -> Self {
        Self {
            style: BoolStyle::Checkbox,
        }
    }
}

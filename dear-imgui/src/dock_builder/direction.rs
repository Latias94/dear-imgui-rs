use crate::sys;

/// Direction for splitting dock nodes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    /// Split to the left
    Left,
    /// Split to the right
    Right,
    /// Split upward
    Up,
    /// Split downward
    Down,
}

impl From<SplitDirection> for sys::ImGuiDir {
    fn from(dir: SplitDirection) -> Self {
        match dir {
            SplitDirection::Left => sys::ImGuiDir_Left,
            SplitDirection::Right => sys::ImGuiDir_Right,
            SplitDirection::Up => sys::ImGuiDir_Up,
            SplitDirection::Down => sys::ImGuiDir_Down,
        }
    }
}

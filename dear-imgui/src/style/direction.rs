#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]

use crate::sys;

/// A cardinal direction
#[repr(i32)]
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum Direction {
    None = sys::ImGuiDir_None as i32,
    Left = sys::ImGuiDir_Left as i32,
    Right = sys::ImGuiDir_Right as i32,
    Up = sys::ImGuiDir_Up as i32,
    Down = sys::ImGuiDir_Down as i32,
}

impl From<sys::ImGuiDir> for Direction {
    fn from(d: sys::ImGuiDir) -> Self {
        match d as i32 {
            x if x == sys::ImGuiDir_Left as i32 => Direction::Left,
            x if x == sys::ImGuiDir_Right as i32 => Direction::Right,
            x if x == sys::ImGuiDir_Up as i32 => Direction::Up,
            x if x == sys::ImGuiDir_Down as i32 => Direction::Down,
            _ => Direction::None,
        }
    }
}

impl From<Direction> for sys::ImGuiDir {
    fn from(d: Direction) -> Self {
        match d {
            Direction::None => sys::ImGuiDir_None,
            Direction::Left => sys::ImGuiDir_Left,
            Direction::Right => sys::ImGuiDir_Right,
            Direction::Up => sys::ImGuiDir_Up,
            Direction::Down => sys::ImGuiDir_Down,
        }
    }
}

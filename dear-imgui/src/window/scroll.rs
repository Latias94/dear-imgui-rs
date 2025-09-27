//! Window scrolling
//!
//! Read and control the current window scroll offsets as well as their maxima
//! to implement custom scrolling behaviors.
//!
use crate::Ui;
use crate::sys;

impl Ui {
    /// Returns the current scroll position of the window
    #[doc(alias = "GetScrollX")]
    pub fn scroll_x(&self) -> f32 {
        unsafe { sys::igGetScrollX() }
    }

    /// Returns the current vertical scroll position of the window
    #[doc(alias = "GetScrollY")]
    pub fn scroll_y(&self) -> f32 {
        unsafe { sys::igGetScrollY() }
    }

    /// Returns the maximum horizontal scroll position
    #[doc(alias = "GetScrollMaxX")]
    pub fn scroll_max_x(&self) -> f32 {
        unsafe { sys::igGetScrollMaxX() }
    }

    /// Returns the maximum vertical scroll position
    #[doc(alias = "GetScrollMaxY")]
    pub fn scroll_max_y(&self) -> f32 {
        unsafe { sys::igGetScrollMaxY() }
    }

    /// Sets the horizontal scroll position
    #[doc(alias = "SetScrollX")]
    pub fn set_scroll_x(&self, scroll_x: f32) {
        unsafe {
            sys::igSetScrollX_Float(scroll_x);
        }
    }

    /// Sets the vertical scroll position
    #[doc(alias = "SetScrollY")]
    pub fn set_scroll_y(&self, scroll_y: f32) {
        unsafe {
            sys::igSetScrollY_Float(scroll_y);
        }
    }

    /// Sets the horizontal scroll position to center on the given position
    ///
    /// The center_x_ratio parameter should be between 0.0 (left) and 1.0 (right)
    #[doc(alias = "SetScrollFromPosX")]
    pub fn set_scroll_from_pos_x(&self, local_x: f32, center_x_ratio: f32) {
        unsafe {
            sys::igSetScrollFromPosX_Float(local_x, center_x_ratio);
        }
    }

    /// Sets the vertical scroll position to center on the given position
    ///
    /// The center_y_ratio parameter should be between 0.0 (top) and 1.0 (bottom)
    #[doc(alias = "SetScrollFromPosY")]
    pub fn set_scroll_from_pos_y(&self, local_y: f32, center_y_ratio: f32) {
        unsafe {
            sys::igSetScrollFromPosY_Float(local_y, center_y_ratio);
        }
    }

    /// Scrolls to make the current item visible
    ///
    /// This is useful when you want to ensure a specific item is visible in a scrollable region
    #[doc(alias = "SetScrollHereX")]
    pub fn set_scroll_here_x(&self, center_x_ratio: f32) {
        unsafe {
            sys::igSetScrollHereX(center_x_ratio);
        }
    }

    /// Scrolls to make the current item visible vertically
    ///
    /// This is useful when you want to ensure a specific item is visible in a scrollable region
    #[doc(alias = "SetScrollHereY")]
    pub fn set_scroll_here_y(&self, center_y_ratio: f32) {
        unsafe {
            sys::igSetScrollHereY(center_y_ratio);
        }
    }
}

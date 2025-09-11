//! Utility functions and helpers for ImNodeFlow

use crate::*;

/// Utility functions for working with ImVec2
pub mod vec2 {
    use super::ImVec2;

    /// Create a new ImVec2
    pub fn new(x: f32, y: f32) -> ImVec2 {
        ImVec2 { x, y }
    }

    /// Create a zero vector
    pub fn zero() -> ImVec2 {
        ImVec2 { x: 0.0, y: 0.0 }
    }

    /// Create a vector with both components set to the same value
    pub fn splat(value: f32) -> ImVec2 {
        ImVec2 { x: value, y: value }
    }

    /// Add two vectors
    pub fn add(a: ImVec2, b: ImVec2) -> ImVec2 {
        ImVec2 {
            x: a.x + b.x,
            y: a.y + b.y,
        }
    }

    /// Subtract two vectors
    pub fn sub(a: ImVec2, b: ImVec2) -> ImVec2 {
        ImVec2 {
            x: a.x - b.x,
            y: a.y - b.y,
        }
    }

    /// Multiply a vector by a scalar
    pub fn mul(v: ImVec2, scalar: f32) -> ImVec2 {
        ImVec2 {
            x: v.x * scalar,
            y: v.y * scalar,
        }
    }

    /// Calculate the distance between two points
    pub fn distance(a: ImVec2, b: ImVec2) -> f32 {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Calculate the length of a vector
    pub fn length(v: ImVec2) -> f32 {
        (v.x * v.x + v.y * v.y).sqrt()
    }

    /// Normalize a vector
    pub fn normalize(v: ImVec2) -> ImVec2 {
        let len = length(v);
        if len > 0.0 {
            ImVec2 {
                x: v.x / len,
                y: v.y / len,
            }
        } else {
            zero()
        }
    }

    /// Linear interpolation between two vectors
    pub fn lerp(a: ImVec2, b: ImVec2, t: f32) -> ImVec2 {
        ImVec2 {
            x: a.x + (b.x - a.x) * t,
            y: a.y + (b.y - a.y) * t,
        }
    }
}

/// Utility functions for working with ImVec4
pub mod vec4 {
    use super::ImVec4;

    /// Create a new ImVec4
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> ImVec4 {
        ImVec4 { x, y, z, w }
    }

    /// Create a zero vector
    pub fn zero() -> ImVec4 {
        ImVec4 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        }
    }

    /// Create a vector with all components set to the same value
    pub fn splat(value: f32) -> ImVec4 {
        ImVec4 {
            x: value,
            y: value,
            z: value,
            w: value,
        }
    }
}

/// Grid utilities for node positioning
pub mod grid {
    use super::*;

    /// Snap a position to a grid
    pub fn snap_to_grid(pos: ImVec2, grid_size: f32) -> ImVec2 {
        ImVec2 {
            x: (pos.x / grid_size).round() * grid_size,
            y: (pos.y / grid_size).round() * grid_size,
        }
    }

    /// Calculate grid lines for drawing
    pub fn grid_lines(
        viewport_min: ImVec2,
        viewport_max: ImVec2,
        grid_size: f32,
        offset: ImVec2,
    ) -> (Vec<ImVec2>, Vec<ImVec2>) {
        let mut vertical_lines = Vec::new();
        let mut horizontal_lines = Vec::new();

        // Calculate the range of grid lines to draw
        let start_x = ((viewport_min.x - offset.x) / grid_size).floor() * grid_size + offset.x;
        let end_x = ((viewport_max.x - offset.x) / grid_size).ceil() * grid_size + offset.x;
        let start_y = ((viewport_min.y - offset.y) / grid_size).floor() * grid_size + offset.y;
        let end_y = ((viewport_max.y - offset.y) / grid_size).ceil() * grid_size + offset.y;

        // Generate vertical lines
        let mut x = start_x;
        while x <= end_x {
            vertical_lines.push(ImVec2 {
                x,
                y: viewport_min.y,
            });
            vertical_lines.push(ImVec2 {
                x,
                y: viewport_max.y,
            });
            x += grid_size;
        }

        // Generate horizontal lines
        let mut y = start_y;
        while y <= end_y {
            horizontal_lines.push(ImVec2 {
                x: viewport_min.x,
                y,
            });
            horizontal_lines.push(ImVec2 {
                x: viewport_max.x,
                y,
            });
            y += grid_size;
        }

        (vertical_lines, horizontal_lines)
    }
}

/// Animation utilities
pub mod animation {
    use super::*;

    /// Easing functions for smooth animations
    pub mod easing {
        /// Linear interpolation (no easing)
        pub fn linear(t: f32) -> f32 {
            t
        }

        /// Ease in (slow start)
        pub fn ease_in(t: f32) -> f32 {
            t * t
        }

        /// Ease out (slow end)
        pub fn ease_out(t: f32) -> f32 {
            1.0 - (1.0 - t) * (1.0 - t)
        }

        /// Ease in-out (slow start and end)
        pub fn ease_in_out(t: f32) -> f32 {
            if t < 0.5 {
                2.0 * t * t
            } else {
                1.0 - 2.0 * (1.0 - t) * (1.0 - t)
            }
        }

        /// Bounce easing
        pub fn bounce(t: f32) -> f32 {
            if t < 1.0 / 2.75 {
                7.5625 * t * t
            } else if t < 2.0 / 2.75 {
                let t = t - 1.5 / 2.75;
                7.5625 * t * t + 0.75
            } else if t < 2.5 / 2.75 {
                let t = t - 2.25 / 2.75;
                7.5625 * t * t + 0.9375
            } else {
                let t = t - 2.625 / 2.75;
                7.5625 * t * t + 0.984375
            }
        }
    }

    /// Simple animator for smooth transitions
    pub struct Animator {
        start_value: f32,
        end_value: f32,
        duration: f32,
        elapsed: f32,
        easing_fn: fn(f32) -> f32,
    }

    impl Animator {
        /// Create a new animator
        pub fn new(start: f32, end: f32, duration: f32) -> Self {
            Self {
                start_value: start,
                end_value: end,
                duration,
                elapsed: 0.0,
                easing_fn: easing::linear,
            }
        }

        /// Set the easing function
        pub fn with_easing(mut self, easing_fn: fn(f32) -> f32) -> Self {
            self.easing_fn = easing_fn;
            self
        }

        /// Update the animator with delta time
        pub fn update(&mut self, delta_time: f32) {
            self.elapsed = (self.elapsed + delta_time).min(self.duration);
        }

        /// Get the current animated value
        pub fn value(&self) -> f32 {
            if self.duration <= 0.0 {
                return self.end_value;
            }

            let t = self.elapsed / self.duration;
            let eased_t = (self.easing_fn)(t);
            self.start_value + (self.end_value - self.start_value) * eased_t
        }

        /// Check if the animation is complete
        pub fn is_complete(&self) -> bool {
            self.elapsed >= self.duration
        }

        /// Reset the animation
        pub fn reset(&mut self) {
            self.elapsed = 0.0;
        }

        /// Set new target values
        pub fn set_target(&mut self, start: f32, end: f32, duration: f32) {
            self.start_value = start;
            self.end_value = end;
            self.duration = duration;
            self.elapsed = 0.0;
        }
    }
}

/// String utilities for working with C strings
pub mod string {
    use std::ffi::{CStr, CString};

    /// Convert a Rust string to a C string safely
    pub fn to_cstring(s: &str) -> Result<CString, crate::NodeFlowError> {
        Ok(CString::new(s)?)
    }

    /// Convert a C string pointer to a Rust string safely
    pub unsafe fn from_cstr_ptr(
        ptr: *const std::os::raw::c_char,
    ) -> Result<String, crate::NodeFlowError> {
        if ptr.is_null() {
            return Ok(String::new());
        }
        let cstr = CStr::from_ptr(ptr);
        Ok(cstr.to_str()?.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vec2_operations() {
        let a = vec2::new(1.0, 2.0);
        let b = vec2::new(3.0, 4.0);

        let sum = vec2::add(a, b);
        assert_eq!(sum.x, 4.0);
        assert_eq!(sum.y, 6.0);

        let diff = vec2::sub(b, a);
        assert_eq!(diff.x, 2.0);
        assert_eq!(diff.y, 2.0);

        let scaled = vec2::mul(a, 2.0);
        assert_eq!(scaled.x, 2.0);
        assert_eq!(scaled.y, 4.0);

        let distance = vec2::distance(a, b);
        assert!((distance - 2.828427).abs() < 0.001);
    }

    #[test]
    fn test_grid_snap() {
        let pos = vec2::new(23.7, 47.3);
        let snapped = grid::snap_to_grid(pos, 10.0);
        assert_eq!(snapped.x, 20.0);
        assert_eq!(snapped.y, 50.0);
    }

    #[test]
    fn test_animator() {
        let mut animator = animation::Animator::new(0.0, 100.0, 1.0);

        assert_eq!(animator.value(), 0.0);
        assert!(!animator.is_complete());

        animator.update(0.5);
        assert_eq!(animator.value(), 50.0);
        assert!(!animator.is_complete());

        animator.update(0.5);
        assert_eq!(animator.value(), 100.0);
        assert!(animator.is_complete());
    }

    #[test]
    fn test_easing_functions() {
        use animation::easing::*;

        assert_eq!(linear(0.5), 0.5);
        assert!(ease_in(0.5) < 0.5);
        assert!(ease_out(0.5) > 0.5);
        assert!(ease_in_out(0.25) < 0.25);
        assert!(ease_in_out(0.75) > 0.75);
    }
}

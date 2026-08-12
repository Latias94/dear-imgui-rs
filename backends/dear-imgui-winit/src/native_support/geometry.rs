use thiserror::Error;

/// A validated rectangle in Winit's physical desktop coordinate space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicalMonitorRect {
    position: [f64; 2],
    size: [f64; 2],
}

/// A reason why native geometry could not be represented safely.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RectValidationError {
    #[error("rectangle coordinates must be finite")]
    NonFinite,
    #[error("rectangle size must be non-negative")]
    NegativeSize,
    #[error("rectangle must have positive area")]
    ZeroArea,
    #[error("rectangle edge arithmetic overflowed")]
    EdgeOverflow,
}

impl PhysicalMonitorRect {
    /// Creates a rectangle after validating its coordinates and dimensions.
    pub fn new(position: [f64; 2], size: [f64; 2]) -> Result<Self, RectValidationError> {
        if !position.into_iter().chain(size).all(f64::is_finite) {
            return Err(RectValidationError::NonFinite);
        }
        if size.iter().any(|value| *value < 0.0) {
            return Err(RectValidationError::NegativeSize);
        }
        for (origin, extent) in position.into_iter().zip(size) {
            if !(origin + extent).is_finite() {
                return Err(RectValidationError::EdgeOverflow);
            }
        }
        Ok(Self { position, size })
    }

    pub(crate) fn from_i32_u32(
        position: [i32; 2],
        size: [u32; 2],
    ) -> Result<Self, RectValidationError> {
        Self::new(
            [f64::from(position[0]), f64::from(position[1])],
            [f64::from(size[0]), f64::from(size[1])],
        )
    }

    /// Returns the top-left position.
    pub fn position(self) -> [f64; 2] {
        self.position
    }

    /// Returns the non-negative width and height.
    pub fn size(self) -> [f64; 2] {
        self.size
    }

    /// Returns the exclusive bottom-right edge.
    pub fn max(self) -> [f64; 2] {
        [
            self.position[0] + self.size[0],
            self.position[1] + self.size[1],
        ]
    }

    pub(crate) fn has_positive_area(self) -> bool {
        self.size[0] > 0.0 && self.size[1] > 0.0
    }

    pub(crate) fn contains(self, other: Self) -> bool {
        let self_max = self.max();
        let other_max = other.max();
        other.position[0] >= self.position[0]
            && other.position[1] >= self.position[1]
            && other_max[0] <= self_max[0]
            && other_max[1] <= self_max[1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_finite_and_negative_rectangles() {
        assert_eq!(
            PhysicalMonitorRect::new([f64::NAN, 0.0], [1.0, 1.0]),
            Err(RectValidationError::NonFinite)
        );
        assert_eq!(
            PhysicalMonitorRect::new([0.0, 0.0], [-1.0, 1.0]),
            Err(RectValidationError::NegativeSize)
        );
        assert!(PhysicalMonitorRect::new([0.0, 0.0], [0.0, 1.0]).is_ok());
    }

    #[test]
    fn accepts_negative_origins_and_checks_containment() {
        let main = PhysicalMonitorRect::new([-1920.0, -100.0], [1920.0, 1080.0]).unwrap();
        let work = PhysicalMonitorRect::new([-1920.0, -60.0], [1920.0, 1040.0]).unwrap();
        assert!(main.contains(work));
        assert_eq!(work.max(), [0.0, 980.0]);
    }
}

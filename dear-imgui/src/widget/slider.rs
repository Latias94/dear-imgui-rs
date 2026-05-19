//! Sliders
//!
//! Scalar and range sliders for numeric input with builder-based configuration
//! of speed, format and clamping.
//!
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]

mod angle;
mod flags;
mod horizontal;
mod ui;
mod validation;
mod vertical;

pub use angle::AngleSlider;
pub use flags::SliderFlags;
pub use horizontal::Slider;
pub use vertical::VerticalSlider;

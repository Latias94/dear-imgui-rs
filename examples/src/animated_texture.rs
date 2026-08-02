//! Shared wall-clock animation used by renderer texture examples.

use std::time::Duration;

const RADIANS_PER_SECOND: f64 = 4.8;
const CYCLE_RADIANS: f64 = std::f64::consts::TAU * 10.0;

fn animation_phase(elapsed: Duration) -> f32 {
    (elapsed.as_secs_f64() * RADIANS_PER_SECOND).rem_euclid(CYCLE_RADIANS) as f32
}

/// Generate the renderer-demo RGBA texture for a wall-clock timestamp.
///
/// The same elapsed time always produces the same pixels, regardless of renderer FPS or present
/// mode. This keeps the Glow, WGPU, and Ash examples visually comparable on different systems.
pub fn animated_rgba_pixels(width: u32, height: u32, elapsed: Duration) -> Vec<u8> {
    let capacity = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .and_then(|bytes| usize::try_from(bytes).ok())
        .expect("animated texture dimensions exceed addressable memory");
    let mut pixels = Vec::with_capacity(capacity);
    let phase = animation_phase(elapsed);

    for y in 0..height {
        for x in 0..width {
            let fx = x as f32 / width as f32;
            let fy = y as f32 / height as f32;
            let red = (fx * 255.0 + phase.sin() * 128.0).clamp(0.0, 255.0) as u8;
            let green = (fy * 255.0 + (phase * 1.7).cos() * 128.0).clamp(0.0, 255.0) as u8;
            let blue = ((fx + fy + phase * 0.1).sin().abs() * 255.0) as u8;
            pixels.extend_from_slice(&[red, green, blue, 255]);
        }
    }

    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animation_phase_uses_elapsed_time_and_wraps_without_a_jump() {
        let offset = Duration::from_millis(500);
        let cycle = Duration::from_secs_f64(CYCLE_RADIANS / RADIANS_PER_SECOND);
        let first = animation_phase(offset);
        let wrapped = animation_phase(cycle + offset);

        assert!((first - 2.4).abs() < 0.000_01);
        assert!((first - wrapped).abs() < 0.000_01);
    }

    #[test]
    fn generated_pixels_are_complete_rgba() {
        let pixels = animated_rgba_pixels(3, 2, Duration::from_millis(250));

        assert_eq!(pixels.len(), 3 * 2 * 4);
        assert!(pixels.chunks_exact(4).all(|pixel| pixel[3] == 255));
    }
}

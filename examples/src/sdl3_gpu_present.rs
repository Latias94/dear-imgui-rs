//! SDLGPU present-mode policy shared by the callback example and its tests.

use sdl3::gpu::PresentMode;

/// Prefer mailbox for the primary window when the active device supports it.
pub fn primary_present_mode(mailbox_supported: bool) -> PresentMode {
    if mailbox_supported {
        PresentMode::Mailbox
    } else {
        PresentMode::Vsync
    }
}

/// Keep secondary viewport swapchains on VSync.
pub const fn secondary_present_mode() -> PresentMode {
    // On D3D12, SDL claims new viewport windows with VSync before applying the requested mode.
    // Switching a newly claimed viewport to Mailbox waits for the shared queue to drain, which
    // turns a normal viewport drag into a synchronous UI stall. The primary window may use Mailbox.
    PresentMode::Vsync
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mailbox_is_preferred_for_the_primary_window_when_supported() {
        assert_eq!(primary_present_mode(true), PresentMode::Mailbox);
        assert_eq!(primary_present_mode(false), PresentMode::Vsync);
    }

    #[test]
    fn secondary_viewports_stay_on_vsync() {
        assert_eq!(secondary_present_mode(), PresentMode::Vsync);
    }
}

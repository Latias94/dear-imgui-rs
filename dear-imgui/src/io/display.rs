use crate::io::{Io, assert_display_framebuffer_scale};

impl Io {
    /// Get the display framebuffer scale
    pub fn display_framebuffer_scale(&self) -> [f32; 2] {
        let scale = self.inner().DisplayFramebufferScale;
        [scale.x, scale.y]
    }

    /// Set the strictly positive, finite display framebuffer scale.
    ///
    /// This controls HiDPI pixel density. Use a zero display size to suspend
    /// rendering for a minimized window; its pixel density remains positive.
    pub fn set_display_framebuffer_scale(&mut self, scale: [f32; 2]) {
        assert_display_framebuffer_scale("Io::set_display_framebuffer_scale()", scale);
        self.inner_mut().DisplayFramebufferScale.x = scale[0];
        self.inner_mut().DisplayFramebufferScale.y = scale[1];
    }
}

use crate::io::{Io, assert_display_framebuffer_scale};

impl Io {
    /// Get the display framebuffer scale
    pub fn display_framebuffer_scale(&self) -> [f32; 2] {
        let scale = self.inner().DisplayFramebufferScale;
        [scale.x, scale.y]
    }

    /// Set the display framebuffer scale
    /// This is important for HiDPI displays to ensure proper rendering
    pub fn set_display_framebuffer_scale(&mut self, scale: [f32; 2]) {
        assert_display_framebuffer_scale("Io::set_display_framebuffer_scale()", scale);
        self.inner_mut().DisplayFramebufferScale.x = scale[0];
        self.inner_mut().DisplayFramebufferScale.y = scale[1];
    }
}

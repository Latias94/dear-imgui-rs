use super::*;

impl Io {
    /// Returns whether fonts are scaled by DPI.
    #[doc(alias = "ConfigDpiScaleFonts")]
    pub fn config_dpi_scale_fonts(&self) -> bool {
        self.inner().ConfigDpiScaleFonts
    }

    /// Set whether fonts are scaled by DPI.
    #[doc(alias = "ConfigDpiScaleFonts")]
    pub fn set_config_dpi_scale_fonts(&mut self, enabled: bool) {
        self.inner_mut().ConfigDpiScaleFonts = enabled;
    }

    /// Returns whether viewports are scaled by DPI.
    #[doc(alias = "ConfigDpiScaleViewports")]
    pub fn config_dpi_scale_viewports(&self) -> bool {
        self.inner().ConfigDpiScaleViewports
    }

    /// Set whether viewports are scaled by DPI.
    #[doc(alias = "ConfigDpiScaleViewports")]
    pub fn set_config_dpi_scale_viewports(&mut self, enabled: bool) {
        self.inner_mut().ConfigDpiScaleViewports = enabled;
    }
}

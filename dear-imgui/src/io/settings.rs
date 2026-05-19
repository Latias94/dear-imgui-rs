use crate::io::{Io, assert_display_size, assert_non_negative_f32};
use std::ffi::{CStr, c_void};

impl Io {
    /// Main display size in pixels
    pub fn display_size(&self) -> [f32; 2] {
        [self.inner().DisplaySize.x, self.inner().DisplaySize.y]
    }

    /// Set main display size in pixels
    pub fn set_display_size(&mut self, size: [f32; 2]) {
        assert_display_size("Io::set_display_size()", size);
        self.inner_mut().DisplaySize.x = size[0];
        self.inner_mut().DisplaySize.y = size[1];
    }

    /// Time elapsed since last frame, in seconds
    pub fn delta_time(&self) -> f32 {
        self.inner().DeltaTime
    }

    /// Set time elapsed since last frame, in seconds
    pub fn set_delta_time(&mut self, delta_time: f32) {
        self.assert_delta_time("Io::set_delta_time()", delta_time);
        self.inner_mut().DeltaTime = delta_time;
    }

    /// Auto-save interval for `.ini` settings, in seconds.
    #[doc(alias = "IniSavingRate")]
    pub fn ini_saving_rate(&self) -> f32 {
        self.inner().IniSavingRate
    }

    /// Set auto-save interval for `.ini` settings, in seconds.
    #[doc(alias = "IniSavingRate")]
    pub fn set_ini_saving_rate(&mut self, seconds: f32) {
        assert_non_negative_f32("Io::set_ini_saving_rate()", "seconds", seconds);
        self.inner_mut().IniSavingRate = seconds;
    }

    /// Returns the current `.ini` filename, or `None` if disabled.
    ///
    /// Note: to set this safely, use `Context::set_ini_filename()`.
    #[doc(alias = "IniFilename")]
    pub fn ini_filename(&self) -> Option<&CStr> {
        let ptr = self.inner().IniFilename;
        unsafe { (!ptr.is_null()).then(|| CStr::from_ptr(ptr)) }
    }

    /// Returns the current `.log` filename, or `None` if disabled.
    ///
    /// Note: to set this safely, use `Context::set_log_filename()`.
    #[doc(alias = "LogFilename")]
    pub fn log_filename(&self) -> Option<&CStr> {
        let ptr = self.inner().LogFilename;
        unsafe { (!ptr.is_null()).then(|| CStr::from_ptr(ptr)) }
    }

    /// Returns user data pointer stored in `ImGuiIO`.
    #[doc(alias = "UserData")]
    pub fn user_data(&self) -> *mut c_void {
        self.inner().UserData
    }

    /// Set user data pointer stored in `ImGuiIO`.
    #[doc(alias = "UserData")]
    pub fn set_user_data(&mut self, user_data: *mut c_void) {
        self.inner_mut().UserData = user_data;
    }

    /// Returns whether font scaling via Ctrl+MouseWheel is enabled.
    #[doc(alias = "FontAllowUserScaling")]
    pub fn font_allow_user_scaling(&self) -> bool {
        self.inner().FontAllowUserScaling
    }

    /// Set whether font scaling via Ctrl+MouseWheel is enabled.
    #[doc(alias = "FontAllowUserScaling")]
    pub fn set_font_allow_user_scaling(&mut self, enabled: bool) {
        self.inner_mut().FontAllowUserScaling = enabled;
    }
}

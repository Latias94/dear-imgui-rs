use crate::io::{Io, assert_display_size, assert_non_negative_f32};
use std::ffi::{CStr, c_void};
use std::num::NonZeroU32;

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

    /// Returns whether supported `.ini` entries store their last-used date.
    #[doc(alias = "ConfigIniSettingsSaveLastUsedDate")]
    pub fn ini_settings_save_last_used_date(&self) -> bool {
        self.inner().ConfigIniSettingsSaveLastUsedDate
    }

    /// Sets whether supported `.ini` entries store their last-used date.
    ///
    /// Automatic discard must be disabled before date storage can be disabled, because entries
    /// without a saved date are discarded when that mode is active.
    #[doc(alias = "ConfigIniSettingsSaveLastUsedDate")]
    pub fn set_ini_settings_save_last_used_date(&mut self, enabled: bool) {
        assert!(
            enabled || self.inner().ConfigIniSettingsAutoDiscardMonths == 0,
            "Io::set_ini_settings_save_last_used_date() cannot disable dates while automatic discard is enabled"
        );
        self.inner_mut().ConfigIniSettingsSaveLastUsedDate = enabled;
    }

    /// Returns the age after which unused `.ini` entries are discarded on load.
    ///
    /// `None` disables automatic discard.
    #[doc(alias = "ConfigIniSettingsAutoDiscardMonths")]
    pub fn ini_settings_auto_discard_months(&self) -> Option<NonZeroU32> {
        let months = self.inner().ConfigIniSettingsAutoDiscardMonths;
        let months = u32::try_from(months)
            .expect("Io::ini_settings_auto_discard_months() found a negative raw month count");
        NonZeroU32::new(months)
    }

    /// Sets the age after which unused `.ini` entries are discarded on load.
    ///
    /// `None` disables automatic discard. Enabling this also enables saving last-used dates.
    #[doc(alias = "ConfigIniSettingsAutoDiscardMonths")]
    pub fn set_ini_settings_auto_discard_months(&mut self, months: Option<NonZeroU32>) {
        let months = months.map_or(0, NonZeroU32::get);
        let months = i32::try_from(months)
            .expect("Io::set_ini_settings_auto_discard_months() supports at most i32::MAX months");
        if months != 0 {
            self.inner_mut().ConfigIniSettingsSaveLastUsedDate = true;
        }
        self.inner_mut().ConfigIniSettingsAutoDiscardMonths = months;
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

use std::path::PathBuf;
use std::ptr;

use crate::ini_settings::{IniSettingsRetention, IniSettingsRetentionError};
use crate::sys;

use super::Context;
use super::binding::{CTX_MUTEX, with_bound_context};

impl Context {
    /// Returns the Context-owned `.ini` retention configuration.
    ///
    /// Native state modified through unsafe FFI is validated before it is returned.
    #[doc(alias = "Platform_SessionDate")]
    #[doc(alias = "ConfigIniSettingsSaveLastUsedDate")]
    #[doc(alias = "ConfigIniSettingsAutoDiscardMonths")]
    pub fn ini_settings_retention(
        &self,
    ) -> Result<IniSettingsRetention, IniSettingsRetentionError> {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            let platform_io = self.platform_io_ptr("Context::ini_settings_retention()");
            let io = self.io_ptr("Context::ini_settings_retention()");
            IniSettingsRetention::from_raw(
                (*platform_io).Platform_SessionDate,
                (*io).ConfigIniSettingsSaveLastUsedDate,
                (*io).ConfigIniSettingsAutoDiscardMonths,
            )
        }
    }

    /// Atomically configures the session date and `.ini` retention behavior.
    ///
    /// This must be called before loading settings and before the first Dear ImGui frame. Automatic
    /// cleanup runs while settings are loaded, and Dear ImGui copies the platform session date at
    /// frame start. Mutating the policy after either boundary would silently produce mixed state.
    ///
    /// Enabling [`IniSettingsRetention::AutoDiscard`] removes supported settings that have no
    /// `LastUsed` field on the next load, including settings written before date recording was
    /// enabled.
    ///
    /// ```compile_fail
    /// # use dear_imgui_rs::{Context, IniSettingsRetention};
    /// let mut context = Context::create();
    /// context.io_mut().set_ini_settings_auto_discard_months(None);
    /// let _ = IniSettingsRetention::disabled();
    /// ```
    ///
    /// ```compile_fail
    /// # use dear_imgui_rs::Context;
    /// let mut context = Context::create();
    /// context.platform_io_mut().set_session_date(None);
    /// ```
    #[doc(alias = "Platform_SessionDate")]
    #[doc(alias = "ConfigIniSettingsSaveLastUsedDate")]
    #[doc(alias = "ConfigIniSettingsAutoDiscardMonths")]
    pub fn set_ini_settings_retention(
        &mut self,
        retention: IniSettingsRetention,
    ) -> Result<(), IniSettingsRetentionError> {
        retention.validate()?;
        let (session_date, save_last_used_date, auto_discard_months) = retention.raw_parts();

        let _guard = CTX_MUTEX.lock();
        if unsafe { (*self.raw).FrameCount } != 0 {
            return Err(IniSettingsRetentionError::LockedAfterFirstFrame);
        }
        if unsafe { (*self.raw).SettingsLoaded } {
            return Err(IniSettingsRetentionError::LockedAfterSettingsLoad);
        }

        unsafe {
            let platform_io = self.platform_io_ptr("Context::set_ini_settings_retention()");
            let io = self.io_ptr("Context::set_ini_settings_retention()");

            // Clear the dependent field first. Every validation and pointer lookup above occurs
            // before mutation, then this sequence reaches the requested state without exposing
            // automatic discard alongside an incompatible date or save flag.
            (*io).ConfigIniSettingsAutoDiscardMonths = 0;
            (*platform_io).Platform_SessionDate = session_date;
            (*io).ConfigIniSettingsSaveLastUsedDate = save_last_used_date;
            (*io).ConfigIniSettingsAutoDiscardMonths = auto_discard_months;
        }
        Ok(())
    }

    /// Sets the INI filename for settings persistence
    ///
    /// # Errors
    ///
    /// Returns an error if the filename contains null bytes
    pub fn set_ini_filename<P: Into<PathBuf>>(
        &mut self,
        filename: Option<P>,
    ) -> crate::error::ImGuiResult<()> {
        use crate::error::SafeStringConversion;
        let _guard = CTX_MUTEX.lock();

        self.ini_filename = match filename {
            Some(f) => Some(f.into().to_string_lossy().to_cstring_safe()?),
            None => None,
        };

        unsafe {
            let io = self.io_ptr("Context::set_ini_filename()");
            let ptr = self
                .ini_filename
                .as_ref()
                .map(|s| s.as_ptr())
                .unwrap_or(ptr::null());
            (*io).IniFilename = ptr;
        }
        Ok(())
    }

    // removed legacy set_ini_filename_or_panic (use set_ini_filename())

    /// Sets the log filename
    ///
    /// # Errors
    ///
    /// Returns an error if the filename contains null bytes
    pub fn set_log_filename<P: Into<PathBuf>>(
        &mut self,
        filename: Option<P>,
    ) -> crate::error::ImGuiResult<()> {
        use crate::error::SafeStringConversion;
        let _guard = CTX_MUTEX.lock();

        self.log_filename = match filename {
            Some(f) => Some(f.into().to_string_lossy().to_cstring_safe()?),
            None => None,
        };

        unsafe {
            let io = self.io_ptr("Context::set_log_filename()");
            let ptr = self
                .log_filename
                .as_ref()
                .map(|s| s.as_ptr())
                .unwrap_or(ptr::null());
            (*io).LogFilename = ptr;
        }
        Ok(())
    }

    // removed legacy set_log_filename_or_panic (use set_log_filename())

    /// Sets the platform name
    ///
    /// # Errors
    ///
    /// Returns an error if the name contains null bytes
    pub fn set_platform_name<S: Into<String>>(
        &mut self,
        name: Option<S>,
    ) -> crate::error::ImGuiResult<()> {
        use crate::error::SafeStringConversion;
        let _guard = CTX_MUTEX.lock();

        self.platform_name = match name {
            Some(n) => Some(n.into().to_cstring_safe()?),
            None => None,
        };

        unsafe {
            let io = self.io_ptr("Context::set_platform_name()");
            let ptr = self
                .platform_name
                .as_ref()
                .map(|s| s.as_ptr())
                .unwrap_or(ptr::null());
            (*io).BackendPlatformName = ptr;
        }
        Ok(())
    }

    // removed legacy set_platform_name_or_panic (use set_platform_name())

    /// Sets the renderer name
    ///
    /// # Errors
    ///
    /// Returns an error if the name contains null bytes
    pub fn set_renderer_name<S: Into<String>>(
        &mut self,
        name: Option<S>,
    ) -> crate::error::ImGuiResult<()> {
        use crate::error::SafeStringConversion;
        let _guard = CTX_MUTEX.lock();

        self.renderer_name = match name {
            Some(n) => Some(n.into().to_cstring_safe()?),
            None => None,
        };

        unsafe {
            let io = self.io_ptr("Context::set_renderer_name()");
            let ptr = self
                .renderer_name
                .as_ref()
                .map(|s| s.as_ptr())
                .unwrap_or(ptr::null());
            (*io).BackendRendererName = ptr;
        }
        Ok(())
    }

    // removed legacy set_renderer_name_or_panic (use set_renderer_name())

    /// Loads settings from a string slice containing settings in .Ini file format
    #[doc(alias = "LoadIniSettingsFromMemory")]
    pub fn load_ini_settings(&mut self, data: &str) {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            with_bound_context(self.raw, || {
                sys::igLoadIniSettingsFromMemory(data.as_ptr() as *const _, data.len());
            });
        }
    }

    /// Saves settings to a mutable string buffer in .Ini file format
    #[doc(alias = "SaveIniSettingsToMemory")]
    pub fn save_ini_settings(&mut self, buf: &mut String) {
        let _guard = CTX_MUTEX.lock();
        unsafe {
            with_bound_context(self.raw, || {
                let mut out_ini_size: usize = 0;
                let data_ptr = sys::igSaveIniSettingsToMemory(&mut out_ini_size as *mut usize);
                if data_ptr.is_null() || out_ini_size == 0 {
                    return;
                }

                let mut bytes = std::slice::from_raw_parts(data_ptr as *const u8, out_ini_size);
                if bytes.last() == Some(&0) {
                    bytes = &bytes[..bytes.len().saturating_sub(1)];
                }
                buf.push_str(&String::from_utf8_lossy(bytes));
            });
        }
    }

    /// Loads settings from a `.ini` file on disk.
    ///
    /// This is a convenience wrapper over `ImGui::LoadIniSettingsFromDisk`.
    ///
    /// Note: this is not available on `wasm32` targets.
    #[cfg(not(target_arch = "wasm32"))]
    #[doc(alias = "LoadIniSettingsFromDisk")]
    pub fn load_ini_settings_from_disk<P: Into<PathBuf>>(
        &mut self,
        filename: P,
    ) -> crate::error::ImGuiResult<()> {
        use crate::error::SafeStringConversion;
        let _guard = CTX_MUTEX.lock();
        let cstr = filename.into().to_string_lossy().to_cstring_safe()?;
        unsafe {
            with_bound_context(self.raw, || {
                sys::igLoadIniSettingsFromDisk(cstr.as_ptr());
            });
        }
        Ok(())
    }

    /// Saves settings to a `.ini` file on disk.
    ///
    /// This is a convenience wrapper over `ImGui::SaveIniSettingsToDisk`.
    ///
    /// Note: this is not available on `wasm32` targets.
    #[cfg(not(target_arch = "wasm32"))]
    #[doc(alias = "SaveIniSettingsToDisk")]
    pub fn save_ini_settings_to_disk<P: Into<PathBuf>>(
        &mut self,
        filename: P,
    ) -> crate::error::ImGuiResult<()> {
        use crate::error::SafeStringConversion;
        let _guard = CTX_MUTEX.lock();
        let cstr = filename.into().to_string_lossy().to_cstring_safe()?;
        unsafe {
            with_bound_context(self.raw, || {
                sys::igSaveIniSettingsToDisk(cstr.as_ptr());
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod retention_tests {
    use std::num::NonZeroU16;

    use crate::IniSessionDate;

    use super::*;

    fn raw_retention(context: &Context) -> (i32, bool, i32) {
        unsafe {
            let platform_io = context.platform_io_ptr("raw_retention()");
            let io = context.io_ptr("raw_retention()");
            (
                (*platform_io).Platform_SessionDate,
                (*io).ConfigIniSettingsSaveLastUsedDate,
                (*io).ConfigIniSettingsAutoDiscardMonths,
            )
        }
    }

    #[test]
    fn session_date_enforces_packed_and_gregorian_boundaries() {
        let earliest = IniSessionDate::new(2001, 1, 1).expect("earliest packed date");
        let latest = IniSessionDate::new(2127, 12, 31).expect("latest packed date");
        assert_eq!(earliest.as_yyyymmdd(), 20_010_101);
        assert_eq!(latest.as_yyyymmdd(), 21_271_231);
        assert_eq!(earliest.max_auto_discard_months(), 0);
        assert_eq!(latest.max_auto_discard_months(), 1_523);
        assert_eq!(
            IniSessionDate::try_from(20_240_229),
            Ok(IniSessionDate::new(2024, 2, 29).unwrap())
        );

        for invalid in [
            IniSessionDate::new(2000, 12, 31),
            IniSessionDate::new(2128, 1, 1),
            IniSessionDate::new(2023, 2, 29),
            IniSessionDate::new(2100, 2, 29),
            IniSessionDate::new(2026, 13, 1),
            IniSessionDate::new(2026, 12, 32),
        ] {
            assert!(invalid.is_err());
        }
        assert!(IniSessionDate::try_from(0).is_err());
    }

    #[test]
    fn context_retention_update_is_date_bounded_and_atomic() {
        let _guard = crate::test_support::imgui_context_guard();
        let mut context = Context::create();
        let date = IniSessionDate::new(2026, 7, 30).unwrap();
        let six_months = NonZeroU16::new(6).unwrap();
        let six_month_retention = IniSettingsRetention::AutoDiscard {
            session_date: date,
            months: six_months,
        };

        context
            .set_ini_settings_retention(six_month_retention)
            .expect("valid retention policy");
        assert_eq!(
            context
                .ini_settings_retention()
                .expect("valid native state"),
            six_month_retention
        );
        assert_eq!(raw_retention(&context), (20_260_730, true, 6));

        let before = raw_retention(&context);
        let earliest = IniSessionDate::new(2001, 1, 1).unwrap();
        let too_old = context
            .set_ini_settings_retention(IniSettingsRetention::AutoDiscard {
                session_date: earliest,
                months: six_months,
            })
            .expect_err("discard must not underflow the packed year");
        assert!(matches!(
            too_old,
            IniSettingsRetentionError::RetentionUnderflowsSessionDate {
                months,
                max_months: 0,
                session_date,
            } if months == six_months && session_date == earliest
        ));
        assert_eq!(raw_retention(&context), before);

        let latest = IniSessionDate::new(2127, 12, 31).unwrap();
        let maximum = NonZeroU16::new(1_523).unwrap();
        context
            .set_ini_settings_retention(IniSettingsRetention::AutoDiscard {
                session_date: latest,
                months: maximum,
            })
            .expect("latest date accepts the global maximum");
        let earlier = IniSessionDate::new(2002, 1, 31).unwrap();
        let twelve_months = NonZeroU16::new(12).unwrap();
        context
            .set_ini_settings_retention(IniSettingsRetention::AutoDiscard {
                session_date: earlier,
                months: twelve_months,
            })
            .expect("one Context transaction may reduce the date and month limit together");
        assert_eq!(raw_retention(&context), (20_020_131, true, 12));

        let record_only = IniSettingsRetention::RecordLastUsed {
            session_date: Some(earlier),
        };
        context
            .set_ini_settings_retention(record_only)
            .expect("last-used recording is independent of automatic cleanup");
        assert_eq!(raw_retention(&context), (20_020_131, true, 0));
        assert_eq!(
            context.ini_settings_retention().expect("record-only state"),
            record_only
        );

        let disabled_with_date = IniSettingsRetention::Disabled {
            session_date: Some(earlier),
        };
        context
            .set_ini_settings_retention(disabled_with_date)
            .expect("a platform date is independent of retention");
        assert_eq!(raw_retention(&context), (20_020_131, false, 0));
        assert_eq!(
            context
                .ini_settings_retention()
                .expect("disabled state with a platform date"),
            disabled_with_date
        );

        context
            .set_ini_settings_retention(IniSettingsRetention::Disabled { session_date: None })
            .expect("disabling date retention never needs a date");
        assert_eq!(raw_retention(&context), (0, false, 0));
    }

    #[test]
    fn context_retention_accepts_the_dynamic_month_limit() {
        let _guard = crate::test_support::imgui_context_guard();
        let mut context = Context::create();
        let date = IniSessionDate::new(2001, 2, 28).unwrap();
        let one_month = NonZeroU16::new(1).unwrap();
        let retention = IniSettingsRetention::AutoDiscard {
            session_date: date,
            months: one_month,
        };

        context
            .set_ini_settings_retention(retention)
            .expect("one month reaches January 2001 without underflow");
        assert_eq!(raw_retention(&context), (20_010_228, true, 1));

        let two_months = NonZeroU16::new(2).unwrap();
        assert!(matches!(
            context.set_ini_settings_retention(IniSettingsRetention::AutoDiscard {
                session_date: date,
                months: two_months,
            }),
            Err(IniSettingsRetentionError::RetentionUnderflowsSessionDate { max_months: 1, .. })
        ));
        assert_eq!(raw_retention(&context), (20_010_228, true, 1));
    }

    #[test]
    fn context_retention_locks_after_the_first_frame() {
        let _guard = crate::test_support::imgui_context_guard();
        let mut context = Context::create();
        assert!(context.font_atlas().build());
        context.io_mut().set_display_size([128.0, 128.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        let _ui = context.frame();
        drop(context.render());

        let before = raw_retention(&context);
        assert!(matches!(
            context
                .set_ini_settings_retention(IniSettingsRetention::Disabled { session_date: None }),
            Err(IniSettingsRetentionError::LockedAfterFirstFrame)
        ));
        assert_eq!(raw_retention(&context), before);
    }

    #[test]
    fn context_retention_locks_after_settings_load() {
        let _guard = crate::test_support::imgui_context_guard();
        let mut context = Context::create();
        context.load_ini_settings("[Window][Example]\nPos=0,0\nSize=100,100\n");

        let before = raw_retention(&context);
        assert!(matches!(
            context
                .set_ini_settings_retention(IniSettingsRetention::Disabled { session_date: None }),
            Err(IniSettingsRetentionError::LockedAfterSettingsLoad)
        ));
        assert_eq!(raw_retention(&context), before);
    }

    #[test]
    fn context_retention_reports_invalid_native_state() {
        let _guard = crate::test_support::imgui_context_guard();
        let context = Context::create();
        let (platform_io, io) = (
            context.platform_io_ptr("invalid native retention test"),
            context.io_ptr("invalid native retention test"),
        );

        unsafe {
            (*platform_io).Platform_SessionDate = 21_280_101;
            (*io).ConfigIniSettingsSaveLastUsedDate = true;
            (*io).ConfigIniSettingsAutoDiscardMonths = 0;
        }
        assert!(matches!(
            context.ini_settings_retention(),
            Err(IniSettingsRetentionError::InvalidNativeSessionDate { raw: 21_280_101 })
        ));

        unsafe {
            (*platform_io).Platform_SessionDate = 20_260_730;
            (*io).ConfigIniSettingsAutoDiscardMonths = -1;
        }
        assert!(matches!(
            context.ini_settings_retention(),
            Err(IniSettingsRetentionError::InvalidNativeAutoDiscardMonths { raw: -1 })
        ));

        unsafe {
            (*io).ConfigIniSettingsSaveLastUsedDate = false;
            (*io).ConfigIniSettingsAutoDiscardMonths = 1;
        }
        assert!(matches!(
            context.ini_settings_retention(),
            Err(IniSettingsRetentionError::InvalidNativeState { .. })
        ));

        unsafe {
            (*platform_io).Platform_SessionDate = 0;
            (*io).ConfigIniSettingsSaveLastUsedDate = true;
        }
        assert!(matches!(
            context.ini_settings_retention(),
            Err(IniSettingsRetentionError::InvalidNativeState { .. })
        ));
    }

    #[test]
    fn context_retention_round_trips_recording_without_a_session_date() {
        let _guard = crate::test_support::imgui_context_guard();
        let mut context = Context::create();
        let raw = IniSettingsRetention::RecordLastUsed { session_date: None };

        context
            .set_ini_settings_retention(raw)
            .expect("date recording may be configured without a platform clock");
        assert_eq!(raw_retention(&context), (0, true, 0));

        let observed = context
            .ini_settings_retention()
            .expect("the native state is representable");
        assert_eq!(observed, raw);
        context
            .set_ini_settings_retention(observed)
            .expect("reading and writing the policy is lossless");
        assert_eq!(raw_retention(&context), (0, true, 0));
    }

    #[test]
    fn auto_discard_removes_undated_entries_on_load() {
        let _guard = crate::test_support::imgui_context_guard();
        let mut context = Context::create();
        context
            .set_ini_settings_retention(IniSettingsRetention::AutoDiscard {
                session_date: IniSessionDate::new(2026, 7, 30).unwrap(),
                months: NonZeroU16::new(6).unwrap(),
            })
            .unwrap();

        context.load_ini_settings(
            "[Window][Undated]\nPos=1,1\nSize=100,100\nCollapsed=0\n\n\
             [Window][Recent]\nPos=2,2\nSize=100,100\nCollapsed=0\nLastUsed=20260730\n\n",
        );

        let mut saved = String::new();
        context.save_ini_settings(&mut saved);
        assert!(!saved.contains("[Window][Undated]"));
        assert!(saved.contains("[Window][Recent]"));
        assert!(saved.contains("LastUsed=20260730"));
    }
}

use std::path::PathBuf;
use std::ptr;

use crate::sys;

use super::Context;
use super::binding::{CTX_MUTEX, with_bound_context};

impl Context {
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

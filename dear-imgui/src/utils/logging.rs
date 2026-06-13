use crate::sys;

/// Auto-open depth for Dear ImGui logging helpers.
///
/// Dear ImGui uses `-1` to mean "use the configured default depth". This type keeps that sentinel
/// out of the safe Rust API while still allowing an explicit non-negative tree depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogAutoOpenDepth(Option<u32>);

impl LogAutoOpenDepth {
    /// Use Dear ImGui's configured default log auto-open depth.
    pub const DEFAULT: Self = Self(None);

    /// Create an explicit non-negative auto-open depth.
    ///
    /// Panics if `depth` exceeds Dear ImGui's signed `int` range.
    #[inline]
    pub const fn new(depth: u32) -> Self {
        assert!(
            depth <= i32::MAX as u32,
            "LogAutoOpenDepth::new() depth exceeded i32::MAX"
        );
        Self(Some(depth))
    }

    #[inline]
    pub(crate) const fn raw(self) -> i32 {
        match self.0 {
            Some(depth) => depth as i32,
            None => -1,
        }
    }
}

impl From<u32> for LogAutoOpenDepth {
    fn from(depth: u32) -> Self {
        Self::new(depth)
    }
}

impl crate::ui::Ui {
    /// Start logging to TTY.
    #[doc(alias = "LogToTTY")]
    pub fn log_to_tty(&self, auto_open_depth: impl Into<LogAutoOpenDepth>) {
        self.run_with_bound_context(|| unsafe { sys::igLogToTTY(auto_open_depth.into().raw()) });
    }

    /// Start logging to file with the default filename.
    #[doc(alias = "LogToFile")]
    pub fn log_to_file_default(&self, auto_open_depth: impl Into<LogAutoOpenDepth>) {
        self.run_with_bound_context(|| unsafe {
            sys::igLogToFile(auto_open_depth.into().raw(), std::ptr::null())
        });
    }

    /// Start logging to file.
    ///
    /// # Errors
    ///
    /// Returns an error if `filename` contains NUL bytes.
    #[doc(alias = "LogToFile")]
    pub fn log_to_file(
        &self,
        auto_open_depth: impl Into<LogAutoOpenDepth>,
        filename: &std::path::Path,
    ) -> crate::error::ImGuiResult<()> {
        use crate::error::SafeStringConversion;
        let cstr = filename.to_string_lossy().into_owned().to_cstring_safe()?;
        self.run_with_bound_context(|| unsafe {
            sys::igLogToFile(auto_open_depth.into().raw(), cstr.as_ptr())
        });
        Ok(())
    }

    /// Start logging to clipboard.
    #[doc(alias = "LogToClipboard")]
    pub fn log_to_clipboard(&self, auto_open_depth: impl Into<LogAutoOpenDepth>) {
        self.run_with_bound_context(|| unsafe {
            sys::igLogToClipboard(auto_open_depth.into().raw())
        });
    }

    /// Show ImGui's logging buttons (TTY/File/Clipboard).
    #[doc(alias = "LogButtons")]
    pub fn log_buttons(&self) {
        self.run_with_bound_context(|| unsafe { sys::igLogButtons() });
    }

    /// Finish logging (close file / copy to clipboard as needed).
    #[doc(alias = "LogFinish")]
    pub fn log_finish(&self) {
        self.run_with_bound_context(|| unsafe { sys::igLogFinish() });
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn log_auto_open_depth_preserves_default_sentinel_and_rejects_overflow() {
        assert_eq!(super::LogAutoOpenDepth::DEFAULT.raw(), -1);
        assert_eq!(super::LogAutoOpenDepth::new(0).raw(), 0);
        assert_eq!(super::LogAutoOpenDepth::new(3).raw(), 3);
        assert!(
            std::panic::catch_unwind(|| super::LogAutoOpenDepth::new(i32::MAX as u32 + 1)).is_err()
        );
    }
}

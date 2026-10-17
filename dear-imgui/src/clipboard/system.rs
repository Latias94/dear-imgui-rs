use super::ClipboardBackend;

/// Failure to initialize or access the optional desktop text clipboard.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SystemClipboardError {
    #[error("system clipboard integration is unavailable on this target")]
    UnsupportedPlatform,
    #[error("failed to initialize the system clipboard: {0}")]
    Initialization(String),
    #[error("failed to read the system clipboard: {0}")]
    Read(String),
    #[error("failed to write the system clipboard: {0}")]
    Write(String),
}

/// Optional desktop text clipboard adapter for both Winit and SDL applications.
///
/// Enable `system-clipboard` to use this adapter on Windows, macOS or Linux. Linux uses X11 by
/// default; `system-clipboard-wayland` also enables arboard's optional data-control protocol.
/// Initialization returns an error when desktop services are unavailable; no internal clipboard
/// fallback pretends to be system integration. Install after a platform backend has initialized.
///
/// ```no_run
/// # use dear_imgui_rs::{Context, SystemClipboard};
/// # fn configure(context: &mut Context) -> Result<(), Box<dyn std::error::Error>> {
/// context.set_clipboard_backend(SystemClipboard::new()?);
/// # Ok(())
/// # }
/// ```
pub struct SystemClipboard {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    clipboard: arboard::Clipboard,
}

impl SystemClipboard {
    pub fn new() -> Result<Self, SystemClipboardError> {
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        {
            let clipboard = arboard::Clipboard::new()
                .map_err(|error| SystemClipboardError::Initialization(error.to_string()))?;
            Ok(Self { clipboard })
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        Err(SystemClipboardError::UnsupportedPlatform)
    }

    pub fn get_text(&mut self) -> Result<String, SystemClipboardError> {
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        {
            self.clipboard
                .get_text()
                .map_err(|error| SystemClipboardError::Read(error.to_string()))
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        Err(SystemClipboardError::UnsupportedPlatform)
    }

    pub fn set_text(&mut self, text: &str) -> Result<(), SystemClipboardError> {
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        {
            self.clipboard
                .set_text(text)
                .map_err(|error| SystemClipboardError::Write(error.to_string()))
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            let _ = text;
            Err(SystemClipboardError::UnsupportedPlatform)
        }
    }
}

impl ClipboardBackend for SystemClipboard {
    fn get(&mut self) -> Option<String> {
        self.get_text().ok()
    }
    fn set(&mut self, text: &str) {
        let _ = self.set_text(text);
    }
}

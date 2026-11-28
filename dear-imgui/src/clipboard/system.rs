use crate::error::context::ContextError;

use super::ClipboardBackend;

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
    pub fn new() -> Result<Self, ContextError> {
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        {
            let clipboard = arboard::Clipboard::new()
                .map_err(|error| ContextError::ClipboardInitialization(Box::new(error)))?;
            Ok(Self { clipboard })
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        Err(ContextError::ClipboardUnsupportedPlatform)
    }

    pub fn get_text(&mut self) -> Result<String, ContextError> {
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        {
            self.clipboard
                .get_text()
                .map_err(|error| ContextError::ClipboardRead(Box::new(error)))
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        Err(ContextError::ClipboardUnsupportedPlatform)
    }

    pub fn set_text(&mut self, text: &str) -> Result<(), ContextError> {
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        {
            self.clipboard
                .set_text(text)
                .map_err(|error| ContextError::ClipboardWrite(Box::new(error)))
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            let _ = text;
            Err(ContextError::ClipboardUnsupportedPlatform)
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

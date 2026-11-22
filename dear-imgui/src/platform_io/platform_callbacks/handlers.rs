use crate::sys;

use super::super::PlatformIo;
use super::super::core::clear_platform_aggregate_callbacks_for_platform_io;

impl PlatformIo {
    /// Clear all platform backend handlers.
    ///
    /// Dear ImGui resets the platform callbacks and reinstalls its default clipboard, IME and
    /// shell handlers. This also releases the aggregate callback storage for this `PlatformIo`.
    ///
    /// # Safety
    ///
    /// Dear ImGui must no longer be able to invoke any platform callback, and all platform-owned
    /// viewport state must already have been released.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn clear_platform_handlers(&mut self) {
        unsafe {
            sys::ImGuiPlatformIO_ClearPlatformHandlers(self.as_raw_mut());
            clear_platform_aggregate_callbacks_for_platform_io(self.as_raw_mut());
        }
    }
}

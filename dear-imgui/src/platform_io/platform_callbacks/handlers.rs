use crate::sys;

use super::super::core::clear_platform_aggregate_callbacks_for_platform_io;
use super::super::{PlatformIo, trampolines};

impl PlatformIo {
    /// Clear all platform backend handlers.
    ///
    /// This resets the `Platform_*` callback table stored in `ImGuiPlatformIO`.
    /// This also clears Rust typed callback storage for this `PlatformIo`'s context and the
    /// aggregate ABI callback shim used by platform getters and vector-input setters.
    ///
    /// # Safety
    ///
    /// Dear ImGui must no longer be able to invoke any platform callback, and all platform-owned
    /// viewport state must already have been released.
    #[cfg(feature = "multi-viewport")]
    pub unsafe fn clear_platform_handlers(&mut self) {
        unsafe { sys::ImGuiPlatformIO_ClearPlatformHandlers(self.as_raw_mut()) }

        trampolines::clear_platform_callbacks_for_platform_io(self.as_raw());
        unsafe {
            clear_platform_aggregate_callbacks_for_platform_io(self.as_raw_mut());
        }
    }
}

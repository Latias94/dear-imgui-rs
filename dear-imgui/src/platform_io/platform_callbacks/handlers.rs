use crate::sys;

use super::super::core::clear_out_param_callbacks_for_platform_io;
use super::super::{PlatformIo, trampolines};

impl PlatformIo {
    /// Clear all platform backend handlers.
    ///
    /// This resets the `Platform_*` callback table stored in `ImGuiPlatformIO`.
    /// This also clears Rust typed callback storage for this `PlatformIo`'s context and the
    /// out-parameter callback shim used by aggregate-return platform getters.
    #[cfg(feature = "multi-viewport")]
    pub fn clear_platform_handlers(&mut self) {
        unsafe { sys::ImGuiPlatformIO_ClearPlatformHandlers(self.as_raw_mut()) }

        trampolines::clear_platform_callbacks_for_platform_io(self.as_raw());
        unsafe {
            clear_out_param_callbacks_for_platform_io(self.as_raw_mut());
        }
    }
}

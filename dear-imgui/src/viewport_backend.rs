//! Simplified viewport support for multi-viewport functionality
//!
//! This module provides simple utilities for multi-viewport support
//! without complex trait systems.

use crate::sys;

/// Simple utility functions for multi-viewport support
#[cfg(feature = "multi-viewport")]
pub mod utils {
    use super::*;

    /// Enable multi-viewport flags in ImGui context
    pub fn enable_viewport_flags(io: &mut crate::Io) {
        let mut flags = io.config_flags();
        flags.insert(crate::ConfigFlags::VIEWPORTS_ENABLE);
        flags.insert(crate::ConfigFlags::DOCKING_ENABLE);
        io.set_config_flags(flags);
    }
}

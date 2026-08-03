use std::path::PathBuf;

use dear_imgui_rs::{ConfigFlags, DockNodeFlags, WindowFlags};

/// Optional extension contexts created with the application UI state.
#[derive(Clone, Copy, Debug, Default)]
pub struct AddOnsConfig {
    pub with_implot: bool,
    pub with_imnodes: bool,
    pub with_implot3d: bool,
}

impl AddOnsConfig {
    /// Enables every add-on compiled into this crate.
    #[must_use]
    pub const fn auto() -> Self {
        Self {
            with_implot: cfg!(feature = "implot"),
            with_imnodes: cfg!(feature = "imnodes"),
            with_implot3d: cfg!(feature = "implot3d"),
        }
    }
}

/// Complete configuration for [`crate::run`] and [`crate::run_ui`].
pub struct AppConfig {
    pub window_title: String,
    pub window_size: (f64, f64),
    pub present_mode: wgpu::PresentMode,
    pub clear_color: [f32; 4],
    pub wgpu: WgpuConfig,
    pub docking: DockingConfig,
    pub addons: AddOnsConfig,
    pub ini_filename: Option<PathBuf>,
    pub restore_previous_geometry: bool,
    pub redraw: RedrawMode,
    pub io_config_flags: Option<ConfigFlags>,
    pub theme: Option<Theme>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            window_title: format!("Dear ImGui App - {}", env!("CARGO_PKG_VERSION")),
            window_size: (1280.0, 720.0),
            present_mode: wgpu::PresentMode::Fifo,
            clear_color: [0.1, 0.2, 0.3, 1.0],
            wgpu: WgpuConfig::default(),
            docking: DockingConfig::default(),
            addons: AddOnsConfig::default(),
            ini_filename: None,
            restore_previous_geometry: true,
            redraw: RedrawMode::Poll,
            io_config_flags: None,
            theme: None,
        }
    }
}

/// Adapter and device requirements used for every GPU generation.
pub struct WgpuConfig {
    pub backends: wgpu::Backends,
    pub power_preference: wgpu::PowerPreference,
    pub force_fallback_adapter: bool,
    pub device_label: Option<String>,
    pub required_features: wgpu::Features,
    pub required_limits: wgpu::Limits,
    pub memory_hints: wgpu::MemoryHints,
}

impl Default for WgpuConfig {
    fn default() -> Self {
        Self {
            backends: wgpu::Backends::PRIMARY,
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            device_label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
        }
    }
}

/// Curated WGPU adapter and limits profiles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WgpuPreset {
    #[default]
    Default,
    HighPerformance,
    LowPower,
    Balanced,
    DownlevelCompatible,
    SoftwareFallback,
}

impl WgpuConfig {
    #[must_use]
    pub fn from_preset(preset: WgpuPreset) -> Self {
        match preset {
            WgpuPreset::Default => Self::default(),
            WgpuPreset::HighPerformance => Self {
                power_preference: wgpu::PowerPreference::HighPerformance,
                memory_hints: wgpu::MemoryHints::Performance,
                ..Self::default()
            },
            WgpuPreset::LowPower => Self {
                power_preference: wgpu::PowerPreference::LowPower,
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                ..Self::default()
            },
            WgpuPreset::Balanced => Self {
                power_preference: wgpu::PowerPreference::None,
                ..Self::default()
            },
            WgpuPreset::DownlevelCompatible => Self {
                power_preference: wgpu::PowerPreference::None,
                required_limits: wgpu::Limits::downlevel_defaults(),
                ..Self::default()
            },
            WgpuPreset::SoftwareFallback => Self {
                power_preference: wgpu::PowerPreference::None,
                force_fallback_adapter: true,
                required_limits: wgpu::Limits::downlevel_defaults(),
                ..Self::default()
            },
        }
    }
}

/// Optional docking and built-in dockspace behavior.
///
/// Docking is disabled by default. Each enabled variant states whether `dear-app` or the
/// application owns the dockspace host window.
#[derive(Default)]
pub enum DockingConfig {
    /// Do not enable Dear ImGui docking.
    #[default]
    Disabled,
    /// Enable docking without drawing a dockspace host window.
    ApplicationManaged { dockspace_flags: DockNodeFlags },
    /// Enable docking and draw a full-viewport dockspace host window every frame.
    FullViewport {
        dockspace_flags: DockNodeFlags,
        host_window_flags: WindowFlags,
        host_window_name: String,
    },
}

impl DockingConfig {
    /// Enables docking while leaving dockspace creation to the application.
    #[must_use]
    pub fn application_managed() -> Self {
        Self::ApplicationManaged {
            dockspace_flags: DockNodeFlags::PASSTHRU_CENTRAL_NODE,
        }
    }

    /// Enables docking with a built-in full-viewport dockspace.
    #[must_use]
    pub fn full_viewport() -> Self {
        Self::FullViewport {
            dockspace_flags: DockNodeFlags::PASSTHRU_CENTRAL_NODE,
            host_window_flags: WindowFlags::NO_TITLE_BAR
                | WindowFlags::NO_RESIZE
                | WindowFlags::NO_MOVE
                | WindowFlags::NO_COLLAPSE
                | WindowFlags::NO_BRING_TO_FRONT_ON_FOCUS
                | WindowFlags::NO_NAV_FOCUS,
            host_window_name: "DockSpaceHost".to_owned(),
        }
    }

    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        !matches!(self, Self::Disabled)
    }

    pub(crate) fn dockspace_flags(&self) -> DockNodeFlags {
        match self {
            Self::Disabled => DockNodeFlags::empty(),
            Self::ApplicationManaged { dockspace_flags }
            | Self::FullViewport {
                dockspace_flags, ..
            } => DockNodeFlags::from_bits_retain(dockspace_flags.bits()),
        }
    }

    pub(crate) fn full_viewport_host(&self) -> Option<(&str, WindowFlags)> {
        match self {
            Self::FullViewport {
                host_window_flags,
                host_window_name,
                ..
            } => Some((
                host_window_name,
                WindowFlags::from_bits_retain(host_window_flags.bits()),
            )),
            Self::Disabled | Self::ApplicationManaged { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum RedrawMode {
    Poll,
    Wait,
    WaitUntil { fps: f32 },
}

#[derive(Clone, Copy, Debug)]
pub enum Theme {
    Dark,
    Light,
    Classic,
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, DockingConfig};

    #[test]
    fn default_app_does_not_enable_docking() {
        assert!(!AppConfig::default().docking.is_enabled());
    }

    #[test]
    fn docking_modes_assign_dockspace_ownership_explicitly() {
        let application_managed = DockingConfig::application_managed();
        let full_viewport = DockingConfig::full_viewport();

        assert!(application_managed.is_enabled());
        assert!(application_managed.full_viewport_host().is_none());
        assert!(full_viewport.is_enabled());
        assert!(full_viewport.full_viewport_host().is_some());
    }
}

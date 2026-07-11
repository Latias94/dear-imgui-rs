use std::path::PathBuf;

use dear_imgui_rs::{ConfigFlags, DockFlags, WindowFlags};

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

/// Complete configuration for [`crate::run`].
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

/// Built-in dockspace behavior.
pub struct DockingConfig {
    pub enable: bool,
    pub auto_dockspace: bool,
    pub dockspace_flags: DockFlags,
    pub host_window_flags: WindowFlags,
    pub host_window_name: &'static str,
}

impl Default for DockingConfig {
    fn default() -> Self {
        Self {
            enable: true,
            auto_dockspace: true,
            dockspace_flags: DockFlags::PASSTHRU_CENTRAL_NODE,
            host_window_flags: WindowFlags::NO_TITLE_BAR
                | WindowFlags::NO_RESIZE
                | WindowFlags::NO_MOVE
                | WindowFlags::NO_COLLAPSE
                | WindowFlags::NO_BRING_TO_FRONT_ON_FOCUS
                | WindowFlags::NO_NAV_FOCUS,
            host_window_name: "DockSpaceHost",
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

use bevy_window::{CompositeAlphaMode, PresentMode, Window, WindowTheme};

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use super::desktop::desktop_framebuffer_scale;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use super::desktop::{finite_desktop_pos, physical_outer_pos_for_client_pos};
#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
use super::desktop::{
    finite_desktop_size, physical_pos_from_desktop, positive_finite_or, set_window_desktop_size,
};
#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
use super::protocol::ImguiViewportSnapshot;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use super::{native_window, protocol::ImguiViewportFeedback};
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_ecs::entity::Entity;
#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
use bevy_window::CursorOptions;
#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
use bevy_window::{WindowLevel, WindowPosition, WindowResolution};
#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
use dear_imgui_rs as imgui;

/// Policy applied to every Bevy window created for a secondary Dear ImGui viewport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImguiViewportWindowConfig {
    pub present_mode: PresentMode,
    pub composite_alpha_mode: CompositeAlphaMode,
    pub desired_maximum_frame_latency: Option<std::num::NonZeroU32>,
    pub window_theme: Option<WindowTheme>,
    pub transparent: bool,
}

/// Invalid secondary-window presentation policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImguiViewportWindowConfigError {
    /// A transparent window selected a compositor mode that does not guarantee alpha blending.
    TransparentCompositeAlphaModeUnsupported {
        composite_alpha_mode: CompositeAlphaMode,
    },
}

impl std::fmt::Display for ImguiViewportWindowConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TransparentCompositeAlphaModeUnsupported {
                composite_alpha_mode,
            } => write!(
                formatter,
                "transparent Dear ImGui viewport windows require PreMultiplied or PostMultiplied composite alpha, got {composite_alpha_mode:?}"
            ),
        }
    }
}

impl std::error::Error for ImguiViewportWindowConfigError {}

impl Default for ImguiViewportWindowConfig {
    fn default() -> Self {
        Self::from_window(&Window::default())
    }
}

impl ImguiViewportWindowConfig {
    /// Copy the presentation policy of an existing Bevy window.
    #[must_use]
    pub fn from_window(window: &Window) -> Self {
        Self {
            present_mode: window.present_mode,
            composite_alpha_mode: window.composite_alpha_mode,
            desired_maximum_frame_latency: window.desired_maximum_frame_latency,
            window_theme: window.window_theme,
            transparent: window.transparent,
        }
    }

    /// Copy and validate the presentation policy of an existing Bevy window.
    pub fn try_from_window(window: &Window) -> Result<Self, ImguiViewportWindowConfigError> {
        Self::from_window(window).validate()
    }

    /// Validate that a transparent window uses a compositor mode which preserves alpha.
    pub fn validate(self) -> Result<Self, ImguiViewportWindowConfigError> {
        if self.transparent
            && !matches!(
                self.composite_alpha_mode,
                CompositeAlphaMode::PreMultiplied | CompositeAlphaMode::PostMultiplied
            )
        {
            return Err(
                ImguiViewportWindowConfigError::TransparentCompositeAlphaModeUnsupported {
                    composite_alpha_mode: self.composite_alpha_mode,
                },
            );
        }
        Ok(self)
    }

    #[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
    fn apply_to(self, window: &mut Window) {
        window.present_mode = self.present_mode;
        window.composite_alpha_mode = self.composite_alpha_mode;
        window.desired_maximum_frame_latency = self.desired_maximum_frame_latency;
        window.window_theme = self.window_theme;
        window.transparent = self.transparent;
    }
}

#[must_use]
#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
pub(crate) fn window_from_snapshot(snapshot: &ImguiViewportSnapshot) -> Window {
    window_from_snapshot_with_config(snapshot, ImguiViewportWindowConfig::default())
        .expect("the default viewport window configuration is valid")
}

/// Build a secondary Bevy window after validating its presentation policy.
#[cfg(any(test, all(feature = "multi-viewport", not(target_arch = "wasm32"))))]
pub(super) fn window_from_snapshot_with_config(
    snapshot: &ImguiViewportSnapshot,
    config: ImguiViewportWindowConfig,
) -> Result<Window, ImguiViewportWindowConfigError> {
    let config = config.validate()?;
    let scale_factor = positive_finite_or(snapshot.dpi_scale, 1.0);
    let desktop_size = finite_desktop_size(snapshot.size);
    let mut window = Window {
        title: format!("Dear ImGui Viewport {}", snapshot.id.raw()),
        position: WindowPosition::At(physical_pos_from_desktop(snapshot.pos, scale_factor)),
        resolution: WindowResolution::new(1, 1),
        decorations: !snapshot.flags.contains(imgui::ViewportFlags::NO_DECORATION),
        skip_taskbar: snapshot
            .flags
            .contains(imgui::ViewportFlags::NO_TASK_BAR_ICON),
        window_level: if snapshot.flags.contains(imgui::ViewportFlags::TOP_MOST) {
            WindowLevel::AlwaysOnTop
        } else {
            WindowLevel::Normal
        },
        visible: false,
        focused: false,
        ..Default::default()
    };
    window.resolution.set_scale_factor(scale_factor);
    set_window_desktop_size(&mut window, desktop_size, scale_factor);
    config.apply_to(&mut window);
    Ok(window)
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) fn apply_snapshot_to_window(
    snapshot: &ImguiViewportSnapshot,
    entity: Entity,
    window: &mut Window,
) {
    let next = window_from_snapshot(snapshot);
    window.position = WindowPosition::At(physical_outer_pos_for_client_pos(
        entity,
        snapshot.pos,
        snapshot.dpi_scale,
    ));
    window.resolution = next.resolution;
    apply_viewport_flags_to_window(snapshot.flags, window);
    window.focused = false;
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) fn apply_viewport_flags_to_window(flags: imgui::ViewportFlags, window: &mut Window) {
    window.decorations = !flags.contains(imgui::ViewportFlags::NO_DECORATION);
    window.skip_taskbar = flags.contains(imgui::ViewportFlags::NO_TASK_BAR_ICON);
    window.window_level = if flags.contains(imgui::ViewportFlags::TOP_MOST) {
        WindowLevel::AlwaysOnTop
    } else {
        WindowLevel::Normal
    };
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) fn apply_viewport_flags_to_cursor_options(
    flags: imgui::ViewportFlags,
    cursor_options: &mut CursorOptions,
) {
    if native_window::supports_pointer_passthrough() {
        cursor_options.hit_test = !flags.contains(imgui::ViewportFlags::NO_INPUTS);
    }
}

#[cfg(all(feature = "multi-viewport", not(target_arch = "wasm32")))]
pub(super) fn feedback_from_snapshot(snapshot: &ImguiViewportSnapshot) -> ImguiViewportFeedback {
    let dpi_scale = positive_finite_or(snapshot.dpi_scale, 1.0);
    ImguiViewportFeedback {
        pos: finite_desktop_pos(snapshot.pos),
        size: finite_desktop_size(snapshot.size),
        framebuffer_scale: desktop_framebuffer_scale(dpi_scale),
        dpi_scale,
        focused: false,
        minimized: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport::ImguiViewportId;

    #[test]
    fn secondary_window_inherits_complete_presentation_policy() {
        let snapshot = ImguiViewportSnapshot {
            id: ImguiViewportId::from(7_u32),
            pos: [0.0, 0.0],
            size: [320.0, 240.0],
            dpi_scale: 1.0,
            flags: imgui::ViewportFlags::empty(),
        };
        let config = ImguiViewportWindowConfig {
            present_mode: PresentMode::AutoNoVsync,
            composite_alpha_mode: CompositeAlphaMode::PostMultiplied,
            desired_maximum_frame_latency: std::num::NonZeroU32::new(3),
            window_theme: Some(WindowTheme::Dark),
            transparent: true,
        };

        let window = window_from_snapshot_with_config(&snapshot, config).unwrap();
        assert_eq!(window.present_mode, config.present_mode);
        assert_eq!(window.composite_alpha_mode, config.composite_alpha_mode);
        assert_eq!(
            window.desired_maximum_frame_latency,
            config.desired_maximum_frame_latency
        );
        assert_eq!(window.window_theme, config.window_theme);
        assert!(window.transparent);
    }
}

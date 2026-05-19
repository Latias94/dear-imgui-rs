use super::{Style, StyleColor};
use crate::Context;
use crate::internal::RawWrapper;
use crate::sys;
use crate::widget::{TableFlags, TableRowFlags};
use crate::window::WindowFlags;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Which base preset to start from when applying a [`Theme`].
///
/// This controls which built-in Dear ImGui color set is used as a starting
/// point before applying any overrides.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ThemePreset {
    /// Do not touch existing style colors; only apply explicit overrides.
    None,
    /// Use Dear ImGui's built-in dark preset.
    Dark,
    /// Use Dear ImGui's built-in light preset.
    Light,
    /// Use Dear ImGui's classic preset.
    Classic,
}

impl Default for ThemePreset {
    fn default() -> Self {
        ThemePreset::None
    }
}

/// A single color override for a given [`StyleColor`] entry.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct ColorOverride {
    /// Target style color to override.
    pub id: StyleColor,
    /// New RGBA color (0.0-1.0 range) to apply.
    pub rgba: [f32; 4],
}

/// High-level style tweaks that can be applied on top of a preset.
///
/// This does not expose the full `ImGuiStyle` surface, only the most commonly
/// themed fields. All fields are optional; `None` means "leave unchanged".
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct StyleTweaks {
    pub window_rounding: Option<f32>,
    pub frame_rounding: Option<f32>,
    pub tab_rounding: Option<f32>,

    pub window_padding: Option<[f32; 2]>,
    pub frame_padding: Option<[f32; 2]>,
    pub cell_padding: Option<[f32; 2]>,
    pub item_spacing: Option<[f32; 2]>,
    pub item_inner_spacing: Option<[f32; 2]>,

    pub scrollbar_size: Option<f32>,
    pub grab_min_size: Option<f32>,

    pub indent_spacing: Option<f32>,
    pub separator_size: Option<f32>,
    pub scrollbar_rounding: Option<f32>,
    pub grab_rounding: Option<f32>,
    pub window_border_size: Option<f32>,
    pub child_border_size: Option<f32>,
    pub popup_border_size: Option<f32>,
    pub frame_border_size: Option<f32>,
    pub tab_border_size: Option<f32>,
    pub child_rounding: Option<f32>,
    pub popup_rounding: Option<f32>,

    pub anti_aliased_lines: Option<bool>,
    pub anti_aliased_fill: Option<bool>,
}

impl Default for StyleTweaks {
    fn default() -> Self {
        Self {
            window_rounding: None,
            frame_rounding: None,
            tab_rounding: None,
            window_padding: None,
            frame_padding: None,
            cell_padding: None,
            item_spacing: None,
            item_inner_spacing: None,
            scrollbar_size: None,
            grab_min_size: None,
            indent_spacing: None,
            separator_size: None,
            scrollbar_rounding: None,
            grab_rounding: None,
            window_border_size: None,
            child_border_size: None,
            popup_border_size: None,
            frame_border_size: None,
            tab_border_size: None,
            child_rounding: None,
            popup_rounding: None,
            anti_aliased_lines: None,
            anti_aliased_fill: None,
        }
    }
}

impl StyleTweaks {
    /// Apply these tweaks to the given style.
    pub fn apply(&self, style: &mut Style) {
        if let Some(v) = self.window_rounding {
            style.set_window_rounding(v);
        }
        if let Some(v) = self.frame_rounding {
            style.set_frame_rounding(v);
        }
        if let Some(v) = self.tab_rounding {
            style.set_tab_rounding(v);
        }

        if let Some(v) = self.window_padding {
            style.set_window_padding(v);
        }
        if let Some(v) = self.frame_padding {
            style.set_frame_padding(v);
        }
        if let Some(v) = self.cell_padding {
            style.set_cell_padding(v);
        }
        if let Some(v) = self.item_spacing {
            style.set_item_spacing(v);
        }
        if let Some(v) = self.item_inner_spacing {
            style.set_item_inner_spacing(v);
        }

        if let Some(v) = self.scrollbar_size {
            style.set_scrollbar_size(v);
        }
        if let Some(v) = self.grab_min_size {
            style.set_grab_min_size(v);
        }

        if let Some(v) = self.indent_spacing {
            style.set_indent_spacing(v);
        }
        if let Some(v) = self.separator_size {
            style.set_separator_size(v);
        }
        if let Some(v) = self.scrollbar_rounding {
            style.set_scrollbar_rounding(v);
        }
        if let Some(v) = self.grab_rounding {
            style.set_grab_rounding(v);
        }
        if let Some(v) = self.window_border_size {
            style.set_window_border_size(v);
        }
        if let Some(v) = self.child_border_size {
            style.set_child_border_size(v);
        }
        if let Some(v) = self.popup_border_size {
            style.set_popup_border_size(v);
        }
        if let Some(v) = self.frame_border_size {
            style.set_frame_border_size(v);
        }
        if let Some(v) = self.tab_border_size {
            style.set_tab_border_size(v);
        }
        if let Some(v) = self.child_rounding {
            style.set_child_rounding(v);
        }
        if let Some(v) = self.popup_rounding {
            style.set_popup_rounding(v);
        }

        if let Some(v) = self.anti_aliased_lines {
            style.set_anti_aliased_lines(v);
        }
        if let Some(v) = self.anti_aliased_fill {
            style.set_anti_aliased_fill(v);
        }
    }
}

/// Window-related theme defaults (flags/behavior).
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct WindowTheme {
    /// Default flags for top-level windows.
    pub default_window_flags: Option<WindowFlags>,
    /// Default flags for popups/modals.
    pub popup_window_flags: Option<WindowFlags>,
}

/// Table-related theme defaults (flags/behavior).
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TableTheme {
    /// Default flags for tables created via `Ui::table` / `Ui::begin_table`.
    pub default_table_flags: Option<TableFlags>,
    /// Default row flags for data tables.
    pub default_row_flags: Option<TableRowFlags>,
}

/// High-level theme configuration for Dear ImGui.
///
/// A theme is applied in three stages:
/// 1) Choose a base preset (`Dark`/`Light`/`Classic` or `None`).
/// 2) Apply any explicit color overrides.
/// 3) Apply a small set of style tweaks.
///
/// Window/table defaults are provided as data and can be used by higher-level
/// helpers when building windows and tables.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Theme {
    /// Base preset to start from, before applying overrides.
    #[cfg_attr(feature = "serde", serde(default))]
    pub preset: ThemePreset,

    /// Color overrides on top of the preset.
    #[cfg_attr(feature = "serde", serde(default))]
    pub colors: Vec<ColorOverride>,

    /// Optional style tweaks on top of the preset.
    #[cfg_attr(feature = "serde", serde(default))]
    pub style: StyleTweaks,

    /// Window-related defaults (flags/behavior).
    #[cfg_attr(feature = "serde", serde(default))]
    pub windows: WindowTheme,

    /// Table-related defaults (flags/behavior).
    #[cfg_attr(feature = "serde", serde(default))]
    pub tables: TableTheme,
}

impl Theme {
    /// Apply this theme to a given style.
    ///
    /// This does not touch fonts or IO; it only updates `ImGuiStyle`.
    pub fn apply_to_style(&self, style: &mut Style) {
        // 1) Base preset
        match self.preset {
            ThemePreset::None => {}
            ThemePreset::Dark => unsafe {
                sys::igStyleColorsDark(style.raw_mut());
            },
            ThemePreset::Light => unsafe {
                sys::igStyleColorsLight(style.raw_mut());
            },
            ThemePreset::Classic => unsafe {
                sys::igStyleColorsClassic(style.raw_mut());
            },
        }

        // 2) Color overrides
        for c in &self.colors {
            style.set_color(c.id, c.rgba);
        }

        // 3) Common style tweaks
        self.style.apply(style);
    }

    /// Apply this theme to the given context (current style).
    pub fn apply_to_context(&self, ctx: &mut Context) {
        let style = ctx.style_mut();
        self.apply_to_style(style);
    }
}

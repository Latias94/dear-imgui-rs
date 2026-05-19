#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::as_conversions
)]

use super::Style;
use super::validation::assert_non_negative_f32;
use crate::sys;
use crate::utils::{HoveredFlags, validate_tooltip_hovered_flags};

impl Style {
    pub fn hover_stationary_delay(&self) -> f32 {
        self.inner().HoverStationaryDelay
    }
    pub fn set_hover_stationary_delay(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_hover_stationary_delay()", "v", v);
        self.inner_mut().HoverStationaryDelay = v;
    }

    pub fn hover_delay_short(&self) -> f32 {
        self.inner().HoverDelayShort
    }
    pub fn set_hover_delay_short(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_hover_delay_short()", "v", v);
        self.inner_mut().HoverDelayShort = v;
    }

    pub fn hover_delay_normal(&self) -> f32 {
        self.inner().HoverDelayNormal
    }
    pub fn set_hover_delay_normal(&mut self, v: f32) {
        assert_non_negative_f32("Style::set_hover_delay_normal()", "v", v);
        self.inner_mut().HoverDelayNormal = v;
    }

    pub fn hover_flags_for_tooltip_mouse(&self) -> HoveredFlags {
        HoveredFlags::from_bits_truncate(self.inner().HoverFlagsForTooltipMouse as i32)
    }
    pub fn set_hover_flags_for_tooltip_mouse(&mut self, flags: HoveredFlags) {
        validate_tooltip_hovered_flags("Style::set_hover_flags_for_tooltip_mouse()", flags);
        self.inner_mut().HoverFlagsForTooltipMouse = flags.bits() as sys::ImGuiHoveredFlags;
    }

    pub fn hover_flags_for_tooltip_nav(&self) -> HoveredFlags {
        HoveredFlags::from_bits_truncate(self.inner().HoverFlagsForTooltipNav as i32)
    }
    pub fn set_hover_flags_for_tooltip_nav(&mut self, flags: HoveredFlags) {
        validate_tooltip_hovered_flags("Style::set_hover_flags_for_tooltip_nav()", flags);
        self.inner_mut().HoverFlagsForTooltipNav = flags.bits() as sys::ImGuiHoveredFlags;
    }
}

// HoveredFlags are defined in utils.rs and re-exported at crate root.

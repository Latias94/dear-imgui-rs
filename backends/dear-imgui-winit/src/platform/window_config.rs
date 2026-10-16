use std::rc::Rc;

use dear_imgui_rs::NativeWindowTarget;
use winit::dpi::PhysicalSize;
use winit::window::{Icon, Theme, WindowAttributes};

use super::WinitPlatformError;
use super::ownership::{WinitPlatform, WinitPlatformControl};

/// Application policy applied before a secondary native window is created.
///
/// Geometry, initial visibility, decorations, native ownership and platform sidecars remain
/// backend-controlled. Changes affect future secondary windows; existing windows are unchanged.
#[derive(Clone, Debug)]
pub struct WinitViewportWindowConfig {
    pub icon: Option<Icon>,
    /// Lower client-size bound in physical pixels.
    pub min_inner_size: Option<PhysicalSize<u32>>,
    /// Upper client-size bound in physical pixels.
    pub max_inner_size: Option<PhysicalSize<u32>>,
    pub theme: Option<Theme>,
    pub resizable: bool,
    /// Requests a transparent native background; the renderer must also supply suitable alpha.
    pub transparent: bool,
}

impl Default for WinitViewportWindowConfig {
    fn default() -> Self {
        Self {
            icon: None,
            min_inner_size: None,
            max_inner_size: None,
            theme: None,
            resizable: true,
            transparent: false,
        }
    }
}

impl WinitViewportWindowConfig {
    /// Validates representable client-size limits before allocating native resources.
    pub fn validate(&self) -> Result<(), WinitPlatformError> {
        for (field, value) in [
            ("min_inner_size", self.min_inner_size),
            ("max_inner_size", self.max_inner_size),
        ] {
            if value.is_some_and(|size| {
                size.width == 0
                    || size.height == 0
                    || size.width > i32::MAX as u32
                    || size.height > i32::MAX as u32
            }) {
                return Err(WinitPlatformError::InvalidWindowConfiguration {
                    field,
                    reason: "dimensions must be positive and fit in i32",
                });
            }
        }
        if let (Some(min), Some(max)) = (self.min_inner_size, self.max_inner_size)
            && (min.width > max.width || min.height > max.height)
        {
            return Err(WinitPlatformError::InvalidWindowConfiguration {
                field: "size_limits",
                reason: "minimum dimensions exceed maximum dimensions",
            });
        }
        if cfg!(target_os = "macos") && self.icon.is_some() {
            return Err(WinitPlatformError::WindowPolicyUnsupported {
                policy: "window_icon",
                operation: "secondary window creation",
            });
        }
        Ok(())
    }

    pub(crate) fn apply(&self, attributes: WindowAttributes) -> WindowAttributes {
        let mut attributes = attributes
            .with_window_icon(self.icon.clone())
            .with_theme(self.theme)
            .with_resizable(self.resizable)
            .with_transparent(self.transparent);
        if let Some(size) = self.min_inner_size {
            attributes = attributes.with_min_inner_size(size);
        }
        if let Some(size) = self.max_inner_size {
            attributes = attributes.with_max_inner_size(size);
        }
        attributes
    }
}

/// Detached creation facts supplied to an application configurator.
///
/// `target.instance_id` is reserved for this attempt and is never reused after failure. Receiving
/// this value does not mean a window was created; only a `Created` lifecycle event proves that.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WinitViewportCreateInfo {
    pub target: NativeWindowTarget,
    /// Requested position in the backend's Dear ImGui desktop coordinate units.
    pub desktop_position: [f32; 2],
    /// Requested client size in the backend's Dear ImGui desktop coordinate units.
    pub desktop_size: [f32; 2],
    pub parent: Option<NativeWindowTarget>,
}

pub(super) type WindowConfigurator = dyn Fn(
    WinitViewportCreateInfo,
    &mut WinitViewportWindowConfig,
) -> Result<(), WinitPlatformError>;

impl WinitPlatformControl {
    pub(crate) fn configure_viewport_window(
        &self,
        info: WinitViewportCreateInfo,
    ) -> Result<WinitViewportWindowConfig, WinitPlatformError> {
        let mut config = self.viewport_window_config.borrow().clone();
        let configurator = self.viewport_window_configurator.borrow().clone();
        if let Some(configurator) = configurator {
            configurator(info, &mut config)?;
        }
        config.validate()?;
        Ok(config)
    }
}

impl WinitPlatform {
    /// Sets bounded creation policy for future secondary windows.
    pub fn set_viewport_window_config(
        &mut self,
        config: WinitViewportWindowConfig,
    ) -> Result<(), WinitPlatformError> {
        self.control.validate_operational_contract()?;
        config.validate()?;
        self.control.viewport_window_config.replace(config);
        Ok(())
    }

    /// Installs per-window policy without exposing owned callback slots or raw native attributes.
    ///
    /// The callback runs before native creation. Errors reject that attempt; panics are contained
    /// by the platform callback boundary. Neither path publishes a registration or lifecycle event.
    pub fn set_viewport_window_configurator(
        &mut self,
        configurator: impl Fn(
            WinitViewportCreateInfo,
            &mut WinitViewportWindowConfig,
        ) -> Result<(), WinitPlatformError>
        + 'static,
    ) -> Result<(), WinitPlatformError> {
        self.control.validate_operational_contract()?;
        self.control
            .viewport_window_configurator
            .replace(Some(Rc::new(configurator)));
        Ok(())
    }

    /// Removes the per-window configurator while retaining the base creation policy.
    pub fn clear_viewport_window_configurator(&mut self) -> Result<(), WinitPlatformError> {
        self.control.validate_operational_contract()?;
        self.control
            .viewport_window_configurator
            .borrow_mut()
            .take();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use dear_imgui_rs::{Context, Id, NativeWindowInstanceId};

    use super::*;
    use crate::test_util::test_sync::lock_context;

    #[test]
    fn invalid_size_limits_are_rejected_before_native_creation() {
        for (min, max) in [
            (Some(PhysicalSize::new(0, 10)), None),
            (None, Some(PhysicalSize::new(u32::MAX, 10))),
            (
                Some(PhysicalSize::new(500, 100)),
                Some(PhysicalSize::new(100, 500)),
            ),
        ] {
            let config = WinitViewportWindowConfig {
                min_inner_size: min,
                max_inner_size: max,
                ..Default::default()
            };
            assert!(matches!(
                config.validate(),
                Err(WinitPlatformError::InvalidWindowConfiguration { .. })
            ));
        }
    }

    #[test]
    fn bounded_configuration_preserves_backend_geometry_and_hidden_creation() {
        let config = WinitViewportWindowConfig {
            resizable: false,
            transparent: true,
            min_inner_size: Some(PhysicalSize::new(100, 80)),
            ..Default::default()
        };
        let attributes = WindowAttributes::default()
            .with_inner_size(PhysicalSize::new(300, 200))
            .with_position(winit::dpi::PhysicalPosition::new(17, 29))
            .with_visible(false)
            .with_decorations(false);
        let geometry = (attributes.inner_size, attributes.position);
        let configured = config.apply(attributes);
        assert_eq!((configured.inner_size, configured.position), geometry);
        assert!(!configured.visible);
        assert!(!configured.decorations);
        assert!(!configured.resizable);
        assert!(configured.transparent);
        assert!(configured.parent_window().is_none());
    }

    #[test]
    fn configurator_is_validated_and_does_not_replace_the_base_policy() {
        let _guard = lock_context();
        let mut context = Context::create();
        let mut platform = WinitPlatform::new(&mut context).unwrap();
        let info = WinitViewportCreateInfo {
            target: NativeWindowTarget {
                instance_id: NativeWindowInstanceId::allocate(context.id()).unwrap(),
                viewport_id: Id::from(7),
                is_main: false,
            },
            desktop_position: [0.0, 0.0],
            desktop_size: [100.0, 100.0],
            parent: None,
        };
        platform
            .set_viewport_window_configurator(|_, config| {
                config.min_inner_size = Some(PhysicalSize::new(0, 100));
                Ok(())
            })
            .unwrap();
        assert!(platform.control.configure_viewport_window(info).is_err());
        platform.clear_viewport_window_configurator().unwrap();
        assert!(
            platform
                .control
                .configure_viewport_window(info)
                .unwrap()
                .min_inner_size
                .is_none()
        );
        assert!(platform.drain_window_lifecycle_events().is_empty());
        platform.shutdown(&mut context).unwrap();
    }
}

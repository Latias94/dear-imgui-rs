use std::sync::Arc;

use dear_imgui_rs::{Id, NativeWindowInstanceId, NativeWindowTarget};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::window::{Window, WindowId};

use super::WinitPlatformError;
use super::ownership::{WinitPlatform, WinitPlatformControl};

/// Detached native facts about one currently owned Winit window registration.
///
/// This value retains neither a native window nor a Context. Its routing ID and geometry describe
/// the observation time; use the instance ID to resolve the same registration later. A retired
/// instance never resolves to a replacement, even if Winit or the operating system reuses an ID.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WinitWindowSnapshot {
    /// Stable registration and the Dear ImGui viewport associated with it at observation time.
    pub target: NativeWindowTarget,
    /// Winit event-routing identifier at observation time.
    pub window_id: WindowId,
    /// Client area size in physical pixels.
    pub physical_client_size: PhysicalSize<u32>,
    /// Client origin in native physical desktop coordinates, when the window system exposes it.
    ///
    /// This is Winit's native observation, independent of Dear ImGui's desktop coordinate units.
    pub physical_client_origin: Option<PhysicalPosition<i32>>,
    /// Native Winit scale factor at observation time.
    pub scale_factor: f64,
    pub native_state: super::WinitWindowNativeState,
    pub capabilities: super::WinitWindowCapabilities,
}

impl WinitWindowSnapshot {
    pub(crate) fn capture(target: NativeWindowTarget, window: &Window) -> Self {
        Self {
            target,
            window_id: window.id(),
            physical_client_size: window.inner_size(),
            physical_client_origin: window.inner_position().ok(),
            scale_factor: window.scale_factor(),
            native_state: super::WinitWindowNativeState::capture(window),
            capabilities: super::WinitWindowCapabilities::capture(window),
        }
    }
}

impl WinitPlatformControl {
    #[cfg(feature = "multi-viewport")]
    pub(crate) fn main_window_render_state(
        &self,
    ) -> Result<std::rc::Rc<super::window_render_lease::WindowRenderState>, WinitPlatformError>
    {
        self.attached_window_render_state
            .borrow()
            .clone()
            .ok_or(WinitPlatformError::WindowNotAttached)
    }

    #[cfg(feature = "multi-viewport")]
    pub(crate) fn main_window_instance_id(
        &self,
    ) -> Result<NativeWindowInstanceId, WinitPlatformError> {
        self.attached_window_target
            .get()
            .map(|target| target.instance_id)
            .ok_or(WinitPlatformError::WindowNotAttached)
    }

    pub(crate) fn window_snapshots(&self) -> Result<Vec<WinitWindowSnapshot>, WinitPlatformError> {
        self.binding.try_with_bound_context(|| {
            self.validate_complete_contract_in_current_context()?;
            #[cfg(feature = "multi-viewport")]
            {
                let runtime = self.runtime.borrow().clone();
                if let Some(runtime) = runtime {
                    return runtime.window_snapshots_in_current_context();
                }
            }
            let attached = self.attached_window.borrow();
            let Some(window) = attached.as_ref() else {
                return Ok(Vec::new());
            };
            let Some(mut target) = self.attached_window_target.get() else {
                return Err(WinitPlatformError::WindowNotAttached);
            };
            let viewport = unsafe { dear_imgui_rs::sys::igGetMainViewport() };
            if viewport.is_null() {
                return Err(WinitPlatformError::ContextMismatch);
            }
            target.viewport_id = Id::from(unsafe { (*viewport).ID });
            self.attached_window_target.set(Some(target));
            Ok(vec![WinitWindowSnapshot::capture(target, window)])
        })?
    }

    pub(crate) fn with_window<R>(
        &self,
        instance_id: NativeWindowInstanceId,
        callback: impl for<'window> FnOnce(&'window Window) -> R,
    ) -> Result<Option<R>, WinitPlatformError> {
        self.binding.try_with_bound_context(|| {
            self.validate_complete_contract_in_current_context()?;
            if instance_id.context_id() != self.binding.id() {
                return Ok(None);
            }
            #[cfg(feature = "multi-viewport")]
            {
                let runtime = self.runtime.borrow().clone();
                if let Some(runtime) = runtime {
                    return runtime.with_window_in_current_context(instance_id, callback);
                }
            }
            if self
                .attached_window_target
                .get()
                .map(|target| target.instance_id)
                != Some(instance_id)
            {
                return Ok(None);
            }
            // Release the registry borrow before user code, while retaining the exact allocation
            // for this scoped access. The public callback cannot clone or retain the private Arc.
            let window = self.attached_window.borrow().as_ref().map(Arc::clone);
            Ok(window.map(|window| callback(&window)))
        })?
    }
}

impl WinitPlatform {
    /// Observes all live registrations owned by this platform, including hidden viewports.
    ///
    /// Results are sorted by registration serial. Reading the registry never drains callback
    /// faults. A replaced owner or native publication returns an error instead of stale handles.
    pub fn window_snapshots(&self) -> Result<Vec<WinitWindowSnapshot>, WinitPlatformError> {
        self.control.window_snapshots()
    }

    /// Observes the live registration currently routed by a Winit event identifier.
    pub fn window_snapshot(
        &self,
        window_id: WindowId,
    ) -> Result<Option<WinitWindowSnapshot>, WinitPlatformError> {
        Ok(self
            .window_snapshots()?
            .into_iter()
            .find(|snapshot| snapshot.window_id == window_id))
    }

    /// Observes this exact registration, returning `None` after retirement or for another owner.
    pub fn window_snapshot_for_instance(
        &self,
        instance_id: NativeWindowInstanceId,
    ) -> Result<Option<WinitWindowSnapshot>, WinitPlatformError> {
        Ok(self
            .window_snapshots()?
            .into_iter()
            .find(|snapshot| snapshot.target.instance_id == instance_id))
    }

    /// Borrows the exact live Winit window for one operation without retaining native ownership.
    ///
    /// The callback may return owned results, but its window borrow cannot escape this scope.
    /// Queries validate the current owner and do not consume deferred callback faults.
    ///
    /// ```compile_fail
    /// use dear_imgui_rs::NativeWindowInstanceId;
    /// use dear_imgui_winit::WinitPlatform;
    /// use winit::window::Window;
    ///
    /// fn retain_window<'a>(platform: &'a WinitPlatform, id: NativeWindowInstanceId) -> &'a Window {
    ///     platform.with_window(id, |window| window).unwrap().unwrap()
    /// }
    /// ```
    pub fn with_window<R>(
        &self,
        instance_id: NativeWindowInstanceId,
        callback: impl for<'window> FnOnce(&'window Window) -> R,
    ) -> Result<Option<R>, WinitPlatformError> {
        self.control.with_window(instance_id, callback)
    }
}

#[cfg(test)]
mod tests {
    use dear_imgui_rs::Context;

    use super::*;
    use crate::test_util::test_sync::lock_context;

    #[test]
    fn detached_snapshots_do_not_retain_a_window_or_context() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<WinitWindowSnapshot>();
    }

    #[test]
    fn an_unattached_platform_has_no_window_capabilities() {
        let _guard = lock_context();
        let mut context = Context::create();
        let mut platform = WinitPlatform::new(&mut context).unwrap();
        let unrelated = NativeWindowInstanceId::allocate(context.id()).unwrap();
        assert!(platform.window_snapshots().unwrap().is_empty());
        assert_eq!(platform.window_snapshot(WindowId::dummy()), Ok(None));
        assert_eq!(platform.window_snapshot_for_instance(unrelated), Ok(None));
        assert_eq!(platform.with_window(unrelated, |_| ()), Ok(None));
        platform.shutdown(&mut context).unwrap();
        assert_eq!(
            platform.window_snapshots(),
            Err(WinitPlatformError::RuntimeDetached)
        );
    }

    #[test]
    fn registry_queries_reject_a_foreign_platform_publication() {
        let _guard = lock_context();
        let mut context = Context::create();
        let mut platform = WinitPlatform::new(&mut context).unwrap();
        let io = unsafe { dear_imgui_rs::sys::igGetIO_ContextPtr(context.as_raw()) };
        let original = unsafe { (*io).BackendPlatformUserData };
        unsafe { (*io).BackendPlatformUserData = std::ptr::dangling_mut::<u8>().cast() };
        assert_eq!(
            platform.window_snapshots(),
            Err(WinitPlatformError::PlatformStateReplaced {
                field: "BackendPlatformUserData"
            })
        );
        unsafe { (*io).BackendPlatformUserData = original };
        platform.shutdown(&mut context).unwrap();
    }

    #[test]
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    #[ignore = "requires a native desktop; run the named native-window registration regression"]
    #[allow(
        deprecated,
        reason = "The fixture creates hidden windows without running an app"
    )]
    fn native_window_registration_lifecycle() {
        let _guard = lock_context();
        let mut builder = winit::event_loop::EventLoop::builder();
        #[cfg(target_os = "windows")]
        {
            use winit::platform::windows::EventLoopBuilderExtWindows;
            builder.with_any_thread(true);
        }
        #[cfg(target_os = "linux")]
        {
            use winit::platform::x11::EventLoopBuilderExtX11;
            builder.with_x11().with_any_thread(true);
        }
        let event_loop = builder.build().expect("a native desktop must be available");
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_visible(false))
                .unwrap(),
        );
        let mut context = Context::create();
        let mut platform = WinitPlatform::new(&mut context).unwrap();
        platform
            .attach_window(Arc::clone(&window), crate::HiDpiMode::Default, &mut context)
            .unwrap();
        let retained_windows = Arc::strong_count(&window);
        let frozen = platform.window_snapshot(window.id()).unwrap().unwrap();
        assert_eq!(Arc::strong_count(&window), retained_windows);
        let instance = frozen.target.instance_id;
        let created = platform.drain_window_lifecycle_events();
        assert!(
            matches!(created.as_slice(), [super::super::WinitWindowLifecycleEvent::Created(snapshot)] if snapshot.target == frozen.target)
        );
        assert_eq!(instance.context_id(), context.id());
        assert!(frozen.target.is_main);
        assert_eq!(frozen.physical_client_size, window.inner_size());
        assert_eq!(
            platform.with_window(instance, Window::id),
            Ok(Some(window.id()))
        );
        let pressed = platform
            .handle_window_event(
                &mut context,
                window.id(),
                &winit::event::WindowEvent::Touch(winit::event::Touch {
                    device_id: winit::event::DeviceId::dummy(),
                    phase: winit::event::TouchPhase::Started,
                    location: winit::dpi::PhysicalPosition::new(40.0, 60.0),
                    force: None,
                    id: 7,
                }),
            )
            .unwrap();
        assert!(pressed.forwarded.pointer);
        let released = platform
            .handle_window_event(
                &mut context,
                window.id(),
                &winit::event::WindowEvent::Focused(false),
            )
            .unwrap();
        assert!(released.forwarded.focus && released.forwarded.pointer);

        platform
            .attach_window(Arc::clone(&window), crate::HiDpiMode::Default, &mut context)
            .unwrap();
        assert_eq!(
            platform
                .window_snapshot(window.id())
                .unwrap()
                .unwrap()
                .target
                .instance_id,
            instance
        );
        assert!(platform.drain_window_lifecycle_events().is_empty());

        #[cfg(feature = "multi-viewport")]
        {
            platform.enable_viewports(&mut context).unwrap();
            let adapter = platform.viewport_renderer_adapter(&context).unwrap();
            assert_eq!(
                adapter
                    .window_snapshot_for_instance(instance)
                    .unwrap()
                    .unwrap()
                    .target
                    .instance_id,
                instance
            );
            let viewport = unsafe { dear_imgui_rs::sys::igGetMainViewport() };
            let original_id = unsafe { (*viewport).ID };
            let remapped_id = original_id.wrapping_add(1);
            unsafe { (*viewport).ID = remapped_id };
            let remapped = adapter
                .window_snapshot_for_instance(instance)
                .unwrap()
                .unwrap();
            assert_eq!(remapped.target.viewport_id, Id::from(remapped_id));
            assert_eq!(remapped.target.instance_id, frozen.target.instance_id);
            assert_eq!(frozen.target.viewport_id, Id::from(original_id));
            unsafe { (*viewport).ID = original_id };

            let original_handle = unsafe { (*viewport).PlatformHandle };
            unsafe { (*viewport).PlatformHandle = std::ptr::dangling_mut::<u8>().cast() };
            assert_eq!(
                adapter.window_snapshot_for_instance(instance),
                Err(WinitPlatformError::ViewportOwnershipLost {
                    viewport_id: original_id,
                    field: "PlatformHandle"
                })
            );
            unsafe { (*viewport).PlatformHandle = original_handle };
            assert!(adapter.window_lease(instance).unwrap().is_some());

            platform.disable_viewports(&mut context).unwrap();
            assert_eq!(
                platform
                    .window_snapshot(window.id())
                    .unwrap()
                    .unwrap()
                    .target
                    .instance_id,
                instance
            );
            platform.enable_viewports(&mut context).unwrap();
            assert_eq!(
                adapter.window_snapshots(),
                Err(WinitPlatformError::RuntimeDetached)
            );
            assert_eq!(
                platform
                    .window_snapshot(window.id())
                    .unwrap()
                    .unwrap()
                    .target
                    .instance_id,
                instance
            );
            platform.disable_viewports(&mut context).unwrap();
            assert!(platform.drain_window_lifecycle_events().is_empty());
        }

        drop(platform.detach_window(&mut context).unwrap());
        assert_eq!(
            platform.drain_window_lifecycle_events(),
            vec![super::super::WinitWindowLifecycleEvent::Retired(
                frozen.target
            )]
        );
        assert_eq!(platform.with_window(instance, |_| ()), Ok(None));
        platform
            .attach_window(Arc::clone(&window), crate::HiDpiMode::Default, &mut context)
            .unwrap();
        let replacement = platform.window_snapshot(window.id()).unwrap().unwrap();
        assert!(
            matches!(platform.drain_window_lifecycle_events().as_slice(), [super::super::WinitWindowLifecycleEvent::Created(snapshot)] if snapshot.target == replacement.target)
        );
        assert_ne!(replacement.target.instance_id, instance);
        assert_eq!(platform.window_snapshot_for_instance(instance), Ok(None));
        platform.shutdown(&mut context).unwrap();
        assert_eq!(
            platform.drain_window_lifecycle_events(),
            vec![super::super::WinitWindowLifecycleEvent::Retired(
                replacement.target
            )]
        );
        platform.shutdown(&mut context).unwrap();
        assert!(platform.drain_window_lifecycle_events().is_empty());

        let mut new_owner = WinitPlatform::new(&mut context).unwrap();
        new_owner
            .attach_window(Arc::clone(&window), crate::HiDpiMode::Default, &mut context)
            .unwrap();
        let new_registration = new_owner.window_snapshot(window.id()).unwrap().unwrap();
        assert_ne!(
            new_registration.target.instance_id,
            replacement.target.instance_id
        );
        assert_eq!(new_owner.window_snapshot_for_instance(instance), Ok(None));
        assert_eq!(
            new_owner.with_window(replacement.target.instance_id, |_| ()),
            Ok(None)
        );
        new_owner.shutdown(&mut context).unwrap();
        assert_eq!(frozen.window_id, window.id());
    }
}

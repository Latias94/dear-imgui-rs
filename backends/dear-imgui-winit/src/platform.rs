//! Main platform implementation for Dear ImGui winit backend
//!
//! This module contains the core `WinitPlatform` struct and its implementation
//! for integrating Dear ImGui with winit windowing.

mod frame;
mod input_events;
mod ownership;
mod window_state;

use dear_imgui_rs::{
    ContextAttachmentError, ContextBindingError, ContextPlatformAttachmentReleaseError,
    ContextPlatformWindowTeardownError,
};
use thiserror::Error;

pub use frame::HiDpiMode;
pub use ownership::WinitPlatform;
#[cfg(feature = "multi-viewport")]
pub(crate) use ownership::{WINIT_VIEWPORT_FLAGS, WinitPlatformControl};

/// Failure to attach or operate the Winit platform backend.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum WinitPlatformError {
    /// The Dear ImGui Context rejected the platform attachment.
    #[error(transparent)]
    Attachment(#[from] ContextAttachmentError),
    /// The Context rejected release of the active platform attachment generation.
    #[error(transparent)]
    PlatformAttachmentRelease(#[from] ContextPlatformAttachmentReleaseError),
    /// The originating Dear ImGui Context can no longer be entered normally.
    #[error(transparent)]
    Context(#[from] ContextBindingError),
    /// Dear ImGui rejected an explicit platform-window teardown transaction.
    #[error(transparent)]
    PlatformWindowTeardown(#[from] ContextPlatformWindowTeardownError),
    /// The supplied Context is not the Context owned by this platform backend.
    #[error("the Winit platform backend belongs to a different Dear ImGui context")]
    ContextMismatch,
    /// Another platform backend already owns a required global field.
    #[error("Dear ImGui platform state `{field}` is already owned")]
    PlatformStateOccupied { field: &'static str },
    /// A field claimed by this platform backend changed while it remained attached.
    #[error("Dear ImGui platform state `{field}` changed while Winit was attached")]
    PlatformStateReplaced { field: &'static str },
    /// No main window has been attached to the platform backend.
    #[error("attach a main Winit window before using this operation")]
    WindowNotAttached,
    /// The supplied window is not the platform backend's attached main window.
    #[error("the Winit window does not match the platform backend's attached main window")]
    WindowMismatch,
    /// Multi-viewport support is already attached to this platform owner.
    #[error("Winit multi-viewport support is already attached")]
    RuntimeAlreadyAttached,
    /// A configuration mutation would invalidate the active multi-viewport coordinate contract.
    #[error("Winit platform configuration is locked while multi-viewport support is attached")]
    RuntimeConfigurationLocked,
    /// The build artifact lacks the aggregate callback bridge required by this backend.
    #[error("dear-imgui-sys was built without PlatformIO aggregate ABI hooks")]
    AggregateCallbackHooksUnavailable,
    /// Another platform backend already owns one of the required callback slots.
    #[error("ImGuiPlatformIO callback `{callback}` is already owned by another platform backend")]
    PlatformCallbackOccupied { callback: &'static str },
    /// Another platform backend already advertises a capability owned by this runtime.
    #[error("Dear ImGui backend capability `{flag}` is already owned by another platform backend")]
    PlatformCapabilityOccupied { flag: &'static str },
    /// A slot in the captured platform callback table changed while the runtime remained attached.
    #[error(
        "Winit platform callback table slot `{callback}` changed while the runtime was attached"
    )]
    PlatformCallbackReplaced { callback: &'static str },
    /// Platform teardown was requested before the renderer released its viewport callback.
    #[error("renderer state `{field}` is still installed; shut down the renderer before Winit")]
    RendererShutdownRequired { field: &'static str },
    /// A viewport already has platform data owned by another backend.
    #[error("viewport platform data or handle is already owned by another platform backend")]
    ForeignPlatformUserData,
    /// A live viewport stopped matching the Winit platform data registered for it.
    #[error("Winit lost ownership of viewport {viewport_id} field `{field}`")]
    ViewportOwnershipLost {
        /// Dear ImGui viewport identifier whose native platform state drifted.
        viewport_id: u32,
        /// Native platform field whose value no longer matches Winit's registration.
        field: &'static str,
    },
    /// Winit did not expose any monitor geometry that can back Dear ImGui viewports.
    #[error("Winit did not expose any monitor geometry")]
    NoMonitors,
    /// Winit could not form a complete native monitor publication.
    #[cfg(feature = "multi-viewport")]
    #[error("Winit native monitor collection is unavailable: {reason}")]
    MonitorCollectionUnavailable {
        reason: crate::multi_viewport::WinitMonitorCollectionFailure,
    },
    /// Winit exposed monitor geometry that violates Dear ImGui's platform contract.
    #[error("Winit monitor {monitor} is invalid: {reason}")]
    InvalidMonitorGeometry {
        monitor: usize,
        reason: &'static str,
    },
    /// Dear ImGui supplied viewport geometry that cannot be represented by Winit.
    #[error("Dear ImGui viewport geometry is invalid during {operation}: {reason}")]
    InvalidViewportGeometry {
        operation: &'static str,
        reason: &'static str,
    },
    /// Custom single-window coordinate scaling is not implemented for platform viewports.
    #[error("Winit multi-viewport requires HiDpiMode::Default")]
    CustomHiDpiModeUnsupported,
    /// Wayland cannot provide the desktop-space positioning required by Dear ImGui viewports.
    #[error("Wayland is unsupported by the Winit multi-viewport backend; use X11 on Linux")]
    WaylandUnsupported,
    /// The target has no supported native desktop window-system contract.
    #[error("the Winit multi-viewport backend does not support target `{target}`")]
    UnsupportedWindowSystem { target: &'static str },
    /// A requested viewport flag cannot be implemented faithfully for this operation.
    #[error("Winit cannot honor viewport flag `{flag}` during {operation}")]
    UnsupportedViewportFlag {
        flag: &'static str,
        operation: &'static str,
    },
    /// The monitor count cannot be represented by Dear ImGui's native vector.
    #[error("the Winit monitor count exceeds i32::MAX")]
    MonitorCountOverflow,
    /// Dear ImGui's allocator could not reserve monitor storage.
    #[error("Dear ImGui failed to allocate Winit monitor storage")]
    MonitorStorageAllocationFailed,
    /// Dear ImGui requested a new viewport outside a scoped Winit event-loop entry.
    #[error("Winit viewport creation requires WinitPlatform::with_event_loop")]
    EventLoopUnavailable,
    /// Winit failed to create a secondary viewport window.
    #[error("Winit failed to create a secondary viewport window: {message}")]
    WindowCreation { message: String },
    /// A fallible operation on a secondary Winit window failed.
    #[error("Winit viewport operation `{operation}` failed: {message}")]
    WindowOperation {
        operation: &'static str,
        message: String,
    },
    /// A Rust platform callback panicked; the panic was contained at the C ABI boundary.
    #[error("Winit platform callback `{callback}` panicked")]
    CallbackPanicked { callback: &'static str },
    /// The owning runtime has already shut down or entered a terminal fault.
    #[error("the Winit platform runtime is no longer attached")]
    RuntimeDetached,
    #[cfg(test)]
    #[error("injected Winit construction failure after {stage}")]
    InjectedConstructionFailure { stage: &'static str },
}

#[cfg(test)]
mod tests {
    use std::ffi::{CStr, CString};
    use std::rc::Rc;

    use dear_imgui_rs::{
        BackendFlags, Context, ContextAttachment, ContextAttachmentRole,
        ContextPlatformAttachmentReleaseError,
    };
    use winit::event::{Event, WindowEvent};

    use super::input_events::{event_targets_window, rescale_mouse_pos_for_hidpi_change};
    use super::ownership::{
        PlatformOwnerToken, PlatformState, WINIT_BASE_FLAGS, WINIT_RESERVED_FLAGS,
        winit_backend_name_ptr,
    };
    use super::window_state::ime_callback_eq;
    use super::{HiDpiMode, WinitPlatform, WinitPlatformError};
    use crate::test_util::test_sync::lock_context;

    struct ActiveRendererMarker;
    struct ActiveRendererAttachment;

    impl ContextAttachment for ActiveRendererAttachment {}

    unsafe extern "C" fn foreign_ime_callback(
        _context: *mut dear_imgui_rs::sys::ImGuiContext,
        _viewport: *mut dear_imgui_rs::sys::ImGuiViewport,
        _data: *mut dear_imgui_rs::sys::ImGuiPlatformImeData,
    ) {
    }

    #[test]
    fn test_hidpi_mode_default() {
        assert_eq!(HiDpiMode::default(), HiDpiMode::Default);
    }

    #[test]
    fn test_platform_creation() {
        let _guard = lock_context();
        let mut ctx = Context::create();
        let platform = WinitPlatform::new(&mut ctx).unwrap();

        assert_eq!(platform.hidpi_mode, HiDpiMode::Default);
        assert_eq!(platform.hidpi_factor, 1.0);
        assert_eq!(platform.cursor_cache, None);
        assert!(!platform.ime_enabled);
    }

    #[test]
    fn platform_shutdown_rejects_an_active_renderer_before_releasing_base_state() {
        let _guard = lock_context();
        let mut context = Context::create();
        let mut platform = WinitPlatform::new(&mut context).unwrap();
        let mut renderer = context
            .register_attachment::<ActiveRendererMarker>(
                ContextAttachmentRole::Renderer,
                Rc::new(ActiveRendererAttachment),
            )
            .unwrap();
        let io = unsafe { dear_imgui_rs::sys::igGetIO_ContextPtr(context.as_raw()) };

        assert!(matches!(
            platform.shutdown(&mut context),
            Err(WinitPlatformError::PlatformAttachmentRelease(
                ContextPlatformAttachmentReleaseError::RendererActive
            ))
        ));
        assert_eq!(
            unsafe { (*io).BackendPlatformUserData },
            platform.control.token_ptr()
        );
        assert!(platform.control.attachment_handle().unwrap().is_attached());

        assert_eq!(renderer.detach(), Ok(true));
        platform.shutdown(&mut context).unwrap();
    }

    #[test]
    fn platform_drop_defers_base_release_while_a_renderer_attachment_is_active() {
        let _guard = lock_context();
        let mut context = Context::create();
        let platform = WinitPlatform::new(&mut context).unwrap();
        let control = platform.control();
        let mut renderer = context
            .register_attachment::<ActiveRendererMarker>(
                ContextAttachmentRole::Renderer,
                Rc::new(ActiveRendererAttachment),
            )
            .unwrap();
        let io = unsafe { dear_imgui_rs::sys::igGetIO_ContextPtr(context.as_raw()) };

        drop(platform);

        assert_eq!(
            unsafe { (*io).BackendPlatformUserData },
            control.token_ptr()
        );
        assert!(control.attachment_handle().unwrap().is_attached());
        assert_eq!(renderer.detach(), Ok(true));
        drop(context);
        assert_eq!(control.state.get(), PlatformState::ContextDestroyed);
    }

    #[test]
    fn platform_claim_publishes_stable_identity_and_cleans_up_exact_ownership() {
        let _guard = lock_context();
        let mut context = Context::create();
        let platform_io =
            unsafe { dear_imgui_rs::sys::igGetPlatformIO_ContextPtr(context.as_raw()) };
        let baseline_ime_callback = unsafe { (*platform_io).Platform_SetImeDataFn };
        let baseline_ime_user_data = unsafe { (*platform_io).Platform_ImeUserData };

        let platform = WinitPlatform::new(&mut context).unwrap();
        let control = platform.control();
        let io = unsafe { dear_imgui_rs::sys::igGetIO_ContextPtr(context.as_raw()) };

        assert_ne!(std::mem::size_of::<PlatformOwnerToken>(), 0);
        assert_eq!(
            unsafe { (*io).BackendPlatformName },
            winit_backend_name_ptr()
        );
        assert_eq!(
            unsafe { (*io).BackendPlatformUserData },
            control.token_ptr()
        );
        assert_eq!(
            unsafe { CStr::from_ptr((*io).BackendPlatformName) },
            unsafe { CStr::from_ptr(winit_backend_name_ptr()) }
        );
        assert_eq!(
            BackendFlags::from_bits_retain(unsafe { (*io).BackendFlags }) & WINIT_RESERVED_FLAGS,
            WINIT_BASE_FLAGS
        );
        assert!(ime_callback_eq(
            unsafe { (*platform_io).Platform_SetImeDataFn },
            baseline_ime_callback
        ));
        assert_eq!(
            unsafe { (*platform_io).Platform_ImeUserData },
            baseline_ime_user_data
        );

        drop(platform);

        assert!(unsafe { (*io).BackendPlatformName.is_null() });
        assert!(unsafe { (*io).BackendPlatformUserData.is_null() });
        assert!(
            (BackendFlags::from_bits_retain(unsafe { (*io).BackendFlags }) & WINIT_RESERVED_FLAGS)
                .is_empty()
        );
        assert!(ime_callback_eq(
            unsafe { (*platform_io).Platform_SetImeDataFn },
            baseline_ime_callback
        ));
        assert_eq!(
            unsafe { (*platform_io).Platform_ImeUserData },
            baseline_ime_user_data
        );
    }

    #[test]
    fn platform_attachment_is_unique_per_context_and_reusable_after_release() {
        let _guard = lock_context();
        let mut context = Context::create();
        let platform = WinitPlatform::new(&mut context).unwrap();

        let error = match WinitPlatform::new(&mut context) {
            Ok(_) => panic!("a Context cannot have two Winit platform owners"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            WinitPlatformError::PlatformStateOccupied {
                field: "BackendPlatformName"
            }
        );

        drop(platform);
        drop(WinitPlatform::new(&mut context).unwrap());
    }

    #[test]
    fn base_contract_reports_each_replaced_owned_field() {
        let _guard = lock_context();
        let mut context = Context::create();
        let mut platform = WinitPlatform::new(&mut context).unwrap();
        let control = platform.control();
        let io = unsafe { dear_imgui_rs::sys::igGetIO_ContextPtr(context.as_raw()) };
        let platform_io =
            unsafe { dear_imgui_rs::sys::igGetPlatformIO_ContextPtr(context.as_raw()) };
        let baseline_ime_callback = unsafe { (*platform_io).Platform_SetImeDataFn };

        let validate = || {
            control
                .binding()
                .with_bound_context(|| control.validate_complete_contract_in_current_context())
        };

        unsafe { (*io).BackendPlatformName = std::ptr::null() };
        assert_eq!(
            validate(),
            Err(WinitPlatformError::PlatformStateReplaced {
                field: "BackendPlatformName"
            })
        );
        unsafe { (*io).BackendPlatformName = winit_backend_name_ptr() };

        unsafe { (*io).BackendPlatformUserData = std::ptr::null_mut() };
        assert_eq!(
            validate(),
            Err(WinitPlatformError::PlatformStateReplaced {
                field: "BackendPlatformUserData"
            })
        );
        unsafe { (*io).BackendPlatformUserData = control.token_ptr() };

        unsafe { (*io).BackendFlags &= !WINIT_BASE_FLAGS.bits() };
        assert_eq!(
            validate(),
            Err(WinitPlatformError::PlatformStateReplaced {
                field: "BackendFlags"
            })
        );
        unsafe { (*io).BackendFlags |= WINIT_BASE_FLAGS.bits() };

        unsafe { (*platform_io).Platform_SetImeDataFn = Some(foreign_ime_callback) };
        assert_eq!(
            validate(),
            Err(WinitPlatformError::PlatformStateReplaced {
                field: "Platform_SetImeDataFn"
            })
        );
        unsafe { (*platform_io).Platform_SetImeDataFn = baseline_ime_callback };

        let foreign_ime_user_data = std::ptr::dangling_mut::<u8>().cast();
        unsafe { (*platform_io).Platform_ImeUserData = foreign_ime_user_data };
        assert_eq!(
            validate(),
            Err(WinitPlatformError::PlatformStateReplaced {
                field: "Platform_ImeUserData"
            })
        );
        unsafe { (*platform_io).Platform_ImeUserData = std::ptr::null_mut() };

        platform.shutdown(&mut context).unwrap();
    }

    #[test]
    fn public_base_entry_latches_contract_drift_until_ordered_shutdown() {
        let _guard = lock_context();
        let mut context = Context::create();
        let mut platform = WinitPlatform::new(&mut context).unwrap();
        let io = unsafe { dear_imgui_rs::sys::igGetIO_ContextPtr(context.as_raw()) };
        unsafe { (*io).BackendPlatformName = std::ptr::null() };

        let expected = WinitPlatformError::PlatformStateReplaced {
            field: "BackendPlatformName",
        };
        assert_eq!(
            platform.set_software_cursor_enabled(&mut context, true),
            Err(expected.clone())
        );
        assert!(
            !BackendFlags::from_bits_retain(unsafe { (*io).BackendFlags })
                .contains(WINIT_BASE_FLAGS)
        );

        unsafe { (*io).BackendPlatformName = winit_backend_name_ptr() };
        assert_eq!(
            platform.set_software_cursor_enabled(&mut context, false),
            Err(expected.clone())
        );
        assert_eq!(platform.shutdown(&mut context), Err(expected));
        assert!(unsafe { (*io).BackendPlatformName.is_null() });
        assert!(unsafe { (*io).BackendPlatformUserData.is_null() });
        assert_eq!(platform.shutdown(&mut context), Ok(()));
    }

    #[test]
    fn shutdown_preserves_a_same_text_foreign_backend_name_pointer() {
        let _guard = lock_context();
        let mut context = Context::create();
        let mut platform = WinitPlatform::new(&mut context).unwrap();
        let io = unsafe { dear_imgui_rs::sys::igGetIO_ContextPtr(context.as_raw()) };
        let foreign_name = CString::new(
            unsafe { CStr::from_ptr(winit_backend_name_ptr()) }
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert_ne!(foreign_name.as_ptr(), winit_backend_name_ptr());
        unsafe { (*io).BackendPlatformName = foreign_name.as_ptr() };

        assert_eq!(
            platform.shutdown(&mut context),
            Err(WinitPlatformError::PlatformStateReplaced {
                field: "BackendPlatformName"
            })
        );
        assert_eq!(unsafe { (*io).BackendPlatformName }, foreign_name.as_ptr());
        assert_eq!(
            unsafe { CStr::from_ptr((*io).BackendPlatformName) },
            unsafe { CStr::from_ptr(winit_backend_name_ptr()) }
        );
        assert!(unsafe { (*io).BackendPlatformUserData.is_null() });

        unsafe { (*io).BackendPlatformName = std::ptr::null() };
        drop(WinitPlatform::new(&mut context).unwrap());
    }

    #[test]
    fn explicit_shutdown_preserves_complete_foreign_base_takeover_and_flags() {
        let _guard = lock_context();
        let mut context = Context::create();
        let mut platform = WinitPlatform::new(&mut context).unwrap();
        let io = unsafe { dear_imgui_rs::sys::igGetIO_ContextPtr(context.as_raw()) };
        let foreign_name = CString::new("foreign-platform").unwrap();
        let foreign_token = std::ptr::dangling_mut::<u8>().cast();
        unsafe {
            (*io).BackendPlatformName = foreign_name.as_ptr();
            (*io).BackendPlatformUserData = foreign_token;
        }

        assert_eq!(
            platform.shutdown(&mut context),
            Err(WinitPlatformError::PlatformStateReplaced {
                field: "BackendPlatformUserData"
            })
        );
        assert_eq!(unsafe { (*io).BackendPlatformName }, foreign_name.as_ptr());
        assert_eq!(unsafe { (*io).BackendPlatformUserData }, foreign_token);
        assert!(
            BackendFlags::from_bits_retain(unsafe { (*io).BackendFlags })
                .contains(WINIT_BASE_FLAGS)
        );

        unsafe {
            (*io).BackendPlatformName = std::ptr::null();
            (*io).BackendPlatformUserData = std::ptr::null_mut();
            (*io).BackendFlags &= !WINIT_BASE_FLAGS.bits();
        }
    }

    #[test]
    fn drop_preserves_complete_foreign_base_takeover_and_flags() {
        let _guard = lock_context();
        let mut context = Context::create();
        let io = unsafe { dear_imgui_rs::sys::igGetIO_ContextPtr(context.as_raw()) };
        let foreign_name = CString::new("foreign-platform").unwrap();
        let foreign_token = std::ptr::dangling_mut::<u8>().cast();
        let platform = WinitPlatform::new(&mut context).unwrap();
        unsafe {
            (*io).BackendPlatformName = foreign_name.as_ptr();
            (*io).BackendPlatformUserData = foreign_token;
        }

        drop(platform);

        assert_eq!(unsafe { (*io).BackendPlatformName }, foreign_name.as_ptr());
        assert_eq!(unsafe { (*io).BackendPlatformUserData }, foreign_token);
        assert!(
            BackendFlags::from_bits_retain(unsafe { (*io).BackendFlags })
                .contains(WINIT_BASE_FLAGS)
        );

        unsafe {
            (*io).BackendPlatformName = std::ptr::null();
            (*io).BackendPlatformUserData = std::ptr::null_mut();
            (*io).BackendFlags &= !WINIT_BASE_FLAGS.bits();
        }
    }

    #[test]
    fn complete_foreign_takeover_does_not_revoke_foreign_flags_on_contract_fault() {
        let _guard = lock_context();
        let mut context = Context::create();
        let mut platform = WinitPlatform::new(&mut context).unwrap();
        let io = unsafe { dear_imgui_rs::sys::igGetIO_ContextPtr(context.as_raw()) };
        let foreign_name = CString::new("foreign-platform").unwrap();
        let foreign_token = std::ptr::dangling_mut::<u8>().cast();
        unsafe {
            (*io).BackendPlatformName = foreign_name.as_ptr();
            (*io).BackendPlatformUserData = foreign_token;
        }

        let expected = WinitPlatformError::PlatformStateReplaced {
            field: "BackendPlatformName",
        };
        assert_eq!(
            platform.set_software_cursor_enabled(&mut context, true),
            Err(expected.clone())
        );
        assert!(
            BackendFlags::from_bits_retain(unsafe { (*io).BackendFlags })
                .contains(WINIT_BASE_FLAGS)
        );
        assert_eq!(platform.shutdown(&mut context), Err(expected));
        assert_eq!(unsafe { (*io).BackendPlatformName }, foreign_name.as_ptr());
        assert_eq!(unsafe { (*io).BackendPlatformUserData }, foreign_token);
        assert!(
            BackendFlags::from_bits_retain(unsafe { (*io).BackendFlags })
                .contains(WINIT_BASE_FLAGS)
        );

        unsafe {
            (*io).BackendPlatformName = std::ptr::null();
            (*io).BackendPlatformUserData = std::ptr::null_mut();
            (*io).BackendFlags &= !WINIT_BASE_FLAGS.bits();
        }
    }

    #[test]
    fn test_hidpi_mode_setting() {
        let _guard = lock_context();
        let mut ctx = Context::create();
        let mut platform = WinitPlatform::new(&mut ctx).unwrap();

        platform.set_hidpi_mode(HiDpiMode::Locked(2.0)).unwrap();
        assert_eq!(platform.hidpi_mode, HiDpiMode::Locked(2.0));

        platform.set_hidpi_mode(HiDpiMode::Rounded).unwrap();
        assert_eq!(platform.hidpi_mode, HiDpiMode::Rounded);
    }

    #[test]
    fn full_window_events_are_filtered_by_window_id_before_dispatch() {
        let target = winit::window::WindowId::from(41_u64);
        let foreign = winit::window::WindowId::from(42_u64);
        let foreign_event = Event::<()>::WindowEvent {
            window_id: foreign,
            event: WindowEvent::Focused(true),
        };
        let target_event = Event::<()>::WindowEvent {
            window_id: target,
            event: WindowEvent::Focused(true),
        };

        assert!(!event_targets_window(target, &foreign_event));
        assert!(event_targets_window(target, &target_event));
        assert!(event_targets_window(target, &Event::<()>::AboutToWait));
    }

    #[test]
    fn rescale_mouse_pos_for_hidpi_change_rejects_non_finite_results() {
        assert_eq!(
            rescale_mouse_pos_for_hidpi_change([10.0, 20.0], 1.0, 2.0),
            Some([20.0, 40.0])
        );
        assert_eq!(
            rescale_mouse_pos_for_hidpi_change([f32::NAN, 20.0], 1.0, 2.0),
            None
        );
        assert_eq!(
            rescale_mouse_pos_for_hidpi_change([10.0, 20.0], 0.0, 2.0),
            Some([20.0, 40.0])
        );
        assert_eq!(
            rescale_mouse_pos_for_hidpi_change([f32::MAX, 20.0], 1.0, f64::MAX),
            None
        );
    }

    #[test]
    fn test_window_attributes_creation() {
        let attrs = WinitPlatform::create_window_attributes();
        // Just test that it doesn't panic - actual values depend on winit defaults
        let _ = attrs;
    }
}

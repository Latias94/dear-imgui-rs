use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::rc::Rc;
#[cfg(not(any(feature = "gpu-allocator", feature = "vk-mem")))]
use std::sync::Arc;

#[cfg(not(any(feature = "gpu-allocator", feature = "vk-mem")))]
use ash::vk;

use dear_imgui_rs::{
    BackendFlags, Context, ContextAttachment, ContextAttachmentLease, ContextAttachmentRole,
    ContextAttachmentTeardownError, ContextBinding, ContextTeardown, FrameLifecycleState, Id, sys,
};
use static_assertions::{assert_impl_all, assert_not_impl_any};

use super::callbacks::{
    renderer_create_window_sys, renderer_destroy_window_sys, renderer_probe_runtime_sys,
    renderer_render_window_sys, renderer_swap_buffers_sys, run_work_callback,
};
use super::registry::SurfaceSupportError;
use super::runtime::{
    AshPreparedViewportFrame, AshViewportError, AshViewportFrameCompletion, AshViewportRouteFault,
    OwningViewportRuntime, RuntimeControl, RuntimeState, preflight_attachment_with,
};
#[cfg(not(any(feature = "gpu-allocator", feature = "vk-mem")))]
use super::{SurfaceAdapter, SurfaceCreateError, ViewportSwapchainPolicy, VulkanViewportConfig};
use crate::RendererError;
#[cfg(not(any(feature = "gpu-allocator", feature = "vk-mem")))]
use crate::renderer::lifecycle::renderer_for_test;

assert_impl_all!(AshViewportFrameCompletion: Send, Sync);
assert_not_impl_any!(AshPreparedViewportFrame<'static>: Send, Sync);

struct TestPlatformMarker;

#[cfg(not(any(feature = "gpu-allocator", feature = "vk-mem")))]
struct InertSurfaceAdapter;

#[cfg(not(any(feature = "gpu-allocator", feature = "vk-mem")))]
impl SurfaceAdapter for InertSurfaceAdapter {
    unsafe fn create_surface(
        &self,
        _entry: &ash::Entry,
        _instance: &ash::Instance,
        _viewport: &mut dear_imgui_rs::platform_io::Viewport,
    ) -> Result<vk::SurfaceKHR, SurfaceCreateError> {
        unreachable!("attachment preflight must fail before surface creation")
    }
}

struct TestPlatformAttachment;

impl ContextAttachment for TestPlatformAttachment {
    fn release_platform_windows(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        context.with_bound_context(clear_test_main_handle_raw);
        Ok(())
    }
}

struct TestPlatformLease {
    binding: ContextBinding,
    _lease: ContextAttachmentLease,
}

impl Drop for TestPlatformLease {
    fn drop(&mut self) {
        let _ = self
            .binding
            .try_with_bound_context(clear_test_main_handle_raw);
    }
}

struct OrderingPlatformMarker;

struct OrderingPlatformAttachment {
    control: Rc<RefCell<Option<Rc<RuntimeControl>>>>,
    renderer_released_first: Rc<Cell<bool>>,
}

impl ContextAttachment for OrderingPlatformAttachment {
    fn release_platform_windows(
        &self,
        context: &ContextTeardown<'_>,
    ) -> Result<(), ContextAttachmentTeardownError> {
        self.renderer_released_first.set(
            self.control
                .borrow()
                .as_ref()
                .is_some_and(|control| control.state() == RuntimeState::ResourceDropped),
        );
        context.with_bound_context(clear_test_main_handle_raw);
        Ok(())
    }
}

unsafe extern "C" fn platform_unary(_viewport: *mut sys::ImGuiViewport) {}
unsafe extern "C" fn foreign_renderer_unary(_viewport: *mut sys::ImGuiViewport) {}
unsafe extern "C" fn foreign_renderer_render(
    _viewport: *mut sys::ImGuiViewport,
    _argument: *mut c_void,
) {
}
unsafe extern "C" fn foreign_renderer_set_window_size(
    _viewport: *mut sys::ImGuiViewport,
    _size: *const sys::ImVec2,
) {
}

fn claim_test_platform_callbacks(context: &mut Context) {
    let io = context.io_mut();
    io.set_backend_flags(io.backend_flags() | BackendFlags::PLATFORM_HAS_VIEWPORTS);
    let platform_io = context.platform_io_mut();
    unsafe {
        platform_io.set_platform_create_window_raw(Some(platform_unary));
        platform_io.set_platform_destroy_window_raw(Some(platform_unary));
        context
            .main_viewport()
            .set_platform_handle(std::ptr::dangling_mut::<c_void>());
    }
}

fn clear_test_main_handle_raw() {
    let viewport = unsafe { sys::igGetMainViewport() };
    if !viewport.is_null() {
        unsafe {
            (*viewport).PlatformHandle = std::ptr::null_mut();
            (*viewport).PlatformHandleRaw = std::ptr::null_mut();
        }
    }
}

fn attach_test_platform(context: &mut Context) -> TestPlatformLease {
    let lease = context
        .register_attachment::<TestPlatformMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(TestPlatformAttachment),
        )
        .unwrap();
    claim_test_platform_callbacks(context);
    TestPlatformLease {
        binding: context.binding(),
        _lease: lease,
    }
}

#[cfg(not(any(feature = "gpu-allocator", feature = "vk-mem")))]
fn inert_vulkan_config() -> VulkanViewportConfig {
    let entry = ash::Entry::from_parts_1_1(
        ash::StaticFn::load(|_| std::ptr::null()),
        ash::EntryFnV1_0::load(|_| std::ptr::null()),
        ash::EntryFnV1_1::load(|_| std::ptr::null()),
    );
    let instance = unsafe { ash::Instance::load_with(|_| std::ptr::null(), vk::Instance::null()) };
    VulkanViewportConfig {
        entry,
        instance,
        physical_device: vk::PhysicalDevice::null(),
        validation_surface: vk::SurfaceKHR::null(),
        present_queue: vk::Queue::null(),
        graphics_queue_family_index: 0,
        present_queue_family_index: 0,
        swapchain_policy: ViewportSwapchainPolicy::default(),
        swapchain_image_usage: vk::ImageUsageFlags::empty(),
    }
}

#[test]
fn attach_requires_registered_platform_role_without_claiming_callbacks() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    claim_test_platform_callbacks(&mut context);

    let error = OwningViewportRuntime::attach_for_test(&mut context).unwrap_err();

    assert!(matches!(
        error,
        AshViewportError::Attachment(dear_imgui_rs::ContextAttachmentError::MissingPlatform)
    ));
    assert!(context.platform_io().renderer_callbacks_are_empty());
    clear_test_main_handle_raw();
}

#[cfg(not(any(feature = "gpu-allocator", feature = "vk-mem")))]
#[test]
fn failed_real_attach_returns_the_renderer_and_preserves_shutdown_ownership() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let mut renderer = renderer_for_test(&mut context);
    renderer.default_texture_id = 0xA551;

    let error = unsafe {
        OwningViewportRuntime::attach(
            &mut context,
            renderer,
            inert_vulkan_config(),
            Arc::new(InertSurfaceAdapter),
        )
    }
    .unwrap_err();

    assert!(matches!(
        error.error(),
        AshViewportError::PlatformBackendUnavailable
    ));
    assert!(context.platform_io().renderer_callbacks_are_empty());

    let mut renderer = error.into_renderer();
    assert_eq!(renderer.default_texture_id, 0xA551);
    assert!(renderer.consumer.is_some());

    renderer
        .shutdown_without_vulkan_for_test(&mut context)
        .unwrap();
    assert!(renderer.destroyed);
    assert!(renderer.consumer.is_none());
    assert!(context.io().backend_renderer_user_data().is_null());
}

#[test]
fn every_occupied_renderer_slot_rejects_attach_transactionally() {
    let _guard = super::test_context_guard();
    for slot in 0..5 {
        let mut context = Context::create();
        let _platform = attach_test_platform(&mut context);
        let platform_io = context.platform_io_mut();
        unsafe {
            match slot {
                0 => platform_io.set_renderer_create_window_raw(Some(foreign_renderer_unary)),
                1 => platform_io.set_renderer_destroy_window_raw(Some(foreign_renderer_unary)),
                2 => platform_io
                    .set_renderer_set_window_size_raw(Some(foreign_renderer_set_window_size)),
                3 => platform_io.set_renderer_render_window_raw(Some(foreign_renderer_render)),
                4 => platform_io.set_renderer_swap_buffers_raw(Some(foreign_renderer_render)),
                _ => unreachable!(),
            }
        }

        let error = OwningViewportRuntime::attach_for_test(&mut context).unwrap_err();
        assert!(matches!(
            error,
            AshViewportError::RendererCallbackOccupied { .. }
        ));
        let occupied = match slot {
            0 => context.platform_io().renderer_create_window_raw().is_some(),
            1 => context
                .platform_io()
                .renderer_destroy_window_raw()
                .is_some(),
            2 => unsafe {
                (*context.platform_io().as_raw())
                    .Renderer_SetWindowSize
                    .is_some()
            },
            3 => context.platform_io().renderer_render_window_raw().is_some(),
            4 => context.platform_io().renderer_swap_buffers_raw().is_some(),
            _ => unreachable!(),
        };
        assert!(occupied);
        unsafe { context.platform_io_mut().clear_renderer_handlers() };
    }
}

#[test]
fn occupied_renderer_viewport_capability_rejects_attach_transactionally() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let io = context.io_mut();
    io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_VIEWPORTS);

    let error = OwningViewportRuntime::attach_for_test(&mut context).unwrap_err();
    assert!(matches!(
        error,
        AshViewportError::RendererViewportCapabilityOccupied
    ));
    assert!(context.platform_io().renderer_callbacks_are_empty());
    assert!(
        context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS),
        "failed attach must preserve the foreign capability bit"
    );

    let io = context.io_mut();
    io.set_backend_flags(io.backend_flags() & !BackendFlags::RENDERER_HAS_VIEWPORTS);
}

#[test]
fn renderer_lease_failure_precedes_attach_publication() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    unsafe {
        context
            .platform_io_mut()
            .set_renderer_create_window_raw(Some(foreign_renderer_unary));
    }

    let error =
        preflight_attachment_with(&context, || Err(RendererError::RendererDestroyed.into()))
            .unwrap_err();

    assert!(matches!(
        error,
        AshViewportError::Renderer(RendererError::RendererDestroyed)
    ));
    assert!(
        context
            .platform_io()
            .renderer_create_window_raw()
            .is_some_and(|callback| {
                std::ptr::fn_addr_eq(
                    callback,
                    foreign_renderer_unary as unsafe extern "C" fn(*mut sys::ImGuiViewport),
                )
            })
    );
    assert!(
        !context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );

    unsafe {
        context
            .platform_io_mut()
            .set_renderer_create_window_raw(None);
    }
    let mut runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn moving_wrapper_keeps_runtime_owned_storage_stable_and_shutdown_is_once() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();
    let address = runtime.renderer_address_for_test();

    fn move_runtime(runtime: OwningViewportRuntime) -> OwningViewportRuntime {
        runtime
    }

    let mut runtime = move_runtime(runtime);
    assert_eq!(runtime.renderer_address_for_test(), address);
    unsafe { renderer_probe_runtime_sys() };
    assert_eq!(runtime.callback_probe_count_for_test(), 1);
    runtime.shutdown(&mut context).unwrap();
    runtime.shutdown(&mut context).unwrap();
    assert_eq!(runtime.state_for_test(), RuntimeState::ResourceDropped);
    assert_eq!(
        runtime.transition_log_for_test(),
        ["ShuttingDown", "Detached", "ResourceDropped"]
    );
}

#[test]
fn dropping_runtime_while_context_is_alive_defers_cleanup_to_context() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();
    let control = runtime.control_for_test();

    drop(runtime);

    assert_eq!(control.state(), RuntimeState::Attached);
    assert!(
        context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS),
        "Drop must not release resources while a live Context still owns managed bindings"
    );
    assert!(context.platform_io().renderer_create_window_raw().is_some());

    drop(context);

    assert_eq!(control.state(), RuntimeState::ResourceDropped);
}

#[test]
fn callback_panic_and_reentry_are_deferred_to_rust_entry() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();

    runtime.trigger_reentrant_entry_for_test();
    assert!(matches!(
        runtime.poll_fault(),
        Err(AshViewportError::CallbackReentered { .. })
    ));

    runtime.panic_next_callback_for_test();
    unsafe { renderer_render_window_sys(std::ptr::null_mut(), std::ptr::null_mut()) };
    assert!(matches!(
        runtime.poll_fault(),
        Err(AshViewportError::CallbackPanicked {
            callback: "Renderer_RenderWindow"
        })
    ));
    assert_eq!(runtime.state_for_test(), RuntimeState::ShuttingDown);
    assert!(
        !context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );

    // Destroy is a cleanup entry and remains callable after a terminal callback fault.
    unsafe { renderer_destroy_window_sys(std::ptr::null_mut()) };
    assert!(matches!(
        runtime.poll_fault(),
        Err(AshViewportError::InvalidCallbackArgument {
            callback: "Renderer_DestroyWindow"
        })
    ));
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn device_loss_from_each_vulkan_callback_stage_is_terminal() {
    let _guard = super::test_context_guard();

    for operation in [
        "wait_for_fences",
        "acquire_next_image",
        "queue_submit",
        "queue_present",
    ] {
        let mut context = Context::create();
        let _platform = attach_test_platform(&mut context);
        let mut runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();

        run_work_callback(operation, |_| {
            Err(RendererError::Vulkan(ash::vk::Result::ERROR_DEVICE_LOST).into())
        });

        assert_eq!(runtime.state_for_test(), RuntimeState::ShuttingDown);
        assert!(
            !context
                .io()
                .backend_flags()
                .contains(BackendFlags::RENDERER_HAS_VIEWPORTS),
            "{operation} device loss must revoke renderer viewport capability"
        );
        assert!(matches!(
            runtime.poll_fault(),
            Err(AshViewportError::Renderer(RendererError::Vulkan(
                ash::vk::Result::ERROR_DEVICE_LOST
            )))
        ));

        unsafe { renderer_probe_runtime_sys() };
        assert_eq!(
            runtime.callback_probe_count_for_test(),
            0,
            "{operation} device loss allowed another renderer callback after polling"
        );
        runtime.shutdown(&mut context).unwrap();
    }
}

#[test]
fn device_loss_wrapped_by_a_surface_query_is_terminal() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();

    run_work_callback("Renderer_CreateWindow", |_| {
        Err(AshViewportError::SurfaceUnsupported(
            SurfaceSupportError::CapabilitiesQuery(ash::vk::Result::ERROR_DEVICE_LOST),
        ))
    });

    assert_eq!(runtime.state_for_test(), RuntimeState::ShuttingDown);
    assert!(
        !context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );
    assert!(matches!(
        runtime.poll_fault(),
        Err(AshViewportError::SurfaceUnsupported(
            SurfaceSupportError::CapabilitiesQuery(ash::vk::Result::ERROR_DEVICE_LOST)
        ))
    ));
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn renderer_state_drift_is_a_terminal_entry_fault() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();

    run_work_callback("injected renderer state drift", |_| {
        Err(AshViewportError::Renderer(
            RendererError::RendererStateDrift {
                field: "BackendRendererUserData",
            },
        ))
    });

    assert_eq!(runtime.state_for_test(), RuntimeState::ShuttingDown);
    assert!(
        !context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );
    assert!(matches!(
        runtime.poll_fault(),
        Err(AshViewportError::Renderer(
            RendererError::RendererStateDrift {
                field: "BackendRendererUserData"
            }
        ))
    ));
    unsafe { renderer_probe_runtime_sys() };
    assert_eq!(runtime.callback_probe_count_for_test(), 0);
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn recoverable_surface_fault_remains_non_terminal() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();

    run_work_callback("Renderer_CreateWindow", |_| {
        Err(AshViewportError::SurfaceUnsupported(
            SurfaceSupportError::CapabilitiesQuery(ash::vk::Result::ERROR_SURFACE_LOST_KHR),
        ))
    });

    assert_eq!(runtime.state_for_test(), RuntimeState::Attached);
    assert!(
        context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );
    assert!(matches!(
        runtime.poll_fault(),
        Err(AshViewportError::SurfaceUnsupported(
            SurfaceSupportError::CapabilitiesQuery(ash::vk::Result::ERROR_SURFACE_LOST_KHR)
        ))
    ));

    unsafe { renderer_probe_runtime_sys() };
    assert_eq!(runtime.callback_probe_count_for_test(), 1);
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn renderer_fault_queue_preserves_observation_order_and_first_terminal_fault() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();
    let control = runtime.control_for_test();

    control.record_fault(AshViewportError::CallbackReentered {
        callback: "earlier recoverable fault",
    });
    control.record_fault(AshViewportError::CallbackReentered {
        callback: "second recoverable fault",
    });
    control.record_runtime_contract_fault(AshViewportError::RendererCallbackReplaced {
        callback: "terminal contract fault",
    });

    assert!(matches!(
        runtime.poll_fault(),
        Err(AshViewportError::CallbackReentered {
            callback: "earlier recoverable fault"
        })
    ));
    assert!(matches!(
        runtime.poll_fault(),
        Err(AshViewportError::CallbackReentered {
            callback: "second recoverable fault"
        })
    ));
    assert!(matches!(
        runtime.poll_fault(),
        Err(AshViewportError::RendererCallbackReplaced {
            callback: "terminal contract fault"
        })
    ));
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn direct_trampoline_skips_remaining_callback_after_another_slot_drifts() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();
    let viewport = unsafe { sys::igGetMainViewport() };
    assert!(!viewport.is_null());

    unsafe {
        context
            .platform_io_mut()
            .set_renderer_destroy_window_raw(Some(foreign_renderer_unary));
        renderer_render_window_sys(viewport, std::ptr::null_mut());
    }

    assert_eq!(
        runtime.callback_probe_count_for_test(),
        0,
        "the surviving render callback must not enter after another slot drifts"
    );
    assert!(matches!(
        runtime.poll_fault(),
        Err(AshViewportError::RendererCallbackReplaced {
            callback: "Renderer_DestroyWindow"
        })
    ));
    assert!(
        !context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );

    unsafe {
        context
            .platform_io_mut()
            .set_renderer_destroy_window_raw(Some(renderer_destroy_window_sys));
    }
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn public_entry_fails_closed_when_renderer_capability_is_lost() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();
    let io = context.io_mut();
    io.set_backend_flags(io.backend_flags() & !BackendFlags::RENDERER_HAS_VIEWPORTS);

    assert!(matches!(
        runtime.poll_fault(),
        Err(AshViewportError::RendererViewportCapabilityLost)
    ));
    assert_eq!(runtime.state_for_test(), RuntimeState::ShuttingDown);
    assert_eq!(runtime.callback_probe_count_for_test(), 0);
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn core_renderer_drift_stops_direct_c_callbacks_and_enters_shutdown() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();
    runtime
        .control_for_test()
        .replace_renderer_contract_for_test("BackendRendererName");

    unsafe { renderer_probe_runtime_sys() };
    assert_eq!(runtime.callback_probe_count_for_test(), 0);
    assert_eq!(runtime.state_for_test(), RuntimeState::ShuttingDown);
    assert!(
        !context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );
    assert!(matches!(
        runtime.poll_fault(),
        Err(AshViewportError::Renderer(
            RendererError::RendererStateReplaced {
                field: "BackendRendererName"
            }
        ))
    ));

    unsafe { renderer_probe_runtime_sys() };
    assert_eq!(runtime.callback_probe_count_for_test(), 0);
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn public_drift_check_clears_only_the_runtime_context_capability() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();
    unsafe {
        context
            .platform_io_mut()
            .set_renderer_destroy_window_raw(Some(foreign_renderer_unary));
    }
    let suspended_context = context.suspend_or_panic();
    let mut other_context = Context::create();
    let io = other_context.io_mut();
    io.set_backend_flags(io.backend_flags() | BackendFlags::RENDERER_HAS_VIEWPORTS);

    assert!(matches!(
        runtime.poll_fault(),
        Err(AshViewportError::RendererCallbackReplaced {
            callback: "Renderer_DestroyWindow"
        })
    ));
    assert!(
        other_context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS),
        "drift cleanup must not mutate the Context restored after validation"
    );

    let suspended_other = other_context.suspend_or_panic();
    let mut context = suspended_context
        .activate()
        .expect("the other Context was suspended");
    assert!(
        !context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );
    unsafe {
        context
            .platform_io_mut()
            .set_renderer_destroy_window_raw(Some(renderer_destroy_window_sys));
    }

    runtime.shutdown(&mut context).unwrap();
    drop(suspended_other);
}

#[test]
fn direct_trampoline_requires_complete_platform_dependencies() {
    let _guard = super::test_context_guard();
    for missing in 0..3 {
        let mut context = Context::create();
        let _platform = attach_test_platform(&mut context);
        let mut runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();
        match missing {
            0 => {
                let io = context.io_mut();
                io.set_backend_flags(io.backend_flags() & !BackendFlags::PLATFORM_HAS_VIEWPORTS);
            }
            1 => unsafe {
                context
                    .platform_io_mut()
                    .set_platform_create_window_raw(None);
            },
            2 => unsafe {
                context
                    .platform_io_mut()
                    .set_platform_destroy_window_raw(None);
            },
            _ => unreachable!(),
        }

        unsafe { renderer_probe_runtime_sys() };
        assert_eq!(
            runtime.callback_probe_count_for_test(),
            0,
            "a C trampoline entered with platform dependency {missing} missing"
        );
        let error = runtime.poll_fault().unwrap_err();
        match missing {
            0 => assert!(matches!(
                error,
                AshViewportError::PlatformBackendUnavailable
            )),
            1 => assert!(matches!(
                error,
                AshViewportError::PlatformCallbackUnavailable {
                    callback: "Platform_CreateWindow"
                }
            )),
            2 => assert!(matches!(
                error,
                AshViewportError::PlatformCallbackUnavailable {
                    callback: "Platform_DestroyWindow"
                }
            )),
            _ => unreachable!(),
        }
        assert!(
            !context
                .io()
                .backend_flags()
                .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
        );
        assert_eq!(runtime.state_for_test(), RuntimeState::ShuttingDown);
        runtime.shutdown(&mut context).unwrap();
    }
}

#[test]
fn renderer_create_failure_reasserts_close_until_destroy_callback() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();
    let viewport = unsafe { sys::igGetMainViewport() };
    assert!(!viewport.is_null());

    unsafe { renderer_create_window_sys(viewport) };
    assert!(unsafe { (*viewport).PlatformRequestClose });

    unsafe { (*viewport).PlatformRequestClose = false };
    assert!(matches!(
        runtime.poll_fault(),
        Err(AshViewportError::RuntimeDetached)
    ));
    assert!(
        unsafe { (*viewport).PlatformRequestClose },
        "polling after UpdatePlatformWindows must reassert a create-failure close request"
    );

    unsafe {
        (*viewport).PlatformRequestClose = false;
        renderer_destroy_window_sys(viewport);
        (*viewport).PlatformRequestClose = false;
    }
    runtime.poll_fault().unwrap();
    assert!(!unsafe { (*viewport).PlatformRequestClose });
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn every_foreign_callback_replacement_is_preserved_during_shutdown() {
    let _guard = super::test_context_guard();
    for (slot, expected_name) in [
        "Renderer_CreateWindow",
        "Renderer_DestroyWindow",
        "Renderer_SetWindowSize",
        "Renderer_RenderWindow",
        "Renderer_SwapBuffers",
    ]
    .into_iter()
    .enumerate()
    {
        let mut context = Context::create();
        let _platform = attach_test_platform(&mut context);
        let mut runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();
        let platform_io = context.platform_io_mut();
        unsafe {
            match slot {
                0 => platform_io.set_renderer_create_window_raw(Some(foreign_renderer_unary)),
                1 => platform_io.set_renderer_destroy_window_raw(Some(foreign_renderer_unary)),
                2 => platform_io
                    .set_renderer_set_window_size_raw(Some(foreign_renderer_set_window_size)),
                3 => platform_io.set_renderer_render_window_raw(Some(foreign_renderer_render)),
                4 => platform_io.set_renderer_swap_buffers_raw(Some(foreign_renderer_render)),
                _ => unreachable!(),
            }
        }

        assert!(matches!(
            runtime.poll_fault(),
            Err(AshViewportError::RendererCallbackReplaced { callback })
                if callback == expected_name
        ));
        assert!(
            !context
                .io()
                .backend_flags()
                .contains(BackendFlags::RENDERER_HAS_VIEWPORTS),
            "callback drift must fail closed before teardown"
        );

        assert!(matches!(
            runtime.shutdown(&mut context),
            Err(AshViewportError::RendererCallbackReplaced { callback })
                if callback == expected_name
        ));
        let platform_io = context.platform_io();
        let preserved = match slot {
            0 => platform_io
                .renderer_create_window_raw()
                .is_some_and(|callback| {
                    std::ptr::fn_addr_eq(
                        callback,
                        foreign_renderer_unary as unsafe extern "C" fn(*mut sys::ImGuiViewport),
                    )
                }),
            1 => platform_io
                .renderer_destroy_window_raw()
                .is_some_and(|callback| {
                    std::ptr::fn_addr_eq(
                        callback,
                        foreign_renderer_unary as unsafe extern "C" fn(*mut sys::ImGuiViewport),
                    )
                }),
            2 => platform_io.renderer_set_window_size_matches_pointer_callback(
                foreign_renderer_set_window_size,
            ),
            3 => platform_io
                .renderer_render_window_raw()
                .is_some_and(|callback| {
                    std::ptr::fn_addr_eq(
                        callback,
                        foreign_renderer_render
                            as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void),
                    )
                }),
            4 => platform_io
                .renderer_swap_buffers_raw()
                .is_some_and(|callback| {
                    std::ptr::fn_addr_eq(
                        callback,
                        foreign_renderer_render
                            as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void),
                    )
                }),
            _ => unreachable!(),
        };
        assert!(preserved, "foreign callback slot {slot} was overwritten");
        unsafe { context.platform_io_mut().clear_renderer_handlers() };
    }
}

#[test]
fn complete_foreign_callback_takeover_preserves_foreign_capability_during_fault_and_release() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();
    let platform_io = context.platform_io_mut();
    unsafe {
        platform_io.set_renderer_create_window_raw(Some(foreign_renderer_unary));
        platform_io.set_renderer_destroy_window_raw(Some(foreign_renderer_unary));
        platform_io.set_renderer_set_window_size_raw(Some(foreign_renderer_set_window_size));
        platform_io.set_renderer_render_window_raw(Some(foreign_renderer_render));
        platform_io.set_renderer_swap_buffers_raw(Some(foreign_renderer_render));
    }

    assert!(matches!(
        runtime.poll_fault(),
        Err(AshViewportError::RendererCallbackReplaced {
            callback: "Renderer_CreateWindow"
        })
    ));
    assert!(
        context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS),
        "a complete foreign callback takeover owns the capability bit"
    );

    assert!(matches!(
        runtime.shutdown(&mut context),
        Err(AshViewportError::RendererCallbackReplaced {
            callback: "Renderer_CreateWindow"
        })
    ));
    assert!(
        context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS),
        "release must not clear a capability no exact Ash publication still owns"
    );
    let platform_io = context.platform_io();
    assert!(
        platform_io
            .renderer_create_window_raw()
            .is_some_and(|callback| {
                std::ptr::fn_addr_eq(
                    callback,
                    foreign_renderer_unary as unsafe extern "C" fn(*mut sys::ImGuiViewport),
                )
            })
    );
    assert!(
        platform_io
            .renderer_destroy_window_raw()
            .is_some_and(|callback| {
                std::ptr::fn_addr_eq(
                    callback,
                    foreign_renderer_unary as unsafe extern "C" fn(*mut sys::ImGuiViewport),
                )
            })
    );
    assert!(
        platform_io
            .renderer_set_window_size_matches_pointer_callback(foreign_renderer_set_window_size)
    );
    assert!(
        platform_io
            .renderer_render_window_raw()
            .is_some_and(|callback| {
                std::ptr::fn_addr_eq(
                    callback,
                    foreign_renderer_render
                        as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void),
                )
            })
    );
    assert!(
        platform_io
            .renderer_swap_buffers_raw()
            .is_some_and(|callback| {
                std::ptr::fn_addr_eq(
                    callback,
                    foreign_renderer_render
                        as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void),
                )
            })
    );
}

#[test]
fn foreign_renderer_user_data_is_reported_without_taking_or_typing_it() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();
    let foreign = 0x1234_usize as *mut c_void;
    let viewport = unsafe { sys::igGetMainViewport() };
    assert!(!viewport.is_null());
    unsafe {
        (*viewport).RendererUserData = foreign;
        renderer_destroy_window_sys(viewport);
    }

    assert_eq!(runtime.state_for_test(), RuntimeState::ShuttingDown);
    assert!(
        !context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );
    assert!(matches!(
        runtime.poll_fault(),
        Err(AshViewportError::RendererUserDataOwnershipLost {
            callback: "Renderer_DestroyWindow"
        })
    ));
    assert_eq!(unsafe { (*viewport).RendererUserData }, foreign);

    unsafe { (*viewport).RendererUserData = std::ptr::null_mut() };
    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn context_first_teardown_releases_renderer_before_platform_and_tombstones_wrapper() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let control_slot = Rc::new(RefCell::new(None));
    let renderer_released_first = Rc::new(Cell::new(false));
    let lease = context
        .register_attachment::<OrderingPlatformMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(OrderingPlatformAttachment {
                control: Rc::clone(&control_slot),
                renderer_released_first: Rc::clone(&renderer_released_first),
            }),
        )
        .unwrap();
    let _platform = TestPlatformLease {
        binding: context.binding(),
        _lease: lease,
    };
    claim_test_platform_callbacks(&mut context);
    let runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();
    control_slot
        .borrow_mut()
        .replace(runtime.control_for_test());

    drop(context);

    assert!(renderer_released_first.get());
    assert_eq!(runtime.state_for_test(), RuntimeState::ResourceDropped);
    assert!(matches!(
        runtime.poll_fault(),
        Err(AshViewportError::RuntimeDetached)
    ));
}

#[test]
fn owned_callback_table_is_complete_after_publish() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();
    let platform_io = context.platform_io();

    assert!(
        platform_io
            .renderer_create_window_raw()
            .is_some_and(|callback| {
                std::ptr::fn_addr_eq(
                    callback,
                    renderer_create_window_sys as unsafe extern "C" fn(*mut sys::ImGuiViewport),
                )
            })
    );
    assert!(
        platform_io
            .renderer_destroy_window_raw()
            .is_some_and(|callback| {
                std::ptr::fn_addr_eq(
                    callback,
                    renderer_destroy_window_sys as unsafe extern "C" fn(*mut sys::ImGuiViewport),
                )
            })
    );
    assert!(
        platform_io
            .renderer_render_window_raw()
            .is_some_and(|callback| {
                std::ptr::fn_addr_eq(
                    callback,
                    renderer_render_window_sys
                        as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void),
                )
            })
    );
    assert!(
        platform_io
            .renderer_swap_buffers_raw()
            .is_some_and(|callback| {
                std::ptr::fn_addr_eq(
                    callback,
                    renderer_swap_buffers_sys
                        as unsafe extern "C" fn(*mut sys::ImGuiViewport, *mut c_void),
                )
            })
    );

    runtime.shutdown(&mut context).unwrap();
    assert!(
        !context
            .io()
            .backend_flags()
            .contains(BackendFlags::RENDERER_HAS_VIEWPORTS)
    );
}

#[test]
fn frame_trace_is_instance_bound_non_nested_and_drop_abortable() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();
    let control = runtime.control_for_test();

    let trace = runtime.begin_frame_trace().unwrap();
    assert!(matches!(
        runtime.begin_frame_trace(),
        Err(AshViewportError::FrameTraceAlreadyActive)
    ));
    let low = Id::from(3_u32);
    let high = Id::from(7_u32);
    control.record_viewport_render_submitted(high);
    control.record_viewport_render_submitted(low);
    control.record_viewport_present_submitted(high);
    let report = trace.finish();
    assert_eq!(report.render_submitted_viewport_ids(), &[low, high]);
    assert_eq!(report.present_submitted_viewport_ids(), &[high]);

    drop(runtime.begin_frame_trace().unwrap());
    let report = runtime.begin_frame_trace().unwrap().finish();
    assert!(report.render_submitted_viewport_ids().is_empty());
    assert!(report.present_submitted_viewport_ids().is_empty());

    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn public_route_rejects_foreign_frame_before_platform_or_renderer_side_effects() {
    let _guard = super::test_context_guard();
    let mut target = Context::create();
    let target_platform = attach_test_platform(&mut target);
    let target_runtime = OwningViewportRuntime::attach_for_test(&mut target).unwrap();
    let expected = target.id();
    target_runtime
        .control_for_test()
        .record_fault(AshViewportError::CallbackReentered {
            callback: "queued renderer fault",
        });

    let suspended_target = target.suspend_or_panic();
    let mut source = Context::create();
    source
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("foreign test Context should own its legacy font atlas")
        .build();
    source.io_mut().set_display_size([128.0, 128.0]);
    source.io_mut().set_delta_time(1.0 / 60.0);
    let actual = source.id();
    let frame = source.begin_frame();
    let platform_scope_called = Cell::new(false);
    let platform_faults = RefCell::new(VecDeque::from([
        std::io::Error::other("queued platform fault one"),
        std::io::Error::other("queued platform fault two"),
    ]));

    // Both public platform routes delegate their Winit/SDL3 entry and deferred-fault drain to
    // this closure. A foreign frame must return before either operation becomes observable.
    let error = match target_runtime.prepare_route_for_context(actual, || {
        platform_scope_called.set(true);
        let faults = platform_faults.borrow_mut().drain(..).collect();
        (None, faults)
    }) {
        Ok(_) => panic!("a foreign frame must not produce a prepared route frame"),
        Err(error) => error,
    };

    assert_eq!(frame.lifecycle_state(), FrameLifecycleState::InFrame);
    assert!(!platform_scope_called.get());
    assert_eq!(platform_faults.borrow().len(), 2);
    assert!(matches!(
        error.faults(),
        [AshViewportRouteFault::Renderer(
            AshViewportError::ContextMismatch {
                expected: error_expected,
                actual: error_actual,
            }
        )] if *error_expected == expected && *error_actual == actual
    ));
    assert!(matches!(
        target_runtime.poll_fault(),
        Err(AshViewportError::CallbackReentered {
            callback: "queued renderer fault"
        })
    ));

    drop(frame);
    drop(source);
    drop(target_runtime);
    drop(target_platform);
    drop(suspended_target);
}

#[test]
fn owning_runtime_rejects_foreign_frame_before_target_fault_or_renderer_entry() {
    let _guard = super::test_context_guard();
    let mut target = Context::create();
    let target_platform = attach_test_platform(&mut target);
    let target_runtime = OwningViewportRuntime::attach_for_test(&mut target).unwrap();
    let expected = target.id();
    target_runtime
        .control_for_test()
        .record_fault(AshViewportError::CallbackPanicked {
            callback: "queued before owning prepare",
        });

    let suspended_target = target.suspend_or_panic();
    let mut source = Context::create();
    source
        .font_atlas()
        .try_claim_legacy_renderer()
        .expect("foreign test Context should own its legacy font atlas")
        .build();
    source.io_mut().set_display_size([128.0, 128.0]);
    source.io_mut().set_delta_time(1.0 / 60.0);
    let actual = source.id();
    let frame = source.begin_frame();

    let error = match target_runtime.prepare(frame) {
        Ok(_) => panic!("a foreign frame must not reach the owning Ash renderer"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        AshViewportError::ContextMismatch {
            expected: error_expected,
            actual: error_actual,
        } if error_expected == expected && error_actual == actual
    ));
    assert!(matches!(
        target_runtime.poll_fault(),
        Err(AshViewportError::CallbackPanicked {
            callback: "queued before owning prepare"
        })
    ));

    drop(source);
    drop(target_runtime);
    drop(target_platform);
    drop(suspended_target);
}

#[test]
fn frame_runtime_identity_mismatch_is_rejected_before_target_faults_are_consumed() {
    let _guard = super::test_context_guard();
    let mut context_a = Context::create();
    let platform_a = attach_test_platform(&mut context_a);
    let runtime_a = OwningViewportRuntime::attach_for_test(&mut context_a).unwrap();
    let expected = context_a.id();
    runtime_a
        .control_for_test()
        .record_fault(AshViewportError::CallbackReentered {
            callback: "queued before runtime validation",
        });

    let suspended_a = context_a.suspend_or_panic();
    let mut context_b = Context::create();
    let platform_b = attach_test_platform(&mut context_b);
    let runtime_b = OwningViewportRuntime::attach_for_test(&mut context_b).unwrap();
    let actual = context_b.id();

    runtime_a
        .ensure_runtime_identity_for_test(&runtime_a)
        .unwrap();
    assert!(matches!(
        runtime_a.ensure_runtime_identity_for_test(&runtime_b),
        Err(AshViewportError::FrameTransactionRuntimeMismatch {
            expected: error_expected,
            actual: error_actual,
        }) if error_expected == expected && error_actual == actual
    ));
    assert!(matches!(
        runtime_a.wait_for_frame_completion(runtime_b.empty_completion_for_test()),
        Err(AshViewportError::FrameTransactionRuntimeMismatch {
            expected: error_expected,
            actual: error_actual,
        }) if error_expected == expected && error_actual == actual
    ));
    assert!(matches!(
        runtime_a.poll_fault(),
        Err(AshViewportError::CallbackReentered {
            callback: "queued before runtime validation"
        })
    ));

    drop(runtime_b);
    drop(platform_b);
    drop(context_b);
    drop(runtime_a);
    drop(platform_a);
    drop(suspended_a);
}

#[test]
fn prepared_transaction_traces_exactly_one_secondary_dispatch_scope() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();
    let control = runtime.control_for_test();
    let low = Id::from(3_u32);
    let high = Id::from(7_u32);

    let (output, report) = runtime
        .trace_secondary_dispatch(|| {
            control.record_viewport_render_submitted(high);
            control.record_viewport_render_submitted(low);
            control.record_viewport_present_submitted(high);
            "secondary complete"
        })
        .unwrap();

    assert_eq!(output, "secondary complete");
    assert_eq!(report.render_submitted_viewport_ids(), &[low, high]);
    assert_eq!(report.present_submitted_viewport_ids(), &[high]);
    assert!(runtime.begin_frame_trace().is_ok());

    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn prepared_transaction_checks_faults_before_and_after_secondary_dispatch() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();
    let control = runtime.control_for_test();
    let dispatched = Cell::new(false);

    control.record_fault(AshViewportError::CallbackReentered {
        callback: "before secondary dispatch",
    });
    assert!(matches!(
        runtime.trace_secondary_dispatch(|| dispatched.set(true)),
        Err(AshViewportError::CallbackReentered {
            callback: "before secondary dispatch"
        })
    ));
    assert!(!dispatched.get());

    assert!(matches!(
        runtime.trace_secondary_dispatch(|| {
            dispatched.set(true);
            control.record_fault(AshViewportError::CallbackPanicked {
                callback: "during secondary dispatch",
            });
        }),
        Err(AshViewportError::CallbackPanicked {
            callback: "during secondary dispatch"
        })
    ));
    assert!(dispatched.get());
    assert!(runtime.begin_frame_trace().is_ok());

    runtime.shutdown(&mut context).unwrap();
}

#[test]
fn route_error_retains_renderer_then_platform_fault_order() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let _platform = attach_test_platform(&mut context);
    let mut runtime = OwningViewportRuntime::attach_for_test(&mut context).unwrap();
    let control = runtime.control_for_test();
    control.record_fault(AshViewportError::CallbackReentered {
        callback: "renderer first",
    });
    control.record_fault(AshViewportError::CallbackPanicked {
        callback: "renderer second",
    });

    let error = match runtime.finish_route_preparation(
        None,
        vec![
            std::io::Error::other("platform first"),
            std::io::Error::other("platform second"),
        ],
    ) {
        Ok(_) => panic!("faulted route must not produce a prepared frame"),
        Err(error) => error,
    };

    assert_eq!(error.faults().len(), 4);
    let faults = error.into_faults();
    assert!(matches!(
        &faults[0],
        AshViewportRouteFault::Renderer(AshViewportError::CallbackReentered {
            callback: "renderer first"
        })
    ));
    assert!(matches!(
        &faults[1],
        AshViewportRouteFault::Renderer(AshViewportError::CallbackPanicked {
            callback: "renderer second"
        })
    ));
    assert!(matches!(
        &faults[2],
        AshViewportRouteFault::Platform(error) if error.to_string() == "platform first"
    ));
    assert!(matches!(
        &faults[3],
        AshViewportRouteFault::Platform(error) if error.to_string() == "platform second"
    ));

    runtime.shutdown(&mut context).unwrap();
}

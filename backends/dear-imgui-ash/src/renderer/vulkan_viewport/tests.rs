use std::cell::Cell;
use std::ffi::c_void;
use std::rc::Rc;

use super::callbacks::{
    advance_acquired_frame_recovery, publish_registered_box,
    publish_registered_box_transactionally, recover_acquired_step, validate_secondary_viewports,
};
use super::registry::{
    ViewportIdentity, fail_next_viewport_registration, preflight_registered_viewport_data,
    register_viewport_data, resolve_viewport, take_viewport_data_from_viewport,
    unregister_viewport_data, validate_queue_family_selection, validate_swapchain_image_usage,
    validate_vulkan_handles, viewport_data_count, viewport_user_data_mut,
};
use super::*;
use ash::vk::Handle;

#[test]
fn foreign_renderer_user_data_preflight_is_transactional() {
    let foreign = 0x1234_usize as *mut c_void;

    assert!(matches!(
        validate_secondary_viewports(&[(false, std::ptr::null_mut()), (false, foreign)]),
        Err(AshViewportError::RendererUserDataOccupied)
    ));
}

#[test]
fn existing_platform_windows_preflight_is_transactional() {
    assert!(matches!(
        validate_secondary_viewports(&[
            (false, std::ptr::null_mut()),
            (true, std::ptr::null_mut())
        ]),
        Err(AshViewportError::PlatformWindowsAlreadyCreated)
    ));
}

#[test]
fn invalid_vulkan_handles_and_queue_families_are_rejected() {
    let physical_device = vk::PhysicalDevice::from_raw(1);
    let present_queue = vk::Queue::from_raw(2);

    assert!(matches!(
        validate_vulkan_handles(vk::PhysicalDevice::null(), present_queue),
        Err(AshViewportError::NullPhysicalDevice)
    ));
    assert!(matches!(
        validate_vulkan_handles(physical_device, vk::Queue::null()),
        Err(AshViewportError::NullPresentQueue)
    ));

    let queue_families = [vk::QueueFamilyProperties {
        queue_flags: vk::QueueFlags::COMPUTE,
        queue_count: 1,
        ..Default::default()
    }];
    assert!(matches!(
        validate_queue_family_selection(&queue_families, 0, 0),
        Err(AshViewportError::GraphicsQueueFamilyUnsupported {
            queue_family_index: 0
        })
    ));
    assert!(matches!(
        validate_queue_family_selection(&queue_families, 1, 0),
        Err(AshViewportError::GraphicsQueueFamilyOutOfRange {
            queue_family_index: 1,
            queue_family_count: 1
        })
    ));
}

#[test]
fn swapchain_image_usage_always_requires_color_attachment_and_validates_extras() {
    let supported = vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC;

    assert_eq!(
        validate_swapchain_image_usage(supported, vk::ImageUsageFlags::empty()),
        Ok(vk::ImageUsageFlags::COLOR_ATTACHMENT)
    );
    assert_eq!(
        validate_swapchain_image_usage(supported, vk::ImageUsageFlags::TRANSFER_SRC),
        Ok(supported)
    );
    assert!(matches!(
        validate_swapchain_image_usage(supported, vk::ImageUsageFlags::STORAGE),
        Err(SurfaceSupportError::ImageUsageUnsupported {
            required,
            supported: actual,
        }) if required == vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::STORAGE
            && actual == supported
    ));
}

struct DropProbe(Rc<Cell<usize>>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn failed_registered_box_publish_returns_ownership_without_publication() {
    let drops = Rc::new(Cell::new(0));
    let published = Cell::new(false);
    let result = publish_registered_box(
        Box::new(DropProbe(Rc::clone(&drops))),
        |_pointer| {
            Err(AshViewportError::InvalidCallbackArgument {
                callback: "injected registration failure",
            })
        },
        |_pointer| published.set(true),
    );

    let (error, owner) = result.expect_err("registration must fail");
    assert!(matches!(
        error,
        AshViewportError::InvalidCallbackArgument { .. }
    ));
    assert!(!published.get());
    assert_eq!(drops.get(), 0);
    drop(owner);
    assert_eq!(drops.get(), 1);
}

#[derive(Default)]
struct ResourceReleaseCounts {
    surface: Cell<usize>,
    swapchain: Cell<usize>,
    command_pool: Cell<usize>,
    fences: Cell<usize>,
}

struct ResourceReleaseProbe {
    counts: Rc<ResourceReleaseCounts>,
    fence_count: usize,
}

impl ResourceReleaseProbe {
    fn destroy_after_device_idle(self) {
        self.counts.surface.set(self.counts.surface.get() + 1);
        self.counts.swapchain.set(self.counts.swapchain.get() + 1);
        self.counts
            .command_pool
            .set(self.counts.command_pool.get() + 1);
        self.counts
            .fences
            .set(self.counts.fences.get() + self.fence_count);
    }
}

#[test]
fn registration_failure_cleans_every_viewport_resource_category() {
    let counts = Rc::new(ResourceReleaseCounts::default());
    let published = Cell::new(false);
    let result = publish_registered_box_transactionally(
        Box::new(ResourceReleaseProbe {
            counts: Rc::clone(&counts),
            fence_count: 3,
        }),
        |_pointer| {
            Err(AshViewportError::InvalidCallbackArgument {
                callback: "injected registration failure",
            })
        },
        |_pointer| published.set(true),
        |probe| {
            (*probe).destroy_after_device_idle();
            Ok(())
        },
    );

    assert!(matches!(
        result,
        Err(AshViewportError::InvalidCallbackArgument { .. })
    ));
    assert!(!published.get());
    assert_eq!(counts.surface.get(), 1);
    assert_eq!(counts.swapchain.get(), 1);
    assert_eq!(counts.command_pool.get(), 1);
    assert_eq!(counts.fences.get(), 3);
}

#[test]
fn injected_viewport_registration_failure_publishes_no_sidecar() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let binding = context.binding();
    let identity = ViewportIdentity::from_viewport(context.as_raw(), context.main_viewport());
    let pointer = std::ptr::NonNull::<ViewportAshData>::dangling().as_ptr();
    fail_next_viewport_registration();

    assert!(matches!(
        register_viewport_data(&binding, identity, pointer),
        Err(AshViewportError::InvalidCallbackArgument {
            callback: "injected RendererUserData registration"
        })
    ));
    assert_eq!(viewport_data_count(context.id()), 0);
}

#[test]
fn viewport_identity_resolver_ignores_public_snapshot_and_mutable_id() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let context_raw = context.as_raw();
    let (identity, raw) = {
        let viewport = context.main_viewport();
        (
            ViewportIdentity::from_viewport(context_raw, viewport),
            viewport.as_raw_mut(),
        )
    };

    // Hidden viewports are absent from the public PlatformIO list. The resolver only requires
    // Dear ImGui's internal ID lookup, so an omitted public entry cannot prevent cleanup.
    let public_viewports: [*mut sys::ImGuiViewport; 0] = [];
    assert!(
        !public_viewports
            .iter()
            .any(|viewport| std::ptr::eq(*viewport, raw))
    );
    assert_eq!(resolve_viewport(identity), Some(raw));
    let original_id = unsafe { (*raw).ID };
    let changed_id = original_id.wrapping_add(1);
    unsafe { (*raw).ID = changed_id };
    let resolved_after_id_change = resolve_viewport(identity);
    unsafe { (*raw).ID = original_id };
    assert_eq!(resolved_after_id_change, Some(raw));

    let replaced_address = ViewportIdentity {
        address: identity
            .address
            .wrapping_add(std::mem::align_of::<sys::ImGuiViewport>()),
        ..identity
    };
    assert_eq!(resolve_viewport(replaced_address), None);
}

#[test]
fn preflight_rejects_foreign_sidecar_filtered_from_public_snapshot() {
    let _guard = super::test_context_guard();
    let mut context = Context::create();
    let binding = context.binding();
    let identity = ViewportIdentity::from_viewport(context.as_raw(), context.main_viewport());
    let viewport = context.main_viewport().as_raw_mut();
    let pointer = std::ptr::NonNull::<ViewportAshData>::dangling().as_ptr();
    register_viewport_data(&binding, identity, pointer).unwrap();
    let foreign = std::ptr::dangling_mut::<std::ffi::c_void>();
    unsafe { (*viewport).RendererUserData = foreign };
    let original_id = unsafe { (*viewport).ID };
    unsafe { (*viewport).ID = original_id.wrapping_add(1) };

    let platform_io = context.platform_io_mut().as_raw_mut();
    let original_size = unsafe { (*platform_io).Viewports.Size };
    // The full internal lookup remains live while the public presentation omits this viewport.
    unsafe { (*platform_io).Viewports.Size = 0 };
    let result = preflight_registered_viewport_data(context.as_raw(), &binding);
    unsafe { (*platform_io).Viewports.Size = original_size };
    unsafe { (*viewport).ID = original_id };

    assert!(matches!(
        result,
        Err(AshViewportError::RendererUserDataOwnershipLost {
            callback: "viewport runtime shutdown"
        })
    ));
    assert_eq!(viewport_data_count(context.id()), 1);
    assert_eq!(unsafe { (*viewport).RendererUserData }, foreign);

    unsafe { (*viewport).RendererUserData = std::ptr::null_mut() };
    unregister_viewport_data(pointer);
}

#[test]
fn foreign_renderer_user_data_is_never_typed_or_taken() {
    let _guard = super::test_context_guard();
    let context = Context::create();
    let foreign = 0x1234_usize as *mut c_void;
    let mut raw_viewport = sys::ImGuiViewport {
        RendererUserData: foreign,
        ..Default::default()
    };
    let viewport = unsafe { Viewport::from_raw_mut(&mut raw_viewport) };

    assert!(unsafe { viewport_user_data_mut(context.as_raw(), viewport) }.is_none());
    assert!(unsafe { take_viewport_data_from_viewport(context.as_raw(), viewport) }.is_none());
    assert_eq!(viewport.renderer_user_data(), foreign);
}

#[test]
fn every_acquired_frame_error_runs_recovery_before_returning() {
    let recovery_count = Cell::new(0);
    let result = recover_acquired_step::<()>(
        Err(RendererError::Init(
            "injected acquired-frame failure".into(),
        )),
        || {
            recovery_count.set(recovery_count.get() + 1);
            Ok(())
        },
    );

    assert!(matches!(result, Err(AshViewportError::Renderer(_))));
    assert_eq!(recovery_count.get(), 1);
}

#[test]
fn acquired_frame_error_preserves_recovery_failure() {
    let result = recover_acquired_step::<()>(
        Err(RendererError::Init(
            "injected acquired-frame failure".into(),
        )),
        || {
            Err(AshViewportError::InvalidCallbackArgument {
                callback: "injected recovery failure",
            })
        },
    );

    assert!(matches!(
        result,
        Err(AshViewportError::InvalidCallbackArgument {
            callback: "injected recovery failure"
        })
    ));
}

#[test]
fn failed_idle_keeps_acquired_frame_poisoned_until_sync_replacement() {
    use crate::renderer::lifecycle::DeviceIdleOutcome;

    let mut state = ViewportRuntimeState::Active;
    assert!(state.begin_acquire(2));

    let replacement_count = Cell::new(0);
    let first = advance_acquired_frame_recovery(
        &mut state,
        || {
            Err(AshViewportError::DeviceCompletionFailed {
                operation: "injected acquire recovery",
                source: vk::Result::ERROR_OUT_OF_HOST_MEMORY,
            })
        },
        |_| {
            replacement_count.set(replacement_count.get() + 1);
            Ok(())
        },
    );

    assert!(matches!(
        first,
        Err(AshViewportError::DeviceCompletionFailed {
            operation: "injected acquire recovery",
            source: vk::Result::ERROR_OUT_OF_HOST_MEMORY,
        })
    ));
    assert_eq!(
        state,
        ViewportRuntimeState::AcquireRecoveryRequired { frame_index: 2 }
    );
    assert!(!state.can_acquire());
    assert_eq!(replacement_count.get(), 0);

    // Observing the callback fault must not make the poisoned frame reusable. The next renderer
    // entry retries recovery before another acquire and only then permits a swapchain rebuild.
    let second = advance_acquired_frame_recovery(
        &mut state,
        || Ok(DeviceIdleOutcome::Complete),
        |frame_index| {
            assert_eq!(frame_index, 2);
            replacement_count.set(replacement_count.get() + 1);
            Ok(())
        },
    );

    assert_eq!(second.unwrap(), DeviceIdleOutcome::Complete);
    assert_eq!(state, ViewportRuntimeState::RebuildRequired);
    assert!(!state.can_acquire());
    assert_eq!(replacement_count.get(), 1);
}

#[test]
fn failed_sync_replacement_keeps_acquired_frame_poisoned() {
    use crate::renderer::lifecycle::DeviceIdleOutcome;

    let mut state = ViewportRuntimeState::Active;
    assert!(state.begin_acquire(1));

    let result = advance_acquired_frame_recovery(
        &mut state,
        || Ok(DeviceIdleOutcome::Complete),
        |_| {
            Err(AshViewportError::InvalidCallbackArgument {
                callback: "injected frame-sync replacement",
            })
        },
    );

    assert!(matches!(
        result,
        Err(AshViewportError::InvalidCallbackArgument {
            callback: "injected frame-sync replacement"
        })
    ));
    assert_eq!(
        state,
        ViewportRuntimeState::AcquireRecoveryRequired { frame_index: 1 }
    );
    assert!(!state.can_acquire());
}

#[test]
fn device_loss_terminally_fails_acquired_frame_recovery() {
    use crate::renderer::lifecycle::DeviceIdleOutcome;

    let mut state = ViewportRuntimeState::Active;
    assert!(state.begin_acquire(0));
    let replacement_count = Cell::new(0);

    let result = advance_acquired_frame_recovery(
        &mut state,
        || Ok(DeviceIdleOutcome::DeviceLost),
        |_| {
            replacement_count.set(replacement_count.get() + 1);
            Ok(())
        },
    );

    assert_eq!(result.unwrap(), DeviceIdleOutcome::DeviceLost);
    assert_eq!(state, ViewportRuntimeState::Failed);
    assert!(!state.can_acquire());
    assert_eq!(replacement_count.get(), 0);
}

#[test]
fn only_active_viewport_state_can_acquire() {
    assert!(ViewportRuntimeState::Active.can_acquire());
    assert!(!ViewportRuntimeState::Paused.can_acquire());
    assert!(!ViewportRuntimeState::RebuildRequired.can_acquire());
    assert!(!ViewportRuntimeState::AcquireRecoveryRequired { frame_index: 0 }.can_acquire());
    assert!(!ViewportRuntimeState::Failed.can_acquire());
}

#[test]
fn present_semaphores_are_selected_by_acquired_image() {
    let image_zero = vk::Semaphore::from_raw(11);
    let image_one = vk::Semaphore::from_raw(22);
    let semaphores = [image_zero, image_one];

    assert_eq!(
        present_semaphore_for_image(&semaphores, 0),
        Some(image_zero)
    );
    assert_eq!(present_semaphore_for_image(&semaphores, 1), Some(image_one));
    assert_eq!(present_semaphore_for_image(&semaphores, 2), None);
}

#[test]
fn zero_extent_pauses_and_variable_extent_is_clamped() {
    assert_eq!(
        swapchain::desired_extent_from_size_and_scale([0.0, 24.0], [1.0, 1.0]),
        None
    );
    assert_eq!(
        swapchain::desired_extent_from_size_and_scale([12.0, 8.0], [2.0, 1.5]),
        Some(vk::Extent2D {
            width: 24,
            height: 12,
        })
    );

    let capabilities = vk::SurfaceCapabilitiesKHR {
        current_extent: vk::Extent2D {
            width: u32::MAX,
            height: u32::MAX,
        },
        min_image_extent: vk::Extent2D {
            width: 64,
            height: 48,
        },
        max_image_extent: vk::Extent2D {
            width: 1920,
            height: 1080,
        },
        ..Default::default()
    };
    assert_eq!(
        swapchain::select_swapchain_extent(
            &capabilities,
            Some(vk::Extent2D {
                width: 16,
                height: 4096,
            })
        ),
        Some(vk::Extent2D {
            width: 64,
            height: 1080,
        })
    );
    assert_eq!(
        swapchain::select_swapchain_extent(&capabilities, None),
        None
    );
}

#[test]
fn dpi_only_framebuffer_extent_change_requires_swapchain_rebuild() {
    let logical_size = [640.0, 480.0];
    let previous = swapchain::desired_extent_from_size_and_scale(logical_size, [1.0, 1.0]);
    let scaled = swapchain::desired_extent_from_size_and_scale(logical_size, [1.5, 1.5]);

    assert!(swapchain::extent_request_changed(previous, scaled));
    assert!(!swapchain::extent_request_changed(scaled, scaled));
}

#[test]
fn auto_no_vsync_prefers_immediate_over_fifo() {
    let modes = [vk::PresentModeKHR::FIFO, vk::PresentModeKHR::IMMEDIATE];

    assert_eq!(
        swapchain::resolve_present_mode(PresentModePolicy::AutoNoVsync, &modes),
        Ok(vk::PresentModeKHR::IMMEDIATE)
    );
}

#[test]
fn auto_no_vsync_prefers_mailbox_over_fifo() {
    let modes = [vk::PresentModeKHR::FIFO, vk::PresentModeKHR::MAILBOX];

    assert_eq!(
        swapchain::resolve_present_mode(PresentModePolicy::AutoNoVsync, &modes),
        Ok(vk::PresentModeKHR::MAILBOX)
    );
}

#[test]
fn auto_no_vsync_safely_falls_back_to_fifo() {
    assert_eq!(
        swapchain::resolve_present_mode(
            PresentModePolicy::AutoNoVsync,
            &[vk::PresentModeKHR::FIFO]
        ),
        Ok(vk::PresentModeKHR::FIFO)
    );
}

#[test]
fn auto_vsync_prefers_fifo_relaxed_then_fifo() {
    assert_eq!(
        swapchain::resolve_present_mode(
            PresentModePolicy::AutoVsync,
            &[vk::PresentModeKHR::FIFO, vk::PresentModeKHR::FIFO_RELAXED]
        ),
        Ok(vk::PresentModeKHR::FIFO_RELAXED)
    );
    assert_eq!(
        swapchain::resolve_present_mode(PresentModePolicy::AutoVsync, &[vk::PresentModeKHR::FIFO]),
        Ok(vk::PresentModeKHR::FIFO)
    );
}

#[test]
fn unsupported_exact_present_mode_is_rejected() {
    assert_eq!(
        swapchain::resolve_present_mode(
            PresentModePolicy::Exact(vk::PresentModeKHR::IMMEDIATE),
            &[vk::PresentModeKHR::FIFO]
        ),
        Err(SurfaceSupportError::PresentModeUnsupported {
            requested: vk::PresentModeKHR::IMMEDIATE,
        })
    );
}

#[test]
fn automatic_srgb_selection_matches_the_complete_surface_pair() {
    let hdr = vk::SurfaceFormatKHR {
        format: vk::Format::B8G8R8A8_SRGB,
        color_space: vk::ColorSpaceKHR::HDR10_ST2084_EXT,
    };
    let srgb = vk::SurfaceFormatKHR {
        format: vk::Format::B8G8R8A8_SRGB,
        color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
    };

    assert_eq!(
        swapchain::resolve_surface_format(SurfaceFormatPolicy::AutoSrgb, &[hdr, srgb]),
        Ok(srgb)
    );
}

#[test]
fn undefined_surface_format_sentinel_resolves_to_an_srgb_pair() {
    let undefined = vk::SurfaceFormatKHR {
        format: vk::Format::UNDEFINED,
        color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
    };

    assert_eq!(
        swapchain::resolve_surface_format(SurfaceFormatPolicy::AutoSrgb, &[undefined]),
        Ok(vk::SurfaceFormatKHR {
            format: vk::Format::B8G8R8A8_SRGB,
            color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
        })
    );
}

#[test]
fn undefined_surface_format_sentinel_preserves_its_color_space() {
    let undefined_hdr = vk::SurfaceFormatKHR {
        format: vk::Format::UNDEFINED,
        color_space: vk::ColorSpaceKHR::HDR10_ST2084_EXT,
    };
    let requested = vk::SurfaceFormatKHR {
        format: vk::Format::A2B10G10R10_UNORM_PACK32,
        color_space: vk::ColorSpaceKHR::HDR10_ST2084_EXT,
    };

    assert_eq!(
        swapchain::resolve_surface_format(SurfaceFormatPolicy::Exact(requested), &[undefined_hdr]),
        Ok(requested)
    );
    assert_eq!(
        swapchain::resolve_surface_format(SurfaceFormatPolicy::AutoSrgb, &[undefined_hdr]),
        Err(SurfaceSupportError::SrgbSurfaceFormatUnsupported)
    );
}

#[test]
fn exact_surface_policy_rejects_undefined_as_a_swapchain_format() {
    let undefined = vk::SurfaceFormatKHR {
        format: vk::Format::UNDEFINED,
        color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
    };

    assert_eq!(
        swapchain::resolve_surface_format(SurfaceFormatPolicy::Exact(undefined), &[undefined]),
        Err(SurfaceSupportError::SurfaceFormatUnsupported {
            requested: undefined,
        })
    );
}

#[test]
fn main_surface_policy_copies_the_pair_and_vsync_intent() {
    let pair = vk::SurfaceFormatKHR {
        format: vk::Format::B8G8R8A8_SRGB,
        color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
    };

    assert_eq!(
        ViewportSwapchainPolicy::from_main_surface(pair, vk::PresentModeKHR::FIFO),
        ViewportSwapchainPolicy {
            surface_format: SurfaceFormatPolicy::Exact(pair),
            present_mode: PresentModePolicy::AutoVsync,
        }
    );
    assert_eq!(
        ViewportSwapchainPolicy::from_main_surface(pair, vk::PresentModeKHR::IMMEDIATE),
        ViewportSwapchainPolicy {
            surface_format: SurfaceFormatPolicy::Exact(pair),
            present_mode: PresentModePolicy::AutoNoVsync,
        }
    );
}

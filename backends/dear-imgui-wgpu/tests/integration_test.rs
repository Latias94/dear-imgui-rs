//! Integration tests for the WGPU backend improvements
//!
//! This test verifies that our improvements maintain compatibility with the C++ implementation

use dear_imgui_rs::{
    BackendFlags, Condition, Context, ManagedTextureId, TextureId,
    texture::{OwnedTextureData, TextureFormat as ImGuiTextureFormat},
};
use dear_imgui_wgpu::wgpu::*;
use dear_imgui_wgpu::{
    ExternalTextureId, RendererError, RendererResult, WgpuInitInfo, WgpuRenderState,
    WgpuRenderStateAccessError, WgpuRenderer,
};
use static_assertions::assert_not_impl_any;
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

fn request_test_device() -> Option<(Device, Queue)> {
    #[cfg(any(feature = "wgpu-27", feature = "wgpu-28"))]
    let instance = Instance::new(&InstanceDescriptor::default());
    #[cfg(any(feature = "wgpu-29", feature = "wgpu-30"))]
    let instance = Instance::new(InstanceDescriptor::new_without_display_handle());
    let request_adapter = |force_fallback_adapter| {
        pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::LowPower,
            force_fallback_adapter,
            ..Default::default()
        }))
    };
    let adapter = request_adapter(true)
        .or_else(|_| request_adapter(false))
        .ok()?;
    pollster::block_on(adapter.request_device(&DeviceDescriptor::default())).ok()
}

fn external_test_texture(device: &Device, label: &'static str) -> (Texture, TextureView) {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some(label),
        size: Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());
    (texture, view)
}

#[test]
fn renderer_is_bound_to_the_context_ui_thread() {
    assert_not_impl_any!(WgpuRenderer: Send, Sync, Default);
    assert_not_impl_any!(WgpuRenderState<'static>: Send, Sync);
}

static CALLBACK_STEP: AtomicUsize = AtomicUsize::new(0);
static CALLBACK_STATE_LIVE: AtomicBool = AtomicBool::new(false);
static CALLBACK_NESTED_BORROW_REJECTED: AtomicBool = AtomicBool::new(false);

fn callback_test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap()
}

unsafe extern "C" fn observe_callback_a(
    _draw_list: *const dear_imgui_rs::sys::ImDrawList,
    _command: *const dear_imgui_rs::sys::ImDrawCmd,
) {
    if CALLBACK_STEP
        .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        CALLBACK_STEP.store(usize::MAX, Ordering::SeqCst);
    }
    let access = unsafe {
        WgpuRenderState::with_current(|mut state| {
            let nested = WgpuRenderState::with_current(|_| ());
            CALLBACK_NESTED_BORROW_REJECTED.store(
                matches!(nested, Err(WgpuRenderStateAccessError::AlreadyBorrowed)),
                Ordering::SeqCst,
            );
            let (device, render_pass) = state.resources();
            let _ = device.features();
            render_pass.set_viewport(0.0, 0.0, 1.0, 1.0, 0.0, 1.0);
        })
    };
    CALLBACK_STATE_LIVE.store(access.is_ok(), Ordering::SeqCst);
}

unsafe extern "C" fn observe_callback_b(
    _draw_list: *const dear_imgui_rs::sys::ImDrawList,
    _command: *const dear_imgui_rs::sys::ImDrawCmd,
) {
    if CALLBACK_STEP
        .compare_exchange(1, 2, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        CALLBACK_STEP.store(usize::MAX, Ordering::SeqCst);
    }
    let access = unsafe {
        WgpuRenderState::with_current(|mut state| {
            let _ = state.device().limits();
            state
                .render_pass()
                .set_viewport(0.0, 0.0, 64.0, 64.0, 0.0, 1.0);
        })
    };
    CALLBACK_STATE_LIVE.fetch_and(access.is_ok(), Ordering::SeqCst);
}

fn render_callback_contract_frame(
    renderer: &mut WgpuRenderer,
    context: &mut Context,
    device: &Device,
    queue: &Queue,
    explicit_extent: bool,
    include_geometry: bool,
) -> RendererResult<()> {
    CALLBACK_STEP.store(0, Ordering::SeqCst);
    CALLBACK_STATE_LIVE.store(false, Ordering::SeqCst);
    CALLBACK_NESTED_BORROW_REJECTED.store(false, Ordering::SeqCst);

    context.io_mut().set_display_size([64.0, 64.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    let reset = context
        .platform_io()
        .draw_callback_reset_render_state_raw()
        .expect("WGPU must publish its reset callback");
    let nearest = context
        .platform_io()
        .draw_callback_set_sampler_nearest_raw()
        .expect("WGPU must publish its nearest-sampler callback");
    let linear = context
        .platform_io()
        .draw_callback_set_sampler_linear_raw()
        .expect("WGPU must publish its linear-sampler callback");
    {
        let ui = context.frame();
        let draw_list = ui.get_foreground_draw_list();
        unsafe {
            draw_list.add_callback(observe_callback_a, std::ptr::null_mut(), 0);
            draw_list.add_callback(reset, std::ptr::null_mut(), 0);
            draw_list.add_callback(nearest, std::ptr::null_mut(), 0);
        }
        if include_geometry {
            draw_list
                .add_rect([8.0, 8.0], [48.0, 48.0], dear_imgui_rs::Color::WHITE)
                .filled(true)
                .build();
        }
        unsafe {
            draw_list.add_callback(linear, std::ptr::null_mut(), 0);
            draw_list.add_callback(observe_callback_b, std::ptr::null_mut(), 0);
        }
    }

    let target = device.create_texture(&TextureDescriptor {
        label: Some("dear-imgui-wgpu callback contract target"),
        size: Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("dear-imgui-wgpu callback contract encoder"),
    });
    {
        let color_attachments = [Some(RenderPassColorAttachment {
            view: &view,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Clear(Color::BLACK),
                store: StoreOp::Store,
            },
            depth_slice: None,
        })];
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("dear-imgui-wgpu callback contract pass"),
            color_attachments: &color_attachments,
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            #[cfg(any(feature = "wgpu-28", feature = "wgpu-29", feature = "wgpu-30"))]
            multiview_mask: None,
            timestamp_writes: None,
        });
        let frame = context.render();
        if !include_geometry {
            assert_eq!(frame.draw_data().total_vtx_count(), 0);
            assert_eq!(frame.draw_data().total_idx_count(), 0);
            assert_eq!(
                frame
                    .draw_data()
                    .draw_lists()
                    .flat_map(|list| list.commands())
                    .filter(|command| {
                        matches!(command, dear_imgui_rs::render::DrawCmd::RawCallback(_))
                    })
                    .count(),
                2
            );
        }
        if explicit_extent {
            renderer.render_with_fb_size(frame, &mut render_pass, 64, 64)?;
        } else {
            renderer.render(frame, &mut render_pass)?;
        }
    }
    queue.submit([encoder.finish()]);

    assert_eq!(CALLBACK_STEP.load(Ordering::SeqCst), 2);
    assert!(CALLBACK_STATE_LIVE.load(Ordering::SeqCst));
    assert!(CALLBACK_NESTED_BORROW_REJECTED.load(Ordering::SeqCst));
    assert!(unsafe { context.platform_io().renderer_render_state() }.is_null());
    assert!(matches!(
        unsafe { WgpuRenderState::with_current(|_| ()) },
        Err(WgpuRenderStateAccessError::Inactive)
    ));
    Ok(())
}

#[test]
fn direct_and_explicit_render_paths_execute_raw_callbacks_with_scoped_state() -> RendererResult<()>
{
    let _guard = callback_test_guard();
    let Some((device, queue)) = request_test_device() else {
        assert!(
            std::env::var_os("DEAR_IMGUI_REQUIRE_WGPU_ADAPTER").is_none(),
            "the WGPU callback contract gate requires a working headless adapter"
        );
        eprintln!("skipping WGPU callback test because no headless adapter is available");
        return Ok(());
    };
    let mut context = Context::create();
    let mut renderer = WgpuRenderer::new(
        WgpuInitInfo::new(device.clone(), queue.clone(), TextureFormat::Rgba8Unorm),
        &mut context,
    )?;

    render_callback_contract_frame(&mut renderer, &mut context, &device, &queue, false, false)?;
    render_callback_contract_frame(&mut renderer, &mut context, &device, &queue, true, false)?;
    render_callback_contract_frame(&mut renderer, &mut context, &device, &queue, false, true)?;
    render_callback_contract_frame(&mut renderer, &mut context, &device, &queue, true, true)?;
    renderer.shutdown(&mut context)
}

#[test]
fn render_opens_its_resource_arena_from_the_rendered_frame_epoch() -> RendererResult<()> {
    let _guard = callback_test_guard();
    let Some((device, queue)) = request_test_device() else {
        return Ok(());
    };
    let mut context = Context::create();
    let mut renderer = WgpuRenderer::new(
        WgpuInitInfo::new(device.clone(), queue.clone(), TextureFormat::Rgba8Unorm),
        &mut context,
    )?;
    context.io_mut().set_display_size([64.0, 64.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context.frame().text("frame without renderer preparation");

    let target = device.create_texture(&TextureDescriptor {
        label: Some("dear-imgui-wgpu frame contract target"),
        size: Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor::default());
    let color_attachments = [Some(RenderPassColorAttachment {
        view: &view,
        resolve_target: None,
        ops: Operations {
            load: LoadOp::Clear(Color::BLACK),
            store: StoreOp::Store,
        },
        depth_slice: None,
    })];
    let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
        label: Some("dear-imgui-wgpu frame contract pass"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        #[cfg(any(feature = "wgpu-28", feature = "wgpu-29", feature = "wgpu-30"))]
        multiview_mask: None,
        timestamp_writes: None,
    });
    renderer.render(context.render(), &mut render_pass)?;
    drop(render_pass);
    queue.submit([encoder.finish()]);

    renderer.shutdown(&mut context)
}

fn render_test_frame(
    renderer: &mut WgpuRenderer,
    context: &mut Context,
    device: &Device,
    queue: &Queue,
    managed_texture: Option<ManagedTextureId>,
    legacy_texture: Option<TextureId>,
) -> RendererResult<()> {
    context.io_mut().set_display_size([64.0, 64.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    let ui = context.frame();
    ui.window("device lifecycle test")
        .position([0.0, 0.0], Condition::Always)
        .size([64.0, 64.0], Condition::Always)
        .build(|| ui.text("rebuild"));
    if let Some(texture) = managed_texture {
        ui.image(texture, [16.0, 16.0]);
    }
    if let Some(texture) = legacy_texture {
        ui.image(texture, [16.0, 16.0]);
    }

    let target = device.create_texture(&TextureDescriptor {
        label: Some("dear-imgui-wgpu lifecycle test target"),
        size: Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("dear-imgui-wgpu lifecycle test encoder"),
    });
    {
        let color_attachments = [Some(RenderPassColorAttachment {
            view: &view,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Clear(Color::BLACK),
                store: StoreOp::Store,
            },
            depth_slice: None,
        })];
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("dear-imgui-wgpu lifecycle test pass"),
            color_attachments: &color_attachments,
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            #[cfg(any(feature = "wgpu-28", feature = "wgpu-29", feature = "wgpu-30"))]
            multiview_mask: None,
            timestamp_writes: None,
        });
        renderer.render_context(context, &mut render_pass)?;
    }
    queue.submit([encoder.finish()]);
    Ok(())
}

fn render_context_without_open_frame(
    renderer: &mut WgpuRenderer,
    context: &mut Context,
    device: &Device,
) -> RendererResult<()> {
    let target = device.create_texture(&TextureDescriptor {
        label: Some("dear-imgui-wgpu context identity test target"),
        size: Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("dear-imgui-wgpu context identity test encoder"),
    });
    let color_attachments = [Some(RenderPassColorAttachment {
        view: &view,
        resolve_target: None,
        ops: Operations {
            load: LoadOp::Clear(Color::BLACK),
            store: StoreOp::Store,
        },
        depth_slice: None,
    })];
    let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
        label: Some("dear-imgui-wgpu context identity test pass"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        #[cfg(any(feature = "wgpu-28", feature = "wgpu-29", feature = "wgpu-30"))]
        multiview_mask: None,
        timestamp_writes: None,
    });
    renderer.render_context(context, &mut render_pass)
}

#[test]
fn device_objects_rebind_after_invalidation_and_shutdown() -> RendererResult<()> {
    let Some((device, queue)) = request_test_device() else {
        eprintln!("skipping WGPU lifecycle test because no headless adapter is available");
        return Ok(());
    };
    let format = TextureFormat::Rgba8Unorm;
    let mut context = Context::create();
    let mut renderer = WgpuRenderer::new(
        WgpuInitInfo::new(device.clone(), queue.clone(), format),
        &mut context,
    )?;

    let (_external_texture, external_view) =
        external_test_texture(&device, "dear-imgui-wgpu external lifecycle texture");
    let external = renderer.register_external_texture(&external_view)?;

    render_test_frame(
        &mut renderer,
        &mut context,
        &device,
        &queue,
        None,
        Some(external.texture_id()),
    )?;
    let first_texture_id = context.font_atlas().texture_id();
    assert!(!first_texture_id.is_null());

    renderer.invalidate_device_objects(&mut context)?;
    assert!(context.font_atlas().texture_id().is_null());

    let (_replacement_texture, replacement_view) =
        external_test_texture(&device, "dear-imgui-wgpu replacement external texture");
    renderer.update_external_texture(external, &replacement_view)?;
    render_test_frame(
        &mut renderer,
        &mut context,
        &device,
        &queue,
        None,
        Some(external.texture_id()),
    )?;
    renderer.unregister_external_texture(external)?;
    assert!(matches!(
        renderer.unregister_external_texture(external),
        Err(RendererError::ExternalTextureNotFound(id)) if id == external.texture_id()
    ));
    let recreated_texture_id = context.font_atlas().texture_id();
    assert!(!recreated_texture_id.is_null());
    assert_ne!(recreated_texture_id, first_texture_id);

    let suspended_owner = context.suspend();
    let mut foreign_context = Context::create();
    let foreign_flags = foreign_context.io().backend_flags();
    assert!(matches!(
        renderer.invalidate_device_objects(&mut foreign_context),
        Err(RendererError::ContextMismatch)
    ));
    assert!(matches!(
        renderer.shutdown(&mut foreign_context),
        Err(RendererError::ContextMismatch)
    ));
    assert!(matches!(
        render_context_without_open_frame(&mut renderer, &mut foreign_context, &device),
        Err(RendererError::ContextMismatch)
    ));
    #[cfg(feature = "multi-viewport-winit")]
    {
        let failure = unsafe {
            dear_imgui_wgpu::multi_viewport::WinitViewportRuntime::attach_unchecked(
                &mut foreign_context,
                renderer,
            )
        }
        .unwrap_err();
        assert!(matches!(
            failure.error(),
            dear_imgui_wgpu::multi_viewport::WgpuViewportError::RendererContextMismatch
        ));
        renderer = failure.into_renderer();
    }
    #[cfg(feature = "multi-viewport-sdl3")]
    {
        let failure = unsafe {
            dear_imgui_wgpu::multi_viewport_sdl3::Sdl3ViewportRuntime::attach_unchecked(
                &mut foreign_context,
                renderer,
            )
        }
        .unwrap_err();
        assert!(matches!(
            failure.error(),
            dear_imgui_wgpu::multi_viewport_sdl3::WgpuViewportError::RendererContextMismatch
        ));
        renderer = failure.into_renderer();
    }
    assert_eq!(foreign_context.io().backend_flags(), foreign_flags);

    let suspended_foreign = foreign_context.suspend();
    let mut context = suspended_owner
        .activate()
        .expect("renderer owner context should reactivate");
    render_test_frame(&mut renderer, &mut context, &device, &queue, None, None)?;

    renderer.shutdown(&mut context)?;
    assert!(
        !context.io().backend_flags().intersects(
            BackendFlags::RENDERER_HAS_TEXTURES | BackendFlags::RENDERER_HAS_VTX_OFFSET
        )
    );
    assert!(context.font_atlas().texture_id().is_null());
    drop(renderer);

    let mut replacement = WgpuRenderer::new(
        WgpuInitInfo::new(device.clone(), queue.clone(), format),
        &mut context,
    )?;
    render_test_frame(&mut replacement, &mut context, &device, &queue, None, None)?;
    replacement.shutdown(&mut context)?;
    drop(replacement);

    let suspended_owner = context.suspend();
    let mut context = suspended_foreign
        .activate()
        .expect("replacement context should activate after owner shutdown");
    drop(suspended_owner);

    let mut foreign_renderer = WgpuRenderer::new(
        WgpuInitInfo::new(device.clone(), queue.clone(), format),
        &mut context,
    )?;
    render_test_frame(
        &mut foreign_renderer,
        &mut context,
        &device,
        &queue,
        None,
        None,
    )?;
    let reinitialized_texture_id = context.font_atlas().texture_id();
    assert!(!reinitialized_texture_id.is_null());
    foreign_renderer.shutdown(&mut context)?;

    Ok(())
}

#[test]
fn rendered_frame_reconciles_managed_lifecycle_and_preserves_external_views() -> RendererResult<()>
{
    let Some((device, queue)) = request_test_device() else {
        eprintln!("skipping WGPU rendered-frame test because no headless adapter is available");
        return Ok(());
    };
    let mut context = Context::create();
    let mut renderer = WgpuRenderer::new(
        WgpuInitInfo::new(device.clone(), queue.clone(), TextureFormat::Rgba8Unorm),
        &mut context,
    )?;

    let mut managed_data = OwnedTextureData::new();
    managed_data.create(ImGuiTextureFormat::RGBA32, 2, 2);
    managed_data.set_data(&[7; 16]);
    let managed = context.register_texture(managed_data);

    let external_texture = device.create_texture(&TextureDescriptor {
        label: Some("dear-imgui-wgpu external render test"),
        size: Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let external_view = external_texture.create_view(&TextureViewDescriptor::default());
    let external = renderer.register_external_texture(&external_view)?;
    let external_id = external.texture_id();

    render_test_frame(
        &mut renderer,
        &mut context,
        &device,
        &queue,
        Some(managed),
        Some(external_id),
    )?;
    let first_managed_id = context
        .with_texture(managed, |texture| texture.texture_id())
        .expect("managed texture should remain active");
    assert!(!first_managed_id.is_null());

    context
        .with_texture_mut(managed, |mut texture| texture.set_data(&[11; 16]))
        .expect("managed texture update should be accepted");
    let replacement_view = external_texture.create_view(&TextureViewDescriptor::default());
    renderer.update_external_texture(external, &replacement_view)?;
    render_test_frame(
        &mut renderer,
        &mut context,
        &device,
        &queue,
        Some(managed),
        Some(external_id),
    )?;
    assert_eq!(
        context
            .with_texture(managed, |texture| texture.texture_id())
            .expect("updated texture should remain active"),
        first_managed_id,
        "updates must keep the renderer-facing texture ID stable"
    );

    context
        .remove_texture(managed)
        .expect("managed texture should begin retirement");
    render_test_frame(
        &mut renderer,
        &mut context,
        &device,
        &queue,
        None,
        Some(external_id),
    )?;

    renderer.unregister_external_texture(external)?;
    assert!(matches!(
        renderer.unregister_external_texture(external),
        Err(RendererError::ExternalTextureNotFound(id)) if id == external_id
    ));

    renderer.shutdown(&mut context)?;
    Ok(())
}

/// Public texture registration exposes a renderer-issued opaque handle rather than internals.
#[test]
fn texture_id_api_boundaries_are_typed() {
    assert_not_impl_any!(ExternalTextureId: From<TextureId>);
    let _: fn(ExternalTextureId) -> TextureId = ExternalTextureId::texture_id;
    let _: for<'a, 'b> fn(
        &'a mut WgpuRenderer,
        &'b TextureView,
    ) -> RendererResult<ExternalTextureId> = WgpuRenderer::register_external_texture;
    let _: for<'a, 'b> fn(
        &'a mut WgpuRenderer,
        ExternalTextureId,
        &'b TextureView,
    ) -> RendererResult<()> = WgpuRenderer::update_external_texture;
    let _: fn(&mut WgpuRenderer, ExternalTextureId) -> RendererResult<()> =
        WgpuRenderer::unregister_external_texture;
}

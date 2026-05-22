use super::*;
use std::cell::UnsafeCell;

fn new_platform_io() -> sys::ImGuiPlatformIO {
    unsafe {
        let p = sys::ImGuiPlatformIO_ImGuiPlatformIO();
        assert!(
            !p.is_null(),
            "ImGuiPlatformIO_ImGuiPlatformIO() returned null"
        );
        let v = *p;
        sys::ImGuiPlatformIO_destroy(p);
        v
    }
}

unsafe extern "C" fn draw_callback_marker(
    _parent_list: *const sys::ImDrawList,
    _cmd: *const sys::ImDrawCmd,
) {
}

#[test]
fn platform_io_textures_empty_is_safe() {
    let mut raw: sys::ImGuiPlatformIO = new_platform_io();

    raw.Textures.Size = 0;
    raw.Textures.Data = std::ptr::null_mut();
    let mut pio = PlatformIo {
        raw: UnsafeCell::new(raw),
    };
    assert_eq!(pio.textures().count(), 0);
    assert!(pio.textures_mut().next().is_none());
    assert_eq!(pio.textures_count(), 0);

    let mut raw: sys::ImGuiPlatformIO = new_platform_io();
    raw.Textures.Size = 1;
    raw.Textures.Data = std::ptr::null_mut();
    let mut pio = PlatformIo {
        raw: UnsafeCell::new(raw),
    };
    assert_eq!(pio.textures().count(), 0);
    assert!(pio.textures_mut().next().is_none());
    assert_eq!(pio.textures_count(), 0);
    assert!(pio.texture(0).is_none());
}

#[test]
fn apply_texture_feedback_matches_typed_managed_texture_ids() {
    let mut texture = crate::texture::TextureData::new();
    unsafe {
        (*texture.as_raw_mut()).UniqueID = 314;
    }
    texture.set_status(crate::texture::TextureStatus::WantCreate);
    let id = texture.unique_id();

    let mut texture_ptr = texture.as_mut().as_raw_mut();
    let mut raw: sys::ImGuiPlatformIO = new_platform_io();
    raw.Textures.Size = 1;
    raw.Textures.Capacity = 1;
    raw.Textures.Data = &mut texture_ptr;

    let mut pio = PlatformIo {
        raw: UnsafeCell::new(raw),
    };
    let applied =
        pio.apply_texture_feedback(&[crate::render::snapshot::TextureFeedback::with_tex_id(
            id,
            crate::texture::TextureStatus::OK,
            crate::texture::TextureId::new(99),
        )]);

    assert_eq!(applied, 1);
    assert_eq!(texture.status(), crate::texture::TextureStatus::OK);
    assert_eq!(texture.tex_id(), crate::texture::TextureId::new(99));
}

#[test]
fn apply_texture_feedback_can_set_backend_user_data_and_clear_on_destroy() {
    let mut texture = crate::texture::TextureData::new();
    unsafe {
        (*texture.as_raw_mut()).UniqueID = 2718;
    }
    texture.set_status(crate::texture::TextureStatus::WantCreate);
    let id = texture.unique_id();

    let mut texture_ptr = texture.as_mut().as_raw_mut();
    let mut raw: sys::ImGuiPlatformIO = new_platform_io();
    raw.Textures.Size = 1;
    raw.Textures.Capacity = 1;
    raw.Textures.Data = &mut texture_ptr;

    let mut pio = PlatformIo {
        raw: UnsafeCell::new(raw),
    };
    let applied =
        pio.apply_texture_feedback(&[crate::render::snapshot::TextureFeedback::with_tex_id(
            id,
            crate::texture::TextureStatus::OK,
            crate::texture::TextureId::new(123),
        )
        .backend_user_data(0xCAFE)]);

    assert_eq!(applied, 1);
    assert_eq!(texture.status(), crate::texture::TextureStatus::OK);
    assert_eq!(texture.tex_id(), crate::texture::TextureId::new(123));
    assert_eq!(texture.backend_user_data() as usize, 0xCAFE);

    let applied = pio.apply_texture_feedback(&[crate::render::snapshot::TextureFeedback::status(
        id,
        crate::texture::TextureStatus::Destroyed,
    )]);

    assert_eq!(applied, 1);
    assert_eq!(texture.status(), crate::texture::TextureStatus::Destroyed);
    assert!(texture.tex_id().is_null());
    assert!(texture.backend_user_data().is_null());
}

#[test]
fn platform_io_from_raw_matches_mut_wrapper() {
    let mut raw: sys::ImGuiPlatformIO = new_platform_io();
    let raw_ptr = (&mut raw) as *mut sys::ImGuiPlatformIO;

    let shared = unsafe { PlatformIo::from_raw(raw_ptr.cast_const()) };
    let mutable = unsafe { PlatformIo::from_raw_mut(raw_ptr) };

    assert_eq!(shared.as_raw(), mutable.as_raw());
}

#[test]
fn platform_io_standard_draw_callback_accessors_roundtrip() {
    let mut raw: sys::ImGuiPlatformIO = new_platform_io();
    let pio = unsafe { PlatformIo::from_raw_mut((&mut raw) as *mut sys::ImGuiPlatformIO) };

    pio.set_draw_callback_reset_render_state_raw(Some(draw_callback_marker));
    pio.set_draw_callback_set_sampler_linear_raw(Some(draw_callback_marker));
    pio.set_draw_callback_set_sampler_nearest_raw(Some(draw_callback_marker));

    assert_eq!(
        pio.draw_callback_reset_render_state_raw()
            .map(|f| f as usize),
        Some(draw_callback_marker as usize)
    );
    assert_eq!(
        pio.draw_callback_set_sampler_linear_raw()
            .map(|f| f as usize),
        Some(draw_callback_marker as usize)
    );
    assert_eq!(
        pio.draw_callback_set_sampler_nearest_raw()
            .map(|f| f as usize),
        Some(draw_callback_marker as usize)
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn platform_io_clear_handlers_resets_platform_and_renderer_callbacks() {
    unsafe extern "C" fn platform_cb(_viewport: *mut sys::ImGuiViewport) {}
    unsafe extern "C" fn renderer_cb(_viewport: *mut sys::ImGuiViewport) {}
    unsafe extern "C" fn platform_dpi_scale_cb(_viewport: *mut sys::ImGuiViewport) -> f32 {
        1.0
    }

    let mut raw: sys::ImGuiPlatformIO = new_platform_io();
    raw.Platform_CreateWindow = Some(platform_cb);
    raw.Platform_DestroyWindow = Some(platform_cb);
    raw.Platform_GetWindowDpiScale = Some(platform_dpi_scale_cb);
    raw.Platform_OnChangedViewport = Some(platform_cb);
    raw.Renderer_CreateWindow = Some(renderer_cb);
    raw.Renderer_DestroyWindow = Some(renderer_cb);

    let pio = unsafe { PlatformIo::from_raw_mut((&mut raw) as *mut sys::ImGuiPlatformIO) };
    pio.clear_platform_handlers();
    pio.clear_renderer_handlers();

    assert!(raw.Platform_CreateWindow.is_none());
    assert!(raw.Platform_DestroyWindow.is_none());
    assert!(raw.Platform_GetWindowDpiScale.is_none());
    assert!(raw.Platform_OnChangedViewport.is_none());
    assert!(raw.Renderer_CreateWindow.is_none());
    assert!(raw.Renderer_DestroyWindow.is_none());
}

#[cfg(feature = "multi-viewport")]
#[test]
fn clear_platform_handlers_clears_typed_get_window_callbacks() {
    unsafe extern "C" fn get_pos(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = sys::ImVec2 { x: 41.0, y: 42.0 };
        }
    }
    unsafe extern "C" fn get_size(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = sys::ImVec2 { x: 43.0, y: 44.0 };
        }
    }
    unsafe extern "C" fn get_scale(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = sys::ImVec2 { x: 45.0, y: 46.0 };
        }
    }
    unsafe extern "C" fn get_insets(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec4) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = sys::ImVec4::new(1.0, 2.0, 3.0, 4.0);
        }
    }

    let mut ctx = crate::Context::create();
    let pio = ctx.platform_io_mut();
    pio.set_platform_get_window_pos_raw(Some(get_pos));
    pio.set_platform_get_window_size_raw(Some(get_size));
    pio.set_platform_get_window_framebuffer_scale_raw(Some(get_scale));
    pio.set_platform_get_window_work_area_insets_raw(Some(get_insets));
    pio.clear_platform_handlers();

    let raw = unsafe { &*pio.as_raw() };
    assert!(raw.Platform_GetWindowPos.is_none());
    assert!(raw.Platform_GetWindowSize.is_none());
    assert!(raw.Platform_GetWindowFramebufferScale.is_none());
    assert!(raw.Platform_GetWindowWorkAreaInsets.is_none());

    let viewport = std::ptr::NonNull::<sys::ImGuiViewport>::dangling().as_ptr();
    let mut pos = sys::ImVec2 { x: 1.0, y: 1.0 };
    let mut size = sys::ImVec2 { x: 1.0, y: 1.0 };
    let mut scale = sys::ImVec2 { x: 2.0, y: 2.0 };
    let mut insets = sys::ImVec4::new(9.0, 9.0, 9.0, 9.0);
    unsafe {
        trampolines::platform_get_window_pos_out(viewport, &mut pos);
        trampolines::platform_get_window_size_out(viewport, &mut size);
        trampolines::platform_get_window_framebuffer_scale_out(viewport, &mut scale);
        trampolines::platform_get_window_work_area_insets_out(viewport, &mut insets);
    }

    assert_eq!((pos.x, pos.y), (0.0, 0.0));
    assert_eq!((size.x, size.y), (0.0, 0.0));
    assert_eq!((scale.x, scale.y), (1.0, 1.0));
    assert_eq!(
        (insets.x, insets.y, insets.z, insets.w),
        (0.0, 0.0, 0.0, 0.0)
    );
}

#[cfg(feature = "multi-viewport")]
#[test]
fn get_window_pos_and_size_callbacks_are_context_local() {
    unsafe extern "C" fn get_pos_a(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = sys::ImVec2 { x: 11.0, y: 12.0 };
        }
    }
    unsafe extern "C" fn get_size_a(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = sys::ImVec2 { x: 13.0, y: 14.0 };
        }
    }
    unsafe extern "C" fn get_pos_b(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = sys::ImVec2 { x: 21.0, y: 22.0 };
        }
    }
    unsafe extern "C" fn get_size_b(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = sys::ImVec2 { x: 23.0, y: 24.0 };
        }
    }
    unsafe extern "C" fn get_scale_a(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = sys::ImVec2 { x: 1.0, y: 2.0 };
        }
    }
    unsafe extern "C" fn get_scale_b(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = sys::ImVec2 { x: 3.0, y: 4.0 };
        }
    }
    unsafe extern "C" fn get_insets_a(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec4) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = sys::ImVec4::new(1.0, 2.0, 3.0, 4.0);
        }
    }
    unsafe extern "C" fn get_insets_b(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec4) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = sys::ImVec4::new(5.0, 6.0, 7.0, 8.0);
        }
    }

    let mut ctx_a = crate::Context::create();
    let language_user_data_a = std::ptr::NonNull::<u8>::dangling().as_ptr().cast();
    ctx_a
        .io_mut()
        .set_backend_language_user_data(language_user_data_a);
    ctx_a
        .platform_io_mut()
        .set_platform_get_window_pos_raw(Some(get_pos_a));
    ctx_a
        .platform_io_mut()
        .set_platform_get_window_size_raw(Some(get_size_a));
    ctx_a
        .platform_io_mut()
        .set_platform_get_window_framebuffer_scale_raw(Some(get_scale_a));
    ctx_a
        .platform_io_mut()
        .set_platform_get_window_work_area_insets_raw(Some(get_insets_a));
    assert_eq!(
        ctx_a.io().backend_language_user_data(),
        language_user_data_a
    );

    let suspended_a = ctx_a.suspend();

    let mut ctx_b = crate::Context::create();
    let language_user_data_b = std::ptr::NonNull::<u16>::dangling().as_ptr().cast();
    ctx_b
        .io_mut()
        .set_backend_language_user_data(language_user_data_b);
    ctx_b
        .platform_io_mut()
        .set_platform_get_window_pos_raw(Some(get_pos_b));
    ctx_b
        .platform_io_mut()
        .set_platform_get_window_size_raw(Some(get_size_b));
    ctx_b
        .platform_io_mut()
        .set_platform_get_window_framebuffer_scale_raw(Some(get_scale_b));
    ctx_b
        .platform_io_mut()
        .set_platform_get_window_work_area_insets_raw(Some(get_insets_b));
    assert_eq!(
        ctx_b.io().backend_language_user_data(),
        language_user_data_b
    );

    let mut b_pos = sys::ImVec2 { x: 0.0, y: 0.0 };
    let mut b_size = sys::ImVec2 { x: 0.0, y: 0.0 };
    let mut b_scale = sys::ImVec2 { x: 0.0, y: 0.0 };
    let mut b_insets = sys::ImVec4::new(9.0, 9.0, 9.0, 9.0);
    unsafe {
        trampolines::platform_get_window_pos_out(std::ptr::null_mut(), &mut b_pos);
        trampolines::platform_get_window_size_out(std::ptr::null_mut(), &mut b_size);
        trampolines::platform_get_window_framebuffer_scale_out(std::ptr::null_mut(), &mut b_scale);
        trampolines::platform_get_window_work_area_insets_out(std::ptr::null_mut(), &mut b_insets);
    }
    assert_eq!((b_pos.x, b_pos.y), (0.0, 0.0));
    assert_eq!((b_size.x, b_size.y), (0.0, 0.0));
    assert_eq!((b_scale.x, b_scale.y), (1.0, 1.0));
    assert_eq!(
        (b_insets.x, b_insets.y, b_insets.z, b_insets.w),
        (0.0, 0.0, 0.0, 0.0)
    );

    let viewport_b = std::ptr::NonNull::<sys::ImGuiViewport>::dangling().as_ptr();
    unsafe {
        trampolines::platform_get_window_pos_out(viewport_b, &mut b_pos);
        trampolines::platform_get_window_size_out(viewport_b, &mut b_size);
        trampolines::platform_get_window_framebuffer_scale_out(viewport_b, &mut b_scale);
        trampolines::platform_get_window_work_area_insets_out(viewport_b, &mut b_insets);
    }
    assert_eq!((b_pos.x, b_pos.y), (21.0, 22.0));
    assert_eq!((b_size.x, b_size.y), (23.0, 24.0));
    assert_eq!((b_scale.x, b_scale.y), (3.0, 4.0));
    assert_eq!(
        (b_insets.x, b_insets.y, b_insets.z, b_insets.w),
        (5.0, 6.0, 7.0, 8.0)
    );

    let suspended_b = ctx_b.suspend();
    let ctx_a = suspended_a.activate().expect("ctx_a should activate");

    let mut a_pos = sys::ImVec2 { x: 0.0, y: 0.0 };
    let mut a_size = sys::ImVec2 { x: 0.0, y: 0.0 };
    let mut a_scale = sys::ImVec2 { x: 0.0, y: 0.0 };
    let mut a_insets = sys::ImVec4::zero();
    let viewport_a = std::ptr::NonNull::<sys::ImGuiViewport>::dangling().as_ptr();
    unsafe {
        trampolines::platform_get_window_pos_out(viewport_a, &mut a_pos);
        trampolines::platform_get_window_size_out(viewport_a, &mut a_size);
        trampolines::platform_get_window_framebuffer_scale_out(viewport_a, &mut a_scale);
        trampolines::platform_get_window_work_area_insets_out(viewport_a, &mut a_insets);
    }
    assert_eq!((a_pos.x, a_pos.y), (11.0, 12.0));
    assert_eq!((a_size.x, a_size.y), (13.0, 14.0));
    assert_eq!((a_scale.x, a_scale.y), (1.0, 2.0));
    assert_eq!(
        (a_insets.x, a_insets.y, a_insets.z, a_insets.w),
        (1.0, 2.0, 3.0, 4.0)
    );

    drop(ctx_a);
    drop(suspended_b);
}

#[cfg(feature = "multi-viewport")]
#[test]
fn typed_callback_setters_reject_non_current_platform_io() {
    unsafe extern "C" fn create_window(_viewport: *mut Viewport) {}

    let mut ctx_a = crate::Context::create();
    let pio_a = ctx_a.platform_io_mut().as_raw_mut();
    let suspended_a = ctx_a.suspend();

    let ctx_b = crate::Context::create();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        PlatformIo::from_raw_mut(pio_a).set_platform_create_window(Some(create_window));
    }));

    assert!(result.is_err());

    drop(ctx_b);
    drop(suspended_a);
}

#[cfg(feature = "multi-viewport")]
#[test]
fn out_param_callback_setters_reject_non_current_platform_io() {
    unsafe extern "C" fn get_pos(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = sys::ImVec2 { x: 41.0, y: 42.0 };
        }
    }

    let mut ctx_a = crate::Context::create();
    let pio_a = ctx_a.platform_io_mut().as_raw_mut();
    let suspended_a = ctx_a.suspend();

    let ctx_b = crate::Context::create();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        PlatformIo::from_raw_mut(pio_a).set_platform_get_window_pos_raw(Some(get_pos));
    }));

    assert!(result.is_err());

    drop(ctx_b);
    drop(suspended_a);
}

#[cfg(feature = "multi-viewport")]
#[test]
fn clear_handlers_target_receiver_platform_io_not_current_context() {
    unsafe extern "C" fn get_pos(_viewport: *mut sys::ImGuiViewport, out: *mut sys::ImVec2) {
        if let Some(out) = unsafe { out.as_mut() } {
            *out = sys::ImVec2 { x: 41.0, y: 42.0 };
        }
    }
    unsafe extern "C" fn create_window(_viewport: *mut Viewport) {}
    unsafe extern "C" fn renderer_window(_viewport: *mut Viewport) {}

    let mut ctx_a = crate::Context::create();
    let raw_a = ctx_a.as_raw();
    let pio_a = ctx_a.platform_io_mut().as_raw_mut();
    ctx_a
        .platform_io_mut()
        .set_platform_get_window_pos_raw(Some(get_pos));
    unsafe {
        ctx_a
            .platform_io_mut()
            .set_platform_create_window(Some(create_window));
        ctx_a
            .platform_io_mut()
            .set_renderer_create_window(Some(renderer_window));
    }
    let suspended_a = ctx_a.suspend();

    let ctx_b = crate::Context::create();
    let raw_b = ctx_b.as_raw();

    unsafe {
        PlatformIo::from_raw_mut(pio_a).clear_platform_handlers();
        PlatformIo::from_raw_mut(pio_a).clear_renderer_handlers();
    }

    unsafe {
        assert_eq!(sys::igGetCurrentContext(), raw_b);

        let raw = &*pio_a;
        assert!(raw.Platform_GetWindowPos.is_none());
        assert!(raw.Platform_CreateWindow.is_none());
        assert!(raw.Renderer_CreateWindow.is_none());

        sys::igSetCurrentContext(raw_a);
    }

    let viewport = std::ptr::NonNull::<sys::ImGuiViewport>::dangling().as_ptr();
    let mut pos = sys::ImVec2 { x: 1.0, y: 1.0 };
    unsafe {
        trampolines::platform_get_window_pos_out(viewport, &mut pos);
        trampolines::platform_create_window(viewport);
        trampolines::renderer_create_window(viewport);
    }

    assert_eq!((pos.x, pos.y), (0.0, 0.0));

    unsafe {
        sys::igSetCurrentContext(raw_b);
    }
    drop(ctx_b);
    drop(suspended_a);
}

#[cfg(feature = "multi-viewport")]
#[test]
fn raw_setters_clear_receiver_typed_callback_slots() {
    unsafe extern "C" fn create_window(_viewport: *mut Viewport) {
        CREATE_WINDOW_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
    unsafe extern "C" fn renderer_window(_viewport: *mut Viewport) {
        RENDERER_WINDOW_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    static CREATE_WINDOW_CALLS: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(0);
    static RENDERER_WINDOW_CALLS: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(0);

    CREATE_WINDOW_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
    RENDERER_WINDOW_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);

    let mut ctx_a = crate::Context::create();
    let raw_a = ctx_a.as_raw();
    let pio_a = ctx_a.platform_io_mut().as_raw_mut();
    unsafe {
        ctx_a
            .platform_io_mut()
            .set_platform_create_window(Some(create_window));
        ctx_a
            .platform_io_mut()
            .set_renderer_create_window(Some(renderer_window));
    }

    let viewport = std::ptr::NonNull::<sys::ImGuiViewport>::dangling().as_ptr();
    unsafe {
        trampolines::platform_create_window(viewport);
        trampolines::renderer_create_window(viewport);
    }
    assert_eq!(
        CREATE_WINDOW_CALLS.load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    assert_eq!(
        RENDERER_WINDOW_CALLS.load(std::sync::atomic::Ordering::SeqCst),
        1
    );

    let suspended_a = ctx_a.suspend();
    let ctx_b = crate::Context::create();
    let raw_b = ctx_b.as_raw();

    unsafe {
        PlatformIo::from_raw_mut(pio_a).set_platform_create_window_raw(None);
        PlatformIo::from_raw_mut(pio_a).set_renderer_create_window_raw(None);
        assert_eq!(sys::igGetCurrentContext(), raw_b);
        sys::igSetCurrentContext(raw_a);
    }

    unsafe {
        trampolines::platform_create_window(viewport);
        trampolines::renderer_create_window(viewport);
    }
    assert_eq!(
        CREATE_WINDOW_CALLS.load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    assert_eq!(
        RENDERER_WINDOW_CALLS.load(std::sync::atomic::Ordering::SeqCst),
        1
    );

    unsafe {
        sys::igSetCurrentContext(raw_b);
    }
    drop(ctx_b);
    drop(suspended_a);
}

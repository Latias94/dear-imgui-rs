use super::*;
use crate::test_util::test_sync::lock_context;

#[test]
fn install_platform_callbacks_targets_passed_context() {
    let _guard = lock_context();

    let mut ctx_a = Context::create();
    let raw_a = ctx_a.as_raw();
    let pio_a = unsafe { dear_imgui_rs::sys::igGetPlatformIO_ContextPtr(raw_a) };

    unsafe {
        dear_imgui_rs::sys::igSetCurrentContext(std::ptr::null_mut());
    }

    let ctx_b = Context::create();
    let raw_b = ctx_b.as_raw();
    let pio_b = unsafe { dear_imgui_rs::sys::igGetPlatformIO_ContextPtr(raw_b) };

    install_platform_callbacks(&mut ctx_a);

    unsafe {
        assert_eq!(dear_imgui_rs::sys::igGetCurrentContext(), raw_b);

        assert!((*pio_a).Platform_CreateWindow.is_some());
        assert!((*pio_a).Platform_GetWindowPos.is_some());
        assert!((*pio_a).Platform_GetWindowSize.is_some());
        assert!((*pio_a).Platform_GetWindowFramebufferScale.is_some());
        assert!((*pio_a).Platform_GetWindowDpiScale.is_some());
        assert!((*pio_a).Platform_OnChangedViewport.is_some());

        assert!((*pio_b).Platform_CreateWindow.is_none());
        assert!((*pio_b).Platform_GetWindowPos.is_none());
        assert!((*pio_b).Platform_GetWindowSize.is_none());
        assert!((*pio_b).Platform_GetWindowFramebufferScale.is_none());
        assert!((*pio_b).Platform_GetWindowDpiScale.is_none());
        assert!((*pio_b).Platform_OnChangedViewport.is_none());

        dear_imgui_rs::sys::igSetCurrentContext(raw_a);
    }
    drop(ctx_a);
    unsafe {
        dear_imgui_rs::sys::igSetCurrentContext(raw_b);
    }
    drop(ctx_b);
}

#[test]
fn shutdown_multi_viewport_support_targets_passed_context() {
    let _guard = lock_context();

    let mut ctx_a = Context::create();
    let raw_a = ctx_a.as_raw();
    let main_viewport_a = unsafe { dear_imgui_rs::sys::igGetMainViewport() };
    let vd_a = unsafe {
        let vd = Box::into_raw(Box::new(ViewportData::new()));
        register_viewport_data(vd);
        (*main_viewport_a).PlatformUserData = vd.cast();
        vd
    };

    unsafe {
        dear_imgui_rs::sys::igSetCurrentContext(std::ptr::null_mut());
    }

    let mut ctx_b = Context::create();
    let raw_b = ctx_b.as_raw();
    let main_viewport_b = unsafe { dear_imgui_rs::sys::igGetMainViewport() };
    let vd_b = unsafe {
        let vd = Box::into_raw(Box::new(ViewportData::new()));
        register_viewport_data(vd);
        (*main_viewport_b).PlatformUserData = vd.cast();
        vd
    };

    shutdown_multi_viewport_support(&mut ctx_a);

    unsafe {
        assert_eq!(dear_imgui_rs::sys::igGetCurrentContext(), raw_b);
        assert!((*main_viewport_a).PlatformUserData.is_null());
        assert!(!(*main_viewport_b).PlatformUserData.is_null());
        assert!(is_winit_viewport_data(vd_b));
        assert!(!std::ptr::eq(
            (*main_viewport_b).PlatformUserData.cast::<ViewportData>(),
            vd_a
        ));
    }

    shutdown_multi_viewport_support(&mut ctx_b);
    unsafe {
        assert_eq!(dear_imgui_rs::sys::igGetCurrentContext(), raw_b);
        assert!((*main_viewport_b).PlatformUserData.is_null());
        assert!(!is_winit_viewport_data(vd_b));
        dear_imgui_rs::sys::igSetCurrentContext(raw_a);
    }
    drop(ctx_a);
    unsafe {
        dear_imgui_rs::sys::igSetCurrentContext(raw_b);
    }
    drop(ctx_b);
}

#[test]
fn window_ptr_for_viewport_targets_passed_context() {
    let _guard = lock_context();

    let ctx_a = Context::create();
    let raw_a = ctx_a.as_raw();
    let main_viewport_a = unsafe { dear_imgui_rs::sys::igGetMainViewport() };
    let window_a = std::ptr::NonNull::<Window>::dangling().as_ptr();
    let vd_a = unsafe {
        let vd = Box::into_raw(Box::new(ViewportData::new()));
        (*vd).window = window_a;
        register_viewport_data(vd);
        (*main_viewport_a).PlatformUserData = vd.cast();
        vd
    };

    unsafe {
        dear_imgui_rs::sys::igSetCurrentContext(std::ptr::null_mut());
    }

    let ctx_b = Context::create();
    let raw_b = ctx_b.as_raw();

    unsafe {
        assert_eq!(
            window_ptr_for_viewport(raw_a, main_viewport_a),
            window_a as *const Window
        );
        assert_eq!(dear_imgui_rs::sys::igGetCurrentContext(), raw_b);

        dear_imgui_rs::sys::igSetCurrentContext(raw_a);
        (*main_viewport_a).PlatformUserData = std::ptr::null_mut();
        drop_viewport_data(vd_a);
    }
    drop(ctx_a);
    unsafe {
        dear_imgui_rs::sys::igSetCurrentContext(raw_b);
    }
    drop(ctx_b);
}

#[test]
fn window_ptr_for_viewport_ignores_foreign_platform_user_data() {
    let _guard = lock_context();

    let ctx = Context::create();
    let raw = ctx.as_raw();
    let main_viewport = unsafe { dear_imgui_rs::sys::igGetMainViewport() };
    unsafe {
        (*main_viewport).PlatformUserData = std::ptr::dangling_mut::<u8>().cast();

        assert!(window_ptr_for_viewport(raw, main_viewport).is_null());

        (*main_viewport).PlatformUserData = std::ptr::null_mut();
    }
}

use super::registry::{
    has_renderer_state_for_context, render_callback_matches, try_install_renderer_callbacks,
    unary_callback_matches,
};
use super::*;
use std::ffi::c_void;
use std::mem::MaybeUninit;
use std::sync::{Mutex as TestMutex, OnceLock};

fn lock_context() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<TestMutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| TestMutex::new(())).lock().unwrap()
}

unsafe extern "C" fn platform_slot_sentinel(
    _viewport: *mut sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
}

unsafe extern "C" fn foreign_renderer_create_window(_viewport: *mut sys::ImGuiViewport) {}

unsafe extern "C" fn foreign_renderer_destroy_window(_viewport: *mut sys::ImGuiViewport) {}

unsafe extern "C" fn foreign_renderer_set_window_size_direct(
    _viewport: *mut sys::ImGuiViewport,
    _size: sys::ImVec2,
) {
}

unsafe extern "C" fn foreign_renderer_set_window_size_pointer(
    _viewport: *mut sys::ImGuiViewport,
    _size: *const sys::ImVec2,
) {
}

unsafe extern "C" fn foreign_renderer_render_window(
    _viewport: *mut sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
}

unsafe extern "C" fn foreign_renderer_swap_buffers(
    _viewport: *mut sys::ImGuiViewport,
    _render_arg: *mut c_void,
) {
}

fn set_window_size_callback_matches(
    actual: Option<unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2)>,
    expected: unsafe extern "C" fn(*mut sys::ImGuiViewport, sys::ImVec2),
) -> bool {
    actual.is_some_and(|actual| std::ptr::fn_addr_eq(actual, expected))
}

fn try_install(ctx: &mut Context) -> Result<(), CallbackOwnershipError> {
    let raw = ctx.as_raw();
    try_install_renderer_callbacks(raw, ctx.platform_io_mut())
}

fn assert_ash_renderer_callbacks(ctx: &Context) {
    let platform_io = ctx.platform_io();
    assert!(unary_callback_matches(
        platform_io.renderer_create_window_raw(),
        renderer_create_window_sys
    ));
    assert!(unary_callback_matches(
        platform_io.renderer_destroy_window_raw(),
        renderer_destroy_window_sys
    ));
    assert!(
        platform_io.renderer_set_window_size_matches_pointer_callback(renderer_set_window_size_sys)
    );
    assert!(render_callback_matches(
        platform_io.renderer_render_window_raw(),
        renderer_render_window_sys
    ));
    assert!(render_callback_matches(
        platform_io.renderer_swap_buffers_raw(),
        renderer_swap_buffers_sys
    ));
}

#[test]
fn renderer_callbacks_preserve_platform_render_slots() {
    let _guard = lock_context();
    let mut ctx = Context::create();
    let raw = ctx.as_raw();
    let platform_io = unsafe { sys::igGetPlatformIO_ContextPtr(raw) };

    unsafe {
        (*platform_io).Platform_RenderWindow = Some(platform_slot_sentinel);
        (*platform_io).Platform_SwapBuffers = Some(platform_slot_sentinel);
    }

    try_install(&mut ctx).expect("empty renderer callback table");

    {
        assert_ash_renderer_callbacks(&ctx);
        unsafe {
            assert!(render_callback_matches(
                (*platform_io).Platform_RenderWindow,
                platform_slot_sentinel
            ));
            assert!(render_callback_matches(
                (*platform_io).Platform_SwapBuffers,
                platform_slot_sentinel
            ));
        }
    }

    disable(&mut ctx);

    unsafe {
        assert!(ctx.platform_io().renderer_callbacks_are_empty());
        assert!(render_callback_matches(
            (*platform_io).Platform_RenderWindow,
            platform_slot_sentinel
        ));
        assert!(render_callback_matches(
            (*platform_io).Platform_SwapBuffers,
            platform_slot_sentinel
        ));

        (*platform_io).Platform_RenderWindow = None;
        (*platform_io).Platform_SwapBuffers = None;
    }
}

#[test]
fn foreign_renderer_callbacks_reject_install_without_mutation() {
    let _guard = lock_context();
    let mut ctx = Context::create();
    let raw = ctx.as_raw();
    let platform_io = unsafe { sys::igGetPlatformIO_ContextPtr(raw) };

    macro_rules! assert_conflict {
        ($field:ident, $callback:ident, $matches:ident) => {{
            unsafe {
                (*platform_io).$field = Some($callback);
            }
            assert_eq!(
                try_install(&mut ctx),
                Err(CallbackOwnershipError::RendererCallbacksOccupied)
            );
            assert!(!has_renderer_state_for_context(raw));
            unsafe {
                assert!($matches((*platform_io).$field, $callback));
                (*platform_io).$field = None;
            }
            assert!(ctx.platform_io().renderer_callbacks_are_empty());
        }};
    }

    assert_conflict!(
        Renderer_CreateWindow,
        foreign_renderer_create_window,
        unary_callback_matches
    );
    assert_conflict!(
        Renderer_DestroyWindow,
        foreign_renderer_destroy_window,
        unary_callback_matches
    );
    assert_conflict!(
        Renderer_SetWindowSize,
        foreign_renderer_set_window_size_direct,
        set_window_size_callback_matches
    );
    assert_conflict!(
        Renderer_RenderWindow,
        foreign_renderer_render_window,
        render_callback_matches
    );
    assert_conflict!(
        Renderer_SwapBuffers,
        foreign_renderer_swap_buffers,
        render_callback_matches
    );
}

#[test]
fn existing_ash_callback_table_can_rebind_renderer_state() {
    let _guard = lock_context();
    let mut ctx = Context::create();
    let raw = ctx.as_raw();
    let mut renderer = MaybeUninit::<AshRenderer>::uninit();

    try_install(&mut ctx).expect("empty renderer callback table");
    upsert_renderer_state(raw, renderer.as_mut_ptr(), None);

    try_install(&mut ctx).expect("existing Ash callback table");
    assert_ash_renderer_callbacks(&ctx);

    disable(&mut ctx);
    assert!(!has_renderer_state_for_context(raw));
}

#[test]
fn disable_preserves_renderer_callbacks_replaced_by_another_backend() {
    let _guard = lock_context();
    let mut ctx = Context::create();
    let raw = ctx.as_raw();
    let mut renderer = MaybeUninit::<AshRenderer>::uninit();

    try_install(&mut ctx).expect("empty renderer callback table");
    upsert_renderer_state(raw, renderer.as_mut_ptr(), None);
    {
        let platform_io = ctx.platform_io_mut();
        platform_io.set_renderer_create_window_raw(Some(foreign_renderer_create_window));
        platform_io.set_renderer_destroy_window_raw(Some(foreign_renderer_destroy_window));
        platform_io
            .set_renderer_set_window_size_raw(Some(foreign_renderer_set_window_size_pointer));
        platform_io.set_renderer_render_window_raw(Some(foreign_renderer_render_window));
        platform_io.set_renderer_swap_buffers_raw(Some(foreign_renderer_swap_buffers));
    }

    disable(&mut ctx);
    assert!(!has_renderer_state_for_context(raw));

    {
        let platform_io = ctx.platform_io();
        assert!(unary_callback_matches(
            platform_io.renderer_create_window_raw(),
            foreign_renderer_create_window
        ));
        assert!(unary_callback_matches(
            platform_io.renderer_destroy_window_raw(),
            foreign_renderer_destroy_window
        ));
        assert!(
            platform_io.renderer_set_window_size_matches_pointer_callback(
                foreign_renderer_set_window_size_pointer
            )
        );
        assert!(render_callback_matches(
            platform_io.renderer_render_window_raw(),
            foreign_renderer_render_window
        ));
        assert!(render_callback_matches(
            platform_io.renderer_swap_buffers_raw(),
            foreign_renderer_swap_buffers
        ));
    }

    let platform_io = ctx.platform_io_mut();
    platform_io.set_renderer_create_window_raw(None);
    platform_io.set_renderer_destroy_window_raw(None);
    platform_io.set_renderer_set_window_size_raw(None);
    platform_io.set_renderer_render_window_raw(None);
    platform_io.set_renderer_swap_buffers_raw(None);
}

#[test]
fn renderer_state_is_context_local() {
    let _guard = lock_context();
    let ctx_a = Context::create();
    let raw_a = ctx_a.as_raw();
    let mut renderer_a = MaybeUninit::<AshRenderer>::uninit();
    let renderer_a_ptr = renderer_a.as_mut_ptr();
    upsert_renderer_state(raw_a, renderer_a_ptr, None);

    unsafe {
        sys::igSetCurrentContext(std::ptr::null_mut());
    }

    let ctx_b = Context::create();
    let raw_b = ctx_b.as_raw();
    let mut renderer_b = MaybeUninit::<AshRenderer>::uninit();
    let renderer_b_ptr = renderer_b.as_mut_ptr();
    upsert_renderer_state(raw_b, renderer_b_ptr, None);

    unsafe {
        sys::igSetCurrentContext(raw_a);
        {
            let borrowed = borrow_renderer().expect("renderer for context A");
            assert_eq!(borrowed.renderer, renderer_a_ptr);
        }

        sys::igSetCurrentContext(raw_b);
        {
            let borrowed = borrow_renderer().expect("renderer for context B");
            assert_eq!(borrowed.renderer, renderer_b_ptr);
        }
    }

    remove_renderer_state_for_context(raw_b);
    unsafe {
        sys::igSetCurrentContext(raw_b);
        assert!(borrow_renderer().is_none());

        sys::igSetCurrentContext(raw_a);
        assert!(borrow_renderer().is_some());
    }

    remove_renderer_state_for_context(raw_a);
    unsafe {
        sys::igSetCurrentContext(raw_a);
    }
    drop(ctx_a);
    unsafe {
        sys::igSetCurrentContext(raw_b);
    }
    drop(ctx_b);
}

#[test]
fn clear_for_drop_removes_renderer_state() {
    let _guard = lock_context();
    let ctx = Context::create();
    let raw = ctx.as_raw();
    let mut renderer = MaybeUninit::<AshRenderer>::uninit();
    let renderer_ptr = renderer.as_mut_ptr();

    upsert_renderer_state(raw, renderer_ptr, None);
    unsafe {
        sys::igSetCurrentContext(raw);
        assert!(borrow_renderer().is_some());
    }

    clear_for_drop(renderer_ptr);
    unsafe {
        sys::igSetCurrentContext(raw);
        assert!(borrow_renderer().is_none());
    }

    drop(ctx);
}

#[test]
fn take_viewport_data_ignores_foreign_renderer_user_data() {
    let _guard = lock_context();
    let mut viewport = sys::ImGuiViewport::default();
    let foreign = 0x1234usize as *mut c_void;
    viewport.RendererUserData = foreign;

    let viewport = unsafe { Viewport::from_raw_mut(&mut viewport) };
    let data = unsafe { take_viewport_data(viewport) };

    assert!(data.is_none());
    assert_eq!(viewport.renderer_user_data(), foreign);
}

#[test]
fn viewport_user_data_mut_ignores_unregistered_renderer_user_data() {
    let _guard = lock_context();
    let mut viewport = sys::ImGuiViewport::default();
    let foreign = 0x1234usize as *mut c_void;
    viewport.RendererUserData = foreign;

    let viewport = unsafe { Viewport::from_raw_mut(&mut viewport) };
    let data = unsafe { viewport_user_data_mut(viewport) };

    assert!(data.is_none());
    assert_eq!(viewport.renderer_user_data(), foreign);
}

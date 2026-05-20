use super::*;
use std::mem::MaybeUninit;

#[test]
fn renderer_state_is_context_local() {
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

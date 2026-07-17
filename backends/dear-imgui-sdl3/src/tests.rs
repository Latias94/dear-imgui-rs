use super::*;
use crate::core::try_with_context;
use std::cell::Cell;

#[test]
fn backend_error_display_is_stable() {
    assert_eq!(
        Sdl3BackendError::InvalidGlslVersion.to_string(),
        "Invalid GLSL version string"
    );
}

#[test]
fn drop_cleanup_rejects_destroyed_context() {
    let binding = {
        let context = Context::create();
        context.binding()
    };
    let called = Cell::new(false);

    assert_eq!(
        try_with_context(&binding, || {
            called.set(true);
            true
        }),
        None
    );
    assert!(!called.get());
}

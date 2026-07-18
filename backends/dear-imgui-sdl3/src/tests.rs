use super::*;
use std::sync::{Mutex, MutexGuard};

static TEST_MUTEX: Mutex<()> = Mutex::new(());

pub(crate) fn test_guard() -> MutexGuard<'static, ()> {
    TEST_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[test]
fn backend_error_display_is_stable() {
    let _guard = test_guard();
    assert_eq!(
        Sdl3BackendError::InvalidGlslVersion.to_string(),
        "Invalid GLSL version string"
    );
}

use super::*;

#[test]
fn backend_error_display_is_stable() {
    assert_eq!(
        Sdl3BackendError::InvalidGlslVersion.to_string(),
        "Invalid GLSL version string"
    );
}

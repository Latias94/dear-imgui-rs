#![cfg(all(
    feature = "abi-probe",
    not(target_arch = "wasm32"),
    dear_imgui_rs_native_symbols
))]

use dear_imgui_sys::ImVec2;

#[repr(C)]
struct PlatformIoInvocationProbeResult {
    platform_set_window_pos: ImVec2,
    platform_set_window_size: ImVec2,
    platform_get_window_pos: ImVec2,
    platform_get_window_size: ImVec2,
    platform_get_window_framebuffer_scale: ImVec2,
    renderer_set_window_size: ImVec2,
}

unsafe extern "C" {
    fn dear_imgui_rs_probe_platform_io_invocation_bridges(
        out_result: *mut PlatformIoInvocationProbeResult,
    ) -> i32;
}

#[test]
fn saved_sdl3_platform_callbacks_cross_the_cpp_invocation_bridges() {
    let mut result = PlatformIoInvocationProbeResult {
        platform_set_window_pos: ImVec2 { x: 0.0, y: 0.0 },
        platform_set_window_size: ImVec2 { x: 0.0, y: 0.0 },
        platform_get_window_pos: ImVec2 { x: 0.0, y: 0.0 },
        platform_get_window_size: ImVec2 { x: 0.0, y: 0.0 },
        platform_get_window_framebuffer_scale: ImVec2 { x: 0.0, y: 0.0 },
        renderer_set_window_size: ImVec2 { x: 0.0, y: 0.0 },
    };

    let invoked = unsafe { dear_imgui_rs_probe_platform_io_invocation_bridges(&mut result) };

    assert_eq!(invoked, 1);
    assert_eq!(
        (
            result.platform_set_window_pos.x,
            result.platform_set_window_pos.y
        ),
        (1.0, 2.0)
    );
    assert_eq!(
        (
            result.platform_set_window_size.x,
            result.platform_set_window_size.y
        ),
        (3.0, 4.0)
    );
    assert_eq!(
        (
            result.platform_get_window_pos.x,
            result.platform_get_window_pos.y
        ),
        (5.0, 6.0)
    );
    assert_eq!(
        (
            result.platform_get_window_size.x,
            result.platform_get_window_size.y
        ),
        (7.0, 8.0)
    );
    assert_eq!(
        (
            result.platform_get_window_framebuffer_scale.x,
            result.platform_get_window_framebuffer_scale.y,
        ),
        (1.5, 2.5)
    );
    assert_eq!(
        (
            result.renderer_set_window_size.x,
            result.renderer_set_window_size.y
        ),
        (9.0, 10.0)
    );
}

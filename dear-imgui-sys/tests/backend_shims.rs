//! Link every advertised shim entry point; cargo check alone cannot catch missing native symbols.
#![cfg(not(target_arch = "wasm32"))]

#[allow(unused_macros)]
macro_rules! linked_symbols {
    ($name:ident, $module:ident, [$($symbol:ident),+ $(,)?]) => {
        #[test]
        fn $name() {
            use dear_imgui_sys::backend_shim::$module::*;
            let symbols = std::hint::black_box([$($symbol as *const ()),+]);
            assert!(symbols.iter().all(|symbol| !symbol.is_null()));
        }
    };
}

#[cfg(all(windows, feature = "backend-shim-dx11"))]
linked_symbols!(
    dx11_symbols_link,
    dx11,
    [
        dear_imgui_backend_dx11_init,
        dear_imgui_backend_dx11_shutdown,
        dear_imgui_backend_dx11_new_frame,
        dear_imgui_backend_dx11_render_draw_data,
        dear_imgui_backend_dx11_create_device_objects,
        dear_imgui_backend_dx11_invalidate_device_objects,
        dear_imgui_backend_dx11_update_texture,
    ]
);

#[cfg(all(windows, feature = "backend-shim-dx12"))]
linked_symbols!(
    dx12_symbols_link,
    dx12,
    [
        dear_imgui_backend_dx12_init,
        dear_imgui_backend_dx12_shutdown,
        dear_imgui_backend_dx12_new_frame,
        dear_imgui_backend_dx12_render_draw_data,
        dear_imgui_backend_dx12_create_device_objects,
        dear_imgui_backend_dx12_invalidate_device_objects,
        dear_imgui_backend_dx12_update_texture,
    ]
);

#[cfg(all(windows, feature = "backend-shim-win32"))]
linked_symbols!(
    win32_symbols_link,
    win32,
    [
        dear_imgui_backend_win32_init,
        dear_imgui_backend_win32_init_for_opengl,
        dear_imgui_backend_win32_shutdown,
        dear_imgui_backend_win32_new_frame,
        dear_imgui_backend_win32_wnd_proc_handler,
        dear_imgui_backend_win32_enable_dpi_awareness,
        dear_imgui_backend_win32_get_dpi_scale_for_hwnd,
        dear_imgui_backend_win32_get_dpi_scale_for_monitor,
        dear_imgui_backend_win32_enable_alpha_compositing,
    ]
);

#[cfg(feature = "backend-shim-opengl3")]
linked_symbols!(
    opengl3_symbols_link,
    opengl3,
    [
        dear_imgui_backend_opengl3_init,
        dear_imgui_backend_opengl3_shutdown,
        dear_imgui_backend_opengl3_new_frame,
        dear_imgui_backend_opengl3_render_draw_data,
        dear_imgui_backend_opengl3_create_device_objects,
        dear_imgui_backend_opengl3_destroy_device_objects,
        dear_imgui_backend_opengl3_update_texture,
    ]
);

#[cfg(all(
    any(target_os = "macos", target_os = "ios"),
    feature = "backend-shim-metal"
))]
linked_symbols!(
    metal_symbols_link,
    metal,
    [
        dear_imgui_backend_metal_init,
        dear_imgui_backend_metal_shutdown,
        dear_imgui_backend_metal_new_frame,
        dear_imgui_backend_metal_render_draw_data,
        dear_imgui_backend_metal_create_device_objects,
        dear_imgui_backend_metal_destroy_device_objects,
        dear_imgui_backend_metal_update_texture,
    ]
);

#[cfg(all(target_os = "macos", feature = "backend-shim-osx"))]
linked_symbols!(
    osx_symbols_link,
    osx,
    [
        dear_imgui_backend_osx_init,
        dear_imgui_backend_osx_shutdown,
        dear_imgui_backend_osx_new_frame,
    ]
);

#[cfg(all(target_os = "android", feature = "backend-shim-android"))]
linked_symbols!(
    android_symbols_link,
    android,
    [
        dear_imgui_backend_android_init,
        dear_imgui_backend_android_shutdown,
        dear_imgui_backend_android_new_frame,
        dear_imgui_backend_android_handle_input_event,
    ]
);

#[cfg(all(windows, feature = "backend-shim-dx12"))]
#[allow(dead_code)]
#[path = "../examples/dx12_headless.rs"]
mod dx12_headless;

#[cfg(all(windows, feature = "backend-shim-dx12"))]
#[test]
fn dx12_warp_renders_and_releases_descriptors() {
    dx12_headless::run().expect("D3D12 WARP renderer smoke failed");
}

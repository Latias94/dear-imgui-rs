//! A raw Win32 + DX11 window. Run with features backend-shim-win32,backend-shim-dx11.
//! The DX12 example separately demonstrates descriptor allocation and explicit command submission.

#[cfg(windows)]
#[path = "support/win32.rs"]
mod native_window;

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use dear_imgui_sys::{
        self as sys,
        backend_shim::{dx11, win32},
    };
    use std::{ptr, time::Instant};
    use windows::{
        Win32::{
            Foundation::*,
            Graphics::{
                Direct3D::*,
                Direct3D11::*,
                Dxgi::{Common::*, *},
            },
        },
        core::{Interface, w},
    };

    struct Context {
        raw: *mut sys::ImGuiContext,
        platform: bool,
        renderer: bool,
    }
    impl Drop for Context {
        fn drop(&mut self) {
            // SAFETY: this guard owns the raw context; the window and DX11 objects outlive it.
            unsafe {
                sys::igSetCurrentContext(self.raw);
                if self.renderer {
                    dx11::dear_imgui_backend_dx11_shutdown();
                }
                if self.platform {
                    win32::dear_imgui_backend_win32_shutdown();
                }
                sys::igDestroyContext(self.raw);
            }
        }
    }
    unsafe fn target(
        device: &ID3D11Device,
        swap: &IDXGISwapChain,
    ) -> windows::core::Result<ID3D11RenderTargetView> {
        // SAFETY: the live swapchain and device share the same DX11 device.
        unsafe {
            let buffer: ID3D11Texture2D = swap.GetBuffer(0)?;
            let mut target = None;
            device.CreateRenderTargetView(&buffer, None, Some(&mut target))?;
            Ok(target.expect("successful render target creation returns a view"))
        }
    }

    // SAFETY: all Win32, Dear ImGui, and DX11 calls run on this thread. Local guards enforce
    // renderer -> platform -> context -> GPU/window teardown, including Result early returns.
    unsafe {
        win32::dear_imgui_backend_win32_enable_dpi_awareness();
        let window = native_window::Window::new(
            w!("DearImGuiSysWin32Dx11"),
            w!("Dear ImGui - raw Win32 / DX11 shims"),
        )?;
        let desc = DXGI_SWAP_CHAIN_DESC {
            BufferDesc: DXGI_MODE_DESC {
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                ..Default::default()
            },
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            OutputWindow: window.0,
            Windowed: windows::core::BOOL(1),
            SwapEffect: DXGI_SWAP_EFFECT_DISCARD,
            ..Default::default()
        };
        let (mut swap, mut device, mut gpu) = (None, None, None);
        D3D11CreateDeviceAndSwapChain(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_FLAG(0),
            None,
            D3D11_SDK_VERSION,
            Some(&desc),
            Some(&mut swap),
            Some(&mut device),
            None,
            Some(&mut gpu),
        )?;
        let swap = swap.expect("successful device creation returns a swapchain");
        let device = device.expect("successful device creation returns a device");
        let gpu = gpu.expect("successful device creation returns an immediate context");
        let mut render_target = Some(target(&device, &swap)?);
        let mut size = (0, 0);

        let raw = sys::igCreateContext(ptr::null_mut());
        assert!(!raw.is_null());
        let mut context = Context {
            raw,
            platform: false,
            renderer: false,
        };
        (*sys::igGetIO_Nil()).IniFilename = ptr::null();
        (*sys::igGetIO_Nil()).ConfigFlags |= sys::ImGuiConfigFlags_NavEnableKeyboard;
        assert!(win32::dear_imgui_backend_win32_init(window.0.0));
        context.platform = true;
        assert!(dx11::dear_imgui_backend_dx11_init(
            device.as_raw(),
            gpu.as_raw()
        ));
        context.renderer = true;
        window.show();
        let limit = std::env::args().find_map(|arg| {
            arg.strip_prefix("--frames=")
                .and_then(|value| value.parse::<u32>().ok())
        });
        let mut frames = 0;
        let mut previous = Instant::now();
        while window.pump_events() {
            let current = window.client_size()?;
            if current.0 == 0 || current.1 == 0 {
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            }
            if size != current {
                gpu.OMSetRenderTargets(None, None);
                drop(render_target.take());
                swap.ResizeBuffers(
                    0,
                    current.0,
                    current.1,
                    DXGI_FORMAT_UNKNOWN,
                    DXGI_SWAP_CHAIN_FLAG(0),
                )?;
                render_target = Some(target(&device, &swap)?);
                size = current;
            }
            let now = Instant::now();
            (*sys::igGetIO_Nil()).DeltaTime = (now - previous).as_secs_f32().max(0.001);
            previous = now;
            dx11::dear_imgui_backend_dx11_new_frame();
            win32::dear_imgui_backend_win32_new_frame();
            sys::igNewFrame();
            if sys::igBegin(c"Native backend shims".as_ptr(), ptr::null_mut(), 0) {
                sys::igTextUnformatted(
                    c"The application owns HWND, DX11, resize, and presentation.".as_ptr(),
                    ptr::null(),
                );
                sys::igTextUnformatted(
                    c"Input and rendering use the official backend C shims.".as_ptr(),
                    ptr::null(),
                );
                sys::igButton(c"Click me".as_ptr(), sys::ImVec2_c { x: 0.0, y: 0.0 });
            }
            sys::igEnd();
            sys::igRender();
            let view = render_target
                .as_ref()
                .expect("non-minimized window has a render target");
            gpu.OMSetRenderTargets(Some(&[Some(view.clone())]), None);
            gpu.ClearRenderTargetView(view, &[0.08, 0.1, 0.14, 1.0]);
            dx11::dear_imgui_backend_dx11_render_draw_data(sys::igGetDrawData());
            swap.Present(1, DXGI_PRESENT(0)).ok()?;
            frames += 1;
            if limit.is_some_and(|limit| frames >= limit) {
                break;
            }
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("win32_dx11 requires Windows");
}

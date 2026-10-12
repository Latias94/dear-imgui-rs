//! A visible raw Win32 / DX12 integration with one serialized GPU frame in flight.
//! Run with features backend-shim-win32,backend-shim-dx12; optional --warp, --debug, --frames=N.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
#[path = "support/dx12.rs"]
mod native_dx12;
#[cfg(windows)]
#[path = "support/win32.rs"]
mod native_window;

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use dear_imgui_sys::{
        self as sys,
        backend_shim::{dx12, win32},
    };
    use native_dx12::{Allocator, wait_gpu};
    use std::{
        mem::ManuallyDrop,
        ptr,
        time::{Duration, Instant},
    };
    use windows::{
        Win32::Graphics::{
            Direct3D::D3D_FEATURE_LEVEL_11_0,
            Direct3D12::*,
            Dxgi::{Common::*, *},
        },
        core::{IUnknown, Interface, w},
    };

    const BUFFER_COUNT: u32 = 2;
    const FORMAT: DXGI_FORMAT = DXGI_FORMAT_R8G8B8A8_UNORM;

    struct Context<'a> {
        raw: *mut sys::ImGuiContext,
        platform: bool,
        renderer: bool,
        queue: &'a ID3D12CommandQueue,
        fence: &'a ID3D12Fence,
    }
    impl Drop for Context<'_> {
        fn drop(&mut self) {
            // SAFETY: this example is the only fence signaler and keeps the queue/fence alive.
            // Abort on uncertain completion instead of releasing resources still used by the GPU.
            if self.renderer && unsafe { wait_gpu(self.queue, self.fence) }.is_err() {
                std::process::abort();
            }
            // SAFETY: the allocator, heap, GPU objects, and HWND outlive this owning context.
            unsafe {
                sys::igSetCurrentContext(self.raw);
                if self.renderer {
                    dx12::dear_imgui_backend_dx12_shutdown();
                }
                if self.platform {
                    win32::dear_imgui_backend_win32_shutdown();
                }
                sys::igDestroyContext(self.raw);
            }
        }
    }

    unsafe fn back_buffers(
        device: &ID3D12Device,
        swap: &IDXGISwapChain3,
        heap: &ID3D12DescriptorHeap,
    ) -> windows::core::Result<Vec<ID3D12Resource>> {
        // SAFETY: all objects share a device; the RTV heap owns BUFFER_COUNT descriptors.
        unsafe {
            let start = heap.GetCPUDescriptorHandleForHeapStart().ptr;
            let step =
                device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize;
            let mut buffers = Vec::with_capacity(BUFFER_COUNT as usize);
            for index in 0..BUFFER_COUNT {
                let buffer: ID3D12Resource = swap.GetBuffer(index)?;
                device.CreateRenderTargetView(
                    &buffer,
                    None,
                    D3D12_CPU_DESCRIPTOR_HANDLE {
                        ptr: start + index as usize * step,
                    },
                );
                buffers.push(buffer);
            }
            Ok(buffers)
        }
    }

    unsafe fn transition(
        list: &ID3D12GraphicsCommandList,
        resource: &ID3D12Resource,
        before: D3D12_RESOURCE_STATES,
        after: D3D12_RESOURCE_STATES,
    ) {
        // SAFETY: the caller tracks the resource's actual state and retains it through completion.
        unsafe {
            let mut barrier = D3D12_RESOURCE_BARRIER {
                Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
                Anonymous: D3D12_RESOURCE_BARRIER_0 {
                    Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                        pResource: ManuallyDrop::new(Some(resource.clone())),
                        Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                        StateBefore: before,
                        StateAfter: after,
                    }),
                },
                ..Default::default()
            };
            list.ResourceBarrier(std::slice::from_ref(&barrier));
            // The union and its nested resource are ManuallyDrop; release our temporary COM clone.
            ManuallyDrop::drop(&mut (*barrier.Anonymous.Transition).pResource);
        }
    }

    let arguments: Vec<String> = std::env::args().collect();
    let warp = arguments.iter().any(|arg| arg == "--warp");
    let debug = arguments.iter().any(|arg| arg == "--debug");
    let limit = arguments.iter().find_map(|arg| {
        arg.strip_prefix("--frames=")
            .and_then(|value| value.parse::<u32>().ok())
    });

    // SAFETY: the entire integration runs on the creating thread, with raw sys frames only.
    // Local declaration order ensures context -> callback storage -> GPU resources -> HWND drop.
    unsafe {
        if debug {
            let mut layer: Option<ID3D12Debug> = None;
            D3D12GetDebugInterface(&mut layer)?;
            layer
                .expect("successful debug interface query returns an interface")
                .EnableDebugLayer();
        }
        win32::dear_imgui_backend_win32_enable_dpi_awareness();
        let window = native_window::Window::new(
            w!("DearImGuiSysWin32Dx12"),
            w!("Dear ImGui - raw Win32 / DX12 shims"),
        )?;
        let factory: IDXGIFactory4 = CreateDXGIFactory2(if debug {
            DXGI_CREATE_FACTORY_DEBUG
        } else {
            DXGI_CREATE_FACTORY_FLAGS(0)
        })?;
        let mut device: Option<ID3D12Device> = None;
        let hardware = if warp {
            false
        } else {
            D3D12CreateDevice(None::<&IUnknown>, D3D_FEATURE_LEVEL_11_0, &mut device).is_ok()
        };
        if !hardware {
            let adapter: IDXGIAdapter = factory.EnumWarpAdapter()?;
            D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device)?;
        }
        let device = device.expect("successful D3D12CreateDevice returns a device");
        let queue: ID3D12CommandQueue = device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
            Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
            ..Default::default()
        })?;
        let fence: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;
        let mut size = window.client_size()?;
        let swap: IDXGISwapChain3 = factory
            .CreateSwapChainForHwnd(
                &queue,
                window.0,
                &DXGI_SWAP_CHAIN_DESC1 {
                    Width: size.0.max(1),
                    Height: size.1.max(1),
                    Format: FORMAT,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                    BufferCount: BUFFER_COUNT,
                    SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                    ..Default::default()
                },
                None,
                None::<&IDXGIOutput>,
            )?
            .cast()?;
        factory.MakeWindowAssociation(window.0, DXGI_MWA_NO_ALT_ENTER)?;
        let rtv_heap: ID3D12DescriptorHeap =
            device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                NumDescriptors: BUFFER_COUNT,
                ..Default::default()
            })?;
        let srv_heap: ID3D12DescriptorHeap =
            device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                NumDescriptors: 32,
                Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
                ..Default::default()
            })?;
        let mut buffers = back_buffers(&device, &swap, &rtv_heap)?;
        let rtv_start = rtv_heap.GetCPUDescriptorHandleForHeapStart().ptr;
        let rtv_step =
            device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize;
        let commands: ID3D12CommandAllocator =
            device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let list: ID3D12GraphicsCommandList = device.CreateCommandList(
            0,
            D3D12_COMMAND_LIST_TYPE_DIRECT,
            &commands,
            None::<&ID3D12PipelineState>,
        )?;
        list.Close()?;
        let mut descriptors = Allocator::new(&device, &srv_heap);
        let allocator = descriptors.callbacks();
        let raw = sys::igCreateContext(ptr::null_mut());
        assert!(!raw.is_null());
        let mut context = Context {
            raw,
            platform: false,
            renderer: false,
            queue: &queue,
            fence: &fence,
        };
        (*sys::igGetIO_Nil()).IniFilename = ptr::null();
        (*sys::igGetIO_Nil()).ConfigFlags |= sys::ImGuiConfigFlags_NavEnableKeyboard;
        assert!(win32::dear_imgui_backend_win32_init(window.0.0));
        context.platform = true;
        assert!(dx12::dear_imgui_backend_dx12_init(&dx12::InitInfo {
            device: device.as_raw(),
            command_queue: queue.as_raw(),
            num_frames_in_flight: 1,
            rtv_format: FORMAT.0 as u32,
            dsv_format: 0,
            srv_descriptor_heap: srv_heap.as_raw(),
            descriptor_allocator: allocator.as_ref(),
        }));
        context.renderer = true;
        let mut dpi_style = native_window::DpiStyle::new(&window);
        window.show();

        let mut frames = 0;
        let mut previous = Instant::now();
        let mut show_demo = true;
        let mut clicks = 0;
        let mut clear = [0.08, 0.1, 0.14, 1.0];
        while window.pump_events() && !limit.is_some_and(|limit| frames >= limit) {
            let current = window.client_size()?;
            if current.0 == 0 || current.1 == 0 {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            if current != size {
                wait_gpu(&queue, &fence)?;
                buffers.clear();
                swap.ResizeBuffers(
                    BUFFER_COUNT,
                    current.0,
                    current.1,
                    FORMAT,
                    DXGI_SWAP_CHAIN_FLAG(0),
                )?;
                buffers = back_buffers(&device, &swap, &rtv_heap)?;
                size = current;
            }

            let now = Instant::now();
            (*sys::igGetIO_Nil()).DeltaTime = (now - previous).as_secs_f32().clamp(0.001, 0.1);
            previous = now;
            dx12::dear_imgui_backend_dx12_new_frame();
            win32::dear_imgui_backend_win32_new_frame();
            let scale = dpi_style.update(&window);
            let panel_width = (size.0 as f32 * 0.38).min(360.0 * scale);
            sys::igNewFrame();
            sys::igSetNextWindowPos(
                sys::ImVec2_c {
                    x: 20.0 * scale,
                    y: 20.0 * scale,
                },
                sys::ImGuiCond_FirstUseEver,
                sys::ImVec2_c { x: 0.0, y: 0.0 },
            );
            sys::igSetNextWindowSize(
                sys::ImVec2_c {
                    x: panel_width,
                    y: 280.0 * scale,
                },
                sys::ImGuiCond_FirstUseEver,
            );
            if sys::igBegin(c"DX12 backend controls".as_ptr(), ptr::null_mut(), 0) {
                sys::igPushTextWrapPos(0.0);
                sys::igTextUnformatted(
                    c"Application-owned Win32 / D3D12 integration".as_ptr(),
                    ptr::null(),
                );
                sys::igTextUnformatted(
                    if hardware {
                        c"Renderer: hardware D3D12".as_ptr()
                    } else {
                        c"Renderer: WARP software D3D12".as_ptr()
                    },
                    ptr::null(),
                );
                sys::igTextUnformatted(
                    c"One GPU frame in flight; explicit barriers and fences.".as_ptr(),
                    ptr::null(),
                );
                sys::igPopTextWrapPos();
                let dpi = std::ffi::CString::new(format!("Display scale: {:.0}%", scale * 100.0))
                    .expect("numeric text has no NUL");
                sys::igTextUnformatted(dpi.as_ptr(), ptr::null());
                sys::igCheckbox(c"Show Dear ImGui demo".as_ptr(), &mut show_demo);
                sys::igColorEdit4(c"Clear color".as_ptr(), clear.as_mut_ptr(), 0);
                if sys::igButton(c"Click me".as_ptr(), sys::ImVec2_c { x: 0.0, y: 0.0 }) {
                    clicks += 1;
                }
                let count = std::ffi::CString::new(format!("Click count: {clicks}"))
                    .expect("integer text has no NUL");
                sys::igSameLine(0.0, -1.0);
                sys::igTextUnformatted(count.as_ptr(), ptr::null());
            }
            sys::igEnd();
            if show_demo {
                sys::igShowDemoWindow(&mut show_demo);
                // The demo chooses its own first-use geometry. Set it after creation
                // so the next frame fits beside the control panel at the current DPI.
                if frames == 0 {
                    let demo_x = panel_width + 40.0 * scale;
                    sys::igSetWindowPos_Str(
                        c"Dear ImGui Demo".as_ptr(),
                        sys::ImVec2_c {
                            x: demo_x,
                            y: 20.0 * scale,
                        },
                        sys::ImGuiCond_Always,
                    );
                    sys::igSetWindowSize_Str(
                        c"Dear ImGui Demo".as_ptr(),
                        sys::ImVec2_c {
                            x: (size.0 as f32 - demo_x - 20.0 * scale)
                                .clamp(100.0 * scale, 500.0 * scale),
                            y: (size.1 as f32 - 40.0 * scale).clamp(100.0 * scale, 530.0 * scale),
                        },
                        sys::ImGuiCond_Always,
                    );
                }
            }
            sys::igRender();

            // The previous iteration waited, so allocator/list reuse and descriptor frees are safe.
            commands.Reset()?;
            list.Reset(&commands, None::<&ID3D12PipelineState>)?;
            let index = swap.GetCurrentBackBufferIndex() as usize;
            let target = &buffers[index];
            let rtv = D3D12_CPU_DESCRIPTOR_HANDLE {
                ptr: rtv_start + index * rtv_step,
            };
            transition(
                &list,
                target,
                D3D12_RESOURCE_STATE_PRESENT,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
            );
            list.SetDescriptorHeaps(&[Some(srv_heap.clone())]);
            list.OMSetRenderTargets(1, Some(&rtv), false, None);
            list.ClearRenderTargetView(rtv, &clear, None);
            dx12::dear_imgui_backend_dx12_render_draw_data(sys::igGetDrawData(), list.as_raw());
            transition(
                &list,
                target,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
                D3D12_RESOURCE_STATE_PRESENT,
            );
            list.Close()?;
            queue.ExecuteCommandLists(&[Some(list.cast()?)]);
            swap.Present(1, DXGI_PRESENT(0)).ok()?;
            wait_gpu(&queue, &fence)?;
            frames += 1;
        }
        drop(context);
        assert!(
            descriptors.valid,
            "descriptor handles must survive the C ABI"
        );
        assert_eq!(descriptors.allocations, descriptors.frees);
        assert!(!descriptors.used.iter().any(|used| *used));
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("win32_dx12 requires Windows");
}

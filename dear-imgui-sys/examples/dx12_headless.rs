//! Raw sys integration: render one Dear ImGui frame on D3D12 WARP without a window.
//! Run with `cargo run -p dear-imgui-sys --example dx12_headless --features backend-shim-dx12`.

#[cfg(windows)]
pub fn run() -> windows::core::Result<()> {
    use dear_imgui_sys::{self as sys, backend_shim::dx12};
    use std::{ffi::c_void, mem::ManuallyDrop, ptr};
    use windows::{
        Win32::Graphics::{
            Direct3D::D3D_FEATURE_LEVEL_11_0,
            Direct3D12::*,
            Dxgi::{Common::*, *},
        },
        core::Interface,
    };

    struct Allocator {
        cpu: usize,
        gpu: u64,
        step: usize,
        used: [bool; 32],
        allocations: usize,
        frees: usize,
        valid: bool,
    }
    unsafe extern "C" fn allocate(data: *mut c_void, cpu: *mut usize, gpu: *mut u64) {
        // SAFETY: the boxed allocator and both output pointers remain live during the callback.
        unsafe {
            let state = &mut *data.cast::<Allocator>();
            let Some(index) = state.used.iter().position(|used| !used) else {
                std::process::abort()
            };
            state.used[index] = true;
            state.allocations += 1;
            *cpu = state.cpu + index * state.step;
            *gpu = state.gpu + (index * state.step) as u64;
        }
    }
    unsafe extern "C" fn free(data: *mut c_void, cpu: usize, gpu: u64) {
        // SAFETY: the backend returns descriptor pairs issued by allocate above.
        unsafe {
            let state = &mut *data.cast::<Allocator>();
            let index = cpu.wrapping_sub(state.cpu) / state.step;
            if index >= state.used.len()
                || !state.used[index]
                || cpu != state.cpu + index * state.step
                || gpu != state.gpu + (index * state.step) as u64
            {
                state.valid = false;
                return;
            }
            state.used[index] = false;
            state.frees += 1;
        }
    }
    fn wait_gpu(queue: &ID3D12CommandQueue, fence: &ID3D12Fence) -> windows::core::Result<()> {
        // SAFETY: both objects belong to the same live device; the fence outlives the signal.
        unsafe {
            let completed = fence.GetCompletedValue();
            let value = completed.checked_add(1).ok_or_else(|| {
                windows::core::Error::new(
                    windows::Win32::Foundation::E_FAIL,
                    "D3D12 device was removed",
                )
            })?;
            queue.Signal(fence, value)?;
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            loop {
                let completed = fence.GetCompletedValue();
                if completed == u64::MAX {
                    return Err(windows::core::Error::new(
                        windows::Win32::Foundation::E_FAIL,
                        "D3D12 device was removed",
                    ));
                }
                if completed >= value {
                    break;
                }
                if std::time::Instant::now() >= deadline {
                    return Err(windows::core::Error::new(
                        windows::Win32::Foundation::E_FAIL,
                        "D3D12 fence timed out",
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        Ok(())
    }
    struct ContextGuard<'a> {
        raw: *mut sys::ImGuiContext,
        initialized: bool,
        queue: &'a ID3D12CommandQueue,
        fence: &'a ID3D12Fence,
    }
    impl Drop for ContextGuard<'_> {
        fn drop(&mut self) {
            // Do not destroy renderer resources while submitted work may still use them.
            if self.initialized && wait_gpu(self.queue, self.fence).is_err() {
                std::process::abort();
            }
            // SAFETY: the guard owns this context; GPU objects and callback storage outlive it.
            unsafe {
                sys::igSetCurrentContext(self.raw);
                if self.initialized {
                    dx12::dear_imgui_backend_dx12_shutdown();
                }
                sys::igDestroyContext(self.raw);
            }
        }
    }

    // SAFETY: the following single-threaded raw integration owns every native/GPU object and
    // keeps callback storage stable until ContextGuard has completed backend shutdown.
    unsafe {
        let factory: IDXGIFactory4 = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))?;
        let adapter: IDXGIAdapter = factory.EnumWarpAdapter()?;
        let mut device = None;
        D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device)?;
        let device: ID3D12Device = device.expect("successful D3D12CreateDevice returns a device");
        let queue: ID3D12CommandQueue = device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
            Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
            ..Default::default()
        })?;
        let fence: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;
        let srv_heap: ID3D12DescriptorHeap =
            device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                NumDescriptors: 32,
                Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
                ..Default::default()
            })?;
        let rtv_heap: ID3D12DescriptorHeap =
            device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                NumDescriptors: 1,
                ..Default::default()
            })?;
        let mut target = None;
        device.CreateCommittedResource(
            &D3D12_HEAP_PROPERTIES {
                Type: D3D12_HEAP_TYPE_DEFAULT,
                ..Default::default()
            },
            D3D12_HEAP_FLAG_NONE,
            &D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                Width: 64,
                Height: 64,
                DepthOrArraySize: 1,
                MipLevels: 1,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Flags: D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
                ..Default::default()
            },
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            None,
            &mut target,
        )?;
        let target: ID3D12Resource =
            target.expect("successful resource creation returns a resource");
        let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
        let mut readback_size = 0;
        device.GetCopyableFootprints(
            &target.GetDesc(),
            0,
            1,
            0,
            Some(&mut footprint),
            None,
            None,
            Some(&mut readback_size),
        );
        let mut readback = None;
        device.CreateCommittedResource(
            &D3D12_HEAP_PROPERTIES {
                Type: D3D12_HEAP_TYPE_READBACK,
                ..Default::default()
            },
            D3D12_HEAP_FLAG_NONE,
            &D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                Width: readback_size,
                Height: 1,
                DepthOrArraySize: 1,
                MipLevels: 1,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                ..Default::default()
            },
            D3D12_RESOURCE_STATE_COPY_DEST,
            None,
            &mut readback,
        )?;
        let readback: ID3D12Resource =
            readback.expect("successful readback creation returns a resource");
        let rtv = rtv_heap.GetCPUDescriptorHandleForHeapStart();
        device.CreateRenderTargetView(&target, None, rtv);
        let commands: ID3D12CommandAllocator =
            device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let list: ID3D12GraphicsCommandList = device.CreateCommandList(
            0,
            D3D12_COMMAND_LIST_TYPE_DIRECT,
            &commands,
            None::<&ID3D12PipelineState>,
        )?;

        let mut state = Box::new(Allocator {
            cpu: srv_heap.GetCPUDescriptorHandleForHeapStart().ptr,
            gpu: srv_heap.GetGPUDescriptorHandleForHeapStart().ptr,
            step: device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV)
                as usize,
            used: [false; 32],
            allocations: 0,
            frees: 0,
            valid: true,
        });
        let allocator = Box::new(dx12::DescriptorAllocator {
            user_data: ptr::from_mut(state.as_mut()).cast(),
            allocate: Some(allocate),
            free: Some(free),
        });
        let raw = sys::igCreateContext(ptr::null_mut());
        assert!(!raw.is_null());
        let mut context = ContextGuard {
            raw,
            initialized: false,
            queue: &queue,
            fence: &fence,
        };
        assert!(!dx12::dear_imgui_backend_dx12_init(ptr::null()));
        {
            let info = dx12::InitInfo {
                device: device.as_raw(),
                command_queue: queue.as_raw(),
                num_frames_in_flight: 1,
                rtv_format: DXGI_FORMAT_R8G8B8A8_UNORM.0 as u32,
                dsv_format: 0,
                srv_descriptor_heap: srv_heap.as_raw(),
                descriptor_allocator: allocator.as_ref(),
            };
            assert!(dx12::dear_imgui_backend_dx12_init(&info));
            context.initialized = true;
            assert!(!dx12::dear_imgui_backend_dx12_init(&info));
        } // InitInfo can be dropped; the boxed allocator stays alive.
        let io = &mut *sys::igGetIO_Nil();
        io.IniFilename = ptr::null();
        io.DisplaySize = sys::ImVec2 { x: 64.0, y: 64.0 };
        io.DeltaTime = 1.0 / 60.0;
        dx12::dear_imgui_backend_dx12_new_frame();
        sys::igNewFrame();
        sys::ImDrawList_AddRectFilled(
            sys::igGetBackgroundDrawList(ptr::null_mut()),
            sys::ImVec2_c { x: 8.0, y: 8.0 },
            sys::ImVec2_c { x: 56.0, y: 56.0 },
            0xff0000ff,
            0.0,
            0,
        );
        sys::igRender();
        let draw = sys::igGetDrawData();
        assert!((*draw).TotalVtxCount > 0);
        list.SetDescriptorHeaps(&[Some(srv_heap.clone())]);
        list.OMSetRenderTargets(1, Some(&rtv), false, None);
        list.ClearRenderTargetView(rtv, &[0.0, 0.0, 0.0, 1.0], None);
        dx12::dear_imgui_backend_dx12_render_draw_data(draw, list.as_raw());
        let mut barrier = D3D12_RESOURCE_BARRIER {
            Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Anonymous: D3D12_RESOURCE_BARRIER_0 {
                Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: ManuallyDrop::new(Some(target.clone())),
                    Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    StateBefore: D3D12_RESOURCE_STATE_RENDER_TARGET,
                    StateAfter: D3D12_RESOURCE_STATE_COPY_SOURCE,
                }),
            },
            ..Default::default()
        };
        list.ResourceBarrier(std::slice::from_ref(&barrier));
        ManuallyDrop::drop(&mut (*barrier.Anonymous.Transition).pResource);
        let mut source = D3D12_TEXTURE_COPY_LOCATION {
            pResource: ManuallyDrop::new(Some(target.clone())),
            Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
            Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                SubresourceIndex: 0,
            },
        };
        let mut destination = D3D12_TEXTURE_COPY_LOCATION {
            pResource: ManuallyDrop::new(Some(readback.clone())),
            Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
            Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                PlacedFootprint: footprint,
            },
        };
        list.CopyTextureRegion(&destination, 0, 0, 0, &source, None);
        ManuallyDrop::drop(&mut source.pResource);
        ManuallyDrop::drop(&mut destination.pResource);
        list.Close()?;
        queue.ExecuteCommandLists(&[Some(list.cast()?)]);
        wait_gpu(&queue, &fence)?;
        let mut pixels = ptr::null_mut();
        readback.Map(0, None, Some(&mut pixels))?;
        let center =
            footprint.Offset as usize + 32 * footprint.Footprint.RowPitch as usize + 32 * 4;
        let actual = ptr::read_unaligned(pixels.cast::<u8>().add(center).cast::<[u8; 4]>());
        let background = ptr::read_unaligned(
            pixels
                .cast::<u8>()
                .add(footprint.Offset as usize)
                .cast::<[u8; 4]>(),
        );
        readback.Unmap(0, Some(&D3D12_RANGE { Begin: 0, End: 0 }));
        assert_eq!(
            actual,
            [255, 0, 0, 255],
            "the UI rectangle must reach the GPU target"
        );
        assert_eq!(
            background,
            [0, 0, 0, 255],
            "pixels outside the UI must retain the clear color"
        );
        assert!(
            state.allocations > 0,
            "dynamic atlas must allocate a descriptor"
        );
        drop(context);
        assert!(
            state.valid,
            "CPU/GPU descriptor handles must survive the callback ABI"
        );
        assert_eq!(state.allocations, state.frees);
        assert!(!state.used.iter().any(|used| *used));
        println!(
            "Rendered a DX12 WARP frame; {} descriptors allocated and freed",
            state.allocations
        );
    }
    Ok(())
}

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    run()
}

#[cfg(not(windows))]
fn main() {
    eprintln!("dx12_headless requires Windows");
}

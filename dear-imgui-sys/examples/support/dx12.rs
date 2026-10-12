//! Shared descriptor callbacks and serialized GPU completion for the raw DX12 examples.

use dear_imgui_sys::backend_shim::dx12;
use std::{
    ffi::c_void,
    ptr,
    time::{Duration, Instant},
};
use windows::{
    Win32::{Foundation::E_FAIL, Graphics::Direct3D12::*},
    core::{Error, Result},
};

pub struct Allocator {
    pub cpu: usize,
    pub gpu: u64,
    pub step: usize,
    pub used: [bool; 32],
    pub allocations: usize,
    pub frees: usize,
    pub valid: bool,
}

impl Allocator {
    /// # Safety
    /// The shader-visible CBV/SRV/UAV heap must contain at least 32 descriptors and belong to
    /// `device`. Keep both objects and this boxed state live until renderer shutdown returns.
    pub unsafe fn new(device: &ID3D12Device, heap: &ID3D12DescriptorHeap) -> Box<Self> {
        unsafe {
            Box::new(Self {
                cpu: heap.GetCPUDescriptorHandleForHeapStart().ptr,
                gpu: heap.GetGPUDescriptorHandleForHeapStart().ptr,
                step: device
                    .GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV)
                    as usize,
                used: [false; 32],
                allocations: 0,
                frees: 0,
                valid: true,
            })
        }
    }

    // Constructing the callback record does not invoke FFI. Its address and the boxed state must
    // remain stable for the borrowed allocator contract of dear_imgui_backend_dx12_init.
    pub fn callbacks(&mut self) -> Box<dx12::DescriptorAllocator> {
        Box::new(dx12::DescriptorAllocator {
            user_data: ptr::from_mut(self).cast(),
            allocate: Some(allocate),
            free: Some(free),
        })
    }
}

unsafe extern "C" fn allocate(data: *mut c_void, cpu: *mut usize, gpu: *mut u64) {
    // SAFETY: the boxed state and both output pointers stay live for this callback.
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
    // SAFETY: the renderer returns descriptor pairs issued by allocate above.
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

/// # Safety
/// The queue and fence must belong to the same device. This thread must be the only signaler of
/// this fence, and it must wait after every submission before recycling GPU resources.
pub unsafe fn wait_gpu(queue: &ID3D12CommandQueue, fence: &ID3D12Fence) -> Result<()> {
    unsafe {
        let value = fence
            .GetCompletedValue()
            .checked_add(1)
            .ok_or_else(|| Error::new(E_FAIL, "D3D12 device was removed"))?;
        queue.Signal(fence, value)?;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let completed = fence.GetCompletedValue();
            if completed == u64::MAX {
                return Err(Error::new(E_FAIL, "D3D12 device was removed"));
            }
            if completed >= value {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(Error::new(E_FAIL, "D3D12 fence timed out"));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

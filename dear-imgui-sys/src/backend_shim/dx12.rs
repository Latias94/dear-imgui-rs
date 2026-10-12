//! Low-level DirectX 12 renderer using the official backend's descriptor allocator API.
//!
//! # Safety
//!
//! Keep the intended Dear ImGui context current for every call. GPU pointers must refer to live
//! D3D12 objects from the same device. The command queue must support direct command lists, and the
//! SRV heap must be shader-visible. The application owns command submission, resource barriers,
//! render targets, heap binding, and GPU completion before resource reuse or backend shutdown.
//!
//! The initialization record is copied, but its [`DescriptorAllocator`] is borrowed: keep the
//! allocator at a stable address, its user data alive, and its callbacks callable until shutdown
//! returns. Callbacks must not unwind, reenter the backend, or fail to supply a valid unused CPU/GPU
//! descriptor pair. Descriptors must remain allocated until the backend invokes the free callback.
//!
//! Draw data and texture pointers must be live and uniquely mutable during calls. Pair successful
//! init with exactly one shutdown before destroying the Dear ImGui context. Backend entry points
//! are not thread-safe. These functions do not implement the safe managed-frame protocol.

use std::ffi::c_void;

use crate::{ImDrawData, ImTextureData};

/// Borrowed, stable-address descriptor allocator. Handles cross the C ABI as integers.
#[repr(C)]
pub struct DescriptorAllocator {
    pub user_data: *mut c_void,
    pub allocate: Option<unsafe extern "C" fn(*mut c_void, *mut usize, *mut u64)>,
    pub free: Option<unsafe extern "C" fn(*mut c_void, usize, u64)>,
}

/// Initialization values copied by the backend; `descriptor_allocator` remains borrowed.
/// Formats are numeric `DXGI_FORMAT` values. Use zero for `dsv_format` when no DSV is used.
#[repr(C)]
pub struct InitInfo {
    pub device: *mut c_void,
    pub command_queue: *mut c_void,
    pub num_frames_in_flight: i32,
    pub rtv_format: u32,
    pub dsv_format: u32,
    pub srv_descriptor_heap: *mut c_void,
    pub descriptor_allocator: *const DescriptorAllocator,
}

unsafe extern "C" {
    /// Returns false for missing required initialization fields or an occupied renderer slot.
    /// Non-null pointers must still satisfy this module's safety contract.
    pub fn dear_imgui_backend_dx12_init(info: *const InitInfo) -> bool;
    pub fn dear_imgui_backend_dx12_shutdown();
    pub fn dear_imgui_backend_dx12_new_frame();
    pub fn dear_imgui_backend_dx12_render_draw_data(
        draw_data: *mut ImDrawData,
        command_list: *mut c_void,
    );
    pub fn dear_imgui_backend_dx12_create_device_objects() -> bool;
    pub fn dear_imgui_backend_dx12_invalidate_device_objects();
    pub fn dear_imgui_backend_dx12_update_texture(texture: *mut ImTextureData);
}

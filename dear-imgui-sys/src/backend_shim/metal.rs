//! Low-level official Metal renderer for macOS and iOS.
//!
//! # Safety
//!
//! Opaque pointers must be live Objective-C objects implementing the named Metal protocols, all
//! from the same device. Keep the intended Dear ImGui context current. Supply a valid
//! `MTLRenderPassDescriptor*` to new-frame and compatible `id<MTLCommandBuffer>` /
//! `id<MTLRenderCommandEncoder>` objects to rendering. This shim borrows objects and does not
//! transfer Objective-C ownership; the application owns an autorelease pool and command submission.
//! Draw data and texture pointers must be live and uniquely mutable. Serialize backend calls,
//! wait for GPU work before tearing down resources, and shut down before destroying the context.
//! These functions do not implement the safe managed-frame protocol.

use std::ffi::c_void;

use crate::{ImDrawData, ImTextureData};

unsafe extern "C" {
    pub fn dear_imgui_backend_metal_init(device: *mut c_void) -> bool;
    pub fn dear_imgui_backend_metal_shutdown();
    pub fn dear_imgui_backend_metal_new_frame(render_pass_descriptor: *mut c_void);
    pub fn dear_imgui_backend_metal_render_draw_data(
        draw_data: *mut ImDrawData,
        command_buffer: *mut c_void,
        command_encoder: *mut c_void,
    );
    pub fn dear_imgui_backend_metal_create_device_objects(device: *mut c_void) -> bool;
    /// Wait for GPU completion before destroying resources. Explicitly call create-device-objects
    /// with the original device before rendering again; new-frame alone does not restore them.
    pub fn dear_imgui_backend_metal_destroy_device_objects();
    pub fn dear_imgui_backend_metal_update_texture(texture: *mut ImTextureData);
}

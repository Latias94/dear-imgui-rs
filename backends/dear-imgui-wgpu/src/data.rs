//! Core data structures for the WGPU renderer
//!
//! This module contains the main backend data structure and initialization info,
//! following the pattern from imgui_impl_wgpu.cpp

use crate::{FrameResources, RenderResources};
use wgpu::*;

/// Selected render state data shared with callbacks
///
/// This corresponds to ImGui_ImplWGPU_RenderState in the C++ implementation.
/// This is temporarily stored during the render_draw_data() call to allow
/// draw callbacks to access the current render state.
#[derive(Debug)]
pub struct WgpuRenderState {
    /// WGPU device for creating resources (raw pointer for lifetime flexibility)
    pub device: *const Device,
    /// Current render pass encoder for drawing (raw pointer for lifetime flexibility)
    pub render_pass_encoder: *mut std::ffi::c_void,
}

impl WgpuRenderState {
    /// Create a new render state from references
    ///
    /// # Safety
    ///
    /// The caller must ensure that the device and render pass remain valid
    /// for the lifetime of this render state.
    pub unsafe fn new(device: &Device, render_pass: &mut RenderPass) -> Self {
        Self {
            device: device as *const Device,
            render_pass_encoder: render_pass as *mut _ as *mut std::ffi::c_void,
        }
    }

    /// Get the device reference
    ///
    /// # Safety
    ///
    /// The caller must ensure that the device pointer is still valid.
    pub unsafe fn device(&self) -> &Device {
        &*self.device
    }

    /// Get the render pass encoder reference
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// 1. The render pass pointer is still valid
    /// 2. No other mutable references to the render pass exist
    /// 3. The lifetime is appropriate
    ///
    /// This method is designed for use in C++ callbacks where we need to provide
    /// mutable access to the render pass from an immutable context.
    ///
    /// # Clippy Allow
    ///
    /// We allow `clippy::mut_from_ref` here because this is a legitimate use case
    /// for FFI interop where we need to provide mutable access through an immutable
    /// interface to match C++ callback expectations.
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn render_pass_encoder(&self) -> &mut RenderPass<'_> {
        &mut *(self.render_pass_encoder as *mut RenderPass)
    }
}

/// Initialization data for ImGui WGPU renderer
///
/// This corresponds to ImGui_ImplWGPU_InitInfo in the C++ implementation
#[derive(Debug, Clone)]
pub struct WgpuInitInfo {
    /// WGPU device
    pub device: Device,
    /// WGPU queue
    pub queue: Queue,
    /// Number of frames in flight (default: 3)
    pub num_frames_in_flight: u32,
    /// Render target format
    pub render_target_format: TextureFormat,
    /// Depth stencil format (None if no depth buffer)
    pub depth_stencil_format: Option<TextureFormat>,
    /// Pipeline multisample state
    pub pipeline_multisample_state: MultisampleState,
}

impl WgpuInitInfo {
    /// Create new initialization info with required parameters
    pub fn new(device: Device, queue: Queue, render_target_format: TextureFormat) -> Self {
        Self {
            device,
            queue,
            num_frames_in_flight: 3,
            render_target_format,
            depth_stencil_format: None,
            pipeline_multisample_state: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
        }
    }

    /// Set the number of frames in flight
    pub fn with_frames_in_flight(mut self, count: u32) -> Self {
        self.num_frames_in_flight = count;
        self
    }

    /// Set the depth stencil format
    pub fn with_depth_stencil_format(mut self, format: TextureFormat) -> Self {
        self.depth_stencil_format = Some(format);
        self
    }

    /// Set the multisample state
    pub fn with_multisample_state(mut self, state: MultisampleState) -> Self {
        self.pipeline_multisample_state = state;
        self
    }
}

/// Main backend data structure
///
/// This corresponds to ImGui_ImplWGPU_Data in the C++ implementation
pub struct WgpuBackendData {
    /// Initialization info
    pub init_info: WgpuInitInfo,
    /// WGPU device
    pub device: Device,
    /// Default queue
    pub queue: Queue,
    /// Render target format
    pub render_target_format: TextureFormat,
    /// Depth stencil format
    pub depth_stencil_format: Option<TextureFormat>,
    /// Render pipeline
    pub pipeline_state: Option<RenderPipeline>,
    /// Render resources (samplers, uniforms, bind groups)
    pub render_resources: RenderResources,
    /// Frame resources (per-frame buffers)
    pub frame_resources: Vec<FrameResources>,
    /// Number of frames in flight
    pub num_frames_in_flight: u32,
    /// Current frame index
    pub frame_index: u32,
}

impl WgpuBackendData {
    /// Create new backend data from initialization info
    pub fn new(init_info: WgpuInitInfo) -> Self {
        let queue = init_info.queue.clone();
        let num_frames = init_info.num_frames_in_flight;

        // Create frame resources for each frame in flight
        let frame_resources = (0..num_frames).map(|_| FrameResources::new()).collect();

        Self {
            device: init_info.device.clone(),
            queue,
            render_target_format: init_info.render_target_format,
            depth_stencil_format: init_info.depth_stencil_format,
            pipeline_state: None,
            render_resources: RenderResources::new(),
            frame_resources,
            num_frames_in_flight: num_frames,
            frame_index: u32::MAX, // Will be set to 0 on first frame
            init_info,
        }
    }

    /// Get the current frame resources
    pub fn current_frame_resources(&mut self) -> &mut FrameResources {
        let index = (self.frame_index % self.num_frames_in_flight) as usize;
        &mut self.frame_resources[index]
    }

    /// Get frame resources by index
    pub fn frame_resources_at(&mut self, index: usize) -> Option<&mut FrameResources> {
        self.frame_resources.get_mut(index)
    }

    /// Advance to the next frame
    pub fn next_frame(&mut self) {
        if self.frame_index == u32::MAX {
            self.frame_index = 0;
        } else {
            self.frame_index = self.frame_index.wrapping_add(1);
        }
    }

    /// Check if the backend is initialized
    pub fn is_initialized(&self) -> bool {
        self.pipeline_state.is_some()
    }
}

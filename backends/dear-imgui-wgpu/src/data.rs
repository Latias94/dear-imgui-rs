//! Core data structures for the WGPU renderer
//!
//! This module contains the main backend data structure and initialization info,
//! following the pattern from imgui_impl_wgpu.cpp

use std::{cell::Cell, ffi::c_void, marker::PhantomData, num::NonZeroU32, ptr::NonNull, rc::Rc};

use crate::{FrameResourceArena, FrameResources, RenderResources, RendererError, RendererResult};
use dear_imgui_rs::sys;
use thiserror::Error;
use wgpu::*;

/// Error returned while borrowing the transient WGPU draw-callback state.
#[derive(Copy, Clone, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum WgpuRenderStateAccessError {
    /// No WGPU render state is active on the current Dear ImGui Context.
    #[error("no WGPU render state is active on the current Dear ImGui context")]
    Inactive,
    /// The active callback state is already borrowed by an outer scoped access.
    #[error("the active WGPU render state is already borrowed")]
    AlreadyBorrowed,
}

pub(crate) struct WgpuRenderStateStorage {
    device: NonNull<Device>,
    render_pass: NonNull<c_void>,
    borrowed: Cell<bool>,
}

impl WgpuRenderStateStorage {
    pub(crate) fn new(device: &Device, render_pass: &mut RenderPass<'_>) -> Self {
        Self {
            device: NonNull::from(device),
            render_pass: NonNull::from(render_pass).cast(),
            borrowed: Cell::new(false),
        }
    }
}

struct WgpuRenderStateBorrow<'storage>(&'storage Cell<bool>);

impl Drop for WgpuRenderStateBorrow<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

/// Scoped access to the WGPU resources selected for a raw draw callback.
///
/// This corresponds to ImGui_ImplWGPU_RenderState in the C++ implementation.
/// The value can only be obtained through [`Self::with_current`] while the
/// renderer is invoking a raw callback. It cannot outlive that callback scope.
///
/// The state borrows the renderer's device and render pass; it does not own
/// either resource and does not provide access to the Dear ImGui [`Context`].
///
/// ```compile_fail
/// use dear_imgui_wgpu::WgpuRenderState;
///
/// // Callback-scoped resources cannot be returned from the higher-ranked borrow.
/// let _escaped = unsafe { WgpuRenderState::with_current(|state| state.device()) };
/// ```
///
/// [`Context`]: dear_imgui_rs::Context
#[derive(Debug)]
pub struct WgpuRenderState<'callback> {
    storage: NonNull<WgpuRenderStateStorage>,
    _callback: PhantomData<&'callback mut WgpuRenderStateStorage>,
    _ui_thread: PhantomData<Rc<()>>,
}

impl WgpuRenderState<'_> {
    /// Borrows the state published for the current raw draw callback.
    ///
    /// # Safety
    ///
    /// This function may only be called from a raw draw callback currently
    /// invoked by `dear-imgui-wgpu`. The current Dear ImGui Context must be the
    /// renderer owner and its `Renderer_RenderState` slot must still contain
    /// the WGPU state installed for this callback. The callback must not replace
    /// that slot while `callback` is running.
    ///
    /// The higher-ranked closure prevents references to the render pass from
    /// escaping the callback scope. Recursive access is rejected at runtime.
    pub unsafe fn with_current<R>(
        callback: impl for<'callback> FnOnce(WgpuRenderState<'callback>) -> R,
    ) -> Result<R, WgpuRenderStateAccessError> {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let raw_state = if platform_io.is_null() {
            None
        } else {
            NonNull::new(unsafe { (*platform_io).Renderer_RenderState })
        }
        .ok_or(WgpuRenderStateAccessError::Inactive)?;
        let storage = raw_state.cast::<WgpuRenderStateStorage>();
        let borrowed = unsafe { &storage.as_ref().borrowed };
        if borrowed.replace(true) {
            return Err(WgpuRenderStateAccessError::AlreadyBorrowed);
        }
        let _borrow = WgpuRenderStateBorrow(borrowed);
        Ok(callback(WgpuRenderState {
            storage,
            _callback: PhantomData,
            _ui_thread: PhantomData,
        }))
    }

    /// Returns the renderer device for the callback duration.
    pub fn device(&self) -> &Device {
        unsafe { self.storage.as_ref().device.as_ref() }
    }

    /// Returns the active render pass for the duration of this borrow.
    pub fn render_pass(&mut self) -> &mut RenderPass<'_> {
        unsafe {
            self.storage
                .as_ref()
                .render_pass
                .cast::<RenderPass<'_>>()
                .as_mut()
        }
    }

    /// Returns the device and render pass as disjoint callback-scoped borrows.
    pub fn resources(&mut self) -> (&Device, &mut RenderPass<'_>) {
        let storage = unsafe { self.storage.as_ref() };
        let device = unsafe { storage.device.as_ref() };
        let render_pass = unsafe { storage.render_pass.cast::<RenderPass<'_>>().as_mut() };
        (device, render_pass)
    }
}

/// Presentation policy for WGPU surfaces created for secondary viewports.
///
/// Surface format and dimensions are renderer- and viewport-owned respectively. Secondary
/// surfaces use the renderer's sRGB output contract; this value keeps the remaining scheduling
/// and compositor choices together so creation and surface-loss recovery cannot silently diverge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WgpuViewportSurfaceConfig {
    /// Requested presentation mode.
    pub present_mode: PresentMode,
    /// Requested compositor alpha mode.
    pub alpha_mode: CompositeAlphaMode,
    /// Maximum number of monitor refreshes between acquisition and presentation.
    pub desired_maximum_frame_latency: u32,
}

impl Default for WgpuViewportSurfaceConfig {
    fn default() -> Self {
        Self {
            present_mode: PresentMode::Fifo,
            alpha_mode: CompositeAlphaMode::Opaque,
            desired_maximum_frame_latency: 2,
        }
    }
}

impl From<&SurfaceConfiguration> for WgpuViewportSurfaceConfig {
    fn from(config: &SurfaceConfiguration) -> Self {
        Self {
            present_mode: config.present_mode,
            alpha_mode: config.alpha_mode,
            desired_maximum_frame_latency: config.desired_maximum_frame_latency,
        }
    }
}

/// Initialization data for ImGui WGPU renderer.
///
/// This corresponds to `ImGui_ImplWGPU_InitInfo` plus Rust-owned multi-viewport policy.
#[derive(Debug, Clone)]
pub struct WgpuInitInfo {
    /// WGPU instance (required for multi-viewport to create per-window surfaces)
    pub instance: Option<Instance>,
    /// WGPU adapter (required by multi-viewport; optional for single-window rendering)
    pub adapter: Option<Adapter>,
    /// WGPU device
    pub device: Device,
    /// WGPU queue
    pub queue: Queue,
    /// Number of submitted frames whose upload resources may remain in flight (default: 3).
    ///
    /// Submit each frame's command buffers before recording more than this many later ImGui
    /// frames. This renderer cannot observe when an application-owned encoder is submitted, so a
    /// command buffer retained beyond the configured window may reference a reused upload buffer.
    pub num_frames_in_flight: NonZeroU32,
    /// Render target format
    pub render_target_format: TextureFormat,
    /// Presentation policy for secondary viewport surfaces.
    pub viewport_surface_config: WgpuViewportSurfaceConfig,
    /// Depth stencil format (None if no depth buffer)
    pub depth_stencil_format: Option<TextureFormat>,
    /// Pipeline multisample state
    pub pipeline_multisample_state: MultisampleState,
}

impl WgpuInitInfo {
    /// Create new initialization info with required parameters
    pub fn new(device: Device, queue: Queue, render_target_format: TextureFormat) -> Self {
        Self {
            instance: None,
            adapter: None,
            device,
            queue,
            num_frames_in_flight: NonZeroU32::new(3).expect("three is non-zero"),
            render_target_format,
            viewport_surface_config: WgpuViewportSurfaceConfig::default(),
            depth_stencil_format: None,
            pipeline_multisample_state: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
        }
    }

    /// Set the maximum number of submitted frames kept in flight.
    ///
    /// Increase this only when the application intentionally queues more frames before GPU
    /// completion. It is not a license to retain unsubmitted command buffers across the ring.
    pub fn with_frames_in_flight(mut self, count: NonZeroU32) -> Self {
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

    /// Provide an instance for creating per-window surfaces (multi-viewport)
    pub fn with_instance(mut self, instance: Instance) -> Self {
        self.instance = Some(instance);
        self
    }

    /// Provide the adapter required to negotiate multi-viewport surface capabilities
    pub fn with_adapter(mut self, adapter: Adapter) -> Self {
        self.adapter = Some(adapter);
        self
    }

    /// Set the complete presentation policy for secondary viewport surfaces.
    pub fn with_viewport_surface_config(mut self, config: WgpuViewportSurfaceConfig) -> Self {
        self.viewport_surface_config = config;
        self
    }
}

/// Main backend data structure
///
/// This corresponds to ImGui_ImplWGPU_Data in the C++ implementation
pub(crate) struct WgpuBackendData {
    /// Initialization info
    pub(crate) init_info: WgpuInitInfo,
    /// WGPU device
    pub(crate) device: Device,
    /// Default queue
    pub(crate) queue: Queue,
    /// Render target format
    pub(crate) render_target_format: TextureFormat,
    /// Depth stencil format
    pub(crate) depth_stencil_format: Option<TextureFormat>,
    /// Render pipeline
    pub(crate) pipeline_state: Option<RenderPipeline>,
    /// Render resources (samplers, uniforms, bind groups)
    pub(crate) render_resources: RenderResources,
    /// Frame resources (per-frame buffers)
    pub(crate) frame_resources: Vec<FrameResourceArena>,
    /// Number of frames in flight
    pub(crate) num_frames_in_flight: NonZeroU32,
    /// Exact Context epoch and in-flight slot currently open for rendering.
    pub(crate) frame_cursor: FrameEpochCursor,
}

#[derive(Default)]
pub(crate) struct FrameEpochCursor {
    ring_index: Option<u32>,
    epoch: Option<u64>,
    native_frame_count: Option<i32>,
}

enum FrameEpochTransition {
    Reuse,
    Advance(u32),
}

impl FrameEpochCursor {
    fn enter(
        &mut self,
        epoch: u64,
        native_frame_count: i32,
    ) -> RendererResult<FrameEpochTransition> {
        if let Some(active_epoch) = self.epoch {
            if epoch < active_epoch {
                return Err(RendererError::FrameEpochOutOfOrder {
                    active: active_epoch,
                    received: epoch,
                });
            }
            if epoch == active_epoch {
                if self.native_frame_count != Some(native_frame_count) {
                    return Err(RendererError::InvalidRenderState(
                        "one WGPU render epoch was observed under multiple Dear ImGui frames"
                            .to_owned(),
                    ));
                }
                return Ok(FrameEpochTransition::Reuse);
            }
        }

        let ring_index = self.ring_index.map_or(0, |index| index.wrapping_add(1));
        self.ring_index = Some(ring_index);
        self.epoch = Some(epoch);
        self.native_frame_count = Some(native_frame_count);
        Ok(FrameEpochTransition::Advance(ring_index))
    }

    pub(crate) fn is_native_frame(&self, native_frame_count: i32) -> bool {
        self.native_frame_count == Some(native_frame_count)
    }

    fn ring_index(&self) -> Option<u32> {
        self.ring_index
    }
}

impl WgpuBackendData {
    /// Create new backend data from initialization info
    pub(crate) fn new(init_info: WgpuInitInfo) -> Self {
        let queue = init_info.queue.clone();
        let num_frames = init_info.num_frames_in_flight;

        // Create frame resources for each frame in flight
        let frame_resources = (0..num_frames.get())
            .map(|_| FrameResourceArena::new())
            .collect();

        Self {
            device: init_info.device.clone(),
            queue,
            render_target_format: init_info.render_target_format,
            depth_stencil_format: init_info.depth_stencil_format,
            pipeline_state: None,
            render_resources: RenderResources::new(),
            frame_resources,
            num_frames_in_flight: num_frames,
            frame_cursor: FrameEpochCursor::default(),
            init_info,
        }
    }

    /// Open the arena for one exact Context render epoch.
    pub(crate) fn begin_frame(
        &mut self,
        epoch: u64,
        native_frame_count: i32,
    ) -> RendererResult<()> {
        if let FrameEpochTransition::Advance(frame_index) =
            self.frame_cursor.enter(epoch, native_frame_count)?
        {
            self.frame_resources[(frame_index % self.num_frames_in_flight.get()) as usize]
                .begin_frame();
        }
        Ok(())
    }

    pub(crate) fn acquire_frame_resources(&mut self) -> RendererResult<&mut FrameResources> {
        let frame_index = self
            .frame_cursor
            .ring_index()
            .ok_or(RendererError::FrameNotPrepared)?;
        let arena = self
            .frame_resources
            .get_mut((frame_index % self.num_frames_in_flight.get()) as usize)
            .ok_or_else(|| {
                RendererError::InvalidRenderState(
                    "WGPU frame resource index is out of bounds".to_owned(),
                )
            })?;
        let frame = arena.acquire();
        frame.ensure_render_bindings(&self.device, &self.render_resources)?;
        Ok(frame)
    }

    /// Check if the backend is initialized
    pub(crate) fn is_initialized(&self) -> bool {
        self.pipeline_state.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_epoch_cursor_is_idempotent_and_rejects_stale_frames() {
        let mut cursor = FrameEpochCursor::default();
        assert!(matches!(
            cursor.enter(4, 12).unwrap(),
            FrameEpochTransition::Advance(0)
        ));
        assert!(matches!(
            cursor.enter(4, 12).unwrap(),
            FrameEpochTransition::Reuse
        ));
        assert!(matches!(
            cursor.enter(5, 13).unwrap(),
            FrameEpochTransition::Advance(1)
        ));
        assert!(matches!(
            cursor.enter(4, 12),
            Err(RendererError::FrameEpochOutOfOrder {
                active: 5,
                received: 4
            })
        ));
    }

    #[test]
    fn frame_epoch_cursor_rejects_one_epoch_under_two_native_frames() {
        let mut cursor = FrameEpochCursor::default();
        cursor.enter(1, 7).unwrap();
        assert!(matches!(
            cursor.enter(1, 8),
            Err(RendererError::InvalidRenderState(_))
        ));
    }

    #[test]
    fn viewport_surface_defaults_are_explicit_and_throughput_safe() {
        let config = WgpuViewportSurfaceConfig::default();
        assert_eq!(config.present_mode, PresentMode::Fifo);
        assert_eq!(config.alpha_mode, CompositeAlphaMode::Opaque);
        assert_eq!(config.desired_maximum_frame_latency, 2);
    }

    #[test]
    fn viewport_surface_config_copies_supported_main_surface_policy() {
        let surface = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: TextureFormat::Bgra8UnormSrgb,
            #[cfg(feature = "wgpu-30")]
            color_space: SurfaceColorSpace::DisplayP3,
            width: 128,
            height: 96,
            present_mode: PresentMode::AutoNoVsync,
            alpha_mode: CompositeAlphaMode::PreMultiplied,
            view_formats: vec![],
            desired_maximum_frame_latency: 3,
        };

        let viewport = WgpuViewportSurfaceConfig::from(&surface);
        assert_eq!(viewport.present_mode, surface.present_mode);
        assert_eq!(viewport.alpha_mode, surface.alpha_mode);
        assert_eq!(
            viewport.desired_maximum_frame_latency,
            surface.desired_maximum_frame_latency
        );
    }
}

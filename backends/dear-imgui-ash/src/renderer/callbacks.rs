use std::{cell::Cell, marker::PhantomData, ptr::NonNull, rc::Rc};

use ash::{Device, vk};
use dear_imgui_rs::sys;
use thiserror::Error;

#[used]
static RESET_RENDER_STATE_MARKER: u8 = 0;
#[used]
static SAMPLER_LINEAR_MARKER: u8 = 1;
#[used]
static SAMPLER_NEAREST_MARKER: u8 = 2;

/// Error returned while borrowing the transient Vulkan draw-callback state.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum AshRenderStateAccessError {
    /// No Ash render state is active on the current Dear ImGui Context.
    #[error("no Ash render state is active on the current Dear ImGui context")]
    Inactive,
    /// The active callback state is already borrowed by an outer scoped access.
    #[error("the active Ash render state is already borrowed")]
    AlreadyBorrowed,
}

#[repr(C)]
pub(crate) struct AshRenderStateStorage {
    // Keep the public Vulkan handles as an ABI-compatible prefix matching
    // ImGui_ImplVulkan_RenderState. Private Rust-only state follows that prefix.
    command_buffer: vk::CommandBuffer,
    pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
    device: NonNull<Device>,
    sampler_descriptor_set: vk::DescriptorSet,
    reset_count: u32,
    draw_commands_since_reset: u32,
    borrowed: Cell<bool>,
}

impl AshRenderStateStorage {
    pub(crate) fn new(
        device: &Device,
        command_buffer: vk::CommandBuffer,
        pipeline: vk::Pipeline,
        pipeline_layout: vk::PipelineLayout,
        sampler_descriptor_set: vk::DescriptorSet,
    ) -> Self {
        Self {
            command_buffer,
            pipeline,
            pipeline_layout,
            device: NonNull::from(device),
            sampler_descriptor_set,
            reset_count: 0,
            draw_commands_since_reset: 0,
            borrowed: Cell::new(false),
        }
    }

    pub(crate) fn set_sampler_descriptor_set(&mut self, descriptor_set: vk::DescriptorSet) {
        self.sampler_descriptor_set = descriptor_set;
    }

    pub(crate) fn record_reset(&mut self, linear_sampler_set: vk::DescriptorSet) {
        self.sampler_descriptor_set = linear_sampler_set;
        self.reset_count = self.reset_count.saturating_add(1);
        self.draw_commands_since_reset = 0;
    }

    pub(crate) fn record_draw_command(&mut self) {
        self.draw_commands_since_reset = self.draw_commands_since_reset.saturating_add(1);
    }
}

struct AshRenderStateBorrow<'storage>(&'storage Cell<bool>);

impl Drop for AshRenderStateBorrow<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

/// Scoped access to the Vulkan resources selected for a raw draw callback.
///
/// This corresponds to `ImGui_ImplVulkan_RenderState` in the official backend.
/// The value can only be obtained through [`Self::with_current`] while Ash is
/// invoking a raw callback, and none of its references can escape that scope.
/// Treat the raw `Renderer_RenderState` pointer as opaque in Rust code; this
/// scoped accessor also validates backend activity and exclusive borrowing.
///
/// ```compile_fail
/// use dear_imgui_ash::AshRenderState;
///
/// let _escaped = unsafe { AshRenderState::with_current(|state| state.device()) };
/// ```
#[derive(Debug)]
pub struct AshRenderState<'callback> {
    storage: NonNull<AshRenderStateStorage>,
    _callback: PhantomData<&'callback mut AshRenderStateStorage>,
    _ui_thread: PhantomData<Rc<()>>,
}

impl AshRenderState<'_> {
    /// Borrows the state published for the current raw draw callback.
    ///
    /// # Safety
    ///
    /// This function may only be called from a raw draw callback currently
    /// invoked by `dear-imgui-ash`. The current Dear ImGui Context must be the
    /// renderer owner, and the callback must not replace `Renderer_RenderState`.
    /// Vulkan commands recorded through the returned device must obey the active
    /// render-pass, pipeline-layout, synchronization, and resource-lifetime rules.
    pub unsafe fn with_current<R>(
        callback: impl for<'callback> FnOnce(AshRenderState<'callback>) -> R,
    ) -> Result<R, AshRenderStateAccessError> {
        let platform_io = unsafe { sys::igGetPlatformIO_Nil() };
        let raw_state = if platform_io.is_null() {
            None
        } else {
            NonNull::new(unsafe { (*platform_io).Renderer_RenderState })
        }
        .ok_or(AshRenderStateAccessError::Inactive)?;
        let storage = raw_state.cast::<AshRenderStateStorage>();
        let borrowed = unsafe { &storage.as_ref().borrowed };
        if borrowed.replace(true) {
            return Err(AshRenderStateAccessError::AlreadyBorrowed);
        }
        let _borrow = AshRenderStateBorrow(borrowed);
        Ok(callback(AshRenderState {
            storage,
            _callback: PhantomData,
            _ui_thread: PhantomData,
        }))
    }

    /// Returns the renderer device for the callback duration.
    pub fn device(&self) -> &Device {
        unsafe { self.storage.as_ref().device.as_ref() }
    }

    /// Returns the command buffer currently recording Dear ImGui draws.
    pub fn command_buffer(&self) -> vk::CommandBuffer {
        unsafe { self.storage.as_ref().command_buffer }
    }

    /// Returns the graphics pipeline selected for this draw scope.
    pub fn pipeline(&self) -> vk::Pipeline {
        unsafe { self.storage.as_ref().pipeline }
    }

    /// Returns the two-set pipeline layout selected for this draw scope.
    pub fn pipeline_layout(&self) -> vk::PipelineLayout {
        unsafe { self.storage.as_ref().pipeline_layout }
    }

    /// Returns the sampler descriptor set most recently bound by the renderer.
    pub fn sampler_descriptor_set(&self) -> vk::DescriptorSet {
        unsafe { self.storage.as_ref().sampler_descriptor_set }
    }

    /// Returns how many reset-render-state commands have executed in this draw scope.
    pub fn reset_count(&self) -> u32 {
        unsafe { self.storage.as_ref().reset_count }
    }

    /// Returns the number of indexed draw commands recorded since the latest reset command.
    pub fn draw_commands_since_reset(&self) -> u32 {
        unsafe { self.storage.as_ref().draw_commands_since_reset }
    }
}

#[inline(never)]
pub(super) unsafe extern "C" fn draw_callback_reset_render_state(
    _parent_list: *const sys::ImDrawList,
    _cmd: *const sys::ImDrawCmd,
) {
    let _ = unsafe { std::ptr::read_volatile(&RESET_RENDER_STATE_MARKER) };
}

#[inline(never)]
pub(super) unsafe extern "C" fn draw_callback_set_sampler_linear(
    _parent_list: *const sys::ImDrawList,
    _cmd: *const sys::ImDrawCmd,
) {
    let _ = unsafe { std::ptr::read_volatile(&SAMPLER_LINEAR_MARKER) };
}

#[inline(never)]
pub(super) unsafe extern "C" fn draw_callback_set_sampler_nearest(
    _parent_list: *const sys::ImDrawList,
    _cmd: *const sys::ImDrawCmd,
) {
    let _ = unsafe { std::ptr::read_volatile(&SAMPLER_NEAREST_MARKER) };
}

#[cfg(test)]
mod tests {
    use ash::vk::Handle;
    use dear_imgui_rs::{Context, FramePrepareOptions, render::DrawCmd};

    use super::*;

    #[repr(C)]
    struct UpstreamVulkanRenderStatePrefix {
        command_buffer: vk::CommandBuffer,
        pipeline: vk::Pipeline,
        pipeline_layout: vk::PipelineLayout,
    }

    #[test]
    fn callback_state_retains_the_upstream_vulkan_handle_prefix() {
        let device = unsafe { Device::load_with(|_| std::ptr::null(), vk::Device::null()) };
        let storage = AshRenderStateStorage::new(
            &device,
            vk::CommandBuffer::from_raw(11),
            vk::Pipeline::from_raw(12),
            vk::PipelineLayout::from_raw(13),
            vk::DescriptorSet::from_raw(14),
        );
        let prefix =
            unsafe { &*std::ptr::from_ref(&storage).cast::<UpstreamVulkanRenderStatePrefix>() };

        assert_eq!(prefix.command_buffer.as_raw(), 11);
        assert_eq!(prefix.pipeline.as_raw(), 12);
        assert_eq!(prefix.pipeline_layout.as_raw(), 13);
    }

    #[test]
    fn standard_draw_callbacks_retain_distinct_identity_and_classification() {
        type Callback = unsafe extern "C" fn(*const sys::ImDrawList, *const sys::ImDrawCmd);
        let callbacks: [Callback; 3] = [
            draw_callback_reset_render_state,
            draw_callback_set_sampler_linear,
            draw_callback_set_sampler_nearest,
        ];
        for left in 0..callbacks.len() {
            for right in (left + 1)..callbacks.len() {
                assert!(!std::ptr::fn_addr_eq(callbacks[left], callbacks[right]));
            }
        }

        let mut context = Context::create();
        let platform_io = context.platform_io_mut();
        unsafe {
            platform_io
                .set_draw_callback_reset_render_state_raw(Some(draw_callback_reset_render_state));
            platform_io
                .set_draw_callback_set_sampler_linear_raw(Some(draw_callback_set_sampler_linear));
            platform_io
                .set_draw_callback_set_sampler_nearest_raw(Some(draw_callback_set_sampler_nearest));
        }

        assert!(context.font_atlas().build());
        context.prepare_frame(FramePrepareOptions::new([64.0, 64.0], 1.0 / 60.0));
        let ui = context.frame();
        let draw_list = ui.get_background_draw_list();
        unsafe {
            draw_list.add_callback(draw_callback_reset_render_state, std::ptr::null_mut(), 0);
            draw_list.add_callback(draw_callback_set_sampler_linear, std::ptr::null_mut(), 0);
            draw_list.add_callback(draw_callback_set_sampler_nearest, std::ptr::null_mut(), 0);
        }
        drop(draw_list);
        let frame = context.render();
        assert_eq!(frame.draw_data().total_vtx_count(), 0);
        let commands = frame
            .draw_data()
            .draw_lists()
            .flat_map(|list| list.commands())
            .filter(|command| {
                matches!(
                    command,
                    DrawCmd::ResetRenderState
                        | DrawCmd::SetSamplerLinear
                        | DrawCmd::SetSamplerNearest
                )
            })
            .collect::<Vec<_>>();
        assert!(matches!(
            commands.as_slice(),
            [
                DrawCmd::ResetRenderState,
                DrawCmd::SetSamplerLinear,
                DrawCmd::SetSamplerNearest,
            ]
        ));
    }

    #[test]
    fn callback_state_is_scoped_non_reentrant_and_handle_complete() {
        let mut context = Context::create();
        assert!(matches!(
            unsafe { AshRenderState::with_current(|_| ()) },
            Err(AshRenderStateAccessError::Inactive)
        ));

        let device = unsafe { Device::load_with(|_| std::ptr::null(), vk::Device::null()) };
        let command_buffer = vk::CommandBuffer::from_raw(11);
        let pipeline = vk::Pipeline::from_raw(12);
        let pipeline_layout = vk::PipelineLayout::from_raw(13);
        let sampler_descriptor_set = vk::DescriptorSet::from_raw(14);
        let mut storage = AshRenderStateStorage::new(
            &device,
            command_buffer,
            pipeline,
            pipeline_layout,
            sampler_descriptor_set,
        );
        unsafe {
            context
                .platform_io_mut()
                .set_renderer_render_state(std::ptr::from_mut(&mut storage).cast());
        }

        unsafe {
            AshRenderState::with_current(|state| {
                assert!(std::ptr::eq(state.device(), &device));
                assert_eq!(state.command_buffer(), command_buffer);
                assert_eq!(state.pipeline(), pipeline);
                assert_eq!(state.pipeline_layout(), pipeline_layout);
                assert_eq!(state.sampler_descriptor_set(), sampler_descriptor_set);
                assert_eq!(state.reset_count(), 0);
                assert_eq!(state.draw_commands_since_reset(), 0);
                assert!(matches!(
                    AshRenderState::with_current(|_| ()),
                    Err(AshRenderStateAccessError::AlreadyBorrowed)
                ));
            })
            .unwrap();
            AshRenderState::with_current(|_| ()).unwrap();
            context
                .platform_io_mut()
                .set_renderer_render_state(std::ptr::null_mut());
        }
        assert!(matches!(
            unsafe { AshRenderState::with_current(|_| ()) },
            Err(AshRenderStateAccessError::Inactive)
        ));
    }
}

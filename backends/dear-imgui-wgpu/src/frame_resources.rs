//! Upload buffers and bindings for independently submitted render passes.

use std::{mem, sync::Arc};

use crate::{RenderResources, RendererError, RendererResult, UniformBuffer};
use dear_imgui_rs::render::{DrawIdx, DrawVert};
use wgpu::*;

const _: () = {
    assert!(mem::size_of::<DrawVert>() == 20);
    assert!(mem::offset_of!(DrawVert, pos) == 0);
    assert!(mem::offset_of!(DrawVert, uv) == 8);
    assert!(mem::offset_of!(DrawVert, col) == 16);
    assert!(mem::size_of::<[f32; 2]>() == 8);
    assert!(mem::size_of::<u32>() == 4);
};

fn vertex_bytes(vertices: &[DrawVert]) -> &[u8] {
    // SAFETY: DrawVert is repr(C) and its two f32 arrays and u32 fill the asserted layout.
    // All bytes are initialized, and the byte slice borrows the original vertex slice.
    unsafe { std::slice::from_raw_parts(vertices.as_ptr().cast(), mem::size_of_val(vertices)) }
}

#[derive(Default)]
struct DrawBuffer {
    buffer: Option<Buffer>,
    staging: Vec<u8>,
}

impl DrawBuffer {
    fn prepare<T>(
        &mut self,
        device: &Device,
        count: usize,
        kind: &'static str,
        label: &'static str,
        usage: BufferUsages,
    ) -> RendererResult<()> {
        self.staging.clear();
        if count == 0 {
            return Ok(());
        }

        let aligned_size = count
            .checked_mul(mem::size_of::<T>())
            .and_then(|size| size.checked_next_multiple_of(COPY_BUFFER_ALIGNMENT as usize))
            .ok_or(RendererError::DrawBufferTooLarge { buffer: kind })?;
        let required = aligned_size as BufferAddress;
        let capacity = self.buffer.as_ref().map_or(0, Buffer::size);
        if required > capacity {
            let max_size = device.limits().max_buffer_size;
            let max_aligned = max_size - max_size % COPY_BUFFER_ALIGNMENT;
            if required > max_aligned {
                return Err(RendererError::DrawBufferTooLarge { buffer: kind });
            }
            self.buffer = Some(device.create_buffer(&BufferDescriptor {
                label: Some(label),
                size: required.max(capacity.saturating_mul(2)).min(max_aligned),
                usage: usage | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        self.staging.reserve(aligned_size);
        Ok(())
    }

    fn upload(&mut self, queue: &Queue) {
        let Some(buffer) = self.buffer.as_ref().filter(|_| !self.staging.is_empty()) else {
            return;
        };
        let aligned_size = self
            .staging
            .len()
            .next_multiple_of(COPY_BUFFER_ALIGNMENT as usize);
        self.staging.resize(aligned_size, 0);
        queue.write_buffer(buffer, 0, &self.staging);
    }

    fn active_buffer(&self) -> Option<&Buffer> {
        self.buffer.as_ref().filter(|_| !self.staging.is_empty())
    }
}

pub(crate) struct FrameResources {
    vertices: DrawBuffer,
    indices: DrawBuffer,
    uniform_buffer: Option<UniformBuffer>,
    nearest_common_bind_group: Option<BindGroup>,
    recorded_pass: Arc<()>,
}

impl FrameResources {
    fn new() -> Self {
        Self {
            vertices: DrawBuffer::default(),
            indices: DrawBuffer::default(),
            uniform_buffer: None,
            nearest_common_bind_group: None,
            recorded_pass: Arc::new(()),
        }
    }

    pub(crate) fn ensure_render_bindings(
        &mut self,
        device: &Device,
        render_resources: &RenderResources,
    ) -> RendererResult<()> {
        if self.uniform_buffer.is_none() || self.nearest_common_bind_group.is_none() {
            let (uniform, nearest) = render_resources.create_frame_bindings(device)?;
            self.uniform_buffer = Some(uniform);
            self.nearest_common_bind_group = Some(nearest);
        }
        Ok(())
    }

    pub(crate) fn upload_draw_lists<'draw>(
        &mut self,
        device: &Device,
        queue: &Queue,
        [vertex_count, index_count]: [usize; 2],
        lists: impl IntoIterator<Item = (&'draw [DrawVert], &'draw [DrawIdx])>,
    ) -> RendererResult<()> {
        self.vertices.prepare::<DrawVert>(
            device,
            vertex_count,
            "vertex",
            "Dear ImGui Vertex Buffer",
            BufferUsages::VERTEX,
        )?;
        self.indices.prepare::<DrawIdx>(
            device,
            index_count,
            "index",
            "Dear ImGui Index Buffer",
            BufferUsages::INDEX,
        )?;
        for (vertices, indices) in lists {
            self.vertices
                .staging
                .extend_from_slice(vertex_bytes(vertices));
            self.indices
                .staging
                .extend_from_slice(bytemuck::cast_slice(indices));
        }
        self.vertices.upload(queue);
        self.indices.upload(queue);
        Ok(())
    }

    pub(crate) fn vertex_buffer(&self) -> Option<&Buffer> {
        self.vertices.active_buffer()
    }

    pub(crate) fn index_buffer(&self) -> Option<&Buffer> {
        self.indices.active_buffer()
    }

    pub(crate) fn uniform_buffer(&self) -> RendererResult<&UniformBuffer> {
        self.uniform_buffer.as_ref().ok_or_else(|| {
            RendererError::InvalidRenderState("Frame uniform buffer not initialized".to_owned())
        })
    }

    pub(crate) fn nearest_common_bind_group(&self) -> RendererResult<&BindGroup> {
        self.nearest_common_bind_group.as_ref().ok_or_else(|| {
            RendererError::InvalidRenderState(
                "Frame nearest sampler bind group not initialized".to_owned(),
            )
        })
    }

    pub(crate) fn recording_lease(&self) -> Arc<()> {
        Arc::clone(&self.recorded_pass)
    }
}

pub(crate) struct FrameResourceArena {
    passes: Vec<FrameResources>,
    next_pass: usize,
}

impl FrameResourceArena {
    pub(crate) const fn new() -> Self {
        Self {
            passes: Vec::new(),
            next_pass: 0,
        }
    }

    pub(crate) fn begin_epoch(&mut self) {
        self.next_pass = 0;
    }

    pub(crate) fn acquire(&mut self) -> &mut FrameResources {
        // An epoch transition does not imply that the caller submitted its encoder. A pass
        // keeps its slot reserved until its encoder is discarded or its GPU work completes.
        if let Some(offset) = self.passes[self.next_pass..]
            .iter_mut()
            .position(|frame| Arc::get_mut(&mut frame.recorded_pass).is_some())
        {
            self.next_pass += offset + 1;
        } else {
            self.passes.push(FrameResources::new());
            self.next_pass = self.passes.len();
        }
        &mut self.passes[self.next_pass - 1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_upload_reuses_staging_and_pads_only_after_the_last_list() -> RendererResult<()> {
        let Some((device, queue)) = crate::texture::request_test_device() else {
            return Ok(());
        };
        let mut frame = FrameResources::new();
        let initial = [DrawIdx::MAX; 4];
        frame.upload_draw_lists(&device, &queue, [0, 4], [(&[][..], &initial[..])])?;
        let buffer = frame.index_buffer().unwrap().clone();
        let staging = frame.indices.staging.as_ptr();

        let indices = [DrawIdx::MAX, 1, 2];
        frame.upload_draw_lists(
            &device,
            &queue,
            [0, indices.len()],
            [(&[][..], &indices[..1]), (&[][..], &indices[1..])],
        )?;

        let host = &frame.indices.staging;
        let expected: &[u8] = bytemuck::cast_slice(&indices);
        assert_eq!(frame.index_buffer(), Some(&buffer));
        assert_eq!(host.as_ptr(), staging);
        assert_eq!(
            host.len(),
            expected
                .len()
                .next_multiple_of(COPY_BUFFER_ALIGNMENT as usize)
        );
        assert_eq!(&host[..expected.len()], expected);
        assert!(host[expected.len()..].iter().all(|&byte| byte == 0));
        Ok(())
    }

    #[test]
    fn frame_arena_only_reuses_released_slots_across_epochs() {
        let mut arena = FrameResourceArena::new();
        let first = arena.acquire().recording_lease();
        let second = arena.acquire().recording_lease();
        assert!(!Arc::ptr_eq(&first, &second));

        arena.begin_epoch();
        let third = arena.acquire().recording_lease();
        assert!(!Arc::ptr_eq(&first, &third));
        assert!(!Arc::ptr_eq(&second, &third));

        let released = Arc::as_ptr(&first);
        drop(first);
        arena.begin_epoch();
        assert_eq!(Arc::as_ptr(&arena.acquire().recording_lease()), released);
        assert_eq!(arena.passes.len(), 3);
    }
}

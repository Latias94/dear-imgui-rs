use std::collections::HashMap;

use dear_imgui_rs::render::{
    SnapshotTextureId, TextureFeedback, TextureOp, TextureRequest, TextureUploadRect,
};
use dear_imgui_rs::{
    OwnedTextureData, TextureData, TextureFormat, TextureRect, TextureStatus,
    get_format_bytes_per_pixel,
};

use crate::core::{Sdl3BackendError, ffi};

#[derive(Default)]
pub(super) struct RendererTextureStore {
    textures: HashMap<SnapshotTextureId, RendererTexture>,
    /// Identities sealed by Destroy, paired with their latest request epoch.
    destroyed: HashMap<SnapshotTextureId, u64>,
}

impl std::fmt::Debug for RendererTextureStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RendererTextureStore")
            .field("textures", &self.textures)
            .field("destroyed", &self.destroyed)
            .finish()
    }
}

#[derive(Debug)]
pub(super) struct ProcessedTextureRequests {
    pub(super) feedback: Vec<TextureFeedback>,
    pub(super) installed: Vec<SnapshotTextureId>,
    pub(super) error: Option<Sdl3BackendError>,
}

struct RendererTexture {
    data: OwnedTextureData,
    /// True after this proxy's renderer ID has been reconciled into Context-owned texture data.
    installed: bool,
}

impl std::fmt::Debug for RendererTexture {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RendererTexture")
            .field("format", &self.data.format())
            .field("width", &self.data.width())
            .field("height", &self.data.height())
            .field("texture_id", &self.data.tex_id())
            .field("installed", &self.installed)
            .finish_non_exhaustive()
    }
}

impl RendererTextureStore {
    #[cfg(test)]
    pub(super) fn insert_uninstalled_for_test(&mut self, texture: SnapshotTextureId) {
        use dear_imgui_rs::TextureId;

        let mut data =
            OwnedTextureData::from_pixels(TextureFormat::RGBA32, 1, 1, &[255, 255, 255, 255])
                .unwrap();
        unsafe {
            data.set_tex_id(TextureId::new(77));
            data.set_status(TextureStatus::OK);
        }
        self.textures.insert(
            texture,
            RendererTexture {
                data,
                installed: false,
            },
        );
    }

    pub(super) fn process_requests(
        &mut self,
        requests: &[TextureRequest],
        request_epoch: u64,
        mut update_texture: impl FnMut(&mut TextureData),
    ) -> ProcessedTextureRequests {
        let mut processed = ProcessedTextureRequests {
            feedback: Vec::with_capacity(requests.len()),
            installed: Vec::new(),
            error: None,
        };
        let mut requests = requests.iter();
        while let Some(request) = requests.next() {
            match self.process_request(
                request,
                request_epoch,
                &mut update_texture,
                &mut processed.installed,
            ) {
                Ok(feedback) => processed.feedback.push(feedback),
                Err(error) => {
                    // Earlier destroys already freed their native IDs. Commit their feedback
                    // before returning the error so teardown cannot destroy those IDs again.
                    processed.error = Some(error);
                    processed.feedback.push(request.retry());
                    processed
                        .feedback
                        .extend(requests.map(TextureRequest::retry));
                    break;
                }
            }
        }
        processed
    }

    fn process_request(
        &mut self,
        request: &TextureRequest,
        request_epoch: u64,
        update_texture: &mut impl FnMut(&mut TextureData),
        installed: &mut Vec<SnapshotTextureId>,
    ) -> Result<TextureFeedback, Sdl3BackendError> {
        let texture = request.texture();
        if self.destroyed.contains_key(&texture)
            && !matches!(request.operation(), TextureOp::Destroy)
        {
            return Ok(request.superseded());
        }
        let id = match request.operation() {
            TextureOp::Create {
                format,
                width,
                height,
                row_pitch,
                pixels,
            } => {
                let pixels =
                    copy_full_upload(texture, *format, *width, *height, *row_pitch, pixels)?;
                let mut data = OwnedTextureData::from_pixels(*format, *width, *height, &pixels)
                    .map_err(|_| Sdl3BackendError::InvalidTextureRequest {
                        texture,
                        reason: "texture payload failed owned texture validation",
                    })?;
                self.destroy_existing(texture, update_texture)?;
                update_texture(&mut data);
                ensure_upload_completed(texture, &data)?;
                let id = data.tex_id();
                self.textures.insert(
                    texture,
                    RendererTexture {
                        data,
                        installed: false,
                    },
                );
                id
            }
            TextureOp::Update {
                format,
                width,
                height,
                rects,
            } => {
                let proxy = self
                    .textures
                    .get_mut(&texture)
                    .ok_or(Sdl3BackendError::ManagedTextureNotCreated { texture })?;
                if proxy.data.format() != *format
                    || proxy.data.width() != *width
                    || proxy.data.height() != *height
                {
                    return Err(Sdl3BackendError::InvalidTextureRequest {
                        texture,
                        reason: "update metadata does not match the renderer allocation",
                    });
                }
                upload_rects(texture, &mut proxy.data, rects, update_texture)?;
                proxy.data.tex_id()
            }
            TextureOp::Destroy => {
                self.destroyed
                    .entry(texture)
                    .and_modify(|epoch| *epoch = (*epoch).max(request_epoch))
                    .or_insert(request_epoch);
                self.destroy_existing(texture, update_texture)?;
                return Ok(request.destroyed()?);
            }
        };
        installed.push(texture);
        Ok(request.uploaded(id)?)
    }

    /// Mark request-created proxies as visible to Context after feedback was reconciled.
    pub(super) fn mark_reconciled(&mut self, installed: &[SnapshotTextureId]) {
        for texture in installed {
            if let Some(proxy) = self.textures.get_mut(texture) {
                proxy.installed = true;
            }
        }
    }

    /// Destroy proxies that were never installed into Context-owned texture data.
    ///
    /// Upstream SDL backends only discover installed IDs during their global teardown. Every
    /// uninstalled proxy therefore needs an explicit native destroy transition while the backend
    /// updater is still alive.
    pub(super) fn destroy_uninstalled(
        &mut self,
        mut update_texture: impl FnMut(&mut TextureData),
    ) -> Result<(), Sdl3BackendError> {
        let uninstalled = self
            .textures
            .iter()
            .filter_map(|(texture, proxy)| (!proxy.installed).then_some(*texture))
            .collect::<Vec<_>>();
        for texture in uninstalled {
            self.destroy_existing(texture, &mut update_texture)?;
        }
        Ok(())
    }

    /// Upstream device teardown destroys the native IDs mirrored by these proxy records.
    pub(super) fn forget_destroyed_by_upstream(&mut self) {
        debug_assert!(
            self.textures.values().all(|proxy| proxy.installed),
            "uninstalled SDL3 proxy must be explicitly destroyed before upstream teardown"
        );
        self.textures.clear();
    }

    pub(super) fn clear_destroyed(&mut self) {
        self.destroyed.clear();
    }

    pub(super) fn prune_destroyed(&mut self, completion_watermark: u64) {
        self.destroyed
            .retain(|_, destroy_epoch| *destroy_epoch > completion_watermark);
    }

    fn destroy_existing(
        &mut self,
        texture: SnapshotTextureId,
        update_texture: &mut impl FnMut(&mut TextureData),
    ) -> Result<(), Sdl3BackendError> {
        let Some(proxy) = self.textures.get_mut(&texture) else {
            return Ok(());
        };
        destroy_proxy(texture, proxy, update_texture)?;
        self.textures.remove(&texture);
        Ok(())
    }
}

fn destroy_proxy(
    texture: SnapshotTextureId,
    proxy: &mut RendererTexture,
    update_texture: &mut impl FnMut(&mut TextureData),
) -> Result<(), Sdl3BackendError> {
    if proxy.data.status() == TextureStatus::Destroyed {
        return Ok(());
    }
    unsafe {
        // The proxy is owned exclusively by this renderer store and is never registered with a
        // Context. The request-bound destroy already passed the managed retirement fence.
        proxy.data.set_status(TextureStatus::WantDestroy);
        // Upstream OpenGL3 and SDLGPU3 intentionally delay destruction until a texture has gone
        // unused. A request-bound destroy is already past that retirement fence.
        (*proxy.data.as_raw_mut()).UnusedFrames = 1;
        (*proxy.data.as_raw_mut()).WantDestroyNextFrame = true;
    }
    update_texture(&mut proxy.data);
    if proxy.data.status() != TextureStatus::Destroyed || !proxy.data.tex_id().is_null() {
        return Err(Sdl3BackendError::TextureOperationFailed {
            texture,
            operation: "destroy",
        });
    }
    Ok(())
}

fn ensure_upload_completed(
    texture: SnapshotTextureId,
    proxy: &TextureData,
) -> Result<(), Sdl3BackendError> {
    if proxy.status() != TextureStatus::OK || proxy.tex_id().is_null() {
        return Err(Sdl3BackendError::TextureOperationFailed {
            texture,
            operation: "upload",
        });
    }
    Ok(())
}

fn copy_full_upload(
    texture: SnapshotTextureId,
    format: TextureFormat,
    width: u32,
    height: u32,
    row_pitch: usize,
    source: &[u8],
) -> Result<Vec<u8>, Sdl3BackendError> {
    ensure_supported_format(texture, format)?;
    let row_bytes = usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(get_format_bytes_per_pixel(format)))
        .ok_or(Sdl3BackendError::InvalidTextureRequest {
            texture,
            reason: "row byte count overflowed usize",
        })?;
    if row_pitch < row_bytes {
        return Err(Sdl3BackendError::InvalidTextureRequest {
            texture,
            reason: "row pitch is smaller than one texture row",
        });
    }
    let height = usize::try_from(height).map_err(|_| Sdl3BackendError::InvalidTextureRequest {
        texture,
        reason: "texture height does not fit usize",
    })?;
    let source_len =
        row_pitch
            .checked_mul(height)
            .ok_or(Sdl3BackendError::InvalidTextureRequest {
                texture,
                reason: "source byte count overflowed usize",
            })?;
    let destination_len =
        row_bytes
            .checked_mul(height)
            .ok_or(Sdl3BackendError::InvalidTextureRequest {
                texture,
                reason: "destination byte count overflowed usize",
            })?;
    if source.len() < source_len {
        return Err(Sdl3BackendError::InvalidTextureRequest {
            texture,
            reason: "create upload is shorter than its declared layout",
        });
    }
    let mut destination = vec![0; destination_len];
    for row in 0..height {
        let source_start = row * row_pitch;
        let destination_start = row * row_bytes;
        destination[destination_start..destination_start + row_bytes]
            .copy_from_slice(&source[source_start..source_start + row_bytes]);
    }
    Ok(destination)
}

fn upload_rects(
    texture: SnapshotTextureId,
    proxy: &mut OwnedTextureData,
    rects: &[TextureUploadRect],
    update_texture: &mut impl FnMut(&mut TextureData),
) -> Result<(), Sdl3BackendError> {
    let invalid = |reason| Sdl3BackendError::InvalidTextureRequest { texture, reason };
    if !matches!(
        proxy.status(),
        TextureStatus::OK | TextureStatus::WantUpdates
    ) || proxy.tex_id().is_null()
        || proxy.pixels().is_none()
    {
        return Err(invalid("update requires a live renderer allocation"));
    }
    let repair = (proxy.status() == TextureStatus::WantUpdates).then(|| proxy.update_rect());
    if rects.is_empty() && repair.is_none() {
        return Ok(());
    }
    let bpp = proxy.bytes_per_pixel();
    for upload in rects {
        let TextureRect { x, y, w, h } = upload.rect;
        if w == 0
            || h == 0
            || u32::from(x) + u32::from(w) > proxy.width()
            || u32::from(y) + u32::from(h) > proxy.height()
        {
            return Err(invalid("update rectangle exceeds the renderer allocation"));
        }
        let row_bytes = usize::from(w) * bpp;
        let required = upload
            .row_pitch
            .checked_mul(usize::from(h) - 1)
            .and_then(|prefix| prefix.checked_add(row_bytes));
        if upload.row_pitch < row_bytes || required.is_none_or(|len| upload.data.len() < len) {
            return Err(invalid("invalid texture update row pitch or pixel payload"));
        }
    }

    // A failed native call may already have written part of its region. Re-upload the
    // committed pixels there as well, even if the next request touches a different region.
    let mut native_rects = Vec::with_capacity(rects.len());
    let (mut left, mut top, mut right, mut bottom) = (u16::MAX, u16::MAX, 0, 0);
    for rect in rects.iter().map(|upload| upload.rect).chain(repair) {
        left = left.min(rect.x);
        top = top.min(rect.y);
        right = right.max(u32::from(rect.x) + u32::from(rect.w));
        bottom = bottom.max(u32::from(rect.y) + u32::from(rect.h));
        native_rects.push(rect.into());
    }
    let bounds = TextureRect {
        x: left,
        y: top,
        w: u16::try_from(right - u32::from(left))
            .map_err(|_| invalid("update bounding box is too wide"))?,
        h: u16::try_from(bottom - u32::from(top))
            .map_err(|_| invalid("update bounding box is too tall"))?,
    };
    let count =
        i32::try_from(native_rects.len()).map_err(|_| invalid("too many update rectangles"))?;
    // Swap in just the dirty rows and keep their old contents until the native updater succeeds.
    let mut patches: Vec<_> = rects.iter().map(|upload| upload.data.clone()).collect();
    for (upload, patch) in rects.iter().zip(&mut patches) {
        swap_upload_pixels(proxy, upload, patch);
    }
    // SAFETY: the store exclusively owns this native proxy. SDLRenderer3 and OpenGL3 read
    // Updates, while SDLGPU3 reads UpdateRect. The wrapper copies the validated rectangles.
    unsafe {
        let raw = proxy.as_raw_mut();
        ffi::dear_imgui_sdl3_backend_set_texture_updates(raw, native_rects.as_ptr(), count);
        (*raw).UpdateRect = bounds.into();
        proxy.set_status(TextureStatus::WantUpdates);
    }
    update_texture(proxy);
    if let Err(error) = ensure_upload_completed(texture, proxy) {
        // Reverse order also restores overlapping updates. Keep the dirty bounds for repair.
        for (upload, patch) in rects.iter().zip(&mut patches).rev() {
            swap_upload_pixels(proxy, upload, patch);
        }
        unsafe { proxy.set_status(TextureStatus::WantUpdates) };
        return Err(error);
    }
    Ok(())
}

fn swap_upload_pixels(proxy: &mut OwnedTextureData, upload: &TextureUploadRect, patch: &mut [u8]) {
    let rect = upload.rect;
    let pitch = proxy.pitch();
    let bpp = proxy.bytes_per_pixel();
    let row_bytes = usize::from(rect.w) * bpp;
    for row in 0..usize::from(rect.h) {
        let offset = (usize::from(rect.y) + row) * pitch + usize::from(rect.x) * bpp;
        // SAFETY: upload_rects validated every row against this owned RGBA allocation.
        // The scratch bytes are independently allocated, and the row borrow ends before FFI.
        let pixels = unsafe {
            std::slice::from_raw_parts_mut((*proxy.as_raw_mut()).Pixels.add(offset), row_bytes)
        };
        let start = row * upload.row_pitch;
        pixels.swap_with_slice(&mut patch[start..start + row_bytes]);
    }
}

fn ensure_supported_format(
    texture: SnapshotTextureId,
    format: TextureFormat,
) -> Result<(), Sdl3BackendError> {
    if format != TextureFormat::RGBA32 {
        return Err(Sdl3BackendError::UnsupportedTextureFormat { texture, format });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dear_imgui_rs::render::{SnapshotTextureId, TextureRequestKind};
    use dear_imgui_rs::{BackendFlags, TextureId, TextureRegion, TextureSubresource};

    fn fake_update(texture: &mut TextureData) {
        match texture.status() {
            TextureStatus::WantCreate | TextureStatus::WantUpdates => unsafe {
                // The test updater exclusively owns the unregistered proxy texture.
                texture.set_tex_id(TextureId::new(77));
                texture.set_status(TextureStatus::OK);
            },
            TextureStatus::WantDestroy => unsafe {
                // The test updater has completed the proxy's requested destroy transition.
                texture.set_status(TextureStatus::Destroyed);
            },
            TextureStatus::OK | TextureStatus::Destroyed => {}
        }
    }

    fn process_matching_requests(
        store: &mut RendererTextureStore,
        requests: &[TextureRequest],
        request_epoch: u64,
        texture: SnapshotTextureId,
        mut update_texture: impl FnMut(&mut TextureData),
    ) -> Result<ProcessedTextureRequests, Sdl3BackendError> {
        let mut feedback = Vec::new();
        let mut installed = Vec::new();
        for request in requests {
            if request.texture() != texture {
                feedback.push(request.superseded());
                continue;
            }
            feedback.push(store.process_request(
                request,
                request_epoch,
                &mut update_texture,
                &mut installed,
            )?);
        }
        Ok(ProcessedTextureRequests {
            feedback,
            installed,
            error: None,
        })
    }

    #[test]
    fn full_upload_removes_source_row_padding() {
        let _guard = crate::tests::test_guard();
        let texture = SnapshotTextureId::FontAtlas {
            context: {
                let context = dear_imgui_rs::Context::create();
                context.id()
            },
            stamp: 1,
            generation: 1,
        };
        let pixels = copy_full_upload(
            texture,
            TextureFormat::RGBA32,
            1,
            2,
            8,
            &[1, 2, 3, 4, 9, 9, 9, 9, 5, 6, 7, 8, 9, 9, 9, 9],
        )
        .unwrap();
        assert_eq!(pixels, [1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn invalid_batch_preserves_pixels_and_failed_native_upload_is_repaired_on_retry() {
        let _guard = crate::tests::test_guard();
        let context = dear_imgui_rs::Context::create();
        let key = SnapshotTextureId::FontAtlas {
            context: context.id(),
            stamp: 1,
            generation: 1,
        };
        let mut proxy =
            OwnedTextureData::from_pixels(TextureFormat::RGBA32, 4, 1, &[0; 16]).unwrap();
        fake_update(&mut proxy);
        let pixel = |x, value| TextureUploadRect {
            rect: TextureRect {
                x,
                y: 0,
                w: 1,
                h: 1,
            },
            row_pitch: 4,
            data: vec![value; 4],
        };
        let empty_bounds = proxy.update_rect();
        let mut short = pixel(1, 2);
        short.data.pop();
        for invalid in [short, pixel(4, 2)] {
            assert!(
                upload_rects(key, &mut proxy, &[pixel(0, 1), invalid], &mut |_| panic!(
                    "invalid batch reached native updater"
                ))
                .is_err()
            );
            assert_eq!(proxy.pixels().unwrap(), [0; 16]);
            assert_eq!(proxy.status(), TextureStatus::OK);
            assert_eq!(proxy.update_rect(), empty_bounds);
            assert_eq!(proxy.updates().count(), 0);
        }

        let mut gpu = [0; 16];
        let failed = [pixel(0, 1), pixel(0, 2), pixel(1, 3)];
        let result = upload_rects(key, &mut proxy, &failed, &mut |data| {
            // Simulate a native updater that writes one pixel and never acknowledges the batch.
            gpu[..4].copy_from_slice(&data.pixels().unwrap()[..4]);
        });
        assert!(matches!(
            result,
            Err(Sdl3BackendError::TextureOperationFailed { .. })
        ));
        assert_eq!(gpu[..4], [2; 4]);
        assert_eq!(proxy.pixels().unwrap(), [0; 16]);
        assert_eq!(proxy.status(), TextureStatus::WantUpdates);

        upload_rects(key, &mut proxy, &[pixel(3, 9)], &mut |data| {
            assert_eq!(
                data.update_rect(),
                TextureRect {
                    x: 0,
                    y: 0,
                    w: 4,
                    h: 1
                }
            );
            for rect in data.updates() {
                let start = usize::from(rect.x) * 4;
                let end = start + usize::from(rect.w) * 4;
                gpu[start..end].copy_from_slice(&data.pixels().unwrap()[start..end]);
            }
            fake_update(data);
        })
        .unwrap();
        assert_eq!(gpu[..12], [0; 12]);
        assert_eq!(gpu[12..], [9; 4]);
        assert_eq!(proxy.pixels().unwrap(), gpu);
        assert_eq!(proxy.status(), TextureStatus::OK);
    }

    #[test]
    fn proxy_store_reconciles_request_bound_create_update_and_destroy() {
        let _guard = crate::tests::test_guard();
        let mut context = dear_imgui_rs::Context::create();
        context.io_mut().set_display_size([128.0, 128.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .io_mut()
            .set_backend_flags(BackendFlags::RENDERER_HAS_TEXTURES);

        let texture = OwnedTextureData::from_pixels(TextureFormat::RGBA32, 4, 2, &[0; 32]).unwrap();
        let texture_id = context.register_texture(texture);
        let consumer = context.create_synchronous_renderer_consumer().unwrap();
        let mut store = RendererTextureStore::default();

        let frame = context.begin_frame();
        frame.ui().image(texture_id, [2.0, 1.0]);
        let pending = frame.render(&consumer);
        assert!(pending.texture_requests().iter().any(|request| {
            request.texture() == SnapshotTextureId::User(texture_id)
                && request.kind() == TextureRequestKind::Create
        }));
        let processed = store.process_requests(
            pending.texture_requests(),
            pending.epoch().sequence(),
            fake_update,
        );
        assert!(processed.error.is_none(), "{:?}", processed.error);
        let reconciled = pending
            .reconcile_texture_feedback(processed.feedback)
            .unwrap();
        store.mark_reconciled(&processed.installed);
        drop(reconciled);
        assert_eq!(
            context
                .with_texture(texture_id, |texture| texture.texture_id())
                .unwrap(),
            TextureId::new(77)
        );

        let mut expected_pixels = [0; 32];
        for (x, y, color) in [(3_u16, 1_u16, [8, 7, 6, 5]), (1, 0, [4, 3, 2, 1])] {
            // Let Dear ImGui retire the previous acknowledged update from the source texture.
            drop(context.begin_frame());
            let region = TextureRegion::new(x.into(), y.into(), 1, 1).unwrap();
            context
                .try_with_texture_mut(texture_id, |mut texture| {
                    texture.update_subresource(TextureSubresource::new(region, 4, &color))
                })
                .unwrap();
            let offset = (usize::from(y) * 4 + usize::from(x)) * 4;
            expected_pixels[offset..offset + 4].copy_from_slice(&color);

            let frame = context.begin_frame();
            frame.ui().image(texture_id, [4.0, 2.0]);
            let pending = frame.render(&consumer);
            let mut uploaded = false;
            let processed = store.process_requests(
                pending.texture_requests(),
                pending.epoch().sequence(),
                |proxy| {
                    if (proxy.width(), proxy.height()) == (4, 2) {
                        let rect = TextureRect { x, y, w: 1, h: 1 };
                        assert_eq!(proxy.status(), TextureStatus::WantUpdates);
                        assert_eq!(
                            proxy.update_rect(),
                            rect,
                            "SDLGPU3 uploads this bounding box"
                        );
                        assert!(
                            proxy.updates().eq([rect]),
                            "SDLRenderer3 and OpenGL3 upload these rectangles"
                        );
                        assert_eq!(proxy.pixels().unwrap(), expected_pixels);
                        uploaded = true;
                    }
                    fake_update(proxy);
                },
            );
            assert!(processed.error.is_none(), "{:?}", processed.error);
            assert!(uploaded);
            let reconciled = pending
                .reconcile_texture_feedback(processed.feedback)
                .unwrap();
            store.mark_reconciled(&processed.installed);
            drop(reconciled);
        }

        context.remove_texture(texture_id).unwrap();
        let pending = context.begin_frame().render(&consumer);
        assert!(pending.texture_requests().iter().any(|request| {
            request.texture() == SnapshotTextureId::User(texture_id)
                && request.kind() == TextureRequestKind::Destroy
        }));
        let processed = store.process_requests(
            pending.texture_requests(),
            pending.epoch().sequence(),
            fake_update,
        );
        assert!(processed.error.is_none(), "{:?}", processed.error);
        let reconciled = pending
            .reconcile_texture_feedback(processed.feedback)
            .unwrap();
        store.mark_reconciled(&processed.installed);
        drop(reconciled);
        assert!(
            !store
                .textures
                .contains_key(&SnapshotTextureId::User(texture_id))
        );
    }

    #[test]
    fn failed_batch_acknowledges_completed_destroys_before_retry() {
        let _guard = crate::tests::test_guard();
        let mut context = dear_imgui_rs::Context::create();
        context.prepare_frame(
            dear_imgui_rs::FramePrepareOptions::new([128.0, 128.0], 1.0 / 60.0)
                .renderer_has_textures(),
        );
        let consumer = context.create_synchronous_renderer_consumer().unwrap();
        let mut store = RendererTextureStore::default();
        let ids = [1, 2].map(|v| {
            context.register_texture(
                OwnedTextureData::from_pixels(TextureFormat::RGBA32, 1, 1, &[v; 4]).unwrap(),
            )
        });
        let frame = context.begin_frame();
        for id in ids {
            frame.ui().image(id, [1.0, 1.0]);
        }
        let pending = frame.render(&consumer);
        let processed = store.process_requests(
            pending.texture_requests(),
            pending.epoch().sequence(),
            fake_update,
        );
        assert!(processed.error.is_none());
        drop(
            pending
                .reconcile_texture_feedback(processed.feedback)
                .unwrap(),
        );
        store.mark_reconciled(&processed.installed);
        for id in ids {
            context.remove_texture(id).unwrap();
        }
        let pending = context.begin_frame().render(&consumer);
        let destroys: Vec<_> = pending
            .texture_requests()
            .iter()
            .filter(|request| request.kind() == TextureRequestKind::Destroy)
            .map(TextureRequest::texture)
            .collect();
        assert_eq!(destroys.len(), 2);
        let mut destroyed_one = false;
        let processed = store.process_requests(
            pending.texture_requests(),
            pending.epoch().sequence(),
            |proxy| {
                if proxy.status() == TextureStatus::WantDestroy {
                    if destroyed_one {
                        return;
                    }
                    destroyed_one = true;
                }
                fake_update(proxy);
            },
        );
        assert!(
            matches!(processed.error, Some(Sdl3BackendError::TextureOperationFailed { texture, .. }) if texture == destroys[1])
        );
        drop(
            pending
                .reconcile_texture_feedback(processed.feedback)
                .unwrap(),
        );

        let retry = context.begin_frame().render(&consumer);
        let remaining: Vec<_> = retry
            .texture_requests()
            .iter()
            .filter(|request| request.kind() == TextureRequestKind::Destroy)
            .map(TextureRequest::texture)
            .collect();
        assert_eq!(remaining, [destroys[1]]);
        let processed = store.process_requests(
            retry.texture_requests(),
            retry.epoch().sequence(),
            fake_update,
        );
        assert!(processed.error.is_none());
        drop(
            retry
                .reconcile_texture_feedback(processed.feedback)
                .unwrap(),
        );
    }

    #[test]
    fn upstream_teardown_forgets_proxy_bindings_without_destroying_them_twice() {
        let _guard = crate::tests::test_guard();
        let texture = SnapshotTextureId::FontAtlas {
            context: {
                let context = dear_imgui_rs::Context::create();
                context.id()
            },
            stamp: 1,
            generation: 1,
        };
        let mut data =
            OwnedTextureData::from_pixels(TextureFormat::RGBA32, 1, 1, &[1, 2, 3, 4]).unwrap();
        fake_update(&mut data);
        let mut store = RendererTextureStore::default();
        store.textures.insert(
            texture,
            RendererTexture {
                data,
                installed: true,
            },
        );

        store.forget_destroyed_by_upstream();

        assert!(store.textures.is_empty());
    }

    #[test]
    fn uninstalled_proxy_is_destroyed_through_the_native_updater() {
        let _guard = crate::tests::test_guard();
        let texture = SnapshotTextureId::FontAtlas {
            context: {
                let context = dear_imgui_rs::Context::create();
                context.id()
            },
            stamp: 2,
            generation: 1,
        };
        let mut data =
            OwnedTextureData::from_pixels(TextureFormat::RGBA32, 1, 1, &[1, 2, 3, 4]).unwrap();
        fake_update(&mut data);
        let mut store = RendererTextureStore::default();
        store.textures.insert(
            texture,
            RendererTexture {
                data,
                installed: false,
            },
        );
        let mut destroy_calls = 0;

        store
            .destroy_uninstalled(|proxy| {
                if proxy.status() == TextureStatus::WantDestroy {
                    destroy_calls += 1;
                }
                fake_update(proxy);
            })
            .unwrap();

        assert_eq!(destroy_calls, 1);
        assert!(store.textures.is_empty());
    }

    #[test]
    fn destroyed_identity_supersedes_late_uploads_until_contiguous_completion() {
        let _guard = crate::tests::test_guard();
        let mut context = dear_imgui_rs::Context::create();
        context.io_mut().set_display_size([128.0, 128.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .io_mut()
            .set_backend_flags(BackendFlags::RENDERER_HAS_TEXTURES);
        let texture =
            OwnedTextureData::from_pixels(TextureFormat::RGBA32, 1, 1, &[1, 2, 3, 4]).unwrap();
        let texture_id = context.register_texture(texture);
        let consumer = context.create_synchronous_renderer_consumer().unwrap();
        let frame = context.begin_frame();
        frame.ui().image(texture_id, [1.0, 1.0]);
        let pending = frame.render(&consumer);
        let snapshot_id = SnapshotTextureId::User(texture_id);
        let request = pending
            .texture_requests()
            .iter()
            .find(|request| request.texture() == snapshot_id)
            .expect("frame should contain the user texture create request");
        let mut store = RendererTextureStore::default();
        store.destroyed.insert(snapshot_id, 5);

        let processed = store.process_requests(std::slice::from_ref(request), 3, |_| {
            panic!("retired upload reached the native updater")
        });
        assert!(processed.error.is_none(), "{:?}", processed.error);
        assert_eq!(processed.feedback.len(), 1);
        assert!(processed.installed.is_empty());
        assert!(store.textures.is_empty());

        store.prune_destroyed(4);
        assert_eq!(store.destroyed.get(&snapshot_id), Some(&5));
        store.prune_destroyed(5);
        assert!(!store.destroyed.contains_key(&snapshot_id));
    }

    #[test]
    fn out_of_order_destroy_blocks_late_create_until_the_gap_closes() {
        let _guard = crate::tests::test_guard();
        let mut context = dear_imgui_rs::Context::create();
        context.io_mut().set_display_size([128.0, 128.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .io_mut()
            .set_backend_flags(BackendFlags::RENDERER_HAS_TEXTURES);
        let texture =
            OwnedTextureData::from_pixels(TextureFormat::RGBA32, 1, 1, &[1, 2, 3, 4]).unwrap();
        let texture_id = context.register_texture(texture);
        let consumer = context.create_detached_renderer_consumer().unwrap();

        let frame = context.begin_frame();
        frame.ui().image(texture_id, [1.0, 1.0]);
        let first = frame.render_snapshot(&consumer).unwrap();
        context.remove_texture(texture_id).unwrap();
        let second = context.begin_frame().render_snapshot(&consumer).unwrap();
        let key = SnapshotTextureId::User(texture_id);
        assert!(second.texture_requests().iter().any(|request| {
            request.texture() == key && matches!(request.operation(), TextureOp::Destroy)
        }));

        let mut store = RendererTextureStore::default();
        let destroy_feedback = process_matching_requests(
            &mut store,
            second.texture_requests(),
            second.epoch().sequence(),
            key,
            fake_update,
        )
        .unwrap();
        second.commit(destroy_feedback.feedback).unwrap();
        let progress = context.poll_snapshot_completions().unwrap();
        assert_eq!(progress.watermark(), 0);
        store.prune_destroyed(progress.watermark());
        assert!(store.destroyed.contains_key(&key));

        let late_feedback = process_matching_requests(
            &mut store,
            first.texture_requests(),
            first.epoch().sequence(),
            key,
            |_| panic!("out-of-order create reached the native updater"),
        )
        .unwrap();
        assert_eq!(late_feedback.feedback.len(), first.texture_requests().len());
        assert!(late_feedback.installed.is_empty());
        first.commit(late_feedback.feedback).unwrap();
        let progress = context.poll_snapshot_completions().unwrap();
        assert_eq!(progress.watermark(), 2);
        store.prune_destroyed(progress.watermark());
        assert!(store.destroyed.is_empty());
        assert!(store.textures.is_empty());
    }

    #[test]
    fn abandoning_old_work_advances_the_watermark_without_resurrection() {
        let _guard = crate::tests::test_guard();
        let mut context = dear_imgui_rs::Context::create();
        context.io_mut().set_display_size([128.0, 128.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .io_mut()
            .set_backend_flags(BackendFlags::RENDERER_HAS_TEXTURES);
        let texture =
            OwnedTextureData::from_pixels(TextureFormat::RGBA32, 1, 1, &[1, 2, 3, 4]).unwrap();
        let texture_id = context.register_texture(texture);
        let consumer = context.create_detached_renderer_consumer().unwrap();

        let frame = context.begin_frame();
        frame.ui().image(texture_id, [1.0, 1.0]);
        let old = frame.render_snapshot(&consumer).unwrap();
        context.remove_texture(texture_id).unwrap();
        let destroy = context.begin_frame().render_snapshot(&consumer).unwrap();
        let key = SnapshotTextureId::User(texture_id);
        let mut store = RendererTextureStore::default();
        let feedback = process_matching_requests(
            &mut store,
            destroy.texture_requests(),
            destroy.epoch().sequence(),
            key,
            fake_update,
        )
        .unwrap();
        destroy.commit(feedback.feedback).unwrap();
        assert_eq!(context.poll_snapshot_completions().unwrap().watermark(), 0);

        drop(old);
        let progress = context.poll_snapshot_completions().unwrap();
        assert_eq!(progress.abandoned(), 1);
        assert_eq!(progress.watermark(), 2);
        store.prune_destroyed(progress.watermark());
        assert!(!store.destroyed.contains_key(&key));
        assert!(store.textures.is_empty());
    }

    #[test]
    fn high_churn_destroy_tombstones_are_pruned_each_contiguous_frame() {
        let _guard = crate::tests::test_guard();
        let mut context = dear_imgui_rs::Context::create();
        context.io_mut().set_display_size([128.0, 128.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .io_mut()
            .set_backend_flags(BackendFlags::RENDERER_HAS_TEXTURES);
        let consumer = context.create_synchronous_renderer_consumer().unwrap();
        let mut store = RendererTextureStore::default();

        for byte in 0..64_u8 {
            let texture =
                OwnedTextureData::from_pixels(TextureFormat::RGBA32, 1, 1, &[byte, 0, 0, 255])
                    .unwrap();
            let texture_id = context.register_texture(texture);
            let frame = context.begin_frame();
            frame.ui().image(texture_id, [1.0, 1.0]);
            let pending = frame.render(&consumer);
            let epoch = pending.epoch().sequence();
            let key = SnapshotTextureId::User(texture_id);
            let processed = process_matching_requests(
                &mut store,
                pending.texture_requests(),
                epoch,
                key,
                fake_update,
            )
            .unwrap();
            let reconciled = pending
                .reconcile_texture_feedback(processed.feedback)
                .unwrap();
            store.mark_reconciled(&processed.installed);
            store.prune_destroyed(reconciled.completion_progress().watermark());
            drop(reconciled);

            context.remove_texture(texture_id).unwrap();
            let pending = context.begin_frame().render(&consumer);
            let epoch = pending.epoch().sequence();
            let processed = process_matching_requests(
                &mut store,
                pending.texture_requests(),
                epoch,
                key,
                fake_update,
            )
            .unwrap();
            let reconciled = pending
                .reconcile_texture_feedback(processed.feedback)
                .unwrap();
            store.mark_reconciled(&processed.installed);
            store.prune_destroyed(reconciled.completion_progress().watermark());
            drop(reconciled);

            assert!(store.destroyed.is_empty());
            assert!(store.textures.is_empty());
        }
    }

    #[test]
    fn unsupported_format_is_rejected_before_calling_the_native_updater() {
        let _guard = crate::tests::test_guard();
        let mut context = dear_imgui_rs::Context::create();
        context.io_mut().set_display_size([128.0, 128.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .io_mut()
            .set_backend_flags(BackendFlags::RENDERER_HAS_TEXTURES);

        let texture = OwnedTextureData::from_pixels(TextureFormat::Alpha8, 1, 1, &[42]).unwrap();
        let texture_id = context.register_texture(texture);
        let consumer = context.create_synchronous_renderer_consumer().unwrap();
        let frame = context.begin_frame();
        frame.ui().image(texture_id, [1.0, 1.0]);
        let pending = frame.render(&consumer);
        let mut store = RendererTextureStore::default();

        let error = store
            .process_requests(
                pending.texture_requests(),
                pending.epoch().sequence(),
                |texture| {
                    assert_eq!(
                        texture.format(),
                        TextureFormat::RGBA32,
                        "the unsupported user upload reached the native renderer"
                    );
                    fake_update(texture);
                },
            )
            .error
            .unwrap();

        assert!(matches!(
            error,
            Sdl3BackendError::UnsupportedTextureFormat {
                texture: SnapshotTextureId::User(actual),
                format: TextureFormat::Alpha8,
            } if actual == texture_id
        ));
    }
}

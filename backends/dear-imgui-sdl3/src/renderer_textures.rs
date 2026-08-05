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
    /// Latest synchronous frame reconciled through this renderer instance.
    reconciled_epoch: Option<u64>,
}

impl std::fmt::Debug for RendererTextureStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RendererTextureStore")
            .field("textures", &self.textures)
            .field("destroyed", &self.destroyed)
            .field("reconciled_epoch", &self.reconciled_epoch)
            .finish()
    }
}

struct RendererTexture {
    data: OwnedTextureData,
    pixels: Vec<u8>,
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

        let mut data = OwnedTextureData::new();
        data.create(TextureFormat::RGBA32, 1, 1);
        data.set_data(&[255, 255, 255, 255]);
        unsafe {
            data.set_tex_id(TextureId::new(77));
            data.set_status(TextureStatus::OK);
        }
        self.textures.insert(
            texture,
            RendererTexture {
                data,
                pixels: vec![255, 255, 255, 255],
                installed: false,
            },
        );
    }

    pub(super) fn process_requests(
        &mut self,
        requests: &[TextureRequest],
        request_epoch: u64,
        mut update_texture: impl FnMut(&mut TextureData),
    ) -> Result<Vec<TextureFeedback>, Sdl3BackendError> {
        let mut feedback = Vec::with_capacity(requests.len());
        for request in requests {
            match request.operation() {
                TextureOp::Create {
                    format,
                    width,
                    height,
                    row_pitch,
                    pixels,
                } => {
                    let texture = request.texture();
                    if self.destroyed.contains_key(&texture) {
                        continue;
                    }
                    self.destroy_existing(texture, &mut update_texture)?;
                    let pixels =
                        copy_full_upload(texture, *format, *width, *height, *row_pitch, pixels)?;
                    let mut proxy = RendererTexture {
                        data: OwnedTextureData::new(),
                        pixels,
                        installed: false,
                    };
                    proxy.data.create(*format, *width, *height);
                    proxy.data.set_data(&proxy.pixels);
                    set_texture_updates(&mut proxy.data, &[]);
                    update_texture(&mut proxy.data);
                    ensure_upload_completed(texture, &proxy.data)?;
                    let texture_id = proxy.data.tex_id();
                    self.textures.insert(texture, proxy);
                    feedback.push(request.uploaded(texture_id)?);
                }
                TextureOp::Update {
                    format,
                    width,
                    height,
                    rects,
                } => {
                    let texture = request.texture();
                    if self.destroyed.contains_key(&texture) {
                        continue;
                    }
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
                    apply_upload_rects(
                        texture,
                        &mut proxy.pixels,
                        *format,
                        *width,
                        *height,
                        rects,
                    )?;
                    if !rects.is_empty() {
                        proxy.data.set_data(&proxy.pixels);
                        let update_rects =
                            rects.iter().map(|upload| upload.rect).collect::<Vec<_>>();
                        set_texture_updates(&mut proxy.data, &update_rects);
                        update_texture(&mut proxy.data);
                        ensure_upload_completed(texture, &proxy.data)?;
                    }
                    feedback.push(request.uploaded(proxy.data.tex_id())?);
                }
                TextureOp::Destroy => {
                    let texture = request.texture();
                    self.destroyed
                        .entry(texture)
                        .and_modify(|destroy_epoch| {
                            *destroy_epoch = (*destroy_epoch).max(request_epoch);
                        })
                        .or_insert(request_epoch);
                    self.destroy_existing(texture, &mut update_texture)?;
                    feedback.push(request.destroyed()?);
                }
            }
        }
        Ok(feedback)
    }

    /// Mark request-created proxies as visible to Context after feedback was reconciled.
    pub(super) fn mark_reconciled(&mut self, requests: &[TextureRequest], request_epoch: u64) {
        for request in requests {
            if matches!(
                request.operation(),
                TextureOp::Create { .. } | TextureOp::Update { .. }
            ) && let Some(proxy) = self.textures.get_mut(&request.texture())
            {
                proxy.installed = true;
            }
        }
        self.reconciled_epoch = Some(request_epoch);
    }

    pub(super) fn reconciled_epoch_is(&self, request_epoch: u64) -> bool {
        self.reconciled_epoch == Some(request_epoch)
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
        self.reconciled_epoch = None;
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
    set_texture_updates(&mut proxy.data, &[]);
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

fn apply_upload_rects(
    texture: SnapshotTextureId,
    destination: &mut [u8],
    format: TextureFormat,
    width: u32,
    height: u32,
    rects: &[TextureUploadRect],
) -> Result<(), Sdl3BackendError> {
    ensure_supported_format(texture, format)?;
    let bytes_per_pixel = get_format_bytes_per_pixel(format);
    let width = usize::try_from(width).map_err(|_| Sdl3BackendError::InvalidTextureRequest {
        texture,
        reason: "texture width does not fit usize",
    })?;
    let height = usize::try_from(height).map_err(|_| Sdl3BackendError::InvalidTextureRequest {
        texture,
        reason: "texture height does not fit usize",
    })?;
    let destination_pitch =
        width
            .checked_mul(bytes_per_pixel)
            .ok_or(Sdl3BackendError::InvalidTextureRequest {
                texture,
                reason: "destination row pitch overflowed usize",
            })?;
    let expected_len =
        destination_pitch
            .checked_mul(height)
            .ok_or(Sdl3BackendError::InvalidTextureRequest {
                texture,
                reason: "destination byte count overflowed usize",
            })?;
    if destination.len() != expected_len {
        return Err(Sdl3BackendError::InvalidTextureRequest {
            texture,
            reason: "renderer allocation does not match update dimensions",
        });
    }

    for upload in rects {
        let rect = upload.rect;
        let x = usize::from(rect.x);
        let y = usize::from(rect.y);
        let rect_width = usize::from(rect.w);
        let rect_height = usize::from(rect.h);
        if x.checked_add(rect_width).is_none_or(|end| end > width)
            || y.checked_add(rect_height).is_none_or(|end| end > height)
        {
            return Err(Sdl3BackendError::InvalidTextureRequest {
                texture,
                reason: "update rectangle exceeds the renderer allocation",
            });
        }
        let copy_bytes = rect_width.checked_mul(bytes_per_pixel).ok_or(
            Sdl3BackendError::InvalidTextureRequest {
                texture,
                reason: "update row byte count overflowed usize",
            },
        )?;
        if upload.row_pitch < copy_bytes {
            return Err(Sdl3BackendError::InvalidTextureRequest {
                texture,
                reason: "update row pitch is smaller than its rectangle",
            });
        }
        let source_len = upload.row_pitch.checked_mul(rect_height).ok_or(
            Sdl3BackendError::InvalidTextureRequest {
                texture,
                reason: "update byte count overflowed usize",
            },
        )?;
        if upload.data.len() < source_len {
            return Err(Sdl3BackendError::InvalidTextureRequest {
                texture,
                reason: "update upload is shorter than its declared layout",
            });
        }
        for row in 0..rect_height {
            let source_start = row * upload.row_pitch;
            let destination_start = (y + row) * destination_pitch + x * bytes_per_pixel;
            destination[destination_start..destination_start + copy_bytes]
                .copy_from_slice(&upload.data[source_start..source_start + copy_bytes]);
        }
    }
    Ok(())
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

fn set_texture_updates(texture: &mut TextureData, rects: &[TextureRect]) {
    let rects = rects.iter().copied().map(Into::into).collect::<Vec<_>>();
    let count = i32::try_from(rects.len()).expect("texture update rectangle count exceeded i32");
    unsafe {
        ffi::dear_imgui_sdl3_backend_set_texture_updates(
            texture.as_raw_mut(),
            rects.as_ptr(),
            count,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dear_imgui_rs::render::{SnapshotTextureId, TextureRequestKind};
    use dear_imgui_rs::{BackendFlags, TextureId};

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
    ) -> Result<Vec<TextureFeedback>, Sdl3BackendError> {
        let mut feedback = Vec::new();
        for request in requests
            .iter()
            .filter(|request| request.texture() == texture)
        {
            feedback.extend(store.process_requests(
                std::slice::from_ref(request),
                request_epoch,
                &mut update_texture,
            )?);
        }
        Ok(feedback)
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
    fn proxy_store_reconciles_request_bound_create_update_and_destroy() {
        let _guard = crate::tests::test_guard();
        let mut context = dear_imgui_rs::Context::create();
        context.io_mut().set_display_size([128.0, 128.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .io_mut()
            .set_backend_flags(BackendFlags::RENDERER_HAS_TEXTURES);

        let mut texture = OwnedTextureData::new();
        texture.create(TextureFormat::RGBA32, 2, 1);
        texture.set_data(&[1, 2, 3, 4, 5, 6, 7, 8]);
        let texture_id = context.register_texture(texture);
        let _consumer = context.create_renderer_consumer().unwrap();
        let mut store = RendererTextureStore::default();

        let frame = context.begin_frame();
        frame.ui().image(texture_id, [2.0, 1.0]);
        let mut rendered = frame.render();
        assert!(rendered.texture_requests().iter().any(|request| {
            request.texture() == SnapshotTextureId::User(texture_id)
                && request.kind() == TextureRequestKind::Create
        }));
        let feedback = store
            .process_requests(
                rendered.texture_requests(),
                rendered.epoch().unwrap().sequence(),
                fake_update,
            )
            .unwrap();
        rendered.reconcile_texture_feedback(feedback).unwrap();
        store.mark_reconciled(
            rendered.texture_requests(),
            rendered.epoch().unwrap().sequence(),
        );
        drop(rendered);
        assert_eq!(
            context
                .with_texture(texture_id, |texture| texture.texture_id())
                .unwrap(),
            TextureId::new(77)
        );

        context
            .with_texture_mut(texture_id, |mut texture| {
                texture.set_data(&[8, 7, 6, 5, 4, 3, 2, 1]);
            })
            .unwrap();
        let frame = context.begin_frame();
        frame.ui().image(texture_id, [2.0, 1.0]);
        let mut rendered = frame.render();
        assert!(rendered.texture_requests().iter().any(|request| {
            request.texture() == SnapshotTextureId::User(texture_id)
                && request.kind() == TextureRequestKind::Update
        }));
        let feedback = store
            .process_requests(
                rendered.texture_requests(),
                rendered.epoch().unwrap().sequence(),
                fake_update,
            )
            .unwrap();
        rendered.reconcile_texture_feedback(feedback).unwrap();
        store.mark_reconciled(
            rendered.texture_requests(),
            rendered.epoch().unwrap().sequence(),
        );
        drop(rendered);
        assert_eq!(
            store
                .textures
                .get(&SnapshotTextureId::User(texture_id))
                .unwrap()
                .pixels,
            [8, 7, 6, 5, 4, 3, 2, 1]
        );

        context.remove_texture(texture_id).unwrap();
        let mut rendered = context.begin_frame().render();
        assert!(rendered.texture_requests().iter().any(|request| {
            request.texture() == SnapshotTextureId::User(texture_id)
                && request.kind() == TextureRequestKind::Destroy
        }));
        let feedback = store
            .process_requests(
                rendered.texture_requests(),
                rendered.epoch().unwrap().sequence(),
                fake_update,
            )
            .unwrap();
        rendered.reconcile_texture_feedback(feedback).unwrap();
        store.mark_reconciled(
            rendered.texture_requests(),
            rendered.epoch().unwrap().sequence(),
        );
        drop(rendered);
        assert!(
            !store
                .textures
                .contains_key(&SnapshotTextureId::User(texture_id))
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
        let mut data = OwnedTextureData::new();
        data.create(TextureFormat::RGBA32, 1, 1);
        data.set_data(&[1, 2, 3, 4]);
        fake_update(&mut data);
        let mut store = RendererTextureStore::default();
        store.textures.insert(
            texture,
            RendererTexture {
                data,
                pixels: vec![1, 2, 3, 4],
                installed: true,
            },
        );

        store.forget_destroyed_by_upstream();

        assert!(store.textures.is_empty());
    }

    #[test]
    fn reconciliation_provenance_is_exact_and_cleared_by_teardown() {
        let mut store = RendererTextureStore::default();

        assert!(!store.reconciled_epoch_is(7));
        store.mark_reconciled(&[], 7);
        assert!(store.reconciled_epoch_is(7));
        assert!(!store.reconciled_epoch_is(6));

        store.forget_destroyed_by_upstream();
        assert!(!store.reconciled_epoch_is(7));
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
        let mut data = OwnedTextureData::new();
        data.create(TextureFormat::RGBA32, 1, 1);
        data.set_data(&[1, 2, 3, 4]);
        fake_update(&mut data);
        let mut store = RendererTextureStore::default();
        store.textures.insert(
            texture,
            RendererTexture {
                data,
                pixels: vec![1, 2, 3, 4],
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
    fn destroyed_identity_ignores_late_uploads_until_contiguous_completion() {
        let _guard = crate::tests::test_guard();
        let mut context = dear_imgui_rs::Context::create();
        context.io_mut().set_display_size([128.0, 128.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .io_mut()
            .set_backend_flags(BackendFlags::RENDERER_HAS_TEXTURES);
        let mut texture = OwnedTextureData::new();
        texture.create(TextureFormat::RGBA32, 1, 1);
        texture.set_data(&[1, 2, 3, 4]);
        let texture_id = context.register_texture(texture);
        let _consumer = context.create_renderer_consumer().unwrap();
        let frame = context.begin_frame();
        frame.ui().image(texture_id, [1.0, 1.0]);
        let rendered = frame.render();
        let snapshot_id = SnapshotTextureId::User(texture_id);
        let request = rendered
            .texture_requests()
            .iter()
            .find(|request| request.texture() == snapshot_id)
            .expect("frame should contain the user texture create request");
        let mut store = RendererTextureStore::default();
        store.destroyed.insert(snapshot_id, 5);

        let feedback = store
            .process_requests(std::slice::from_ref(request), 3, |_| {
                panic!("retired upload reached the native updater")
            })
            .unwrap();
        assert!(feedback.is_empty());
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
        let mut texture = OwnedTextureData::new();
        texture.create(TextureFormat::RGBA32, 1, 1);
        texture.set_data(&[1, 2, 3, 4]);
        let texture_id = context.register_texture(texture);
        let consumer = context.create_renderer_consumer().unwrap();

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
        second.commit(destroy_feedback).unwrap();
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
        assert!(late_feedback.is_empty());
        first.commit(late_feedback).unwrap();
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
        let mut texture = OwnedTextureData::new();
        texture.create(TextureFormat::RGBA32, 1, 1);
        texture.set_data(&[1, 2, 3, 4]);
        let texture_id = context.register_texture(texture);
        let consumer = context.create_renderer_consumer().unwrap();

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
        destroy.commit(feedback).unwrap();
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
        let _consumer = context.create_renderer_consumer().unwrap();
        let mut store = RendererTextureStore::default();

        for byte in 0..64_u8 {
            let mut texture = OwnedTextureData::new();
            texture.create(TextureFormat::RGBA32, 1, 1);
            texture.set_data(&[byte, 0, 0, 255]);
            let texture_id = context.register_texture(texture);
            let frame = context.begin_frame();
            frame.ui().image(texture_id, [1.0, 1.0]);
            let mut rendered = frame.render();
            let epoch = rendered.epoch().unwrap().sequence();
            let key = SnapshotTextureId::User(texture_id);
            let feedback = process_matching_requests(
                &mut store,
                rendered.texture_requests(),
                epoch,
                key,
                fake_update,
            )
            .unwrap();
            let progress = rendered.reconcile_texture_feedback(feedback).unwrap();
            store.mark_reconciled(rendered.texture_requests(), epoch);
            store.prune_destroyed(progress.watermark());
            drop(rendered);

            context.remove_texture(texture_id).unwrap();
            let mut rendered = context.begin_frame().render();
            let epoch = rendered.epoch().unwrap().sequence();
            let feedback = process_matching_requests(
                &mut store,
                rendered.texture_requests(),
                epoch,
                key,
                fake_update,
            )
            .unwrap();
            let progress = rendered.reconcile_texture_feedback(feedback).unwrap();
            store.mark_reconciled(rendered.texture_requests(), epoch);
            store.prune_destroyed(progress.watermark());
            drop(rendered);

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

        let mut texture = OwnedTextureData::new();
        texture.create(TextureFormat::Alpha8, 1, 1);
        texture.set_data(&[42]);
        let texture_id = context.register_texture(texture);
        let _consumer = context.create_renderer_consumer().unwrap();
        let frame = context.begin_frame();
        frame.ui().image(texture_id, [1.0, 1.0]);
        let rendered = frame.render();
        let mut store = RendererTextureStore::default();

        let error = store
            .process_requests(
                rendered.texture_requests(),
                rendered.epoch().unwrap().sequence(),
                |texture| {
                    assert_eq!(
                        texture.format(),
                        TextureFormat::RGBA32,
                        "the unsupported user upload reached the native renderer"
                    );
                    fake_update(texture);
                },
            )
            .unwrap_err();

        assert!(matches!(
            error,
            Sdl3BackendError::UnsupportedTextureFormat {
                texture: SnapshotTextureId::User(actual),
                format: TextureFormat::Alpha8,
            } if actual == texture_id
        ));
    }
}

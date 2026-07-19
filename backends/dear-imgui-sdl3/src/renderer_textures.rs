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
}

impl std::fmt::Debug for RendererTextureStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RendererTextureStore")
            .field("textures", &self.textures)
            .finish()
    }
}

struct RendererTexture {
    data: OwnedTextureData,
    pixels: Vec<u8>,
}

impl std::fmt::Debug for RendererTexture {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RendererTexture")
            .field("format", &self.data.format())
            .field("width", &self.data.width())
            .field("height", &self.data.height())
            .field("texture_id", &self.data.tex_id())
            .finish_non_exhaustive()
    }
}

impl RendererTextureStore {
    pub(super) fn process_requests(
        &mut self,
        requests: &[TextureRequest],
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
                    self.destroy_existing(texture, &mut update_texture)?;
                    let pixels =
                        copy_full_upload(texture, *format, *width, *height, *row_pitch, pixels)?;
                    let mut proxy = RendererTexture {
                        data: OwnedTextureData::new(),
                        pixels,
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
                    self.destroy_existing(texture, &mut update_texture)?;
                    feedback.push(request.destroyed()?);
                }
            }
        }
        Ok(feedback)
    }

    /// Upstream device teardown destroys the native IDs mirrored by these proxy records.
    pub(super) fn forget_destroyed_by_upstream(&mut self) {
        self.textures.clear();
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
    proxy.data.set_status(TextureStatus::WantDestroy);
    unsafe {
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
            TextureStatus::WantCreate | TextureStatus::WantUpdates => {
                texture.set_tex_id(TextureId::new(77));
                texture.set_status(TextureStatus::OK);
            }
            TextureStatus::WantDestroy => texture.set_status(TextureStatus::Destroyed),
            TextureStatus::OK | TextureStatus::Destroyed => {}
        }
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
            .process_requests(rendered.texture_requests(), fake_update)
            .unwrap();
        rendered.reconcile_texture_feedback(feedback).unwrap();
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
            .process_requests(rendered.texture_requests(), fake_update)
            .unwrap();
        rendered.reconcile_texture_feedback(feedback).unwrap();
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
            .process_requests(rendered.texture_requests(), fake_update)
            .unwrap();
        rendered.reconcile_texture_feedback(feedback).unwrap();
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
            },
        );

        store.forget_destroyed_by_upstream();

        assert!(store.textures.is_empty());
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
            .process_requests(rendered.texture_requests(), |texture| {
                assert_eq!(
                    texture.format(),
                    TextureFormat::RGBA32,
                    "the unsupported user upload reached the native renderer"
                );
                fake_update(texture);
            })
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

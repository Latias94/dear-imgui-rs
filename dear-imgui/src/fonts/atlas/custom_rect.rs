use std::marker::PhantomData;
use std::rc::Rc;

use crate::texture::{TextureFormat, TextureRect, TextureRef, get_format_bytes_per_pixel};
use crate::{Ui, sys};

use super::state::{
    current_context_font_atlas, custom_rect_nonce_is_active, font_atlas_state,
    register_custom_rect_nonce, unregister_custom_rect_nonce,
};
use super::{FontAtlas, FontAtlasTexture};

/// Persistent, atlas-validated identity for a custom font-atlas rectangle.
///
/// The ID survives texture growth and repacking. Rectangle coordinates, UVs, and texture
/// references do not; query a fresh [`CustomRectSnapshot`] whenever they are needed.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct CustomRectId {
    raw: sys::ImFontAtlasRectId,
    atlas: *mut sys::ImFontAtlas,
    atlas_stamp: u64,
    generation: u64,
    nonce: u64,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl CustomRectId {
    fn from_raw_parts(raw: sys::ImFontAtlasRectId, atlas: *mut sys::ImFontAtlas) -> Self {
        assert!(raw >= 0, "CustomRectId requires a valid native ID");
        let (state, nonce) = register_custom_rect_nonce(atlas, raw);
        Self {
            raw,
            atlas,
            atlas_stamp: state.stamp,
            generation: state.custom_rect_generation,
            nonce,
            _not_send_sync: PhantomData,
        }
    }
}

/// Pixel data used to create or fully replace a custom atlas rectangle.
#[derive(Copy, Clone, Debug)]
pub struct CustomRectData<'data> {
    size: [u16; 2],
    format: TextureFormat,
    pixels: &'data [u8],
}

impl<'data> CustomRectData<'data> {
    /// Create RGBA8 custom-rectangle data.
    pub fn rgba32(size: [u16; 2], pixels: &'data [u8]) -> Self {
        Self::new(size, TextureFormat::RGBA32, pixels)
    }

    /// Create alpha-only custom-rectangle data.
    pub fn alpha8(size: [u16; 2], pixels: &'data [u8]) -> Self {
        Self::new(size, TextureFormat::Alpha8, pixels)
    }

    fn new(size: [u16; 2], format: TextureFormat, pixels: &'data [u8]) -> Self {
        assert!(
            size[0] > 0 && size[1] > 0,
            "CustomRectData dimensions must be positive"
        );
        let expected = usize::from(size[0])
            .checked_mul(usize::from(size[1]))
            .and_then(|pixels| pixels.checked_mul(get_format_bytes_per_pixel(format)))
            .expect("CustomRectData byte length overflowed usize");
        assert_eq!(
            pixels.len(),
            expected,
            "CustomRectData pixel byte count does not match its size and format"
        );
        Self {
            size,
            format,
            pixels,
        }
    }

    /// Rectangle dimensions in pixels.
    pub fn size(&self) -> [u16; 2] {
        self.size
    }

    /// Source pixel format.
    pub fn format(&self) -> TextureFormat {
        self.format
    }
}

/// Copy-out snapshot of a custom rectangle's current texture placement.
///
/// Texture growth may make these values stale after another ImGui call. Use the snapshot
/// immediately, or query the ID again before submitting a later draw command.
#[derive(Debug)]
pub struct CustomRectSnapshot<'scope> {
    atlas: *mut sys::ImFontAtlas,
    texture: sys::ImTextureRef,
    _texture_lease: Option<FontAtlasTexture<'scope>>,
    pixels: TextureRect,
    uv0: [f32; 2],
    uv1: [f32; 2],
}

impl<'scope> CustomRectSnapshot<'scope> {
    /// Current managed texture reference.
    pub fn texture(&self) -> TextureRef<'_> {
        TextureRef::from_font_atlas_raw(self.atlas, self.texture)
    }

    /// Current pixel-space rectangle.
    pub fn pixels(&self) -> TextureRect {
        self.pixels
    }

    /// Current upper-left UV coordinate.
    pub fn uv0(&self) -> [f32; 2] {
        self.uv0
    }

    /// Current lower-right UV coordinate.
    pub fn uv1(&self) -> [f32; 2] {
        self.uv1
    }
}

fn validate_for_atlas(
    id: CustomRectId,
    atlas: *mut sys::ImFontAtlas,
    caller: &str,
) -> Option<sys::ImFontAtlasRectId> {
    assert!(
        std::ptr::addr_eq(id.atlas.cast_const(), atlas.cast_const()),
        "{caller} received a CustomRectId from a different font atlas"
    );
    let state = font_atlas_state(atlas);
    assert_eq!(
        id.atlas_stamp, state.stamp,
        "{caller} received a CustomRectId from a destroyed or reused font atlas"
    );
    assert_eq!(
        id.generation, state.custom_rect_generation,
        "{caller} received a stale CustomRectId invalidated by font atlas mutation"
    );
    custom_rect_nonce_is_active(atlas, id.raw, id.nonce).then_some(id.raw)
}

fn get_native_rect(
    atlas: *mut sys::ImFontAtlas,
    id: CustomRectId,
    caller: &str,
) -> Option<sys::ImFontAtlasRect> {
    let raw_id = validate_for_atlas(id, atlas, caller)?;
    let mut rect = sys::ImFontAtlasRect::default();
    if unsafe { sys::ImFontAtlas_GetCustomRect(atlas, raw_id, &mut rect) } {
        Some(rect)
    } else {
        None
    }
}

unsafe fn snapshot_from_native<'scope>(
    atlas: *mut sys::ImFontAtlas,
    rect: sys::ImFontAtlasRect,
    texture_lease: Option<FontAtlasTexture<'scope>>,
) -> CustomRectSnapshot<'scope> {
    CustomRectSnapshot {
        atlas,
        texture: unsafe { (*atlas).TexRef },
        _texture_lease: texture_lease,
        pixels: TextureRect {
            x: rect.x,
            y: rect.y,
            w: rect.w,
            h: rect.h,
        },
        uv0: [rect.uv0.x, rect.uv0.y],
        uv1: [rect.uv1.x, rect.uv1.y],
    }
}

impl FontAtlas {
    /// Add and initialize a custom atlas rectangle.
    ///
    /// Returns `None` when the atlas cannot allocate the requested rectangle.
    #[doc(alias = "AddCustomRect")]
    pub fn add_custom_rect(&self, data: CustomRectData<'_>) -> Option<CustomRectId> {
        self.assert_mutation_allowed("FontAtlas::add_custom_rect()");
        self.assert_custom_rect_write_supported(data, "FontAtlas::add_custom_rect()");
        let atlas = self.raw();
        let mut rect = sys::ImFontAtlasRect::default();
        let raw_id = unsafe {
            sys::ImFontAtlas_AddCustomRect(
                atlas,
                i32::from(data.size[0]),
                i32::from(data.size[1]),
                &mut rect,
            )
        };
        if raw_id < 0 {
            return None;
        }

        self.write_native_rect(rect, data);
        Some(CustomRectId::from_raw_parts(raw_id, atlas))
    }

    /// Fully replace a custom rectangle's pixels and queue the exact region for upload.
    ///
    /// Returns `false` if the native rectangle has been removed.
    pub fn write_custom_rect(&self, id: CustomRectId, data: CustomRectData<'_>) -> bool {
        self.assert_mutation_allowed("FontAtlas::write_custom_rect()");
        self.assert_custom_rect_write_supported(data, "FontAtlas::write_custom_rect()");
        let Some(rect) = get_native_rect(self.raw(), id, "FontAtlas::write_custom_rect()") else {
            return false;
        };
        assert_eq!(
            data.size,
            [rect.w, rect.h],
            "FontAtlas::write_custom_rect() data size must match the allocated rectangle"
        );
        self.write_native_rect(rect, data);
        true
    }

    /// Remove a custom rectangle.
    ///
    /// Returns `false` if the rectangle was already removed.
    #[doc(alias = "RemoveCustomRect")]
    pub fn remove_custom_rect(&self, id: CustomRectId) -> bool {
        self.assert_mutation_allowed("FontAtlas::remove_custom_rect()");
        let atlas = self.raw();
        let Some(raw_id) = validate_for_atlas(id, atlas, "FontAtlas::remove_custom_rect()") else {
            return false;
        };
        let exists = unsafe { sys::ImFontAtlas_GetCustomRect(atlas, raw_id, std::ptr::null_mut()) };
        if exists {
            unsafe { sys::ImFontAtlas_RemoveCustomRect(atlas, raw_id) };
        }
        unregister_custom_rect_nonce(atlas, raw_id, id.nonce);
        exists
    }

    /// Query the rectangle's current placement.
    #[doc(alias = "GetCustomRect")]
    pub fn custom_rect(&self, id: CustomRectId) -> Option<CustomRectSnapshot<'_>> {
        let atlas = self.raw();
        let rect = get_native_rect(atlas, id, "FontAtlas::custom_rect()")?;
        let texture_lease = self.tex_data_internal();
        Some(unsafe { snapshot_from_native(atlas, rect, texture_lease) })
    }

    fn assert_custom_rect_write_supported(&self, data: CustomRectData<'_>, caller: &str) {
        let atlas = self.raw();
        unsafe {
            let texture = (*atlas).TexData;
            let destination_format = if texture.is_null() {
                TextureFormat::from((*atlas).TexDesiredFormat)
            } else {
                TextureFormat::from((*texture).Format)
            };
            assert!(
                data.format != TextureFormat::RGBA32 || destination_format == TextureFormat::RGBA32,
                "{caller} cannot store RGBA32 pixels in an Alpha8 font atlas; use an RGBA32 atlas or alpha8 custom-rectangle data"
            );
            assert!(
                (*atlas).RendererHasTextures
                    || texture.is_null()
                    || (*texture).Status != sys::ImTextureStatus_OK,
                "{caller} cannot update an already-uploaded legacy font atlas; enable a renderer with RENDERER_HAS_TEXTURES or rebuild and fully re-upload the atlas"
            );
        }
    }

    fn write_native_rect(&self, rect: sys::ImFontAtlasRect, data: CustomRectData<'_>) {
        let atlas = self.raw();
        unsafe {
            let texture = (*atlas).TexData;
            assert!(
                !texture.is_null() && !(*texture).Pixels.is_null(),
                "custom rectangle requires allocated atlas texture pixels"
            );
            assert!(
                (*texture).Status != sys::ImTextureStatus_WantDestroy
                    && (*texture).Status != sys::ImTextureStatus_Destroyed,
                "custom rectangle texture is not writable in its current status"
            );

            let texture_width = usize::try_from((*texture).Width)
                .expect("font atlas texture width must be non-negative");
            let texture_height = usize::try_from((*texture).Height)
                .expect("font atlas texture height must be non-negative");
            let destination_bpp = usize::try_from((*texture).BytesPerPixel)
                .expect("font atlas texture bytes per pixel must be non-negative");
            assert_eq!(
                destination_bpp,
                get_format_bytes_per_pixel(TextureFormat::from((*texture).Format)),
                "font atlas texture format and bytes-per-pixel metadata disagree"
            );
            let x = usize::from(rect.x);
            let y = usize::from(rect.y);
            let width = usize::from(rect.w);
            let height = usize::from(rect.h);
            assert!(
                x.checked_add(width).is_some_and(|end| end <= texture_width)
                    && y.checked_add(height)
                        .is_some_and(|end| end <= texture_height),
                "custom rectangle exceeds the current atlas texture"
            );

            let source_bpp = get_format_bytes_per_pixel(data.format);
            let source_pitch = width
                .checked_mul(source_bpp)
                .expect("custom rectangle source pitch overflowed usize");
            let destination_pitch = texture_width
                .checked_mul(destination_bpp)
                .expect("font atlas texture pitch overflowed usize");
            let destination_offset = y
                .checked_mul(texture_width)
                .and_then(|offset| offset.checked_add(x))
                .and_then(|offset| offset.checked_mul(destination_bpp))
                .expect("custom rectangle destination offset overflowed usize");
            let destination = (*texture).Pixels.add(destination_offset);
            let destination_format = TextureFormat::from((*texture).Format);
            for row in 0..height {
                let source_row = data.pixels.as_ptr().add(row * source_pitch);
                let destination_row = destination.add(row * destination_pitch);
                match (data.format, destination_format) {
                    (TextureFormat::RGBA32, TextureFormat::RGBA32)
                    | (TextureFormat::Alpha8, TextureFormat::Alpha8) => {
                        std::ptr::copy_nonoverlapping(source_row, destination_row, source_pitch);
                    }
                    (TextureFormat::Alpha8, TextureFormat::RGBA32) => {
                        for column in 0..width {
                            let alpha = *source_row.add(column);
                            let pixel = destination_row.add(column * 4);
                            std::ptr::copy_nonoverlapping(
                                [255, 255, 255, alpha].as_ptr(),
                                pixel,
                                4,
                            );
                        }
                    }
                    (TextureFormat::RGBA32, TextureFormat::Alpha8) => unreachable!(
                        "RGBA32 to Alpha8 custom-rectangle writes are rejected before mutation"
                    ),
                }
            }
            if data.format == TextureFormat::RGBA32 {
                (*atlas).TexPixelsUseColors = true;
                (*texture).UseColors = true;
            }
            if (*atlas).RendererHasTextures {
                sys::igImFontAtlasTextureBlockQueueUpload(
                    atlas,
                    texture,
                    i32::from(rect.x),
                    i32::from(rect.y),
                    i32::from(rect.w),
                    i32::from(rect.h),
                );
            }
        }
    }
}

impl Ui {
    /// Query a custom rectangle for immediate use in the current frame.
    pub fn custom_rect(&self, id: CustomRectId) -> Option<CustomRectSnapshot<'_>> {
        self.run_with_bound_context(|| {
            let atlas = current_context_font_atlas("Ui::custom_rect()");
            let rect = get_native_rect(atlas, id, "Ui::custom_rect()")?;
            Some(unsafe { snapshot_from_native(atlas, rect, None) })
        })
    }

    /// Draw a custom rectangle using its latest texture reference and UVs.
    ///
    /// Returns `false` if the rectangle has been removed.
    pub fn image_custom_rect(&self, id: CustomRectId, size: [f32; 2]) -> bool {
        let Some(rect) = self.custom_rect(id) else {
            return false;
        };
        self.image_config(rect.texture(), size)
            .uv0(rect.uv0())
            .uv1(rect.uv1())
            .build();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_rect_writes_pixels_and_queues_exact_updates() {
        let ctx = crate::Context::create();
        let pixels = [
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];
        let id = ctx
            .font_atlas()
            .add_custom_rect(CustomRectData::rgba32([2, 2], &pixels))
            .expect("the custom rectangle should fit");

        let snapshot = ctx
            .font_atlas()
            .custom_rect(id)
            .expect("the custom rectangle should resolve");
        assert_eq!(snapshot.pixels().w, 2);
        assert_eq!(snapshot.pixels().h, 2);

        let rect = snapshot.pixels();
        drop(snapshot);
        let atlas = ctx.font_atlas();
        unsafe {
            (*atlas.raw()).RendererHasTextures = true;
            (*(*atlas.raw()).TexData).Status = sys::ImTextureStatus_OK;
        }

        let replacement = [7u8; 16];
        assert!(atlas.write_custom_rect(id, CustomRectData::rgba32([2, 2], &replacement)));
        let updates: Vec<_> = atlas
            .tex_data_internal()
            .expect("atlas texture should remain available")
            .updates()
            .collect();
        assert_eq!(updates.last().copied(), Some(rect));
    }

    #[test]
    fn custom_rect_snapshot_leases_its_texture_until_drop() {
        let ctx = crate::Context::create();
        let atlas = ctx.font_atlas();
        let id = atlas
            .add_custom_rect(CustomRectData::alpha8([1, 1], &[0x7f]))
            .expect("the custom rectangle should fit");
        let snapshot = atlas
            .custom_rect(id)
            .expect("the custom rectangle should resolve");

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = atlas.write_custom_rect(id, CustomRectData::alpha8([1, 1], &[0xff]));
        }));
        assert!(result.is_err());

        drop(snapshot);
        assert!(atlas.write_custom_rect(id, CustomRectData::alpha8([1, 1], &[0xff])));
    }

    #[test]
    fn custom_rect_rejects_updates_after_a_legacy_upload() {
        let ctx = crate::Context::create();
        let id = ctx
            .font_atlas()
            .add_custom_rect(CustomRectData::alpha8([1, 1], &[0x7f]))
            .expect("the custom rectangle should fit");
        let legacy = ctx
            .font_atlas()
            .try_claim_legacy_renderer()
            .expect("the test models a legacy renderer");
        unsafe {
            // The test models a completed legacy upload for this unshared atlas.
            legacy.set_texture_id(crate::TextureId::new(17));
        }

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = ctx
                .font_atlas()
                .write_custom_rect(id, CustomRectData::alpha8([1, 1], &[0xff]));
        }));
        assert!(result.is_err());
    }

    #[test]
    fn custom_rect_rejects_rgba_data_for_an_alpha_atlas_before_reading_alignment() {
        let ctx = crate::Context::create();
        let atlas = ctx.font_atlas();
        unsafe { (*atlas.raw()).TexDesiredFormat = sys::ImTextureFormat_Alpha8 };
        let legacy = atlas
            .try_claim_legacy_renderer()
            .expect("the test requires a legacy font atlas");
        legacy.build();
        assert_eq!(
            legacy
                .tex_data()
                .expect("atlas texture should exist")
                .format(),
            TextureFormat::Alpha8
        );

        let storage = [0u8; 8];
        let alignment = std::mem::align_of::<u32>();
        let offset = (0..alignment)
            .find(|offset| (storage.as_ptr() as usize + offset) % alignment != 0)
            .expect("one of the first u32-alignment offsets must be unaligned");
        let unaligned_rgba = &storage[offset..offset + 4];
        assert_ne!((unaligned_rgba.as_ptr() as usize) % alignment, 0);
        let builder = unsafe { (*atlas.raw()).Builder };
        assert!(!builder.is_null());
        let rect_count_before = unsafe { (*builder).RectsIndex.Size };
        let texture_status_before = unsafe { (*(*atlas.raw()).TexData).Status };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = atlas.add_custom_rect(CustomRectData::rgba32([1, 1], unaligned_rgba));
        }));
        let panic = result.expect_err("RGBA32 data must be rejected before FFI");
        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&str>().copied())
            .unwrap_or_default();
        assert!(message.contains("cannot store RGBA32 pixels in an Alpha8 font atlas"));
        assert_eq!(unsafe { (*builder).RectsIndex.Size }, rect_count_before);
        assert_eq!(
            unsafe { (*(*atlas.raw()).TexData).Status },
            texture_status_before
        );
    }

    #[test]
    fn custom_rect_converts_alpha_pixels_to_rgba() {
        let ctx = crate::Context::create();
        let id = ctx
            .font_atlas()
            .add_custom_rect(CustomRectData::alpha8([1, 1], &[0x7f]))
            .expect("the custom rectangle should fit");
        let rect = ctx
            .font_atlas()
            .custom_rect(id)
            .expect("the custom rectangle should resolve")
            .pixels();
        let legacy = ctx
            .font_atlas()
            .try_claim_legacy_renderer()
            .expect("the test requires a legacy font atlas");
        let texture = legacy.tex_data().expect("atlas texture should exist");
        let pixel = texture
            .pixels_at(u32::from(rect.x), u32::from(rect.y))
            .expect("custom rectangle pixel should be addressable");
        assert_eq!(&pixel[..4], &[255, 255, 255, 0x7f]);
    }

    #[test]
    fn removed_and_cross_atlas_custom_rect_ids_are_rejected() {
        let ctx_a = crate::Context::create();
        let id = ctx_a
            .font_atlas()
            .add_custom_rect(CustomRectData::alpha8([1, 1], &[255]))
            .expect("the custom rectangle should fit");
        assert!(ctx_a.font_atlas().remove_custom_rect(id));
        assert!(!ctx_a.font_atlas().remove_custom_rect(id));
        let suspended_a = ctx_a.suspend_or_panic();

        let ctx_b = crate::Context::create();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = ctx_b.font_atlas().custom_rect(id);
        }));
        assert!(result.is_err());

        drop(ctx_b);
        drop(suspended_a);
    }

    #[test]
    fn removed_custom_rect_id_does_not_revive_after_native_generation_wrap() {
        let ctx = crate::Context::create();
        let atlas = ctx.font_atlas();
        let data = CustomRectData::alpha8([1, 1], &[255]);

        let generation_zero = atlas
            .add_custom_rect(data)
            .expect("the initial custom rectangle should fit");
        assert!(atlas.remove_custom_rect(generation_zero));

        let stale = atlas
            .add_custom_rect(data)
            .expect("the reusable custom rectangle should fit");
        assert!(atlas.remove_custom_rect(stale));

        for _ in 0..1022 {
            let current = atlas
                .add_custom_rect(data)
                .expect("the recycled custom rectangle should fit");
            assert!(atlas.remove_custom_rect(current));
        }

        let replacement = atlas
            .add_custom_rect(data)
            .expect("the wrapped custom rectangle should fit");
        assert_eq!(
            stale.raw, replacement.raw,
            "the test must exercise native ID reuse after its 10-bit generation wraps"
        );
        assert!(atlas.custom_rect(stale).is_none());
        assert!(!atlas.write_custom_rect(stale, data));
        assert!(!atlas.remove_custom_rect(stale));
        assert!(atlas.custom_rect(replacement).is_some());
    }

    #[test]
    fn custom_rect_data_requires_an_exact_pixel_count() {
        assert!(std::panic::catch_unwind(|| CustomRectData::rgba32([2, 2], &[0; 15])).is_err());
    }

    #[test]
    fn custom_rect_id_survives_repacking_but_not_builder_clear() {
        let ctx = crate::Context::create();
        let discarded = ctx
            .font_atlas()
            .add_custom_rect(CustomRectData::alpha8([32, 32], &[255; 32 * 32]))
            .expect("the first custom rectangle should fit");
        let retained = ctx
            .font_atlas()
            .add_custom_rect(CustomRectData::alpha8([16, 16], &[127; 16 * 16]))
            .expect("the second custom rectangle should fit");
        assert!(ctx.font_atlas().remove_custom_rect(discarded));
        ctx.font_atlas().compact_cache();
        assert!(ctx.font_atlas().custom_rect(retained).is_some());

        ctx.font_atlas().clear_fonts();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = ctx.font_atlas().custom_rect(retained);
        }));
        assert!(result.is_err());
    }

    #[test]
    fn ui_draws_custom_rect_from_a_fresh_frame_snapshot() {
        let mut ctx = crate::Context::create();
        let id = ctx
            .font_atlas()
            .add_custom_rect(CustomRectData::rgba32([1, 1], &[255, 0, 0, 255]))
            .expect("the custom rectangle should fit");
        ctx.font_atlas()
            .try_claim_legacy_renderer()
            .expect("legacy renderer font atlas should be available")
            .build();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);

        assert!(ctx.frame().image_custom_rect(id, [8.0, 8.0]));
        let _ = ctx.render_legacy();
    }
}

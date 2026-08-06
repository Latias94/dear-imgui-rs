use super::validation::{
    TextureLayout, non_negative_texture_count_from_i32, require_exact_payload,
    validate_native_texture_layout,
};
use super::{
    TextureDataError, TextureFormat, TextureId, TextureRect, TextureRegion, TextureStatus,
    TextureSubresource,
};
use crate::sys;
use std::cell::UnsafeCell;
use std::ffi::c_void;

/// Texture data managed by Dear ImGui
///
/// This is a wrapper around ImTextureData that provides safe access to
/// texture information and pixel data. Application code configures an owned value before
/// registration and mutates registered textures only through their owning Context.
///
/// Lifecycle & Backend Flow (ImGui 1.92+)
/// - Create an instance with `OwnedTextureData::from_pixels()`.
/// - Mutate pixels with `replace_pixels()` or `update_subresource()`.
/// - Transfer user-created owned textures via `Context::register_texture(tex)`.
/// - A renderer owns one synchronous or detached consumer and processes the pointer-free requests
///   exposed by `PendingFrame::texture_requests` or `FrameSnapshot::texture_requests`.
/// - The renderer returns request-bound `TextureFeedback`; the owning Context validates and
///   reconciles it before mutating native texture status or identifiers.
///
/// Context owns every registered user allocation through retirement. Application pixel mutation
/// normally uses `Context::try_with_texture_mut`, so safe code cannot drop the allocation while
/// native draw data or a renderer still refers to it. `Context::with_texture_mut` is the lower-level
/// result-composition API; callers must handle any inner pixel-mutation result themselves.
///
/// Construct owned values explicitly through [`super::OwnedTextureData::from_pixels`].
/// `TextureData` is a borrowed view and therefore has no constructor:
///
/// ```compile_fail
/// use dear_imgui_rs::texture::TextureData;
/// let _ = TextureData::new();
/// ```
///
/// The former metadata and storage mutators are not safe public operations:
///
/// ```compile_fail
/// use dear_imgui_rs::texture::TextureData;
/// fn resize(texture: &mut TextureData) { texture.set_width(2); }
/// ```
///
/// ```compile_fail
/// use dear_imgui_rs::texture::TextureData;
/// fn resize(texture: &mut TextureData) { texture.set_height(2); }
/// ```
///
/// ```compile_fail
/// use dear_imgui_rs::texture::{TextureData, TextureFormat};
/// fn reformat(texture: &mut TextureData) { texture.set_format(TextureFormat::Alpha8); }
/// ```
///
/// ```compile_fail
/// use dear_imgui_rs::texture::TextureData;
/// fn destroy_storage(texture: &mut TextureData) { texture.destroy_pixels(); }
/// ```
#[repr(transparent)]
pub struct TextureData {
    raw: UnsafeCell<sys::ImTextureData>,
}

// Ensure the wrapper stays layout-compatible with the sys bindings.
const _: [(); std::mem::size_of::<sys::ImTextureData>()] = [(); std::mem::size_of::<TextureData>()];
const _: [(); std::mem::align_of::<sys::ImTextureData>()] =
    [(); std::mem::align_of::<TextureData>()];

impl TextureData {
    #[inline]
    pub(super) fn inner(&self) -> &sys::ImTextureData {
        // Safety: `TextureData` is a view into an ImGui-owned `ImTextureData`. Dear ImGui and
        // renderer backends can mutate fields (e.g. Status/TexID/BackendUserData) while Rust holds
        // `&TextureData`, so we store it behind `UnsafeCell` to make that interior mutability
        // explicit.
        unsafe { &*self.raw.get() }
    }

    #[inline]
    pub(super) fn inner_mut(&mut self) -> &mut sys::ImTextureData {
        // Safety: caller has `&mut TextureData`, so this is a unique Rust borrow for this wrapper.
        unsafe { &mut *self.raw.get() }
    }

    /// Create a new texture data from raw pointer (crate-internal)
    ///
    /// Safety: caller must ensure the pointer is valid for the returned lifetime.
    pub(crate) unsafe fn from_raw<'a>(raw: *mut sys::ImTextureData) -> &'a mut Self {
        unsafe { &mut *(raw as *mut Self) }
    }

    /// Create a shared texture data view from a raw pointer (crate-internal).
    ///
    /// Safety: caller must ensure the pointer is valid for the returned lifetime.
    pub(crate) unsafe fn from_raw_ref<'a>(raw: *const sys::ImTextureData) -> &'a Self {
        unsafe { &*(raw as *const Self) }
    }

    /// Get the raw pointer to the underlying ImTextureData
    pub fn as_raw(&self) -> *const sys::ImTextureData {
        self.raw.get() as *const _
    }

    /// Get the raw mutable pointer to the underlying ImTextureData
    pub fn as_raw_mut(&mut self) -> *mut sys::ImTextureData {
        self.raw.get()
    }

    /// Get Dear ImGui's debug-only native texture number.
    ///
    /// This value is not a safe identity: user textures default to zero and atlas values are only
    /// unique within one atlas.
    pub(crate) fn native_unique_id(&self) -> i32 {
        self.inner().UniqueID
    }

    /// Get the current status of this texture
    pub fn status(&self) -> TextureStatus {
        TextureStatus::from(self.inner().Status)
    }

    /// Marks this allocation as retained by the Context-owned renderer queue.
    ///
    /// The marker is opaque and is never dereferenced. Keeping it on the native allocation tells
    /// Dear ImGui not to auto-complete a destroy while Rust still owns queued work.
    pub(crate) fn claim_managed_queue(&mut self) {
        let raw = self.as_raw_mut();
        let marker = raw.cast::<c_void>();
        unsafe {
            assert!(
                (*raw).QueueUserData.is_null() || (*raw).QueueUserData == marker,
                "managed texture is already retained by a different native queue"
            );
            (*raw).QueueUserData = marker;
        }
    }

    /// Set the renderer-owned lifecycle status of this texture.
    ///
    /// Managed renderers should return request-bound `TextureFeedback` instead of calling this
    /// method. This escape hatch exists for renderer-owned textures passed directly to native
    /// Dear ImGui backends.
    ///
    /// # Safety
    ///
    /// The caller must own the renderer transition represented by `status`, uphold Dear ImGui's
    /// texture state machine, and synchronize every GPU use affected by the transition. A texture
    /// registered with a `Context` may only be changed by that Context's validated reconciliation
    /// or renderer-teardown path; external callers must use request-bound `TextureFeedback`
    /// instead.
    pub unsafe fn set_status(&mut self, status: TextureStatus) {
        unsafe {
            // When marking a texture as destroyed, Dear ImGui expects the backend to clear any
            // backend bindings (TexID/BackendUserData). Otherwise ImGui will assert when
            // processing the texture list.
            if status == TextureStatus::Destroyed {
                sys::ImTextureData_SetTexID(self.as_raw_mut(), 0 as sys::ImTextureID);
                (*self.as_raw_mut()).BackendUserData = std::ptr::null_mut();
                (*self.as_raw_mut()).QueueUserData = std::ptr::null_mut();
            }
            sys::ImTextureData_SetStatus(self.as_raw_mut(), status.into());
        }
    }

    /// Get the backend user data pointer without dereferencing it.
    pub fn backend_user_data(&self) -> *mut c_void {
        self.inner().BackendUserData
    }

    /// Set the renderer-owned backend data pointer.
    ///
    /// # Safety
    ///
    /// `data` must be null or point to state with the layout expected by the active renderer
    /// backend. That state must remain valid until the backend has stopped using it and the pointer
    /// is cleared. A texture registered with a `Context` may only be changed by that Context's
    /// validated reconciliation path, whose managed protocol does not expose backend-owned
    /// pointers.
    pub unsafe fn set_backend_user_data(&mut self, data: *mut c_void) {
        self.inner_mut().BackendUserData = data;
    }

    /// Get the texture ID
    pub fn tex_id(&self) -> TextureId {
        TextureId::from(self.inner().TexID)
    }

    /// Set the renderer-owned texture identifier.
    ///
    /// Managed renderers should return request-bound `TextureFeedback` instead of calling this
    /// method.
    ///
    /// # Safety
    ///
    /// `tex_id` must be valid for the active renderer backend for every draw command that can
    /// observe it, and changing or clearing it must be synchronized with all GPU use. A texture
    /// registered with a `Context` may only be changed by that Context's validated reconciliation
    /// path; external callers must use request-bound `TextureFeedback` instead.
    pub unsafe fn set_tex_id(&mut self, tex_id: TextureId) {
        unsafe {
            sys::ImTextureData_SetTexID(self.as_raw_mut(), sys::ImTextureID::from(tex_id));
        }
    }

    /// Get the texture format
    pub fn format(&self) -> TextureFormat {
        TextureFormat::from(self.inner().Format)
    }

    /// Get the texture width
    pub fn width(&self) -> u32 {
        u32::try_from(self.raw_width_i32()).unwrap_or(0)
    }

    /// Get the texture height
    pub fn height(&self) -> u32 {
        u32::try_from(self.raw_height_i32()).unwrap_or(0)
    }

    /// Get the bytes per pixel
    pub fn bytes_per_pixel(&self) -> usize {
        usize::try_from(self.raw_bytes_per_pixel_i32()).unwrap_or(0)
    }

    /// Get the number of unused frames
    pub fn unused_frames(&self) -> usize {
        non_negative_texture_count_from_i32(
            "TextureData::unused_frames()",
            self.inner().UnusedFrames,
        )
    }

    /// Get the reference count
    pub fn ref_count(&self) -> u16 {
        self.inner().RefCount
    }

    /// Check if the texture uses colors (rather than just white + alpha)
    pub fn use_colors(&self) -> bool {
        self.inner().UseColors
    }

    /// Check if the texture is queued for destruction next frame
    pub fn want_destroy_next_frame(&self) -> bool {
        self.inner().WantDestroyNextFrame
    }

    /// Get the pixel data
    ///
    /// Returns None if no pixel data is available.
    pub fn pixels(&self) -> Option<&[u8]> {
        let raw = self.inner();
        if raw.Pixels.is_null() {
            None
        } else {
            let width = raw.Width;
            let height = raw.Height;
            let bytes_per_pixel = raw.BytesPerPixel;
            if width <= 0 || height <= 0 || bytes_per_pixel <= 0 {
                return None;
            }

            let size = (width as usize)
                .checked_mul(height as usize)?
                .checked_mul(bytes_per_pixel as usize)?;
            unsafe { Some(std::slice::from_raw_parts(raw.Pixels as *const u8, size)) }
        }
    }

    /// Get the bounding box of all used pixels in the texture
    pub fn used_rect(&self) -> TextureRect {
        TextureRect::from(self.inner().UsedRect)
    }

    /// Get the bounding box of all queued updates
    pub fn update_rect(&self) -> TextureRect {
        TextureRect::from(self.inner().UpdateRect)
    }

    /// Iterate over queued update rectangles (copying to safe TextureRect)
    pub fn updates(&self) -> impl Iterator<Item = TextureRect> + '_ {
        let vec = &self.inner().Updates;
        let count = if vec.Data.is_null() {
            0
        } else {
            usize::try_from(vec.Size).unwrap_or(0)
        };
        let data = vec.Data as *const sys::ImTextureRect;
        (0..count).map(move |i| unsafe { TextureRect::from(*data.add(i)) })
    }

    /// Get the pixel data at a specific position
    ///
    /// Returns None if no pixel data is available or coordinates are out of bounds.
    pub fn pixels_at(&self, x: u32, y: u32) -> Option<&[u8]> {
        let raw = self.inner();
        let width = u32::try_from(raw.Width).ok()?;
        let height = u32::try_from(raw.Height).ok()?;
        let bytes_per_pixel = usize::try_from(raw.BytesPerPixel).ok()?;
        if raw.Pixels.is_null() || width == 0 || height == 0 || bytes_per_pixel == 0 {
            return None;
        }
        if x >= width || y >= height {
            None
        } else {
            let width_usize = usize::try_from(width).ok()?;
            let height_usize = usize::try_from(height).ok()?;
            let x_usize = usize::try_from(x).ok()?;
            let y_usize = usize::try_from(y).ok()?;

            let total_size = width_usize
                .checked_mul(height_usize)?
                .checked_mul(bytes_per_pixel)?;

            let offset_px = y_usize.checked_mul(width_usize)?.checked_add(x_usize)?;
            let offset_bytes = offset_px.checked_mul(bytes_per_pixel)?;
            let remaining_size = total_size.checked_sub(offset_bytes)?;

            unsafe {
                let ptr = (raw.Pixels as *const u8).add(offset_bytes);
                Some(std::slice::from_raw_parts(ptr, remaining_size))
            }
        }
    }

    /// Get the pitch (bytes per row)
    pub fn pitch(&self) -> usize {
        let width = self.width();
        let bytes_per_pixel = self.bytes_per_pixel();
        if width == 0 || bytes_per_pixel == 0 {
            return 0;
        }
        usize::try_from(width)
            .expect("TextureData::pitch() width must fit usize")
            .checked_mul(bytes_per_pixel)
            .expect("TextureData::pitch() byte pitch overflowed usize")
    }

    /// Replace every pixel using an exact, tightly packed payload.
    ///
    /// Validation is transactional: an error leaves the pixel allocation, contents, status, and
    /// queued update rectangles unchanged. Live textures queue a full update through Dear ImGui's
    /// native update list; textures awaiting initial creation keep `WantCreate`.
    ///
    /// # Errors
    ///
    /// Returns [`TextureDataError`] when the texture is not mutable, its native layout is invalid,
    /// the payload length is not exact, or a live full update cannot be represented safely.
    pub fn replace_pixels(&mut self, pixels: &[u8]) -> Result<(), TextureDataError> {
        let raw = self.inner();
        let layout = validate_native_texture_layout(raw.Width, raw.Height, raw.BytesPerPixel)?;
        require_exact_payload(layout.byte_len, pixels.len())?;
        let status = validate_mutable_texture(raw)?;
        let queued_rect = if status == TextureStatus::WantCreate {
            None
        } else {
            let region =
                TextureRegion::from_validated_dimensions(0, 0, layout.width, layout.height);
            Some(
                validate_live_update_region(raw, layout, region).map_err(|error| match error {
                    TextureDataError::UpdateRegionNotRepresentable(_) => {
                        TextureDataError::FullUpdateRectOutOfRange {
                            width: layout.width,
                            height: layout.height,
                        }
                    }
                    other => other,
                })?,
            )
        };

        unsafe {
            std::ptr::copy_nonoverlapping(
                pixels.as_ptr(),
                self.inner().Pixels.cast::<u8>(),
                layout.byte_len,
            );
        }
        if let Some(rect) = queued_rect {
            queue_texture_upload(self.as_raw_mut(), rect);
        }
        Ok(())
    }

    /// Copy a strided source payload into one texture region.
    ///
    /// The exact payload length is `(height - 1) * row_pitch + tight_row_bytes`. This permits
    /// padding between rows without accepting unused bytes after the final row. All validation and
    /// offset checks complete before the first destination byte is written. A texture in
    /// `WantCreate` changes its initial pixels without queuing an update rectangle or changing
    /// status. A texture in `OK` or `WantUpdates` queues the region and ends in `WantUpdates`.
    ///
    /// # Errors
    ///
    /// Returns [`TextureDataError`] when the texture is not mutable, the region is out of bounds,
    /// the row pitch or payload is invalid, or the queued native rectangle is not representable.
    pub fn update_subresource(
        &mut self,
        update: TextureSubresource<'_>,
    ) -> Result<(), TextureDataError> {
        let raw = self.inner();
        let layout = validate_native_texture_layout(raw.Width, raw.Height, raw.BytesPerPixel)?;
        let status = validate_mutable_texture(raw)?;
        let validated = validate_subresource(raw, layout, update, status)?;

        let destination = unsafe {
            std::slice::from_raw_parts_mut(self.inner().Pixels.cast::<u8>(), layout.byte_len)
        };
        let region = update.region();
        let row_count = usize::try_from(region.height()).expect("validated height must fit usize");
        let y = usize::try_from(region.y()).expect("validated y must fit usize");
        for row in 0..row_count {
            let source_start = row * update.row_pitch();
            let destination_start = (y + row) * layout.row_pitch + validated.x_bytes;
            destination[destination_start..destination_start + validated.tight_row_bytes]
                .copy_from_slice(
                    &update.pixels()[source_start..source_start + validated.tight_row_bytes],
                );
        }
        if let Some(rect) = validated.queued_rect {
            queue_texture_upload(self.as_raw_mut(), rect);
        }
        Ok(())
    }

    pub(crate) fn raw_width_i32(&self) -> i32 {
        self.inner().Width
    }

    pub(crate) fn raw_height_i32(&self) -> i32 {
        self.inner().Height
    }

    pub(crate) fn raw_bytes_per_pixel_i32(&self) -> i32 {
        self.inner().BytesPerPixel
    }
}

#[derive(Clone, Copy, Debug)]
struct ValidatedSubresource {
    x_bytes: usize,
    tight_row_bytes: usize,
    queued_rect: Option<sys::ImTextureRect>,
}

fn validate_mutable_texture(raw: &sys::ImTextureData) -> Result<TextureStatus, TextureDataError> {
    let status = TextureStatus::from(raw.Status);
    if matches!(
        status,
        TextureStatus::Destroyed | TextureStatus::WantDestroy
    ) {
        return Err(TextureDataError::InvalidStatus(status));
    }
    if raw.Pixels.is_null() {
        return Err(TextureDataError::MissingPixelStorage(status));
    }
    Ok(status)
}

fn validate_subresource(
    raw: &sys::ImTextureData,
    layout: TextureLayout,
    update: TextureSubresource<'_>,
    status: TextureStatus,
) -> Result<ValidatedSubresource, TextureDataError> {
    let region = update.region();
    validate_region_bounds(layout, region)?;

    let region_width = usize::try_from(region.width()).expect("validated width must fit usize");
    let region_height = usize::try_from(region.height()).expect("validated height must fit usize");
    let tight_row_bytes = region_width.checked_mul(layout.bytes_per_pixel).ok_or(
        TextureDataError::PayloadSizeOutOfRange {
            row_pitch: update.row_pitch(),
            height: region.height(),
        },
    )?;
    if update.row_pitch() < tight_row_bytes {
        return Err(TextureDataError::RowPitchTooSmall {
            minimum: tight_row_bytes,
            actual: update.row_pitch(),
        });
    }
    let expected = update
        .row_pitch()
        .checked_mul(region_height - 1)
        .and_then(|bytes| bytes.checked_add(tight_row_bytes))
        .ok_or(TextureDataError::PayloadSizeOutOfRange {
            row_pitch: update.row_pitch(),
            height: region.height(),
        })?;
    require_exact_payload(expected, update.pixels().len())?;

    let x = usize::try_from(region.x()).expect("validated x must fit usize");
    let y = usize::try_from(region.y()).expect("validated y must fit usize");
    let x_bytes =
        x.checked_mul(layout.bytes_per_pixel)
            .ok_or(TextureDataError::PayloadSizeOutOfRange {
                row_pitch: update.row_pitch(),
                height: region.height(),
            })?;
    let last_row = y + region_height - 1;
    let destination_end = last_row
        .checked_mul(layout.row_pitch)
        .and_then(|offset| offset.checked_add(x_bytes))
        .and_then(|offset| offset.checked_add(tight_row_bytes))
        .ok_or(TextureDataError::PayloadSizeOutOfRange {
            row_pitch: update.row_pitch(),
            height: region.height(),
        })?;
    debug_assert!(destination_end <= layout.byte_len);

    let queued_rect = if status == TextureStatus::WantCreate {
        None
    } else {
        Some(validate_live_update_region(raw, layout, region)?)
    };
    Ok(ValidatedSubresource {
        x_bytes,
        tight_row_bytes,
        queued_rect,
    })
}

fn validate_region_bounds(
    layout: TextureLayout,
    region: TextureRegion,
) -> Result<(), TextureDataError> {
    let right = region.x().checked_add(region.width());
    let bottom = region.y().checked_add(region.height());
    if right.is_none_or(|right| right > layout.width)
        || bottom.is_none_or(|bottom| bottom > layout.height)
    {
        return Err(TextureDataError::UpdateRegionOutOfBounds {
            region,
            width: layout.width,
            height: layout.height,
        });
    }
    Ok(())
}

fn validate_live_update_region(
    raw: &sys::ImTextureData,
    layout: TextureLayout,
    region: TextureRegion,
) -> Result<sys::ImTextureRect, TextureDataError> {
    validate_region_bounds(layout, region)?;
    let right = region
        .x()
        .checked_add(region.width())
        .expect("validated region endpoint must not overflow");
    let bottom = region
        .y()
        .checked_add(region.height())
        .expect("validated region endpoint must not overflow");
    let native_endpoint_limit = u32::from(u16::MAX) + 1;
    if right > native_endpoint_limit || bottom > native_endpoint_limit {
        return Err(TextureDataError::UpdateRegionNotRepresentable(region));
    }
    let rect = sys::ImTextureRect {
        x: u16::try_from(region.x())
            .map_err(|_| TextureDataError::UpdateRegionNotRepresentable(region))?,
        y: u16::try_from(region.y())
            .map_err(|_| TextureDataError::UpdateRegionNotRepresentable(region))?,
        w: u16::try_from(region.width())
            .map_err(|_| TextureDataError::UpdateRegionNotRepresentable(region))?,
        h: u16::try_from(region.height())
            .map_err(|_| TextureDataError::UpdateRegionNotRepresentable(region))?,
    };
    if !queued_union_is_representable(raw.UpdateRect, rect, true)
        || !queued_union_is_representable(raw.UsedRect, rect, false)
    {
        return Err(TextureDataError::UpdateRegionNotRepresentable(region));
    }
    Ok(rect)
}

fn queued_union_is_representable(
    existing: sys::ImTextureRect,
    request: sys::ImTextureRect,
    empty_axis_starts_at_zero: bool,
) -> bool {
    union_axis_is_representable(
        existing.x,
        existing.w,
        request.x,
        request.w,
        empty_axis_starts_at_zero,
    ) && union_axis_is_representable(
        existing.y,
        existing.h,
        request.y,
        request.h,
        empty_axis_starts_at_zero,
    )
}

fn union_axis_is_representable(
    existing_start: u16,
    existing_len: u16,
    request_start: u16,
    request_len: u16,
    empty_axis_starts_at_zero: bool,
) -> bool {
    let existing_start = u32::from(existing_start);
    let existing_end = if empty_axis_starts_at_zero && existing_len == 0 {
        0
    } else {
        existing_start + u32::from(existing_len)
    };
    let request_start = u32::from(request_start);
    let request_end = request_start + u32::from(request_len);
    let union_start = existing_start.min(request_start);
    let union_end = existing_end.max(request_end);
    union_end - union_start <= u32::from(u16::MAX)
}

fn queue_texture_upload(texture: *mut sys::ImTextureData, rect: sys::ImTextureRect) {
    unsafe {
        // All native assertions (status, non-empty 16-bit fields, endpoints, and bounding unions)
        // were validated before any pixel mutation.
        sys::igImTextureDataQueueUpload(
            texture,
            i32::from(rect.x),
            i32::from(rect.y),
            i32::from(rect.w),
            i32::from(rect.h),
        );
    }
}

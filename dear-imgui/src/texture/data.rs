use super::format::{texture_format_bytes_per_pixel, texture_format_bytes_per_pixel_i32};
use super::validation::{
    checked_texture_byte_len, checked_texture_byte_len_if_valid, checked_texture_dimension_to_i32,
    non_negative_texture_count_from_i32,
};
use super::{
    ManagedTextureId, OwnedTextureData, TextureFormat, TextureId, TextureRect, TextureRef,
    TextureStatus,
};
use crate::sys;
use std::cell::UnsafeCell;
use std::ffi::c_void;

/// Texture data managed by Dear ImGui
///
/// This is a wrapper around ImTextureData that provides safe access to
/// texture information and pixel data. It's used by renderer backends
/// to create, update, and destroy textures.
///
/// Lifecycle & Backend Flow (ImGui 1.92+)
/// - Create an instance (e.g. via `OwnedTextureData::new()` + `create()`)
/// - Mutate pixels, set flags/rects (e.g. call `set_data()` or directly write `Pixels` then
///   set `UpdateRect`), and set status to `WantCreate`/`WantUpdates`.
/// - Register user-created owned textures once via `Context::register_user_texture(&mut tex)`. Dear
///   ImGui builds `DrawData::textures()` from its internal `PlatformIO.Textures[]` list (font atlas
///   textures are registered by ImGui itself).
/// - Your renderer backend iterates `DrawData::textures_mut()` and performs the requested
///   create/update/destroy operations, then updates status to `OK`/`Destroyed`.
/// - You can also set/get a `TexID` (e.g., GPU handle) via `set_tex_id()/tex_id()` after creation.
///
/// Lifetime Note: If using the managed path, you must keep the underlying `ImTextureData` alive at
/// least until the end of the frame where it is referenced by UI calls. If you create textures
/// yourself, use [`OwnedTextureData`] to ensure the object is correctly constructed and destroyed
/// by the C++ side.
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

    pub(super) fn assert_metadata_mutation_allowed(&self, caller: &str) {
        let raw = self.inner();
        assert!(
            raw.Pixels.is_null(),
            "{caller} cannot change texture metadata while pixel storage is allocated"
        );
        assert!(
            raw.Status == sys::ImTextureStatus_Destroyed,
            "{caller} requires Destroyed texture status"
        );
    }

    /// Create a new owned texture data object.
    ///
    /// This is kept for convenience. Prefer [`OwnedTextureData::new()`] for clarity.
    pub fn new() -> OwnedTextureData {
        OwnedTextureData::new()
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

    /// Get this managed texture's stable ImGui identity.
    pub fn unique_id(&self) -> ManagedTextureId {
        ManagedTextureId::from_raw(self.inner().UniqueID)
    }

    /// Get the current status of this texture
    pub fn status(&self) -> TextureStatus {
        TextureStatus::from(self.inner().Status)
    }

    /// Set the status of this texture
    ///
    /// This should only be called by renderer backends after handling a request.
    pub fn set_status(&mut self, status: TextureStatus) {
        unsafe {
            // When marking a texture as destroyed, Dear ImGui expects the backend to clear any
            // backend bindings (TexID/BackendUserData). Otherwise ImGui will assert when
            // processing the texture list.
            if status == TextureStatus::Destroyed {
                sys::ImTextureData_SetTexID(self.as_raw_mut(), 0 as sys::ImTextureID);
                (*self.as_raw_mut()).BackendUserData = std::ptr::null_mut();
            }
            sys::ImTextureData_SetStatus(self.as_raw_mut(), status.into());
        }
    }

    /// Get the backend user data
    pub fn backend_user_data(&self) -> *mut c_void {
        self.inner().BackendUserData
    }

    /// Set the backend user data
    pub fn set_backend_user_data(&mut self, data: *mut c_void) {
        self.inner_mut().BackendUserData = data;
    }

    /// Get the texture ID
    pub fn tex_id(&self) -> TextureId {
        TextureId::from(self.inner().TexID)
    }

    /// Set the texture ID
    ///
    /// This should only be called by renderer backends after creating or destroying the texture.
    pub fn set_tex_id(&mut self, tex_id: TextureId) {
        unsafe {
            sys::ImTextureData_SetTexID(self.as_raw_mut(), tex_id.id() as sys::ImTextureID);
        }
    }

    /// Get the current texture reference for this managed texture.
    #[inline]
    pub fn texture_ref(&mut self) -> TextureRef<'_> {
        unsafe { TextureRef::from_raw(sys::ImTextureData_GetTexRef(self.as_raw_mut())) }
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

    /// Create a new texture with the specified format and dimensions
    ///
    /// This allocates pixel data and sets the status to WantCreate.
    pub fn create(&mut self, format: TextureFormat, width: u32, height: u32) {
        assert!(
            self.status() == TextureStatus::Destroyed,
            "TextureData::create() requires Destroyed texture status"
        );
        let bytes_per_pixel = texture_format_bytes_per_pixel(format);
        let _ = checked_texture_byte_len("TextureData::create()", width, height, bytes_per_pixel);
        let width = checked_texture_dimension_to_i32("TextureData::create()", "width", width);
        let height = checked_texture_dimension_to_i32("TextureData::create()", "height", height);

        unsafe {
            sys::ImTextureData_Create(self.as_raw_mut(), format.into(), width, height);
        }
    }

    /// Destroy the pixel data
    ///
    /// This frees the CPU-side pixel data but doesn't affect the GPU texture.
    pub fn destroy_pixels(&mut self) {
        unsafe {
            sys::ImTextureData_DestroyPixels(self.as_raw_mut());
        }
    }

    /// Set the pixel data for the texture
    ///
    /// This copies the provided data into the texture's pixel buffer.
    pub fn set_data(&mut self, data: &[u8]) {
        unsafe {
            let raw = self.as_raw_mut();
            let Some(needed) = checked_texture_byte_len_if_valid(
                "TextureData::set_data()",
                (*raw).Width,
                (*raw).Height,
                (*raw).BytesPerPixel,
            ) else {
                // Nothing to do without valid dimensions/format.
                return;
            };

            // Ensure pixel buffer exists and has correct size
            if (*raw).Pixels.is_null() {
                assert!(
                    (*raw).Status == sys::ImTextureStatus_Destroyed,
                    "TextureData::set_data() requires Destroyed texture status when allocating missing pixel storage"
                );
                sys::ImTextureData_Create(
                    self.as_raw_mut(),
                    (*raw).Format,
                    (*raw).Width,
                    (*raw).Height,
                );
            }

            let copy_bytes = std::cmp::min(needed, data.len());
            if copy_bytes == 0 {
                return;
            }

            std::ptr::copy_nonoverlapping(data.as_ptr(), (*raw).Pixels as *mut u8, copy_bytes);

            // Mark entire texture as updated
            (*raw).UpdateRect = sys::ImTextureRect {
                x: 0u16,
                y: 0u16,
                w: (*raw).Width.clamp(0, u16::MAX as i32) as u16,
                h: (*raw).Height.clamp(0, u16::MAX as i32) as u16,
            };
            sys::ImTextureData_SetStatus(raw, sys::ImTextureStatus_WantUpdates);
        }
    }

    /// Set the width of the texture
    pub fn set_width(&mut self, width: u32) {
        self.assert_metadata_mutation_allowed("TextureData::set_width()");
        assert!(width > 0, "TextureData::set_width() width must be positive");
        let width =
            i32::try_from(width).expect("TextureData::set_width() width exceeded i32 range");
        self.inner_mut().Width = width;
    }

    /// Set the height of the texture
    pub fn set_height(&mut self, height: u32) {
        self.assert_metadata_mutation_allowed("TextureData::set_height()");
        assert!(
            height > 0,
            "TextureData::set_height() height must be positive"
        );
        let height =
            i32::try_from(height).expect("TextureData::set_height() height exceeded i32 range");
        self.inner_mut().Height = height;
    }

    /// Set the format of the texture
    pub fn set_format(&mut self, format: TextureFormat) {
        self.assert_metadata_mutation_allowed("TextureData::set_format()");
        let raw = self.inner_mut();
        raw.Format = format.into();
        raw.BytesPerPixel = texture_format_bytes_per_pixel_i32(format);
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

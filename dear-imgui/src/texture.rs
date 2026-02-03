//! Texture management for Dear ImGui
//!
//! This module provides access to Dear ImGui's modern texture management system
//! introduced in version 1.92+. It includes support for ImTextureData, texture
//! status management, and automatic texture updates.

use crate::sys;
use std::cell::UnsafeCell;
use std::ffi::c_void;
use std::ptr::NonNull;

/// Simple texture ID for backward compatibility
///
/// This is a simple wrapper around u64 that can be used to identify textures.
/// For modern texture management, use TextureData instead.
///
/// Note: Changed from usize to u64 in Dear ImGui 1.91.4+ to support 64-bit handles
/// like Vulkan and DX12 on 32-bit targets.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct TextureId(u64);

impl TextureId {
    /// Creates a new texture id with the given identifier
    #[inline]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Returns the id of the TextureId
    #[inline]
    pub const fn id(self) -> u64 {
        self.0
    }

    /// Creates a null texture ID
    #[inline]
    pub const fn null() -> Self {
        Self(0)
    }

    /// Checks if this texture ID is null
    #[inline]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }
}

impl From<u64> for TextureId {
    #[inline]
    fn from(id: u64) -> Self {
        TextureId(id)
    }
}

impl<T> From<*const T> for TextureId {
    #[inline]
    fn from(ptr: *const T) -> Self {
        TextureId(ptr as usize as u64)
    }
}

impl<T> From<*mut T> for TextureId {
    #[inline]
    fn from(ptr: *mut T) -> Self {
        TextureId(ptr as usize as u64)
    }
}

impl From<TextureId> for *const c_void {
    #[inline]
    fn from(id: TextureId) -> Self {
        debug_assert!(
            id.0 <= (usize::MAX as u64),
            "TextureId value {} exceeds pointer width on this target",
            id.0
        );
        id.0 as usize as *const c_void
    }
}

impl From<TextureId> for *mut c_void {
    #[inline]
    fn from(id: TextureId) -> Self {
        debug_assert!(
            id.0 <= (usize::MAX as u64),
            "TextureId value {} exceeds pointer width on this target",
            id.0
        );
        id.0 as usize as *mut c_void
    }
}

// Backward compatibility: allow conversion from usize for legacy code
impl From<usize> for TextureId {
    #[inline]
    fn from(id: usize) -> Self {
        TextureId(id as u64)
    }
}

// Allow conversion to usize for legacy code
impl From<TextureId> for usize {
    #[inline]
    fn from(id: TextureId) -> Self {
        debug_assert!(
            id.0 <= (usize::MAX as u64),
            "TextureId value {} exceeds usize width on this target",
            id.0
        );
        id.0 as usize
    }
}

impl Default for TextureId {
    #[inline]
    fn default() -> Self {
        Self::null()
    }
}

/// Raw texture ID type for compatibility with Dear ImGui
pub type RawTextureId = sys::ImTextureID;

impl From<TextureId> for RawTextureId {
    #[inline]
    fn from(id: TextureId) -> Self {
        id.id() as sys::ImTextureID
    }
}

/// A convenient, typed wrapper around ImGui's ImTextureRef (v1.92+)
///
/// Can reference either a plain `TextureId` (legacy path) or a managed `TextureData`.
///
/// Examples
/// - With a plain GPU handle (legacy path):
/// ```no_run
/// # use dear_imgui_rs::{Ui, TextureId};
/// # fn demo(ui: &Ui) {
/// let tex_id = TextureId::new(12345);
/// ui.image(tex_id, [64.0, 64.0]);
/// # }
/// ```
/// - With a managed texture (ImGui 1.92 texture system):
/// ```no_run
/// # use dear_imgui_rs::{Ui, texture::{TextureData, TextureFormat}};
/// # fn demo(ui: &Ui) {
/// let mut tex = TextureData::new();
/// tex.create(TextureFormat::RGBA32, 256, 256);
/// // Fill pixels or schedule updates...
/// ui.image(&mut *tex, [256.0, 256.0]);
/// // The renderer backend will honor WantCreate/WantUpdates/WantDestroy
/// // via DrawData::textures() when rendering this frame.
/// # }
/// ```
#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
pub struct TextureRef(sys::ImTextureRef);

// Ensure the wrapper stays layout-compatible with the sys bindings.
const _: [(); std::mem::size_of::<sys::ImTextureRef>()] = [(); std::mem::size_of::<TextureRef>()];
const _: [(); std::mem::align_of::<sys::ImTextureRef>()] = [(); std::mem::align_of::<TextureRef>()];

impl TextureRef {
    /// Create a texture reference from a raw ImGui texture ref
    #[inline]
    pub fn from_raw(raw: sys::ImTextureRef) -> Self {
        Self(raw)
    }

    /// Get the underlying ImGui texture ref (by value)
    #[inline]
    pub fn raw(self) -> sys::ImTextureRef {
        self.0
    }
}

impl From<TextureId> for TextureRef {
    #[inline]
    fn from(id: TextureId) -> Self {
        TextureRef(sys::ImTextureRef {
            _TexData: std::ptr::null_mut(),
            _TexID: id.id() as sys::ImTextureID,
        })
    }
}

impl From<u64> for TextureRef {
    #[inline]
    fn from(id: u64) -> Self {
        TextureRef::from(TextureId::from(id))
    }
}

impl From<&TextureData> for TextureRef {
    #[inline]
    fn from(td: &TextureData) -> Self {
        // Safety: A shared `&TextureData` must not be used to give Dear ImGui a mutable
        // `ImTextureData*` because ImGui/backends may mutate fields such as `Status`/`TexID`
        // during the frame, which would violate Rust aliasing rules.
        //
        // We therefore treat `&TextureData` as a legacy reference: only forward the current
        // `TexID` value (if any). For managed textures, pass `&mut TextureData` instead.
        TextureRef(sys::ImTextureRef {
            _TexData: std::ptr::null_mut(),
            _TexID: td.tex_id().id() as sys::ImTextureID,
        })
    }
}

impl From<&mut TextureData> for TextureRef {
    #[inline]
    fn from(td: &mut TextureData) -> Self {
        TextureRef(sys::ImTextureRef {
            _TexData: td.as_raw_mut(),
            _TexID: 0,
        })
    }
}

/// Texture format supported by Dear ImGui
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum TextureFormat {
    /// 4 components per pixel, each is unsigned 8-bit. Total size = TexWidth * TexHeight * 4
    RGBA32 = sys::ImTextureFormat_RGBA32 as i32,
    /// 1 component per pixel, each is unsigned 8-bit. Total size = TexWidth * TexHeight
    Alpha8 = sys::ImTextureFormat_Alpha8 as i32,
}

impl From<sys::ImTextureFormat> for TextureFormat {
    fn from(format: sys::ImTextureFormat) -> Self {
        match format {
            sys::ImTextureFormat_RGBA32 => TextureFormat::RGBA32,
            sys::ImTextureFormat_Alpha8 => TextureFormat::Alpha8,
            _ => TextureFormat::RGBA32, // Default fallback
        }
    }
}

impl From<TextureFormat> for sys::ImTextureFormat {
    fn from(format: TextureFormat) -> Self {
        format as sys::ImTextureFormat
    }
}

/// Status of a texture to communicate with Renderer Backend
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum TextureStatus {
    /// Texture is ready and can be used
    OK = sys::ImTextureStatus_OK as i32,
    /// Backend destroyed the texture
    Destroyed = sys::ImTextureStatus_Destroyed as i32,
    /// Requesting backend to create the texture. Set status OK when done.
    WantCreate = sys::ImTextureStatus_WantCreate as i32,
    /// Requesting backend to update specific blocks of pixels. Set status OK when done.
    WantUpdates = sys::ImTextureStatus_WantUpdates as i32,
    /// Requesting backend to destroy the texture. Set status to Destroyed when done.
    WantDestroy = sys::ImTextureStatus_WantDestroy as i32,
}

impl From<sys::ImTextureStatus> for TextureStatus {
    fn from(status: sys::ImTextureStatus) -> Self {
        match status {
            sys::ImTextureStatus_OK => TextureStatus::OK,
            sys::ImTextureStatus_Destroyed => TextureStatus::Destroyed,
            sys::ImTextureStatus_WantCreate => TextureStatus::WantCreate,
            sys::ImTextureStatus_WantUpdates => TextureStatus::WantUpdates,
            sys::ImTextureStatus_WantDestroy => TextureStatus::WantDestroy,
            _ => TextureStatus::Destroyed, // Default fallback
        }
    }
}

impl From<TextureStatus> for sys::ImTextureStatus {
    fn from(status: TextureStatus) -> Self {
        status as sys::ImTextureStatus
    }
}

/// Coordinates of a rectangle within a texture
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct TextureRect {
    /// Upper-left X coordinate of rectangle to update
    pub x: u16,
    /// Upper-left Y coordinate of rectangle to update
    pub y: u16,
    /// Width of rectangle to update
    pub w: u16,
    /// Height of rectangle to update
    pub h: u16,
}

impl From<sys::ImTextureRect> for TextureRect {
    fn from(rect: sys::ImTextureRect) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            w: rect.w,
            h: rect.h,
        }
    }
}

impl From<TextureRect> for sys::ImTextureRect {
    fn from(rect: TextureRect) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            w: rect.w,
            h: rect.h,
        }
    }
}

/// Owned texture data managed by Dear ImGui.
///
/// This owns an `ImTextureData` instance allocated by Dear ImGui (C++) and will
/// destroy it on drop. It dereferences to [`TextureData`] so you can call the
/// same APIs as on borrowed texture data (e.g. items returned by
/// `DrawData::textures()`).
pub struct OwnedTextureData {
    raw: NonNull<sys::ImTextureData>,
}

impl OwnedTextureData {
    /// Create a new empty texture data object (C++ constructed).
    pub fn new() -> Self {
        let raw = unsafe { sys::ImTextureData_ImTextureData() };
        let raw = NonNull::new(raw).expect("ImTextureData_ImTextureData() returned null");
        Self { raw }
    }

    /// Leak the underlying `ImTextureData*` without destroying it.
    pub fn into_raw(self) -> *mut sys::ImTextureData {
        let raw = self.raw.as_ptr();
        std::mem::forget(self);
        raw
    }

    /// Take ownership of a raw `ImTextureData*`.
    ///
    /// # Safety
    /// - `raw` must be a valid pointer returned by `ImTextureData_ImTextureData()`.
    /// - The caller must ensure no other owner will call `ImTextureData_destroy(raw)`.
    pub unsafe fn from_raw_owned(raw: *mut sys::ImTextureData) -> Self {
        let raw = NonNull::new(raw).expect("raw ImTextureData pointer was null");
        Self { raw }
    }
}

impl Drop for OwnedTextureData {
    fn drop(&mut self) {
        unsafe { sys::ImTextureData_destroy(self.raw.as_ptr()) }
    }
}

impl std::ops::Deref for OwnedTextureData {
    type Target = TextureData;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.raw.as_ptr() as *const TextureData) }
    }
}

impl std::ops::DerefMut for OwnedTextureData {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(self.raw.as_ptr() as *mut TextureData) }
    }
}

impl AsRef<TextureData> for OwnedTextureData {
    fn as_ref(&self) -> &TextureData {
        self
    }
}

impl AsMut<TextureData> for OwnedTextureData {
    fn as_mut(&mut self) -> &mut TextureData {
        self
    }
}

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
/// - Register user-created textures once via `Context::register_user_texture(&mut tex)`. Dear
///   ImGui builds `DrawData::textures()` from its internal `PlatformIO.Textures[]` list (font atlas
///   textures are registered by ImGui itself).
/// - Your renderer backend iterates `DrawData::textures()` and performs the requested
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
    fn inner(&self) -> &sys::ImTextureData {
        // Safety: `TextureData` is a view into an ImGui-owned `ImTextureData`. Dear ImGui and
        // renderer backends can mutate fields (e.g. Status/TexID/BackendUserData) while Rust holds
        // `&TextureData`, so we store it behind `UnsafeCell` to make that interior mutability
        // explicit.
        unsafe { &*self.raw.get() }
    }

    #[inline]
    fn inner_mut(&mut self) -> &mut sys::ImTextureData {
        // Safety: caller has `&mut TextureData`, so this is a unique Rust borrow for this wrapper.
        unsafe { &mut *self.raw.get() }
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

    /// Get the unique ID of this texture (for debugging)
    pub fn unique_id(&self) -> i32 {
        self.inner().UniqueID
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

    /// Get the texture format
    pub fn format(&self) -> TextureFormat {
        TextureFormat::from(self.inner().Format)
    }

    /// Get the texture width
    pub fn width(&self) -> i32 {
        self.inner().Width
    }

    /// Get the texture height
    pub fn height(&self) -> i32 {
        self.inner().Height
    }

    /// Get the bytes per pixel
    pub fn bytes_per_pixel(&self) -> i32 {
        self.inner().BytesPerPixel
    }

    /// Get the number of unused frames
    pub fn unused_frames(&self) -> i32 {
        self.inner().UnusedFrames
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
    pub fn pixels_at(&self, x: i32, y: i32) -> Option<&[u8]> {
        let raw = self.inner();
        let width = raw.Width;
        let height = raw.Height;
        let bytes_per_pixel = raw.BytesPerPixel;
        if raw.Pixels.is_null()
            || width <= 0
            || height <= 0
            || bytes_per_pixel <= 0
            || x < 0
            || y < 0
            || x >= width
            || y >= height
        {
            None
        } else {
            let width_usize = width as usize;
            let x_usize = x as usize;
            let y_usize = y as usize;
            let bpp_usize = bytes_per_pixel as usize;

            let total_size = width_usize
                .checked_mul(height as usize)?
                .checked_mul(bpp_usize)?;

            let offset_px = y_usize.checked_mul(width_usize)?.checked_add(x_usize)?;
            let offset_bytes = offset_px.checked_mul(bpp_usize)?;
            let remaining_size = total_size.checked_sub(offset_bytes)?;

            unsafe {
                let ptr = (raw.Pixels as *const u8).add(offset_bytes);
                Some(std::slice::from_raw_parts(ptr, remaining_size))
            }
        }
    }

    /// Get the pitch (bytes per row)
    pub fn pitch(&self) -> i32 {
        self.width() * self.bytes_per_pixel()
    }

    /// Create a new texture with the specified format and dimensions
    ///
    /// This allocates pixel data and sets the status to WantCreate.
    pub fn create(&mut self, format: TextureFormat, width: i32, height: i32) {
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
            let needed = (*raw)
                .Width
                .saturating_mul((*raw).Height)
                .saturating_mul((*raw).BytesPerPixel);
            if needed <= 0 {
                // Nothing to do without valid dimensions/format.
                return;
            }

            // Ensure pixel buffer exists and has correct size
            if (*raw).Pixels.is_null() {
                sys::ImTextureData_Create(
                    self.as_raw_mut(),
                    (*raw).Format,
                    (*raw).Width,
                    (*raw).Height,
                );
            }

            let copy_bytes = std::cmp::min(needed as usize, data.len());
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
        let width = width.min(i32::MAX as u32) as i32;
        self.inner_mut().Width = width;
    }

    /// Set the height of the texture
    pub fn set_height(&mut self, height: u32) {
        let height = height.min(i32::MAX as u32) as i32;
        self.inner_mut().Height = height;
    }

    /// Set the format of the texture
    pub fn set_format(&mut self, format: TextureFormat) {
        self.inner_mut().Format = format.into();
    }
}

/// Get the number of bytes per pixel for a texture format
pub fn get_format_bytes_per_pixel(format: TextureFormat) -> i32 {
    unsafe { sys::igImTextureDataGetFormatBytesPerPixel(format.into()) }
}

/// Create an ImTextureRef from a texture ID
///
/// This is the safe way to create an ImTextureRef for use with Dear ImGui.
/// Use this instead of directly constructing the sys::ImTextureRef structure.
pub fn create_texture_ref(texture_id: u64) -> sys::ImTextureRef {
    sys::ImTextureRef {
        _TexData: std::ptr::null_mut(),
        _TexID: texture_id,
    }
}

/// Get the name of a texture status (for debugging)
pub fn get_status_name(status: TextureStatus) -> &'static str {
    unsafe {
        let ptr = sys::igImTextureDataGetStatusName(status.into());
        if ptr.is_null() {
            "Unknown"
        } else {
            std::ffi::CStr::from_ptr(ptr).to_str().unwrap_or("Invalid")
        }
    }
}

/// Get the name of a texture format (for debugging)
pub fn get_format_name(format: TextureFormat) -> &'static str {
    unsafe {
        let ptr = sys::igImTextureDataGetFormatName(format.into());
        if ptr.is_null() {
            "Unknown"
        } else {
            std::ffi::CStr::from_ptr(ptr).to_str().unwrap_or("Invalid")
        }
    }
}

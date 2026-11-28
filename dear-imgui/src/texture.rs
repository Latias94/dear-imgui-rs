//! Texture management for Dear ImGui
//!
//! This module provides access to Dear ImGui's modern texture management system introduced in
//! version 1.92+. [`OwnedTextureData::from_pixels`] creates a complete CPU texture, while
//! [`TextureData::replace_pixels`] and [`TextureData::update_subresource`] perform transactional
//! mutations.
//!
//! [`TextureRegion`] is the `u32` input rectangle for a requested update. [`TextureSubresource`]
//! pairs that region with an explicit source row pitch and payload. [`TextureRect`] is the narrower
//! `u16` snapshot of rectangles already accepted by Dear ImGui's native update queue; it is not an
//! input builder.
//!
//! ```
//! use dear_imgui_rs::texture::{
//!     OwnedTextureData, TextureFormat, TextureRegion, TextureSubresource,
//! };
//!
//! let mut texture =
//!     OwnedTextureData::from_pixels(TextureFormat::Alpha8, 4, 2, &[0; 8])?;
//! let region = TextureRegion::new(1, 0, 2, 2)?;
//! texture.update_subresource(TextureSubresource::new(
//!     region,
//!     3,
//!     &[1, 2, 99, 3, 4],
//! ))?;
//! # Ok::<(), dear_imgui_rs::TextureError>(())
//! ```

mod data;
mod managed;
mod owned;

pub use crate::error::TextureError;
pub use data::TextureData;
pub use managed::{ManagedTextureMut, ManagedTextureRef};
pub use owned::OwnedTextureData;

use std::marker::PhantomData;
use std::num::{NonZeroU32, NonZeroU64, NonZeroUsize};

use crate::{ContextId, sys};

fn texture_format_bytes_per_pixel(format: TextureFormat) -> usize {
    match format {
        TextureFormat::RGBA32 => 4,
        TextureFormat::Alpha8 => 1,
    }
}

/// Texture format supported by Dear ImGui
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum TextureFormat {
    /// 4 components per pixel, each is unsigned 8-bit. Total size = TexWidth * TexHeight * 4
    RGBA32 = sys::ImTextureFormat_RGBA32,
    /// 1 component per pixel, each is unsigned 8-bit. Total size = TexWidth * TexHeight
    Alpha8 = sys::ImTextureFormat_Alpha8,
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

/// Get the number of bytes per pixel for a texture format
pub fn get_format_bytes_per_pixel(format: TextureFormat) -> usize {
    texture_format_bytes_per_pixel(format)
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

/// Status of a texture to communicate with Renderer Backend
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum TextureStatus {
    /// Texture is ready and can be used
    OK = sys::ImTextureStatus_OK,
    /// Backend destroyed the texture
    Destroyed = sys::ImTextureStatus_Destroyed,
    /// Requesting backend to create the texture. Set status OK when done.
    WantCreate = sys::ImTextureStatus_WantCreate,
    /// Requesting backend to update specific blocks of pixels. Set status OK when done.
    WantUpdates = sys::ImTextureStatus_WantUpdates,
    /// Requesting backend to destroy the texture. Set status to Destroyed when done.
    WantDestroy = sys::ImTextureStatus_WantDestroy,
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

/// Opaque renderer handle for an application-owned texture.
///
/// Dear ImGui stores legacy renderer handles as an unsigned 64-bit value. Integer, non-zero
/// integer, and raw-pointer handles can be converted into `TextureId` with [`From`]. Conversion
/// back to `u32` or `usize` uses [`TryFrom`] (or [`Self::try_as_u32`] and
/// [`Self::try_as_usize`]) so a handle cannot be silently truncated.
///
/// Use [`ManagedTextureId`] instead when the [`crate::Context`] owns the texture's pixels and
/// renderer lifecycle. Managed IDs intentionally do not convert to primitive handles because
/// their Context and generation are part of their identity.
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

    /// Try to view this texture id as a `usize`.
    ///
    /// Returns `None` if the id does not fit on this target.
    pub fn try_as_usize(self) -> Option<usize> {
        usize::try_from(self).ok()
    }

    /// Try to view this texture id as a `u32` handle.
    ///
    /// Returns `None` if the id does not fit in 32 bits.
    pub fn try_as_u32(self) -> Option<u32> {
        u32::try_from(self).ok()
    }

    /// Try to view this texture id as a raw pointer.
    ///
    /// Returns `None` if the id does not fit on this target.
    pub fn try_as_ptr<T>(self) -> Option<*const T> {
        self.try_as_usize().map(|value| value as *const T)
    }

    /// Try to view this texture id as a mutable raw pointer.
    ///
    /// Returns `None` if the id does not fit on this target.
    pub fn try_as_mut_ptr<T>(self) -> Option<*mut T> {
        self.try_as_usize().map(|value| value as *mut T)
    }
}

impl From<u64> for TextureId {
    #[inline]
    fn from(id: u64) -> Self {
        TextureId(id)
    }
}

impl From<u32> for TextureId {
    #[inline]
    fn from(id: u32) -> Self {
        Self(u64::from(id))
    }
}

impl From<NonZeroU32> for TextureId {
    #[inline]
    fn from(id: NonZeroU32) -> Self {
        Self(u64::from(id.get()))
    }
}

impl From<NonZeroU64> for TextureId {
    #[inline]
    fn from(id: NonZeroU64) -> Self {
        Self(id.get())
    }
}

impl From<NonZeroUsize> for TextureId {
    #[inline]
    fn from(id: NonZeroUsize) -> Self {
        Self(id.get() as u64)
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

// Renderer handles are commonly represented as pointer-sized integers.
impl From<usize> for TextureId {
    #[inline]
    fn from(id: usize) -> Self {
        TextureId(id as u64)
    }
}

impl TryFrom<TextureId> for usize {
    type Error = std::num::TryFromIntError;

    #[inline]
    fn try_from(id: TextureId) -> Result<Self, Self::Error> {
        Self::try_from(id.0)
    }
}

impl TryFrom<TextureId> for u32 {
    type Error = std::num::TryFromIntError;

    #[inline]
    fn try_from(id: TextureId) -> Result<Self, Self::Error> {
        Self::try_from(id.0)
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
        id.id()
    }
}

/// Opaque identity for a Context-owned managed texture.
///
/// The identity remains stable while the texture is active or retiring. It includes the owning
/// Context and a private slot generation, so a stale handle can never address a texture registered
/// later in a reused slot. Use [`TextureId`] for application-owned GPU texture identifiers.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ManagedTextureId {
    context: ContextId,
    slot: u32,
    generation: NonZeroU64,
}

impl ManagedTextureId {
    #[inline]
    pub(crate) const fn new(context: ContextId, slot: u32, generation: NonZeroU64) -> Self {
        Self {
            context,
            slot,
            generation,
        }
    }

    /// Returns the Context that owns this managed texture.
    #[inline]
    pub const fn context_id(self) -> ContextId {
        self.context
    }

    #[inline]
    pub(crate) const fn slot(self) -> u32 {
        self.slot
    }

    #[inline]
    pub(crate) const fn generation(self) -> NonZeroU64 {
        self.generation
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

/// A non-empty rectangular region of texture pixels.
///
/// Coordinates use `u32` because a region may be used to initialize a texture before the renderer
/// owns it. Live renderer updates are additionally checked against Dear ImGui's narrower native
/// update-rectangle representation when they are queued.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TextureRegion {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl TextureRegion {
    /// Create a non-empty texture region.
    ///
    /// # Errors
    ///
    /// Returns [`TextureError::InvalidRegionDimensions`] when either dimension is zero.
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Result<Self, TextureError> {
        if width == 0 || height == 0 {
            return Err(TextureError::InvalidRegionDimensions { width, height });
        }
        Ok(Self {
            x,
            y,
            width,
            height,
        })
    }

    /// Horizontal origin in pixels.
    #[must_use]
    pub const fn x(self) -> u32 {
        self.x
    }

    /// Vertical origin in pixels.
    #[must_use]
    pub const fn y(self) -> u32 {
        self.y
    }

    /// Region width in pixels.
    #[must_use]
    pub const fn width(self) -> u32 {
        self.width
    }

    /// Region height in pixels.
    #[must_use]
    pub const fn height(self) -> u32 {
        self.height
    }

    pub(crate) const fn from_validated_dimensions(x: u32, y: u32, width: u32, height: u32) -> Self {
        debug_assert!(width > 0 && height > 0);
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// A strided source payload for one texture subresource update.
#[derive(Clone, Copy, Debug)]
pub struct TextureSubresource<'pixels> {
    region: TextureRegion,
    row_pitch: usize,
    pixels: &'pixels [u8],
}

impl<'pixels> TextureSubresource<'pixels> {
    /// Describe a subresource update.
    ///
    /// Validation against the destination texture, pixel format, row pitch, and exact payload
    /// length occurs transactionally in [`super::TextureData::update_subresource`].
    #[must_use]
    pub const fn new(region: TextureRegion, row_pitch: usize, pixels: &'pixels [u8]) -> Self {
        Self {
            region,
            row_pitch,
            pixels,
        }
    }

    /// Destination region.
    #[must_use]
    pub const fn region(self) -> TextureRegion {
        self.region
    }

    /// Byte stride between source rows.
    #[must_use]
    pub const fn row_pitch(self) -> usize {
        self.row_pitch
    }

    /// Source pixel payload.
    #[must_use]
    pub const fn pixels(self) -> &'pixels [u8] {
        self.pixels
    }
}

/// A logical image source accepted by safe Dear ImGui drawing APIs.
///
/// Legacy [`TextureId`] values are forwarded directly. The `Ui` resolves managed handles through
/// its owning Context when submitting a draw command. The Context retains those textures for
/// the resulting frame. Font-atlas references borrow the atlas through their originating owner.
///
/// ```compile_fail
/// # use dear_imgui_rs::texture::{OwnedTextureData, TextureFormat, TextureRef};
/// let mut texture = OwnedTextureData::from_pixels(TextureFormat::RGBA32, 1, 1, &[0; 4]).unwrap();
/// let _: TextureRef<'_> = (&mut *texture).into();
/// ```
///
/// Raw `ImTextureRef` construction is a sys-layer escape hatch. The former safe helper is
/// intentionally unavailable:
///
/// ```compile_fail
/// use dear_imgui_rs::texture::{create_texture_ref, TextureId};
/// let _ = create_texture_ref(TextureId::new(1));
/// ```
#[derive(Copy, Clone, Debug)]
pub struct TextureRef<'texture> {
    source: TextureSource,
    _lease: PhantomData<&'texture ()>,
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum TextureSource {
    Legacy(TextureId),
    Managed(ManagedTextureId),
    FontAtlas {
        atlas: *mut sys::ImFontAtlas,
        texture: sys::ImTextureRef,
    },
}

impl<'texture> TextureRef<'texture> {
    pub(crate) fn from_font_atlas_raw(
        atlas: *mut sys::ImFontAtlas,
        texture: sys::ImTextureRef,
    ) -> Self {
        assert!(!atlas.is_null(), "font atlas texture requires an owner");
        Self {
            source: TextureSource::FontAtlas { atlas, texture },
            _lease: PhantomData,
        }
    }

    pub(crate) const fn source(self) -> TextureSource {
        self.source
    }

    /// Returns the embedded legacy texture ID, if this is a legacy reference.
    pub const fn legacy_id(self) -> Option<TextureId> {
        match self.source {
            TextureSource::Legacy(id) => Some(id),
            TextureSource::Managed(_) | TextureSource::FontAtlas { .. } => None,
        }
    }
}

impl<'texture> From<TextureId> for TextureRef<'texture> {
    #[inline]
    fn from(id: TextureId) -> Self {
        Self {
            source: TextureSource::Legacy(id),
            _lease: PhantomData,
        }
    }
}

impl<'texture> From<ManagedTextureId> for TextureRef<'texture> {
    #[inline]
    fn from(id: ManagedTextureId) -> Self {
        Self {
            source: TextureSource::Managed(id),
            _lease: PhantomData,
        }
    }
}

/// Resolve a raw texture reference without asserting that a managed texture has already been
/// uploaded by a renderer.
///
/// # Safety
///
/// A non-null `_TexData` pointer must be valid for reads.
pub(crate) unsafe fn effective_texture_id(raw: &sys::ImTextureRef) -> TextureId {
    if raw._TexData.is_null() {
        TextureId::from(raw._TexID)
    } else {
        unsafe { TextureId::from((*raw._TexData).TexID) }
    }
}

#[derive(Clone, Copy, Debug)]
struct TextureLayout {
    pub width: u32,
    pub height: u32,
    pub bytes_per_pixel: usize,
    pub row_pitch: usize,
    pub byte_len: usize,
}

impl TextureLayout {
    fn from_parts(width: u32, height: u32, bytes_per_pixel: usize) -> Result<Self, TextureError> {
        let row_pitch = usize::try_from(width)
            .ok()
            .and_then(|width| width.checked_mul(bytes_per_pixel));
        let byte_len = row_pitch.and_then(|pitch| {
            usize::try_from(height)
                .ok()
                .and_then(|height| pitch.checked_mul(height))
        });
        let Some((row_pitch, byte_len)) = row_pitch.zip(byte_len) else {
            return Err(TextureError::ByteSizeOutOfRange {
                width,
                height,
                bytes_per_pixel,
            });
        };
        if byte_len > i32::MAX as usize {
            return Err(TextureError::ByteSizeOutOfRange {
                width,
                height,
                bytes_per_pixel,
            });
        }
        Ok(Self {
            width,
            height,
            bytes_per_pixel,
            row_pitch,
            byte_len,
        })
    }
}

fn validate_new_texture_layout(
    format: TextureFormat,
    width: u32,
    height: u32,
) -> Result<TextureLayout, TextureError> {
    if width == 0 || height == 0 {
        return Err(TextureError::InvalidDimensions { width, height });
    }
    if i32::try_from(width).is_err() {
        return Err(TextureError::WidthOutOfRange(width));
    }
    if i32::try_from(height).is_err() {
        return Err(TextureError::HeightOutOfRange(height));
    }
    TextureLayout::from_parts(width, height, texture_format_bytes_per_pixel(format))
}

fn validate_native_texture_layout(
    width: i32,
    height: i32,
    bytes_per_pixel: i32,
) -> Result<TextureLayout, TextureError> {
    if width <= 0 || height <= 0 || bytes_per_pixel <= 0 {
        return Err(TextureError::InvalidLayout {
            width,
            height,
            bytes_per_pixel,
        });
    }
    let width = u32::try_from(width).expect("positive i32 width must fit u32");
    let height = u32::try_from(height).expect("positive i32 height must fit u32");
    let bytes_per_pixel =
        usize::try_from(bytes_per_pixel).expect("positive i32 bytes_per_pixel must fit usize");
    TextureLayout::from_parts(width, height, bytes_per_pixel)
}

fn require_exact_payload(expected: usize, actual: usize) -> Result<(), TextureError> {
    if expected == actual {
        Ok(())
    } else {
        Err(TextureError::ByteLengthMismatch { expected, actual })
    }
}

fn non_negative_texture_count_from_i32(caller: &str, raw: i32) -> usize {
    usize::try_from(raw).unwrap_or_else(|_| panic!("{caller} returned a negative count"))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod reference_tests {
    use super::*;

    #[test]
    fn logical_references_do_not_expose_managed_native_pointers() {
        let legacy = TextureId::new(7);
        assert_eq!(TextureRef::from(legacy).legacy_id(), Some(legacy));
        assert!(
            TextureRef::from(ManagedTextureId::new(
                crate::ContextId::allocate().expect("context id"),
                3,
                std::num::NonZeroU64::new(2).expect("generation"),
            ))
            .legacy_id()
            .is_none()
        );
    }
}

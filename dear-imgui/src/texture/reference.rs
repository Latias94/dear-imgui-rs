use super::{TextureData, TextureId};
use crate::sys;
use std::marker::PhantomData;

/// A convenient, typed wrapper around ImGui's ImTextureRef (v1.92+)
///
/// Can reference either a plain `TextureId` (legacy path) or a managed `TextureData`.
/// Managed texture references carry the lifetime of the referenced texture data; legacy
/// `TextureId` references can be converted into any texture-reference lifetime because they do not
/// borrow Rust texture data.
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
/// // via DrawData::textures_mut() when rendering this frame.
/// # }
/// ```
///
/// Managed references cannot be stored beyond the texture data they point at:
///
/// ```compile_fail
/// # use dear_imgui_rs::texture::{TextureData, TextureRef};
/// let leaked: TextureRef<'static>;
/// {
///     let mut tex = TextureData::new();
///     leaked = (&mut tex).into();
/// }
/// let _ = leaked.raw();
/// ```
///
/// Raw `ImTextureRef` values can contain arbitrary managed texture pointers, so constructing this
/// wrapper from raw data is unsafe:
///
/// ```compile_fail
/// # use dear_imgui_rs::{sys, texture::TextureRef};
/// let raw = sys::ImTextureRef {
///     _TexData: std::ptr::null_mut(),
///     _TexID: 0,
/// };
/// let _ = TextureRef::from_raw(raw);
/// ```
#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
pub struct TextureRef<'tex> {
    raw: sys::ImTextureRef,
    _marker: PhantomData<&'tex mut TextureData>,
}

// Ensure the wrapper stays layout-compatible with the sys bindings.
const _: [(); std::mem::size_of::<sys::ImTextureRef>()] =
    [(); std::mem::size_of::<TextureRef<'static>>()];
const _: [(); std::mem::align_of::<sys::ImTextureRef>()] =
    [(); std::mem::align_of::<TextureRef<'static>>()];

impl<'tex> TextureRef<'tex> {
    /// Create a texture reference from a raw ImGui texture ref.
    ///
    /// # Safety
    ///
    /// If `raw._TexData` is non-null, the caller must guarantee that it points to a valid
    /// `ImTextureData` for the entire `'tex` lifetime and that using the resulting reference does
    /// not violate Rust aliasing rules.
    #[inline]
    pub unsafe fn from_raw(raw: sys::ImTextureRef) -> Self {
        Self {
            raw,
            _marker: PhantomData,
        }
    }

    /// Get the underlying ImGui texture ref (by value)
    #[inline]
    pub fn raw(self) -> sys::ImTextureRef {
        self.raw
    }
}

impl<'tex> From<TextureId> for TextureRef<'tex> {
    #[inline]
    fn from(id: TextureId) -> Self {
        TextureRef {
            raw: sys::ImTextureRef {
                _TexData: std::ptr::null_mut(),
                _TexID: id.id() as sys::ImTextureID,
            },
            _marker: PhantomData,
        }
    }
}

impl<'tex> From<&TextureData> for TextureRef<'tex> {
    #[inline]
    fn from(td: &TextureData) -> Self {
        // Safety: A shared `&TextureData` must not be used to give Dear ImGui a mutable
        // `ImTextureData*` because ImGui/backends may mutate fields such as `Status`/`TexID`
        // during the frame, which would violate Rust aliasing rules.
        //
        // We therefore treat `&TextureData` as a legacy reference: only forward the current
        // `TexID` value (if any). For managed textures, pass `&mut TextureData` instead.
        TextureRef {
            raw: sys::ImTextureRef {
                _TexData: std::ptr::null_mut(),
                _TexID: td.tex_id().id() as sys::ImTextureID,
            },
            _marker: PhantomData,
        }
    }
}

impl<'tex> From<&'tex mut TextureData> for TextureRef<'tex> {
    #[inline]
    fn from(td: &'tex mut TextureData) -> Self {
        TextureRef {
            raw: sys::ImTextureRef {
                _TexData: td.as_raw_mut(),
                _TexID: 0,
            },
            _marker: PhantomData,
        }
    }
}

/// Create an ImTextureRef from a texture ID.
///
/// This is the safe way to create an ImTextureRef for use with Dear ImGui.
/// Use this instead of directly constructing the sys::ImTextureRef structure.
pub fn create_texture_ref(texture_id: TextureId) -> sys::ImTextureRef {
    sys::ImTextureRef {
        _TexData: std::ptr::null_mut(),
        _TexID: texture_id.id() as sys::ImTextureID,
    }
}

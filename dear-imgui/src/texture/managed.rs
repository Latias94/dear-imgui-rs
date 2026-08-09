use super::{
    TextureData, TextureDataError, TextureFormat, TextureId, TextureRect, TextureStatus,
    TextureSubresource,
};

/// Read-only access to a Context-owned managed texture.
///
/// This facade deliberately omits raw pointers and backend-owned fields. Its lifetime is scoped by
/// [`crate::Context::with_texture`], so borrowed pixel data cannot escape the Context operation.
#[derive(Copy, Clone)]
pub struct ManagedTextureRef<'texture> {
    texture: &'texture TextureData,
}

impl<'texture> ManagedTextureRef<'texture> {
    pub(crate) fn new(texture: &'texture TextureData) -> Self {
        Self { texture }
    }

    /// Current renderer lifecycle status.
    #[must_use]
    pub fn status(self) -> TextureStatus {
        self.texture.status()
    }

    /// Renderer texture identifier, or the null identifier before creation and after destruction.
    #[must_use]
    pub fn texture_id(self) -> TextureId {
        self.texture.tex_id()
    }

    /// Pixel format requested from the renderer.
    #[must_use]
    pub fn format(self) -> TextureFormat {
        self.texture.format()
    }

    /// Texture width in pixels.
    #[must_use]
    pub fn width(self) -> u32 {
        self.texture.width()
    }

    /// Texture height in pixels.
    #[must_use]
    pub fn height(self) -> u32 {
        self.texture.height()
    }

    /// Number of bytes stored for each pixel.
    #[must_use]
    pub fn bytes_per_pixel(self) -> usize {
        self.texture.bytes_per_pixel()
    }

    /// Number of frames since the texture was referenced by Dear ImGui.
    #[must_use]
    pub fn unused_frames(self) -> usize {
        self.texture.unused_frames()
    }

    /// Dear ImGui's current reference count for this texture.
    #[must_use]
    pub fn ref_count(self) -> u16 {
        self.texture.ref_count()
    }

    /// Whether the texture stores color channels rather than white plus alpha.
    #[must_use]
    pub fn uses_colors(self) -> bool {
        self.texture.use_colors()
    }

    /// Whether Dear ImGui queued this texture for retirement on the next frame.
    #[must_use]
    pub fn is_queued_for_destruction(self) -> bool {
        self.texture.want_destroy_next_frame()
    }

    /// Complete pixel buffer, when CPU-side storage is available.
    #[must_use]
    pub fn pixels(self) -> Option<&'texture [u8]> {
        self.texture.pixels()
    }

    /// Pixel data beginning at `(x, y)`, when the coordinate and storage are valid.
    #[must_use]
    pub fn pixels_at(self, x: u32, y: u32) -> Option<&'texture [u8]> {
        self.texture.pixels_at(x, y)
    }

    /// Number of bytes in one complete texture row.
    #[must_use]
    pub fn pitch(self) -> usize {
        self.texture.pitch()
    }

    /// Bounding rectangle containing every used pixel.
    #[must_use]
    pub fn used_rect(self) -> TextureRect {
        self.texture.used_rect()
    }

    /// Bounding rectangle covering queued pixel changes.
    #[must_use]
    pub fn update_rect(self) -> TextureRect {
        self.texture.update_rect()
    }

    /// Individual queued update rectangles.
    pub fn updates(self) -> impl Iterator<Item = TextureRect> + 'texture {
        self.texture.updates()
    }
}

/// Restricted mutable access to a Context-owned managed texture.
///
/// Applications can update pixel data and inspect metadata, but renderer-owned status, texture
/// identifiers, backend data, and the native allocation remain behind the Context protocol.
///
/// The former unchecked managed setter is intentionally unavailable:
///
/// ```compile_fail
/// use dear_imgui_rs::ManagedTextureMut;
/// fn replace(mut texture: ManagedTextureMut<'_>) { texture.set_data(&[0; 4]); }
/// ```
pub struct ManagedTextureMut<'texture> {
    texture: &'texture mut TextureData,
    mutated: &'texture mut bool,
}

impl<'texture> ManagedTextureMut<'texture> {
    pub(crate) fn new(texture: &'texture mut TextureData, mutated: &'texture mut bool) -> Self {
        Self { texture, mutated }
    }

    /// Replace every pixel using the destination's exact tightly packed byte length.
    ///
    /// Validation is transactional: an error leaves pixels, status, and queued update rectangles
    /// unchanged. See [`TextureData::replace_pixels`] for the complete lifecycle contract.
    ///
    /// # Errors
    ///
    /// Returns [`TextureDataError`] when the texture is not mutable, its layout is invalid, the
    /// payload length is not exact, or a live full update is not natively representable.
    pub fn replace_pixels(&mut self, pixels: &[u8]) -> Result<(), TextureDataError> {
        let result = self.texture.replace_pixels(pixels);
        if result.is_ok() {
            *self.mutated = true;
        }
        result
    }

    /// Copy one strided source payload into the requested texture region.
    ///
    /// The exact payload length is `(height - 1) * row_pitch + tight_row_bytes`; final-row padding
    /// is not accepted. Validation is transactional. In `WantCreate`, the operation changes the
    /// initial pixels without queuing a rectangle; in `OK` or `WantUpdates`, it queues the region
    /// and ends in `WantUpdates`. See [`TextureData::update_subresource`] for the complete contract.
    ///
    /// # Errors
    ///
    /// Returns [`TextureDataError`] when the texture is not mutable, the region is invalid, the
    /// row pitch or payload length is wrong, or the native update rectangle is not representable.
    pub fn update_subresource(
        &mut self,
        update: TextureSubresource<'_>,
    ) -> Result<(), TextureDataError> {
        let result = self.texture.update_subresource(update);
        if result.is_ok() {
            *self.mutated = true;
        }
        result
    }

    /// Inspect the texture without exposing renderer-owned fields or native pointers.
    #[must_use]
    pub fn view(&self) -> ManagedTextureRef<'_> {
        ManagedTextureRef::new(self.texture)
    }
}

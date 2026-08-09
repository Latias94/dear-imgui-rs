use super::TextureDataError;

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
    /// Returns [`TextureDataError::InvalidRegionDimensions`] when either dimension is zero.
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Result<Self, TextureDataError> {
        if width == 0 || height == 0 {
            return Err(TextureDataError::InvalidRegionDimensions { width, height });
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

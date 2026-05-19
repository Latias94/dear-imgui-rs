/// Handle to a font atlas texture
#[derive(Clone, Debug)]
pub struct FontAtlasTexture<'a> {
    /// Texture width (in pixels)
    pub width: u32,
    /// Texture height (in pixels)
    pub height: u32,
    /// Raw texture data (in bytes).
    ///
    /// The format depends on which function was called to obtain this data:
    /// - For RGBA32: 4 bytes per pixel (R, G, B, A)
    /// - For Alpha8: 1 byte per pixel (Alpha only)
    pub data: &'a [u8],
}

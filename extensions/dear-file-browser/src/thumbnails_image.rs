use image::GenericImageView;
use image::imageops::FilterType;

use crate::{DecodedRgbaImage, ThumbnailProvider, ThumbnailRequest};

/// Default thumbnail decoder using the `image` crate.
///
/// This provider:
/// - reads `req.path` using `std::fs`,
/// - decodes the file via `image::load_from_memory`,
/// - downscales to fit `req.max_size` (no upscaling),
/// - returns an RGBA8 buffer.
///
/// Notes:
/// - This is intended for native builds. It does not work on `wasm32` without a custom filesystem.
/// - Supported formats depend on enabled `image` crate features (this crate enables PNG + JPEG).
#[derive(Clone, Debug)]
pub struct ImageThumbnailProvider {
    /// Downscale filter.
    pub filter: FilterType,
}

impl Default for ImageThumbnailProvider {
    fn default() -> Self {
        Self {
            filter: FilterType::Triangle,
        }
    }
}

impl ImageThumbnailProvider {
    /// Create a provider with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a provider with the given resize filter.
    pub fn with_filter(filter: FilterType) -> Self {
        Self { filter }
    }
}

impl ThumbnailProvider for ImageThumbnailProvider {
    fn decode(&mut self, req: &ThumbnailRequest) -> Result<DecodedRgbaImage, String> {
        let bytes =
            std::fs::read(&req.path).map_err(|e| format!("read {}: {e}", req.path.display()))?;
        let mut img = image::load_from_memory(&bytes)
            .map_err(|e| format!("decode {}: {e}", req.path.display()))?;

        let max_w = req.max_size[0].max(1);
        let max_h = req.max_size[1].max(1);
        let (w, h) = img.dimensions();
        if w > max_w || h > max_h {
            img = img.resize(max_w, max_h, self.filter);
        }

        let rgba = img.to_rgba8();
        Ok(DecodedRgbaImage {
            width: rgba.width(),
            height: rgba.height(),
            rgba: rgba.into_raw(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_and_downscales_png() {
        use image::codecs::png::PngEncoder;
        use image::{ColorType, ImageEncoder, RgbaImage};

        let mut img = RgbaImage::new(16, 8);
        for p in img.pixels_mut() {
            *p = image::Rgba([10, 20, 30, 40]);
        }

        let mut bytes = Vec::new();
        let enc = PngEncoder::new(&mut bytes);
        enc.write_image(&img, img.width(), img.height(), ColorType::Rgba8.into())
            .unwrap();

        let dir = std::env::temp_dir().join("dear-file-browser-tests");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("thumb_test.png");
        std::fs::write(&path, &bytes).unwrap();

        let mut p = ImageThumbnailProvider::default();
        let out = p
            .decode(&ThumbnailRequest {
                path: path.clone(),
                max_size: [8, 8],
            })
            .unwrap();
        assert!(out.width <= 8);
        assert!(out.height <= 8);
        assert_eq!(out.rgba.len() as u32, out.width * out.height * 4);
    }
}

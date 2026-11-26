use std::fs::{self, File};
use std::io::{Cursor, Read};

use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageReader, Limits};

use crate::{DecodedRgbaImage, ThumbnailProvider, ThumbnailRequest};

const MAX_DECODE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_DIMENSION: u32 = 32_768;

/// Default thumbnail decoder using the `image` crate.
///
/// This provider:
/// - accepts regular files with a `.png`, `.jpg`, or `.jpeg` extension (case insensitive),
/// - reads up to the configured input size limit using `std::fs`,
/// - checks dimensions and decoded pixel bytes before allocating the full image,
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
    /// Maximum input file size in bytes. Defaults to 32 MiB.
    pub max_bytes: u64,
}

impl Default for ImageThumbnailProvider {
    fn default() -> Self {
        Self {
            filter: FilterType::Triangle,
            max_bytes: 32 * 1024 * 1024,
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
        Self {
            filter,
            ..Self::default()
        }
    }
}

impl ThumbnailProvider for ImageThumbnailProvider {
    fn decode(&mut self, req: &ThumbnailRequest) -> Result<DecodedRgbaImage, String> {
        let path = &req.path;
        if !path.extension().is_some_and(|ext| {
            ["png", "jpg", "jpeg"]
                .into_iter()
                .any(|allowed| ext.eq_ignore_ascii_case(allowed))
        }) {
            return Err(format!(
                "unsupported thumbnail extension: {}",
                path.display()
            ));
        }
        let metadata =
            fs::metadata(path).map_err(|e| format!("inspect {}: {e}", path.display()))?;
        if !metadata.is_file() {
            return Err(format!("expected a regular file: {}", path.display()));
        }
        let too_large = || {
            format!(
                "thumbnail input exceeds {} bytes: {}",
                self.max_bytes,
                path.display()
            )
        };
        if metadata.len() > self.max_bytes {
            return Err(too_large());
        }

        let file = File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
        let mut bytes = Vec::new();
        // Bound the read even if the file grows or is replaced after the metadata lookup.
        file.take(self.max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|e| format!("read {}: {e}", path.display()))?;
        if bytes.len() as u64 > self.max_bytes {
            return Err(too_large());
        }

        let mut reader = ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|e| format!("detect {}: {e}", path.display()))?;
        let mut limits = Limits::default();
        limits.max_image_width = Some(MAX_DIMENSION);
        limits.max_image_height = Some(MAX_DIMENSION);
        limits.max_alloc = Some(MAX_DECODE_BYTES);
        reader.limits(limits);
        let decoder = reader
            .into_decoder()
            .map_err(|e| format!("decode {}: {e}", path.display()))?;
        let (width, height) = decoder.dimensions();
        if decoder.total_bytes() > MAX_DECODE_BYTES
            || u64::from(width) * u64::from(height) * 4 > MAX_DECODE_BYTES
        {
            return Err(format!(
                "decoded thumbnail exceeds {MAX_DECODE_BYTES} bytes: {}",
                path.display()
            ));
        }

        let max_w = req.max_size[0].max(1);
        let max_h = req.max_size[1].max(1);
        let mut img = DynamicImage::from_decoder(decoder)
            .map_err(|e| format!("decode {}: {e}", path.display()))?;
        if width > max_w || height > max_h {
            img = img.resize(max_w, max_h, self.filter);
        }

        let rgba = img.into_rgba8();
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
    use crate::fs_ops::tests::TempDir;

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

        let temp = TempDir::new();
        let path = temp.path().join("thumb_test.PNG");
        fs::write(&path, &bytes).unwrap();

        let mut provider = ImageThumbnailProvider::new();
        provider.max_bytes = bytes.len() as u64;
        let out = provider
            .decode(&ThumbnailRequest::new(path, [8, 8]))
            .unwrap();
        assert!(out.width <= 8);
        assert!(out.height <= 8);
        assert_eq!(out.rgba.len() as u32, out.width * out.height * 4);
    }

    #[test]
    fn rejects_unsupported_extensions_before_accessing_the_file() {
        let temp = TempDir::new();
        let error = ImageThumbnailProvider::default()
            .decode(&ThumbnailRequest::new(
                temp.path().join("missing.txt"),
                [8, 8],
            ))
            .unwrap_err();
        assert!(error.starts_with("unsupported thumbnail extension"));
    }

    #[test]
    fn rejects_files_over_the_default_size_limit_before_decoding() {
        let temp = TempDir::new();
        let path = temp.path().join("large.jpeg");
        let max_bytes = 32 * 1024 * 1024;
        File::create(&path).unwrap().set_len(max_bytes + 1).unwrap();
        let mut provider = ImageThumbnailProvider::default();
        assert_eq!(provider.max_bytes, max_bytes);
        let error = provider
            .decode(&ThumbnailRequest::new(path, [8, 8]))
            .unwrap_err();
        assert!(error.starts_with("thumbnail input exceeds 33554432 bytes"));
    }
}

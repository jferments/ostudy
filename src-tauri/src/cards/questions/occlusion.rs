//! Bounded, platform-independent raster preparation for image occlusion.
//! No paths, IPC, database, scheduler, or original metadata cross this boundary.
//! Callers retain the canonical PNG as a managed asset and authorize reveal
//! BEFORE selecting a render phase. A masked PNG, not a CSS overlay, is the
//! pre-answer image. Cache entries are derivatives, never canonical content.

use std::{collections::BTreeSet, io::Cursor, io::Write};

use image::codecs::png::{CompressionType, FilterType as PngFilter};
use image::{
    codecs::{jpeg::JpegDecoder, png::PngDecoder, png::PngEncoder, webp::WebPDecoder},
    imageops::{resize, FilterType},
    DynamicImage, ExtendedColorType, ImageDecoder, ImageEncoder, ImageFormat, Limits, Rgba,
    RgbaImage,
};
use sha2::{Digest, Sha256};

use super::{NormalizedRect, QuestionError, ValidatedQuestion};

pub const RENDER_POLICY_VERSION: u32 = 3;
pub const MAX_IMAGE_BYTES: usize = 25 * 1024 * 1024;
pub const MAX_IMAGE_EDGE: u32 = 8192;
pub const MAX_IMAGE_PIXELS: u64 = 16_000_000;
pub const MAX_RENDER_EDGE: u32 = 4096;
const MAX_DECODE_BYTES: u64 = 128 * 1024 * 1024;
const MASK_PIXEL: Rgba<u8> = Rgba([48, 52, 61, 255]);

/// Native-only choice, never a client-supplied authorization to reveal an answer.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RenderPhase {
    Hidden,
    Revealed,
}

/// Pixels have one upright orientation. Encoding never copies EXIF, filenames,
/// thumbnails, comments or answer-bearing metadata from the input container.
pub struct CanonicalImage {
    pixels: RgbaImage,
    fingerprint: String,
}

pub struct RenderedOcclusion {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub cache_key: String,
    /// Hidden-side highlight geometry only; revealed images have no overlays.
    pub target_rectangles: Vec<NormalizedRect>,
}

impl CanonicalImage {
    pub fn decode(bytes: &[u8]) -> Result<Self, QuestionError> {
        if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
            return Err(image_error("Use a nonempty image no larger than 25 MB."));
        }
        std::panic::catch_unwind(|| decode_still(bytes))
            .map_err(|_| image_error("The image decoder could not read this file."))?
    }

    pub fn dimensions(&self) -> (u32, u32) {
        self.pixels.dimensions()
    }

    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn canonical_png(&self) -> Result<Vec<u8>, QuestionError> {
        encode_png(&self.pixels)
    }

    pub fn render(
        &self,
        question: &ValidatedQuestion,
        presentation_key: &str,
        phase: RenderPhase,
        maximum_edge: u32,
    ) -> Result<RenderedOcclusion, QuestionError> {
        if !(1..=MAX_RENDER_EDGE).contains(&maximum_edge) {
            return Err(image_error(
                "Choose a supported occlusion image resolution.",
            ));
        }
        let plan = question.occlusion_mask_plan(presentation_key)?;
        let targets = plan
            .target_mask_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let target_rectangles = plan
            .hidden_masks
            .iter()
            .filter(|mask| phase == RenderPhase::Hidden && targets.contains(mask.id.as_str()))
            .map(|mask| mask.rect)
            .collect::<Vec<_>>();
        let mut hidden = plan
            .hidden_masks
            .iter()
            .filter(|_| phase == RenderPhase::Hidden)
            .collect::<Vec<_>>();
        hidden.sort_by(|left, right| left.id.cmp(&right.id));
        let (width, height) = self.dimensions();
        let mut pixels = self.pixels.clone();
        for mask in &hidden {
            paint(&mut pixels, mask.rect);
        }
        let largest = width.max(height);
        if largest > maximum_edge {
            let output_width =
                ((u64::from(width) * u64::from(maximum_edge)) / u64::from(largest)).max(1) as u32;
            let output_height =
                ((u64::from(height) * u64::from(maximum_edge)) / u64::from(largest)).max(1) as u32;
            pixels = resize(&pixels, output_width, output_height, FilterType::Triangle);
        }
        let mut hash = Sha256::new();
        hash.update(RENDER_POLICY_VERSION.to_le_bytes());
        hash.update(self.fingerprint.as_bytes());
        hash.update(pixels.width().to_le_bytes());
        hash.update(pixels.height().to_le_bytes());
        hash.update([u8::from(phase == RenderPhase::Revealed)]);
        for mask in &plan.hidden_masks {
            hash.update(mask.id.as_bytes());
            hash.update([u8::from(targets.contains(mask.id.as_str()))]);
            for value in [mask.rect.x, mask.rect.y, mask.rect.width, mask.rect.height] {
                hash.update(value.to_le_bytes());
            }
        }
        let compression = if pixels.width().max(pixels.height()) <= 2048 {
            CompressionType::Fast
        } else {
            CompressionType::Default
        };
        Ok(RenderedOcclusion {
            png: encode_png_with_quality(&pixels, compression, PngFilter::Sub)?,
            width: pixels.width(),
            height: pixels.height(),
            cache_key: format!("{:x}", hash.finalize()),
            target_rectangles,
        })
    }
}

fn decode_still(bytes: &[u8]) -> Result<CanonicalImage, QuestionError> {
    let format = image::guess_format(bytes).map_err(codec_error)?;
    let reader = Cursor::new(bytes);
    match format {
        ImageFormat::Png => {
            let decoder = PngDecoder::with_limits(reader, limits()).map_err(codec_error)?;
            if decoder.is_apng().map_err(codec_error)? {
                return Err(animated_error());
            }
            decode_pixels(decoder)
        }
        ImageFormat::Jpeg => decode_pixels(JpegDecoder::new(reader).map_err(codec_error)?),
        ImageFormat::WebP => {
            let decoder = WebPDecoder::new(reader).map_err(codec_error)?;
            if decoder.has_animation() {
                return Err(animated_error());
            }
            decode_pixels(decoder)
        }
        _ => Err(image_error(
            "Use a still PNG, JPEG or WebP image for image occlusion.",
        )),
    }
}

fn decode_pixels(mut decoder: impl ImageDecoder) -> Result<CanonicalImage, QuestionError> {
    decoder.set_limits(limits()).map_err(codec_error)?;
    let (width, height) = decoder.dimensions();
    if width == 0
        || height == 0
        || width > MAX_IMAGE_EDGE
        || height > MAX_IMAGE_EDGE
        || u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS
        || decoder.total_bytes() > MAX_DECODE_BYTES
    {
        return Err(image_error(
            "The image exceeds the supported decoded size. Use a smaller image.",
        ));
    }
    let orientation = decoder.orientation().map_err(codec_error)?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(codec_error)?;
    image.apply_orientation(orientation);
    let pixels = image.into_rgba8();
    let mut hash = Sha256::new();
    hash.update(pixels.width().to_le_bytes());
    hash.update(pixels.height().to_le_bytes());
    hash.update(pixels.as_raw());
    Ok(CanonicalImage {
        pixels,
        fingerprint: format!("{:x}", hash.finalize()),
    })
}

fn limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_EDGE);
    limits.max_image_height = Some(MAX_IMAGE_EDGE);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    limits
}

fn paint(pixels: &mut RgbaImage, rectangle: NormalizedRect) {
    let width = f64::from(pixels.width());
    let height = f64::from(pixels.height());
    let left = (rectangle.x * width).floor() as u32;
    let top = (rectangle.y * height).floor() as u32;
    let right = ((rectangle.x + rectangle.width) * width).ceil() as u32;
    let bottom = ((rectangle.y + rectangle.height) * height).ceil() as u32;
    for y in top.saturating_sub(1)..bottom.saturating_add(1).min(pixels.height()) {
        for x in left.saturating_sub(1)..right.saturating_add(1).min(pixels.width()) {
            pixels.put_pixel(x, y, MASK_PIXEL);
        }
    }
}

fn encode_png(pixels: &RgbaImage) -> Result<Vec<u8>, QuestionError> {
    encode_png_with_quality(pixels, CompressionType::Default, PngFilter::Adaptive)
}

fn encode_png_with_quality(
    pixels: &RgbaImage,
    compression: CompressionType,
    filter: PngFilter,
) -> Result<Vec<u8>, QuestionError> {
    let mut output = BoundedOutput(Vec::new());
    PngEncoder::new_with_quality(&mut output, compression, filter)
        .write_image(
            pixels.as_raw(),
            pixels.width(),
            pixels.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(codec_error)?;
    Ok(output.0)
}

struct BoundedOutput(Vec<u8>);

impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_IMAGE_BYTES.saturating_sub(self.0.len()) {
            return Err(std::io::Error::other("Occlusion PNG exceeds 25 MB."));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn image_error(message: &'static str) -> QuestionError {
    QuestionError::new("invalid_occlusion_image", message)
}

fn animated_error() -> QuestionError {
    image_error("Animated images are not supported. Export a still frame first.")
}

fn codec_error(_: image::ImageError) -> QuestionError {
    image_error("The image is malformed, unsupported or exceeds an image-processing limit.")
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod encoding_tests {
    use super::*;

    #[test]
    fn interactive_png_filters_preserve_exact_pixels_and_alpha() {
        let pixels = RgbaImage::from_fn(127, 93, |x, y| {
            Rgba([x as u8, y as u8, (x ^ y) as u8, ((x + y) % 256) as u8])
        });
        for (compression, filter) in [
            (CompressionType::Default, PngFilter::Adaptive),
            (CompressionType::Fast, PngFilter::Sub),
        ] {
            let png = encode_png_with_quality(&pixels, compression, filter).unwrap();
            let decoded = image::load_from_memory(&png).unwrap().into_rgba8();
            assert_eq!(decoded, pixels);
        }
    }
}

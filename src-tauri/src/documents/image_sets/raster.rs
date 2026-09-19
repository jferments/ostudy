//! Source-preserving image transforms. OCR sees upright full-resolution pixels;
//! model-visible derivatives contain only pixels inside explicit inclusion bounds.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
};

use image::{codecs::png::PngEncoder, imageops, ExtendedColorType, ImageEncoder, Rgba, RgbaImage};

use crate::{
    cards::questions::occlusion::{CanonicalImage, MAX_IMAGE_BYTES},
    document_canonical::sha256_hex,
    error::CommandError,
    paths::AppPaths,
    pdf_ocr_backend::regions::PixelBounds,
};

use super::manifest::{invalid, ImageMember, InclusionCrop};

pub(crate) struct Raster {
    pixels: RgbaImage,
}

impl Raster {
    pub(crate) fn read(paths: &AppPaths, member: &ImageMember) -> Result<Self, CommandError> {
        let mut bytes = Vec::new();
        File::open(paths.resolve_relative(&member.relative_path))
            .map_err(CommandError::io)?
            .take(MAX_IMAGE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(CommandError::io)?;
        if bytes.len() as u64 != member.byte_size || sha256_hex(&bytes) != member.sha256 {
            return Err(invalid("The original image changed after admission."));
        }
        let image = CanonicalImage::decode(&bytes)?;
        if image.dimensions() != (member.width, member.height)
            || image.fingerprint() != member.pixel_sha256
        {
            return Err(invalid(
                "The original image no longer matches its admitted pixels.",
            ));
        }
        // Decode only the bounded native PNG, never a second unadmitted image.
        let png = image.canonical_png()?;
        let upright = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
            .map_err(|_| invalid("The upright image could not be decoded."))?
            .into_rgba8();
        let pixels = match member.quarter_turns {
            0 => upright,
            1 => imageops::rotate90(&upright),
            2 => imageops::rotate180(&upright),
            3 => imageops::rotate270(&upright),
            _ => return Err(invalid("Unsupported image orientation.")),
        };
        Ok(Self { pixels })
    }

    pub(crate) fn dimensions(&self) -> (u32, u32) {
        self.pixels.dimensions()
    }

    pub(crate) fn png(&self) -> Result<Vec<u8>, CommandError> {
        encode(&self.pixels)
    }

    pub(crate) fn filtered_png(
        &self,
        inclusion: PixelBounds,
        excluded: &[PixelBounds],
    ) -> Result<(Vec<u8>, u32, u32), CommandError> {
        let (width, height) = self.dimensions();
        if !inclusion.within(width, height)
            || excluded.iter().any(|bounds| !bounds.within(width, height))
        {
            return Err(invalid("Invalid image inclusion or exclusion geometry."));
        }
        let mut pixels = self.pixels.clone();
        for (x, y, pixel) in pixels.enumerate_pixels_mut() {
            if x < inclusion.left
                || x >= inclusion.right
                || y < inclusion.top
                || y >= inclusion.bottom
            {
                *pixel = Rgba([255, 255, 255, 255]);
            }
        }
        for bounds in excluded {
            for y in bounds.top..bounds.bottom {
                for x in bounds.left..bounds.right {
                    pixels.put_pixel(x, y, Rgba([255, 255, 255, 255]));
                }
            }
        }
        // Evidence coordinates remain normalized against the full raster.
        // Original inspection remains full-resolution; provider images are bounded.
        let edge = width.max(height);
        if edge > 2048 {
            pixels = imageops::resize(
                &pixels,
                (u64::from(width) * 2048 / u64::from(edge)).max(1) as u32,
                (u64::from(height) * 2048 / u64::from(edge)).max(1) as u32,
                imageops::FilterType::Triangle,
            );
        }
        let (width, height) = pixels.dimensions();
        Ok((encode(&pixels)?, width, height))
    }
}

/// Round inward: not even one excluded edge pixel is admitted by float rounding.
pub(crate) fn inclusion(member: &ImageMember) -> Result<PixelBounds, CommandError> {
    let crop = member.inclusion.unwrap_or(InclusionCrop {
        left: 0.0,
        top: 0.0,
        right: 1.0,
        bottom: 1.0,
    });
    crop.validate()?;
    let bounds = PixelBounds {
        left: (crop.left * f64::from(member.width)).ceil() as u32,
        top: (crop.top * f64::from(member.height)).ceil() as u32,
        right: (crop.right * f64::from(member.width)).floor() as u32,
        bottom: (crop.bottom * f64::from(member.height)).floor() as u32,
    };
    if !bounds.within(member.width, member.height) {
        return Err(invalid(
            "The inclusion crop contains no complete source pixels.",
        ));
    }
    rotate_bounds(bounds, member.width, member.height, member.quarter_turns)
}

pub(crate) fn contains(outer: PixelBounds, inner: PixelBounds) -> bool {
    outer.left <= inner.left
        && outer.top <= inner.top
        && outer.right >= inner.right
        && outer.bottom >= inner.bottom
}

fn rotate_bounds(
    bounds: PixelBounds,
    width: u32,
    height: u32,
    turns: u8,
) -> Result<PixelBounds, CommandError> {
    if !bounds.within(width, height) {
        return Err(invalid("Invalid source image bounds."));
    }
    Ok(match turns {
        0 => bounds,
        1 => PixelBounds {
            left: height - bounds.bottom,
            top: bounds.left,
            right: height - bounds.top,
            bottom: bounds.right,
        },
        2 => PixelBounds {
            left: width - bounds.right,
            top: height - bounds.bottom,
            right: width - bounds.left,
            bottom: height - bounds.top,
        },
        3 => PixelBounds {
            left: bounds.top,
            top: width - bounds.right,
            right: bounds.bottom,
            bottom: width - bounds.left,
        },
        _ => return Err(invalid("Unsupported source image rotation.")),
    })
}

fn encode(pixels: &RgbaImage) -> Result<Vec<u8>, CommandError> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(
            pixels.as_raw(),
            pixels.width(),
            pixels.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(|_| invalid("The admitted image could not be encoded."))?;
    Ok(bytes)
}

/// Content-addressed files cannot overwrite an older in-flight source snapshot.
/// Decimal components preserve the existing managed source-asset path contract.
pub(crate) fn retain_derivative(
    paths: &AppPaths,
    document_id: &str,
    member_id: &str,
    bytes: &[u8],
) -> Result<String, CommandError> {
    crate::util::validate_entity_id(document_id, "image-set document")?;
    crate::util::validate_entity_id(member_id, "image member")?;
    if bytes.is_empty() || bytes.len() > 32 * 1024 * 1024 {
        return Err(invalid(
            "The image derivative exceeds its bounded transfer size.",
        ));
    }
    let digest = sha256_hex(format!("{member_id}:{}", sha256_hex(bytes)).as_bytes());
    let high = u128::from_str_radix(&digest[..32], 16)
        .map_err(|_| invalid("Invalid native image digest."))?;
    let low = u128::from_str_radix(&digest[32..], 16)
        .map_err(|_| invalid("Invalid native image digest."))?;
    let relative = format!("documents/{document_id}/assets/asset-{high:039}{low:039}.png");
    let path = paths.resolve_relative(&relative);
    fs::create_dir_all(
        path.parent()
            .ok_or_else(|| invalid("Missing image asset directory."))?,
    )
    .map_err(CommandError::io)?;
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
                let _ = fs::remove_file(&path);
                return Err(CommandError::io(error));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let mut saved = Vec::new();
            File::open(&path)
                .map_err(CommandError::io)?
                .take(32 * 1024 * 1024 + 1)
                .read_to_end(&mut saved)
                .map_err(CommandError::io)?;
            if saved != bytes {
                return Err(invalid(
                    "A retained image derivative failed its content check.",
                ));
            }
        }
        Err(error) => return Err(CommandError::io(error)),
    }
    Ok(relative)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_rotation_preserves_exact_asymmetric_region_coordinates() {
        let bounds = PixelBounds {
            left: 1,
            top: 2,
            right: 4,
            bottom: 6,
        };
        let rotated = rotate_bounds(bounds, 8, 10, 1).unwrap();
        assert_eq!(
            rotated,
            PixelBounds {
                left: 4,
                top: 1,
                right: 8,
                bottom: 4
            }
        );
        assert_eq!(rotate_bounds(rotated, 10, 8, 3).unwrap(), bounds);
        assert_eq!(
            rotate_bounds(rotate_bounds(bounds, 8, 10, 2).unwrap(), 8, 10, 2).unwrap(),
            bounds
        );
        assert!(rotate_bounds(bounds, 8, 10, 4).is_err());
    }

    #[test]
    fn excluded_pixels_are_absent_from_the_actual_encoded_derivative() {
        let pixels = RgbaImage::from_pixel(8, 6, Rgba([2, 30, 70, 255]));
        let raster = Raster {
            pixels: pixels.clone(),
        };
        let crop = PixelBounds {
            left: 1,
            top: 1,
            right: 7,
            bottom: 5,
        };
        let hidden = PixelBounds {
            left: 2,
            top: 2,
            right: 4,
            bottom: 4,
        };
        let (png, width, height) = raster.filtered_png(crop, &[hidden]).unwrap();
        let decoded = image::load_from_memory(&png).unwrap().into_rgba8();
        assert_eq!((width, height), (8, 6));
        for (x, y, pixel) in decoded.enumerate_pixels() {
            let excluded = !(1..7).contains(&x)
                || !(1..5).contains(&y)
                || ((2..4).contains(&x) && (2..4).contains(&y));
            assert_eq!(
                *pixel,
                if excluded {
                    Rgba([255, 255, 255, 255])
                } else {
                    Rgba([2, 30, 70, 255])
                }
            );
        }
        assert_eq!(raster.pixels, pixels, "the original must remain unchanged");
        assert_eq!(
            image::load_from_memory(&raster.png().unwrap())
                .unwrap()
                .into_rgba8(),
            pixels
        );
    }
}

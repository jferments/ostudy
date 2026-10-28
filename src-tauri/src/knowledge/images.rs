//! Bounded managed-image evidence for semantic extraction and focused tutoring.
//! No external image relationship, arbitrary path, or URL is fetched here.

use std::{
    collections::VecDeque,
    fs::File,
    io::Read,
    path::{Component, Path},
    sync::{LazyLock, Mutex},
};

use image::{codecs::jpeg::JpegEncoder, imageops::FilterType, ImageFormat, Rgb, RgbImage};
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};

use crate::{
    ai::document_intelligence::{ImageShows, SourcePageImage},
    database::Database,
    error::CommandError,
    paths::AppPaths,
};

use super::contracts::{invalid, EvidenceImage, SourceRecord};

const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_TOTAL_IMAGE_BYTES: usize = 24 * 1024 * 1024;
/// Managed renders stay lossless on disk; a provider gets a legible JPEG copy
/// sized near what vision models read natively, which keeps requests small.
/// A 120 DPI page render (about 1000 x 1400) keeps its size, so small table
/// numbers stay readable, and goes out as roughly 200 KB.
const UPLOAD_LONG_EDGE: u32 = 1568;
const UPLOAD_JPEG_QUALITY: u8 = 80;
const REENCODE_ABOVE_BYTES: usize = 64 * 1024;
/// Upload copies remembered by stored-content hash: a page render goes out in
/// many requests (reading, cards, guide), and is re-encoded once.
const REMEMBERED_UPLOAD_COPIES: usize = 64;

/// (stored sha256, upload copy), oldest first.
type UploadCopies = VecDeque<(String, Vec<u8>)>;

static UPLOAD_COPIES: LazyLock<Mutex<UploadCopies>> =
    LazyLock::new(|| Mutex::new(VecDeque::with_capacity(REMEMBERED_UPLOAD_COPIES)));

struct ManagedImage {
    path: String,
    size: i64,
    sha256: Option<String>,
    media_type: String,
    source_unit: i64,
}

pub(crate) fn load_images(
    database: &Database,
    paths: &AppPaths,
    source: &SourceRecord,
) -> Result<Vec<SourcePageImage>, CommandError> {
    source.validate()?;
    load_image_references(database, paths, &source.images)
}

/// Loads exactly these stored images, in order, as their provider copies.
pub(crate) fn load_image_references(
    database: &Database,
    paths: &AppPaths,
    references: &[EvidenceImage],
) -> Result<Vec<SourcePageImage>, CommandError> {
    let mut images = Vec::new();
    let mut total_bytes = 0_usize;
    for reference in references {
        let record = database.with_connection(|connection| {
            let record = match reference {
                EvidenceImage::CardAsset { id } => connection
                    .query_row(
                        "SELECT relative_path, byte_size, mime_type FROM assets
                         WHERE id = ?1 AND deleted_at IS NULL AND lifecycle_state = 'ready'",
                        [id],
                        |row| {
                            Ok(ManagedImage {
                                path: row.get(0)?,
                                size: row.get(1)?,
                                sha256: None,
                                media_type: row.get(2)?,
                                source_unit: 1,
                            })
                        },
                    )
                    .optional()
                    .map_err(CommandError::database)?,
                EvidenceImage::DocumentAsset {
                    document_id,
                    asset_id,
                } => connection
                    .query_row(
                        "SELECT relative_path, byte_size, media_type, sha256,
                                COALESCE(source_unit_number, 1)
                         FROM document_source_assets WHERE document_id = ?1 AND asset_id = ?2",
                        params![document_id, asset_id],
                        |row| {
                            Ok(ManagedImage {
                                path: row.get(0)?,
                                size: row.get(1)?,
                                media_type: row.get(2)?,
                                sha256: Some(row.get(3)?),
                                source_unit: row.get(4)?,
                            })
                        },
                    )
                    .optional()
                    .map_err(CommandError::database)?,
            };
            record.ok_or_else(|| invalid("An image referenced by this material is unavailable."))
        })?;
        if record.size <= 0
            || usize::try_from(record.size).unwrap_or(usize::MAX) > MAX_IMAGE_BYTES
            || !record.media_type.starts_with("image/")
        {
            return Err(invalid(
                "An evidence image is unsupported or exceeds the 8 MiB image limit.",
            ));
        }
        let relative = Path::new(&record.path);
        if relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(invalid("An evidence image has an invalid managed path."));
        }
        let root = paths
            .app_data_dir()
            .canonicalize()
            .map_err(CommandError::io)?;
        let resolved = paths
            .resolve_relative(&record.path)
            .canonicalize()
            .map_err(CommandError::io)?;
        if !resolved.starts_with(&root) {
            return Err(invalid("An evidence image is outside the managed library."));
        }
        let file = File::open(resolved).map_err(CommandError::io)?;
        let mut bytes = Vec::with_capacity(usize::try_from(record.size).unwrap_or(0));
        file.take((MAX_IMAGE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(CommandError::io)?;
        if bytes.len() > MAX_IMAGE_BYTES || i64::try_from(bytes.len()).ok() != Some(record.size) {
            return Err(invalid(
                "An evidence image changed or exceeded its size limit.",
            ));
        }
        if let Some(expected) = record.sha256.as_deref() {
            if format!("{:x}", Sha256::digest(&bytes)) != expected {
                return Err(invalid(
                    "An evidence image failed its source-integrity check.",
                ));
            }
        }
        if !supported_image_bytes(&bytes) {
            return Err(invalid(
                "This evidence image is not a supported PNG, JPEG, GIF, or WebP file.",
            ));
        }
        let bytes = match record.sha256.as_deref() {
            Some(sha256) => upload_copy_of(sha256, bytes),
            None => upload_copy(bytes),
        };
        total_bytes = total_bytes.saturating_add(bytes.len());
        if total_bytes > MAX_TOTAL_IMAGE_BYTES {
            return Err(invalid(
                "The selected image evidence exceeds the 24 MiB request limit.",
            ));
        }
        images.push(SourcePageImage {
            page_number: record.source_unit,
            bytes,
            shows: ImageShows::Unit,
        });
    }
    Ok(images)
}

/// The provider copy of stored image `bytes`, already checked against their
/// `sha256`. Every request that sends a managed image goes through here, so a
/// page is sent the same way whichever step sends it.
pub(crate) fn upload_copy_of(sha256: &str, bytes: Vec<u8>) -> Vec<u8> {
    if let Ok(copies) = UPLOAD_COPIES.lock() {
        if let Some((_, copy)) = copies.iter().find(|(key, _)| key == sha256) {
            return copy.clone();
        }
    }
    let copy = upload_copy(bytes);
    if let Ok(mut copies) = UPLOAD_COPIES.lock() {
        if !copies.iter().any(|(key, _)| key == sha256) {
            if copies.len() >= REMEMBERED_UPLOAD_COPIES {
                copies.pop_front();
            }
            copies.push_back((sha256.to_string(), copy.clone()));
        }
    }
    copy
}

/// Re-encodes a large PNG as a JPEG no wider than `UPLOAD_LONG_EDGE`, flattening
/// transparency onto white. Anything else, or a copy that would not be smaller,
/// is sent as stored.
fn upload_copy(bytes: Vec<u8>) -> Vec<u8> {
    if bytes.len() <= REENCODE_ABOVE_BYTES
        || image::guess_format(&bytes).ok() != Some(ImageFormat::Png)
    {
        return bytes;
    }
    let Ok(decoded) = image::load_from_memory_with_format(&bytes, ImageFormat::Png) else {
        return bytes;
    };
    let decoded = if decoded.width().max(decoded.height()) > UPLOAD_LONG_EDGE {
        decoded.resize(UPLOAD_LONG_EDGE, UPLOAD_LONG_EDGE, FilterType::Triangle)
    } else {
        decoded
    };
    let rgba = decoded.to_rgba8();
    let flattened = RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        let over_white = |channel: u8| {
            ((u16::from(channel) * u16::from(a) + 255 * (255 - u16::from(a)) + 127) / 255) as u8
        };
        Rgb([over_white(r), over_white(g), over_white(b)])
    });
    let mut jpeg = Vec::new();
    if JpegEncoder::new_with_quality(&mut jpeg, UPLOAD_JPEG_QUALITY)
        .encode_image(&flattened)
        .is_err()
        || jpeg.len() >= bytes.len()
    {
        return bytes;
    }
    jpeg
}

fn supported_image_bytes(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a])
        || bytes.starts_with(&[0xff, 0xd8, 0xff])
        || bytes.starts_with(b"GIF87a")
        || bytes.starts_with(b"GIF89a")
        || (bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_evidence_is_content_checked_not_filename_checked() {
        assert!(supported_image_bytes(b"GIF89aexample"));
        assert!(!supported_image_bytes(
            b"<svg><script>not evidence</script></svg>"
        ));
        assert!(!supported_image_bytes(b"named-image.png"));
    }

    fn png(image: &image::RgbaImage) -> Vec<u8> {
        let mut bytes = Vec::new();
        image
            .write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn large_page_renders_are_sent_as_bounded_jpegs_on_white() {
        // Noise defeats PNG compression, like a scanned page does.
        let mut seed = 0x2545_f491_u32;
        let page = image::RgbaImage::from_fn(1700, 2000, |x, _| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let alpha = if x < 200 { 0 } else { 255 };
            image::Rgba([seed as u8, (seed >> 8) as u8, (seed >> 16) as u8, alpha])
        });
        let stored = png(&page);
        assert!(stored.len() > REENCODE_ABOVE_BYTES);

        let sent = upload_copy(stored.clone());
        assert!(sent.len() < stored.len() / 2);
        assert!(supported_image_bytes(&sent));
        let sent = image::load_from_memory_with_format(&sent, ImageFormat::Jpeg).unwrap();
        assert_eq!((sent.width(), sent.height()), (1333, 1568));
        let margin = sent.to_rgb8().get_pixel(10, 10).0;
        assert!(margin.iter().all(|channel| *channel > 240), "{margin:?}");
    }

    #[test]
    fn a_stored_image_is_reencoded_once_and_sent_the_same_way_everywhere() {
        let mut seed = 0x9e37_79b9_u32;
        let page = image::RgbaImage::from_fn(900, 1200, |_, _| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            image::Rgba([seed as u8, (seed >> 8) as u8, (seed >> 16) as u8, 255])
        });
        let stored = png(&page);
        let key = format!("{:x}", Sha256::digest(&stored));
        let first = upload_copy_of(&key, stored.clone());
        assert_eq!(first, upload_copy(stored.clone()));
        assert!(first.starts_with(&[0xff, 0xd8, 0xff]));
        // A remembered copy is returned without decoding the bytes passed again.
        assert_eq!(upload_copy_of(&key, b"not decoded again".to_vec()), first);
    }

    #[test]
    fn small_or_non_png_images_are_sent_as_stored() {
        let small = png(&image::RgbaImage::from_pixel(
            40,
            40,
            image::Rgba([0, 0, 0, 0]),
        ));
        assert_eq!(upload_copy(small.clone()), small);
        let gif = [b"GIF89a".as_slice(), &vec![0; REENCODE_ABOVE_BYTES]].concat();
        assert_eq!(upload_copy(gif.clone()), gif);
    }
}

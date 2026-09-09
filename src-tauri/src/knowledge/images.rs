//! Bounded managed-image evidence for semantic extraction and focused tutoring.
//! No external image relationship, arbitrary path, or URL is fetched here.

use std::{
    fs::File,
    io::Read,
    path::{Component, Path},
};

use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};

use crate::{
    ai::document_intelligence::SourcePageImage, database::Database, error::CommandError,
    paths::AppPaths,
};

use super::contracts::{invalid, EvidenceImage, SourceRecord};

const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_TOTAL_IMAGE_BYTES: usize = 24 * 1024 * 1024;

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
    let mut images = Vec::new();
    let mut total_bytes = 0_usize;
    for reference in &source.images {
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
        if let Some(expected) = record.sha256 {
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
        total_bytes = total_bytes.saturating_add(bytes.len());
        if total_bytes > MAX_TOTAL_IMAGE_BYTES {
            return Err(invalid(
                "The selected image evidence exceeds the 24 MiB request limit.",
            ));
        }
        images.push(SourcePageImage {
            page_number: record.source_unit,
            bytes,
        });
    }
    Ok(images)
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
}

//! Verified reads of a document's managed source images, for exports and
//! viewers. Page renders reach a provider only through knowledge extraction.

use std::fs;

use sha2::{Digest, Sha256};

use crate::{document_store::SourceAssetRecord, error::CommandError, paths::AppPaths};

const MAX_VISUAL_EVIDENCE_BYTES: usize = 32 * 1024 * 1024;

pub(crate) fn read_source_image_bytes(
    paths: &AppPaths,
    asset: &SourceAssetRecord,
) -> Result<Vec<u8>, CommandError> {
    if !asset.media_type.starts_with("image/") {
        return Err(CommandError::new(
            "document_source_asset_not_image",
            "A selected canonical source asset is not an image.",
        ));
    }
    let bytes = fs::read(paths.resolve_relative(&asset.relative_path)).map_err(CommandError::io)?;
    if bytes.is_empty()
        || bytes.len() > MAX_VISUAL_EVIDENCE_BYTES
        || i64::try_from(bytes.len()).unwrap_or(i64::MAX) != asset.byte_size
        || sha256_hex(&bytes) != asset.sha256
    {
        return Err(CommandError::new(
            "document_source_asset_invalid",
            "A canonical source asset failed its size or integrity check. Retry document processing to rebuild it.",
        ));
    }
    Ok(bytes)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

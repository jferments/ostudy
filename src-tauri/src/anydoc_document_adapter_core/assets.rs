use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
};

use anydoc::model::Document;

use crate::{
    document_canonical::{CanonicalAsset, CanonicalAssetRole},
    error::CommandError,
    paths::AppPaths,
};

use super::support::{bounded_text, sha256_hex};

const MAX_ANYDOC_ASSET_BYTES: usize = 32 * 1024 * 1024;
const MAX_ANYDOC_RETAINED_ASSET_BYTES: usize = 128 * 1024 * 1024;

pub(crate) fn persist_image_assets(
    paths: &AppPaths,
    document_id: &str,
    document: &Document,
) -> Result<(Vec<CanonicalAsset>, HashMap<usize, String>), CommandError> {
    let directory = paths.document_assets_dir(document_id);
    fs::create_dir_all(&directory).map_err(CommandError::io)?;
    let mut total_bytes = 0_usize;
    let mut assets = Vec::new();
    let mut identity_by_source_index = HashMap::new();
    for (source_index, asset) in document.assets.iter().enumerate() {
        if !asset.media_type.starts_with("image/") || asset.bytes.is_empty() {
            continue;
        }
        if asset.bytes.len() > MAX_ANYDOC_ASSET_BYTES {
            return Err(CommandError::new(
                "document_asset_too_large",
                "An embedded document image exceeds oStudy's 32 MB asset limit.",
            ));
        }
        total_bytes = total_bytes.saturating_add(asset.bytes.len());
        if total_bytes > MAX_ANYDOC_RETAINED_ASSET_BYTES {
            return Err(CommandError::new(
                "document_assets_too_large",
                "The document's retained embedded images exceed oStudy's 128 MB limit.",
            ));
        }
        let ordinal = assets.len() + 1;
        let asset_id =
            register_asset_identity(&mut identity_by_source_index, source_index, ordinal);
        let extension = image_extension(&asset.media_type);
        let sha256 = sha256_hex(&asset.bytes);
        let relative_path = AppPaths::document_versioned_asset_relative_path(
            document_id,
            ordinal,
            asset_content_fingerprint(&sha256),
            extension,
        );
        let destination = paths.resolve_relative(&relative_path);
        write_immutable_file(&destination, &asset.bytes)?;
        assets.push(CanonicalAsset {
            asset_id,
            source_unit_number: None,
            block_index: None,
            role: CanonicalAssetRole::EmbeddedImage,
            media_type: bounded_text(&asset.media_type, 160),
            relative_path,
            byte_size: i64::try_from(asset.bytes.len()).unwrap_or(i64::MAX),
            sha256,
            width: None,
            height: None,
            alt_text: None,
        });
    }
    Ok((assets, identity_by_source_index))
}

fn register_asset_identity(
    identity_by_source_index: &mut HashMap<usize, String>,
    source_index: usize,
    ordinal: usize,
) -> String {
    let asset_id = format!("anydoc-asset:{ordinal:06}");
    identity_by_source_index.insert(source_index, asset_id.clone());
    asset_id
}

fn image_extension(media_type: &str) -> &'static str {
    match media_type.to_ascii_lowercase().as_str() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        "image/bmp" => "bmp",
        "image/tiff" => "tif",
        _ => "bin",
    }
}

fn asset_content_fingerprint(sha256: &str) -> u64 {
    sha256
        .get(..16)
        .and_then(|value| u64::from_str_radix(value, 16).ok())
        .unwrap_or_default()
}

fn write_immutable_file(destination: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    let parent = destination.parent().ok_or_else(|| {
        CommandError::new(
            "invalid_document_asset_path",
            "The document asset path is invalid.",
        )
    })?;
    fs::create_dir_all(parent).map_err(CommandError::io)?;
    if destination.exists() {
        let existing = fs::read(destination).map_err(CommandError::io)?;
        if existing == bytes {
            return Ok(());
        }
        return Err(CommandError::new(
            "document_asset_identity_collision",
            "An embedded document image conflicted with an existing immutable asset path.",
        ));
    }
    let temporary = destination.with_extension(format!("tmp-{}", std::process::id()));
    let _ = fs::remove_file(&temporary);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(CommandError::io)?;
    file.write_all(bytes).map_err(CommandError::io)?;
    file.sync_all().map_err(CommandError::io)?;
    fs::rename(&temporary, destination).map_err(CommandError::io)?;
    File::open(destination)
        .and_then(|file| file.sync_all())
        .map_err(CommandError::io)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_identity_preserves_source_indices() {
        let mut identity = HashMap::new();
        assert_eq!(
            register_asset_identity(&mut identity, 1, 1),
            "anydoc-asset:000001"
        );
        assert_eq!(
            register_asset_identity(&mut identity, 3, 2),
            "anydoc-asset:000002"
        );
        assert_eq!(
            identity.get(&1).map(String::as_str),
            Some("anydoc-asset:000001")
        );
        assert!(!identity.contains_key(&0));
    }

    #[test]
    fn content_fingerprints_change_with_hashes() {
        let first = sha256_hex(b"first");
        let second = sha256_hex(b"second");
        assert_ne!(
            asset_content_fingerprint(&first),
            asset_content_fingerprint(&second)
        );
    }
}

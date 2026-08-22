//! Canonical visual-evidence loading for document learning artifacts.
//!
//! Topic planners own stable asset identities through `SourceLocator`; this
//! adapter verifies the corresponding managed bytes before provider transport.
//! It is format-neutral and never parses source documents or chooses AI topics.

use std::fs;

use sha2::{Digest, Sha256};

use crate::{
    ai::document_intelligence::SourcePageImage,
    document_domain::SourceLocator,
    document_store::{self, SourceAssetRecord},
    error::CommandError,
    paths::AppPaths,
};

const MAX_VISUAL_EVIDENCE_BYTES: usize = 32 * 1024 * 1024;

pub(crate) fn load_locator_images(
    paths: &AppPaths,
    assets: &[SourceAssetRecord],
    locator: &SourceLocator,
    maximum: usize,
) -> Result<Vec<SourcePageImage>, CommandError> {
    if locator.asset_ids.is_empty() || maximum == 0 {
        return Ok(Vec::new());
    }
    let selected = document_store::select_source_assets(&locator.asset_ids, maximum);
    let mut images = Vec::with_capacity(selected.len());
    for asset_id in selected {
        let asset = assets
            .iter()
            .find(|asset| asset.asset_id == asset_id)
            .ok_or_else(|| {
                CommandError::new(
                    "document_source_asset_missing",
                    "A selected canonical source asset is missing.",
                )
            })?;
        if !asset.media_type.starts_with("image/") {
            return Err(CommandError::new(
                "document_source_asset_not_image",
                "A selected multimodal source asset is not an image.",
            ));
        }
        let bytes =
            fs::read(paths.resolve_relative(&asset.relative_path)).map_err(CommandError::io)?;
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
        images.push(SourcePageImage {
            page_number: asset.source_unit_number.unwrap_or_else(|| locator.first()),
            bytes,
        });
    }
    Ok(images)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_domain::{SourceLocator, SourceUnitKind};

    #[test]
    fn locator_without_assets_needs_no_filesystem_evidence() {
        let locator = SourceLocator::new(SourceUnitKind::Page, vec![1], None).unwrap();
        let root = std::env::temp_dir().join(format!(
            "ostudy-empty-visual-evidence-{}",
            std::process::id()
        ));
        let paths = AppPaths::new(root.clone()).unwrap();
        assert!(load_locator_images(&paths, &[], &locator, 4)
            .unwrap()
            .is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}

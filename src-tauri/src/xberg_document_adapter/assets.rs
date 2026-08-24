use std::collections::HashMap;

use xberg::{ExtractedDocument, ImageKind};

use crate::{
    document_canonical::{CanonicalAsset, CanonicalAssetRole},
    document_ingestion::canonical::{ManagedAssetInput, ManagedAssetStore},
    error::CommandError,
    paths::AppPaths,
};

use super::support::image_media_type;

pub(crate) struct PersistedImages {
    pub(crate) assets: Vec<CanonicalAsset>,
    pub(crate) asset_id_by_image_index: HashMap<u32, String>,
    pub(crate) skipped_image_count: usize,
}

pub(crate) fn persist_images(
    paths: &AppPaths,
    document_id: &str,
    document: &ExtractedDocument,
) -> Result<PersistedImages, CommandError> {
    let Some(images) = document.images.as_ref() else {
        return Ok(PersistedImages {
            assets: Vec::new(),
            asset_id_by_image_index: HashMap::new(),
            skipped_image_count: 0,
        });
    };

    let mut store = ManagedAssetStore::new(paths, document_id)?;
    let mut assets = Vec::new();
    let mut asset_id_by_image_index = HashMap::new();
    let mut skipped_image_count = 0_usize;
    for (position, image) in images.iter().enumerate() {
        let Some(media_type) = image_media_type(image.format.as_ref()) else {
            skipped_image_count = skipped_image_count.saturating_add(1);
            continue;
        };
        if image.data.is_empty() {
            skipped_image_count = skipped_image_count.saturating_add(1);
            continue;
        }
        let role = asset_role(image.image_kind, image.source_path.as_deref());
        let alt_text = image.caption.as_deref().or(image.description.as_deref());
        let asset = store.persist(ManagedAssetInput {
            bytes: image.data.as_ref(),
            media_type,
            role,
            width: image.width,
            height: image.height,
            alt_text,
            source_unit_number: None,
        })?;
        let position = u32::try_from(position).unwrap_or(u32::MAX);
        asset_id_by_image_index.insert(position, asset.asset_id.clone());
        asset_id_by_image_index
            .entry(image.image_index)
            .or_insert_with(|| asset.asset_id.clone());
        assets.push(asset);
    }

    Ok(PersistedImages {
        assets,
        asset_id_by_image_index,
        skipped_image_count,
    })
}

fn asset_role(kind: Option<ImageKind>, source_path: Option<&str>) -> CanonicalAssetRole {
    if source_path.is_some_and(|path| path.to_ascii_lowercase().contains("cover")) {
        return CanonicalAssetRole::CoverImage;
    }
    match kind {
        Some(ImageKind::Diagram | ImageKind::Drawing) => CanonicalAssetRole::Diagram,
        Some(ImageKind::Chart) => CanonicalAssetRole::Chart,
        Some(ImageKind::PageRaster) => CanonicalAssetRole::Thumbnail,
        _ => CanonicalAssetRole::EmbeddedImage,
    }
}

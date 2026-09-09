use rusqlite::{params, Connection, OptionalExtension};

use crate::{document_domain::SourceLocator, error::CommandError};

use super::{
    super::contracts::{invalid, ContentOrigin, EvidenceImage, SourceRecord},
    blank_record, segment_record, SourceSeed,
};

pub(super) fn read(
    connection: &Connection,
    seed: &SourceSeed,
) -> Result<Vec<SourceRecord>, CommandError> {
    let chunk_index = seed
        .parent_key
        .parse::<i64>()
        .map_err(|_| invalid("The canonical source chunk identity is invalid."))?;
    if chunk_index < 0 {
        return Err(invalid("The canonical source chunk identity is invalid."));
    }
    let row = connection
        .query_row(
            "SELECT d.title, c.full_text, c.source_locator_json, c.visual_asset_ids_json,
                    c.content_sha256, c.heading_context
             FROM document_semantic_chunk_sources c JOIN documents d ON d.id = c.document_id
             WHERE c.document_id = ?1 AND c.chunk_index = ?2",
            params![seed.owner_id, chunk_index],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((title, text, locator, assets, revision, heading)) = row else {
        return Ok(Vec::new());
    };
    let title = heading
        .filter(|value| !value.trim().is_empty())
        .map_or_else(|| title.clone(), |heading| format!("{title} — {heading}"));
    let mut source = blank_record(seed, &title, &revision);
    source.text = text;
    source.origin = ContentOrigin::SourceDocument;
    source.document_id = Some(seed.owner_id.clone());
    source.locator =
        Some(serde_json::from_str::<SourceLocator>(&locator).map_err(CommandError::serialization)?);
    let mut asset_ids: Vec<String> =
        serde_json::from_str(&assets).map_err(CommandError::serialization)?;
    asset_ids.sort();
    asset_ids.dedup();
    for asset_id in asset_ids {
        source.images.push(EvidenceImage::DocumentAsset {
            document_id: seed.owner_id.clone(),
            asset_id,
        });
    }
    Ok(segment_record(source))
}

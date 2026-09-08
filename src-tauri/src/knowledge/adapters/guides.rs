use rusqlite::{params, Connection, OptionalExtension};

use crate::{document_domain::SourceLocator, error::CommandError};

use super::{
    super::contracts::{hash, ContentOrigin, EvidenceImage, SourceRecord},
    blank_record, segment_record, SourceSeed,
};

struct GuideRow {
    document_id: String,
    revision: i64,
    title: String,
    text: String,
    content_hash: String,
    locator: Option<String>,
}

struct OverlayRow {
    id: String,
    kind: String,
    text: String,
    rich_doc: String,
    revision: i64,
}

pub(super) fn read(
    connection: &Connection,
    seed: &SourceSeed,
) -> Result<Vec<SourceRecord>, CommandError> {
    let row = if seed.parent_key == "overview" {
        connection
            .query_row(
                "SELECT g.document_id, g.revision, d.title, g.overview_text
                 FROM document_study_guides g JOIN documents d ON d.id = g.document_id
                 WHERE g.id = ?1 AND g.state = 'active'",
                [&seed.owner_id],
                |row| {
                    let text: String = row.get(3)?;
                    Ok(GuideRow {
                        document_id: row.get(0)?,
                        revision: row.get(1)?,
                        title: row.get(2)?,
                        content_hash: hash(&text),
                        text,
                        locator: None,
                    })
                },
            )
            .optional()
            .map_err(CommandError::database)?
    } else {
        connection
            .query_row(
                "SELECT g.document_id, g.revision, p.title, s.content_text,
                        s.content_hash, s.source_locator_json
                 FROM document_study_guides g
                 JOIN document_study_guide_sections s ON s.guide_id = g.id
                 JOIN document_learning_plan_topics p ON p.plan_id = g.plan_id AND p.topic_id = s.topic_id
                 WHERE g.id = ?1 AND s.topic_id = ?2 AND g.state = 'active'",
                params![seed.owner_id, seed.parent_key],
                |row| {
                    Ok(GuideRow {
                        document_id: row.get(0)?,
                        revision: row.get(1)?,
                        title: row.get(2)?,
                        text: row.get(3)?,
                        content_hash: row.get(4)?,
                        locator: Some(row.get(5)?),
                    })
                },
            )
            .optional()
            .map_err(CommandError::database)?
    };
    let Some(row) = row else {
        return Ok(Vec::new());
    };
    let topic_id = (seed.parent_key != "overview").then_some(seed.parent_key.as_str());
    let mut statement = connection
        .prepare(
            "SELECT id, overlay_kind, plain_text, rich_doc_json, revision
             FROM document_study_guide_overlays
             WHERE document_id = ?1 AND topic_id IS ?2 AND state = 'active'
               AND (guide_id IS NULL OR guide_id = ?3)
               AND (overlay_kind IN ('guide_note', 'topic_note') OR base_content_hash = ?4)
             ORDER BY updated_at, id",
        )
        .map_err(CommandError::database)?;
    let overlays = statement
        .query_map(params![row.document_id, topic_id, seed.owner_id, row.content_hash], |row| {
            Ok(OverlayRow {
                id: row.get(0)?,
                kind: row.get(1)?,
                text: row.get(2)?,
                rich_doc: row.get(3)?,
                revision: row.get(4)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let mut title = row.title;
    let mut text = row.text;
    let mut modified = false;
    for overlay in &overlays {
        if overlay.text.trim().is_empty() {
            continue;
        }
        match overlay.kind.as_str() {
            "title_override" => {
                title.clone_from(&overlay.text);
                modified = true;
            }
            "body_override" => {
                text.clone_from(&overlay.text);
                modified = true;
            }
            _ => {}
        }
    }
    let overlay_revisions = overlays
        .iter()
        .map(|overlay| format!("{}:{}", overlay.id, overlay.revision))
        .collect::<Vec<_>>()
        .join("|");
    let revision = hash(&format!("{}|{}|{overlay_revisions}", row.revision, row.content_hash));
    let mut source = blank_record(seed, &title, &revision);
    source.text = text;
    source.document_id = Some(row.document_id.clone());
    source.guide_id = Some(seed.owner_id.clone());
    source.origin = if modified {
        ContentOrigin::UserAuthored
    } else {
        ContentOrigin::Generated
    };
    if !modified {
        source.locator = row
            .locator
            .map(|value| serde_json::from_str::<SourceLocator>(&value).map_err(CommandError::serialization))
            .transpose()?;
    }
    let mut result = segment_record(source);
    // Notes are separate semantic records so an annotation cannot masquerade
    // as a statement from the immutable generated guide or original textbook.
    for overlay in overlays {
        if !matches!(overlay.kind.as_str(), "guide_note" | "topic_note" | "highlight")
            || overlay.text.trim().is_empty()
        {
            continue;
        }
        let mut annotation = blank_record(seed, &format!("Learner note: {title}"), &revision);
        annotation.segment_key = format!("{}:annotation:{}", seed.parent_key, overlay.id);
        annotation.text = overlay.text;
        annotation.document_id = Some(row.document_id.clone());
        annotation.guide_id = Some(seed.owner_id.clone());
        annotation.origin = ContentOrigin::UserAuthored;
        let rich_doc = serde_json::from_str(&overlay.rich_doc).map_err(CommandError::serialization)?;
        annotation.images = crate::cards::collect_asset_ids(&rich_doc)?
            .into_iter()
            .map(|id| EvidenceImage::CardAsset { id })
            .collect();
        result.extend(segment_record(annotation));
    }
    Ok(result)
}

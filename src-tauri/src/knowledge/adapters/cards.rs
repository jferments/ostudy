use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    authoring::{render_projected_presentation, ProjectedPresentationInput},
    error::CommandError,
};

use super::{
    super::contracts::{ContentOrigin, EvidenceImage, SourceRecord},
    blank_record, segment_record, SourceSeed,
};

struct CardSourceRow {
    note_id: String,
    deck_name: String,
    direction: String,
    ordinal: i64,
    note_type: String,
    front_doc: String,
    back_doc: String,
    front_text: String,
    back_text: String,
    cloze_source: String,
    cloze_mode: String,
    revision: i64,
    generated: bool,
}

pub(super) fn read(
    connection: &Connection,
    seed: &SourceSeed,
) -> Result<Vec<SourceRecord>, CommandError> {
    let row = connection
        .query_row(
            "SELECT n.id, d.name, c.direction, c.template_ordinal, n.note_type,
                    n.front_doc, n.back_doc, n.front_text, n.back_text,
                    n.cloze_source, n.cloze_mode, n.revision,
                    EXISTS(SELECT 1 FROM document_note_topics nt WHERE nt.note_id = n.id)
             FROM cards c JOIN notes n ON n.id = c.note_id JOIN decks d ON d.id = n.deck_id
             WHERE c.id = ?1 AND c.deleted_at IS NULL AND n.deleted_at IS NULL AND d.deleted_at IS NULL",
            [&seed.owner_id],
            |row| {
                Ok(CardSourceRow {
                    note_id: row.get(0)?,
                    deck_name: row.get(1)?,
                    direction: row.get(2)?,
                    ordinal: row.get(3)?,
                    note_type: row.get(4)?,
                    front_doc: row.get(5)?,
                    back_doc: row.get(6)?,
                    front_text: row.get(7)?,
                    back_text: row.get(8)?,
                    cloze_source: row.get(9)?,
                    cloze_mode: row.get(10)?,
                    revision: row.get(11)?,
                    generated: row.get(12)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some(row) = row else {
        return Ok(Vec::new());
    };
    // Index a stable content presentation, never a schedule-derived rotating
    // variant. The tutor's focused session context supplies the exact currently
    // displayed cloze/reverse question separately from this retrieval record.
    let rendered = render_projected_presentation(ProjectedPresentationInput {
        direction: &row.direction,
        template_ordinal: row.ordinal,
        note_type: &row.note_type,
        front_doc_json: &row.front_doc,
        back_doc_json: &row.back_doc,
        front_text: &row.front_text,
        back_text: &row.back_text,
        cloze_source: &row.cloze_source,
        cloze_mode: &row.cloze_mode,
        reps: 0,
        presentation_variant: None,
    })?;
    let title = format!(
        "{} — {}: {}",
        row.deck_name,
        rendered.card_label,
        rendered.front_text.chars().take(160).collect::<String>(),
    );
    let mut source = blank_record(seed, &title, &row.revision.to_string());
    source.text = format!(
        "Question ({} presentation):\n{}\n\nAnswer:\n{}",
        row.direction, rendered.front_text, rendered.back_text,
    );
    source.note_id = Some(row.note_id);
    source.card_id = Some(seed.owner_id.clone());
    // Existing source-topic provenance does not prove that later learner edits
    // are still supported by the original document.
    source.origin = if row.generated && row.revision == 1 {
        ContentOrigin::Generated
    } else {
        ContentOrigin::UserAuthored
    };
    let mut asset_ids = crate::cards::collect_asset_ids(&rendered.front_doc)?;
    asset_ids.extend(crate::cards::collect_asset_ids(&rendered.back_doc)?);
    asset_ids.sort();
    asset_ids.dedup();
    if !asset_ids.is_empty() {
        let serialized = serde_json::to_string(&asset_ids).map_err(CommandError::serialization)?;
        let mut statement = connection
            .prepare(
                "SELECT id FROM assets WHERE id IN (SELECT value FROM json_each(?1))
                 AND mime_type LIKE 'image/%' AND deleted_at IS NULL
                 ORDER BY id",
            )
            .map_err(CommandError::database)?;
        source.images = statement
            .query_map(params![serialized], |row| {
                Ok(EvidenceImage::CardAsset { id: row.get(0)? })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
    }
    Ok(segment_record(source))
}

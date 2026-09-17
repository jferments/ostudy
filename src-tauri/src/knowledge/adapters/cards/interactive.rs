//! Typed card-to-knowledge bridge. Never route a question-json note through
//! legacy nullable front/back documents or flatten its key into unlabeled facts.

use rusqlite::{params, Connection};

use super::{blank_record, segment_record, CardSourceRow, ContentOrigin, SourceRecord, SourceSeed};
use crate::{
    cards::questions::{QuestionKind, ValidatedQuestion},
    error::CommandError,
    knowledge::contracts::EvidenceImage,
};

pub(super) fn read(
    connection: &Connection,
    seed: &SourceSeed,
    row: &CardSourceRow,
) -> Result<Vec<SourceRecord>, CommandError> {
    let (definition, key): (String, String) = connection
        .query_row(
            "SELECT n.question_definition, p.question_key
             FROM card_question_presentations p JOIN notes n ON n.id = p.note_id
             WHERE p.card_id = ?1 AND p.note_id = ?2",
            params![seed.owner_id, row.note_id],
            |record| Ok((record.get(0)?, record.get(1)?)),
        )
        .map_err(CommandError::database)?;
    let question = ValidatedQuestion::parse(&definition)?;
    let projection = question.knowledge_text(&key)?;
    let title = format!(
        "{} — {}: {}",
        row.deck_name,
        question.kind().label(),
        projection.prompt.chars().take(160).collect::<String>(),
    );
    let mut source = blank_record(seed, &title, &row.revision.to_string());
    source.text =
        format!(
        "Authored {} question:\n{}\n\nAuthored answer relation (not independently verified):\n{}",
        question.kind().label(), projection.prompt, projection.answer,
    );
    source.note_id = Some(row.note_id.clone());
    source.card_id = Some(seed.owner_id.clone());
    source.origin = if row.generated && row.revision == 1 {
        ContentOrigin::Generated
    } else {
        ContentOrigin::UserAuthored
    };
    // An occlusion's source image contains answers to other presentations. It
    // never enters the semantic image route, even after this target is revealed.
    if question.kind() != QuestionKind::ImageOcclusion {
        let ids =
            serde_json::to_string(&question.asset_ids()).map_err(CommandError::serialization)?;
        let mut statement = connection
            .prepare(
                "SELECT id FROM assets WHERE id IN (SELECT value FROM json_each(?1))
                 AND mime_type LIKE 'image/%' AND deleted_at IS NULL ORDER BY id",
            )
            .map_err(CommandError::database)?;
        source.images = statement
            .query_map([ids], |record| {
                Ok(EvidenceImage::CardAsset { id: record.get(0)? })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
    }
    Ok(segment_record(source))
}

#[cfg(test)]
mod tests {
    use crate::{
        authoring::card_types_and_templates::generated_import::import_generated_flashcards_bytes_with_commit_context,
        database::Database,
        knowledge::{adapters, contracts::SourceKind},
    };
    use serde_json::json;

    use super::*;

    #[test]
    fn typed_source_publication_keeps_the_answer_relation_and_nullable_recall_columns() {
        let database = Database::open_in_memory().unwrap();
        let payload = json!({"format":"ostudy-flashcards","formatVersion":2,
        "deck":{"mode":"create","name":"Typed material"},"cards":[
            {"type":"interactive","question":{"kind":"true_false",
                "statement":"A triangle has four sides.","correct":false,
                "correctedStatement":"A triangle has three sides."}},
            {"type":"interactive","question":{"kind":"matching","prompt":"Match side counts.",
                "pairs":[{"id":"a","left":"Triangle","right":"Three"},
                    {"id":"b","left":"Square","right":"Four"}]}}
        ]})
        .to_string();
        import_generated_flashcards_bytes_with_commit_context(
            &database,
            payload.as_bytes(),
            |_, _, _, _| Ok(()),
        )
        .unwrap();
        let ids = database
            .with_connection(|connection| {
                let mut statement = connection
                    .prepare("SELECT id FROM cards ORDER BY id")
                    .map_err(CommandError::database)?;
                let result = statement
                    .query_map([], |row| row.get::<_, String>(0))
                    .map_err(CommandError::database)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(CommandError::database)?;
                Ok(result)
            })
            .unwrap();
        let mut text = String::new();
        for id in ids {
            let sources = adapters::materialize_source(
                &database,
                &SourceSeed {
                    kind: SourceKind::Card,
                    owner_id: id,
                    parent_key: "presentation".to_string(),
                },
            )
            .unwrap();
            assert!(!sources.is_empty());
            for source in sources {
                source.validate().unwrap();
                assert!(source.images.is_empty());
                text.push_str(&source.text);
            }
        }
        assert!(text.contains("Authored verdict: False"));
        assert!(text.contains("Corrected statement: A triangle has three sides."));
        assert!(text.contains("Match: Triangle => Three"));
        assert!(text.contains("Match: Square => Four"));
    }
}

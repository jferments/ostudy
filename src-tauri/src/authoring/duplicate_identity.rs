//! Private duplicate identity for rich card content.
//!
//! `notes.duplicate_hash` remains the historical normalized-text FNV candidate
//! index. This module performs the authoritative second-stage comparison for
//! rich cards so managed assets participate without leaking filenames, paths,
//! runtime URLs, or other presentation metadata into searchable/public text.

use rusqlite::{params, Connection};
use serde_json::Value;

use crate::{cards::ValidatedContent, error::CommandError, util::validate_entity_id};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(in super::super) struct DuplicateIdentity {
    normalized_front: String,
    normalized_back: String,
    front_managed: Vec<ManagedNodeIdentity>,
    back_managed: Vec<ManagedNodeIdentity>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct ManagedNodeIdentity {
    kind: ManagedNodeKind,
    asset_id: String,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum ManagedNodeKind {
    Image,
    Audio,
    Video,
}

impl ManagedNodeKind {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "image" => Some(Self::Image),
            "audio" => Some(Self::Audio),
            "video" => Some(Self::Video),
            _ => None,
        }
    }
}

pub(in super::super) fn from_validated_content(
    content: &ValidatedContent,
) -> Result<DuplicateIdentity, CommandError> {
    from_serialized_documents(
        &content.front_doc_json,
        &content.back_doc_json,
        &content.front_text,
        &content.back_text,
    )
}

pub(in super::super) fn from_serialized_documents(
    front_doc_json: &str,
    back_doc_json: &str,
    front_text: &str,
    back_text: &str,
) -> Result<DuplicateIdentity, CommandError> {
    let front: Value = serde_json::from_str(front_doc_json).map_err(CommandError::serialization)?;
    let back: Value = serde_json::from_str(back_doc_json).map_err(CommandError::serialization)?;
    Ok(DuplicateIdentity {
        normalized_front: normalize_text(front_text),
        normalized_back: normalize_text(back_text),
        front_managed: managed_nodes(&front)?,
        back_managed: managed_nodes(&back)?,
    })
}

pub(in super::super) fn has_exact_duplicate(
    connection: &Connection,
    deck_id: &str,
    candidate_hash: &str,
    identity: &DuplicateIdentity,
    exclude_note_id: Option<&str>,
) -> Result<bool, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT front_doc, back_doc, front_text, back_text
             FROM notes
             WHERE deck_id = ?1
               AND (duplicate_hash = ?2 OR duplicate_hash = '')
               AND deleted_at IS NULL
               AND (?3 IS NULL OR id != ?3)
             ORDER BY updated_at DESC",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map(params![deck_id, candidate_hash, exclude_note_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(CommandError::database)?;

    for row in rows {
        let (front_doc, back_doc, front_text, back_text) = row.map_err(CommandError::database)?;
        let stored = from_serialized_documents(&front_doc, &back_doc, &front_text, &back_text)?;
        if &stored == identity {
            return Ok(true);
        }
    }
    Ok(false)
}

fn managed_nodes(document: &Value) -> Result<Vec<ManagedNodeIdentity>, CommandError> {
    let mut output = Vec::new();
    visit(document, &mut output)?;
    Ok(output)
}

fn visit(value: &Value, output: &mut Vec<ManagedNodeIdentity>) -> Result<(), CommandError> {
    match value {
        Value::Array(values) => {
            for value in values {
                visit(value, output)?;
            }
        }
        Value::Object(object) => {
            if let Some(kind) = object
                .get("type")
                .and_then(Value::as_str)
                .and_then(ManagedNodeKind::parse)
            {
                let asset_id = object
                    .get("attrs")
                    .and_then(Value::as_object)
                    .and_then(|attrs| attrs.get("assetId"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        CommandError::new(
                            "invalid_card_document",
                            "A managed card asset is missing its identity.",
                        )
                    })?;
                validate_entity_id(asset_id, "asset")?;
                output.push(ManagedNodeIdentity {
                    kind,
                    asset_id: asset_id.to_string(),
                });
            }
            if let Some(content) = object.get("content") {
                visit(content, output)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn normalize_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{
        database::Database,
        decks::{create_deck_in_connection, CreateDeckInput},
    };

    fn doc(content: Value) -> String {
        serde_json::to_string(&json!({
            "type": "doc",
            "attrs": {"schemaVersion": 3},
            "content": content,
        }))
        .unwrap()
    }

    fn media(kind: &str, asset_id: &str, title: &str) -> Value {
        json!({
            "type": kind,
            "attrs": {
                "assetId": asset_id,
                "src": format!("study-asset://{asset_id}"),
                "title": title,
            }
        })
    }

    fn answer_doc() -> String {
        doc(json!([{
            "type": "paragraph",
            "content": [{"type": "text", "text": "Answer"}]
        }]))
    }

    #[test]
    fn managed_identity_distinguishes_asset_kind_side_and_order_without_metadata() {
        let a = "a".repeat(32);
        let b = "b".repeat(32);
        let empty = answer_doc();
        let first = from_serialized_documents(
            &doc(json!([
                media("audio", &a, "private-one"),
                media("video", &b, "private-two")
            ])),
            &empty,
            "  [Audio]   [Video]  ",
            "Answer",
        )
        .unwrap();
        let metadata_only = from_serialized_documents(
            &doc(json!([
                media("audio", &a, "different title"),
                media("video", &b, "also different")
            ])),
            &empty,
            "[audio] [video]",
            " answer ",
        )
        .unwrap();
        assert_eq!(first, metadata_only);

        let different_asset = from_serialized_documents(
            &doc(json!([media("audio", &b, "x"), media("video", &a, "y")])),
            &empty,
            "[Audio] [Video]",
            "Answer",
        )
        .unwrap();
        assert_ne!(first, different_asset);

        let different_order = from_serialized_documents(
            &doc(json!([media("video", &b, "x"), media("audio", &a, "y")])),
            &empty,
            "[Audio] [Video]",
            "Answer",
        )
        .unwrap();
        assert_ne!(first, different_order);

        let different_side = from_serialized_documents(
            &empty,
            &doc(json!([media("audio", &a, "x"), media("video", &b, "y")])),
            "Answer",
            "[Audio] [Video]",
        )
        .unwrap();
        assert_ne!(first, different_side);
    }

    #[test]
    fn database_duplicate_query_uses_managed_identity_after_text_hash_lookup() {
        let database = Database::open_in_memory().expect("database should open");
        let deck_id = database
            .with_connection(|connection| {
                create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: "Media duplicate identity".to_string(),
                        description: None,
                    },
                )
            })
            .expect("deck should be created")
            .id;
        let note_id = "1".repeat(32);
        let first_asset = "a".repeat(32);
        let second_asset = "b".repeat(32);
        let stored_front = doc(json!([media("audio", &first_asset, "private filename one")]));
        let same_front = doc(json!([media("audio", &first_asset, "different private title")]));
        let different_front = doc(json!([media("audio", &second_asset, "private filename two")]));
        let back = answer_doc();
        let normalized = crate::authoring::normalized_pair("[Audio]", "Answer");
        let hash = crate::authoring::duplicate_hash(&normalized.0, &normalized.1);

        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO notes (
                             id, deck_id, note_type, content_format, content_schema_version,
                             front_doc, back_doc, front_text, back_text, duplicate_hash,
                             reverse_enabled, cloze_source, cloze_mode,
                             created_at, updated_at, revision
                         ) VALUES (
                             ?1, ?2, 'basic', 'tiptap-json', 1,
                             ?3, ?4, '[Audio]', 'Answer', ?5,
                             0, '', 'one_at_a_time', 1, 1, 1
                         )",
                        params![note_id, deck_id, stored_front, back, hash],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .expect("stored media note should insert");

        let same = from_serialized_documents(&same_front, &back, "[Audio]", "Answer")
            .expect("same identity should parse");
        let different = from_serialized_documents(&different_front, &back, "[Audio]", "Answer")
            .expect("different identity should parse");
        database
            .with_connection(|connection| {
                assert!(has_exact_duplicate(
                    connection, &deck_id, &hash, &same, None
                )?);
                assert!(!has_exact_duplicate(
                    connection,
                    &deck_id,
                    &hash,
                    &different,
                    None
                )?);
                assert!(!has_exact_duplicate(
                    connection,
                    &deck_id,
                    &hash,
                    &same,
                    Some(&note_id)
                )?);
                Ok(())
            })
            .expect("database duplicate identity query should succeed");
    }

    #[test]
    fn text_only_identity_preserves_normalized_duplicate_semantics() {
        let front = doc(json!([{
            "type": "paragraph",
            "content": [{"type": "text", "text": "Question"}]
        }]));
        let back = answer_doc();
        let first = from_serialized_documents(&front, &back, "  QUESTION  ", "Answer").unwrap();
        let second = from_serialized_documents(&front, &back, "question", " answer ").unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn invalid_managed_identity_is_rejected_instead_of_becoming_public_text() {
        let back = answer_doc();
        let invalid = doc(json!([media("audio", "../private.mp3", "secret")]));
        assert!(from_serialized_documents(&invalid, &back, "[Audio]", "Answer").is_err());
    }
}

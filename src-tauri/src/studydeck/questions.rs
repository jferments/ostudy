//! Version-3 package projection. Authored keys and mask relationships travel as
//! typed content; attempts, reviews and source-evidence claims never do.

use super::*;
use crate::cards::questions::{QuestionBody, ValidatedQuestion};

pub(super) fn definition(note: &PackageNote) -> Result<Option<ValidatedQuestion>, CommandError> {
    if note.note_type != "interactive" {
        if note.question_definition.is_some() || note.content_format != "tiptap-json" {
            return Err(invalid("A recall note contains a typed question payload."));
        }
        return Ok(None);
    }
    if note.content_format != "question-json"
        || !note.front_doc.is_null()
        || !note.back_doc.is_null()
        || note.reverse_enabled
        || !note.cloze_source.is_empty()
    {
        return Err(invalid("An interactive note has competing content owners."));
    }
    let source = note
        .question_definition
        .as_deref()
        .ok_or_else(|| invalid("An interactive note is missing its definition."))?;
    Ok(Some(ValidatedQuestion::parse(source)?))
}

pub(super) fn normalize(note: &mut PackageNote) -> Result<(), CommandError> {
    if let Some(question) = definition(note)? {
        let text = question.search_text()?;
        note.front_text = text.prompt;
        note.back_text = text.answer;
        note.duplicate_hash = question.duplicate_hash()?;
        note.question_definition = Some(question.to_json()?);
    } else {
        let validated = validate_content(
            note.front_doc.clone(),
            note.back_doc.clone(),
            note.front_text.clone(),
            note.back_text.clone(),
        )
        .map_err(|error| invalid(error.message))?;
        note.front_doc =
            serde_json::from_str(&validated.front_doc_json).map_err(CommandError::serialization)?;
        note.back_doc =
            serde_json::from_str(&validated.back_doc_json).map_err(CommandError::serialization)?;
        note.front_text = validated.front_text;
        note.back_text = validated.back_text;
        if note.duplicate_hash.is_empty() {
            note.duplicate_hash = duplicate_fingerprint(&note.front_text, &note.back_text);
        }
    }
    Ok(())
}

/// Returns true only after the entire typed presentation graph is admitted.
pub(super) fn validate_presentations(
    note: &PackageNote,
    cards: &[&PackageCard],
    version: i64,
    assets: &[PackageAsset],
) -> Result<bool, CommandError> {
    let Some(question) = definition(note)? else {
        if cards.iter().any(|card| card.question_key.is_some()) {
            return Err(invalid(
                "A recall card contains an interactive presentation key.",
            ));
        }
        return Ok(false);
    };
    let expected = question
        .presentation_keys()
        .into_iter()
        .collect::<BTreeSet<_>>();
    let actual = cards
        .iter()
        .filter_map(|card| card.question_key.clone())
        .collect::<BTreeSet<_>>();
    if version < CURRENT_PACKAGE_VERSION
        || actual != expected
        || actual.len() != cards.len()
        || cards.iter().any(|card| card.direction != "forward")
    {
        return Err(invalid(
            "The interactive presentation keys do not match their note.",
        ));
    }
    if let QuestionBody::ImageOcclusion { image_asset_id, .. } = &question.definition().question {
        let image = assets
            .iter()
            .find(|asset| &asset.source_id == image_asset_id);
        if !image.is_some_and(|asset| {
            matches!(
                asset.mime_type.as_str(),
                "image/png" | "image/jpeg" | "image/webp"
            )
        }) {
            return Err(invalid(
                "An occlusion source must be a packaged PNG, JPEG or WebP image.",
            ));
        }
    }
    Ok(true)
}

pub(super) fn document_json(value: &Value) -> Result<Option<String>, CommandError> {
    if value.is_null() {
        Ok(None)
    } else {
        serde_json::to_string(value)
            .map(Some)
            .map_err(CommandError::serialization)
    }
}

/// Reuse the caller's single import transaction and native definition admission.
/// Item/mask keys are note-local; asset and note/card entity identities are mapped
/// independently. In particular, an image source is not a rich image node.
pub(super) fn insert_note(
    connection: &Connection,
    note: &PackageNote,
    note_id: &str,
    deck_id: &str,
    asset_map: &HashMap<String, String>,
) -> Result<bool, CommandError> {
    let Some(question) = definition(note)? else {
        return Ok(false);
    };
    let mut value =
        serde_json::to_value(question.definition()).map_err(CommandError::serialization)?;
    rewrite_asset_ids(&mut value, asset_map)?;
    if let QuestionBody::ImageOcclusion { image_asset_id, .. } = &question.definition().question {
        let mapped = asset_map
            .get(image_asset_id)
            .ok_or_else(|| invalid("An occlusion source is missing from the asset map."))?;
        value["question"]["imageAssetId"] = Value::String(mapped.clone());
    }
    let question = ValidatedQuestion::parse(&value.to_string())?;
    let text = question.search_text()?;
    connection
        .execute(
            "INSERT INTO notes (
                 id, deck_id, note_type, content_format, content_schema_version,
                 question_definition, front_text, back_text, duplicate_hash,
                 created_at, updated_at, revision
             ) VALUES (?1, ?2, 'interactive', 'question-json', 1,
                       ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                note_id,
                deck_id,
                question.to_json()?,
                text.prompt,
                text.answer,
                question.duplicate_hash()?,
                note.created_at,
                note.updated_at,
                note.revision,
            ],
        )
        .map_err(CommandError::database)?;
    Ok(true)
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::invalid_package(message)
}

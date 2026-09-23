//! Transactional application of subject-centered document artifact metadata.
//!
//! Desired values and the values ostudy last applied are separate. A later
//! planning pass may therefore refresh generated metadata without overwriting a
//! learner's document rename or deck description edit.

use rusqlite::{params, OptionalExtension, Transaction};

use crate::{
    ai::document_intelligence::{
        DocumentArtifactMetadataCandidate, DocumentOutline, ARTIFACT_METADATA_GENERATOR_VERSION,
        ARTIFACT_METADATA_PROMPT_VERSION, ARTIFACT_METADATA_SCHEMA_VERSION,
    },
    document_citations,
    error::CommandError,
};

#[path = "artifact_metadata_values.rs"]
mod values;

use values::{
    derive_desired_metadata, exact_generated_description, exact_source_deck_title,
    exact_source_document_title, DesiredMetadata,
};

#[derive(Clone, Debug)]
struct SourceRecord {
    current_title: String,
    original_filename: String,
    source_format: String,
    source_fingerprint_sha256: String,
    generated_deck_id: Option<String>,
    created_at: i64,
    updated_at: i64,
}

#[derive(Clone, Debug)]
struct ExistingMetadata {
    deck_id: Option<String>,
    source_fingerprint_sha256: String,
    metadata_origin: String,
    generator_version: i64,
    schema_version: i64,
    prompt_version: i64,
    document_title: String,
    deck_title: String,
    deck_description: String,
    applied_document_title: Option<String>,
    applied_deck_title: Option<String>,
    applied_deck_description: Option<String>,
}

pub(super) fn ensure_document_artifact_metadata(
    transaction: &Transaction<'_>,
    document_id: &str,
    outline: &DocumentOutline,
    provider_candidate: Option<&DocumentArtifactMetadataCandidate>,
    now: i64,
) -> Result<(), CommandError> {
    let source = load_source(transaction, document_id)?;

    // Capture source identity before any display title is changed. Migration
    // 0007 makes later citation-title refreshes immutable.
    document_citations::upsert_document_snapshot(transaction, document_id, now)?;
    let source_title = load_source_title(transaction, document_id)?;
    let source_title_hint = load_source_title_hint(transaction, document_id)?;
    let existing = load_existing_metadata(transaction, document_id)?;

    let desired = existing
        .as_ref()
        .filter(|metadata| {
            provider_candidate.is_none()
                && metadata.source_fingerprint_sha256 == source.source_fingerprint_sha256
                && metadata.generator_version == ARTIFACT_METADATA_GENERATOR_VERSION
                && metadata.schema_version == ARTIFACT_METADATA_SCHEMA_VERSION
                && metadata.prompt_version == ARTIFACT_METADATA_PROMPT_VERSION
        })
        .map(desired_from_existing)
        .unwrap_or_else(|| {
            derive_desired_metadata(
                outline,
                provider_candidate,
                source_title_hint.as_deref(),
                &source.original_filename,
                &source.source_format,
            )
        });

    persist_desired_metadata(
        transaction,
        document_id,
        &source,
        &desired,
        existing.as_ref(),
        now,
    )?;
    apply_document_title(
        transaction,
        document_id,
        &source,
        &source_title,
        existing
            .as_ref()
            .and_then(|value| value.applied_document_title.as_deref()),
        &desired.document_title,
        now,
    )?;
    if let Some(deck_id) = source
        .generated_deck_id
        .as_deref()
        .or_else(|| existing.as_ref().and_then(|value| value.deck_id.as_deref()))
    {
        apply_deck_metadata(
            transaction,
            deck_id,
            &source,
            &source_title,
            existing.as_ref(),
            &desired,
            now,
        )?;
    }
    record_applied_values(transaction, document_id)?;
    Ok(())
}

fn load_source(
    transaction: &Transaction<'_>,
    document_id: &str,
) -> Result<SourceRecord, CommandError> {
    transaction
        .query_row(
            "SELECT title, original_filename, source_format, sha256,
                    generated_deck_id, created_at, updated_at
             FROM documents
             WHERE id = ?1",
            [document_id],
            |row| {
                Ok(SourceRecord {
                    current_title: row.get(0)?,
                    original_filename: row.get(1)?,
                    source_format: row.get(2)?,
                    source_fingerprint_sha256: row.get(3)?,
                    generated_deck_id: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "document_not_found",
                "That document is no longer in the document library.",
            )
        })
}

fn load_source_title(
    transaction: &Transaction<'_>,
    document_id: &str,
) -> Result<String, CommandError> {
    transaction
        .query_row(
            "SELECT source_title
             FROM source_citation_snapshots
             WHERE source_document_id = ?1",
            [document_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)
}

fn load_source_title_hint(
    transaction: &Transaction<'_>,
    document_id: &str,
) -> Result<Option<String>, CommandError> {
    let mut statement = transaction
        .prepare(
            "SELECT role, text, source_unit_number
             FROM document_source_blocks
             WHERE document_id = ?1
               AND source_unit_number <= 3
               AND role IN ('title', 'heading')
             ORDER BY source_unit_number, block_ordinal, block_index
             LIMIT 24",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([document_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(source_title_hint_from_blocks(&rows))
}

fn source_title_hint_from_blocks(blocks: &[(String, String, i64)]) -> Option<String> {
    for (role, text, _) in blocks {
        let text = text.trim();
        if role == "title" && !text.is_empty() && !looks_like_chapter_marker(text) {
            return Some(text.to_string());
        }
    }
    for (index, (_, text, unit)) in blocks.iter().enumerate() {
        if !looks_like_chapter_marker(text) {
            continue;
        }
        if let Some((_, title, _)) = blocks
            .iter()
            .skip(index + 1)
            .take(5)
            .find(|(role, candidate, candidate_unit)| {
                *candidate_unit == *unit
                    && matches!(role.as_str(), "title" | "heading")
                    && !candidate.trim().is_empty()
                    && !looks_like_chapter_marker(candidate)
            })
        {
            return Some(title.trim().to_string());
        }
    }
    None
}

fn looks_like_chapter_marker(value: &str) -> bool {
    let normalized = value
        .trim()
        .trim_matches(|character: char| matches!(character, ':' | '-' | '–' | '—'))
        .to_ascii_lowercase();
    let mut parts = normalized.split_whitespace();
    matches!(parts.next(), Some("chapter"))
        && parts.next().is_some_and(|part| {
            part.chars()
                .all(|character| character.is_ascii_digit() || "ivxlcdm".contains(character))
        })
        && parts.next().is_none()
}

fn load_existing_metadata(
    transaction: &Transaction<'_>,
    document_id: &str,
) -> Result<Option<ExistingMetadata>, CommandError> {
    transaction
        .query_row(
            "SELECT deck_id, source_fingerprint_sha256, metadata_origin,
                    generator_version, schema_version, prompt_version,
                    document_title, deck_title, deck_description,
                    applied_document_title, applied_deck_title,
                    applied_deck_description
             FROM document_artifact_metadata
             WHERE document_id = ?1",
            [document_id],
            |row| {
                Ok(ExistingMetadata {
                    deck_id: row.get(0)?,
                    source_fingerprint_sha256: row.get(1)?,
                    metadata_origin: row.get(2)?,
                    generator_version: row.get(3)?,
                    schema_version: row.get(4)?,
                    prompt_version: row.get(5)?,
                    document_title: row.get(6)?,
                    deck_title: row.get(7)?,
                    deck_description: row.get(8)?,
                    applied_document_title: row.get(9)?,
                    applied_deck_title: row.get(10)?,
                    applied_deck_description: row.get(11)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)
}

fn desired_from_existing(existing: &ExistingMetadata) -> DesiredMetadata {
    DesiredMetadata {
        metadata_origin: if existing.metadata_origin == "provider" {
            "provider"
        } else {
            "deterministic_fallback"
        },
        document_title: existing.document_title.clone(),
        deck_title: existing.deck_title.clone(),
        deck_description: existing.deck_description.clone(),
    }
}

fn persist_desired_metadata(
    transaction: &Transaction<'_>,
    document_id: &str,
    source: &SourceRecord,
    desired: &DesiredMetadata,
    existing: Option<&ExistingMetadata>,
    now: i64,
) -> Result<(), CommandError> {
    let deck_id = source
        .generated_deck_id
        .as_deref()
        .or_else(|| existing.and_then(|value| value.deck_id.as_deref()));
    transaction
        .execute(
            "INSERT INTO document_artifact_metadata (
                 document_id, deck_id, source_fingerprint_sha256, metadata_origin,
                 generator_version, schema_version, prompt_version,
                 document_title, deck_title, deck_description,
                 applied_document_title, applied_deck_title,
                 applied_deck_description, created_at, updated_at
             ) VALUES (
                 ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                 NULL, NULL, NULL, ?11, ?12
             )
             ON CONFLICT(document_id) DO UPDATE SET
                 deck_id = excluded.deck_id,
                 source_fingerprint_sha256 = excluded.source_fingerprint_sha256,
                 metadata_origin = excluded.metadata_origin,
                 generator_version = excluded.generator_version,
                 schema_version = excluded.schema_version,
                 prompt_version = excluded.prompt_version,
                 document_title = excluded.document_title,
                 deck_title = excluded.deck_title,
                 deck_description = excluded.deck_description,
                 updated_at = MAX(document_artifact_metadata.updated_at, excluded.updated_at)",
            params![
                document_id,
                deck_id,
                source.source_fingerprint_sha256.as_str(),
                desired.metadata_origin,
                ARTIFACT_METADATA_GENERATOR_VERSION,
                ARTIFACT_METADATA_SCHEMA_VERSION,
                ARTIFACT_METADATA_PROMPT_VERSION,
                desired.document_title.as_str(),
                desired.deck_title.as_str(),
                desired.deck_description.as_str(),
                source.created_at,
                now.max(source.updated_at),
            ],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

fn apply_document_title(
    transaction: &Transaction<'_>,
    document_id: &str,
    source: &SourceRecord,
    source_title: &str,
    previously_applied: Option<&str>,
    desired_title: &str,
    now: i64,
) -> Result<(), CommandError> {
    if source.current_title == desired_title {
        return Ok(());
    }
    let generated = previously_applied == Some(source.current_title.as_str())
        || exact_source_document_title(
            &source.current_title,
            source_title,
            &source.original_filename,
            &source.source_format,
        );
    if !generated {
        return Ok(());
    }
    let changed = transaction
        .execute(
            "UPDATE documents
             SET title = ?1, updated_at = MAX(updated_at, ?2), revision = revision + 1
             WHERE id = ?3 AND title = ?4",
            params![
                desired_title,
                now,
                document_id,
                source.current_title.as_str()
            ],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(CommandError::new(
            "document_artifact_metadata_conflict",
            "The document title changed while generated metadata was being applied.",
        ));
    }
    Ok(())
}

fn apply_deck_metadata(
    transaction: &Transaction<'_>,
    deck_id: &str,
    source: &SourceRecord,
    source_title: &str,
    existing: Option<&ExistingMetadata>,
    desired: &DesiredMetadata,
    now: i64,
) -> Result<(), CommandError> {
    let deck = transaction
        .query_row(
            "SELECT name, description
             FROM decks
             WHERE id = ?1 AND deleted_at IS NULL",
            [deck_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((current_name, current_description)) = deck else {
        return Ok(());
    };

    let name_is_generated = existing.and_then(|value| value.applied_deck_title.as_deref())
        == Some(current_name.as_str())
        || exact_source_deck_title(
            &current_name,
            source_title,
            &source.original_filename,
            &source.source_format,
        );
    let description_is_generated = existing
        .and_then(|value| value.applied_deck_description.as_deref())
        == Some(current_description.as_str())
        || exact_generated_description(&current_description, &source.original_filename);

    let next_name = if name_is_generated {
        desired.deck_title.as_str()
    } else {
        current_name.as_str()
    };
    let next_description = if description_is_generated {
        desired.deck_description.as_str()
    } else {
        current_description.as_str()
    };
    if next_name == current_name && next_description == current_description {
        return Ok(());
    }

    let changed = transaction
        .execute(
            "UPDATE decks
             SET name = ?1, description = ?2,
                 updated_at = MAX(updated_at, ?3), revision = revision + 1
             WHERE id = ?4 AND name = ?5 AND description = ?6 AND deleted_at IS NULL",
            params![
                next_name,
                next_description,
                now,
                deck_id,
                current_name.as_str(),
                current_description.as_str()
            ],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(CommandError::new(
            "document_artifact_metadata_conflict",
            "The generated deck metadata changed while ostudy was refreshing it.",
        ));
    }
    Ok(())
}

fn record_applied_values(
    transaction: &Transaction<'_>,
    document_id: &str,
) -> Result<(), CommandError> {
    transaction
        .execute(
            "UPDATE document_artifact_metadata
             SET applied_document_title = CASE
                     WHEN (SELECT title FROM documents
                           WHERE id = document_artifact_metadata.document_id) = document_title
                     THEN document_title
                     ELSE applied_document_title
                 END,
                 applied_deck_title = CASE
                     WHEN (SELECT name FROM decks
                           WHERE id = document_artifact_metadata.deck_id) = deck_title
                     THEN deck_title
                     ELSE applied_deck_title
                 END,
                 applied_deck_description = CASE
                     WHEN (SELECT description FROM decks
                           WHERE id = document_artifact_metadata.deck_id) = deck_description
                     THEN deck_description
                     ELSE applied_deck_description
                 END
             WHERE document_id = ?1",
            [document_id],
        )
        .map_err(CommandError::database)?;
    Ok(())
}


#[cfg(test)]
mod source_title_tests {
    use super::*;

    #[test]
    fn chapter_marker_promotes_the_following_heading_to_document_scope() {
        let blocks = vec![
            ("heading".to_string(), "CHAPTER 8".to_string(), 1),
            ("heading".to_string(), "Gastrointestinal Physiology".to_string(), 1),
            ("heading".to_string(), "Structure of the Gastrointestinal Tract".to_string(), 1),
        ];
        assert_eq!(
            source_title_hint_from_blocks(&blocks).as_deref(),
            Some("Gastrointestinal Physiology")
        );
    }

    #[test]
    fn explicit_title_beats_section_headings() {
        let blocks = vec![
            ("title".to_string(), "Pulmonary Physiology".to_string(), 1),
            ("heading".to_string(), "Ventilation".to_string(), 1),
        ];
        assert_eq!(
            source_title_hint_from_blocks(&blocks).as_deref(),
            Some("Pulmonary Physiology")
        );
    }
}

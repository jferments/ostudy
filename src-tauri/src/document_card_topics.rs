//! Source provenance bridge from document generation windows to imported card notes.
//!
//! Document card curation is selection-only. This module reattaches each returned
//! validated card to the canonical source-window locator that authored it, then
//! commits stable topic links after card import. Provider output never supplies a
//! topic ID and learner-editable tags are not used as provenance.

use std::collections::{HashMap, HashSet, VecDeque};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    authoring::card_types_and_templates::generated_import::GeneratedImportCommitContext,
    database::Database, document_domain::SourceLocator, document_learning::DocumentLearningPlan,
    document_semantics::SemanticWindow, document_topic_links::link_generated_note_to_learning_plan,
    error::CommandError,
};

#[derive(Clone, Debug)]
pub(crate) struct SourceGroundedGeneratedCard {
    pub card: Value,
    pub source_locator: SourceLocator,
}

/// Attach flat, deterministic generation output to the source-window card counts
/// already committed by the document generation executor.
pub(crate) fn attach_generated_card_source_windows(
    database: &Database,
    job_id: &str,
    windows: &[SemanticWindow],
    cards: Vec<Value>,
) -> Result<Vec<SourceGroundedGeneratedCard>, CommandError> {
    let counts = database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT window_index, card_count
                 FROM document_generation_windows
                 WHERE job_id = ?1 AND state IN ('succeeded', 'skipped')
                 ORDER BY window_index",
            )
            .map_err(CommandError::database)?;
        statement
            .query_map([job_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)
    })?;
    if counts.len() != windows.len() {
        return Err(CommandError::new(
            "document_card_provenance_incomplete",
            "The completed document generation job did not retain one source-window result for every planned window.",
        ));
    }

    let mut cursor = 0_usize;
    let mut grounded = Vec::with_capacity(cards.len());
    for (expected_index, ((stored_index, count), window)) in
        counts.into_iter().zip(windows).enumerate()
    {
        let stored_index = usize::try_from(stored_index).map_err(|_| {
            CommandError::new(
                "document_card_provenance_invalid",
                "A persisted generation-window index was invalid.",
            )
        })?;
        let count = usize::try_from(count).map_err(|_| {
            CommandError::new(
                "document_card_provenance_invalid",
                "A persisted generation-window card count was invalid.",
            )
        })?;
        if stored_index != expected_index {
            return Err(CommandError::new(
                "document_card_provenance_invalid",
                "Document generation windows were not persisted in canonical plan order.",
            ));
        }
        let end = cursor.saturating_add(count);
        if end > cards.len() {
            return Err(CommandError::new(
                "document_card_provenance_incomplete",
                "A generation-window card count exceeded the returned validated card set.",
            ));
        }
        let locator = window.source_locator();
        grounded.extend(cards[cursor..end].iter().cloned().map(|card| {
            SourceGroundedGeneratedCard {
                card,
                source_locator: locator.clone(),
            }
        }));
        cursor = end;
    }
    if cursor != cards.len() {
        return Err(CommandError::new(
            "document_card_provenance_incomplete",
            "The returned document cards exceeded the persisted source-window card counts.",
        ));
    }
    Ok(grounded)
}

/// Match selection-only curated values back to source-grounded candidates.
/// Duplicate card values use deterministic FIFO source order.
pub(crate) fn retain_curated_card_provenance(
    candidates: Vec<SourceGroundedGeneratedCard>,
    curated_cards: Vec<Value>,
) -> Result<Vec<SourceGroundedGeneratedCard>, CommandError> {
    let mut by_identity = HashMap::<String, VecDeque<SourceGroundedGeneratedCard>>::new();
    for candidate in candidates {
        by_identity
            .entry(card_identity(&candidate.card)?)
            .or_default()
            .push_back(candidate);
    }

    curated_cards
        .into_iter()
        .map(|card| {
            let identity = card_identity(&card)?;
            let mut candidate = by_identity
                .get_mut(&identity)
                .and_then(VecDeque::pop_front)
                .ok_or_else(|| {
                    CommandError::new(
                        "document_card_curation_changed_content",
                        "Document card curation returned content outside the validated candidate set.",
                    )
                })?;
            candidate.card = card;
            Ok(candidate)
        })
        .collect()
}

/// Attach stable topic links to exactly the notes that the generated importer
/// created. `source_index` originates from the JSON card array and survives
/// duplicate filtering, so skipped duplicates cannot shift provenance onto a
/// different note.
pub(crate) fn link_imported_notes_to_topics(
    connection: &rusqlite::Connection,
    document_id: &str,
    generation_job_id: &str,
    plan: &DocumentLearningPlan,
    cards: &[SourceGroundedGeneratedCard],
    commit_context: &GeneratedImportCommitContext,
    imported_at: i64,
) -> Result<(), CommandError> {
    let mut seen_indices = HashSet::new();
    for created in &commit_context.created_notes {
        if !seen_indices.insert(created.source_index) {
            return Err(CommandError::new(
                "document_card_topic_commit_invalid",
                "Generated import provenance contained a duplicate source-card index.",
            ));
        }
        let card = cards.get(created.source_index).ok_or_else(|| {
            CommandError::new(
                "document_card_topic_commit_incomplete",
                "Generated import provenance referenced a source card outside the curated candidate set.",
            )
        })?;
        link_generated_note_to_learning_plan(
            connection,
            document_id,
            &created.note_id,
            Some(generation_job_id),
            plan,
            &card.source_locator,
            imported_at,
        )?;
    }
    Ok(())
}

fn card_identity(card: &Value) -> Result<String, CommandError> {
    let serialized = serde_json::to_vec(card).map_err(CommandError::serialization)?;
    Ok(format!("{:x}", Sha256::digest(serialized)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_domain::{SourceLocator, SourceUnitKind};

    fn locator(unit: i64) -> SourceLocator {
        SourceLocator::new(SourceUnitKind::Page, vec![unit], None).unwrap()
    }

    #[test]
    fn curation_preserves_fifo_provenance_for_equal_values() {
        let card = serde_json::json!({"type": "basic", "front": "Q", "back": "A"});
        let candidates = vec![
            SourceGroundedGeneratedCard {
                card: card.clone(),
                source_locator: locator(1),
            },
            SourceGroundedGeneratedCard {
                card: card.clone(),
                source_locator: locator(2),
            },
        ];
        let curated = retain_curated_card_provenance(candidates, vec![card.clone(), card]).unwrap();
        assert_eq!(curated[0].source_locator.first(), 1);
        assert_eq!(curated[1].source_locator.first(), 2);
    }

    #[test]
    fn curation_cannot_introduce_new_card_content() {
        let candidate = SourceGroundedGeneratedCard {
            card: serde_json::json!({"type": "basic", "front": "Q", "back": "A"}),
            source_locator: locator(1),
        };
        let error = retain_curated_card_provenance(
            vec![candidate],
            vec![serde_json::json!({"type": "basic", "front": "Different", "back": "A"})],
        )
        .unwrap_err();
        assert_eq!(error.code, "document_card_curation_changed_content");
    }
}

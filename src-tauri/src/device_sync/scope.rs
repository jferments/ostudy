use std::collections::BTreeSet;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::{
    cards::questions::{AttemptProgress, QuestionAttempt, ValidatedQuestion, Verdict},
    error::CommandError,
};

/// Sender-selected synchronization domains.
///
/// The selection is application-owned policy. Transport adapters only move the
/// authenticated byte stream; they never decide which learner data belongs in a
/// transfer. Dependencies are explicit so a future raw-USB/iOS adapter can reuse
/// exactly the same package and merge semantics.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncSelection {
    pub flashcards: bool,
    pub documents: bool,
    pub study_guides: bool,
    pub api_settings: bool,
    pub api_credentials: bool,
}

impl Default for SyncSelection {
    fn default() -> Self {
        Self {
            flashcards: true,
            documents: true,
            study_guides: true,
            api_settings: false,
            api_credentials: false,
        }
    }
}

impl SyncSelection {
    /// Scope represented by the pre-granular package format.
    pub(crate) fn legacy(include_api_settings: bool) -> Self {
        Self {
            flashcards: true,
            documents: false,
            study_guides: false,
            api_settings: include_api_settings,
            api_credentials: false,
        }
    }

    pub(crate) fn validate(self) -> Result<Self, CommandError> {
        if self.study_guides && !self.documents {
            return Err(CommandError::new(
                "device_sync_scope_invalid",
                "Study Guides can only be synchronized together with Documents.",
            ));
        }
        if self.api_credentials && !self.api_settings {
            return Err(CommandError::new(
                "device_sync_scope_invalid",
                "Saved API keys can only be synchronized together with API provider settings.",
            ));
        }
        if !self.flashcards && !self.documents && !self.api_settings {
            return Err(CommandError::new(
                "device_sync_scope_empty",
                "Select at least one synchronization category.",
            ));
        }
        Ok(self)
    }

    pub(crate) fn includes_table(self, table: &str) -> bool {
        if FLASHCARD_TABLES.contains(&table) {
            return self.flashcards;
        }
        if DOCUMENT_TABLES.contains(&table) {
            return self.documents;
        }
        if STUDY_GUIDE_TABLES.contains(&table) {
            return self.study_guides;
        }
        if CROSS_DOCUMENT_FLASHCARD_TABLES.contains(&table) {
            return self.documents && self.flashcards;
        }
        if crate::ai::profiles::is_device_transfer_table(table) {
            return self.api_settings;
        }
        false
    }
}

pub(crate) const FLASHCARD_TABLES: &[&str] = &[
    "deck_folders",
    "decks",
    "tags",
    "card_templates",
    "card_template_fields",
    "assets",
    "notes",
    "question_generation_evidence",
    "cards",
    "card_question_presentations",
    "card_template_rules",
    "note_tags",
    "note_assets",
    "card_template_assets",
    "card_schedules",
    "practice_sessions",
    "practice_session_cards",
    "question_attempts",
    "question_attempt_assets",
    "practice_session_card_snapshots",
    "practice_session_card_tag_snapshots",
    "review_events",
    "review_event_attributions",
    "review_event_tag_attributions",
    "question_review_links",
    "question_remediation_attempts",
    "source_citation_snapshots",
    "deck_source_citations",
    "note_source_citations",
];

pub(crate) const DOCUMENT_TABLES: &[&str] = &[
    "documents",
    "document_pages",
    "document_semantic_elements",
    "document_semantic_chunks",
    "document_outlines",
    "document_outline_checkpoints",
    "document_generation_jobs",
    "question_generation_reports",
    "document_generation_units",
    "document_generation_windows",
    "document_generation_checkpoints",
    "document_source_files",
    "document_source_units",
    "document_source_blocks",
    "document_source_assets",
    "document_semantic_chunk_sources",
    "document_learning_plans",
    "document_learning_topics",
    "document_learning_plan_topics",
    "document_artifact_jobs",
    "document_artifact_metadata",
];

pub(crate) const STUDY_GUIDE_TABLES: &[&str] = &[
    "document_study_guides",
    "document_study_guide_sections",
    "document_study_guide_overlays",
    "document_study_guide_checkpoints",
];

pub(crate) const CROSS_DOCUMENT_FLASHCARD_TABLES: &[&str] =
    &["document_decks", "document_note_topics"];

/// Native semantic admission for a copied/received question graph. The SQLite
/// constraints prove relational shape; this additionally proves that active
/// presentation keys still belong to the canonical typed definition and that
/// private attempt checkpoints restore to their recorded owner and first result.
pub(crate) fn require_supported_question_snapshot(
    connection: &Connection,
) -> Result<(), CommandError> {
    let mut note_statement = connection
        .prepare(
            "SELECT id, question_definition FROM notes
             WHERE note_type = 'interactive' AND deleted_at IS NULL ORDER BY id",
        )
        .map_err(CommandError::database)?;
    let notes = note_statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    drop(note_statement);

    for (note_id, definition) in notes {
        let question = ValidatedQuestion::parse(&definition).map_err(question_invalid)?;
        let expected = question
            .presentation_keys()
            .into_iter()
            .collect::<BTreeSet<_>>();
        let mut presentation_statement = connection
            .prepare(
                "SELECT qp.question_key FROM card_question_presentations qp
                 JOIN cards c ON c.id = qp.card_id
                 WHERE qp.note_id = ?1 AND c.deleted_at IS NULL
                 ORDER BY qp.question_key",
            )
            .map_err(CommandError::database)?;
        let actual = presentation_statement
            .query_map([&note_id], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<BTreeSet<_>, _>>()
            .map_err(CommandError::database)?;
        if actual != expected {
            return Err(question_invalid_message(
                "An interactive note's active presentation keys do not match its canonical definition.",
            ));
        }
        let active_cards: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM cards WHERE note_id = ?1 AND deleted_at IS NULL",
                [&note_id],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        if usize::try_from(active_cards).ok() != Some(expected.len()) {
            return Err(question_invalid_message(
                "An interactive note has an active card without a canonical presentation.",
            ));
        }
        let linked_assets = {
            let mut statement = connection
                .prepare(
                    "SELECT asset_id FROM note_assets WHERE note_id = ?1 AND side = 'content' ORDER BY asset_id",
                )
                .map_err(CommandError::database)?;
            let rows = statement
                .query_map([&note_id], |row| row.get::<_, String>(0))
                .map_err(CommandError::database)?;
            rows.collect::<Result<BTreeSet<_>, _>>()
                .map_err(CommandError::database)?
        };
        let expected_assets = question.asset_ids().iter().cloned().collect::<BTreeSet<_>>();
        if linked_assets != expected_assets {
            return Err(question_invalid_message(
                "An interactive note's managed-asset links do not match its canonical definition.",
            ));
        }
    }

    let invalid_recall_presentation: bool = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM card_question_presentations qp
                 JOIN cards c ON c.id = qp.card_id
                 JOIN notes n ON n.id = qp.note_id
                 WHERE c.deleted_at IS NULL AND n.note_type <> 'interactive')",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if invalid_recall_presentation {
        return Err(question_invalid_message(
            "A recall card contains an interactive presentation identity.",
        ));
    }

    let mut attempt_statement = connection
        .prepare(
            "SELECT q.instance_id, q.card_id, q.note_id, q.question_kind, q.instance_hash,
                    q.revision, q.phase, q.assisted, q.checkpoint_json,
                    q.first_checked_at, q.verdict, n.question_definition
             FROM question_attempts q JOIN notes n ON n.id = q.note_id
             ORDER BY q.instance_id",
        )
        .map_err(CommandError::database)?;
    let attempts = attempt_statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, Option<i64>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<String>>(11)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    drop(attempt_statement);

    for row in attempts {
        let definition = row.11.ok_or_else(|| {
            question_invalid_message("A question attempt belongs to a non-interactive note.")
        })?;
        let current = ValidatedQuestion::parse(&definition).map_err(question_invalid)?;
        if current.kind().as_str() != row.3 {
            return Err(question_invalid_message(
                "A question attempt's type does not match its retained note.",
            ));
        }
        let attempt = QuestionAttempt::restore(&row.8).map_err(question_invalid)?;
        let identity = attempt.instance().identity();
        if identity.instance_id != row.0
            || identity.card_id != row.1
            || identity.note_id != row.2
            || attempt.instance().fingerprint() != row.4
            || attempt.revision() != row.5
        {
            return Err(question_invalid_message(
                "A question attempt checkpoint does not match its stored identity.",
            ));
        }
        let view = attempt.view().map_err(question_invalid)?;
        if i64::from(view.assisted) != row.7 {
            return Err(question_invalid_message(
                "A question attempt checkpoint has inconsistent assistance evidence.",
            ));
        }
        let (phase, checked_at, verdict) = match view.progress {
            AttemptProgress::Answering { .. } => ("answering", None, None),
            AttemptProgress::Checked {
                checked_at_ms,
                evaluation,
                ..
            } => (
                "checked",
                Some(checked_at_ms),
                Some(match evaluation.verdict {
                    Verdict::Correct => "correct",
                    Verdict::Incorrect => "incorrect",
                    Verdict::Incomplete => "incomplete",
                }),
            ),
            AttemptProgress::Revealed { revealed_at_ms, .. } => {
                ("revealed", Some(revealed_at_ms), None)
            }
        };
        if row.6 != phase || row.9 != checked_at || row.10.as_deref() != verdict {
            return Err(question_invalid_message(
                "A question attempt's indexed first-result fields do not match its immutable checkpoint.",
            ));
        }
    }

    let mismatched_snapshot: bool = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM practice_session_card_snapshots snapshot
                 JOIN question_attempts attempt ON attempt.instance_id = snapshot.question_instance_id
                 WHERE snapshot.question_instance_id IS NOT NULL
                   AND (snapshot.card_id <> attempt.card_id OR snapshot.note_id <> attempt.note_id))",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    let mismatched_review: bool = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM question_review_links link
                 JOIN question_attempts attempt ON attempt.instance_id = link.instance_id
                 JOIN review_events review ON review.id = link.review_event_id
                 WHERE review.card_id <> attempt.card_id)",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if mismatched_snapshot || mismatched_review {
        return Err(question_invalid_message(
            "Question evidence is linked to a different retained card or note.",
        ));
    }
    Ok(())
}

pub(crate) use require_supported_question_snapshot as validate_question_snapshot;

fn question_invalid(error: crate::cards::questions::QuestionError) -> CommandError {
    question_invalid_message(error.message)
}

fn question_invalid_message(message: impl Into<String>) -> CommandError {
    CommandError::new("device_sync_question_invalid", message)
}

/// Remove every unselected domain from a copied SQLite snapshot before the
/// snapshot becomes transfer material. The copied database is deliberately
/// sanitized even though the merge engine is also scope-aware: a category that
/// the sender did not choose must not hitchhike inside the encrypted package.
pub(crate) fn sanitize_snapshot(
    connection: &Connection,
    selection: SyncSelection,
) -> Result<(), CommandError> {
    let selection = selection.validate()?;
    if selection.flashcards {
        require_supported_question_snapshot(connection)?;
    }
    // A sync package is not a complete-library backup. Remove private learning
    // state before deleting selected-domain parents, including serialized source
    // projections which can otherwise carry unselected material as plain text.
    connection
        .execute_batch("PRAGMA secure_delete = ON;")
        .map_err(CommandError::database)?;
    crate::tutor::sanitize_for_device_transfer(connection)?;
    crate::knowledge::sanitize_for_device_transfer(connection)?;
    connection
        .execute_batch(
            "PRAGMA foreign_keys = OFF;
             DELETE FROM app_settings;
             DELETE FROM card_search_fts;
             DELETE FROM asset_reconciliation_events;
             DELETE FROM editor_draft_assets;
             DELETE FROM editor_drafts;
             DELETE FROM question_note_writes;",
        )
        .map_err(CommandError::database)?;

    if !selection.study_guides && selection.documents {
        connection
            .execute_batch(
                "DELETE FROM document_study_guide_sections;
                 DELETE FROM document_study_guide_overlays;
                 DELETE FROM document_study_guide_checkpoints;
                 DELETE FROM document_study_guides;
                 DELETE FROM document_artifact_jobs WHERE artifact_kind = 'study_guide';",
            )
            .map_err(CommandError::database)?;
    }

    if !selection.flashcards {
        // Documents may retain their source/LLM processing graph without the
        // generated flashcard artifact. Null nullable cross-domain references
        // before removing the card graph itself.
        if selection.documents {
            connection
                .execute_batch(
                    "UPDATE documents SET generated_deck_id = NULL;
                     UPDATE document_generation_jobs SET generated_deck_id = NULL;
                     UPDATE document_artifact_metadata SET deck_id = NULL;
                     DELETE FROM document_decks;
                     DELETE FROM document_note_topics;",
                )
                .map_err(CommandError::database)?;
        }
        connection
            .execute_batch(
                "DELETE FROM question_generation_evidence;
                 DELETE FROM question_remediation_attempts;
                 DELETE FROM question_review_links;
                 DELETE FROM question_attempt_assets;
                 DELETE FROM question_attempts;
                 DELETE FROM card_question_presentations;
                 DELETE FROM review_event_tag_attributions;
                 DELETE FROM review_event_attributions;
                 DELETE FROM review_events;
                 DELETE FROM practice_session_card_tag_snapshots;
                 DELETE FROM practice_session_card_snapshots;
                 DELETE FROM practice_session_cards;
                 DELETE FROM practice_sessions;
                 DELETE FROM card_schedules;
                 DELETE FROM note_source_citations;
                 DELETE FROM deck_source_citations;
                 DELETE FROM source_citation_snapshots;
                 DELETE FROM note_assets;
                 DELETE FROM card_template_assets;
                 DELETE FROM note_tags;
                 DELETE FROM card_template_rules;
                 DELETE FROM cards;
                 DELETE FROM notes;
                 DELETE FROM assets;
                 DELETE FROM card_template_fields;
                 DELETE FROM card_templates;
                 DELETE FROM tags;
                 DELETE FROM decks;
                 DELETE FROM deck_folders;",
            )
            .map_err(CommandError::database)?;
    } else if !selection.documents {
        // Portable source citations remain part of the flashcard graph. Their
        // source_document_id intentionally is not a foreign key, so generated
        // cards can retain provenance without transferring the live document.
        connection
            .execute_batch(
                "UPDATE question_generation_evidence SET document_id = NULL;
                 DELETE FROM document_decks;
                 DELETE FROM document_note_topics;",
            )
            .map_err(CommandError::database)?;
    }

    if selection.documents && !selection.api_settings {
        connection
            .execute_batch(
                "UPDATE document_outlines SET profile_id = NULL;
                 UPDATE document_generation_jobs SET profile_id = NULL;
                 UPDATE document_artifact_jobs SET profile_id = NULL;",
            )
            .map_err(CommandError::database)?;
    }

    if !selection.documents {
        connection
            .execute_batch(
                "DELETE FROM document_study_guide_sections;
                 DELETE FROM document_study_guide_overlays;
                 DELETE FROM document_study_guide_checkpoints;
                 DELETE FROM document_study_guides;
                 DELETE FROM document_artifact_metadata;
                 DELETE FROM document_artifact_jobs;
                 DELETE FROM document_learning_plan_topics;
                 DELETE FROM document_learning_topics;
                 DELETE FROM document_learning_plans;
                 DELETE FROM document_generation_checkpoints;
                 DELETE FROM document_generation_windows;
                 DELETE FROM document_generation_units;
                 DELETE FROM question_generation_reports;
                 DELETE FROM document_generation_jobs;
                 DELETE FROM document_outline_checkpoints;
                 DELETE FROM document_outlines;
                 DELETE FROM document_semantic_chunk_sources;
                 DELETE FROM document_source_assets;
                 DELETE FROM document_source_blocks;
                 DELETE FROM document_source_units;
                 DELETE FROM document_source_files;
                 DELETE FROM document_semantic_chunks;
                 DELETE FROM document_semantic_elements;
                 DELETE FROM document_pages;
                 DELETE FROM documents;",
            )
            .map_err(CommandError::database)?;
    }

    // AI subdomains own their own SQLite schema and transfer-table identity.
    // Sync asks them for a safe transfer projection rather than duplicating
    // provider/probe schema knowledge here.
    crate::ai::probes::sanitize_for_device_transfer(connection)?;
    crate::ai::profiles::sanitize_for_device_transfer(connection, selection.api_settings)?;

    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(CommandError::database)?;
    let foreign_key_violations: i64 = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .map_err(CommandError::database)?;
    if foreign_key_violations != 0 {
        return Err(CommandError::new(
            "device_sync_snapshot_invalid",
            "The selected synchronization categories would produce an incomplete relational graph.",
        ));
    }
    if selection.flashcards {
        require_supported_question_snapshot(connection)?;
    }
    // The package contains the SQLite file, not just live rows. Deleted private
    // text must not survive in free pages or an uncheckpointed WAL snapshot.
    // This connection belongs to the transfer copy, never the live library.
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE; VACUUM;")
        .map_err(CommandError::database)?;
    Ok(())
}

#[cfg(test)]
#[path = "question_transfer_tests.rs"]
mod question_transfer_tests;

#[cfg(test)]
#[path = "privacy_tests.rs"]
mod privacy_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependency_rules_are_explicit() {
        assert!(SyncSelection {
            flashcards: true,
            documents: false,
            study_guides: true,
            api_settings: false,
            api_credentials: false,
        }
        .validate()
        .is_err());
        assert!(SyncSelection {
            flashcards: true,
            documents: false,
            study_guides: false,
            api_settings: false,
            api_credentials: true,
        }
        .validate()
        .is_err());
    }

    #[test]
    fn table_scope_keeps_typed_evidence_and_cross_graph_links_explicit() {
        let documents_only = SyncSelection {
            flashcards: false,
            documents: true,
            study_guides: true,
            api_settings: false,
            api_credentials: false,
        };
        assert!(documents_only.includes_table("documents"));
        assert!(documents_only.includes_table("question_generation_reports"));
        assert!(documents_only.includes_table("document_study_guides"));
        assert!(!documents_only.includes_table("document_decks"));
        assert!(!documents_only.includes_table("notes"));
        assert!(!documents_only.includes_table("question_attempts"));

        let all_learning = SyncSelection {
            flashcards: true,
            ..documents_only
        };
        assert!(all_learning.includes_table("card_question_presentations"));
        assert!(all_learning.includes_table("question_attempts"));
        assert!(all_learning.includes_table("question_generation_evidence"));
        assert!(all_learning.includes_table("document_decks"));
        assert!(all_learning.includes_table("document_note_topics"));
    }
}

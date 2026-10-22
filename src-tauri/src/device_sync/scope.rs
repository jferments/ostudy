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
    /// Notebooks with their pages, marks and pictures. Packages made before
    /// notebooks travelled have no such field and carry none.
    #[serde(default)]
    pub notes: bool,
    /// Lessons with their progress, teacher's lecture scripts, podcasts, voices,
    /// narration audio and the marks drawn on slides.
    #[serde(default)]
    pub lessons: bool,
    pub api_settings: bool,
    pub api_credentials: bool,
}

impl Default for SyncSelection {
    fn default() -> Self {
        Self {
            flashcards: true,
            documents: true,
            study_guides: true,
            notes: true,
            lessons: true,
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
            notes: false,
            lessons: false,
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
        if !self.flashcards
            && !self.documents
            && !self.notes
            && !self.lessons
            && !self.api_settings
        {
            return Err(CommandError::new(
                "device_sync_scope_empty",
                "Select at least one synchronization category.",
            ));
        }
        Ok(self)
    }

    pub(crate) fn includes_table(self, table: &str) -> bool {
        if SUBJECT_TABLES.contains(&table) {
            return self.carries_subjects();
        }
        if FLASHCARD_TABLES.contains(&table) {
            return self.flashcards;
        }
        if DOCUMENT_TABLES.contains(&table) {
            return self.documents;
        }
        if STUDY_GUIDE_TABLES.contains(&table) {
            return self.study_guides;
        }
        if NOTE_TABLES.contains(&table) {
            return self.notes;
        }
        if LESSON_TABLES.contains(&table) {
            return self.lessons;
        }
        if CROSS_DOCUMENT_FLASHCARD_TABLES.contains(&table) {
            return self.documents && self.flashcards;
        }
        if crate::ai::profiles::is_device_transfer_table(table) {
            return self.api_settings;
        }
        false
    }

    /// Whether subjects travel: everything but AI settings is filed in one.
    pub(crate) fn carries_subjects(self) -> bool {
        self.flashcards || self.documents || self.notes || self.lessons
    }
}

/// Subjects hold decks, documents, notebooks and lessons, so they travel with
/// any of those categories.
/// Removing them from a Flashcards-only package would leave every filed deck
/// pointing at a missing subject, and clearing `decks.subject_id` instead would
/// unfile the receiver's decks: the receiver would see the sender change it.
pub(crate) const SUBJECT_TABLES: &[&str] = &["document_collection_organization", "document_collections"];

pub(crate) const FLASHCARD_TABLES: &[&str] = &[
    "decks",
    "tags",
    "card_templates",
    "card_template_fields",
    "assets",
    "notes",
    "question_generation_evidence",
    "cards",
    "card_merges",
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
    "review_event_scheduler_context",
    "study_activity_retained",
    "question_review_links",
    "question_remediation_attempts",
    "source_citation_snapshots",
    "deck_source_citations",
    "note_source_citations",
];

pub(crate) const DOCUMENT_TABLES: &[&str] = &[
    "documents",
    "document_study_ranges",
    "study_targets",
    "study_target_suppressions",
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
    "document_annotations",
    "document_figure_scans",
    "document_figures",
    "document_figure_passages",
    "document_figure_readings",
];

pub(crate) const STUDY_GUIDE_TABLES: &[&str] = &[
    "document_study_guides",
    "document_study_guide_sections",
    "document_study_guide_overlays",
    "document_study_guide_checkpoints",
    "study_target_guides",
    "study_target_guide_overlays",
    "document_study_guide_section_reads",
    "document_study_guide_positions",
    // `material_ink` rows drawn on guide pages; see `merge::SYNC_TABLES`.
    "guide_ink",
];

/// Notebooks. `notebook_pictures` is the part of `assets` that notebook
/// pictures hold.
pub(crate) const NOTE_TABLES: &[&str] = &[
    "notebooks",
    "notebook_pages",
    "notebook_items",
    "notebook_pictures",
    "notebook_item_assets",
];

/// Lessons and what is made from them. `lesson_audio` is the part of `assets`
/// that narration and saved voices hold, and `slide_ink` the `material_ink`
/// rows drawn on slides.
pub(crate) const LESSON_TABLES: &[&str] = &[
    "lessons",
    "lesson_revisions",
    "lesson_revision_units",
    "lesson_progress",
    "lecture_scripts",
    "lesson_voices",
    "podcasts",
    "lesson_audio",
    "speech_segments",
    "speech_voices",
    "slide_ink",
];

pub(crate) const CROSS_DOCUMENT_FLASHCARD_TABLES: &[&str] = &[
    "document_decks",
    "document_note_topics",
    "study_target_card_origins",
    "note_annotation_sources",
];

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
    // Lesson narration and lessons themselves travel, but the knowledge units
    // they are written from do not (removed below), and the speech setting is
    // this device's choice, like its other settings.
    connection
        .execute_batch("PRAGMA secure_delete = ON;")
        .map_err(CommandError::database)?;
    crate::tutor::sanitize_for_device_transfer(connection)?;
    crate::knowledge::sanitize_for_device_transfer(connection)?;
    crate::collections::sanitize_for_device_transfer(connection)?;
    crate::study_materials::sanitize_for_device_transfer(connection)?;
    connection
        .execute_batch(
            "PRAGMA foreign_keys = OFF;
             DELETE FROM app_settings;
             DELETE FROM card_search_fts;
             DELETE FROM asset_reconciliation_events;
             DELETE FROM editor_draft_assets;
             DELETE FROM editor_drafts;
             DELETE FROM question_note_writes;
             DELETE FROM subject_filing_reviews;
             DELETE FROM review_day_rollup_built_decks;
             DELETE FROM review_day_rollups;
             DELETE FROM speech_settings;",
        )
        .map_err(CommandError::database)?;

    if !(selection.documents && selection.flashcards) {
        // A card made from a mark names the mark's document.
        connection
            .execute_batch("DELETE FROM note_annotation_sources;")
            .map_err(CommandError::database)?;
    }
    if !selection.study_guides {
        connection
            .execute_batch(
                "DELETE FROM document_study_guide_section_reads;
                 DELETE FROM document_study_guide_positions;
                 DELETE FROM material_ink WHERE document_id IS NOT NULL;",
            )
            .map_err(CommandError::database)?;
    }
    if !selection.lessons {
        connection
            .execute_batch(
                "DELETE FROM material_ink WHERE lesson_id IS NOT NULL;
                 DELETE FROM lesson_progress;
                 DELETE FROM lecture_scripts;
                 DELETE FROM podcasts;
                 DELETE FROM lesson_revision_units;
                 DELETE FROM lesson_revisions;
                 DELETE FROM lesson_voices;
                 DELETE FROM lessons;
                 CREATE TEMP TABLE sync_narration_assets AS
                     SELECT asset_id FROM speech_segments
                     UNION SELECT asset_id FROM speech_voices;
                 DELETE FROM speech_segments;
                 DELETE FROM speech_voices;
                 DELETE FROM assets WHERE id IN (SELECT asset_id FROM sync_narration_assets);
                 DROP TABLE sync_narration_assets;",
            )
            .map_err(CommandError::database)?;
    }
    if !selection.notes {
        connection
            .execute_batch(
                "CREATE TEMP TABLE sync_notebook_assets AS SELECT asset_id FROM notebook_item_assets;
                 DELETE FROM notebook_item_assets;
                 DELETE FROM notebook_items;
                 DELETE FROM notebook_pages;
                 DELETE FROM notebooks;
                 DELETE FROM assets WHERE id IN (SELECT asset_id FROM sync_notebook_assets)
                     AND id NOT IN (SELECT asset_id FROM note_assets)
                     AND id NOT IN (SELECT asset_id FROM card_template_assets)
                     AND id NOT IN (SELECT asset_id FROM question_attempt_assets);
                 DROP TABLE sync_notebook_assets;",
            )
            .map_err(CommandError::database)?;
    }

    if !selection.study_guides && selection.documents {
        connection
            .execute_batch(
                "DELETE FROM study_target_guide_overlays;
                 DELETE FROM study_target_guides;
                 DELETE FROM document_study_guide_sections;
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
                     UPDATE study_targets SET deck_id = NULL;
                     DELETE FROM study_target_card_origins;
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
                 DELETE FROM review_event_scheduler_context;
                 DELETE FROM review_events;
                 DELETE FROM study_activity_retained;
                 DELETE FROM schedule_action_events;
                 DELETE FROM schedule_action_operations;
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
                 DELETE FROM card_merges;
                 DELETE FROM cards;
                 DELETE FROM notes;
                 DELETE FROM assets
                     WHERE id NOT IN (SELECT asset_id FROM notebook_item_assets)
                     AND id NOT IN (SELECT asset_id FROM speech_segments)
                     AND id NOT IN (SELECT asset_id FROM speech_voices);
                 DELETE FROM card_template_fields;
                 DELETE FROM card_templates;
                 DELETE FROM tags;
                 DELETE FROM decks;",
            )
            .map_err(CommandError::database)?;
    } else if !selection.documents {
        // Portable source citations remain part of the flashcard graph. Their
        // source_document_id intentionally is not a foreign key, so generated
        // cards can retain provenance without transferring the live document.
        connection
            .execute_batch("UPDATE question_generation_evidence SET document_id = NULL;")
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
        // Document-to-card links leave with either side.
        connection
            .execute_batch(
                "DELETE FROM document_decks;
                 DELETE FROM document_note_topics;
                 DELETE FROM study_target_guide_overlays;
                 DELETE FROM study_target_guides;
                 DELETE FROM study_target_card_origins;
                 DELETE FROM study_target_suppressions;
                 DELETE FROM study_targets;
                 DELETE FROM document_study_ranges;
                 DELETE FROM document_study_guide_sections;
                 DELETE FROM document_study_guide_overlays;
                 DELETE FROM document_study_guide_checkpoints;
                 DELETE FROM document_study_guides;
                 DELETE FROM document_artifact_metadata;
                 DELETE FROM document_artifact_jobs;
                 DELETE FROM document_figure_readings;
                 DELETE FROM document_figure_passages;
                 DELETE FROM document_figures;
                 DELETE FROM document_figure_scans;
                 DELETE FROM document_annotations;
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
    if !selection.carries_subjects() {
        connection
            .execute_batch(
                "DELETE FROM document_collections;
                 DELETE FROM document_collection_organization;",
            )
            .map_err(CommandError::database)?;
    }

    // AI subdomains own their own SQLite schema and transfer-table identity.
    // Sync asks them for a safe transfer projection rather than duplicating
    // provider/probe schema knowledge here.
    crate::ai::generation::sanitize_for_device_transfer(connection)?;
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
#[path = "review_context_sync_tests.rs"]
mod review_context_sync_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependency_rules_are_explicit() {
        assert!(SyncSelection {
            flashcards: true,
            documents: false,
            study_guides: true,
            notes: false,
            lessons: false,
            api_settings: false,
            api_credentials: false,
        }
        .validate()
        .is_err());
        assert!(SyncSelection {
            flashcards: true,
            documents: false,
            study_guides: false,
            notes: false,
            lessons: false,
            api_settings: false,
            api_credentials: true,
        }
        .validate()
        .is_err());
    }

    #[test]
    fn subjects_travel_with_either_category_and_the_check_list_never_does() {
        let flashcards_only = SyncSelection {
            flashcards: true,
            documents: false,
            study_guides: false,
            notes: false,
            lessons: false,
            api_settings: false,
            api_credentials: false,
        };
        let documents_only = SyncSelection {
            flashcards: false,
            documents: true,
            ..flashcards_only
        };
        let settings_only = SyncSelection {
            flashcards: false,
            api_settings: true,
            ..flashcards_only
        };
        for table in SUBJECT_TABLES {
            assert!(flashcards_only.includes_table(table));
            assert!(documents_only.includes_table(table));
            assert!(!settings_only.includes_table(table));
        }
        assert!(flashcards_only.includes_table("card_merges"));
        assert!(!documents_only.includes_table("card_merges"));
        assert!(documents_only.includes_table("document_study_ranges"));
        assert!(!flashcards_only.includes_table("document_study_ranges"));
        for selection in [flashcards_only, documents_only, settings_only] {
            assert!(!selection.includes_table("subject_filing_reviews"));
        }
    }

    #[test]
    fn table_scope_keeps_typed_evidence_and_cross_graph_links_explicit() {
        let documents_only = SyncSelection {
            flashcards: false,
            documents: true,
            study_guides: true,
            notes: false,
            lessons: false,
            api_settings: false,
            api_credentials: false,
        };
        assert!(documents_only.includes_table("documents"));
        assert!(documents_only.includes_table("question_generation_reports"));
        assert!(documents_only.includes_table("document_study_guides"));
        assert!(documents_only.includes_table("study_targets"));
        assert!(documents_only.includes_table("study_target_guides"));
        assert!(!documents_only.includes_table("document_decks"));
        assert!(!documents_only.includes_table("study_target_card_origins"));
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
        assert!(all_learning.includes_table("study_target_card_origins"));
    }

    /// Rows `count` finds once `seed` ran and the snapshot was sanitized for
    /// `selection`.
    fn left_after(
        selection: SyncSelection,
        seed: impl Fn(&Connection) -> Result<(), CommandError>,
        count: &str,
    ) -> i64 {
        let database = crate::database::Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                seed(connection)?;
                sanitize_snapshot(connection, selection)?;
                connection
                    .query_row(count, [], |row| row.get(0))
                    .map_err(CommandError::database)
            })
            .unwrap()
    }

    fn without(change: impl FnOnce(&mut SyncSelection)) -> SyncSelection {
        let mut selection = SyncSelection::default();
        change(&mut selection);
        selection
    }

    fn seed_annotation(connection: &Connection) -> Result<(), CommandError> {
        let document = "d".repeat(32);
        crate::collections::test_support::seed_document(connection, &document, 1, 'a')?;
        connection
            .execute(
                "INSERT INTO document_annotations(
                     id, document_id, page_number, kind, payload_json,
                     revision, created_at, updated_at
                 ) VALUES (?1, ?2, 1, 'text', ?3, 1, 1, 1)",
                rusqlite::params![
                    "a".repeat(32),
                    document,
                    r##"{"x":1,"y":1,"width":90,"text":"Margin note","color":"#111111","fontSize":12}"##
                ],
            )
            .map_err(CommandError::database)?;
        Ok(())
    }

    #[test]
    fn document_annotations_travel_with_documents() {
        let count = "SELECT COUNT(*) FROM document_annotations";
        assert_eq!(left_after(SyncSelection::default(), seed_annotation, count), 1);
        let flashcards_only = without(|selection| {
            selection.documents = false;
            selection.study_guides = false;
        });
        assert_eq!(left_after(flashcards_only, seed_annotation, count), 0);
        assert!(SyncSelection::default().includes_table("document_annotations"));
        assert!(!flashcards_only.includes_table("document_annotations"));
    }

    fn seed_notebook(connection: &Connection) -> Result<(), CommandError> {
        connection
            .execute_batch(&format!(
                "INSERT INTO assets(id, original_filename, mime_type, byte_size,
                     relative_path, created_at)
                 VALUES ('{a}', 'sketch.png', 'image/png', 10, 'assets/images/{a}.png', 1);
                 INSERT INTO notebooks(id, subject_id, title, paper, page_size, revision, created_at, updated_at)
                 VALUES ('{n}', NULL, 'Cardiac cycle', 'grid', 'letter', 1, 1, 1);
                 INSERT INTO notebook_pages(id, notebook_id, position, paper, width, height, created_at)
                 VALUES ('{p}', '{n}', 1, 'grid', 612, 792, 1);
                 INSERT INTO notebook_items(id, page_id, kind, payload_json, revision, created_at, updated_at)
                 VALUES ('{i}', '{p}', 'line', '{{\"x1\":1,\"y1\":1,\"x2\":9,\"y2\":9,\"color\":\"#111111\",\"strokeWidth\":2}}', 1, 1, 1);
                 INSERT INTO notebook_items(id, page_id, kind, payload_json, revision, created_at, updated_at)
                 VALUES ('{j}', '{p}', 'image', '{{}}', 1, 1, 1);
                 INSERT INTO notebook_item_assets(item_id, asset_id) VALUES ('{j}', '{a}');",
                a = "f".repeat(32),
                n = "a".repeat(32),
                p = "b".repeat(32),
                i = "c".repeat(32),
                j = "e".repeat(32),
            ))
            .map_err(CommandError::database)
    }

    #[test]
    fn notebooks_travel_with_notes_even_without_flashcards() {
        let count = "SELECT (SELECT COUNT(*) FROM notebooks)
                          + (SELECT COUNT(*) FROM notebook_pages)
                          + (SELECT COUNT(*) FROM notebook_items)
                          + (SELECT COUNT(*) FROM notebook_item_assets)
                          + (SELECT COUNT(*) FROM assets)";
        assert_eq!(left_after(SyncSelection::default(), seed_notebook, count), 6);
        let notes_only = SyncSelection {
            flashcards: false,
            documents: false,
            study_guides: false,
            notes: true,
            lessons: false,
            api_settings: false,
            api_credentials: false,
        };
        assert!(notes_only.validate().is_ok());
        // The picture is kept though card images are not.
        assert_eq!(left_after(notes_only, seed_notebook, count), 6);
        let without_notes = without(|selection| selection.notes = false);
        assert_eq!(left_after(without_notes, seed_notebook, count), 0);
        for table in NOTE_TABLES {
            assert!(notes_only.includes_table(table));
            assert!(!without_notes.includes_table(table));
        }
        assert!(notes_only.includes_table("document_collections"));
    }

    fn seed_figures(connection: &Connection) -> Result<(), CommandError> {
        let document = "d".repeat(32);
        crate::collections::test_support::seed_document(connection, &document, 1, 'a')?;
        connection
            .execute_batch(&format!(
                "INSERT INTO document_figure_scans(document_id, use_images, state,
                     detector, scanned_at, updated_at)
                 VALUES ('{document}', 1, 'found', 'test', 1, 1);
                 INSERT INTO document_figures(document_id, figure_id, ordinal,
                     page_number, page_width, page_height, kind, box_left, box_top,
                     box_width, box_height, label, caption, hidden, created_at,
                     updated_at)
                 VALUES ('{document}', 'p00001-01', 0, 1, 612, 792, 'image',
                     72, 72, 200, 150, 'Figure 1', 'Figure 1. The heart.', 0, 1, 1);
                 INSERT INTO document_figure_passages(document_id, figure_id, ordinal,
                     page_number, kind, excerpt)
                 VALUES ('{document}', 'p00001-01', 0, 1, 'mention', 'See Figure 1.');
                 INSERT INTO document_figure_readings(document_id, figure_id, title,
                     shows, concepts_json, explained_json, importance,
                     worth_studying, provider, model, read_at)
                 VALUES ('{document}', 'p00001-01', 'The heart', 'Four chambers.',
                     '[]', '[1]', 4, 1, 'openrouter', 'test', 1);"
            ))
            .map_err(CommandError::database)
    }

    #[test]
    fn document_figures_travel_with_documents() {
        let count = "SELECT (SELECT COUNT(*) FROM document_figure_scans)
                          + (SELECT COUNT(*) FROM document_figures)
                          + (SELECT COUNT(*) FROM document_figure_passages)
                          + (SELECT COUNT(*) FROM document_figure_readings)";
        assert_eq!(left_after(SyncSelection::default(), seed_figures, count), 4);
        let flashcards_only = without(|selection| {
            selection.documents = false;
            selection.study_guides = false;
        });
        assert_eq!(left_after(flashcards_only, seed_figures, count), 0);
        for table in [
            "document_figure_scans",
            "document_figures",
            "document_figure_passages",
            "document_figure_readings",
        ] {
            assert!(SyncSelection::default().includes_table(table));
            assert!(!flashcards_only.includes_table(table));
        }
    }

    fn seed_card_mark_link(connection: &Connection) -> Result<(), CommandError> {
        let document = "d".repeat(32);
        crate::collections::test_support::seed_document(connection, &document, 1, 'a')?;
        connection
            .execute_batch(&format!(
                "INSERT INTO decks(id, name, created_at, updated_at)
                 VALUES ('{deck}', 'Cardiology', 1, 1);
                 INSERT INTO notes(id, deck_id, front_doc, back_doc, front_text,
                     back_text, created_at, updated_at)
                 VALUES ('{note}', '{deck}', '{{}}', '{{}}', 'Q', 'A', 1, 1);
                 INSERT INTO note_annotation_sources(note_id, document_id,
                     annotation_id, page_number, quote, created_at, updated_at)
                 VALUES ('{note}', '{document}', NULL, 1, 'Marked words', 1, 1);",
                deck = "1".repeat(32),
                note = "2".repeat(32),
            ))
            .map_err(CommandError::database)
    }

    #[test]
    fn card_mark_links_travel_with_flashcards_and_documents() {
        let count = "SELECT COUNT(*) FROM note_annotation_sources";
        assert_eq!(left_after(SyncSelection::default(), seed_card_mark_link, count), 1);
        let flashcards_only = without(|selection| {
            selection.documents = false;
            selection.study_guides = false;
        });
        assert_eq!(left_after(flashcards_only, seed_card_mark_link, count), 0);
        assert!(SyncSelection::default().includes_table("note_annotation_sources"));
        assert!(!flashcards_only.includes_table("note_annotation_sources"));
    }

    fn seed_lesson(connection: &Connection) -> Result<(), CommandError> {
        let subject = "5".repeat(32);
        let lesson = "a".repeat(32);
        connection
            .execute_batch(&format!(
                "INSERT INTO document_collections(id, name, sort_order, created_at,
                     updated_at, revision)
                 VALUES ('{subject}', 'Renal', 1000, 1, 1, 1);
                 INSERT INTO lessons(id, subject_id, sort_order, title, revision,
                     created_at, updated_at)
                 VALUES ('{lesson}', '{subject}', 1000, 'Loop of Henle', 1, 1, 1);
                 INSERT INTO lesson_revisions(lesson_id, revision_number, origin,
                     content_json, content_hash, provenance_json, created_at)
                 VALUES ('{lesson}', 1, 'authored', '{{}}', '{hash}', NULL, 1);
                 INSERT INTO lesson_revision_units(lesson_id, revision_number, unit_id,
                     unit_content_hash)
                 VALUES ('{lesson}', 1, '{unit}', '{hash}');
                 INSERT INTO lesson_progress(lesson_id, revision_number, current_step,
                     furthest_step, checks_json, revision, started_at, updated_at,
                     finished_at)
                 VALUES ('{lesson}', 1, 0, 0, '{{}}', 1, 1, 1, NULL);
                 INSERT INTO lecture_scripts(lesson_id, revision_number, script_json,
                     provenance_json, created_at)
                 VALUES ('{lesson}', 1, '{{}}', '{{}}', 1);
                 INSERT INTO lesson_voices(lesson_id, model_id, voice, created_at)
                 VALUES ('{lesson}', 'hexgrad/kokoro-82m', 'af_heart', 1);
                 INSERT INTO podcasts(id, subject_id, title, target_minutes, model_id,
                     first_voice, second_voice, script_json, provenance_json,
                     created_at)
                 VALUES ('{lesson}', '{subject}', 'Renal', 15, 'hexgrad/kokoro-82m',
                     'af_heart', 'am_michael', '{{}}', '{{}}', 1);
                 INSERT INTO material_ink(id, lesson_id, document_id, page_key, kind,
                     payload_json, revision, created_at, updated_at)
                 VALUES ('{ink}', '{lesson}', NULL, 'slide-1', 'line', '{{}}', 1, 1, 1);",
                hash = "b".repeat(64),
                unit = "c".repeat(32),
                ink = "e".repeat(32),
            ))
            .map_err(CommandError::database)
    }

    #[test]
    fn lessons_travel_with_lessons() {
        let count = "SELECT (SELECT COUNT(*) FROM lessons)
                          + (SELECT COUNT(*) FROM lesson_revisions)
                          + (SELECT COUNT(*) FROM lesson_revision_units)
                          + (SELECT COUNT(*) FROM lesson_progress)
                          + (SELECT COUNT(*) FROM lecture_scripts)
                          + (SELECT COUNT(*) FROM lesson_voices)
                          + (SELECT COUNT(*) FROM podcasts)
                          + (SELECT COUNT(*) FROM material_ink)";
        assert_eq!(left_after(SyncSelection::default(), seed_lesson, count), 8);
        let lessons_only = SyncSelection {
            flashcards: false,
            documents: false,
            study_guides: false,
            notes: false,
            lessons: true,
            api_settings: false,
            api_credentials: false,
        };
        assert_eq!(left_after(lessons_only, seed_lesson, count), 8);
        let without_lessons = without(|selection| selection.lessons = false);
        assert_eq!(left_after(without_lessons, seed_lesson, count), 0);
        for table in LESSON_TABLES {
            assert!(lessons_only.includes_table(table));
            assert!(!without_lessons.includes_table(table));
        }
    }

    fn seed_narration(connection: &Connection) -> Result<(), CommandError> {
        let asset = "d".repeat(32);
        connection
            .execute_batch(&format!(
                "INSERT INTO assets(id, original_filename, mime_type, byte_size,
                     relative_path, created_at)
                 VALUES ('{asset}', 'narration.mp3', 'audio/mpeg', 10,
                     'assets/images/{asset}.mp3', 1);
                 INSERT INTO speech_segments(id, text_sha256, engine, model_id, voice,
                     format, asset_id, characters, created_at)
                 VALUES ('{segment}', '{hash}', 'openrouter', 'hexgrad/kokoro-82m',
                     'af_heart', 'mp3', '{asset}', 12, 1);
                 INSERT INTO speech_settings(id, profile_id, model_id, voice, revision,
                     updated_at)
                 VALUES (1, NULL, 'hexgrad/kokoro-82m', 'af_heart', 1, 1);
                 INSERT INTO assets(id, original_filename, mime_type, byte_size,
                     relative_path, created_at)
                 VALUES ('{sample}', 'voice.mp3', 'audio/mpeg', 10,
                     'assets/images/{sample}.mp3', 1);
                 INSERT INTO speech_voices(id, model_id, name, state, sample_text,
                     asset_id, created_at, updated_at)
                 VALUES ('{voice}', 'fish-audio/drama-3', 'Voice 1', 'saved', 'Hello.',
                     '{sample}', 1, 1);",
                segment = "e".repeat(32),
                hash = "b".repeat(64),
                sample = "f".repeat(32),
                voice = "a".repeat(32),
            ))
            .map_err(CommandError::database)
    }

    #[test]
    fn narration_audio_travels_with_lessons_and_the_speech_setting_never_does() {
        let count = "SELECT (SELECT COUNT(*) FROM speech_segments)
                          + (SELECT COUNT(*) FROM speech_voices)
                          + (SELECT COUNT(*) FROM assets)";
        let settings = "SELECT COUNT(*) FROM speech_settings";
        let lessons_only = SyncSelection {
            flashcards: false,
            documents: false,
            study_guides: false,
            notes: false,
            lessons: true,
            api_settings: false,
            api_credentials: false,
        };
        assert_eq!(left_after(SyncSelection::default(), seed_narration, count), 4);
        assert_eq!(left_after(lessons_only, seed_narration, count), 4);
        assert_eq!(left_after(SyncSelection::default(), seed_narration, settings), 0);
        let without_lessons = without(|selection| selection.lessons = false);
        assert_eq!(left_after(without_lessons, seed_narration, count), 0);
        assert!(!SyncSelection::default().includes_table("speech_settings"));
    }

    fn seed_guide_reading(connection: &Connection) -> Result<(), CommandError> {
        let document = "d".repeat(32);
        crate::collections::test_support::seed_document(connection, &document, 1, 'a')?;
        connection
            .execute_batch(&format!(
                "INSERT INTO material_ink(id, lesson_id, document_id, page_key, kind,
                     payload_json, revision, created_at, updated_at)
                 VALUES ('{ink}', NULL, '{document}', 'overview', 'line', '{{}}', 1, 1, 1);",
                ink = "e".repeat(32),
            ))
            .map_err(CommandError::database)
    }

    #[test]
    fn ink_on_guide_pages_travels_with_study_guides() {
        let count = "SELECT COUNT(*) FROM material_ink";
        assert_eq!(left_after(SyncSelection::default(), seed_guide_reading, count), 1);
        let without_guides = without(|selection| selection.study_guides = false);
        assert_eq!(left_after(without_guides, seed_guide_reading, count), 0);
        for table in [
            "document_study_guide_section_reads",
            "document_study_guide_positions",
            "guide_ink",
        ] {
            assert!(SyncSelection::default().includes_table(table));
            assert!(!without_guides.includes_table(table));
        }
    }
}

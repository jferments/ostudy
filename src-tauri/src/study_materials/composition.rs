//! Local composition for one durable StudyTarget batch.
//!
//! Collection flashcards are never authored from CollectionSynthesis. They are
//! exact copies of already-admitted, unedited document-generated notes whose
//! canonical source locators remain inside the batch's exact SourceSetSnapshot.
//! This keeps the mature individual-document generation/admission pipeline as the
//! quality authority. Collection synthesis can organize the collection guide,
//! but it cannot lower or bypass the flashcard quality gate.

use std::collections::BTreeMap;

use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;

use crate::{
    ai::{
        document_intelligence::{
            generate_collection_study_guide_topic_section, load_document_task_context,
            CollectionStudyGuideAuthoring,
        },
        document_provider, generation, AiRuntimeState,
    },
    authoring::card_commands::interactive,
    cards::questions::ValidatedQuestion,
    collections::{self, SourceSetMemberDto, SourceSetSnapshotDto},
    database::Database,
    document_citations,
    document_domain::SourceLocator,
    document_study_guides::{
        StudyGuideComparison, StudyGuideOverview, StudyGuideSectionContent, StudyGuideTerm,
    },
    error::CommandError,
    knowledge::{
        assessment::{self, AssessmentRecord},
        collection_view::{self, CollectionViewState},
        contracts::KnowledgeKind,
    },
    scheduler::insert_new_schedule,
    task_cancellation::CancellationSignal,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

use super::{
    load_bundle_status, reuse, StudyTargetArtifactKind, StudyTargetBundleStatus, StudyTargetKind,
    StudyTargetRef,
};

const MAX_COLLECTION_SOURCE_NOTES: usize = 10_000;
const COLLECTION_GUIDE_SCHEMA_VERSION: i64 = 2;
const MAX_COLLECTION_GUIDE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CollectionGuideContent {
    pub schema_version: i64,
    pub collection_id: String,
    pub title: String,
    pub overview: StudyGuideOverview,
    pub sections: Vec<CollectionGuideSection>,
    #[serde(default)]
    pub coverage_notes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CollectionGuideSection {
    pub block_key: String,
    pub title: String,
    pub content: StudyGuideSectionContent,
    pub content_hash: String,
    pub source_document_id: Option<String>,
    pub source_document_title: Option<String>,
    pub source_guide_id: Option<String>,
    pub source_topic_id: Option<String>,
    pub source_locator: Option<SourceLocator>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ComposeStudyTargetBundleInput {
    pub operation_id: String,
}

struct BatchClaim {
    target_id: String,
    target: StudyTargetRef,
    snapshot: SourceSetSnapshotDto,
    provider_authorized: bool,
    claimed: bool,
}

#[derive(Clone)]
struct SourceBundleNote {
    document_id: String,
    note_id: String,
    note_revision: i64,
    locator: SourceLocator,
}

struct StoredSourceNote {
    note_type: String,
    content_format: String,
    content_schema_version: i64,
    front_doc: String,
    back_doc: String,
    front_text: String,
    back_text: String,
    duplicate_hash: String,
    reverse_enabled: i64,
    cloze_source: String,
    cloze_mode: String,
    question_definition: Option<String>,
    revision: i64,
}

struct CopiedNote {
    note_id: String,
    source_identity_sha256: String,
}

#[tauri::command]
pub(crate) async fn compose_study_target_bundle(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: ComposeStudyTargetBundleInput,
) -> Result<StudyTargetBundleStatus, CommandError> {
    compose(database.inner(), Some(runtime.inner()), input).await
}

async fn compose(
    database: &Database,
    runtime: Option<&AiRuntimeState>,
    input: ComposeStudyTargetBundleInput,
) -> Result<StudyTargetBundleStatus, CommandError> {
    validate_entity_id(&input.operation_id, "study target operation")?;
    let claim = claim_batch(database, &input.operation_id)?;
    if !claim.claimed {
        return status_for_claim(database, &claim);
    }

    if let Err(error) = database.with_connection(|connection| {
        collections::ensure_snapshot_current(connection, &claim.snapshot)
    }) {
        fail_pending_jobs(database, &input.operation_id, &error)?;
        finalize_batch(database, &input.operation_id)?;
        return status_for_claim(database, &claim);
    }

    for kind in pending_artifacts(database, &input.operation_id)? {
        let result = match (claim.target.kind, kind) {
            (StudyTargetKind::Document, StudyTargetArtifactKind::Flashcards) => {
                adopt_document_flashcards(database, &input.operation_id, &claim)
            }
            (StudyTargetKind::Document, StudyTargetArtifactKind::StudyGuide) => {
                adopt_document_guide(database, &input.operation_id, &claim)
            }
            (StudyTargetKind::Collection, StudyTargetArtifactKind::Flashcards) => {
                compose_collection_flashcards(database, &input.operation_id, &claim)
            }
            (StudyTargetKind::Collection, StudyTargetArtifactKind::StudyGuide) => {
                let runtime = runtime.ok_or_else(|| {
                    CommandError::new(
                        "study_target_provider_runtime_missing",
                        "Collection study-guide authoring requires the configured AI runtime.",
                    )
                })?;
                match generation::Operation::begin(
                    database,
                    Some(&input.operation_id),
                    None,
                    Some(&claim.target.id),
                    "collection_study_guide",
                ) {
                    Ok(mut diagnostics) => {
                        let result = compose_collection_guide(
                            database,
                            runtime,
                            &input.operation_id,
                            &claim,
                            &diagnostics.context(),
                        )
                        .await;
                        diagnostics.finish(result.is_ok());
                        result
                    }
                    Err(error) => Err(error),
                }
            }
        };
        if let Err(error) = result {
            record_artifact_failure(database, &input.operation_id, kind, &error)?;
        }
    }

    finalize_batch(database, &input.operation_id)?;
    status_for_claim(database, &claim)
}

fn claim_batch(database: &Database, operation_id: &str) -> Result<BatchClaim, CommandError> {
    database.with_connection(|connection| {
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let row = tx
            .query_row(
                "SELECT b.target_id,b.state,b.source_snapshot_json,b.provider_authorized,
                        t.target_kind,t.document_id,t.collection_id
                 FROM study_target_batches b
                 JOIN study_targets t ON t.id=b.target_id
                 WHERE b.operation_id=?1",
                [operation_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                    ))
                },
            )
            .optional()
            .map_err(CommandError::database)?
            .ok_or_else(|| {
                CommandError::new(
                    "study_target_batch_not_found",
                    "That study-material operation is no longer available.",
                )
            })?;
        let (
            target_id,
            state,
            snapshot_json,
            provider_authorized,
            kind,
            document_id,
            collection_id,
        ) = row;
        let target_kind = match kind.as_str() {
            "document" => StudyTargetKind::Document,
            "collection" => StudyTargetKind::Collection,
            _ => {
                return Err(CommandError::new(
                    "invalid_study_target",
                    "A saved study target has an invalid kind.",
                ))
            }
        };
        let target_source_id = match target_kind {
            StudyTargetKind::Document => document_id,
            StudyTargetKind::Collection => collection_id,
        }
        .ok_or_else(|| {
            CommandError::new(
                "invalid_study_target",
                "A saved study target lost its source identity.",
            )
        })?;
        let snapshot: SourceSetSnapshotDto =
            serde_json::from_str(&snapshot_json).map_err(CommandError::serialization)?;
        let claimed = if state == "queued" {
            tx.execute(
                "UPDATE study_target_batches
                 SET state='composing',updated_at=?1
                 WHERE operation_id=?2 AND state='queued'",
                params![unix_time_millis()?, operation_id],
            )
            .map_err(CommandError::database)?
                == 1
        } else {
            false
        };
        tx.commit().map_err(CommandError::database)?;
        Ok(BatchClaim {
            target_id,
            target: StudyTargetRef {
                kind: target_kind,
                id: target_source_id,
            },
            snapshot,
            provider_authorized: provider_authorized != 0,
            claimed,
        })
    })
}

fn status_for_claim(
    database: &Database,
    claim: &BatchClaim,
) -> Result<StudyTargetBundleStatus, CommandError> {
    load_bundle_status(database, &claim.target)?.ok_or_else(|| {
        CommandError::new(
            "study_target_not_found",
            "That study target is no longer available.",
        )
    })
}

fn pending_artifacts(
    database: &Database,
    operation_id: &str,
) -> Result<Vec<StudyTargetArtifactKind>, CommandError> {
    database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT artifact_kind FROM study_target_artifact_jobs
                 WHERE batch_id=?1 AND state='pending'
                 ORDER BY CASE artifact_kind
                     WHEN 'flashcards' THEN 0 ELSE 1 END",
            )
            .map_err(CommandError::database)?;
        let artifacts = statement
            .query_map([operation_id], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .map(|row| {
                row.map_err(CommandError::database)
                    .and_then(|value| match value.as_str() {
                        "flashcards" => Ok(StudyTargetArtifactKind::Flashcards),
                        "study_guide" => Ok(StudyTargetArtifactKind::StudyGuide),
                        _ => Err(CommandError::new(
                            "invalid_study_target",
                            "A saved study-target artifact kind was invalid.",
                        )),
                    })
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        Ok(artifacts)
    })
}

fn adopt_document_flashcards(
    database: &Database,
    operation_id: &str,
    claim: &BatchClaim,
) -> Result<(), CommandError> {
    database.with_connection(|connection| {
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        collections::ensure_snapshot_current(&tx, &claim.snapshot)?;
        let deck_id: Option<String> = tx
            .query_row(
                "SELECT d.generated_deck_id
                 FROM documents d JOIN decks deck ON deck.id=d.generated_deck_id
                 WHERE d.id=?1 AND d.generated_deck_id IS NOT NULL
                   AND deck.deleted_at IS NULL",
                [claim.target.id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let deck_id = deck_id.ok_or_else(|| {
            CommandError::new(
                "study_target_source_flashcards_missing",
                "Generate or retry this document's flashcards with the existing document pipeline before publishing its study bundle.",
            )
        })?;
        let now = unix_time_millis()?;
        tx.execute(
            "UPDATE study_targets
             SET deck_id=?1,updated_at=?2,revision=revision+1
             WHERE id=?3",
            params![deck_id, now, claim.target_id],
        )
        .map_err(CommandError::database)?;
        succeed_job(
            &tx,
            operation_id,
            StudyTargetArtifactKind::Flashcards,
            &deck_id,
            1,
            1,
            now,
        )?;
        tx.commit().map_err(CommandError::database)
    })
}

fn adopt_document_guide(
    database: &Database,
    operation_id: &str,
    claim: &BatchClaim,
) -> Result<(), CommandError> {
    database.with_connection(|connection| {
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        collections::ensure_snapshot_current(&tx, &claim.snapshot)?;
        let guide_id: Option<String> = tx
            .query_row(
                "SELECT id FROM document_study_guides
                 WHERE document_id=?1 AND state='active'",
                [claim.target.id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let guide_id = guide_id.ok_or_else(|| {
            CommandError::new(
                "study_target_source_guide_missing",
                "Generate this document's study guide with the existing document guide pipeline before publishing its study bundle.",
            )
        })?;
        let now = unix_time_millis()?;
        succeed_job(
            &tx,
            operation_id,
            StudyTargetArtifactKind::StudyGuide,
            &guide_id,
            1,
            1,
            now,
        )?;
        tx.execute(
            "UPDATE study_targets
             SET updated_at=?1,revision=revision+1 WHERE id=?2",
            params![now, claim.target_id],
        )
        .map_err(CommandError::database)?;
        tx.commit().map_err(CommandError::database)
    })
}

fn compose_collection_flashcards(
    database: &Database,
    operation_id: &str,
    claim: &BatchClaim,
) -> Result<(), CommandError> {
    database.with_connection(|connection| {
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        collections::ensure_snapshot_current(&tx, &claim.snapshot)?;
        let candidates = source_note_candidates(&tx, &claim.snapshot)?;
        if candidates.is_empty() {
            return Err(CommandError::new(
                "study_target_source_flashcards_missing",
                "This collection has no range-authorized, unedited document-generated flashcards to bundle. Generate flashcards for its source documents with the existing document pipeline first.",
            ));
        }
        let now = unix_time_millis()?;
        let deck_id = ensure_collection_target_deck(&tx, &claim.target_id, &claim.target, now)?;
        set_job_running(
            &tx,
            operation_id,
            StudyTargetArtifactKind::Flashcards,
            candidates.len(),
            now,
        )?;

        let mut completed = 0_usize;
        for candidate in &candidates {
            if origin_exists(&tx, &claim.target_id, &candidate.note_id)? {
                completed += 1;
                continue;
            }
            let source = load_source_note(&tx, candidate)?;
            if destination_has_exact_note(&tx, &deck_id, &source)? {
                completed += 1;
                continue;
            }
            let copied = copy_source_note(&tx, candidate, &source, &deck_id, now)?;
            tx.execute(
                "INSERT INTO study_target_card_origins(
                     target_id,target_note_id,source_document_id,source_note_id,
                     source_note_revision,source_identity_sha256,created_at
                 ) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    claim.target_id,
                    copied.note_id,
                    candidate.document_id,
                    candidate.note_id,
                    candidate.note_revision,
                    copied.source_identity_sha256,
                    now
                ],
            )
            .map_err(CommandError::database)?;
            attach_target_citation(
                &tx,
                &deck_id,
                &copied.note_id,
                &candidate.document_id,
                &candidate.locator,
                now,
            )?;
            completed += 1;
        }

        succeed_job(
            &tx,
            operation_id,
            StudyTargetArtifactKind::Flashcards,
            &deck_id,
            candidates.len(),
            completed,
            now,
        )?;
        tx.execute(
            "UPDATE study_targets
             SET deck_id=?1,updated_at=?2,revision=revision+1
             WHERE id=?3",
            params![deck_id, now, claim.target_id],
        )
        .map_err(CommandError::database)?;
        tx.commit().map_err(CommandError::database)
    })
}

fn source_note_candidates(
    tx: &Transaction<'_>,
    snapshot: &SourceSetSnapshotDto,
) -> Result<Vec<SourceBundleNote>, CommandError> {
    let mut candidates = Vec::new();
    for member in &snapshot.members {
        let mut statement = tx
            .prepare(
                "SELECT n.id,n.revision,link.source_locator_json
                 FROM document_note_topics link
                 JOIN notes n ON n.id=link.note_id
                 JOIN decks d ON d.id=n.deck_id
                 WHERE link.document_id=?1
                   AND link.relationship_kind='primary'
                   AND n.revision=1
                   AND n.deleted_at IS NULL
                   AND d.deleted_at IS NULL
                   AND EXISTS(
                       SELECT 1 FROM cards c
                       WHERE c.note_id=n.id AND c.deleted_at IS NULL
                   )
                 ORDER BY n.created_at,n.id",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([member.document_id.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(CommandError::database)?;
        for row in rows {
            let (note_id, note_revision, locator_json) = row.map_err(CommandError::database)?;
            let locator: SourceLocator =
                serde_json::from_str(&locator_json).map_err(CommandError::serialization)?;
            let source_unit_count = usize::try_from(member.source_unit_count).map_err(|_| {
                CommandError::new(
                    "invalid_document_source",
                    "A collection member has an invalid source-unit count.",
                )
            })?;
            locator.validate_within(source_unit_count)?;
            if locator_allowed(member, &locator) {
                candidates.push(SourceBundleNote {
                    document_id: member.document_id.clone(),
                    note_id,
                    note_revision,
                    locator,
                });
                if candidates.len() > MAX_COLLECTION_SOURCE_NOTES {
                    return Err(CommandError::new(
                        "study_target_partition_required",
                        "The collection contains too many admitted source flashcards for one bundle update. Narrow the collection ranges.",
                    ));
                }
            }
        }
    }
    candidates.sort_by(|left, right| {
        (&left.document_id, &left.note_id).cmp(&(&right.document_id, &right.note_id))
    });
    candidates.dedup_by(|left, right| left.note_id == right.note_id);
    Ok(candidates)
}

fn locator_allowed(member: &SourceSetMemberDto, locator: &SourceLocator) -> bool {
    if locator.unit_kind.as_str() != member.source_unit_kind || locator.unit_numbers.is_empty() {
        return false;
    }
    if member.ranges.is_empty() {
        return true;
    }
    locator.unit_numbers.iter().all(|unit| {
        member.ranges.iter().any(|range| {
            range.unit_kind == member.source_unit_kind
                && range.start_unit <= *unit
                && *unit <= range.end_unit
        })
    })
}

fn ensure_collection_target_deck(
    tx: &Transaction<'_>,
    target_id: &str,
    target: &StudyTargetRef,
    now: i64,
) -> Result<String, CommandError> {
    let existing: Option<String> = tx
        .query_row(
            "SELECT deck_id FROM study_targets WHERE id=?1",
            [target_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?
        .flatten();
    if let Some(deck_id) = existing.as_deref() {
        let retained = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM decks WHERE id=?1 AND deleted_at IS NULL)",
                [deck_id],
                |row| row.get::<_, i64>(0),
            )
            .map(|value| value != 0)
            .map_err(CommandError::database)?;
        if retained {
            return Ok(deck_id.to_string());
        }
        tx.execute(
            "DELETE FROM study_target_card_origins
             WHERE target_id=?1 AND target_note_id IN(
                 SELECT n.id FROM notes n
                 JOIN decks d ON d.id=n.deck_id
                 WHERE n.deleted_at IS NOT NULL OR d.deleted_at IS NOT NULL
             )",
            [target_id],
        )
        .map_err(CommandError::database)?;
    }

    let title: String = tx
        .query_row(
            "SELECT name FROM document_collections WHERE id=?1",
            [target.id.as_str()],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    let base = format!("{title} — Study Bundle");
    let name = base.chars().take(200).collect::<String>();
    let description =
        "Coherent flashcards copied from the collection's admitted document-generated material.";
    let deck_id = generate_id(tx)?;
    let sort_order: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(sort_order),-1)+1 FROM decks WHERE deleted_at IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    tx.execute(
        "INSERT INTO decks(
             id,name,description,sort_order,created_at,updated_at,revision
         ) VALUES(?1,?2,?3,?4,?5,?5,1)",
        params![deck_id, name, description, sort_order, now],
    )
    .map_err(CommandError::database)?;
    Ok(deck_id)
}

fn origin_exists(
    tx: &Transaction<'_>,
    target_id: &str,
    source_note_id: &str,
) -> Result<bool, CommandError> {
    tx.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM study_target_card_origins
             WHERE target_id=?1 AND source_note_id=?2
         )",
        params![target_id, source_note_id],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value != 0)
    .map_err(CommandError::database)
}

fn load_source_note(
    tx: &Transaction<'_>,
    candidate: &SourceBundleNote,
) -> Result<StoredSourceNote, CommandError> {
    tx.query_row(
        "SELECT note_type,content_format,content_schema_version,
                COALESCE(front_doc,''),COALESCE(back_doc,''),
                front_text,back_text,duplicate_hash,reverse_enabled,
                cloze_source,cloze_mode,question_definition,revision
         FROM notes
         WHERE id=?1 AND revision=?2 AND deleted_at IS NULL",
        params![candidate.note_id, candidate.note_revision],
        |row| {
            Ok(StoredSourceNote {
                note_type: row.get(0)?,
                content_format: row.get(1)?,
                content_schema_version: row.get(2)?,
                front_doc: row.get(3)?,
                back_doc: row.get(4)?,
                front_text: row.get(5)?,
                back_text: row.get(6)?,
                duplicate_hash: row.get(7)?,
                reverse_enabled: row.get(8)?,
                cloze_source: row.get(9)?,
                cloze_mode: row.get(10)?,
                question_definition: row.get(11)?,
                revision: row.get(12)?,
            })
        },
    )
    .map_err(CommandError::database)
}

fn destination_has_exact_note(
    tx: &Transaction<'_>,
    deck_id: &str,
    source: &StoredSourceNote,
) -> Result<bool, CommandError> {
    if source.note_type == "interactive" {
        let definition = source.question_definition.as_deref().ok_or_else(|| {
            CommandError::new(
                "study_target_source_card_invalid",
                "A generated objective question lost its validated definition.",
            )
        })?;
        let expected = ValidatedQuestion::parse(definition)?.duplicate_identity()?;
        let mut statement = tx
            .prepare(
                "SELECT question_definition FROM notes
                 WHERE deck_id=?1 AND note_type='interactive'
                   AND deleted_at IS NULL",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([deck_id], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?;
        for row in rows {
            let definition = row.map_err(CommandError::database)?;
            if ValidatedQuestion::parse(&definition)?.duplicate_identity()? == expected {
                return Ok(true);
            }
        }
        return Ok(false);
    }

    tx.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM notes
             WHERE deck_id=?1 AND deleted_at IS NULL
               AND note_type=?2 AND content_format=?3
               AND content_schema_version=?4
               AND COALESCE(front_doc,'')=?5
               AND COALESCE(back_doc,'')=?6
               AND front_text=?7 AND back_text=?8
               AND reverse_enabled=?9
               AND cloze_source=?10 AND cloze_mode=?11
         )",
        params![
            deck_id,
            source.note_type,
            source.content_format,
            source.content_schema_version,
            source.front_doc,
            source.back_doc,
            source.front_text,
            source.back_text,
            source.reverse_enabled,
            source.cloze_source,
            source.cloze_mode
        ],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value != 0)
    .map_err(CommandError::database)
}

fn copy_source_note(
    tx: &Transaction<'_>,
    candidate: &SourceBundleNote,
    source: &StoredSourceNote,
    deck_id: &str,
    now: i64,
) -> Result<CopiedNote, CommandError> {
    if source.revision != 1 {
        return Err(CommandError::new(
            "study_target_source_card_edited",
            "A source flashcard changed after document generation and was not copied into the collection bundle.",
        ));
    }
    let new_note_id = generate_id(tx)?;
    match source.note_type.as_str() {
        "basic" | "cloze" => {
            copy_recall_note(tx, &candidate.note_id, source, &new_note_id, deck_id, now)?
        }
        "interactive" => {
            let definition = source.question_definition.as_deref().ok_or_else(|| {
                CommandError::new(
                    "study_target_source_card_invalid",
                    "A generated objective question lost its validated definition.",
                )
            })?;
            let question = ValidatedQuestion::parse(definition)?;
            interactive::insert_generated(tx, &new_note_id, deck_id, &question, now)?;
            copy_question_evidence(
                tx,
                &candidate.note_id,
                &new_note_id,
                &candidate.document_id,
                now,
            )?;
        }
        _ => {
            return Err(CommandError::new(
                "study_target_source_card_invalid",
                "A source document flashcard uses an unsupported generated note type.",
            ))
        }
    }

    tx.execute(
        "INSERT INTO note_tags(note_id,tag_id)
         SELECT ?1,tag_id FROM note_tags WHERE note_id=?2",
        params![new_note_id, candidate.note_id],
    )
    .map_err(CommandError::database)?;
    if source.note_type != "interactive" {
        tx.execute(
            "INSERT INTO note_assets(note_id,asset_id,side)
             SELECT ?1,asset_id,side FROM note_assets WHERE note_id=?2",
            params![new_note_id, candidate.note_id],
        )
        .map_err(CommandError::database)?;
    }

    let identity = serde_json::to_vec(&(
        &candidate.document_id,
        &candidate.note_id,
        candidate.note_revision,
        &candidate.locator,
        &source.note_type,
        &source.front_doc,
        &source.back_doc,
        &source.front_text,
        &source.back_text,
        &source.question_definition,
    ))
    .map_err(CommandError::serialization)?;
    Ok(CopiedNote {
        note_id: new_note_id,
        source_identity_sha256: format!("{:x}", Sha256::digest(identity)),
    })
}

fn copy_recall_note(
    tx: &Transaction<'_>,
    source_note_id: &str,
    source: &StoredSourceNote,
    note_id: &str,
    deck_id: &str,
    now: i64,
) -> Result<(), CommandError> {
    if source.content_format != "tiptap-json" || source.content_schema_version != 1 {
        return Err(CommandError::new(
            "study_target_source_card_invalid",
            "A source recall card uses an unsupported content representation.",
        ));
    }
    tx.execute(
        "INSERT INTO notes(
             id,deck_id,note_type,content_format,content_schema_version,
             front_doc,back_doc,front_text,back_text,duplicate_hash,
             reverse_enabled,cloze_source,cloze_mode,
             created_at,updated_at,revision
         ) VALUES(
             ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?14,1
         )",
        params![
            note_id,
            deck_id,
            source.note_type,
            source.content_format,
            source.content_schema_version,
            source.front_doc,
            source.back_doc,
            source.front_text,
            source.back_text,
            source.duplicate_hash,
            source.reverse_enabled,
            source.cloze_source,
            source.cloze_mode,
            now
        ],
    )
    .map_err(CommandError::database)?;
    let mut statement = tx
        .prepare(
            "SELECT direction,template_ordinal FROM cards
             WHERE note_id=?1 AND deleted_at IS NULL
             ORDER BY CASE direction WHEN 'forward' THEN 0 ELSE 1 END,
                      template_ordinal,direction",
        )
        .map_err(CommandError::database)?;
    let cards = statement
        .query_map([source_note_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    if cards.is_empty() {
        return Err(CommandError::new(
            "study_target_source_card_invalid",
            "A source document note has no active study presentation.",
        ));
    }
    for (direction, ordinal) in cards {
        let card_id = generate_id(tx)?;
        tx.execute(
            "INSERT INTO cards(
                 id,note_id,deck_id,direction,template_ordinal,created_at,updated_at
             ) VALUES(?1,?2,?3,?4,?5,?6,?6)",
            params![card_id, note_id, deck_id, direction, ordinal, now],
        )
        .map_err(CommandError::database)?;
        insert_new_schedule(tx, &card_id, now)?;
    }
    Ok(())
}

fn copy_question_evidence(
    tx: &Transaction<'_>,
    source_note_id: &str,
    target_note_id: &str,
    document_id: &str,
    now: i64,
) -> Result<(), CommandError> {
    let record_json: String = tx
        .query_row(
            "SELECT record_json FROM question_generation_evidence
             WHERE note_id=?1 AND note_revision=1 AND document_id=?2",
            params![source_note_id, document_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "study_target_source_card_invalid",
                "A generated objective question no longer has matching component evidence.",
            )
        })?;
    let record: AssessmentRecord =
        serde_json::from_str(&record_json).map_err(CommandError::serialization)?;
    assessment::store::publish(tx, target_note_id, document_id, &record, now)
}

fn attach_target_citation(
    tx: &Transaction<'_>,
    deck_id: &str,
    note_id: &str,
    document_id: &str,
    locator: &SourceLocator,
    now: i64,
) -> Result<(), CommandError> {
    let citation_id = document_citations::upsert_document_snapshot(tx, document_id, now)?;
    tx.execute(
        "INSERT OR IGNORE INTO deck_source_citations(deck_id,citation_id,created_at)
         VALUES(?1,?2,?3)",
        params![deck_id, citation_id, now],
    )
    .map_err(CommandError::database)?;
    tx.execute(
        "INSERT INTO note_source_citations(
             note_id,citation_id,source_locator_json,created_at,updated_at
         ) VALUES(?1,?2,?3,?4,?4)
         ON CONFLICT(note_id) DO UPDATE SET
             citation_id=excluded.citation_id,
             source_locator_json=excluded.source_locator_json,
             updated_at=excluded.updated_at",
        params![
            note_id,
            citation_id,
            serde_json::to_string(locator).map_err(CommandError::serialization)?,
            now
        ],
    )
    .map_err(CommandError::database)?;
    Ok(())
}

async fn compose_collection_guide(
    database: &Database,
    runtime: &AiRuntimeState,
    operation_id: &str,
    claim: &BatchClaim,
    diagnostics: &generation::Context,
) -> Result<(), CommandError> {
    if !claim.provider_authorized {
        return Err(CommandError::new(
            "study_target_provider_not_authorized",
            "Collection study-guide generation was not authorized for provider work.",
        ));
    }
    let native = collection_view::refresh_native(database, &claim.target.id)?;
    if native.stale || native.synthesis.snapshot_id != claim.snapshot.snapshot_id {
        return Err(CommandError::new(
            "source_set_changed",
            "The collection changed before its study guide could be authored.",
        ));
    }
    if native.synthesis.topics.is_empty() {
        return Err(CommandError::new(
            "collection_study_guide_empty",
            "This collection has no retained interpreted topics available for a study guide.",
        ));
    }
    let native_windows =
        collection_view::guide_section_windows(database, &claim.target.id, &native.synthesis)?;
    if native_windows.is_empty() {
        return Err(CommandError::new(
            "collection_study_guide_empty",
            "This collection has no retained interpreted material available for a study guide.",
        ));
    }
    let work_total = native_windows.len().saturating_add(1);

    let mut task = load_document_task_context(database)?;
    let capabilities = document_provider::load_document_provider_capabilities(
        database,
        &task.profile_id,
        &task.provider_kind,
        &task.model_id,
    )?;
    task.apply_provider_capabilities(&capabilities);
    task.diagnostics = Some(diagnostics.clone());
    let guide_contract = reuse::guide_contract(&task)?;
    let signal = CancellationSignal::new();
    let view = collection_view::ensure_reduced(database, runtime, &task, &claim.target.id, &signal)
        .await?;
    if view.stale || view.synthesis.snapshot_id != claim.snapshot.snapshot_id {
        return Err(CommandError::new(
            "source_set_changed",
            "The collection changed while its study guide was being organized.",
        ));
    }
    let windows =
        collection_view::guide_section_windows(database, &claim.target.id, &view.synthesis)?;
    if windows.len() != native_windows.len() {
        return Err(CommandError::new(
            "source_set_changed",
            "The collection outline changed while its study guide was being organized.",
        ));
    }
    begin_artifact_job(
        database,
        operation_id,
        StudyTargetArtifactKind::StudyGuide,
        work_total,
    )?;
    update_artifact_progress(
        database,
        operation_id,
        StudyTargetArtifactKind::StudyGuide,
        1,
    )?;

    let collection_title = collection_title(database, &claim.target.id)?;
    let parallelism = crate::ai::concurrency::document_request_parallelism(&task.network_scope)
        .clamp(1, windows.len().max(1));
    crate::logging::info_target(
        "ai.document.study_guide",
        "collection_guide_execution_plan",
        "Prepared independent collection-guide section requests with bounded provider concurrency.",
        serde_json::json!({
            "sections": windows.len(),
            "parallelism": parallelism,
            "networkScope": task.network_scope.as_str(),
        }),
    );
    let mut sections = vec![None; windows.len()];
    let mut cursor = 0_usize;
    let mut completed_sections = 0_usize;
    while cursor < windows.len() {
        task.ensure_current(database)?;
        database.with_connection(|connection| {
            collections::ensure_snapshot_current(connection, &claim.snapshot)?;
            collection_view::ensure_analysis_current(
                connection,
                &claim.target.id,
                &view.analysis_hash,
            )
        })?;
        let end = cursor.saturating_add(parallelism).min(windows.len());
        let indices = (cursor..end).collect::<Vec<_>>();
        let p0 = indices.first().copied();
        let p1 = indices.get(1).copied();
        let p2 = indices.get(2).copied();
        let p3 = indices.get(3).copied();
        let (r0, r1, r2, r3) = tokio::join!(
            collection_guide_slot(
                database,
                runtime,
                &task,
                &guide_contract,
                &collection_title,
                &view,
                &windows,
                &signal,
                p0
            ),
            collection_guide_slot(
                database,
                runtime,
                &task,
                &guide_contract,
                &collection_title,
                &view,
                &windows,
                &signal,
                p1
            ),
            collection_guide_slot(
                database,
                runtime,
                &task,
                &guide_contract,
                &collection_title,
                &view,
                &windows,
                &signal,
                p2
            ),
            collection_guide_slot(
                database,
                runtime,
                &task,
                &guide_contract,
                &collection_title,
                &view,
                &windows,
                &signal,
                p3
            ),
        );
        for (index, result) in [r0, r1, r2, r3].into_iter().flatten() {
            sections[index] = Some(result?);
            completed_sections = completed_sections.saturating_add(1);
            update_artifact_progress(
                database,
                operation_id,
                StudyTargetArtifactKind::StudyGuide,
                completed_sections.saturating_add(1),
            )?;
        }
        task.ensure_current(database)?;
        database.with_connection(|connection| {
            collections::ensure_snapshot_current(connection, &claim.snapshot)?;
            collection_view::ensure_analysis_current(
                connection,
                &claim.target.id,
                &view.analysis_hash,
            )
        })?;
        cursor = end;
    }
    let sections = sections
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            CommandError::new(
                "collection_study_guide_incomplete",
                "A collection study-guide section was not completed.",
            )
        })?;

    let content = build_collection_guide_content(claim, &view, collection_title, sections)?;
    publish_collection_guide(database, operation_id, claim, &view, &content, work_total)
}

async fn collection_guide_slot(
    database: &Database,
    runtime: &AiRuntimeState,
    task: &crate::ai::document_intelligence::DocumentTaskContext,
    guide_contract: &str,
    collection_title: &str,
    view: &CollectionViewState,
    windows: &[collection_view::CollectionGuideWindow],
    signal: &CancellationSignal,
    index: Option<usize>,
) -> Option<(usize, Result<CollectionGuideSection, CommandError>)> {
    let index = index?;
    let window = windows.get(index)?;
    Some((
        index,
        author_collection_guide_window(
            database,
            runtime,
            task,
            guide_contract,
            collection_title,
            view,
            window,
            signal,
        )
        .await,
    ))
}

async fn author_collection_guide_window(
    database: &Database,
    runtime: &AiRuntimeState,
    task: &crate::ai::document_intelligence::DocumentTaskContext,
    guide_contract: &str,
    collection_title: &str,
    view: &CollectionViewState,
    window: &collection_view::CollectionGuideWindow,
    signal: &CancellationSignal,
) -> Result<CollectionGuideSection, CommandError> {
    let relationship_context = relationship_context(view, &window.topic_ids);
    let cache_key = reuse::section_key(
        task,
        &view.synthesis.collection_id,
        collection_title,
        window,
        &relationship_context,
    )?;
    let generated = if let Some(saved) = reuse::load_section(
        database,
        &view.synthesis.collection_id,
        &cache_key,
        guide_contract,
        &window.catalog,
        task.diagnostics.as_ref(),
    )? {
        saved
    } else {
        let mut section_task = task.clone();
        section_task.diagnostic_window_key = Some(cache_key.clone());
        let generated = generate_collection_study_guide_topic_section(
            runtime,
            &section_task,
            collection_title,
            CollectionStudyGuideAuthoring {
                topic_title: &window.title,
                topic_summary: &window.topic_context,
                relationship_context: &relationship_context,
                knowledge: &window.catalog,
            },
            signal,
        )
        .await?;
        section_task.ensure_current(database)?;
        database.with_connection(|connection| {
            collection_view::ensure_analysis_current(
                connection,
                &view.synthesis.collection_id,
                &view.analysis_hash,
            )
        })?;
        reuse::save_section(
            database,
            &view.synthesis.collection_id,
            &cache_key,
            guide_contract,
            &window.catalog,
            &generated,
        )?;
        generated
    };
    let mut content = generated.content;
    if content.important_terms.is_empty() {
        content.important_terms = window
            .catalog
            .units
            .iter()
            .filter(|unit| unit.content.kind == KnowledgeKind::Definition)
            .take(8)
            .map(|unit| StudyGuideTerm {
                term: unit.content.title.clone(),
                explanation: unit.content.statement.clone(),
            })
            .collect();
    }
    let content_hash = hash_text(
        &serde_json::to_string(&(&window.title, &content)).map_err(CommandError::serialization)?,
    );
    Ok(CollectionGuideSection {
        block_key: window.block_key.clone(),
        title: window.title.clone(),
        content,
        content_hash,
        source_document_id: None,
        source_document_title: None,
        source_guide_id: None,
        source_topic_id: None,
        source_locator: None,
    })
}

fn collection_title(database: &Database, collection_id: &str) -> Result<String, CommandError> {
    database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT name FROM document_collections WHERE id=?1",
                [collection_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(CommandError::database)?
            .ok_or_else(|| {
                CommandError::new(
                    "document_collection_not_found",
                    "That document collection is no longer available.",
                )
            })
    })
}

fn relationship_context(view: &CollectionViewState, topic_ids: &[String]) -> String {
    let wanted = topic_ids
        .iter()
        .map(String::as_str)
        .collect::<std::collections::HashSet<_>>();
    let mut notes = view
        .synthesis
        .relationships
        .iter()
        .filter(|relationship| {
            relationship
                .topic_ids
                .iter()
                .any(|id| wanted.contains(id.as_str()))
        })
        .map(|relationship| relationship.note.trim())
        .filter(|note| !note.is_empty())
        .map(|note| note.chars().take(240).collect::<String>())
        .take(6)
        .collect::<Vec<_>>();
    notes.dedup();
    if notes.is_empty() {
        "No additional validated cross-document relationship was retained for this topic."
            .to_string()
    } else {
        notes.join(" ")
    }
}

fn build_collection_guide_content(
    claim: &BatchClaim,
    view: &CollectionViewState,
    collection_title: String,
    mut sections: Vec<CollectionGuideSection>,
) -> Result<CollectionGuideContent, CommandError> {
    let topic_titles = view
        .synthesis
        .topics
        .iter()
        .map(|topic| (topic.id.as_str(), topic.title.as_str()))
        .collect::<BTreeMap<_, _>>();

    let key_concepts = sections
        .iter()
        .map(|section| section.title.clone())
        .take(48)
        .collect::<Vec<_>>();

    let comparisons = view
        .synthesis
        .relationships
        .iter()
        .filter_map(|relationship| {
            let mut titles = relationship
                .topic_ids
                .iter()
                .filter_map(|id| topic_titles.get(id.as_str()).copied())
                .collect::<Vec<_>>();
            titles.dedup();
            if titles.len() < 2 || relationship.note.trim().is_empty() {
                return None;
            }
            Some(StudyGuideComparison {
                subject_a: titles[0].to_string(),
                subject_b: titles[1..].join(", "),
                relationship: relationship.note.clone(),
            })
        })
        .take(16)
        .collect::<Vec<_>>();
    if !comparisons.is_empty() {
        let content = StudyGuideSectionContent {
            overview: "These relationships connect concepts that appear across the selected documents. Use them to compare mechanisms, prerequisites, contrasts, or conflicts without collapsing the underlying source distinctions.".to_string(),
            comparisons,
            ..Default::default()
        };
        sections.push(collection_semantic_section(
            "connections",
            "Connections across the collection",
            content,
        )?);
    }

    let mut coverage_notes = view
        .synthesis
        .gaps
        .iter()
        .map(|gap| gap.detail.trim().to_string())
        .filter(|detail| !detail.is_empty())
        .collect::<Vec<_>>();
    coverage_notes.sort();
    coverage_notes.dedup();

    if sections.is_empty() {
        return Err(CommandError::new(
            "collection_study_guide_empty",
            "No readable study-guide sections could be authored from the retained collection knowledge.",
        ));
    }
    let generated_narrative = sections
        .iter()
        .take(2)
        .map(|section| section.content.overview.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let summary = if view.reduced && !view.synthesis.summary.trim().is_empty() {
        view.synthesis.summary.clone()
    } else if !generated_narrative.is_empty() {
        generated_narrative.chars().take(1_200).collect()
    } else {
        format!(
            "This study guide organizes the retained knowledge from {} selected document{} into connected collection-level topics.",
            claim.snapshot.members.len(),
            if claim.snapshot.members.len() == 1 { "" } else { "s" },
        )
    };
    let learning_objectives = view
        .synthesis
        .learning_objectives
        .iter()
        .filter(|objective| !objective.trim().to_ascii_lowercase().starts_with("review "))
        .cloned()
        .collect();
    let content = CollectionGuideContent {
        schema_version: COLLECTION_GUIDE_SCHEMA_VERSION,
        collection_id: claim.target.id.clone(),
        title: collection_title,
        overview: StudyGuideOverview {
            summary,
            learning_objectives,
            key_concepts,
        },
        sections,
        // Gaps remain durable internal metadata for honest coverage accounting.
        // They are deliberately not promoted into learner-visible guide sections.
        coverage_notes,
    };
    let serialized = serde_json::to_vec(&content).map_err(CommandError::serialization)?;
    if serialized.len() > MAX_COLLECTION_GUIDE_BYTES {
        return Err(CommandError::new(
            "study_target_partition_required",
            "The collection study guide exceeds the bounded local guide size. Narrow the collection ranges.",
        ));
    }
    Ok(content)
}

fn collection_semantic_section(
    block_key: &str,
    title: &str,
    content: StudyGuideSectionContent,
) -> Result<CollectionGuideSection, CommandError> {
    let content_hash =
        hash_text(&serde_json::to_string(&(title, &content)).map_err(CommandError::serialization)?);
    Ok(CollectionGuideSection {
        block_key: block_key.to_string(),
        title: title.to_string(),
        content,
        content_hash,
        source_document_id: None,
        source_document_title: None,
        source_guide_id: None,
        source_topic_id: None,
        source_locator: None,
    })
}

fn publish_collection_guide(
    database: &Database,
    operation_id: &str,
    claim: &BatchClaim,
    view: &CollectionViewState,
    content: &CollectionGuideContent,
    work_total: usize,
) -> Result<(), CommandError> {
    let content_json = serde_json::to_string(content).map_err(CommandError::serialization)?;
    let content_hash = hash_text(&content_json);
    database.with_connection(|connection| {
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        collections::ensure_snapshot_current(&tx, &claim.snapshot)?;
        collection_view::ensure_analysis_current(&tx, &claim.target.id, &view.analysis_hash)?;
        if view.synthesis.snapshot_id != claim.snapshot.snapshot_id {
            return Err(CommandError::new(
                "source_set_changed",
                "The collection changed before its study guide could be committed.",
            ));
        }
        let now = unix_time_millis()?;
        let existing: Option<(String, String)> = tx
            .query_row(
                "SELECT id,content_hash FROM study_target_guides
                 WHERE target_id=?1 AND state='active'",
                [claim.target_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(CommandError::database)?;
        if let Some((guide_id, old_hash)) = existing.as_ref() {
            if *old_hash == content_hash {
                succeed_job(
                    &tx,
                    operation_id,
                    StudyTargetArtifactKind::StudyGuide,
                    guide_id,
                    work_total,
                    work_total,
                    now,
                )?;
                tx.commit().map_err(CommandError::database)?;
                return Ok(());
            }
        }

        let revision: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(revision),0)+1
                 FROM study_target_guides WHERE target_id=?1",
                [claim.target_id.as_str()],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        let guide_id = generate_id(&tx)?;
        if let Some((old_id, _)) = existing.as_ref() {
            tx.execute(
                "UPDATE study_target_guides
                 SET state='superseded',updated_at=?1
                 WHERE id=?2 AND state='active'",
                params![now, old_id],
            )
            .map_err(CommandError::database)?;
        }
        tx.execute(
            "INSERT INTO study_target_guides(
                 id,target_id,batch_id,revision,source_snapshot_id,
                 synthesis_analysis_hash,content_json,content_hash,state,
                 created_at,updated_at,published_at
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'active',?9,?9,?9)",
            params![
                guide_id,
                claim.target_id,
                operation_id,
                revision,
                claim.snapshot.snapshot_id,
                view.analysis_hash,
                content_json,
                content_hash,
                now
            ],
        )
        .map_err(CommandError::database)?;
        if let Some((old_id, _)) = existing.as_ref() {
            reconcile_collection_overlays(&tx, old_id, &guide_id, content, now)?;
        }
        succeed_job(
            &tx,
            operation_id,
            StudyTargetArtifactKind::StudyGuide,
            &guide_id,
            work_total,
            work_total,
            now,
        )?;
        tx.execute(
            "UPDATE study_targets
             SET updated_at=?1,revision=revision+1 WHERE id=?2",
            params![now, claim.target_id],
        )
        .map_err(CommandError::database)?;
        tx.commit().map_err(CommandError::database)
    })
}

fn reconcile_collection_overlays(
    tx: &Transaction<'_>,
    old_guide_id: &str,
    new_guide_id: &str,
    content: &CollectionGuideContent,
    now: i64,
) -> Result<(), CommandError> {
    let hashes = collection_block_hashes(content)?;
    let mut statement = tx
        .prepare(
            "SELECT id,block_key,overlay_kind,base_content_hash
             FROM study_target_guide_overlays
             WHERE guide_id=?1 AND state='active'
             ORDER BY id",
        )
        .map_err(CommandError::database)?;
    let overlays = statement
        .query_map([old_guide_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    for (id, block_key, kind, base_hash) in overlays {
        let content_bound = matches!(
            kind.as_str(),
            "title_override" | "body_override" | "highlight"
        );
        let compatible = !content_bound
            || block_key.as_ref().and_then(|key| hashes.get(key)) == base_hash.as_ref();
        tx.execute(
            "UPDATE study_target_guide_overlays
             SET guide_id=?1,state=?2,updated_at=?3,revision=revision+1
             WHERE id=?4 AND guide_id=?5 AND state='active'",
            params![
                new_guide_id,
                if compatible { "active" } else { "conflicted" },
                now,
                id,
                old_guide_id
            ],
        )
        .map_err(CommandError::database)?;
    }
    Ok(())
}

fn collection_block_hashes(
    content: &CollectionGuideContent,
) -> Result<BTreeMap<String, String>, CommandError> {
    let mut result = BTreeMap::new();
    let overview = serde_json::to_string(&content.overview).map_err(CommandError::serialization)?;
    result.insert("overview".to_string(), hash_text(&overview));
    for section in &content.sections {
        result.insert(section.block_key.clone(), section.content_hash.clone());
    }
    Ok(result)
}

fn begin_artifact_job(
    database: &Database,
    operation_id: &str,
    kind: StudyTargetArtifactKind,
    total: usize,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        set_job_running(&tx, operation_id, kind, total, now)?;
        tx.commit().map_err(CommandError::database)
    })
}

fn update_artifact_progress(
    database: &Database,
    operation_id: &str,
    kind: StudyTargetArtifactKind,
    completed: usize,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    let changed = database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE study_target_artifact_jobs
                 SET completed_units=?1,updated_at=?2
                 WHERE batch_id=?3 AND artifact_kind=?4 AND state='running'
                   AND total_units>=?1 AND completed_units<=?1",
                params![to_i64(completed)?, now, operation_id, kind.as_db()],
            )
            .map_err(CommandError::database)
    })?;
    if changed != 1 {
        return Err(CommandError::new(
            "study_target_job_changed",
            "The study-material progress record changed while collection authoring was running.",
        ));
    }
    Ok(())
}

fn set_job_running(
    tx: &Transaction<'_>,
    operation_id: &str,
    kind: StudyTargetArtifactKind,
    total: usize,
    now: i64,
) -> Result<(), CommandError> {
    let total = to_i64(total)?;
    let changed = tx
        .execute(
            "UPDATE study_target_artifact_jobs
             SET state='running',total_units=?1,completed_units=0,
                 safe_error_code='',safe_error_message='',updated_at=?2
             WHERE batch_id=?3 AND artifact_kind=?4 AND state='pending'",
            params![total, now, operation_id, kind.as_db()],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(CommandError::new(
            "study_target_job_changed",
            "The study-material artifact job changed before composition began.",
        ));
    }
    Ok(())
}

fn succeed_job(
    tx: &Transaction<'_>,
    operation_id: &str,
    kind: StudyTargetArtifactKind,
    artifact_id: &str,
    total: usize,
    completed: usize,
    now: i64,
) -> Result<(), CommandError> {
    let changed = tx
        .execute(
            "UPDATE study_target_artifact_jobs
             SET state='succeeded',artifact_id=?1,total_units=?2,completed_units=?3,
                 safe_error_code='',safe_error_message='',
                 updated_at=?4,completed_at=?4
             WHERE batch_id=?5 AND artifact_kind=?6
               AND state IN('pending','running','committing')",
            params![
                artifact_id,
                to_i64(total)?,
                to_i64(completed)?,
                now,
                operation_id,
                kind.as_db()
            ],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(CommandError::new(
            "study_target_job_changed",
            "The study-material artifact job changed before publication completed.",
        ));
    }
    Ok(())
}

fn record_artifact_failure(
    database: &Database,
    operation_id: &str,
    kind: StudyTargetArtifactKind,
    error: &CommandError,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    let code = error.code.chars().take(80).collect::<String>();
    let message = error.message.chars().take(1000).collect::<String>();
    database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE study_target_artifact_jobs
                 SET state='failed',safe_error_code=?1,safe_error_message=?2,
                     updated_at=?3,completed_at=?3
                 WHERE batch_id=?4 AND artifact_kind=?5
                   AND state IN('pending','running','committing')",
                params![code, message, now, operation_id, kind.as_db()],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

fn fail_pending_jobs(
    database: &Database,
    operation_id: &str,
    error: &CommandError,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    let code = error.code.chars().take(80).collect::<String>();
    let message = error.message.chars().take(1000).collect::<String>();
    database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE study_target_artifact_jobs
                 SET state='failed',safe_error_code=?1,safe_error_message=?2,
                     updated_at=?3,completed_at=?3
                 WHERE batch_id=?4 AND state='pending'",
                params![code, message, now, operation_id],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

fn finalize_batch(database: &Database, operation_id: &str) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT state,safe_error_code,safe_error_message
                 FROM study_target_artifact_jobs
                 WHERE batch_id=?1 ORDER BY artifact_kind",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([operation_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        let success_count = rows
            .iter()
            .filter(|(state, _, _)| matches!(state.as_str(), "succeeded" | "suppressed"))
            .count();
        let failure_count = rows
            .iter()
            .filter(|(state, _, _)| matches!(state.as_str(), "failed" | "canceled" | "interrupted"))
            .count();
        let state = if failure_count == 0 {
            "complete"
        } else if success_count > 0 {
            "partial"
        } else {
            "failed"
        };
        let first_error = rows
            .iter()
            .find(|(row_state, _, _)| {
                matches!(row_state.as_str(), "failed" | "canceled" | "interrupted")
            })
            .map(|(_, code, message)| (code.as_str(), message.as_str()))
            .unwrap_or(("", ""));
        connection
            .execute(
                "UPDATE study_target_batches
                 SET state=?1,safe_error_code=?2,safe_error_message=?3,
                     updated_at=?4,completed_at=?4
                 WHERE operation_id=?5 AND state='composing'",
                params![state, first_error.0, first_error.1, now, operation_id],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

fn hash_text(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn to_i64(value: usize) -> Result<i64, CommandError> {
    i64::try_from(value).map_err(|_| {
        CommandError::new(
            "study_target_count_invalid",
            "The study-material item count exceeds SQLite's supported range.",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cards::validate_content,
        collections::ResolvedSourceRangeDto,
        decks::{create_deck_in_connection, CreateDeckInput},
        document_domain::SourceUnitKind,
    };
    use serde_json::json;

    fn locator(units: Vec<i64>) -> SourceLocator {
        SourceLocator::new(SourceUnitKind::Page, units, None).unwrap()
    }

    fn member(ranges: Vec<(i64, i64)>) -> SourceSetMemberDto {
        SourceSetMemberDto {
            document_id: "d".repeat(32),
            source_sha256: "e".repeat(64),
            document_revision: 1,
            source_format: "pdf".to_string(),
            source_unit_kind: "page".to_string(),
            source_unit_count: 10,
            ranges: ranges
                .into_iter()
                .map(|(start_unit, end_unit)| ResolvedSourceRangeDto {
                    unit_kind: "page".to_string(),
                    start_unit,
                    end_unit,
                })
                .collect(),
            collection_references: Vec::new(),
        }
    }

    #[test]
    fn collection_flashcard_admission_requires_every_source_unit_inside_snapshot_ranges() {
        let bounded = member(vec![(2, 4), (7, 8)]);
        assert!(locator_allowed(&bounded, &locator(vec![2, 4, 7])));
        assert!(!locator_allowed(&bounded, &locator(vec![2, 5])));
        assert!(!locator_allowed(
            &bounded,
            &SourceLocator::new(SourceUnitKind::Section, vec![2], None).unwrap()
        ));
        let whole = member(Vec::new());
        assert!(locator_allowed(&whole, &locator(vec![1, 10])));
    }

    fn rich_doc(text: &str) -> serde_json::Value {
        json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{"type": "text", "text": text}]
            }]
        })
    }

    struct BundleFixture {
        database: Database,
        document_id: String,
        collection_id: String,
        membership_id: String,
        source_deck_id: String,
        topic_id: String,
        first_source_note_id: String,
    }

    struct SourceNoteSpec<'a> {
        note_id: String,
        card_id: String,
        question: &'a str,
        answer: &'a str,
        unit: i64,
        created_at: i64,
    }

    fn insert_source_note(
        connection: &mut rusqlite::Connection,
        deck_id: &str,
        document_id: &str,
        topic_id: &str,
        spec: SourceNoteSpec<'_>,
    ) -> Result<(), CommandError> {
        let SourceNoteSpec {
            note_id,
            card_id,
            question,
            answer,
            unit,
            created_at,
        } = spec;
        let content = validate_content(
            rich_doc(question),
            rich_doc(answer),
            question.to_string(),
            answer.to_string(),
        )?;
        connection
            .execute(
                "INSERT INTO notes(
                     id,deck_id,note_type,content_format,content_schema_version,
                     front_doc,back_doc,front_text,back_text,duplicate_hash,
                     created_at,updated_at,revision
                 ) VALUES(?1,?2,'basic','tiptap-json',1,?3,?4,?5,?6,?7,?8,?8,1)",
                params![
                    note_id,
                    deck_id,
                    content.front_doc_json,
                    content.back_doc_json,
                    content.front_text,
                    content.back_text,
                    format!("fixture-{note_id}"),
                    created_at
                ],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO cards(
                     id,note_id,deck_id,direction,template_ordinal,created_at,updated_at
                 ) VALUES(?1,?2,?3,'forward',0,?4,?4)",
                params![card_id, note_id, deck_id, created_at],
            )
            .map_err(CommandError::database)?;
        let locator = SourceLocator::new(SourceUnitKind::Page, vec![unit], None)?;
        connection
            .execute(
                "INSERT INTO document_note_topics(
                     document_id,note_id,topic_id,relationship_kind,
                     source_locator_json,evidence_hash,generation_job_id,created_at
                 ) VALUES(?1,?2,?3,'primary',?4,?5,NULL,?6)",
                params![
                    document_id,
                    note_id,
                    topic_id,
                    serde_json::to_string(&locator).map_err(CommandError::serialization)?,
                    "9".repeat(64),
                    created_at
                ],
            )
            .map_err(CommandError::database)?;
        Ok(())
    }

    fn bundle_fixture() -> BundleFixture {
        let database = Database::open_in_memory().expect("database should open");
        let document_id = "d".repeat(32);
        let collection_id = "c".repeat(32);
        let membership_id = "b".repeat(32);
        let topic_id = "3".repeat(32);
        let first_source_note_id = "1".repeat(32);
        let first_source_card_id = "2".repeat(32);
        let source_hash = "e".repeat(64);
        let source_relative = format!("documents/{document_id}/source.pdf");
        let source_deck_id = database
            .with_connection(|connection| {
                let deck = create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: "Authoritative document deck".to_string(),
                        description: Some("Existing document pipeline output".to_string()),
                    },
                )?;
                connection
                    .execute(
                        "INSERT INTO documents(
                             id,title,original_filename,mime_type,byte_size,sha256,
                             source_relative_path,lifecycle_state,page_count,
                             extracted_page_count,rendered_page_count,created_at,updated_at,
                             revision,source_format,source_unit_kind,source_unit_count,
                             extracted_source_unit_count,source_media_type
                         ) VALUES(
                             ?1,'Physiology','physiology.pdf','application/pdf',10,?2,
                             ?3,'ready',2,2,2,1,1,1,'pdf','page',2,2,'application/pdf'
                         )",
                        params![document_id, source_hash, source_relative],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO document_source_files(
                             document_id,file_ordinal,original_filename,media_type,
                             relative_path,byte_size,sha256,created_at,updated_at
                         ) VALUES(?1,1,'physiology.pdf','application/pdf',?2,10,?3,1,1)",
                        params![document_id, source_relative, source_hash],
                    )
                    .map_err(CommandError::database)?;
                for unit in 1_i64..=2 {
                    let locator = SourceLocator::new(SourceUnitKind::Page, vec![unit], None)?;
                    connection
                        .execute(
                            "INSERT INTO document_source_units(
                                 document_id,unit_number,unit_kind,label,title,plain_text,
                                 text_sha256,source_file_ordinal,locator_json,
                                 extraction_confidence,created_at,updated_at
                             ) VALUES(?1,?2,'page',?3,NULL,?4,?5,1,?6,1.0,1,1)",
                            params![
                                document_id,
                                unit,
                                format!("Page {unit}"),
                                format!("Source unit {unit}"),
                                format!("{:064x}", unit),
                                serde_json::to_string(&locator)
                                    .map_err(CommandError::serialization)?
                            ],
                        )
                        .map_err(CommandError::database)?;
                }
                connection
                    .execute(
                        "INSERT INTO document_learning_topics(id,document_id,anchor_hash,created_at)
                         VALUES(?1,?2,?3,1)",
                        params![topic_id, document_id, "8".repeat(64)],
                    )
                    .map_err(CommandError::database)?;
                insert_source_note(
                    connection,
                    &deck.id,
                    &document_id,
                    &topic_id,
                    SourceNoteSpec {
                        note_id: first_source_note_id.clone(),
                        card_id: first_source_card_id.clone(),
                        question: "Question one?",
                        answer: "Answer one.",
                        unit: 1,
                        created_at: 10,
                    },
                )?;
                connection
                    .execute(
                        "INSERT INTO document_collections(
                             id,parent_id,name,sort_order,archived_at,created_at,updated_at,revision
                         ) VALUES(?1,NULL,'Week 1',1000,NULL,1,1,1)",
                        [collection_id.as_str()],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO document_collection_memberships(
                             id,collection_id,document_id,sort_order,created_at,updated_at,revision
                         ) VALUES(?1,?2,?3,1000,1,1,1)",
                        params![membership_id, collection_id, document_id],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "UPDATE document_collection_organization
                         SET revision=2,updated_at=1 WHERE singleton_id=1",
                        [],
                    )
                    .map_err(CommandError::database)?;
                Ok(deck.id)
            })
            .expect("fixture should seed");

        BundleFixture {
            database,
            document_id,
            collection_id,
            membership_id,
            source_deck_id,
            topic_id,
            first_source_note_id,
        }
    }

    fn begin_flashcard_bundle(
        fixture: &BundleFixture,
        operation_id: &str,
    ) -> StudyTargetBundleStatus {
        super::super::begin_bundle(
            &fixture.database,
            super::super::BeginStudyTargetBundleInput {
                operation_id: operation_id.to_string(),
                target: StudyTargetRef {
                    kind: StudyTargetKind::Collection,
                    id: fixture.collection_id.clone(),
                },
                output_policy: super::super::StudyTargetOutputPolicy {
                    flashcards: true,
                    study_guide: false,
                },
                recreate_artifacts: Vec::new(),
            },
        )
        .expect("bundle should begin")
    }

    fn compose_flashcard_bundle(
        fixture: &BundleFixture,
        operation_id: &str,
    ) -> StudyTargetBundleStatus {
        tauri::async_runtime::block_on(compose(
            &fixture.database,
            None,
            ComposeStudyTargetBundleInput {
                operation_id: operation_id.to_string(),
            },
        ))
        .expect("bundle should compose")
    }

    #[test]
    fn collection_bundle_replay_is_exactly_once_and_updates_preserve_learner_state() {
        let fixture = bundle_fixture();
        let first_operation = "4".repeat(32);
        begin_flashcard_bundle(&fixture, &first_operation);
        let first = compose_flashcard_bundle(&fixture, &first_operation);
        assert_eq!(first.latest_batch_state.as_deref(), Some("complete"));
        let deck_id = first.deck_id.expect("collection deck");
        assert_ne!(deck_id, fixture.source_deck_id);

        let (target_note_id, target_card_id, schedule_due): (String, String, i64) = fixture
            .database
            .with_connection(|connection| {
                let target_note_id: String = connection
                    .query_row(
                        "SELECT target_note_id FROM study_target_card_origins
                         WHERE source_note_id=?1",
                        [fixture.first_source_note_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                let target_card_id: String = connection
                    .query_row(
                        "SELECT id FROM cards WHERE note_id=?1 AND deleted_at IS NULL",
                        [target_note_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                let schedule_due: i64 = connection
                    .query_row(
                        "SELECT due_at FROM card_schedules WHERE card_id=?1",
                        [target_card_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                Ok((target_note_id, target_card_id, schedule_due))
            })
            .expect("target identities should read");

        let replay = compose_flashcard_bundle(&fixture, &first_operation);
        assert_eq!(replay.deck_id.as_deref(), Some(deck_id.as_str()));
        fixture
            .database
            .with_connection(|connection| {
                let counts: (i64, i64) = connection
                    .query_row(
                        "SELECT
                             (SELECT COUNT(*) FROM study_target_card_origins
                              WHERE target_id=?1),
                             (SELECT COUNT(*) FROM notes
                              WHERE deck_id=?2 AND deleted_at IS NULL)",
                        params![replay.target_id, deck_id],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(counts, (1, 1));
                Ok(())
            })
            .expect("replay should be exactly once");

        fixture
            .database
            .with_connection(|connection| {
                connection
                    .execute(
                        "UPDATE notes
                         SET front_text='Learner edit',revision=revision+1,updated_at=20
                         WHERE id=?1",
                        [target_note_id.as_str()],
                    )
                    .map_err(CommandError::database)?;
                insert_source_note(
                    connection,
                    &fixture.source_deck_id,
                    &fixture.document_id,
                    &fixture.topic_id,
                    SourceNoteSpec {
                        note_id: "5".repeat(32),
                        card_id: "6".repeat(32),
                        question: "Question two?",
                        answer: "Answer two.",
                        unit: 2,
                        created_at: 20,
                    },
                )?;
                insert_source_note(
                    connection,
                    &fixture.source_deck_id,
                    &fixture.document_id,
                    &fixture.topic_id,
                    SourceNoteSpec {
                        note_id: "7".repeat(32),
                        card_id: "8".repeat(32),
                        question: "Edited source question?",
                        answer: "Edited source answer.",
                        unit: 2,
                        created_at: 21,
                    },
                )?;
                connection
                    .execute("UPDATE notes SET revision=2 WHERE id=?1", ["7".repeat(32)])
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .expect("update fixture should seed");

        let second_operation = "a".repeat(32);
        begin_flashcard_bundle(&fixture, &second_operation);
        let second = compose_flashcard_bundle(&fixture, &second_operation);
        assert_eq!(second.latest_batch_state.as_deref(), Some("complete"));
        assert_eq!(second.deck_id.as_deref(), Some(deck_id.as_str()));

        fixture
            .database
            .with_connection(|connection| {
                let state: (String, i64, String, i64, i64, i64) = connection
                    .query_row(
                        "SELECT n.front_text,n.revision,c.id,s.due_at,
                                (SELECT COUNT(*) FROM notes
                                 WHERE deck_id=?2 AND deleted_at IS NULL),
                                (SELECT COUNT(*) FROM study_target_card_origins
                                 WHERE target_id=?3)
                         FROM notes n
                         JOIN cards c ON c.note_id=n.id AND c.deleted_at IS NULL
                         JOIN card_schedules s ON s.card_id=c.id
                         WHERE n.id=?1",
                        params![target_note_id, deck_id, second.target_id],
                        |row| {
                            Ok((
                                row.get(0)?,
                                row.get(1)?,
                                row.get(2)?,
                                row.get(3)?,
                                row.get(4)?,
                                row.get(5)?,
                            ))
                        },
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(state.0, "Learner edit");
                assert_eq!(state.1, 2);
                assert_eq!(state.2, target_card_id);
                assert_eq!(state.3, schedule_due);
                assert_eq!(state.4, 2);
                assert_eq!(state.5, 2);
                Ok(())
            })
            .expect("learner state and append-only update should survive");
    }

    #[test]
    fn removing_collection_source_never_deletes_successful_bundle_cards() {
        let fixture = bundle_fixture();
        let first_operation = "4".repeat(32);
        begin_flashcard_bundle(&fixture, &first_operation);
        let first = compose_flashcard_bundle(&fixture, &first_operation);
        let deck_id = first.deck_id.expect("collection deck");

        fixture
            .database
            .with_connection(|connection| {
                connection
                    .execute(
                        "DELETE FROM document_collection_memberships WHERE id=?1",
                        [fixture.membership_id.as_str()],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "UPDATE document_collection_organization
                         SET revision=revision+1,updated_at=30 WHERE singleton_id=1",
                        [],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .expect("source removal should commit");

        let status = super::super::load_bundle_status(
            &fixture.database,
            &StudyTargetRef {
                kind: StudyTargetKind::Collection,
                id: fixture.collection_id.clone(),
            },
        )
        .expect("status should load")
        .expect("target should remain");
        assert!(status.source_changed);

        let removal_operation = "a".repeat(32);
        begin_flashcard_bundle(&fixture, &removal_operation);
        let failed = compose_flashcard_bundle(&fixture, &removal_operation);
        assert_eq!(failed.latest_batch_state.as_deref(), Some("failed"));
        assert_eq!(failed.flashcards.state, "failed");
        assert_eq!(failed.deck_id.as_deref(), Some(deck_id.as_str()));

        fixture
            .database
            .with_connection(|connection| {
                let active: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM notes
                         WHERE deck_id=?1 AND deleted_at IS NULL",
                        [deck_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(active, 1);
                Ok(())
            })
            .expect("successful bundle cards should survive source removal");
    }

    #[test]
    fn failed_sibling_keeps_successful_collection_deck_and_batch_is_partial() {
        let fixture = bundle_fixture();
        let operation_id = "4".repeat(32);
        super::super::begin_bundle(
            &fixture.database,
            super::super::BeginStudyTargetBundleInput {
                operation_id: operation_id.clone(),
                target: StudyTargetRef {
                    kind: StudyTargetKind::Collection,
                    id: fixture.collection_id.clone(),
                },
                output_policy: super::super::StudyTargetOutputPolicy {
                    flashcards: true,
                    study_guide: true,
                },
                recreate_artifacts: Vec::new(),
            },
        )
        .expect("bundle should begin");
        let claim = claim_batch(&fixture.database, &operation_id).expect("batch should claim");
        assert!(claim.claimed);
        compose_collection_flashcards(&fixture.database, &operation_id, &claim)
            .expect("flashcards should publish");
        record_artifact_failure(
            &fixture.database,
            &operation_id,
            StudyTargetArtifactKind::StudyGuide,
            &CommandError::new(
                "fixture_guide_failure",
                "The guide sibling failed after flashcards committed.",
            ),
        )
        .expect("guide failure should record");
        finalize_batch(&fixture.database, &operation_id).expect("batch should finalize");

        let status = status_for_claim(&fixture.database, &claim).expect("status should load");
        assert_eq!(status.latest_batch_state.as_deref(), Some("partial"));
        assert_eq!(status.flashcards.state, "succeeded");
        assert_eq!(status.study_guide.state, "failed");
        let deck_id = status.deck_id.expect("successful deck should remain");
        fixture
            .database
            .with_connection(|connection| {
                let active_notes: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM notes
                         WHERE deck_id=?1 AND deleted_at IS NULL",
                        [deck_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(active_notes, 1);
                Ok(())
            })
            .expect("successful sibling should remain intact");
    }

    #[test]
    fn deleted_bundle_deck_stays_suppressed_until_explicit_recreation() {
        let fixture = bundle_fixture();
        let first_operation = "4".repeat(32);
        begin_flashcard_bundle(&fixture, &first_operation);
        let first = compose_flashcard_bundle(&fixture, &first_operation);
        let deleted_deck = first.deck_id.expect("initial collection deck");

        fixture
            .database
            .with_connection(|connection| {
                crate::deletion::delete_deck(connection, &deleted_deck)?;
                Ok(())
            })
            .expect("ordinary deck deletion should record target suppression");

        let suppressed_operation = "5".repeat(32);
        begin_flashcard_bundle(&fixture, &suppressed_operation);
        let suppressed = compose_flashcard_bundle(&fixture, &suppressed_operation);
        assert_eq!(suppressed.latest_batch_state.as_deref(), Some("complete"));
        assert_eq!(suppressed.flashcards.state, "suppressed");
        assert!(suppressed.deck_id.is_none());

        let recreate_operation = "6".repeat(32);
        super::super::begin_bundle(
            &fixture.database,
            super::super::BeginStudyTargetBundleInput {
                operation_id: recreate_operation.clone(),
                target: StudyTargetRef {
                    kind: StudyTargetKind::Collection,
                    id: fixture.collection_id.clone(),
                },
                output_policy: super::super::StudyTargetOutputPolicy {
                    flashcards: true,
                    study_guide: false,
                },
                recreate_artifacts: vec![StudyTargetArtifactKind::Flashcards],
            },
        )
        .expect("explicit recreation should begin");
        let recreated = compose_flashcard_bundle(&fixture, &recreate_operation);
        assert_eq!(recreated.latest_batch_state.as_deref(), Some("complete"));
        assert_eq!(recreated.flashcards.state, "succeeded");
        let recreated_deck = recreated.deck_id.expect("recreated deck");
        assert_ne!(recreated_deck, deleted_deck);
    }

    #[test]
    fn collection_guide_block_hashes_change_only_with_their_semantic_block() {
        let mut content = CollectionGuideContent {
            schema_version: COLLECTION_GUIDE_SCHEMA_VERSION,
            collection_id: "c".repeat(32),
            title: "Week 1".to_string(),
            overview: StudyGuideOverview {
                summary: "Overview".to_string(),
                learning_objectives: vec!["Objective".to_string()],
                key_concepts: vec!["Topic".to_string()],
            },
            sections: vec![collection_semantic_section(
                "topic",
                "Topic",
                StudyGuideSectionContent {
                    overview: "Summary".to_string(),
                    ..Default::default()
                },
            )
            .unwrap()],
            coverage_notes: Vec::new(),
        };
        let before = collection_block_hashes(&content).unwrap();
        content.overview.summary = "Changed overview".to_string();
        let after_overview = collection_block_hashes(&content).unwrap();
        assert_ne!(before["overview"], after_overview["overview"]);
        assert_eq!(before["topic"], after_overview["topic"]);
        content.sections[0].content.overview = "Changed topic".to_string();
        content.sections[0].content_hash =
            hash_text(&serde_json::to_string(&content.sections[0].content).unwrap());
        let after_topic = collection_block_hashes(&content).unwrap();
        assert_ne!(after_overview["topic"], after_topic["topic"]);
    }
}

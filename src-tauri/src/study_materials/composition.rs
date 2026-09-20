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
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tauri::State;

use crate::{
    authoring::card_commands::interactive,
    cards::questions::ValidatedQuestion,
    collections::{self, SourceSetMemberDto, SourceSetSnapshotDto},
    database::Database,
    document_citations,
    document_domain::SourceLocator,
    error::CommandError,
    knowledge::{
        assessment::{self, AssessmentRecord},
        collection_view::{self, CollectionSynthesis, CollectionViewState},
    },
    scheduler::insert_new_schedule,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

use super::{
    load_bundle_status, StudyTargetArtifactKind, StudyTargetBundleStatus, StudyTargetKind,
    StudyTargetRef,
};

const MAX_COLLECTION_SOURCE_NOTES: usize = 10_000;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ComposeStudyTargetBundleInput {
    pub operation_id: String,
}

struct BatchClaim {
    target_id: String,
    target: StudyTargetRef,
    snapshot: SourceSetSnapshotDto,
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
pub(crate) fn compose_study_target_bundle(
    database: State<'_, Database>,
    input: ComposeStudyTargetBundleInput,
) -> Result<StudyTargetBundleStatus, CommandError> {
    compose(database.inner(), input)
}

fn compose(
    database: &Database,
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
                compose_collection_guide(database, &input.operation_id, &claim)
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
                "SELECT b.target_id,b.state,b.source_snapshot_json,
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
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
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
        let (target_id, state, snapshot_json, kind, document_id, collection_id) = row;
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

fn compose_collection_guide(
    database: &Database,
    operation_id: &str,
    claim: &BatchClaim,
) -> Result<(), CommandError> {
    let view = collection_view::refresh_native(database, &claim.target.id)?;
    if view.stale || view.synthesis.snapshot_id != claim.snapshot.snapshot_id {
        return Err(CommandError::new(
            "source_set_changed",
            "The collection changed before its study guide could be published.",
        ));
    }
    publish_collection_guide(database, operation_id, claim, &view)
}

fn publish_collection_guide(
    database: &Database,
    operation_id: &str,
    claim: &BatchClaim,
    view: &CollectionViewState,
) -> Result<(), CommandError> {
    let content_json =
        serde_json::to_string(&view.synthesis).map_err(CommandError::serialization)?;
    let content_hash = hash_text(&content_json);
    database.with_connection(|connection| {
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        collections::ensure_snapshot_current(&tx, &claim.snapshot)?;
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
                    view.synthesis.topics.len(),
                    view.synthesis.topics.len(),
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
            reconcile_collection_overlays(&tx, old_id, &guide_id, &view.synthesis, now)?;
        }
        succeed_job(
            &tx,
            operation_id,
            StudyTargetArtifactKind::StudyGuide,
            &guide_id,
            view.synthesis.topics.len(),
            view.synthesis.topics.len(),
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
    synthesis: &CollectionSynthesis,
    now: i64,
) -> Result<(), CommandError> {
    let hashes = collection_block_hashes(synthesis)?;
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
    synthesis: &CollectionSynthesis,
) -> Result<BTreeMap<String, String>, CommandError> {
    let mut result = BTreeMap::new();
    let overview = serde_json::to_string(&(
        &synthesis.summary,
        &synthesis.learning_objectives,
        &synthesis.relationships,
        &synthesis.gaps,
    ))
    .map_err(CommandError::serialization)?;
    result.insert("overview".to_string(), hash_text(&overview));
    for topic in &synthesis.topics {
        let encoded = serde_json::to_string(topic).map_err(CommandError::serialization)?;
        result.insert(topic.id.clone(), hash_text(&encoded));
    }
    Ok(result)
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
    use crate::{collections::ResolvedSourceRangeDto, document_domain::SourceUnitKind};

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

    #[test]
    fn collection_guide_block_hashes_change_only_with_their_semantic_block() {
        let mut synthesis = CollectionSynthesis {
            version: 1,
            collection_id: "c".repeat(32),
            snapshot_id: "a".repeat(64),
            summary: "Overview".into(),
            learning_objectives: vec!["Objective".into()],
            topics: vec![crate::knowledge::collection_view::CollectionTopic {
                id: "t".repeat(64),
                title: "Topic".into(),
                summary: "Summary".into(),
                summary_support_unit_ids: Vec::new(),
                unit_ids: Vec::new(),
                support_source_ids: Vec::new(),
                support_document_ids: Vec::new(),
                emphasis_source_ids: Vec::new(),
            }],
            relationships: Vec::new(),
            gaps: Vec::new(),
            reduction_status: "native".into(),
        };
        let before = collection_block_hashes(&synthesis).unwrap();
        synthesis.summary = "Changed overview".into();
        let after_overview = collection_block_hashes(&synthesis).unwrap();
        assert_ne!(before["overview"], after_overview["overview"]);
        assert_eq!(before[&"t".repeat(64)], after_overview[&"t".repeat(64)]);
        synthesis.topics[0].summary = "Changed topic".into();
        let after_topic = collection_block_hashes(&synthesis).unwrap();
        assert_ne!(
            after_overview[&"t".repeat(64)],
            after_topic[&"t".repeat(64)]
        );
    }
}

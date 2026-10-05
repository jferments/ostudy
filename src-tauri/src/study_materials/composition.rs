//! Local composition for one durable StudyTarget batch.
//!
//! A collection is a subject, and its flashcards are the decks filed in it,
//! studied live (see `practice_selection`): composition never generates,
//! copies or authors collection flashcards, so every fact keeps one card and
//! one schedule. Document flashcards stay with the individual-document
//! generation and admission pipeline. Collection synthesis organizes the
//! collection guide only.

use std::collections::BTreeMap;

use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;

use crate::{
    ai::{
        concurrency::{run_bounded_provider_queue, QueueControl},
        document_intelligence::{
            generate_collection_study_guide_topic_section, load_document_task_context,
            CollectionStudyGuideAuthoring,
        },
        document_provider, generation, AiRuntimeState,
    },
    collections::{self, SourceSetSnapshotDto},
    database::Database,
    document_domain::SourceLocator,
    document_study_guides::{
        StudyGuideComparison, StudyGuideOverview, StudyGuideSectionContent, StudyGuideTerm,
    },
    error::CommandError,
    knowledge::{
        collection_view::{self, CollectionViewState},
        contracts::KnowledgeKind,
    },
    task_cancellation::CancellationSignal,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

use super::{
    cancellation, load_bundle_status, reuse, subject_has_cards, StudyTargetArtifactKind,
    StudyTargetBundleStatus, StudyTargetKind, StudyTargetRef,
};

mod planning;

pub(crate) const COLLECTION_GUIDE_SCHEMA_VERSION: i64 = 2;
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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CancelStudyTargetBundleInput {
    pub operation_id: String,
}

struct BatchClaim {
    target_id: String,
    target: StudyTargetRef,
    snapshot: SourceSetSnapshotDto,
    provider_authorized: bool,
    claimed: bool,
}

#[tauri::command]
pub(crate) async fn compose_study_target_bundle(
    database: State<'_, Database>,
    runtime: State<'_, AiRuntimeState>,
    input: ComposeStudyTargetBundleInput,
) -> Result<StudyTargetBundleStatus, CommandError> {
    compose(database.inner(), Some(runtime.inner()), input).await
}

#[tauri::command]
pub(crate) fn cancel_study_target_bundle(
    database: State<'_, Database>,
    input: CancelStudyTargetBundleInput,
) -> Result<(), CommandError> {
    validate_entity_id(&input.operation_id, "study target operation")?;
    cancellation::cancel(&input.operation_id)?;
    cancel_active_jobs(database.inner(), &input.operation_id)
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
    let _active = cancellation::Guard::begin(&input.operation_id)?;
    let signal = _active.signal();

    if let Err(error) = database.with_connection(|connection| {
        collections::ensure_snapshot_current(connection, &claim.snapshot)
    }) {
        fail_pending_jobs(database, &input.operation_id, &error)?;
        finalize_batch(database, &input.operation_id)?;
        return status_for_claim(database, &claim);
    }

    for kind in pending_artifacts(database, &input.operation_id)? {
        if signal.is_canceled() {
            cancel_active_jobs(database, &input.operation_id)?;
            break;
        }
        let result = match (claim.target.kind, kind) {
            (StudyTargetKind::Document, StudyTargetArtifactKind::Flashcards) => {
                adopt_document_flashcards(database, &input.operation_id, &claim)
            }
            (StudyTargetKind::Document, StudyTargetArtifactKind::StudyGuide) => {
                adopt_document_guide(database, &input.operation_id, &claim)
            }
            (StudyTargetKind::Collection, StudyTargetArtifactKind::Flashcards) => {
                adopt_subject_flashcards(database, &input.operation_id, &claim)
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
                            &signal,
                        )
                        .await;
                        diagnostics.finish_result(&result);
                        result
                    }
                    Err(error) => Err(error),
                }
            }
        };
        if let Err(error) = result {
            if canceled_error(&error) {
                record_artifact_canceled(database, &input.operation_id, kind, &error)?;
                cancel_active_jobs(database, &input.operation_id)?;
                break;
            }
            record_artifact_failure(database, &input.operation_id, kind, &error)?;
        }
    }

    if signal.is_canceled() {
        cancel_active_jobs(database, &input.operation_id)?;
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

/// A collection's flashcards are its subject's live decks: the job succeeds,
/// without writing a card, when the subject has a live card to study. The
/// target no longer points at a study-bundle deck, so deleting a deck that
/// used to be one never suppresses the live view.
fn adopt_subject_flashcards(
    database: &Database,
    operation_id: &str,
    claim: &BatchClaim,
) -> Result<(), CommandError> {
    database.with_connection(|connection| {
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        collections::ensure_snapshot_current(&tx, &claim.snapshot)?;
        if !subject_has_cards(&tx, &claim.target.id)? {
            return Err(CommandError::new(
                "study_target_source_flashcards_missing",
                "This collection has no flashcards to study yet. Generate flashcards for its documents, or move a deck into it.",
            ));
        }
        let now = unix_time_millis()?;
        tx.execute(
            "UPDATE study_targets
             SET deck_id=NULL,updated_at=?1,revision=revision+1
             WHERE id=?2 AND deck_id IS NOT NULL",
            params![now, claim.target_id],
        )
        .map_err(CommandError::database)?;
        succeed_job(
            &tx,
            operation_id,
            StudyTargetArtifactKind::Flashcards,
            &claim.target.id,
            1,
            1,
            now,
        )?;
        tx.commit().map_err(CommandError::database)
    })
}

async fn compose_collection_guide(
    database: &Database,
    runtime: &AiRuntimeState,
    operation_id: &str,
    claim: &BatchClaim,
    diagnostics: &generation::Context,
    signal: &CancellationSignal,
) -> Result<(), CommandError> {
    ensure_bundle_running(signal)?;
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
    let view =
        collection_view::ensure_reduced(database, runtime, &task, &claim.target.id, signal).await?;
    ensure_bundle_running(signal)?;
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
    let collection_title = collection_title(database, &claim.target.id)?;
    let windows = planning::plan(&task, &collection_title, &view, windows, signal)?;
    let work_total = windows.len().saturating_add(1);
    let section_keys = windows
        .iter()
        .map(|window| {
            let relationship_context = relationship_context(&view, &window.topic_ids);
            reuse::section_key(
                &task,
                &claim.target.id,
                &collection_title,
                window,
                &relationship_context,
            )
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    let manifest_key = reuse::manifest_key(&task, &collection_title, &view, &section_keys)?;
    if let Some(guide_id) = reuse::load_guide(
        database,
        &claim.target_id,
        &claim.target.id,
        &claim.snapshot.snapshot_id,
        &view.analysis_hash,
        &manifest_key,
        task.diagnostics.as_ref(),
    )? {
        database.with_connection(|connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(CommandError::database)?;
            collections::ensure_snapshot_current(&tx, &claim.snapshot)?;
            collection_view::ensure_analysis_current(&tx, &claim.target.id, &view.analysis_hash)?;
            succeed_job(
                &tx,
                operation_id,
                StudyTargetArtifactKind::StudyGuide,
                &guide_id,
                work_total,
                work_total,
                unix_time_millis()?,
            )?;
            tx.commit().map_err(CommandError::database)
        })?;
        return Ok(());
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
    let authoring = CollectionGuideAuthoringContext {
        database,
        runtime,
        task: &task,
        guide_contract: &guide_contract,
        collection_title: &collection_title,
        view: &view,
        signal,
    };
    let mut sections = vec![None; windows.len()];
    let mut completed_sections = 0_usize;
    let mut first_error: Option<(usize, CommandError)> = None;
    run_bounded_provider_queue(
        windows.len(),
        parallelism,
        |index| {
            let authoring = &authoring;
            let windows = &windows;
            async move {
                let result = async {
                    ensure_bundle_running(authoring.signal)?;
                    authoring.task.ensure_current(authoring.database)?;
                    authoring.database.with_connection(|connection| {
                        collections::ensure_snapshot_current(connection, &claim.snapshot)?;
                        collection_view::ensure_analysis_current(
                            connection,
                            &claim.target.id,
                            &authoring.view.analysis_hash,
                        )
                    })?;
                    let section =
                        author_collection_guide_window(authoring, &windows[index]).await?;
                    authoring.task.ensure_current(authoring.database)?;
                    authoring.database.with_connection(|connection| {
                        collections::ensure_snapshot_current(connection, &claim.snapshot)?;
                        collection_view::ensure_analysis_current(
                            connection,
                            &claim.target.id,
                            &authoring.view.analysis_hash,
                        )
                    })?;
                    Ok(section)
                }
                .await;
                (index, result)
            }
        },
        |(index, result)| {
            let control = match result {
                Ok(section) => {
                    sections[index] = Some(section);
                    completed_sections = completed_sections.saturating_add(1);
                    match update_artifact_progress(
                        database,
                        operation_id,
                        StudyTargetArtifactKind::StudyGuide,
                        completed_sections.saturating_add(1),
                    ) {
                        Ok(()) => QueueControl::Continue,
                        Err(error) => {
                            remember_collection_guide_error(&mut first_error, index, error);
                            QueueControl::Stop
                        }
                    }
                }
                Err(error) => {
                    remember_collection_guide_error(&mut first_error, index, error);
                    QueueControl::Stop
                }
            };
            Ok::<QueueControl, CommandError>(control)
        },
    )
    .await?;
    if let Some((_, error)) = first_error {
        return Err(error);
    }
    ensure_bundle_running(signal)?;
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
    ensure_bundle_running(signal)?;
    publish_collection_guide(
        database,
        operation_id,
        claim,
        &view,
        &content,
        work_total,
        &manifest_key,
    )
}

struct CollectionGuideAuthoringContext<'a> {
    database: &'a Database,
    runtime: &'a AiRuntimeState,
    task: &'a crate::ai::document_intelligence::DocumentTaskContext,
    guide_contract: &'a str,
    collection_title: &'a str,
    view: &'a CollectionViewState,
    signal: &'a CancellationSignal,
}

fn remember_collection_guide_error(
    first_error: &mut Option<(usize, CommandError)>,
    index: usize,
    error: CommandError,
) {
    let replace = match first_error.as_ref() {
        None => true,
        Some((failed_index, _)) => index < *failed_index,
    };
    if replace {
        *first_error = Some((index, error));
    }
}

async fn author_collection_guide_window(
    authoring: &CollectionGuideAuthoringContext<'_>,
    window: &collection_view::CollectionGuideWindow,
) -> Result<CollectionGuideSection, CommandError> {
    let relationship_context = relationship_context(authoring.view, &window.topic_ids);
    let cache_key = reuse::section_key(
        authoring.task,
        &authoring.view.synthesis.collection_id,
        authoring.collection_title,
        window,
        &relationship_context,
    )?;
    let generated = if let Some(saved) = reuse::load_section(
        authoring.database,
        &authoring.view.synthesis.collection_id,
        &cache_key,
        authoring.guide_contract,
        &window.catalog,
        authoring.task.diagnostics.as_ref(),
    )? {
        saved
    } else {
        let mut section_task = authoring.task.clone();
        section_task.diagnostic_window_key = Some(cache_key.clone());
        let generated = generate_collection_study_guide_topic_section(
            authoring.runtime,
            &section_task,
            authoring.collection_title,
            CollectionStudyGuideAuthoring {
                topic_title: &window.title,
                topic_summary: &window.topic_context,
                relationship_context: &relationship_context,
                knowledge: &window.catalog,
            },
            authoring.signal,
        )
        .await?;
        section_task.ensure_current(authoring.database)?;
        authoring.database.with_connection(|connection| {
            collection_view::ensure_analysis_current(
                connection,
                &authoring.view.synthesis.collection_id,
                &authoring.view.analysis_hash,
            )
        })?;
        reuse::save_section(
            authoring.database,
            &authoring.view.synthesis.collection_id,
            &cache_key,
            authoring.guide_contract,
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
    manifest_key: &str,
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
                reuse::save_manifest(&tx, guide_id, manifest_key, now)?;
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
        reuse::save_manifest(&tx, &guide_id, manifest_key, now)?;
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

fn canceled_error(error: &CommandError) -> bool {
    matches!(
        error.code.as_str(),
        "ai_request_canceled" | "study_target_generation_canceled"
    )
}

fn ensure_bundle_running(signal: &CancellationSignal) -> Result<(), CommandError> {
    if signal.is_canceled() {
        Err(CommandError::new(
            "study_target_generation_canceled",
            "Collection study-material generation was stopped.",
        ))
    } else {
        Ok(())
    }
}

fn record_artifact_canceled(
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
                 SET state='canceled',safe_error_code=?1,safe_error_message=?2,
                     updated_at=?3,completed_at=?3
                 WHERE batch_id=?4 AND artifact_kind=?5
                   AND state IN('pending','running')",
                params![code, message, now, operation_id, kind.as_db()],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

fn cancel_active_jobs(database: &Database, operation_id: &str) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE study_target_artifact_jobs
                 SET state='canceled',
                     safe_error_code='study_target_generation_canceled',
                     safe_error_message='Collection study-material generation was stopped. Completed checkpoints remain reusable.',
                     updated_at=?1,completed_at=?1
                 WHERE batch_id=?2 AND state IN('pending','running')",
                params![now, operation_id],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
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
        let canceled_count = rows
            .iter()
            .filter(|(state, _, _)| state == "canceled")
            .count();
        let failure_count = rows
            .iter()
            .filter(|(state, _, _)| matches!(state.as_str(), "failed" | "interrupted"))
            .count();
        let state = if failure_count == 0 && canceled_count == 0 {
            "complete"
        } else if success_count > 0 {
            "partial"
        } else if failure_count == 0 && canceled_count > 0 {
            "canceled"
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
        decks::{create_deck_in_connection, CreateDeckInput},
        document_domain::SourceUnitKind,
        knowledge::assessment::policy::instructions::GenerationInstructions,
    };
    use serde_json::json;

    fn test_operation_id(test_case: u16, sequence: u16) -> String {
        format!(
            "{:032x}",
            (u128::from(test_case) << 16) | u128::from(sequence)
        )
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
        collection_id: String,
        source_deck_id: String,
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
                // The collection is a subject holding the document and its deck.
                connection
                    .execute(
                        "INSERT INTO document_collections(
                             id,name,sort_order,created_at,updated_at,revision
                         ) VALUES(?1,'Week 1',1000,1,1,1)",
                        [collection_id.as_str()],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "UPDATE documents SET subject_id=?1 WHERE id=?2",
                        params![collection_id, document_id],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "UPDATE decks SET subject_id=?1 WHERE id=?2",
                        params![collection_id, deck.id],
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
            collection_id,
            source_deck_id,
        }
    }

    fn library_counts(database: &Database) -> (i64, i64, i64, i64) {
        database
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT (SELECT COUNT(*) FROM decks),
                                (SELECT COUNT(*) FROM notes),
                                (SELECT COUNT(*) FROM cards),
                                (SELECT COUNT(*) FROM study_target_card_origins)",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                    )
                    .map_err(CommandError::database)
            })
            .expect("counts should read")
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
                generation_instructions: GenerationInstructions::default(),
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
    fn collection_bundles_take_no_flashcard_instructions() {
        let fixture = bundle_fixture();
        let input =
            |operation_id: String, text: Option<&str>| super::super::BeginStudyTargetBundleInput {
                operation_id,
                target: StudyTargetRef {
                    kind: StudyTargetKind::Collection,
                    id: fixture.collection_id.clone(),
                },
                generation_instructions: GenerationInstructions::parse(text).unwrap(),
                output_policy: super::super::StudyTargetOutputPolicy {
                    flashcards: true,
                    study_guide: false,
                },
                recreate_artifacts: Vec::new(),
            };
        let refused = super::super::begin_bundle(
            &fixture.database,
            input(test_operation_id(9, 1), Some("Focus on cranial nerves.")),
        )
        .unwrap_err();
        assert_eq!(refused.code, "invalid_study_target");

        let operation_id = test_operation_id(9, 2);
        super::super::begin_bundle(&fixture.database, input(operation_id.clone(), None))
            .expect("a plain collection bundle should begin");
        let provider_authorized: i64 = fixture
            .database
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT provider_authorized FROM study_target_batches
                         WHERE operation_id=?1",
                        [&operation_id],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)
            })
            .unwrap();
        // Studying the subject's decks needs no provider.
        assert_eq!(provider_authorized, 0);
    }

    #[test]
    fn collection_flashcards_study_the_subjects_decks_without_copying() {
        let fixture = bundle_fixture();
        let operation_id = test_operation_id(1, 1);
        begin_flashcard_bundle(&fixture, &operation_id);
        // A target from before release 6 still names its study-bundle deck.
        let old_bundle_deck = fixture
            .database
            .with_connection(|connection| {
                let deck = create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: "Week 1 study bundle".to_string(),
                        description: None,
                    },
                )?;
                connection
                    .execute(
                        "UPDATE study_targets SET deck_id=?1 WHERE collection_id=?2",
                        params![deck.id, fixture.collection_id],
                    )
                    .map_err(CommandError::database)?;
                Ok(deck.id)
            })
            .unwrap();
        let before = library_counts(&fixture.database);

        let composed = compose_flashcard_bundle(&fixture, &operation_id);
        assert_eq!(composed.latest_batch_state.as_deref(), Some("complete"));
        assert_eq!(composed.flashcards.state, "succeeded");
        assert_eq!(
            composed.flashcards.artifact_id.as_deref(),
            Some(fixture.collection_id.as_str())
        );
        assert!(composed.deck_id.is_none());
        assert_eq!(library_counts(&fixture.database), before);
        let target_deck: Option<String> = fixture
            .database
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT deck_id FROM study_targets WHERE collection_id=?1",
                        [&fixture.collection_id],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)
            })
            .unwrap();
        assert!(target_deck.is_none());

        // Deleting the old bundle deck is an ordinary deck deletion: the live
        // view no longer names it, so nothing is suppressed.
        fixture
            .database
            .with_connection(|connection| {
                crate::deletion::delete_deck(connection, &old_bundle_deck)
            })
            .unwrap();
        let replay = compose_flashcard_bundle(&fixture, &operation_id);
        assert_eq!(replay.flashcards, composed.flashcards);
        let next = test_operation_id(1, 2);
        begin_flashcard_bundle(&fixture, &next);
        let again = compose_flashcard_bundle(&fixture, &next);
        assert_eq!(again.flashcards.state, "succeeded");
    }

    #[test]
    fn a_collection_without_live_decks_has_no_flashcards_to_study() {
        let fixture = bundle_fixture();
        fixture
            .database
            .with_connection(|connection| {
                connection
                    .execute(
                        "UPDATE decks SET archived_at=20 WHERE id=?1",
                        [&fixture.source_deck_id],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .unwrap();
        let operation_id = test_operation_id(2, 1);
        begin_flashcard_bundle(&fixture, &operation_id);
        let failed = compose_flashcard_bundle(&fixture, &operation_id);
        assert_eq!(failed.latest_batch_state.as_deref(), Some("failed"));
        assert_eq!(failed.flashcards.state, "failed");
        assert_eq!(
            failed.flashcards.safe_error_code,
            "study_target_source_flashcards_missing"
        );
        assert!(failed.deck_id.is_none());
    }

    #[test]
    fn failed_sibling_keeps_collection_flashcards_and_batch_is_partial() {
        let fixture = bundle_fixture();
        let operation_id = test_operation_id(3, 1);
        super::super::begin_bundle(
            &fixture.database,
            super::super::BeginStudyTargetBundleInput {
                operation_id: operation_id.clone(),
                target: StudyTargetRef {
                    kind: StudyTargetKind::Collection,
                    id: fixture.collection_id.clone(),
                },
                generation_instructions: GenerationInstructions::default(),
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
        adopt_subject_flashcards(&fixture.database, &operation_id, &claim)
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

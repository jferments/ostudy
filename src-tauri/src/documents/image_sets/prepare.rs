//! Explicit local source preparation; no provider selection or authoring side
//! effect. Compatible OCR and knowledge survive a repeat or partial failure.

use std::time::Instant;

use serde::Deserialize;

use crate::{
    database::Database, document_ingestion, error::CommandError, paths::AppPaths,
    util::validate_entity_id,
};

use super::{manifest, operations::EditResult, repository, storage};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareImageSetInput {
    pub document_id: String,
    #[serde(default)]
    pub retry_unreadable: bool,
}

pub(crate) fn prepare(
    database: &Database,
    paths: &AppPaths,
    input: &PrepareImageSetInput,
) -> Result<EditResult, CommandError> {
    validate_entity_id(&input.document_id, "image document")?;
    let (manifest, settings, ready) = database.with_connection(|connection| {
        Ok((
            repository::load(connection, &input.document_id)?,
            storage::settings(connection, &input.document_id)?,
            storage::ready(connection, &input.document_id)?,
        ))
    })?;
    let result = EditResult {
        document_id: input.document_id.clone(),
        revision: manifest.revision,
        filter_revision: settings.revision,
    };
    if ready && !input.retry_unreadable {
        return Ok(result);
    }
    let source = paths.resolve_relative(&manifest::manifest_path(
        &manifest.document_id,
        &manifest.fingerprint()?,
    )?);
    let started = Instant::now();
    let resources = crate::system_resources::SystemResourceSnapshot::capture();
    crate::logging::info_target(
        "documents.images",
        "image_set_preparation_started",
        "Native image-set preparation started.",
        serde_json::json!({
            "document": input.document_id.as_str(),
            "members": manifest.members.len(),
            "activeDocumentTasks": crate::task_cancellation::active_document_task_count(),
            "logicalCpus": resources.logical_cpus,
            "availableMemoryBytes": resources.available_memory_bytes,
            "lowMemory": resources.low_memory,
            "thermalLimited": resources.thermal_limited,
        }),
    );
    crate::document_store::mark_document_processing(database, &input.document_id)?;
    let preparation = (|| {
        let extraction = document_ingestion::ingest_image_set(
            database,
            paths,
            &input.document_id,
            &source,
            128,
            |progress| {
                document_ingestion::update_ingestion_progress(
                    database,
                    &input.document_id,
                    progress,
                )
            },
        )?;
        document_ingestion::persist_prepared_image_set(database, &input.document_id, &extraction)?;
        // Status and source publication committed together. A subsequent UI read
        // is reconciliation, not part of this acknowledged durable operation.
        Ok(result)
    })();
    if let Err(error) = &preparation {
        crate::documents::record_document_failure(database, &input.document_id, error);
    } else {
        crate::logging::info_target(
            "documents.images",
            "image_set_preparation_completed",
            "Native image-set preparation completed.",
            serde_json::json!({
                "document": input.document_id.as_str(),
                "durationMs": i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX),
                "members": manifest.members.len(),
            }),
        );
    }
    preparation
}
